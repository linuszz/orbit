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
        .borders(ratatui::widgets::Borders::BOTTOM)
        .border_style(Style::default().fg(BORDER));
    frame.render_widget(bg, area);

    let mut spans = vec![Span::raw(" ")];

    for (i, tab) in app.tabs.iter().enumerate() {
        if i == app.active_tab {
            spans.push(Span::styled(
                &tab.name,
                Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(" ", Style::default()));
        } else {
            spans.push(Span::styled(&tab.name, Style::default().fg(FG_MUTED)));
            spans.push(Span::raw("  "));
        }
    }

    spans.push(Span::raw("  "));

    let agent_color = if app.agent_panel_visible {
        ACCENT
    } else {
        FG_MUTED
    };
    spans.push(Span::styled("[A]", Style::default().fg(agent_color)));

    if matches!(app.mode, InputMode::CommandPalette { .. }) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "[FLIGHT DECK]",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);
    frame.render_widget(line, area);
}

use ratatui::widgets::Block;
