use ratatui::{layout::Rect, Frame};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    use crate::tui::theme::*;

    let mut spans: Vec<Span> = Vec::new();

    for (i, tab) in app.tabs.iter().enumerate() {
        let label = format!(" {} ", tab.name);
        let (bg, fg, mods) = if tab.id == app.active_tab_id {
            (ACCENT, BG_PRIMARY, Modifier::BOLD)
        } else if app.tab_hovered == Some(i) {
            (ACCENT_HOVER, FG_PRIMARY, Modifier::empty())
        } else {
            (BG_CARD, FG_MUTED, Modifier::empty())
        };
        spans.push(Span::styled(
            label,
            Style::default().fg(fg).bg(bg).add_modifier(mods),
        ));
    }

    // New tab button: default text=FG_MUTED bg=BG_CARD, hover text=ACCENT bg=BG_CARD
    let new_tab_fg = if app.tab_hovered == Some(app.tabs.len()) {
        ACCENT
    } else {
        FG_MUTED
    };
    spans.push(Span::styled(
        " + ",
        Style::default().fg(new_tab_fg).bg(BG_CARD),
    ));

    // Build the "[A] Satellites" label; append fleet status badge when panel is hidden.
    use orbit_protocol::AgentStatus;
    let fleet_badge: String = if !app.agent_panel_visible && !app.agents.is_empty() {
        let n_blocked = app
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Blocked)
            .count();
        let n_working = app
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Working)
            .count();
        let n_error = app
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Error)
            .count();
        if n_blocked > 0 {
            format!(" \u{25CE}{}", n_blocked)
        } else if n_working > 0 {
            format!(" \u{25CF}{}", n_working)
        } else if n_error > 0 {
            format!(" \u{25C9}{}", n_error)
        } else {
            format!(" \u{25CB}{}", app.agents.len())
        }
    } else {
        String::new()
    };
    let agent_label = format!(" [A] Satellites{} ", fleet_badge);

    // Fill remaining space with BG_SECONDARY
    let used_width: u16 = spans.iter().map(|s| s.content.len() as u16).sum::<u16>();
    let agent_badge_w: u16 = agent_label.chars().count() as u16;
    let fill_len = area.width.saturating_sub(used_width + agent_badge_w) as usize;
    spans.push(Span::styled(
        " ".repeat(fill_len),
        Style::default().bg(BG_SECONDARY),
    ));

    // Agent panel toggle — right-aligned; badge color reflects fleet urgency when hidden.
    let (agent_fg, agent_bg) = if app.agent_panel_visible {
        (BG_PRIMARY, ACCENT)
    } else if app.tab_hovered == Some(app.tabs.len() + 1) {
        (FG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_CARD)
    };
    // When panel is closed and agents need attention, tint the badge icon.
    let badge_icon_color = if !app.agent_panel_visible {
        let n_blocked = app
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Blocked)
            .count();
        let n_working = app
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Working)
            .count();
        if n_blocked > 0 {
            Some(ACCENT_BLOCKED)
        } else if n_working > 0 {
            Some(ACCENT)
        } else {
            None
        }
    } else {
        None
    };
    if let Some(icon_color) = badge_icon_color {
        // Render the base label without the badge, then the badge in accent color.
        let base = " [A] Satellites";
        let badge = format!("{} ", fleet_badge);
        spans.push(Span::styled(
            base,
            Style::default().fg(agent_fg).bg(agent_bg),
        ));
        spans.push(Span::styled(
            badge,
            Style::default().fg(icon_color).bg(agent_bg),
        ));
    } else {
        spans.push(Span::styled(
            agent_label,
            Style::default().fg(agent_fg).bg(agent_bg),
        ));
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}
