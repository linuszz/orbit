use orbit_protocol::{AgentInfo, AgentStatus};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{AgentHover, App};
use crate::tui::theme::*;

fn status_icon(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Working => "\u{25CF}", // ●
        AgentStatus::Idle => "\u{25CB}",    // ○
        AgentStatus::Blocked => "\u{25CE}", // ◎
        AgentStatus::Error => "\u{25C9}",   // ◉
        AgentStatus::Done => "\u{25CC}",    // ◌
    }
}

fn status_color(status: &AgentStatus) -> ratatui::style::Color {
    match status {
        AgentStatus::Working => ACCENT,
        AgentStatus::Idle => FG_MUTED,
        AgentStatus::Blocked => ACCENT_BLOCKED,
        AgentStatus::Error => ACCENT_ERROR,
        AgentStatus::Done => FG_MUTED,
    }
}

fn status_label(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Working => "Working",
        AgentStatus::Idle => "Standby",
        AgentStatus::Blocked => "Eclipse",
        AgentStatus::Error => "Debris",
        AgentStatus::Done => "Done",
    }
}

// Returns ([btn_label, is_danger]; 3 slots, each label exactly 6 chars)
fn card_buttons(status: &AgentStatus) -> [(&'static str, bool); 3] {
    match status {
        AgentStatus::Working => [("[View]", false), ("[Stop]", false), ("[Chat]", false)],
        AgentStatus::Idle => [("[View]", false), ("[Chat]", false), ("[Rmov]", true)],
        AgentStatus::Blocked => [("[View]", false), ("[Resp]", false), ("[Abrt]", true)],
        AgentStatus::Error => [("[View]", false), ("[Rstr]", false), ("[Rmov]", true)],
        AgentStatus::Done => [("[View]", false), ("[Chat]", false), ("[Rmov]", true)],
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('\u{2026}');
        t
    }
}

fn format_duration(secs: u32) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .style(Style::default().bg(BG_SECONDARY))
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(BORDER));
    frame.render_widget(block, area);

    let ix = area.x + 1; // inner x (after left border)
    let iw = area.width.saturating_sub(1); // inner width = 21

    let any_blocked = app.agents.iter().any(|a| a.status == AgentStatus::Blocked);
    let blocked_agents: Vec<&AgentInfo> = app
        .agents
        .iter()
        .filter(|a| a.status == AgentStatus::Blocked)
        .collect();

    // --- Header ---
    {
        let n = app.agents.len();
        let badge = format!("[{}]", n);
        let badge_color = if any_blocked {
            ACCENT_BLOCKED
        } else {
            FG_MUTED
        };
        // right side: "[+]×" = 4 chars
        let right_chars = 4u16;
        let fill = iw.saturating_sub(10 + 1 + badge.len() as u16 + right_chars) as usize;

        let (add_fg, add_bg) = if app.agent_hovered == Some(AgentHover::HeaderAdd) {
            (BG_PRIMARY, ACCENT_HOVER)
        } else {
            (FG_MUTED, BG_SECONDARY)
        };
        let (close_fg, close_bg) = if app.agent_hovered == Some(AgentHover::HeaderClose) {
            (BG_PRIMARY, ACCENT_ERROR)
        } else {
            (FG_MUTED, BG_SECONDARY)
        };

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "SATELLITES",
                    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(badge, Style::default().fg(badge_color)),
                Span::raw(" ".repeat(fill)),
                Span::styled("[+]", Style::default().fg(add_fg).bg(add_bg)),
                Span::styled("\u{00D7}", Style::default().fg(close_fg).bg(close_bg)),
            ])),
            Rect {
                x: ix,
                y: area.y,
                width: iw,
                height: 1,
            },
        );
    }

    // --- Divider ---
    let div_y = area.y + 1;
    frame.render_widget(
        Line::from(Span::styled(
            "\u{2500}".repeat(iw as usize),
            Style::default().fg(BORDER),
        )),
        Rect {
            x: ix,
            y: div_y,
            width: iw,
            height: 1,
        },
    );

    let mut y = area.y + 2;

    // --- Eclipse banner ---
    if !blocked_agents.is_empty() {
        let name_part = if blocked_agents.len() == 1 {
            truncate_str(&blocked_agents[0].name, 10)
        } else {
            format!("{} agents", blocked_agents.len())
        };
        let banner_text = format!(
            "{:<width$}",
            format!("\u{25CE} Eclipse: {}", name_part),
            width = iw as usize
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                banner_text,
                Style::default().fg(ACCENT_BLOCKED).bg(BG_TERTIARY),
            )),
            Rect {
                x: ix,
                y,
                width: iw,
                height: 1,
            },
        );
        y += 1;

        let (resp_fg, resp_bg) = if app.agent_hovered == Some(AgentHover::EclipseRespond) {
            (BG_PRIMARY, ACCENT_BLOCKED)
        } else {
            (ACCENT_BLOCKED, BG_TERTIARY)
        };
        let respond_fill = " ".repeat(iw.saturating_sub(11) as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ", Style::default().bg(BG_TERTIARY)),
                Span::styled("[Respond]", Style::default().fg(resp_fg).bg(resp_bg)),
                Span::styled(respond_fill, Style::default().bg(BG_TERTIARY)),
            ])),
            Rect {
                x: ix,
                y,
                width: iw,
                height: 1,
            },
        );
        y += 1;
    }

    // --- Cards or empty state ---
    if app.agents.is_empty() {
        let mid_y = (area.y + area.height) / 2;
        if mid_y >= y && mid_y < area.y + area.height {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(
                        "{:^width$}",
                        "\u{25CB} \u{25CB} \u{25CB}",
                        width = iw as usize
                    ),
                    Style::default().fg(FG_MUTED),
                ))),
                Rect {
                    x: ix,
                    y: mid_y,
                    width: iw,
                    height: 1,
                },
            );
            if mid_y + 1 < area.y + area.height {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("{:^width$}", "No satellites", width = iw as usize),
                        Style::default().fg(FG_MUTED),
                    ))),
                    Rect {
                        x: ix,
                        y: mid_y + 1,
                        width: iw,
                        height: 1,
                    },
                );
            }
        }
    } else {
        for (card_idx, agent) in app.agents.iter().enumerate() {
            if y + 5 > area.y + area.height {
                break;
            }
            render_card(frame, ix, y, iw, agent, card_idx, app);
            y += 5;
            if card_idx + 1 < app.agents.len() && y < area.y + area.height {
                frame.render_widget(
                    Line::from(Span::styled(
                        "\u{2500}".repeat(iw as usize),
                        Style::default().fg(BORDER),
                    )),
                    Rect {
                        x: ix,
                        y,
                        width: iw,
                        height: 1,
                    },
                );
                y += 1;
            }
        }
    }
}

fn render_card(
    frame: &mut Frame,
    x: u16,
    y: u16,
    w: u16,
    agent: &AgentInfo,
    card_idx: usize,
    app: &App,
) {
    let sc = status_color(&agent.status);
    let icon = status_icon(&agent.status);
    let label = status_label(&agent.status);

    // Row 0: icon + " " + name (11 cols) + " " + status (7 cols) = 21
    {
        let name_w = (w.saturating_sub(2 + 1 + 7)) as usize; // icon+sp + sp + status
        let name = truncate_str(&agent.name, name_w);
        let name_padded = format!("{:<width$}", name, width = name_w);
        let status_padded = format!("{:>7}", label);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(sc)),
                Span::raw(" "),
                Span::styled(
                    name_padded,
                    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(status_padded, Style::default().fg(sc)),
            ])),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
    }

    // Row 1: " " + model (left) + duration (right-aligned) — total w cols
    {
        let duration_s = agent.detail.as_ref().map(|d| d.duration_s).unwrap_or(0);
        let dur_str = if duration_s > 0 {
            format_duration(duration_s)
        } else {
            String::new()
        };
        let inner_w = w.saturating_sub(1) as usize; // 1 leading space
        let model_max = if dur_str.is_empty() {
            inner_w
        } else {
            inner_w.saturating_sub(dur_str.len() + 1) // space before duration
        };
        let model = truncate_str(&agent.model, model_max);
        let model_text = if dur_str.is_empty() {
            format!(" {:<width$}", model, width = inner_w)
        } else {
            let pad = inner_w.saturating_sub(model.len() + dur_str.len());
            format!(" {}{}{}", model, " ".repeat(pad), dur_str)
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                model_text,
                Style::default().fg(FG_MUTED),
            )])),
            Rect {
                x,
                y: y + 1,
                width: w,
                height: 1,
            },
        );
    }

    // Row 2: " " + task description or block message
    {
        let task_str = agent
            .detail
            .as_ref()
            .and_then(|d| match agent.status {
                AgentStatus::Blocked => d.block_msg.as_deref(),
                _ => d.task.as_deref(),
            })
            .unwrap_or("");
        let task = truncate_str(task_str, w.saturating_sub(1) as usize);
        let task_text = format!(" {:<width$}", task, width = w.saturating_sub(1) as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(task_text, Style::default().fg(FG_SECONDARY))),
            Rect {
                x,
                y: y + 2,
                width: w,
                height: 1,
            },
        );
    }

    // Row 3: progress bar (Working/Blocked with Some(progress)), or blank
    {
        let show_bar = matches!(agent.status, AgentStatus::Working | AgentStatus::Blocked);
        let progress = agent.detail.as_ref().and_then(|d| d.progress);
        if show_bar {
            let pct = progress.unwrap_or(0.0).clamp(0.0, 1.0);
            // " " + bar (w-5) + "xxx%" (4) = w
            let bar_w = w.saturating_sub(5) as usize;
            let filled = (pct * bar_w as f32) as usize;
            let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(bar_w - filled);
            let pct_text = format!("{:3.0}%", pct * 100.0);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(bar, Style::default().fg(sc)),
                    Span::styled(pct_text, Style::default().fg(FG_MUTED)),
                ])),
                Rect {
                    x,
                    y: y + 3,
                    width: w,
                    height: 1,
                },
            );
        } else {
            frame.render_widget(
                Paragraph::new("").style(Style::default().bg(BG_SECONDARY)),
                Rect {
                    x,
                    y: y + 3,
                    width: w,
                    height: 1,
                },
            );
        }
    }

    // Row 4: " " + [Btn1] + " " + [Btn2] + " " + [Btn3] = 1+6+1+6+1+6 = 21
    {
        let buttons = card_buttons(&agent.status);
        let mut spans = vec![Span::raw(" ")];
        for (slot, (btn_label, is_danger)) in buttons.iter().enumerate() {
            if slot > 0 {
                spans.push(Span::raw(" "));
            }
            let hovered = app.agent_hovered
                == Some(AgentHover::CardBtn {
                    card_idx,
                    slot: slot as u8,
                });
            let (fg, bg) = if hovered {
                (
                    BG_PRIMARY,
                    if *is_danger {
                        ACCENT_ERROR
                    } else {
                        ACCENT_HOVER
                    },
                )
            } else if *is_danger {
                (ACCENT_ERROR, BG_SECONDARY)
            } else {
                (FG_MUTED, BG_SECONDARY)
            };
            spans.push(Span::styled(*btn_label, Style::default().fg(fg).bg(bg)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x,
                y: y + 4,
                width: w,
                height: 1,
            },
        );
    }
}

/// Returns the row (absolute) where agent card `card_idx` starts, given panel geometry.
/// `panel_y`: top row of the agent panel.
/// `any_blocked`: whether the eclipse banner is showing (adds 2 rows).
pub fn card_start_row(panel_y: u16, any_blocked: bool, card_idx: usize) -> u16 {
    let base = panel_y + 2 + if any_blocked { 2 } else { 0 };
    base + card_idx as u16 * 6
}
