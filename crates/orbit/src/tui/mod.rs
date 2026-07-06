pub mod theme;
pub mod widgets;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use orbit_protocol::{SplitDir, TermColor};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};
use std::io::{self, Stdout};

use crate::app::{App, InputMode};
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

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let sidebar_w = if app.sidebar_visible {
        SIDEBAR_W
    } else {
        SIDEBAR_COLLAPSED_W
    };
    let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };

    let cols = Layout::horizontal([
        Constraint::Length(sidebar_w),
        Constraint::Fill(1),
        Constraint::Length(agent_w),
    ])
    .split(area);

    widgets::spaces_sidebar::render(frame, cols[0], app);

    let right = Rect {
        x: cols[1].x,
        y: cols[1].y,
        width: cols[1].width,
        height: cols[1].height,
    };

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(right);

    widgets::tab_bar::render(frame, rows[0], app);
    render_panes(frame, rows[1], app);
    widgets::status_bar::render(frame, rows[2], app);

    if app.agent_panel_visible {
        widgets::agent_monitor::render(frame, cols[2], app);
    }
}

fn render_panes(frame: &mut Frame, area: Rect, app: &App) {
    let n = app.pane_order.len();
    if n == 0 {
        return;
    }

    let pane_areas: Vec<Rect> = if n == 1 {
        vec![area]
    } else {
        match app.layout {
            SplitDir::Horizontal => {
                let constraints = vec![Constraint::Ratio(1, n as u32); n];
                Layout::horizontal(constraints).split(area).to_vec()
            }
            SplitDir::Vertical => {
                let constraints = vec![Constraint::Ratio(1, n as u32); n];
                Layout::vertical(constraints).split(area).to_vec()
            }
        }
    };

    for (i, &pane_id) in app.pane_order.iter().enumerate() {
        let is_active = pane_id == app.active_pane;
        let pane_area = pane_areas[i];

        let chunks =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(pane_area);

        let border_color = if is_active { ACCENT } else { BORDER };
        let title_bg = if is_active { BG_SECONDARY } else { BG_TERTIARY };
        let title_fg = if is_active { FG_SECONDARY } else { FG_MUTED };

        let title_block = Block::default()
            .style(Style::default().fg(title_fg).bg(title_bg))
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(border_color));
        frame.render_widget(title_block, chunks[0]);

        let label = if is_active { "~ *" } else { "~" };
        let title_line = ratatui::text::Line::from(vec![
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                label,
                Style::default().fg(if is_active { ACCENT_IDLE } else { FG_MUTED }),
            ),
        ]);
        frame.render_widget(title_line, chunks[0]);

        if let Some(pane) = app.panes.get(&pane_id) {
            render_cells(
                frame,
                chunks[1],
                &pane.parser,
                is_active && app.mode == InputMode::Normal,
            );
        }

        if n > 1 && i < n - 1 {
            let separator_style = Style::default().fg(BORDER);
            match app.layout {
                SplitDir::Horizontal => {
                    let x = pane_area.x + pane_area.width;
                    for y in pane_area.y..pane_area.y + pane_area.height {
                        if let Some(c) = frame.buffer_mut().cell_mut((x, y)) {
                            c.set_char('\u{2502}');
                            c.set_style(separator_style);
                        }
                    }
                }
                SplitDir::Vertical => {
                    let y = pane_area.y + pane_area.height;
                    for x in pane_area.x..pane_area.x + pane_area.width {
                        if let Some(c) = frame.buffer_mut().cell_mut((x, y)) {
                            c.set_char('\u{2500}');
                            c.set_style(separator_style);
                        }
                    }
                }
            }
        }
    }
}

fn render_cells(frame: &mut Frame, area: Rect, parser: &orbit_core::VtParser, show_cursor: bool) {
    let grid = &parser.grid;
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
