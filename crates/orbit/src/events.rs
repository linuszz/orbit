use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use orbit_protocol::{ClientMessage, SplitDir};
use tracing::debug;

use crate::app::{App, InputMode};
use crate::ipc::{IpcClient, IpcWriter};
use crate::tui::{render, OrbitTerminal};

fn is_prefix_key(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b')
}

fn key_to_pty_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    match (key.modifiers, key.code) {
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
        // handled separately for pane focus
        (_, KeyCode::Tab) => None,
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
            if app.show_help {
                app.show_help = false;
                app.needs_redraw = true;
                return;
            }
            if is_prefix_key(&key) {
                app.mode = InputMode::Prefix;
                app.needs_redraw = true;
                return;
            }
            if key.code == KeyCode::Tab && app.pane_tree().leaves().len() > 1 {
                app.cycle_focus();
                let _ = writer
                    .send(ClientMessage::FocusPane {
                        pane_id: app.active_pane,
                    })
                    .await;
                return;
            }
            if let Some(bytes) = key_to_pty_bytes(&key) {
                let _ = writer
                    .send(ClientMessage::PaneInput {
                        pane_id: app.active_pane,
                        data: bytes,
                    })
                    .await;
            }
        }
        InputMode::Scroll { ref mut offset } => {
            let pane_height = app
                .panes
                .get(&app.active_pane)
                .map(|p| p.parser.grid.rows as usize)
                .unwrap_or(24);
            let scrollback_len = app
                .panes
                .get(&app.active_pane)
                .map(|p| p.scrollback.len())
                .unwrap_or(0);
            let max_offset = scrollback_len + pane_height;

            match key.code {
                KeyCode::Up | KeyCode::Char('k') => *offset = (*offset + 1).min(max_offset),
                KeyCode::Down | KeyCode::Char('j') => {
                    *offset = offset.saturating_sub(1);
                    if *offset == 0 {
                        app.mode = InputMode::Normal;
                    }
                }
                KeyCode::PageUp => *offset = (*offset + pane_height).min(max_offset),
                KeyCode::PageDown => {
                    *offset = offset.saturating_sub(pane_height);
                    if *offset == 0 {
                        app.mode = InputMode::Normal;
                    }
                }
                KeyCode::Char('G') => *offset = 0,
                KeyCode::Char('g') => *offset = max_offset,
                KeyCode::Char('q') | KeyCode::Esc => {
                    app.mode = InputMode::Normal;
                }
                _ => {}
            }
            app.needs_redraw = true;
        }
        InputMode::Prefix => {
            if is_prefix_key(&key) || key.code == KeyCode::Esc {
                app.mode = InputMode::Normal;
                app.needs_redraw = true;
                return;
            }
            match key.code {
                KeyCode::Char('x') => {
                    let leaves = app.pane_tree().leaves();
                    let pane_id = app.active_pane;
                    if leaves.len() <= 1 {
                        app.should_quit = true;
                    }
                    let _ = writer.send(ClientMessage::ClosePane { pane_id }).await;
                }
                KeyCode::Char('d') => {
                    app.should_quit = true;
                }
                KeyCode::Char('h') => {
                    app.pending_split = Some((app.active_pane, SplitDir::Horizontal));
                    let _ = writer
                        .send(ClientMessage::SplitPane {
                            pane_id: app.active_pane,
                            direction: SplitDir::Horizontal,
                        })
                        .await;
                }
                KeyCode::Char('v') => {
                    app.pending_split = Some((app.active_pane, SplitDir::Vertical));
                    let _ = writer
                        .send(ClientMessage::SplitPane {
                            pane_id: app.active_pane,
                            direction: SplitDir::Vertical,
                        })
                        .await;
                }
                KeyCode::Char('b') => {
                    app.sidebar_visible = !app.sidebar_visible;
                }
                KeyCode::Char('a') => {
                    app.agent_panel_visible = !app.agent_panel_visible;
                }
                KeyCode::Char('[') => {
                    app.mode = InputMode::Scroll { offset: 1 };
                    app.needs_redraw = true;
                    return;
                }
                KeyCode::Char('c') => {
                    app.pending_new_tab = true;
                    let _ = writer
                        .send(ClientMessage::SplitPane {
                            pane_id: app.active_pane,
                            direction: SplitDir::Horizontal,
                        })
                        .await;
                }
                KeyCode::Char('n') => {
                    app.next_tab();
                }
                KeyCode::Char('p') => {
                    app.prev_tab();
                }
                KeyCode::Char('?') => {
                    app.show_help = !app.show_help;
                    return;
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
                        let total_cols = cols.saturating_sub(sidebar_w).max(20);
                        let total_rows = rows.saturating_sub(3).max(5);
                        let pane_area = ratatui::layout::Rect {
                            x: 0,
                            y: 0,
                            width: total_cols,
                            height: total_rows,
                        };
                        let areas = crate::tui::compute_leaf_areas(app.pane_tree(), pane_area);
                        for (pid, rect) in areas {
                            let pc = rect.width;
                            let pr = rect.height.saturating_sub(1);
                            if let Some(pane) = app.panes.get_mut(&pid) {
                                pane.parser.grid.resize(pc, pr);
                            }
                            let _ = writer
                                .send(ClientMessage::ResizePane {
                                    pane_id: pid,
                                    cols: pc,
                                    rows: pr,
                                })
                                .await;
                        }
                        app.needs_redraw = true;
                    }
                    Some(Err(e)) => debug!("event stream error: {e}"),
                    None => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
