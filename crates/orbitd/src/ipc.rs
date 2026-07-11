use std::sync::Arc;

use anyhow::{bail, Result};
use arboard::Clipboard;
use orbit_protocol::{ClientMessage, ServerEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

use crate::session::SpaceManager;

type Stream = interprocess::local_socket::tokio::Stream;

async fn write_msg(stream: &mut Stream, msg: &ServerEvent) -> Result<()> {
    let bytes = orbit_protocol::encode_message(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn read_msg(stream: &mut Stream) -> Result<ClientMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > orbit_protocol::MAX_MSG_BYTES {
        bail!("message too large: {len}");
    }
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;
    let (msg, _) = orbit_protocol::decode_message(&data)?;
    Ok(msg)
}

pub async fn handle_client(mut stream: Stream, space_manager: Arc<SpaceManager>) -> Result<()> {
    match read_msg(&mut stream).await {
        Ok(ClientMessage::Hello {
            protocol_version, ..
        }) => {
            if protocol_version != PROTOCOL_VERSION {
                let _ = write_msg(
                    &mut stream,
                    &ServerEvent::ProtocolError {
                        code: 1,
                        message: format!(
                            "version mismatch: client={protocol_version}, server={PROTOCOL_VERSION}"
                        ),
                    },
                )
                .await;
                bail!("protocol version mismatch");
            }
        }
        _ => bail!("expected Hello"),
    }

    write_msg(
        &mut stream,
        &ServerEvent::Welcome {
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            capabilities: orbit_protocol::Capabilities::default(),
            state: space_manager.collect_full_state().await,
        },
    )
    .await?;

    let mut rx = space_manager.event_bus.subscribe();

    loop {
        tokio::select! {
            biased;
            msg = read_msg(&mut stream) => {
                let msg = match msg { Ok(m) => m, Err(e) => { debug!("client read: {e:#}"); break; } };
                match msg {
                    ClientMessage::PaneInput { tab_id, pane_id, data } => {
                        let session = space_manager.active_session().await;
                        session.send_input(tab_id, pane_id, data).await;
                    }
                    ClientMessage::ClosePane { tab_id, pane_id } => {
                        let session = space_manager.active_session().await;
                        session.close_pane(tab_id, pane_id).await;
                    }
                    ClientMessage::SplitPane { tab_id, direction, .. } => {
                        let session = space_manager.active_session().await;
                        if let Err(e) = session.split_pane(tab_id, direction).await {
                            tracing::warn!("split: {e:#}");
                        }
                    }
                    ClientMessage::ResizePane { tab_id, pane_id, cols, rows } => {
                        let session = space_manager.active_session().await;
                        session.resize_pane(tab_id, pane_id, cols, rows).await;
                    }
                    ClientMessage::FocusPane { tab_id, pane_id } => {
                        let session = space_manager.active_session().await;
                        session.focus_pane(tab_id, pane_id).await;
                    }
                    ClientMessage::NewTab { name } => {
                        let session = space_manager.active_session().await;
                        if let Err(e) = session.new_tab(name).await {
                            tracing::warn!("new_tab: {e:#}");
                        }
                    }
                    ClientMessage::CloseTab { tab_id } => {
                        let session = space_manager.active_session().await;
                        session.close_tab(tab_id).await;
                    }
                    ClientMessage::SwitchTab { tab_id } => {
                        let session = space_manager.active_session().await;
                        session.switch_tab(tab_id).await;
                    }
                    ClientMessage::RequestFullState => {
                        let s = space_manager.collect_full_state().await;
                        let _ = write_msg(&mut stream, &ServerEvent::Welcome {
                            server_version: env!("CARGO_PKG_VERSION").to_string(),
                            protocol_version: PROTOCOL_VERSION,
                            capabilities: orbit_protocol::Capabilities::default(),
                            state: s,
                        }).await;
                    }
                    ClientMessage::CopyToClipboard { text } => {
                        if let Ok(mut cb) = Clipboard::new() {
                            let _ = cb.set_text(text);
                        }
                    }
                    ClientMessage::CreateSpace { name } => {
                        if let Err(e) = space_manager.create_space(name).await {
                            tracing::warn!("create_space: {e:#}");
                        }
                    }
                    ClientMessage::SwitchSpace { space_id } => {
                        if let Err(e) = space_manager.switch_space(space_id).await {
                            tracing::warn!("switch_space: {e:#}");
                        }
                    }
                    ClientMessage::AgentAbort { agent_id } => {
                        space_manager.agent_registry.abort_agent(agent_id).await;
                    }
                    ClientMessage::AgentRemove { agent_id } => {
                        space_manager.agent_registry.remove_agent(agent_id).await;
                    }
                    ClientMessage::AgentSkip { agent_id } => {
                        let agents = space_manager.agent_registry.get_agents().await;
                        if let Some(agent) = agents.iter().find(|a| a.id == agent_id) {
                            if let Some(pane_id) = agent.pane_id {
                                let session = space_manager.active_session().await;
                                let active_tab_id = *session.active_tab.read().await;
                                session.send_input(active_tab_id, pane_id, b"\r".to_vec()).await;
                            }
                        }
                    }
                    ClientMessage::AgentRespond { agent_id, response } => {
                        let agents = space_manager.agent_registry.get_agents().await;
                        if let Some(agent) = agents.iter().find(|a| a.id == agent_id) {
                            if let Some(pane_id) = agent.pane_id {
                                let session = space_manager.active_session().await;
                                let active_tab_id = *session.active_tab.read().await;
                                let input = format!("{}\r", response.trim());
                                session.send_input(active_tab_id, pane_id, input.into_bytes()).await;
                            }
                        }
                    }
                    ClientMessage::AgentLaunch { config } => {
                        let session = space_manager.active_session().await;
                        let active_tab_id = *session.active_tab.read().await;
                        match session.split_pane(active_tab_id, orbit_protocol::SplitDir::Horizontal).await {
                            Ok(new_pane_id) => {
                                let cmd = format!("{}\r", config.name.trim());
                                tokio::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                                    session.send_input(active_tab_id, new_pane_id, cmd.into_bytes()).await;
                                });
                            }
                            Err(e) => tracing::warn!("AgentLaunch split failed: {e:#}"),
                        }
                    }
                    _ => {}
                }
            }
            recv = rx.recv() => {
                let event = match recv {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => { debug!("lagged {n}"); continue; }
                    Err(_) => break,
                };
                if write_msg(&mut stream, &event).await.is_err() { break; }
            }
        }
    }
    debug!("client disconnected");
    Ok(())
}
