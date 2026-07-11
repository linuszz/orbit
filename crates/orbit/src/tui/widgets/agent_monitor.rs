use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::app::App;
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, _app: &App) {
    let bg = Block::default()
        .style(Style::default().bg(BG_SECONDARY).fg(FG_MUTED))
        .borders(ratatui::widgets::Borders::LEFT)
        .border_style(Style::default().fg(BORDER));
    frame.render_widget(bg, area);

    let mut y = area.y;

    let header = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "SATELLITES",
            Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("0", Style::default().fg(FG_MUTED)),
        Span::raw(" "),
    ]);
    frame.render_widget(
        header,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    y += 1;

    let divider = Span::styled(
        "\u{2500}".repeat((area.width as usize).saturating_sub(1)),
        Style::default().fg(BORDER),
    );
    frame.render_widget(
        Line::from(divider),
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    y += 2;

    let empty = Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{25CB} \u{25CB} \u{25CB}", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(
        empty,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    y += 1;

    let msg = Line::from(vec![
        Span::raw(" "),
        Span::styled("No agents", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(
        msg,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
}

use ratatui::widgets::Block;
