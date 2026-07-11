pub mod theme;
pub mod widgets;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use orbit_protocol::{PaneId, SplitDir, TermColor};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
    Frame,
};
use std::io::{self, Stdout};

use crate::app::{App, InputMode, PaneState, Selection};
use orbit_protocol::Cell;
use orbit_protocol::PaneLayout;
use theme::*;

pub type OrbitTerminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub fn setup_terminal() -> io::Result<OrbitTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    ratatui::Terminal::new(CrosstermBackend::new(stdout))
}

pub fn restore_terminal(terminal: &mut OrbitTerminal) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}

pub fn term_color(c: &TermColor) -> Color {
    match c {
        TermColor::Default => Color::Reset,
        TermColor::Ansi(n) => Color::Indexed(*n),
        TermColor::Ansi256(n) => Color::Indexed(*n),
        TermColor::Rgb(r, g, b) => Color::Rgb(*r, *g, *b),
    }
}

pub const SIDEBAR_W: u16 = 20;
pub const SIDEBAR_COLLAPSED_W: u16 = 2;
pub const AGENT_W: u16 = 22;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let sidebar_w = if app.sidebar_visible {
        SIDEBAR_W
    } else {
        SIDEBAR_COLLAPSED_W
    };
    let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };

    let cols = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Length(sidebar_w),
        ratatui::layout::Constraint::Fill(1),
        ratatui::layout::Constraint::Length(agent_w),
    ])
    .split(area);

    widgets::spaces_sidebar::render(frame, cols[0], app);

    let right = Rect {
        x: cols[1].x,
        y: cols[1].y,
        width: cols[1].width,
        height: cols[1].height,
    };

    let rows = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Fill(1),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(right);

    widgets::tab_bar::render(frame, rows[0], app);
    render_pane_tree(frame, rows[1], app.pane_tree(), app);
    widgets::status_bar::render(frame, rows[2], app);

    if app.agent_panel_visible {
        widgets::agent_monitor::render(frame, cols[2], app);
    }

    if app.show_help {
        render_help_overlay(frame, area);
    }

    if app.context_menu.is_some() {
        widgets::context_menu::render(frame, area, app);
    }

    if matches!(app.mode, crate::app::InputMode::CommandPalette { .. }) {
        widgets::command_palette::render(frame, area, app);
    }

    if app.launch_modal.is_some() {
        widgets::launch_modal::render(frame, area, app);
    }

    if app.eclipse_modal.is_some() {
        widgets::eclipse_modal::render(frame, area, app);
    }
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let dim = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 14)));
    frame.render_widget(dim, area);

    let help_w = 48u16.min(area.width.saturating_sub(4));
    let help_h = 18u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width - help_w) / 2;
    let y = area.y + (area.height - help_h) / 2;
    let help_area = Rect {
        x,
        y,
        width: help_w,
        height: help_h,
    };

    let block = Block::default()
        .style(Style::default().bg(BG_SECONDARY).fg(FG_PRIMARY))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    frame.render_widget(block, help_area);

    let lines = vec![
        ("Ctrl+B", "prefix key (enter command mode)"),
        ("  h", "split pane horizontal (left|right)"),
        ("  v", "split pane vertical (top/bottom)"),
        ("  c", "new tab"),
        ("  n / p", "next / previous tab"),
        ("  [", "enter scroll mode"),
        ("  x", "close current pane"),
        ("  d", "detach (quit, keep session)"),
        ("  b", "toggle sidebar"),
        ("  a", "toggle agent monitor"),
        ("  ?", "this help"),
        ("Tab", "cycle focus between panes"),
        ("Scroll: k/j/PgUp/PgDn/g/G/q", ""),
    ];

    let mut y_off = 1u16;
    let title = ratatui::text::Line::from(vec![ratatui::text::Span::styled(
        " Orbit — Keyboard Reference ",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )]);
    frame.render_widget(
        title,
        Rect {
            x: help_area.x + 1,
            y: help_area.y + y_off,
            width: help_w - 2,
            height: 1,
        },
    );
    y_off += 2;

    for (key, desc) in &lines {
        let line = if desc.is_empty() {
            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                *key,
                Style::default().fg(ACCENT_IDLE),
            )])
        } else {
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(format!(" {:<14}", key), Style::default().fg(ACCENT)),
                ratatui::text::Span::styled(*desc, Style::default().fg(FG_SECONDARY)),
            ])
        };
        frame.render_widget(
            line,
            Rect {
                x: help_area.x + 1,
                y: help_area.y + y_off,
                width: help_w - 2,
                height: 1,
            },
        );
        y_off += 1;
    }

    y_off += 1;
    let hint = ratatui::text::Line::from(vec![ratatui::text::Span::styled(
        " Press any key to close ",
        Style::default().fg(FG_MUTED),
    )]);
    frame.render_widget(
        hint,
        Rect {
            x: help_area.x + 1,
            y: help_area.y + y_off,
            width: help_w - 2,
            height: 1,
        },
    );
}

pub fn compute_leaf_areas(node: &PaneLayout, area: Rect) -> Vec<(PaneId, Rect)> {
    match node {
        PaneLayout::Leaf(pid) => vec![(*pid, area)],
        PaneLayout::Split {
            direction,
            first,
            second,
        } => {
            let (first_area, second_area) = split_area(area, direction);
            let mut v = compute_leaf_areas(first, first_area);
            v.extend(compute_leaf_areas(second, second_area));
            v
        }
    }
}

fn render_pane_tree(frame: &mut Frame, area: Rect, node: &PaneLayout, app: &App) {
    match node {
        PaneLayout::Leaf(pid) => {
            render_single_pane(frame, area, *pid, app);
        }
        PaneLayout::Split {
            direction,
            first,
            second,
        } => {
            let (first_area, second_area) = split_area(area, direction);

            render_pane_tree(frame, first_area, first, app);
            render_pane_tree(frame, second_area, second, app);
        }
    }
}

fn split_area(area: Rect, dir: &SplitDir) -> (Rect, Rect) {
    match dir {
        SplitDir::Horizontal => {
            let total = area.width;
            let half = total / 2;
            let first = Rect {
                width: half,
                ..area
            };
            let second = Rect {
                x: area.x + half,
                width: total - half,
                ..area
            };
            (first, second)
        }
        SplitDir::Vertical => {
            let total = area.height;
            let half = total / 2;
            let first = Rect {
                height: half,
                ..area
            };
            let second = Rect {
                y: area.y + half,
                height: total - half,
                ..area
            };
            (first, second)
        }
    }
}

fn render_single_pane(frame: &mut Frame, area: Rect, pane_id: PaneId, app: &App) {
    let is_active = pane_id == app.active_pane;
    let pane_idx = app
        .pane_tree()
        .leaves()
        .iter()
        .position(|&p| p == pane_id)
        .map(|i| i + 1)
        .unwrap_or(1);

    let border_color = if is_active { ACCENT } else { BORDER };

    let title = if is_active {
        format!(" {pane_idx}:~ *")
    } else {
        format!(" {pane_idx}:~ ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(if is_active { ACCENT_IDLE } else { FG_MUTED })
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(pane) = app.panes.get(&pane_id) {
        let scroll_offset = if is_active {
            if let InputMode::Scroll { offset } = &app.mode {
                Some(*offset)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(offset) = scroll_offset {
            render_cells_scrolled(frame, inner, pane, offset);
        } else {
            render_cells(
                frame,
                inner,
                pane,
                is_active && app.mode == InputMode::Normal,
                app.selection.as_ref(),
                pane_id,
            );
        }
    }
}

fn render_cells(
    frame: &mut Frame,
    area: Rect,
    pane: &PaneState,
    show_cursor: bool,
    selection: Option<&Selection>,
    pane_id: PaneId,
) {
    let grid = &pane.parser.grid;
    let rows = (area.height as usize).min(grid.rows as usize);
    let cols = (area.width as usize).min(grid.cols as usize);

    for row in 0..rows {
        for col in 0..cols {
            let cell = &grid.cells[row * grid.cols as usize + col];
            let x = area.x + col as u16;
            let y = area.y + row as u16;
            if let Some(buf_cell) = frame.buffer_mut().cell_mut((x, y)) {
                let ch = if cell.ch == '\0' { ' ' } else { cell.ch };
                buf_cell.set_char(ch);
                let mut fg = term_color(&cell.fg);
                let mut bg = term_color(&cell.bg);
                let in_selection = selection.is_some_and(|sel| {
                    sel.pane_id == pane_id && {
                        let (min_col, max_col) = if sel.start.0 <= sel.end.0 {
                            (sel.start.0, sel.end.0)
                        } else {
                            (sel.end.0, sel.start.0)
                        };
                        let (min_row, max_row) = if sel.start.1 <= sel.end.1 {
                            (sel.start.1, sel.end.1)
                        } else {
                            (sel.end.1, sel.start.1)
                        };
                        col as u16 >= min_col
                            && col as u16 <= max_col
                            && row as u16 >= min_row
                            && row as u16 <= max_row
                    }
                });
                if in_selection {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let mut style = Style::default().fg(fg).bg(bg);
                let mut mods = Modifier::empty();
                if cell.flags.bold() {
                    mods |= Modifier::BOLD;
                }
                if cell.flags.italic() {
                    mods |= Modifier::ITALIC;
                }
                if cell.flags.underline() {
                    mods |= Modifier::UNDERLINED;
                }
                if cell.flags.dim() {
                    mods |= Modifier::DIM;
                }
                if !mods.is_empty() {
                    style = style.add_modifier(mods);
                }
                buf_cell.set_style(style);
            }
        }
    }

    if show_cursor && grid.cursor_visible {
        let cx = area.x + grid.cursor_x.min(cols as u16);
        let cy = area.y + grid.cursor_y.min(rows as u16);
        if let Some(cell) = frame.buffer_mut().cell_mut((cx, cy)) {
            let s = cell.style();
            cell.set_style(s.fg(Color::Black).bg(Color::White));
        }
    }
}

fn render_cells_scrolled(
    frame: &mut Frame,
    area: Rect,
    pane: &crate::app::PaneState,
    offset: usize,
) {
    let grid = &pane.parser.grid;
    let height = area.height as usize;
    let grid_rows = grid.rows as usize;
    let scrollback_len = pane.scrollback.len();
    let total = scrollback_len + grid_rows;

    let start = total.saturating_sub(height + offset);

    for display_row in 0..height {
        let content_idx = start + display_row;
        let y = area.y + display_row as u16;

        let row_cells: &[Cell] = if content_idx < scrollback_len {
            &pane.scrollback[content_idx]
        } else {
            let gr = content_idx - scrollback_len;
            if gr < grid_rows {
                let row_start = gr * grid.cols as usize;
                &grid.cells[row_start..row_start + grid.cols as usize]
            } else {
                continue;
            }
        };

        render_row(frame, area.x, y, row_cells, area.width as usize);
    }
}

fn render_row(frame: &mut Frame, x: u16, y: u16, cells: &[Cell], max_cols: usize) {
    for (col, cell) in cells.iter().take(max_cols).enumerate() {
        if let Some(buf_cell) = frame.buffer_mut().cell_mut((x + col as u16, y)) {
            let ch = if cell.ch == '\0' { ' ' } else { cell.ch };
            buf_cell.set_char(ch);
            let fg = term_color(&cell.fg);
            let bg = term_color(&cell.bg);
            let mut style = Style::default().fg(fg).bg(bg);
            let mut mods = Modifier::empty();
            if cell.flags.bold() {
                mods |= Modifier::BOLD;
            }
            if cell.flags.italic() {
                mods |= Modifier::ITALIC;
            }
            if cell.flags.underline() {
                mods |= Modifier::UNDERLINED;
            }
            if cell.flags.dim() {
                mods |= Modifier::DIM;
            }
            if !mods.is_empty() {
                style = style.add_modifier(mods);
            }
            buf_cell.set_style(style);
        }
    }
}
