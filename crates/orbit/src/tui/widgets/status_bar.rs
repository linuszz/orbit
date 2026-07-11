use orbit_protocol::AgentStatus;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::app::{App, InputMode};
use crate::tui::theme::*;
use crate::tui::widgets::agent_monitor::{blocked_pulse_color, working_pulse_color};

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

    if matches!(app.mode, InputMode::AgentPanel { .. }) {
        spans.push(Span::styled(
            " SATELLITE NAV ",
            Style::default()
                .fg(BG_PRIMARY)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            " \u{2191}\u{2193}:nav Enter:view q:exit",
            Style::default().fg(FG_MUTED),
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

    // Live satellite fleet summary — highest-severity status wins.
    let n_blocked = app
        .agents
        .iter()
        .filter(|a| a.status == AgentStatus::Blocked)
        .count();
    let n_error = app
        .agents
        .iter()
        .filter(|a| a.status == AgentStatus::Error)
        .count();
    let n_working = app
        .agents
        .iter()
        .filter(|a| a.status == AgentStatus::Working)
        .count();
    let n_idle = app
        .agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Idle | AgentStatus::Done))
        .count();

    let (icon, label, color) = if n_blocked > 0 {
        let s = if n_blocked == 1 {
            "eclipse".to_string()
        } else {
            format!("{n_blocked} eclipse")
        };
        // Blocked uses animated pulse (same helper as the agent card icon).
        ("\u{25CE}", s, blocked_pulse_color(app.tick_count))
    } else if n_error > 0 {
        let s = if n_error == 1 {
            "debris".to_string()
        } else {
            format!("{n_error} debris")
        };
        ("\u{25C9}", s, ACCENT_ERROR)
    } else if n_working > 0 {
        let s = if n_working == 1 {
            "transmitting".to_string()
        } else {
            format!("{n_working} transmitting")
        };
        // Working uses animated pulse.
        ("\u{25CF}", s, working_pulse_color(app.tick_count))
    } else if n_idle > 0 {
        let s = if n_idle == 1 {
            "standby".to_string()
        } else {
            format!("{n_idle} standby")
        };
        ("\u{25CB}", s, ACCENT_IDLE)
    } else {
        ("\u{25CB}", "idle".to_string(), ACCENT_IDLE)
    };
    spans.push(Span::styled(
        format!("{icon} {label}"),
        Style::default().fg(color),
    ));

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
