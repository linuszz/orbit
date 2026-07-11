use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, LaunchModalState, LAUNCH_AGENTS};
use crate::tui::theme::*;

pub const MODAL_W: u16 = 42;
// Inner rows: blank + "Agent:" + N agents + blank + buttons = N + 4
// Total height = inner + 2 borders = N + 6
pub const INNER_H: u16 = (LAUNCH_AGENTS.len() as u16) + 4;

/// Render the "Launch Satellite" agent picker overlay centered in `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(modal) = &app.launch_modal else {
        return;
    };
    let modal_h = (INNER_H + 2).min(area.height.saturating_sub(4));
    let modal_w = MODAL_W.min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_w,
        height: modal_h,
    };

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .title(" Launch Satellite ")
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG_SECONDARY));
    frame.render_widget(block, modal_area);

    let ix = modal_area.x + 1;
    let iw = modal_area.width.saturating_sub(2);
    let mut row = modal_area.y + 1;

    // blank
    row += 1;

    // "Agent:" label
    frame.render_widget(
        Paragraph::new(Span::styled(" Agent:", Style::default().fg(FG_MUTED))),
        Rect {
            x: ix,
            y: row,
            width: iw,
            height: 1,
        },
    );
    row += 1;

    // Agent rows
    for (i, (cmd, label)) in LAUNCH_AGENTS.iter().enumerate() {
        if row >= modal_area.y + modal_area.height.saturating_sub(1) {
            break;
        }
        let selected = modal.selected == i;
        let (prefix_fg, name_fg, label_fg, bg) = if selected {
            (ACCENT, FG_PRIMARY, FG_SECONDARY, BG_TERTIARY)
        } else {
            (FG_MUTED, FG_MUTED, FG_MUTED, BG_SECONDARY)
        };
        let prefix = if selected { "\u{25B8}" } else { " " };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", prefix),
                    Style::default().fg(prefix_fg).bg(bg),
                ),
                Span::styled(
                    format!("{cmd:<10}"),
                    Style::default()
                        .fg(name_fg)
                        .add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        })
                        .bg(bg),
                ),
                Span::styled(
                    format!(" {:<width$}", label, width = iw.saturating_sub(14) as usize),
                    Style::default().fg(label_fg).bg(bg),
                ),
            ])),
            Rect {
                x: ix,
                y: row,
                width: iw,
                height: 1,
            },
        );
        row += 1;
    }

    // blank
    if row < modal_area.y + modal_area.height.saturating_sub(1) {
        row += 1;
    }

    // Buttons
    if row < modal_area.y + modal_area.height {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("[Launch]", Style::default().fg(ACCENT)),
                Span::raw("  "),
                Span::styled("[Cancel]", Style::default().fg(FG_MUTED)),
                Span::styled("   Enter:launch  Esc:cancel", Style::default().fg(FG_MUTED)),
            ])),
            Rect {
                x: ix,
                y: row,
                width: iw,
                height: 1,
            },
        );
    }
}

/// Open the launch modal with the first agent pre-selected.
pub fn open(app: &mut App) {
    app.launch_modal = Some(LaunchModalState { selected: 0 });
    app.needs_redraw = true;
}
