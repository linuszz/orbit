use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::app::{App, InputMode};
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let bg = Block::default()
        .style(Style::default().bg(BG_SECONDARY).fg(FG_MUTED))
        .borders(ratatui::widgets::Borders::TOP)
        .border_style(Style::default().fg(BORDER));
    frame.render_widget(bg, area);

    let mut spans: Vec<Span> = vec![];

    if matches!(app.mode, InputMode::CommandPalette { .. }) {
        spans.push(Span::styled(
            " FLIGHT DECK  Esc:cancel ",
            Style::default()
                .fg(BG_PRIMARY)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" | ", Style::default().fg(BORDER)));
    }

    if let InputMode::Scroll { offset } = &app.mode {
        spans.push(Span::styled(
            format!(" SCROLL  -{offset} "),
            Style::default()
                .fg(BG_PRIMARY)
                .bg(ACCENT_IDLE)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" | ", Style::default().fg(BORDER)));
    }

    spans.push(Span::styled(
        "[SPACE] ",
        Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        &app.space_name,
        Style::default().fg(FG_SECONDARY),
    ));
    spans.push(Span::styled(" | ", Style::default().fg(BORDER)));

    spans.push(Span::styled(
        app.current_tab_name(),
        Style::default().fg(FG_MUTED),
    ));
    spans.push(Span::styled(
        "*",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(" | ", Style::default().fg(BORDER)));

    spans.push(Span::styled("○ idle", Style::default().fg(ACCENT_IDLE)));

    if !app.space_path.is_empty() && app.space_path != "." {
        spans.push(Span::styled(" | ", Style::default().fg(BORDER)));
        spans.push(Span::styled(
            &app.space_path,
            Style::default().fg(FG_SECONDARY),
        ));
    }

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hh = (secs / 3600 % 24) as u8;
    let mm = (secs / 60 % 60) as u8;
    spans.push(Span::styled(
        format!(" | {hh:02}:{mm:02}"),
        Style::default().fg(FG_MUTED),
    ));

    let line = Line::from(spans);
    frame.render_widget(line, area);
}

use ratatui::widgets::Block;
