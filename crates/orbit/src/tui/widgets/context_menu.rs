use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::{App, ContextMenuItem, InputMode};
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(menu) = &app.context_menu {
        let max_label_len = menu
            .items
            .iter()
            .filter_map(|i| match i {
                ContextMenuItem::Action {
                    label, shortcut, ..
                } => Some(label.len() + shortcut.len() + 2),
                _ => None,
            })
            .max()
            .unwrap_or(16);
        let w = (max_label_len as u16 + 4).min(area.width.saturating_sub(2));
        let h = (menu.items.len() as u16 + 2).min(area.height.saturating_sub(2));
        let x = menu.x.min(area.width.saturating_sub(w));
        let y = menu.y.min(area.height.saturating_sub(h));
        let menu_area = Rect {
            x,
            y,
            width: w,
            height: h,
        };

        frame.render_widget(Clear, menu_area);

        let block = Block::default()
            .style(Style::default().bg(bg_secondary()).fg(fg_primary()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border()));
        frame.render_widget(block, menu_area);

        for (i, item) in menu.items.iter().enumerate() {
            let row_y = menu_area.y + 1 + i as u16;
            match item {
                ContextMenuItem::Action {
                    label, shortcut, ..
                } => {
                    let is_selected = i == menu.selected;
                    let bg = if is_selected {
                        bg_primary()
                    } else {
                        bg_secondary()
                    };
                    let label_style = if is_selected {
                        Style::default()
                            .fg(fg_primary())
                            .bg(bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg_secondary()).bg(bg)
                    };
                    let shortcut_style = Style::default().fg(accent()).bg(bg);

                    let label_span = Span::styled(format!(" {label}"), label_style);
                    let mut spans = vec![label_span];
                    if !shortcut.is_empty() {
                        let pad = (menu_area.width as usize)
                            .saturating_sub(label.len() + shortcut.len() + 4);
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.push(Span::styled(shortcut.clone(), shortcut_style));
                    }

                    let line = Line::from(spans);
                    frame.render_widget(
                        line,
                        Rect {
                            x: menu_area.x,
                            y: row_y,
                            width: menu_area.width,
                            height: 1,
                        },
                    );
                }
                ContextMenuItem::Separator => {
                    let line = Line::from(Span::styled(
                        "\u{2500}".repeat(menu_area.width as usize),
                        Style::default().fg(border()),
                    ));
                    frame.render_widget(
                        line,
                        Rect {
                            x: menu_area.x,
                            y: row_y,
                            width: menu_area.width,
                            height: 1,
                        },
                    );
                }
            }
        }
    }

    if matches!(app.mode, InputMode::CommandPalette { .. }) {
        super::command_palette::render(frame, area, app);
    }
}
