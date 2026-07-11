use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::{App, InputMode, COMMANDS};
use crate::tui::theme::*;

fn filter_indices(search: &str) -> Vec<usize> {
    if search.is_empty() {
        return (0..COMMANDS.len()).collect();
    }
    let s = search.to_lowercase();
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, c)| c.label.to_lowercase().contains(&s))
        .map(|(i, _)| i)
        .collect()
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if let InputMode::CommandPalette {
        search,
        selected,
        search_focused,
    } = &app.mode
    {
        let palette_w = 50u16.min(area.width.saturating_sub(4));
        let palette_h = 20u16.min(area.height.saturating_sub(4));
        let x = area.x + (area.width - palette_w) / 2;
        let y = area.y + (area.height - palette_h) / 2;
        let palette_area = Rect {
            x,
            y,
            width: palette_w,
            height: palette_h,
        };

        // Dim only the non-sidebar area so the active space card remains readable
        let sb_w = if app.sidebar_visible {
            crate::tui::SIDEBAR_W
        } else {
            crate::tui::SIDEBAR_COLLAPSED_W
        };
        let dim_area = Rect {
            x: area.x + sb_w,
            y: area.y,
            width: area.width.saturating_sub(sb_w),
            height: area.height,
        };
        let dim = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 14)));
        frame.render_widget(dim, dim_area);

        frame.render_widget(Clear, palette_area);

        let block = Block::default()
            .style(Style::default().bg(BG_SECONDARY).fg(FG_PRIMARY))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        frame.render_widget(block, palette_area);

        let inner = Rect {
            x: palette_area.x + 1,
            y: palette_area.y + 1,
            width: palette_area.width.saturating_sub(2),
            height: palette_area.height.saturating_sub(2),
        };

        let search_line = if search.is_empty() && !*search_focused {
            Line::from(vec![
                Span::styled("/ to search", Style::default().fg(FG_MUTED)),
                Span::raw("  "),
                Span::styled(
                    "up/down navigate  Enter select  Esc close",
                    Style::default().fg(FG_MUTED),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("> ", Style::default().fg(ACCENT)),
                Span::styled(search.as_str(), Style::default().fg(FG_PRIMARY)),
                Span::styled(
                    "_",
                    Style::default()
                        .fg(FG_PRIMARY)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ])
        };
        frame.render_widget(
            search_line,
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
        );

        let sep = Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(BORDER),
        );
        frame.render_widget(
            Line::from(sep),
            Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: 1,
            },
        );

        let filtered = filter_indices(search);
        let list_y = inner.y + 2;
        let list_h = inner.height.saturating_sub(3);
        let mut current_y = list_y;
        let mut render_idx = 0;

        let mut last_group = "";

        for (vis_idx, &cmd_idx) in filtered.iter().enumerate() {
            if render_idx >= list_h as usize {
                break;
            }

            let cmd = &COMMANDS[cmd_idx];

            if cmd.group != last_group && search.is_empty() {
                if current_y < list_y + list_h {
                    let group_line = Line::from(vec![Span::styled(
                        cmd.group.to_uppercase(),
                        Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
                    )]);
                    frame.render_widget(
                        group_line,
                        Rect {
                            x: inner.x,
                            y: current_y,
                            width: inner.width,
                            height: 1,
                        },
                    );
                    current_y += 1;
                    render_idx += 1;
                }
                last_group = cmd.group;
            }

            if current_y >= list_y + list_h {
                break;
            }

            let is_selected = vis_idx == *selected;
            let bg = if is_selected {
                BG_PRIMARY
            } else {
                BG_SECONDARY
            };
            let fg = if is_selected {
                FG_PRIMARY
            } else {
                FG_SECONDARY
            };
            let border = if is_selected { ACCENT } else { BG_SECONDARY };

            let mut spans = vec![
                Span::styled(
                    if is_selected { ">" } else { " " },
                    Style::default().fg(border),
                ),
                Span::raw(" "),
                Span::styled(cmd.label, Style::default().fg(fg).bg(bg)),
            ];

            if !cmd.shortcut.is_empty() {
                let pad = inner
                    .width
                    .saturating_sub(cmd.label.len() as u16 + cmd.shortcut.len() as u16 + 6);
                spans.push(Span::raw(" ".repeat(pad as usize)));
                spans.push(Span::styled(
                    cmd.shortcut,
                    Style::default().fg(ACCENT).bg(BG_TERTIARY),
                ));
            }

            let line = Line::from(spans);
            frame.render_widget(
                line,
                Rect {
                    x: inner.x,
                    y: current_y,
                    width: inner.width,
                    height: 1,
                },
            );
            current_y += 1;
            render_idx += 1;
        }

        if filtered.is_empty() {
            let empty = Line::from(vec![Span::styled(
                "No commands found",
                Style::default().fg(FG_MUTED),
            )]);
            frame.render_widget(
                empty,
                Rect {
                    x: inner.x,
                    y: list_y,
                    width: inner.width,
                    height: 1,
                },
            );
        }

        let footer_y = inner.y + inner.height.saturating_sub(1);
        let footer = Line::from(vec![
            Span::styled("Esc ", Style::default().fg(ACCENT)),
            Span::styled("close  ", Style::default().fg(FG_MUTED)),
            Span::styled("up/down ", Style::default().fg(ACCENT)),
            Span::styled("navigate  ", Style::default().fg(FG_MUTED)),
            Span::styled("Enter ", Style::default().fg(ACCENT)),
            Span::styled("select", Style::default().fg(FG_MUTED)),
        ]);
        frame.render_widget(
            footer,
            Rect {
                x: inner.x,
                y: footer_y,
                width: inner.width,
                height: 1,
            },
        );
    }
}

use ratatui::style::Color;
