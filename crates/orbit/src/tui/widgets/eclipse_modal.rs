use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, EclipseModalState};
use crate::tui::theme::*;

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('\u{2026}');
        t
    }
}

/// Render the Satellite Eclipse intervention modal centered in `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(modal) = &app.eclipse_modal else {
        return;
    };

    let modal_w = 64u16.min(area.width.saturating_sub(4));
    let modal_h = 16u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_w,
        height: modal_h,
    };

    // Clear background and draw outer block.
    frame.render_widget(Clear, modal_area);
    let title = format!(
        " \u{25CE} Satellite Eclipse — {} ",
        truncate_str(&modal.agent_name, 20)
    );
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(ACCENT_BLOCKED).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_BLOCKED))
        .style(Style::default().bg(BG_SECONDARY));
    frame.render_widget(block, modal_area);

    let inner_x = modal_area.x + 1;
    let inner_w = modal_area.width.saturating_sub(2);
    let mut row = modal_area.y + 1;

    // "AGENT BLOCKED" header
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "AGENT BLOCKED",
                Style::default()
                    .fg(ACCENT_BLOCKED)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        Rect { x: inner_x, y: row, width: inner_w, height: 1 },
    );
    row += 1;

    // "agent requires intervention"
    let subtitle = format!(" {} requires intervention", modal.agent_name);
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate_str(&subtitle, inner_w as usize),
            Style::default().fg(FG_SECONDARY),
        )),
        Rect { x: inner_x, y: row, width: inner_w, height: 1 },
    );
    row += 1;

    // Blank row
    row += 1;

    // "Last message:" label
    frame.render_widget(
        Paragraph::new(Span::styled(" Last message:", Style::default().fg(FG_MUTED))),
        Rect { x: inner_x, y: row, width: inner_w, height: 1 },
    );
    row += 1;

    // Block message box (2 rows).
    let block_w = inner_w.saturating_sub(2);
    let msg_inner_x = inner_x + 1;
    {
        let border_top = format!(
            "\u{250c}{}\u{2510}",
            "\u{2500}".repeat(block_w as usize)
        );
        frame.render_widget(
            Paragraph::new(Span::styled(border_top, Style::default().fg(BORDER))),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
        let msg_display = format!(
            "\u{2502}{:<width$}\u{2502}",
            truncate_str(&modal.block_msg, block_w as usize),
            width = block_w as usize
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    &msg_display[..1],
                    Style::default().fg(BORDER),
                ),
                Span::styled(
                    msg_display[1..msg_display.len()-1].to_string(),
                    Style::default().fg(FG_SECONDARY),
                ),
                Span::styled(
                    &msg_display[msg_display.len()-1..],
                    Style::default().fg(BORDER),
                ),
            ])),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
        let border_bot = format!(
            "\u{2514}{}\u{2518}",
            "\u{2500}".repeat(block_w as usize)
        );
        frame.render_widget(
            Paragraph::new(Span::styled(border_bot, Style::default().fg(BORDER))),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
    }

    // "Your response:" label
    frame.render_widget(
        Paragraph::new(Span::styled(" Your response:", Style::default().fg(FG_MUTED))),
        Rect { x: inner_x, y: row, width: inner_w, height: 1 },
    );
    row += 1;

    // Response input box.
    {
        let border_top = format!(
            "\u{250c}{}\u{2510}",
            "\u{2500}".repeat(block_w as usize)
        );
        frame.render_widget(
            Paragraph::new(Span::styled(border_top, Style::default().fg(ACCENT))),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
        let cursor = "\u{2588}"; // block cursor
        let input = format!("> {}{}", modal.response, cursor);
        let input_truncated = truncate_str(&input, block_w as usize);
        let input_padded = format!("{:<width$}", input_truncated, width = block_w as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("\u{2502}", Style::default().fg(ACCENT)),
                Span::styled(input_padded, Style::default().fg(FG_PRIMARY)),
                Span::styled("\u{2502}", Style::default().fg(ACCENT)),
            ])),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
        let border_bot = format!(
            "\u{2514}{}\u{2518}",
            "\u{2500}".repeat(block_w as usize)
        );
        frame.render_widget(
            Paragraph::new(Span::styled(border_bot, Style::default().fg(ACCENT))),
            Rect { x: msg_inner_x, y: row, width: block_w + 2, height: 1 },
        );
        row += 1;
    }

    // Blank row
    if row < modal_area.y + modal_area.height.saturating_sub(1) {
        row += 1;
    }

    // Buttons: [Send] [Cancel] [Abort Eclipse]
    if row < modal_area.y + modal_area.height {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("[Send]", Style::default().fg(ACCENT)),
                Span::raw("  "),
                Span::styled("[Cancel]", Style::default().fg(FG_MUTED)),
                Span::raw("  "),
                Span::styled("[Abort Eclipse]", Style::default().fg(ACCENT_ERROR)),
                Span::styled(
                    "   Enter:send  Esc:cancel",
                    Style::default().fg(FG_MUTED),
                ),
            ])),
            Rect { x: inner_x, y: row, width: inner_w, height: 1 },
        );
    }
}

/// Open the Eclipse modal for the given agent info.
pub fn open(state: &mut crate::app::App, agent_id: orbit_protocol::AgentId) {
    if let Some(agent) = state.agents.iter().find(|a| a.id == agent_id) {
        let block_msg = agent
            .detail
            .as_ref()
            .and_then(|d| d.block_msg.clone())
            .unwrap_or_default();
        state.eclipse_modal = Some(EclipseModalState {
            agent_id,
            agent_name: agent.name.clone(),
            block_msg,
            response: String::new(),
        });
        state.needs_redraw = true;
    }
}
