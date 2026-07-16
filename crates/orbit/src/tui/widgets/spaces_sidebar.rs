use orbit_protocol::AgentStatus;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
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
    let w = area.width.saturating_sub(1);
    let mut y = area.y;
    let x = area.x;
    let sep_x = area.x + area.width - 1;

    frame.render_widget(
        Block::default().style(Style::default().bg(bg_primary())),
        area,
    );

    // Header: « collapse button on the LEFT (cols 0-2), keeping it far from the tab bar edge.
    // This prevents accidental tab-bar clicks from triggering sidebar collapse.
    let collapse_fg = if app.sidebar_toggle_hovered {
        accent()
    } else {
        fg_muted()
    };
    let spaces_fill = format!("{:<width$}", "SPACES", width = w.saturating_sub(3) as usize);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" \u{00AB} ", Style::default().fg(collapse_fg)),
            Span::styled(
                spaces_fill,
                Style::default().fg(fg_muted()).add_modifier(Modifier::BOLD),
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
        Paragraph::new(Line::from(Span::styled(div, Style::default().fg(border())))),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    y += 1;

    // Reserve 2 rows at the bottom: one divider + one button bar.
    let bottom_content = area.y + area.height.saturating_sub(3);

    for (i, space) in app.spaces.iter().enumerate() {
        if y + 4 > bottom_content {
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
            (accent(), bg_primary(), Modifier::BOLD)
        } else if is_hovered {
            (accent_hover(), fg_primary(), Modifier::BOLD)
        } else {
            (bg_secondary(), fg_primary(), Modifier::empty())
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

        let cwd_display = space
            .cwd
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&space.cwd);
        let cwd_trunc = truncate(cwd_display, (w as usize).saturating_sub(3));
        let cwd_text = format!(
            " {:<width$}",
            cwd_trunc,
            width = (w as usize).saturating_sub(1)
        );
        let (cwd_bg, cwd_fg) = if is_active {
            (accent(), bg_primary())
        } else if is_hovered {
            (accent_hover(), fg_secondary())
        } else {
            (bg_secondary(), fg_secondary())
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

        // Stats row — tab/pane counts + agent fleet summary for this space.
        let status_sym = if space.pane_count > 0 {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        // Count agents belonging to this space.
        let space_agents: Vec<_> = app
            .agents
            .iter()
            .filter(|a| a.space_id == space.space_id)
            .collect();
        let agent_badge = if !space_agents.is_empty() {
            let n_blocked = space_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Blocked)
                .count();
            let n_working = space_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Working)
                .count();
            if n_blocked > 0 {
                format!(" \u{25CE}{}", n_blocked)
            } else if n_working > 0 {
                format!(" \u{25CF}{}", n_working)
            } else {
                format!(" \u{25CB}{}", space_agents.len())
            }
        } else {
            String::new()
        };
        let base_stats = format!(" {} {}t {}p", status_sym, space.tab_count, space.pane_count);
        let (stats_bg, stats_fg) = if is_active {
            (accent(), bg_primary())
        } else if is_hovered {
            (accent_hover(), fg_secondary())
        } else {
            (bg_secondary(), fg_secondary())
        };
        // Color the agent badge by urgency (only on non-active rows so it's visible).
        let badge_color = if is_active || is_hovered {
            stats_fg
        } else if !space_agents.is_empty() {
            let n_blocked = space_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Blocked)
                .count();
            let n_working = space_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Working)
                .count();
            if n_blocked > 0 {
                accent_blocked()
            } else if n_working > 0 {
                accent()
            } else {
                fg_muted()
            }
        } else {
            stats_fg
        };
        let total_len = base_stats.chars().count() + agent_badge.chars().count();
        let pad = (w as usize).saturating_sub(total_len);
        let pad_str = " ".repeat(pad);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(base_stats, Style::default().fg(stats_fg).bg(stats_bg)),
                Span::styled(agent_badge, Style::default().fg(badge_color).bg(stats_bg)),
                Span::styled(pad_str, Style::default().bg(stats_bg)),
            ])),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;

        frame.render_widget(
            Paragraph::new(Span::styled(
                "\u{2500}".repeat(w as usize),
                Style::default().fg(border()).bg(bg_primary()),
            )),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
        y += 1;
    }

    // Bottom divider — separates card list from action buttons
    let divider_y = area.y + area.height.saturating_sub(2);
    let div2 = "\u{2500}".repeat(w as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            div2,
            Style::default().fg(border()),
        ))),
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
        (fg_primary(), accent_hover())
    } else {
        (fg_muted(), bg_card())
    };
    let (right_fg, right_bg) = if app.sidebar_hovered == Some(n + 1) {
        (fg_primary(), accent_hover())
    } else {
        (fg_muted(), bg_card())
    };

    let left_text = format!("{:<width$}", " [+] New", width = left_w as usize);
    let right_text = format!("{:<width$}", " \u{2261} Command", width = right_w as usize);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left_text, Style::default().fg(left_fg).bg(left_bg)),
            Span::styled("\u{2502}", Style::default().fg(border()).bg(bg_card())),
            Span::styled(right_text, Style::default().fg(right_fg).bg(right_bg)),
        ])),
        Rect {
            x,
            y: bottom_y,
            width: w,
            height: 1,
        },
    );

    for row in area.y..area.y + area.height {
        if let Some(cell) = frame.buffer_mut().cell_mut((sep_x, row)) {
            cell.set_char('\u{2502}')
                .set_style(Style::default().fg(border()));
        }
    }
}

fn render_collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let x = area.x;
    let bg = bg_secondary();
    let w = area.width;
    let last_row = area.height.saturating_sub(1);
    let btn2_row = area.height.saturating_sub(2);
    let mid_col = w / 2;

    for row in 0..area.height {
        let y = area.y + row;

        let is_btn_plus = row == btn2_row;
        let is_btn_cmd = row == last_row;
        let btn_hover = if is_btn_plus {
            app.sidebar_hovered == Some(app.spaces.len())
        } else if is_btn_cmd {
            app.sidebar_hovered == Some(app.spaces.len() + 1)
        } else {
            false
        };

        let (ch, fg_val, bg_val) = if row == 0 {
            let fg = if app.sidebar_toggle_hovered {
                accent()
            } else {
                fg_muted()
            };
            ('\u{00BB}', fg, bg)
        } else if is_btn_cmd {
            let fg = if btn_hover { fg_primary() } else { fg_muted() };
            let bg_btn = if btn_hover { accent_hover() } else { bg_card() };
            ('\u{2261}', fg, bg_btn)
        } else if is_btn_plus {
            let fg = if btn_hover { fg_primary() } else { fg_muted() };
            let bg_btn = if btn_hover { accent_hover() } else { bg_card() };
            ('+', fg, bg_btn)
        } else {
            let sr = row - 1;
            if (sr as usize) < app.spaces.len() {
                let is_active = sr as usize == app.active_space_idx;
                let digit = char::from_digit((sr + 1) as u32, 10).unwrap_or(' ');
                if is_active {
                    (digit, bg_primary(), accent())
                } else {
                    (digit, fg_muted(), bg)
                }
            } else {
                (' ', fg_muted(), bg)
            }
        };

        for col in 0..w {
            let (cell_ch, cell_style) = if col == 0 || col == w - 1 {
                ('\u{2502}', Style::default().fg(border()).bg(bg_primary()))
            } else if col == mid_col {
                (ch, Style::default().fg(fg_val).bg(bg_val))
            } else {
                (' ', Style::default().bg(bg_val))
            };
            if let Some(cell) = frame.buffer_mut().cell_mut((x + col, y)) {
                cell.set_char(cell_ch).set_style(cell_style);
            }
        }
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
