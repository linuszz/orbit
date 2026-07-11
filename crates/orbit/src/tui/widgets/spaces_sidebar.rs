use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.sidebar_visible {
        render_expanded(frame, area, app);
    } else {
        render_collapsed(frame, area, app);
    }
}

fn render_expanded(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width;
    let mut y = area.y;
    let x = area.x;

    // Header row: "SPACES" + collapse hint «
    let header = format!(
        "{:<width$}\u{00AB}",
        "SPACES",
        width = w.saturating_sub(1) as usize
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            header,
            Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
        ))),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    y += 1;

    // Divider
    let div = "\u{2500}".repeat(w as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            div,
            Style::default().fg(BORDER),
        ))),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    y += 1;

    // Cards
    for (i, space) in app.spaces.iter().enumerate() {
        if y + 6 > area.y + area.height {
            break;
        }

        let is_active = i == app.active_space_idx;
        let is_hovered = app.sidebar_hovered == Some(i);

        let card_bg = if is_active {
            BG_CARD
        } else if is_hovered {
            BG_TERTIARY
        } else {
            BG_SECONDARY
        };

        let name_fg = if is_active { FG_PRIMARY } else { FG_SECONDARY };

        // Top border row: ╭─ name ─╮ (or ▌─ name ─╮ for active)
        let name_trunc = truncate(&space.name, w.saturating_sub(4) as usize);
        let dashes_right = w.saturating_sub(4 + name_trunc.len() as u16);
        let top_left = if is_active { "\u{258C}" } else { "\u{256D}" }; // ▌ or ╭
        let accent_fg = if is_active { ACCENT } else { BORDER };
        // Build the rest of the border after the first char: "─ name ─...─╮"
        let rest_of_border = format!(
            "\u{2500} {}{} \u{256E}",
            name_trunc,
            "\u{2500}".repeat(dashes_right as usize)
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(top_left, Style::default().fg(accent_fg).bg(card_bg)),
                Span::styled(
                    rest_of_border,
                    Style::default().fg(BORDER).bg(card_bg),
                ),
            ]))
            .style(Style::default().bg(card_bg)),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        // Overlay the name in the correct color
        let name_x = x + 3; // after "▌─ " or "╭─ "
        frame.render_widget(
            Paragraph::new(Span::styled(
                name_trunc.clone(),
                Style::default()
                    .fg(name_fg)
                    .bg(card_bg)
                    .add_modifier(if is_active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )),
            Rect {
                x: name_x,
                y,
                width: w.saturating_sub(4),
                height: 1,
            },
        );
        y += 1;

        // CWD row: │ ~/path    │
        let cwd_trunc = truncate(&space.cwd, w.saturating_sub(4) as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("\u{2502}", Style::default().fg(BORDER).bg(card_bg)),
                Span::styled(
                    format!(
                        " {:<width$} ",
                        cwd_trunc,
                        width = w.saturating_sub(4) as usize
                    ),
                    Style::default().fg(FG_SECONDARY).bg(card_bg),
                ),
                Span::styled("\u{2502}", Style::default().fg(BORDER).bg(card_bg)),
            ])),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // Stats row: │ ● N  Xt Yp │
        let status_sym = if space.pane_count > 0 {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        let stats = format!(
            "{}   {}t {}p",
            status_sym, space.tab_count, space.pane_count
        );
        let stats_trunc = truncate(&stats, w.saturating_sub(4) as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("\u{2502}", Style::default().fg(BORDER).bg(card_bg)),
                Span::styled(
                    format!(
                        " {:<width$} ",
                        stats_trunc,
                        width = w.saturating_sub(4) as usize
                    ),
                    Style::default().fg(FG_MUTED).bg(card_bg),
                ),
                Span::styled("\u{2502}", Style::default().fg(BORDER).bg(card_bg)),
            ])),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // Bottom border: ╰──────────────╯
        let bottom = format!(
            "\u{2570}{}\u{256F}",
            "\u{2500}".repeat(w.saturating_sub(2) as usize)
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                bottom,
                Style::default().fg(BORDER).bg(card_bg),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // Gap between cards
        if i + 1 < app.spaces.len() {
            frame.render_widget(
                Paragraph::new("").style(Style::default().bg(BG_PRIMARY)),
                Rect {
                    x,
                    y,
                    width: w,
                    height: 1,
                },
            );
            y += 1;
        }
    }

    // New space button at bottom if space allows
    if y < area.y + area.height {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " [+] New ",
                Style::default().fg(ACCENT).bg(BG_CARD),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
    }
}

fn render_collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width; // should be 2
    let x = area.x;

    for (i, _space) in app.spaces.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height.saturating_sub(1) {
            break;
        }
        let is_active = i == app.active_space_idx;
        let (fg, bg) = if is_active {
            (BG_PRIMARY, ACCENT)
        } else {
            (FG_MUTED, BG_SECONDARY)
        };
        let label = format!("{:>2}", i + 1);
        frame.render_widget(
            Paragraph::new(Span::styled(label, Style::default().fg(fg).bg(bg))),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
    }

    // Expand hint at bottom
    let expand_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "\u{00BB}",
            Style::default().fg(FG_MUTED),
        )),
        Rect {
            x,
            y: expand_y,
            width: w,
            height: 1,
        },
    );
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('\u{2026}'); // horizontal ellipsis …
        t
    }
}
