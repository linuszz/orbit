use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use orbit_protocol::TermColor;
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
    Frame, Terminal,
};
use std::io::{self, Stdout};

use crate::app::{App, InputMode};

pub type OrbitTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn setup_terminal() -> io::Result<OrbitTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn restore_terminal(terminal: &mut OrbitTerminal) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn term_color_to_ratatui(c: &TermColor) -> Color {
    match c {
        TermColor::Default => Color::Reset,
        TermColor::Ansi(n) => Color::Indexed(*n),
        TermColor::Ansi256(n) => Color::Indexed(*n),
        TermColor::Rgb(r, g, b) => Color::Rgb(*r, *g, *b),
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let grid = &app.parser.grid;

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = Block::default().borders(Borders::NONE);
    frame.render_widget(block, area);
    let pane_area = inner.inner(area);
    frame.render_widget(inner, pane_area);

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

                let fg = term_color_to_ratatui(&cell.fg);
                let bg = term_color_to_ratatui(&cell.bg);
                let mut style = Style::default().fg(fg).bg(bg);
                let mut mod_flags = Modifier::empty();
                if cell.flags.bold() {
                    mod_flags |= Modifier::BOLD;
                }
                if cell.flags.italic() {
                    mod_flags |= Modifier::ITALIC;
                }
                if cell.flags.underline() {
                    mod_flags |= Modifier::UNDERLINED;
                }
                if cell.flags.dim() {
                    mod_flags |= Modifier::DIM;
                }
                if !mod_flags.is_empty() {
                    style = style.add_modifier(mod_flags);
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

    let status_y = area.height.saturating_sub(1);
    let status_text = match app.mode {
        InputMode::Normal => " ORBIT  Ctrl+B:command".to_string(),
        InputMode::Prefix => " [PREFIX  x:close  d:detach  Esc:cancel]".to_string(),
    };
    let status_style = match app.mode {
        InputMode::Normal => Style::default().fg(Color::Yellow),
        InputMode::Prefix => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    };
    let span = Span::styled(status_text, status_style);
    frame.render_widget(
        span,
        Rect {
            x: 0,
            y: status_y,
            width: area.width,
            height: 1,
        },
    );
}
