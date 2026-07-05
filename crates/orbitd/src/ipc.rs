use std::sync::Arc;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use orbit_protocol::{Capabilities, ClientMessage, ServerEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, warn};

use crate::session::SessionState;

async fn write_msg(stream: &mut LocalSocketStream, msg: &ServerEvent) -> Result<()> {
    let bytes = orbit_protocol::encode_message(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn read_msg(stream: &mut LocalSocketStream) -> Result<ClientMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > orbit_protocol::MAX_MSG_BYTES {
        bail!("message too large: {len} bytes");
    }
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;
    let (msg, _) = orbit_protocol::decode_message(&data)?;
    Ok(msg)
}

pub async fn handle_client(
    mut stream: LocalSocketStream,
    session: Arc<SessionState>,
) -> Result<()> {
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
        _ => {
            bail!("expected Hello as first message");
        }
    };

    let welcome = ServerEvent::Welcome {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: PROTOCOL_VERSION,
        capabilities: Capabilities::default(),
        state: session.collect_full_state(),
    };
    write_msg(&mut stream, &welcome).await?;
    debug!("client connected and welcomed");

    let mut broadcast_rx = session.event_bus.subscribe();
    let mut buf = Vec::new();

    loop {
        tokio::select! {
            biased;
            msg = read_msg(&mut stream) => {
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        debug!("client read error: {e:#}");
                        break;
                    }
                };
                match msg {
                    ClientMessage::PaneInput { data, .. } => {
                        let _ = session.pty_input_tx.send(data).await;
                    }
                    ClientMessage::ClosePane { .. } => {
                        debug!("client requested pane close");
                        break;
                    }
                    ClientMessage::RequestFullState => {
                        let state = session.collect_full_state();
                        let _ = write_msg(
                            &mut stream,
                            &ServerEvent::Welcome {
                                server_version: env!("CARGO_PKG_VERSION").to_string(),
                                protocol_version: PROTOCOL_VERSION,
                                capabilities: Capabilities::default(),
                                state,
                            },
                        ).await;
                    }
                    ClientMessage::ResizePane { cols, rows, .. } => {
                        debug!("resize request: {cols}x{rows} (not yet implemented)");
                    }
                    _ => {}
                }
            }
            recv_result = broadcast_rx.recv() => {
                let event = match recv_result {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("client lagged by {n} events");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("event bus closed");
                        break;
                    }
                };
                buf.clear();
                let bytes = orbit_protocol::encode_message(&event)
                    .context("encode event")?;
                buf = bytes;
                if stream.write_all(&buf).await.is_err() {
                    debug!("client write failed, disconnecting");
                    break;
                }
            }
        }
    }

    debug!("client disconnected");
    Ok(())
}
