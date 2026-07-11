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

    let tab_width = area.width.saturating_sub(14) as usize;
    let mut spans: Vec<Span> = vec![Span::raw(" ")];

    let mut used = 1usize;
    for (i, tab) in app.tabs.iter().enumerate() {
        if used >= tab_width {
            break;
        }
        let is_active = i == app.active_tab;
        let label = if is_active {
            format!(" {}* ", tab.name)
        } else {
            format!(" {} ", tab.name)
        };
        let (style, underline) = if is_active {
            (
                Style::default()
                    .fg(FG_PRIMARY)
                    .bg(BG_TERTIARY)
                    .add_modifier(Modifier::BOLD),
                true,
            )
        } else {
            (Style::default().fg(FG_MUTED), false)
        };
        if underline {
            spans.push(Span::styled(
                label.clone(),
                style.add_modifier(Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::styled(label, style));
        }
        used += tab.name.len() + 3;
    }

    spans.push(Span::styled(" + ", Style::default().fg(ACCENT)));

    let remaining = area.width.saturating_sub(used as u16 + 8);
    if remaining > 0 {
        spans.push(Span::raw(" ".repeat(remaining as usize)));
    }

    let agent_color = if app.agent_panel_visible {
        ACCENT
    } else {
        FG_MUTED
    };
    spans.push(Span::styled("[A]", Style::default().fg(agent_color)));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "Agents",
        Style::default().fg(if app.agent_panel_visible {
            ACCENT
        } else {
            FG_MUTED
        }),
    ));

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
