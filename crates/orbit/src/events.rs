use anyhow::Result;
use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};
use futures::StreamExt;
use orbit_protocol::{ClientMessage, SplitDir};
use tracing::debug;

use crate::app::{App, InputMode, COMMANDS};
use crate::ipc::{IpcClient, IpcWriter};
use crate::tui::{render, OrbitTerminal};

fn is_prefix_key(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b')
}

fn filtered_indices(search: &str) -> Vec<usize> {
    if search.is_empty() {
        return (0..COMMANDS.len()).collect();
    }
    let s = search.to_lowercase();
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, c)| c.label.to_lowercase().contains(&s))
        .map(|(i, _)| i)
        .collect()
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

async fn execute_command(id: &str, app: &mut App, writer: &IpcWriter) {
    match id {
        "split_h" => {
            app.pending_split = Some((app.active_pane, SplitDir::Horizontal));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    pane_id: app.active_pane,
                    direction: SplitDir::Horizontal,
                })
                .await;
        }
        "split_v" => {
            app.pending_split = Some((app.active_pane, SplitDir::Vertical));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    pane_id: app.active_pane,
                    direction: SplitDir::Vertical,
                })
                .await;
        }
        "close_pane" => {
            if app.pane_tree().leaves().len() <= 1 {
                app.should_quit = true;
            }
            let _ = writer
                .send(ClientMessage::ClosePane {
                    pane_id: app.active_pane,
                })
                .await;
        }
        "new_tab" => {
            app.pending_new_tab = true;
            let _ = writer
                .send(ClientMessage::SplitPane {
                    pane_id: app.active_pane,
                    direction: SplitDir::Horizontal,
                })
                .await;
        }
        "next_tab" => app.next_tab(),
        "prev_tab" => app.prev_tab(),
        "scroll_mode" => {
            app.mode = InputMode::Scroll { offset: 1 };
        }
        "toggle_sidebar" => app.sidebar_visible = !app.sidebar_visible,
        "toggle_agent" => app.agent_panel_visible = !app.agent_panel_visible,
        "detach" => app.should_quit = true,
        "help" => app.show_help = true,
        _ => {}
    }
    app.needs_redraw = true;
}

async fn handle_key(key: KeyEvent, app: &mut App, writer: &IpcWriter) {
    if app.show_help {
        app.show_help = false;
        app.needs_redraw = true;
        return;
    }

    match &mut app.mode {
        InputMode::Normal => {
            if is_prefix_key(&key) {
                app.mode = InputMode::CommandPalette {
                    search: String::new(),
                    selected: 0,
                    search_focused: false,
                };
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
        InputMode::CommandPalette {
            search, selected, ..
        } => {
            if is_prefix_key(&key) || key.code == KeyCode::Esc {
                if search.is_empty() {
                    app.mode = InputMode::Normal;
                } else {
                    search.clear();
                    *selected = 0;
                }
                app.needs_redraw = true;
                return;
            }

            let filtered = filtered_indices(search);

            match key.code {
                KeyCode::Up => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down if !filtered.is_empty() => {
                    *selected = (*selected + 1).min(filtered.len() - 1);
                }
                KeyCode::Enter => {
                    if let Some(&cmd_idx) = filtered.get(*selected) {
                        let cmd_id = COMMANDS[cmd_idx].id;
                        app.mode = InputMode::Normal;
                        execute_command(cmd_id, app, writer).await;
                        return;
                    }
                }
                KeyCode::Backspace => {
                    search.pop();
                    *selected = 0;
                }
                KeyCode::Char(c) => {
                    let sc = c.to_string();
                    let shortcut_cmd = search
                        .is_empty()
                        .then(|| COMMANDS.iter().find(|cmd| cmd.shortcut == sc))
                        .flatten();
                    if let Some(cmd) = shortcut_cmd {
                        app.mode = InputMode::Normal;
                        execute_command(cmd.id, app, writer).await;
                        return;
                    }
                    search.push(c);
                    *selected = 0;
                }
                _ => {}
            }
            app.needs_redraw = true;
        }
        InputMode::Scroll { offset } => {
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
                    Some(Ok(Event::Mouse(mouse))) => {
                        match mouse.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                let sidebar_w: u16 = if app.sidebar_visible { 14 } else { 2 };
                                let agent_w: u16 = if app.agent_panel_visible { 22 } else { 0 };
                                let term = terminal.size().unwrap_or_default();
                                let pane_area = ratatui::layout::Rect {
                                    x: sidebar_w,
                                    y: 1,
                                    width: term.width.saturating_sub(sidebar_w + agent_w),
                                    height: term.height.saturating_sub(3),
                                };
                                let areas = crate::tui::compute_leaf_areas(app.pane_tree(), pane_area);
                                for (pid, rect) in &areas {
                                    if mouse.column >= rect.x
                                        && mouse.column < rect.x + rect.width
                                        && mouse.row >= rect.y
                                        && mouse.row < rect.y + rect.height
                                    {
                                        app.active_pane = *pid;
                                        let _ = writer.send(ClientMessage::FocusPane { pane_id: *pid }).await;
                                        app.needs_redraw = true;
                                        break;
                                    }
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                if let InputMode::Scroll { offset } = &mut app.mode {
                                    *offset += 3;
                                    app.needs_redraw = true;
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                if let InputMode::Scroll { offset } = &mut app.mode {
                                    *offset = offset.saturating_sub(3);
                                    if *offset == 0 { app.mode = InputMode::Normal; }
                                    app.needs_redraw = true;
                                }
                            }
                            _ => {}
                        }
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
