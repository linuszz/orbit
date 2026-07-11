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

    // Header: « collapse button on the LEFT (cols 0-2), keeping it far from the tab bar edge.
    // This prevents accidental tab-bar clicks from triggering sidebar collapse.
    let collapse_fg = if app.sidebar_toggle_hovered {
        ACCENT
    } else {
        FG_MUTED
    };
    let spaces_fill = format!("{:<width$}", "SPACES", width = w.saturating_sub(3) as usize);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" \u{00AB} ", Style::default().fg(collapse_fg)),
            Span::styled(
                spaces_fill,
                Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
            ),
        ])),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    y += 1;

    // Top divider
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

    // Reserve 2 rows at the bottom: one divider + one button bar.
    let bottom_content = area.y + area.height.saturating_sub(2);

    // Cards — 3 rows each: name, cwd, stats; 1-row gap between cards
    for (i, space) in app.spaces.iter().enumerate() {
        if y + 3 > bottom_content {
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

        // Gap row between cards (not after the last one)
        if i + 1 < app.spaces.len() && y < bottom_content {
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

    // Bottom divider — separates card list from action buttons
    let divider_y = area.y + area.height.saturating_sub(2);
    let div2 = "\u{2500}".repeat(w as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(div2, Style::default().fg(BORDER)))),
        Rect {
            x,
            y: divider_y,
            width: w,
            height: 1,
        },
    );

    // Bottom button bar: [+] New │ ≡ Command, pinned to last row.
    // Split as (w-1)/2 chars + │ + remainder to fill exactly w chars.
    let bottom_y = area.y + area.height.saturating_sub(1);
    let n = app.spaces.len();
    let left_w = w.saturating_sub(1) / 2; // 9 when w=20
    let right_w = w.saturating_sub(1 + left_w); // 10 when w=20

    let (left_fg, left_bg) = if app.sidebar_hovered == Some(n) {
        (FG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_CARD)
    };
    let (right_fg, right_bg) = if app.sidebar_hovered == Some(n + 1) {
        (FG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_CARD)
    };

    let left_text = format!("{:<width$}", " [+] New", width = left_w as usize);
    let right_text = format!("{:<width$}", " \u{2261} Command", width = right_w as usize);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left_text, Style::default().fg(left_fg).bg(left_bg)),
            Span::styled("\u{2502}", Style::default().fg(BORDER).bg(BG_CARD)),
            Span::styled(right_text, Style::default().fg(right_fg).bg(right_bg)),
        ])),
        Rect {
            x,
            y: bottom_y,
            width: w,
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
