use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use orbit_protocol::ClientMessage;
use tracing::debug;

use crate::app::{App, InputMode};
use crate::ipc::{IpcClient, IpcWriter};
use crate::tui::{render, OrbitTerminal};

fn is_prefix_key(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b')
}

fn key_to_pty_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    let code = key.code;
    let mods = key.modifiers;
    match (mods, code) {
        (m, KeyCode::Char(c))
            if m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(vec![(c as u8) & 0x1f])
        }
        (m, KeyCode::Char(c)) if m.contains(KeyModifiers::ALT) => {
            let mut bytes = vec![0x1b];
            bytes.extend(c.to_string().as_bytes());
            Some(bytes)
        }
        (_, KeyCode::Char(c)) => Some(c.to_string().into_bytes()),
        (_, KeyCode::Enter) => Some(b"\r".to_vec()),
        (_, KeyCode::Backspace) => Some(b"\x7f".to_vec()),
        (_, KeyCode::Tab) => Some(b"\t".to_vec()),
        (_, KeyCode::Up) => Some(b"\x1b[A".to_vec()),
        (_, KeyCode::Down) => Some(b"\x1b[B".to_vec()),
        (_, KeyCode::Right) => Some(b"\x1b[C".to_vec()),
        (_, KeyCode::Left) => Some(b"\x1b[D".to_vec()),
        (_, KeyCode::Home) => Some(b"\x1b[H".to_vec()),
        (_, KeyCode::End) => Some(b"\x1b[F".to_vec()),
        (_, KeyCode::PageUp) => Some(b"\x1b[5~".to_vec()),
        (_, KeyCode::PageDown) => Some(b"\x1b[6~".to_vec()),
        (_, KeyCode::Delete) => Some(b"\x1b[3~".to_vec()),
        (_, KeyCode::Esc) => Some(b"\x1b".to_vec()),
        _ => None,
    }
}

async fn handle_key(key: KeyEvent, app: &mut App, writer: &IpcWriter) {
    match app.mode {
        InputMode::Normal => {
            if is_prefix_key(&key) {
                app.mode = InputMode::Prefix;
                app.needs_redraw = true;
                return;
            }
            if let Some(bytes) = key_to_pty_bytes(&key) {
                let _ = writer
                    .send(ClientMessage::PaneInput {
                        pane_id: app.pane_id,
                        data: bytes,
                    })
                    .await;
            }
        }
        InputMode::Prefix => {
            if is_prefix_key(&key) || key.code == KeyCode::Esc {
                app.mode = InputMode::Normal;
                app.needs_redraw = true;
                return;
            }
            match key.code {
                KeyCode::Char('x') => {
                    debug!("close pane requested");
                    let _ = writer
                        .send(ClientMessage::ClosePane {
                            pane_id: app.pane_id,
                        })
                        .await;
                    app.should_quit = true;
                }
                KeyCode::Char('d') => {
                    debug!("detach requested");
                    app.should_quit = true;
                }
                KeyCode::Char('b') => {
                    app.sidebar_visible = !app.sidebar_visible;
                }
                KeyCode::Char('a') => {
                    app.agent_panel_visible = !app.agent_panel_visible;
                }
                _ => {}
            }
            app.mode = InputMode::Normal;
            app.needs_redraw = true;
        }
    }
}

pub async fn run(app: &mut App, ipc: IpcClient, terminal: &mut OrbitTerminal) -> Result<()> {
    let (writer, mut reader) = ipc.into_split();
    let mut event_stream = EventStream::new();

    app.needs_redraw = true;

    loop {
        if app.needs_redraw {
            terminal.draw(|frame| render(frame, app))?;
            app.needs_redraw = false;
        }

        if app.should_quit {
            break;
        }

        tokio::select! {
            biased;

            event_result = reader.recv() => {
                match event_result {
                    Ok(event) => app.handle_server_event(&event),
                    Err(e) => {
                        debug!("server disconnected: {e:#}");
                        app.server_connected = false;
                        app.should_quit = true;
                    }
                }
            }

            raw = event_stream.next() => {
                match raw {
                    Some(Ok(Event::Key(key))) => {
                        handle_key(key, app, &writer).await;
                    }
                    Some(Ok(Event::Resize(cols, rows))) => {
                        let sidebar_w: u16 = if app.sidebar_visible { 14 } else { 2 };
                        let pane_cols = cols.saturating_sub(sidebar_w).max(20);
                        let pane_rows = rows.saturating_sub(3).max(5);
                        app.parser.grid.resize(pane_cols, pane_rows);
                        let _ = writer
                            .send(ClientMessage::ResizePane {
                                pane_id: app.pane_id,
                                cols: pane_cols,
                                rows: pane_rows,
                            })
                            .await;
                        app.needs_redraw = true;
                    }
                    Some(Err(e)) => {
                        debug!("event stream error: {e}");
                    }
                    None => {
                        debug!("event stream closed");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
