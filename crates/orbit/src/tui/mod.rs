pub mod theme;
pub mod widgets;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use orbit_protocol::{PaneId, SplitDir, TermColor};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};
use std::io::{self, Stdout};

use crate::app::{App, InputMode, PaneNode, PaneState};
use orbit_protocol::Cell;
use theme::*;

pub type OrbitTerminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub fn setup_terminal() -> io::Result<OrbitTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    ratatui::Terminal::new(CrosstermBackend::new(stdout))
}

pub fn restore_terminal(terminal: &mut OrbitTerminal) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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

const SIDEBAR_W: u16 = 14;
const SIDEBAR_COLLAPSED_W: u16 = 2;
const AGENT_W: u16 = 22;
const SEP: u16 = 1;

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
    render_pane_tree(frame, rows[1], &app.pane_tree, app);
    widgets::status_bar::render(frame, rows[2], app);

    if app.agent_panel_visible {
        widgets::agent_monitor::render(frame, cols[2], app);
    }
}

pub fn compute_leaf_areas(node: &PaneNode, area: Rect) -> Vec<(PaneId, Rect)> {
    match node {
        PaneNode::Leaf(pid) => vec![(*pid, area)],
        PaneNode::Split {
            direction,
            first,
            second,
        } => {
            let (first_area, _sep, second_area) = split_area(area, direction);
            let mut v = compute_leaf_areas(first, first_area);
            v.extend(compute_leaf_areas(second, second_area));
            v
        }
    }
}

fn render_pane_tree(frame: &mut Frame, area: Rect, node: &PaneNode, app: &App) {
    match node {
        PaneNode::Leaf(pid) => {
            render_single_pane(frame, area, *pid, app);
        }
        PaneNode::Split {
            direction,
            first,
            second,
        } => {
            let (first_area, sep_area, second_area) = split_area(area, direction);

            render_pane_tree(frame, first_area, first, app);
            render_separator(frame, sep_area, *direction);
            render_pane_tree(frame, second_area, second, app);
        }
    }
}

fn split_area(area: Rect, dir: &SplitDir) -> (Rect, Rect, Rect) {
    match dir {
        SplitDir::Horizontal => {
            let total = area.width;
            let half = total / 2;
            let first_w = half.saturating_sub(SEP / 2);
            let second_w = total.saturating_sub(half).saturating_sub(SEP / 2);
            let first = Rect {
                width: first_w,
                ..area
            };
            let sep = Rect {
                x: area.x + first_w,
                width: SEP,
                ..area
            };
            let second = Rect {
                x: area.x + first_w + SEP,
                width: second_w,
                ..area
            };
            (first, sep, second)
        }
        SplitDir::Vertical => {
            let total = area.height;
            let half = total / 2;
            let first_h = half.saturating_sub(SEP / 2);
            let second_h = total.saturating_sub(half).saturating_sub(SEP / 2);
            let first = Rect {
                height: first_h,
                ..area
            };
            let sep = Rect {
                y: area.y + first_h,
                height: SEP,
                ..area
            };
            let second = Rect {
                y: area.y + first_h + SEP,
                height: second_h,
                ..area
            };
            (first, sep, second)
        }
    }
}

fn render_separator(frame: &mut Frame, area: Rect, dir: SplitDir) {
    let style = Style::default().fg(BORDER);
    let buf = frame.buffer_mut();
    match dir {
        SplitDir::Horizontal => {
            let x = area.x;
            for y in area.y..area.y + area.height {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_char('\u{2502}');
                    c.set_style(style);
                }
            }
        }
        SplitDir::Vertical => {
            let y = area.y;
            for x in area.x..area.x + area.width {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_char('\u{2500}');
                    c.set_style(style);
                }
            }
        }
    }
}

fn render_single_pane(frame: &mut Frame, area: Rect, pane_id: PaneId, app: &App) {
    let is_active = pane_id == app.active_pane;
    let pane_idx = app
        .pane_tree
        .leaves()
        .iter()
        .position(|&p| p == pane_id)
        .map(|i| i + 1)
        .unwrap_or(1);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Fill(1),
    ])
    .split(area);

    let border_color = if is_active { ACCENT } else { BORDER };
    let title_bg = if is_active { BG_SECONDARY } else { BG_TERTIARY };
    let title_fg = if is_active { FG_SECONDARY } else { FG_MUTED };

    let title_block = Block::default()
        .style(Style::default().fg(title_fg).bg(title_bg))
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(title_block, chunks[0]);

    let label = if is_active {
        format!("{pane_idx}:~ *")
    } else {
        format!("{pane_idx}:~")
    };
    let title_line = ratatui::text::Line::from(vec![
        ratatui::text::Span::raw(" "),
        ratatui::text::Span::styled(
            label,
            Style::default().fg(if is_active { ACCENT_IDLE } else { FG_MUTED }),
        ),
    ]);
    frame.render_widget(title_line, chunks[0]);

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
            render_cells_scrolled(frame, chunks[1], pane, offset);
        } else {
            render_cells(
                frame,
                chunks[1],
                pane,
                is_active && app.mode == InputMode::Normal,
            );
        }
    }
}

fn render_cells(frame: &mut Frame, area: Rect, pane: &PaneState, show_cursor: bool) {
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
