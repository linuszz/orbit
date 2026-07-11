use anyhow::Result;
use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};
use futures::StreamExt;
use orbit_protocol::{ClientMessage, SplitDir};
use tracing::debug;

use crate::app::{App, ContextMenuItem, ContextMenuTarget, InputMode, COMMANDS};
use crate::ipc::{IpcClient, IpcWriter};
use crate::tui::{render, OrbitTerminal, AGENT_W, SIDEBAR_COLLAPSED_W, SIDEBAR_W};

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

fn content_area(term_size: ratatui::layout::Rect, app: &App) -> ratatui::layout::Rect {
    let sidebar_w = if app.sidebar_visible {
        SIDEBAR_W
    } else {
        SIDEBAR_COLLAPSED_W
    };
    let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };
    ratatui::layout::Rect {
        x: sidebar_w,
        y: 1, // below tab bar
        width: term_size.width.saturating_sub(sidebar_w + agent_w),
        height: term_size.height.saturating_sub(2), // above status bar
    }
}

async fn execute_command(id: &str, app: &mut App, writer: &IpcWriter) {
    match id {
        "split_h" => {
            app.pending_split = Some((app.active_pane, SplitDir::Horizontal));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    tab_id: app.active_tab_id,
                    pane_id: app.active_pane,
                    direction: SplitDir::Horizontal,
                })
                .await;
        }
        "split_v" => {
            app.pending_split = Some((app.active_pane, SplitDir::Vertical));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    tab_id: app.active_tab_id,
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
                    tab_id: app.active_tab_id,
                    pane_id: app.active_pane,
                })
                .await;
        }
        "scroll_mode" => {
            app.mode = InputMode::Scroll { offset: 1 };
        }
        "new_tab" => {
            let _ = writer.send(ClientMessage::NewTab { name: None }).await;
        }
        "next_tab" => {
            app.next_tab();
            let _ = writer
                .send(ClientMessage::SwitchTab {
                    tab_id: app.active_tab_id,
                })
                .await;
        }
        "prev_tab" => {
            app.prev_tab();
            let _ = writer
                .send(ClientMessage::SwitchTab {
                    tab_id: app.active_tab_id,
                })
                .await;
        }
        "toggle_sidebar" => app.sidebar_visible = !app.sidebar_visible,
        "toggle_agent" => app.agent_panel_visible = !app.agent_panel_visible,
        "detach" => app.should_quit = true,
        "help" => app.show_help = true,
        _ => {}
    }

    app.needs_redraw = true;
}

async fn execute_context_action(
    id: &str,
    target: &ContextMenuTarget,
    app: &mut App,
    writer: &IpcWriter,
) {
    match id {
        "new_tab" => {
            let _ = writer.send(ClientMessage::NewTab { name: None }).await;
        }
        "close_tab" => {
            if let ContextMenuTarget::Tab(tab_id) = target {
                if app.tabs.len() > 1 {
                    let _ = writer
                        .send(ClientMessage::CloseTab { tab_id: *tab_id })
                        .await;
                }
            }
        }
        "split_h" => {
            app.pending_split = Some((app.active_pane, SplitDir::Horizontal));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    tab_id: app.active_tab_id,
                    pane_id: app.active_pane,
                    direction: SplitDir::Horizontal,
                })
                .await;
        }
        "split_v" => {
            app.pending_split = Some((app.active_pane, SplitDir::Vertical));
            let _ = writer
                .send(ClientMessage::SplitPane {
                    tab_id: app.active_tab_id,
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
                    tab_id: app.active_tab_id,
                    pane_id: app.active_pane,
                })
                .await;
        }
        "copy_selection" => {
            if let Some(sel) = app.selection.clone() {
                let pane_id = sel.pane_id;
                if let Some(pane_state) = app.panes.get(&pane_id) {
                    let grid = &pane_state.parser.grid;
                    let (min_col, max_col) = if sel.start.0 <= sel.end.0 {
                        (sel.start.0 as usize, sel.end.0 as usize)
                    } else {
                        (sel.end.0 as usize, sel.start.0 as usize)
                    };
                    let (min_row, max_row) = if sel.start.1 <= sel.end.1 {
                        (sel.start.1 as usize, sel.end.1 as usize)
                    } else {
                        (sel.end.1 as usize, sel.start.1 as usize)
                    };
                    let cols = grid.cols as usize;
                    let max_row_clamped = max_row.min(grid.rows as usize - 1);
                    let max_col_clamped = max_col.min(cols - 1);
                    let mut lines: Vec<String> = Vec::new();
                    for row in min_row..=max_row_clamped {
                        let row_start = row * cols;
                        let line: String = grid.cells
                            [row_start + min_col..=row_start + max_col_clamped]
                            .iter()
                            .map(|c| if c.ch == '\0' { ' ' } else { c.ch })
                            .collect::<String>()
                            .trim_end()
                            .to_string();
                        lines.push(line);
                    }
                    let text = lines.join("\n");
                    let _ = writer
                        .send(orbit_protocol::ClientMessage::CopyToClipboard { text })
                        .await;
                }
                app.selection = None;
            }
        }
        "maximize" | "move_up" | "move_down" | "rename_space" | "close_space" | "new_space" => {}
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

    if app.context_menu.is_some() && key.code == KeyCode::Esc {
        app.close_context_menu();
        return;
    }

    match &mut app.mode {
        InputMode::Normal => {
            if app.selection.is_some() {
                app.selection = None;
                app.needs_redraw = true;
            }
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
                        tab_id: app.active_tab_id,
                        pane_id: app.active_pane,
                    })
                    .await;
                return;
            }
            if let Some(bytes) = key_to_pty_bytes(&key) {
                let _ = writer
                    .send(ClientMessage::PaneInput {
                        tab_id: app.active_tab_id,
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
                        let sidebar_w: u16 = if app.sidebar_visible { SIDEBAR_W } else { SIDEBAR_COLLAPSED_W };
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
                            let pr = rect.height.saturating_sub(2);
                            if let Some(pane) = app.panes.get_mut(&pid) {
                                pane.parser.grid.resize(pc, pr);
                            }
                            let _ = writer
                                .send(ClientMessage::ResizePane {
                                    tab_id: app.active_tab_id,
                                    pane_id: pid,
                                    cols: pc,
                                    rows: pr,
                                })
                                .await;
                        }
                        app.needs_redraw = true;
                    }
                    Some(Ok(Event::Mouse(mouse))) => {
                        let term_size = terminal.size().unwrap_or_default();
                        let term_rect = ratatui::layout::Rect::new(0, 0, term_size.width, term_size.height);
                        handle_mouse(mouse, app, &writer, term_rect).await;
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

async fn handle_mouse(
    mouse: crossterm::event::MouseEvent,
    app: &mut App,
    writer: &IpcWriter,
    term_size: ratatui::layout::Rect,
) {
    if let Some(menu) = app.context_menu.clone() {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let menu_y = menu.y;
                for (i, item) in menu.items.iter().enumerate() {
                    if let ContextMenuItem::Action { .. } = item {
                        if mouse.row == menu_y + i as u16
                            && mouse.column >= menu.x
                            && mouse.column < menu.x + 24
                        {
                            if let Some(ContextMenuItem::Action { id, .. }) = menu.items.get(i) {
                                let id = *id;
                                let target = menu.target.clone();
                                app.context_menu = None;
                                app.needs_redraw = true;
                                execute_context_action(id, &target, app, writer).await;
                            }
                            return;
                        }
                    }
                }
                app.close_context_menu();
            }
            MouseEventKind::Down(MouseButton::Right) => {
                app.close_context_menu();
            }
            MouseEventKind::Moved => {
                let menu_y = menu.y;
                let mut new_selected = 0;
                for (i, item) in menu.items.iter().enumerate() {
                    if mouse.row == menu_y + i as u16
                        && mouse.column >= menu.x
                        && mouse.column < menu.x + 24
                    {
                        if let ContextMenuItem::Action { .. } = item {
                            new_selected = i;
                        }
                    }
                }
                if let Some(ref mut m) = app.context_menu {
                    m.selected = new_selected;
                }
                app.needs_redraw = true;
            }
            _ => {}
        }
        return;
    }

    let sidebar_w: u16 = if app.sidebar_visible {
        SIDEBAR_W
    } else {
        SIDEBAR_COLLAPSED_W
    };
    let agent_w: u16 = if app.agent_panel_visible { 22 } else { 0 };
    let term_w = term_size.width;
    let term_h = term_size.height;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row == 0 {
                // Expand collapsed sidebar when clicking the » row
                if !app.sidebar_visible && mouse.column < SIDEBAR_COLLAPSED_W {
                    app.sidebar_visible = true;
                    app.needs_redraw = true;
                    return;
                }
                // Handle « collapse button first
                if app.sidebar_visible && mouse.column == SIDEBAR_W - 1 {
                    app.sidebar_visible = false;
                    app.needs_redraw = true;
                    return;
                }
                if app.sidebar_visible && mouse.column < sidebar_w {
                    return;
                }
                let tab_x_start = sidebar_w;
                if mouse.column >= tab_x_start {
                    let mut x = tab_x_start;
                    for (i, tab) in app.tabs.iter().enumerate() {
                        let label_len = tab.name.len() as u16 + 2;
                        if mouse.column >= x && mouse.column < x + label_len {
                            app.selection = None;
                            app.active_tab = i;
                            app.active_tab_id = tab.id;
                            if let Some(&first) = app.pane_tree().leaves().first() {
                                app.active_pane = first;
                            }
                            app.needs_redraw = true;
                            return;
                        }
                        x += label_len;
                    }
                    if mouse.column >= x && mouse.column < x + 3 {
                        let _ = writer.send(ClientMessage::NewTab { name: None }).await;
                        app.needs_redraw = true;
                        return;
                    }
                    if mouse.column >= term_w.saturating_sub(14) {
                        app.agent_panel_visible = !app.agent_panel_visible;
                        app.needs_redraw = true;
                        return;
                    }
                }
            }

            // Collapsed sidebar: clicking a space number row switches space and expands
            if !app.sidebar_visible && mouse.column < SIDEBAR_COLLAPSED_W && mouse.row > 0 {
                let space_idx = (mouse.row - 1) as usize;
                if space_idx < app.spaces.len() {
                    app.active_space_idx = space_idx;
                    let space_id = app.spaces[space_idx].space_id;
                    let _ = writer
                        .send(orbit_protocol::ClientMessage::SwitchSpace { space_id })
                        .await;
                }
                app.sidebar_visible = true;
                app.needs_redraw = true;
                return;
            }
            // Sidebar: click a space card
            if app.sidebar_visible && mouse.column < SIDEBAR_W {
                let mut y: u16 = 2; // after header + divider
                for (i, space) in app.spaces.iter().enumerate() {
                    // card occupies 3 rows: name, cwd, stats
                    if mouse.row >= y && mouse.row < y + 3 {
                        let space_id = space.space_id;
                        app.active_space_idx = i;
                        let _ = writer
                            .send(orbit_protocol::ClientMessage::SwitchSpace { space_id })
                            .await;
                        app.needs_redraw = true;
                        return;
                    }
                    y += 3;
                    // gap row between cards (not after last)
                    if i + 1 < app.spaces.len() {
                        y += 1;
                    }
                }
                // [+] New Space button row
                if mouse.row == y {
                    let _ = writer
                        .send(orbit_protocol::ClientMessage::CreateSpace { name: None })
                        .await;
                    app.needs_redraw = true;
                    return;
                }
                // Flight Deck button row
                if mouse.row == y + 1 {
                    app.mode = InputMode::CommandPalette {
                        search: String::new(),
                        selected: 0,
                        search_focused: false,
                    };
                    app.needs_redraw = true;
                    return;
                }
                return;
            }

            let pane_area = content_area(term_size, app);
            let areas = crate::tui::compute_leaf_areas(app.pane_tree(), pane_area);
            for (pid, rect) in &areas {
                if mouse.column >= rect.x
                    && mouse.column < rect.x + rect.width
                    && mouse.row >= rect.y
                    && mouse.row < rect.y + rect.height
                {
                    app.active_pane = *pid;
                    let _ = writer
                        .send(ClientMessage::FocusPane {
                            tab_id: app.active_tab_id,
                            pane_id: *pid,
                        })
                        .await;
                    // Start selection at inner cell coords (account for border),
                    // but only in Normal mode — scroll/command modes must not begin selections.
                    if matches!(app.mode, InputMode::Normal) {
                        let inner_x = rect.x + 1;
                        let inner_y = rect.y + 1;
                        if mouse.column >= inner_x && mouse.row >= inner_y {
                            let col = mouse.column - inner_x;
                            let row = mouse.row - inner_y;
                            app.selection = Some(crate::app::Selection {
                                pane_id: *pid,
                                start: (col, row),
                                end: (col, row),
                                active: true,
                            });
                        } else {
                            app.selection = None;
                        }
                    }
                    app.needs_redraw = true;
                    return;
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if mouse.row == 0 && mouse.column >= sidebar_w {
                // Tab bar right-click: show tab context menu
                let mut x = sidebar_w;
                for tab in app.tabs.iter() {
                    let label_len = tab.name.len() as u16 + 2;
                    if mouse.column >= x && mouse.column < x + label_len {
                        app.open_context_menu(mouse.column, 1, ContextMenuTarget::Tab(tab.id));
                        return;
                    }
                    x += label_len;
                }
                return;
            }
            if mouse.row == 0 {
                return;
            }
            if app.sidebar_visible && mouse.column < sidebar_w {
                if mouse.row >= 2 {
                    app.open_context_menu(mouse.column, mouse.row, ContextMenuTarget::Space);
                } else {
                    app.open_context_menu(mouse.column, mouse.row, ContextMenuTarget::Sidebar);
                }
            } else {
                let pane_area = ratatui::layout::Rect {
                    x: sidebar_w,
                    y: 1,
                    width: term_w.saturating_sub(sidebar_w + agent_w),
                    height: term_h.saturating_sub(3),
                };
                let areas = crate::tui::compute_leaf_areas(app.pane_tree(), pane_area);
                let mut found_pane = None;
                for (pid, rect) in &areas {
                    if mouse.column >= rect.x
                        && mouse.column < rect.x + rect.width
                        && mouse.row >= rect.y
                        && mouse.row < rect.y + rect.height
                    {
                        found_pane = Some(*pid);
                        break;
                    }
                }
                let target = if let Some(pid) = found_pane {
                    ContextMenuTarget::Pane(pid)
                } else {
                    ContextMenuTarget::Pane(app.active_pane)
                };
                app.open_context_menu(mouse.column, mouse.row, target);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Update selection end during drag, clamping to pane inner area
            let drag_info = app
                .selection
                .as_ref()
                .filter(|s| s.active)
                .map(|s| s.pane_id);
            if let Some(sel_pane_id) = drag_info {
                let pane_area = content_area(term_size, app);
                let areas = crate::tui::compute_leaf_areas(app.pane_tree(), pane_area);
                for (pid, rect) in &areas {
                    if *pid == sel_pane_id {
                        let inner_x = rect.x + 1;
                        let inner_y = rect.y + 1;
                        let inner_w = rect.width.saturating_sub(2);
                        let inner_h = rect.height.saturating_sub(2);
                        let col = mouse
                            .column
                            .saturating_sub(inner_x)
                            .min(inner_w.saturating_sub(1));
                        let row = mouse
                            .row
                            .saturating_sub(inner_y)
                            .min(inner_h.saturating_sub(1));
                        if let Some(sel) = &mut app.selection {
                            sel.end = (col, row);
                            app.needs_redraw = true;
                        }
                        break;
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(sel) = &mut app.selection {
                sel.active = false;
                if sel.start == sel.end {
                    app.selection = None;
                }
                app.needs_redraw = true;
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
                if *offset == 0 {
                    app.mode = InputMode::Normal;
                }
                app.needs_redraw = true;
            }
        }
        MouseEventKind::Moved => {
            // Tab bar hover (row 0 of the frame, after the sidebar).
            let sb_w = if app.sidebar_visible {
                SIDEBAR_W
            } else {
                SIDEBAR_COLLAPSED_W
            };
            if mouse.row == 0 && mouse.column >= sb_w {
                let col = mouse.column - sb_w;
                let mut acc: u16 = 0;
                let mut hovered = None;
                for (i, tab) in app.tabs.iter().enumerate() {
                    let w = tab.name.len() as u16 + 2; // " name "
                    if col < acc + w {
                        hovered = Some(i);
                        break;
                    }
                    acc += w;
                }
                // Check new-tab button
                if hovered.is_none() && col < acc + 3 {
                    hovered = Some(app.tabs.len());
                }
                // Check [A] Satellites button (right-aligned, 16 chars wide)
                if hovered.is_none() {
                    let badge_start = term_w.saturating_sub(agent_w + 16);
                    if mouse.column >= badge_start {
                        hovered = Some(app.tabs.len() + 1);
                    }
                }
                if app.tab_hovered != hovered {
                    app.tab_hovered = hovered;
                    app.needs_redraw = true;
                }
            } else if app.tab_hovered.is_some() {
                app.tab_hovered = None;
                app.needs_redraw = true;
            }

            // Sidebar card hover
            if app.sidebar_visible && mouse.column < SIDEBAR_W {
                let mut y: u16 = 2;
                let mut hovered = None;
                for (i, _space) in app.spaces.iter().enumerate() {
                    if mouse.row >= y && mouse.row < y + 3 {
                        hovered = Some(i);
                        break;
                    }
                    y += 3;
                    if i + 1 < app.spaces.len() {
                        y += 1;
                    }
                }
                // Check button rows: [+] New Space then ≡ Command
                if hovered.is_none() {
                    if mouse.row == y {
                        hovered = Some(app.spaces.len());
                    } else if mouse.row == y + 1 {
                        hovered = Some(app.spaces.len() + 1);
                    }
                }
                if app.sidebar_hovered != hovered {
                    app.sidebar_hovered = hovered;
                    app.needs_redraw = true;
                }
            } else if app.sidebar_hovered.is_some() {
                app.sidebar_hovered = None;
                app.needs_redraw = true;
            }
        }
        _ => {}
    }
}
