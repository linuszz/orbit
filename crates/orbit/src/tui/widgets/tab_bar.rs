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

    // Fill remaining space with BG_SECONDARY
    let used_width: u16 = spans.iter().map(|s| s.content.len() as u16).sum::<u16>();
    let agent_badge_w: u16 = " [A] Satellites ".len() as u16;
    let fill_len = area.width.saturating_sub(used_width + agent_badge_w) as usize;
    spans.push(Span::styled(
        " ".repeat(fill_len),
        Style::default().bg(BG_SECONDARY),
    ));

    // Agent panel toggle — right-aligned
    let (agent_fg, agent_bg) = if app.agent_panel_visible {
        (BG_PRIMARY, ACCENT)
    } else {
        (FG_MUTED, BG_CARD)
    };
    spans.push(Span::styled(
        " [A] Satellites ",
        Style::default().fg(agent_fg).bg(agent_bg),
    ));

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}
