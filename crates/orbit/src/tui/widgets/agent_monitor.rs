use orbit_protocol::{AgentInfo, AgentMetrics, AgentStatus};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{AgentHover, App, InputMode};
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

/// Smooth lerp between two u8 values at phase in [0.0, 1.0].
#[inline(always)]
fn lerp_u8(a: u8, b: u8, phase: f32) -> u8 {
    (a as f32 + phase * (b as f32 - a as f32)) as u8
}

/// Triangle wave: returns phase in [0.0, 1.0] over `period` ticks, peaking at mid-cycle.
#[inline(always)]
fn triangle_phase(tick: u64, period: u64) -> f32 {
    let t = (tick % period) as f32;
    let half = period as f32 / 2.0;
    if t < half {
        t / half
    } else {
        (period as f32 - t) / half
    }
}

/// Working slow pulse color (90 ticks / ~1.5 s): ACCENT_DIM → ACCENT_BRIGHT.
pub fn working_pulse_color(tick: u64) -> ratatui::style::Color {
    let p = triangle_phase(tick, 90);
    ratatui::style::Color::Rgb(
        lerp_u8(120, 251, p), // #783c00 → #fba028
        lerp_u8(60, 160, p),
        lerp_u8(0, 40, p),
    )
}

/// Blocked fast pulse color (48 ticks / ~0.8 s): dark gold → ACCENT_BLOCKED.
pub fn blocked_pulse_color(tick: u64) -> ratatui::style::Color {
    let p = triangle_phase(tick, 48);
    ratatui::style::Color::Rgb(
        lerp_u8(100, 217, p), // dim → #d9ac00
        lerp_u8(85, 172, p),
        0,
    )
}

/// Error blink color (60 ticks / ~1.0 s): dark red → ACCENT_ERROR.
pub fn error_blink_color(tick: u64) -> ratatui::style::Color {
    let p = triangle_phase(tick, 60);
    ratatui::style::Color::Rgb(
        lerp_u8(80, 200, p), // dark red → #c8321e
        lerp_u8(10, 50, p),
        lerp_u8(5, 30, p),
    )
}

/// Animated status color per spec §3.3 animation table.
fn animated_status_color(status: &AgentStatus, tick: u64) -> ratatui::style::Color {
    match status {
        AgentStatus::Working => working_pulse_color(tick),
        AgentStatus::Blocked => blocked_pulse_color(tick),
        AgentStatus::Error => error_blink_color(tick),
        _ => status_color(status),
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

fn format_rss(rss_kb: u32) -> String {
    if rss_kb < 1024 {
        format!("{rss_kb}k")
    } else if rss_kb < 1024 * 1024 {
        format!("{}M", rss_kb / 1024)
    } else {
        format!("{}G", rss_kb / 1024 / 1024)
    }
}

fn format_duration(secs: u32) -> String {
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
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
        let badge = if app.agent_scroll_offset > 0 {
            format!("[{}/{}]", n.saturating_sub(app.agent_scroll_offset), n)
        } else {
            format!("[{}]", n)
        };
        // Badge pulses with smooth Blocked animation when any agent is blocked.
        let badge_color = if any_blocked {
            blocked_pulse_color(app.tick_count)
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

    // --- "N above" scroll indicator ---
    if app.agent_scroll_offset > 0 && y < area.y + area.height {
        let above_text = format!(" \u{25B4} {} above", app.agent_scroll_offset);
        frame.render_widget(
            Paragraph::new(Span::styled(above_text, Style::default().fg(FG_MUTED))),
            Rect {
                x: ix,
                y,
                width: iw,
                height: 1,
            },
        );
        y += 1;
    }

    // --- Eclipse banner ---
    if !blocked_agents.is_empty() {
        let name_part = if blocked_agents.len() == 1 {
            truncate_str(&blocked_agents[0].name, 10)
        } else {
            format!("{} agents", blocked_agents.len())
        };
        // Eclipse icon pulses with smooth Blocked animation (48-tick / ~0.8 s cycle).
        let icon_color = blocked_pulse_color(app.tick_count);
        let text_part = format!(
            "{:<width$}",
            format!(" Eclipse: {}", name_part),
            width = iw.saturating_sub(1) as usize
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("\u{25CE}", Style::default().fg(icon_color).bg(BG_TERTIARY)),
                Span::styled(
                    text_part,
                    Style::default().fg(ACCENT_BLOCKED).bg(BG_TERTIARY),
                ),
            ])),
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
                        format!("{:^width$}", "No satellites running", width = iw as usize),
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
        let visible_agents: Vec<&AgentInfo> =
            app.agents.iter().skip(app.agent_scroll_offset).collect();
        let total = app.agents.len();
        // Reserve 1 row at the bottom for the "[+] Add Satellite" footer.
        let content_bottom = area.y + area.height.saturating_sub(1);
        for (card_idx, agent) in visible_agents.iter().enumerate() {
            if y + 5 > content_bottom {
                // Show "▼ N more" indicator when cards are truncated (above footer).
                let remaining = total - app.agent_scroll_offset - card_idx;
                if remaining > 0 && content_bottom >= 1 && y < content_bottom {
                    let more_text = format!(" \u{25BE} {} more", remaining);
                    frame.render_widget(
                        Paragraph::new(Span::styled(more_text, Style::default().fg(FG_MUTED))),
                        Rect {
                            x: ix,
                            y: content_bottom.saturating_sub(1),
                            width: iw,
                            height: 1,
                        },
                    );
                }
                break;
            }
            let metrics = app.agent_metrics.get(&agent.id);
            render_card(frame, ix, y, iw, agent, card_idx, app, metrics);
            y += 5;
            if card_idx + 1 < visible_agents.len() && y < area.y + area.height {
                // Blank separator row between cards (per design spec §5.1).
                frame.render_widget(
                    Paragraph::new("").style(Style::default().bg(BG_SECONDARY)),
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

    render_footer(frame, ix, iw, area, app);
}

fn render_card(
    frame: &mut Frame,
    x: u16,
    y: u16,
    w: u16,
    agent: &AgentInfo,
    card_idx: usize,
    app: &App,
    metrics: Option<&AgentMetrics>,
) {
    let sc = animated_status_color(&agent.status, app.tick_count);
    let icon = status_icon(&agent.status);
    let label = status_label(&agent.status);

    // Keyboard selection: card is highlighted when AgentPanel nav mode targets it.
    let is_selected = if let InputMode::AgentPanel { selected } = &app.mode {
        *selected == card_idx + app.agent_scroll_offset
    } else {
        false
    };
    let card_bg = if is_selected { BG_CARD } else { BG_SECONDARY };
    // Leading accent mark: orange ▸ for keyboard-selected cards; animated ▌ for blocked/error
    // cards (left-border accent, spec §3.3 "边框: Warning"); plain space otherwise.
    let sel_mark = if is_selected {
        Span::styled("\u{25B8}", Style::default().fg(ACCENT).bg(card_bg)) // ▸ orange selection
    } else {
        match agent.status {
            AgentStatus::Blocked => Span::styled(
                "\u{258C}", // ▌ half-block left border
                Style::default()
                    .fg(blocked_pulse_color(app.tick_count))
                    .bg(card_bg),
            ),
            AgentStatus::Error => Span::styled(
                "\u{258C}",
                Style::default()
                    .fg(error_blink_color(app.tick_count))
                    .bg(card_bg),
            ),
            _ => Span::styled(" ", Style::default().bg(card_bg)),
        }
    };

    // Row 0: icon + sel_mark + name (left) + status + dur (right-aligned).
    // Layout: icon(1) + mark(1) + name_padded + " " + right_part
    // right_part = "{label} {dur}" or just "{label}" when duration=0.
    {
        let duration_s = app
            .agent_start_times
            .get(&agent.id)
            .map(|t| t.elapsed().as_secs() as u32)
            .or_else(|| agent.detail.as_ref().map(|d| d.duration_s))
            .unwrap_or(0);
        let right_part = if duration_s > 0 {
            format!("{} {}", label, format_duration(duration_s))
        } else {
            label.to_string()
        };
        // name fills the space between icon+mark(2) and " "+right_part.
        let right_len = (1 + right_part.len()) as u16; // leading space + right_part
        let name_w = w.saturating_sub(2 + right_len) as usize;
        let name = truncate_str(&agent.name, name_w);
        let name_padded = format!("{:<width$}", name, width = name_w);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(sc).bg(card_bg)),
                sel_mark.clone(),
                Span::styled(
                    name_padded,
                    Style::default()
                        .fg(FG_PRIMARY)
                        .bg(card_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().bg(card_bg)),
                Span::styled(right_part, Style::default().fg(sc).bg(card_bg)),
            ])),
            Rect {
                x,
                y,
                width: w,
                height: 1,
            },
        );
    }

    // Row 1: sel_mark + "cwd · model" (left) + rss (right).
    {
        let rss_str = metrics.and_then(|m| m.rss_kb).map(format_rss);
        let inner_w = w.saturating_sub(1) as usize; // sel_mark takes col 0

        // Short cwd: basename of the space's working directory.
        let cwd_short = app
            .spaces
            .iter()
            .find(|s| s.space_id == agent.space_id)
            .and_then(|s| {
                std::path::Path::new(&s.cwd)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            });

        // left = "cwd · model" — omit separator when model is empty.
        let left_content = match (&cwd_short, agent.model.is_empty()) {
            (Some(cwd), false) if !cwd.is_empty() => {
                format!("{} \u{00B7} {}", cwd, agent.model)
            }
            (Some(cwd), true) if !cwd.is_empty() => cwd.clone(),
            (_, false) => agent.model.clone(),
            _ => String::new(),
        };
        let right = rss_str.unwrap_or_default();
        let left_max = if right.is_empty() {
            inner_w
        } else {
            inner_w.saturating_sub(right.len() + 1)
        };
        let left = truncate_str(&left_content, left_max);
        let model_body = if right.is_empty() {
            format!("{:<width$}", left, width = inner_w)
        } else {
            let pad = inner_w.saturating_sub(left.len() + right.len());
            format!("{}{}{}", left, " ".repeat(pad), right)
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                sel_mark.clone(),
                Span::styled(model_body, Style::default().fg(FG_MUTED).bg(card_bg)),
            ])),
            Rect {
                x,
                y: y + 1,
                width: w,
                height: 1,
            },
        );
    }

    // Row 2: task/block_msg; when Working, prefer live recent_lines activity.
    {
        let task_str = match agent.status {
            AgentStatus::Blocked => agent
                .detail
                .as_ref()
                .and_then(|d| d.block_msg.as_deref())
                .unwrap_or(""),
            AgentStatus::Working => {
                // Show live activity line when available; fall back to task.
                metrics
                    .and_then(|m| m.recent_lines.first().map(String::as_str))
                    .or_else(|| agent.detail.as_ref().and_then(|d| d.task.as_deref()))
                    .unwrap_or("")
            }
            _ => agent
                .detail
                .as_ref()
                .and_then(|d| d.task.as_deref())
                .unwrap_or(""),
        };
        let task = truncate_str(task_str, w.saturating_sub(1) as usize);
        let task_body = format!("{:<width$}", task, width = w.saturating_sub(1) as usize);
        // Blocked: block reason highlighted in ACCENT_BLOCKED + Bold (spec §7.1 Level 2).
        // Error: error text in ACCENT_ERROR.
        let (task_fg, task_mod) = match agent.status {
            AgentStatus::Blocked => (ACCENT_BLOCKED, Modifier::BOLD),
            AgentStatus::Error => (ACCENT_ERROR, Modifier::empty()),
            _ => (FG_SECONDARY, Modifier::empty()),
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                sel_mark.clone(),
                Span::styled(
                    task_body,
                    Style::default()
                        .fg(task_fg)
                        .bg(card_bg)
                        .add_modifier(task_mod),
                ),
            ])),
            Rect {
                x,
                y: y + 2,
                width: w,
                height: 1,
            },
        );
    }

    // Row 3: progress bar (Working/Blocked/Error show bar; spec §3.3).
    {
        let show_bar = matches!(
            agent.status,
            AgentStatus::Working | AgentStatus::Blocked | AgentStatus::Error
        );
        let progress = agent.detail.as_ref().and_then(|d| d.progress);
        if show_bar {
            // " " + bar (w-5) + suffix (4) = w
            let bar_w = w.saturating_sub(5) as usize;
            let (bar, suffix) = if let Some(pct) = progress {
                let pct = pct.clamp(0.0, 1.0);
                let filled = (pct * bar_w as f32) as usize;
                let b = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(bar_w - filled);
                (b, format!("{:3.0}%", pct * 100.0))
            } else {
                // Indeterminate: 4-cell window scrolling over bar_w cells.
                let window = 4usize;
                let cycle = (bar_w + window + 2) as u64;
                let pos = ((app.tick_count / 5) % cycle) as usize;
                let b: String = (0..bar_w)
                    .map(|c| {
                        if c >= pos && c < pos + window {
                            "\u{2588}"
                        } else {
                            "\u{2591}"
                        }
                    })
                    .collect();
                // Suffix: show cpu% when available, else blank (4 chars).
                let sfx = metrics
                    .and_then(|m| m.cpu_percent)
                    .map(|c| format!("{:3.0}%", c))
                    .unwrap_or_else(|| "    ".to_string());
                (b, sfx)
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    sel_mark.clone(),
                    Span::styled(bar, Style::default().fg(sc).bg(card_bg)),
                    Span::styled(suffix, Style::default().fg(FG_MUTED).bg(card_bg)),
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
                Paragraph::new(Line::from(vec![
                    sel_mark.clone(),
                    Span::styled(
                        " ".repeat(w.saturating_sub(1) as usize),
                        Style::default().bg(card_bg),
                    ),
                ])),
                Rect {
                    x,
                    y: y + 3,
                    width: w,
                    height: 1,
                },
            );
        }
    }

    // Row 4: sel_mark + [Btn1] + " " + [Btn2] + " " + [Btn3] = 1+6+1+6+1+6 = 21
    {
        let buttons = card_buttons(&agent.status);
        let mut spans = vec![sel_mark];
        for (slot, (btn_label, is_danger)) in buttons.iter().enumerate() {
            if slot > 0 {
                spans.push(Span::styled(" ", Style::default().bg(card_bg)));
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
                (ACCENT_ERROR, card_bg)
            } else {
                (FG_MUTED, card_bg)
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

/// Footer: "[+] Add Satellite" pinned to the last row of the agent panel.
fn render_footer(frame: &mut Frame, ix: u16, iw: u16, area: Rect, app: &App) {
    let footer_y = area.y + area.height.saturating_sub(1);
    let (fg, bg) = if app.agent_hovered == Some(AgentHover::PanelFooter) {
        (BG_PRIMARY, ACCENT_HOVER)
    } else {
        (FG_MUTED, BG_SECONDARY)
    };
    let label = format!("{:<width$}", " [+] Add Satellite", width = iw as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(fg).bg(bg))),
        Rect {
            x: ix,
            y: footer_y,
            width: iw,
            height: 1,
        },
    );
}

/// Returns the row (absolute) where agent card `card_idx` starts, given panel geometry.
/// `panel_y`: top row of the agent panel.
/// `scroll_offset`: number of agents scrolled past (adds 1 row for "N above" indicator).
/// `any_blocked`: whether the eclipse banner is showing (adds 2 rows).
pub fn card_start_row(
    panel_y: u16,
    scroll_offset: usize,
    any_blocked: bool,
    card_idx: usize,
) -> u16 {
    let above_row = if scroll_offset > 0 { 1u16 } else { 0 };
    let blocked_rows = if any_blocked { 2u16 } else { 0 };
    panel_y + 2 + above_row + blocked_rows + card_idx as u16 * 6
}
