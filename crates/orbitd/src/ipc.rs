use std::sync::Arc;

use anyhow::{bail, Result};
use arboard::Clipboard;
use orbit_protocol::{ClientMessage, ServerEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

use crate::session::SessionState;

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

pub async fn handle_client(mut stream: Stream, session: Arc<SessionState>) -> Result<()> {
    match read_msg(&mut stream).await {
        Ok(ClientMessage::Hello {
            protocol_version, ..
        }) => {
            if protocol_version != PROTOCOL_VERSION {
                let _ = write_msg(
                    &mut stream,
                    &ServerEvent::ProtocolError {
                        code: 1,
                        message: format!("version mismatch: client={protocol_version}, server={PROTOCOL_VERSION}"),
                    },
                ).await;
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
            state: session.collect_full_state().await,
        },
    )
    .await?;

    let mut rx = session.event_bus.subscribe();

    loop {
        tokio::select! {
            biased;
            msg = read_msg(&mut stream) => {
                let msg = match msg { Ok(m) => m, Err(e) => { debug!("client read: {e:#}"); break; } };
                match msg {
                    ClientMessage::PaneInput { tab_id, pane_id, data } => session.send_input(tab_id, pane_id, data).await,
                    ClientMessage::ClosePane { tab_id, pane_id } => session.close_pane(tab_id, pane_id).await,
                    ClientMessage::SplitPane { tab_id, direction, .. } => {
                        if let Err(e) = session.split_pane(tab_id, direction).await { tracing::warn!("split: {e:#}"); }
                    }
                    ClientMessage::ResizePane { tab_id, pane_id, cols, rows } => session.resize_pane(tab_id, pane_id, cols, rows).await,
                    ClientMessage::FocusPane { tab_id, pane_id } => session.focus_pane(tab_id, pane_id).await,
                    ClientMessage::NewTab { name } => {
                        if let Err(e) = session.new_tab(name).await {
                            tracing::warn!("new_tab: {e:#}");
                        }
                    }
                    ClientMessage::CloseTab { tab_id } => session.close_tab(tab_id).await,
                    ClientMessage::SwitchTab { tab_id } => session.switch_tab(tab_id).await,
                    ClientMessage::RequestFullState => {
                        let s = session.collect_full_state().await;
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
                    ClientMessage::SwitchSpace { space_id } => {
                        session.switch_space(space_id).await;
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
