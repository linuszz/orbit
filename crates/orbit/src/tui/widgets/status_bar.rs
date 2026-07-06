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

    if app.mode == InputMode::Prefix {
        spans.push(Span::styled(
            " COMMAND  Esc:cancel ",
            Style::default()
                .fg(BG_PRIMARY)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let InputMode::Scroll { offset } = &app.mode {
        spans.push(Span::styled(
            format!(" SCROLL  -{offset} "),
            Style::default()
                .fg(BG_PRIMARY)
                .bg(ACCENT_IDLE)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        &app.space_name,
        Style::default().fg(FG_SECONDARY),
    ));
    spans.push(Span::styled(" | ", Style::default().fg(BORDER)));

    spans.push(Span::styled("dev*", Style::default().fg(FG_MUTED)));
    spans.push(Span::styled(" | ", Style::default().fg(BORDER)));

    spans.push(Span::styled("○ idle", Style::default().fg(ACCENT_IDLE)));

    spans.push(Span::styled(
        format!(" | recv:{}", app.bytes_received),
        Style::default().fg(FG_MUTED),
    ));

    let line = Line::from(spans);
    frame.render_widget(line, area);
}

use ratatui::widgets::Block;
