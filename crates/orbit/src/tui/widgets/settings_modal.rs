use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::App;
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if !app.settings_open {
        return;
    }

    let modal_w = 44u16.min(area.width.saturating_sub(4));
    let modal_h = 12u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width - modal_w) / 2;
    let y = area.y + (area.height - modal_h) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_w,
        height: modal_h,
    };

    let dim = Block::default().style(Style::default().bg(ratatui::style::Color::Rgb(10, 10, 14)));
    frame.render_widget(dim, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .style(Style::default().bg(bg_secondary()).fg(fg_primary()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border()))
        .title(Span::styled(
            " Settings ",
            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(2),
        height: modal_area.height.saturating_sub(2),
    };

    let theme_display = if app.theme_name == "orbit" {
        "Orbit"
    } else {
        "Tokyo Night"
    };
    let sidebar_display = if app.sidebar_visible { "On" } else { "Off" };
    let agent_display = if app.agent_panel_visible { "On" } else { "Off" };

    let rows: Vec<(&str, &str)> = vec![
        ("Theme", theme_display),
        ("Sidebar", sidebar_display),
        ("Agent Panel", agent_display),
    ];

    for (i, (label, value)) in rows.iter().enumerate() {
        let is_selected = i == app.settings_selected;
        let row_y = inner.y + i as u16 + 1;
        let bg = if is_selected {
            bg_primary()
        } else {
            bg_secondary()
        };
        let fg_label = if is_selected {
            fg_primary()
        } else {
            fg_secondary()
        };
        let fg_value = if is_selected { accent() } else { fg_muted() };
        let marker = if is_selected {
            Span::styled("> ", Style::default().fg(accent()))
        } else {
            Span::raw("  ")
        };
        let val_width = inner.width.saturating_sub(label.len() as u16 + 6);
        let line = Line::from(vec![
            marker,
            Span::styled(*label, Style::default().fg(fg_label).bg(bg)),
            Span::raw(" ".repeat(val_width as usize)),
            Span::styled(format!("[{}]", value), Style::default().fg(fg_value).bg(bg)),
            Span::raw(
                " ".repeat(
                    inner
                        .width
                        .saturating_sub(label.len() as u16 + val_width + value.len() as u16 + 6)
                        as usize,
                ),
            ),
        ]);
        frame.render_widget(
            line,
            Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            },
        );
    }

    let footer_y = inner.y + inner.height.saturating_sub(1);
    let footer = Line::from(vec![
        Span::styled("Esc ", Style::default().fg(accent())),
        Span::styled("close  ", Style::default().fg(fg_muted())),
        Span::styled("\u{2191}\u{2193} ", Style::default().fg(accent())),
        Span::styled("navigate  ", Style::default().fg(fg_muted())),
        Span::styled("Enter ", Style::default().fg(accent())),
        Span::styled("toggle", Style::default().fg(fg_muted())),
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
