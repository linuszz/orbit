pub mod theme;
pub mod widgets;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use orbit_protocol::TermColor;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};
use std::io::{self, Stdout};

use crate::app::{App, InputMode};
use theme::*;
use widgets::{agent_monitor, spaces_sidebar, status_bar, tab_bar};

pub type OrbitTerminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub fn setup_terminal() -> io::Result<OrbitTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    ratatui::Terminal::new(backend)
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

const SIDEBAR_WIDTH: u16 = 14;
const SIDEBAR_COLLAPSED_WIDTH: u16 = 2;
const AGENT_WIDTH: u16 = 22;
const TAB_BAR_HEIGHT: u16 = 1;
const STATUS_BAR_HEIGHT: u16 = 1;
const PANE_TITLE_HEIGHT: u16 = 1;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let sidebar_w = if app.sidebar_visible {
        SIDEBAR_WIDTH
    } else {
        SIDEBAR_COLLAPSED_WIDTH
    };
    let agent_w = if app.agent_panel_visible {
        AGENT_WIDTH
    } else {
        0
    };

    let main_cols = Layout::horizontal([
        Constraint::Length(sidebar_w),
        Constraint::Fill(1),
        Constraint::Length(agent_w),
    ])
    .split(area);

    spaces_sidebar::render(frame, main_cols[0], app);

    let right_area = if agent_w > 0 {
        Rect {
            x: main_cols[1].x,
            y: main_cols[1].y,
            width: main_cols[1].width + agent_w,
            height: main_cols[1].height,
        }
    } else {
        main_cols[1]
    };

    let rows = Layout::vertical([
        Constraint::Length(TAB_BAR_HEIGHT),
        Constraint::Fill(1),
        Constraint::Length(STATUS_BAR_HEIGHT),
    ])
    .split(right_area);

    tab_bar::render(frame, rows[0], app);

    render_pane(frame, rows[1], app);

    status_bar::render(frame, rows[2], app);

    if app.agent_panel_visible {
        agent_monitor::render(frame, main_cols[2], app);
    }
}

fn render_pane(frame: &mut Frame, area: Rect, app: &App) {
    let chunks =
        Layout::vertical([Constraint::Length(PANE_TITLE_HEIGHT), Constraint::Fill(1)]).split(area);

    let title_style = Style::default().fg(FG_MUTED).bg(BG_TERTIARY);
    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(&app.space_name, Style::default().fg(FG_SECONDARY)),
        Span::raw(" "),
    ]);
    frame.render_widget(
        Block::default()
            .style(title_style)
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER)),
        chunks[0],
    );
    frame.render_widget(title_line, chunks[0]);

    let grid = &app.parser.grid;
    let pane_area = chunks[1];
    let rows = (pane_area.height as usize).min(grid.rows as usize);
    let cols = (pane_area.width as usize).min(grid.cols as usize);

    for row in 0..rows {
        for col in 0..cols {
            let cell = &grid.cells[row * grid.cols as usize + col];
            let x = pane_area.x + col as u16;
            let y = pane_area.y + row as u16;
            let buf = frame.buffer_mut();
            if let Some(buf_cell) = buf.cell_mut((x, y)) {
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

    if grid.cursor_visible && app.mode == InputMode::Normal {
        let cx = pane_area.x + grid.cursor_x.min(cols as u16);
        let cy = pane_area.y + grid.cursor_y.min(rows as u16);
        let buf = frame.buffer_mut();
        if let Some(cell) = buf.cell_mut((cx, cy)) {
            let current = cell.style();
            cell.set_style(current.fg(Color::Black).bg(Color::White));
        }
    }
}
