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

    // Header row: "SPACES" left-aligned + collapse button « (3-char target for easier clicking)
    let collapse_fg = if app.sidebar_toggle_hovered {
        ACCENT
    } else {
        FG_MUTED
    };
    let spaces_label = format!("{:<width$}", "SPACES", width = w.saturating_sub(3) as usize);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                spaces_label,
                Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" \u{00AB} ", Style::default().fg(collapse_fg)),
        ])),
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
        Paragraph::new(Line::from(Span::styled(div, Style::default().fg(BORDER)))),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    y += 1;

    // Cards — 3 rows each: name, cwd, stats; 1-row gap between cards
    // Stop early enough to leave the bottom bar row free.
    for (i, space) in app.spaces.iter().enumerate() {
        if y + 3 >= area.y + area.height {
            break;
        }

        let is_active = i == app.active_space_idx;
        let is_hovered = app.sidebar_hovered == Some(i);

        // Name row
        let name_trunc = truncate(&space.name, (w as usize).saturating_sub(1));
        let name_text = format!(
            " {:<width$}",
            name_trunc,
            width = (w as usize).saturating_sub(1)
        );
        let (name_bg, name_fg, name_mod) = if is_active {
            (ACCENT, BG_PRIMARY, Modifier::BOLD)
        } else if is_hovered {
            (ACCENT_HOVER, FG_PRIMARY, Modifier::empty())
        } else {
            (BG_SECONDARY, FG_SECONDARY, Modifier::empty())
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                name_text,
                Style::default()
                    .fg(name_fg)
                    .bg(name_bg)
                    .add_modifier(name_mod),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // CWD row
        let cwd_trunc = truncate(&space.cwd, (w as usize).saturating_sub(1));
        let cwd_text = format!(
            " {:<width$}",
            cwd_trunc,
            width = (w as usize).saturating_sub(1)
        );
        let (cwd_bg, cwd_fg) = if is_active {
            (ACCENT, BG_PRIMARY)
        } else if is_hovered {
            (ACCENT_HOVER, FG_SECONDARY)
        } else {
            (BG_SECONDARY, FG_MUTED)
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                cwd_text,
                Style::default().fg(cwd_fg).bg(cwd_bg),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // Stats row
        let status_sym = if space.pane_count > 0 {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        let stats_raw = format!(" {} {}t {}p", status_sym, space.tab_count, space.pane_count);
        let stats_text = format!("{:<width$}", stats_raw, width = w as usize);
        let (stats_bg, stats_fg) = if is_active {
            (ACCENT, BG_PRIMARY)
        } else if is_hovered {
            (ACCENT_HOVER, FG_MUTED)
        } else {
            (BG_SECONDARY, FG_MUTED)
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                stats_text,
                Style::default().fg(stats_fg).bg(stats_bg),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        // Gap row between cards (not after the last one, not into the bottom bar)
        if i + 1 < app.spaces.len() && y < area.y + area.height - 1 {
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

    // Bottom bar: [+] New (left half) and ≡ Command (right half), always pinned to last row.
    let bottom_y = area.y + area.height - 1;
    let half_w = w / 2;
    let n = app.spaces.len();

    let (left_fg, left_bg) = if app.sidebar_hovered == Some(n) {
        (FG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_CARD)
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{:<width$}", " [+] New", width = half_w as usize),
            Style::default().fg(left_fg).bg(left_bg),
        )),
        Rect {
            x,
            y: bottom_y,
            width: half_w,
            height: 1,
        },
    );

    let (right_fg, right_bg) = if app.sidebar_hovered == Some(n + 1) {
        (FG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_CARD)
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(
                "{:<width$}",
                " \u{2261} Command",
                width = (w - half_w) as usize
            ),
            Style::default().fg(right_fg).bg(right_bg),
        )),
        Rect {
            x: x + half_w,
            y: bottom_y,
            width: w - half_w,
            height: 1,
        },
    );
}

fn render_collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width; // should be 2
    let x = area.x;

    // Expand hint at top (row 0) — right-aligned to match the space number labels below
    let expand_fg = if app.sidebar_toggle_hovered {
        ACCENT
    } else {
        FG_MUTED
    };
    frame.render_widget(
        Paragraph::new(Span::styled(" \u{00BB}", Style::default().fg(expand_fg))),
        Rect {
            x,
            y: area.y,
            width: w,
            height: 1,
        },
    );

    // Space numbers starting at row 1
    for (i, _space) in app.spaces.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.y + area.height {
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
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('\u{2026}'); // horizontal ellipsis
        t
    }
}
