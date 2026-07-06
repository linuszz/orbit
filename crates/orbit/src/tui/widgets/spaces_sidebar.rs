use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::app::App;
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let bg = Block::default().style(Style::default().bg(BG_SECONDARY).fg(FG_MUTED));
    frame.render_widget(bg, area);

    if app.sidebar_visible {
        render_expanded(frame, area, app);
    } else {
        render_collapsed(frame, area, app);
    }
}

fn render_expanded(frame: &mut Frame, area: Rect, app: &App) {
    let mut y = area.y;

    let header = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "SPACES",
            Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("\u{00AB}", Style::default().fg(FG_MUTED)),
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
        "\u{2500}".repeat(area.width as usize),
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
    y += 1;
    y += 1;

    let card = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            &app.space_name,
            Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        card,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    y += 1;

    let task = Line::from(vec![
        Span::raw(" "),
        Span::styled("shell", Style::default().fg(FG_SECONDARY)),
    ]);
    frame.render_widget(
        task,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    y += 1;

    let status_line = if app.agent_panel_visible {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("○ idle", Style::default().fg(ACCENT_IDLE)),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("○ idle", Style::default().fg(FG_MUTED)),
        ])
    };
    frame.render_widget(
        status_line,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
    let bottom_y = area.y + area.height.saturating_sub(1);
    let actions = Line::from(vec![
        Span::styled(" [+] ", Style::default().fg(ACCENT).bg(BG_CARD)),
        Span::styled("[>]", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(
        actions,
        Rect {
            x: area.x,
            y: bottom_y,
            width: area.width,
            height: 1,
        },
    );
}

fn render_collapsed(frame: &mut Frame, area: Rect, _app: &App) {
    let num = Line::from(vec![Span::styled(
        "1",
        Style::default()
            .fg(BG_PRIMARY)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD),
    )]);
    let cx = area.x + area.width / 2;
    frame.render_widget(
        num,
        Rect {
            x: cx,
            y: area.y + 2,
            width: 1,
            height: 1,
        },
    );

    let expand = Span::styled("\u{00BB}", Style::default().fg(FG_MUTED));
    frame.render_widget(
        Line::from(vec![Span::raw(" "), expand]),
        Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(2),
            width: area.width,
            height: 1,
        },
    );
}

use ratatui::widgets::Block;
