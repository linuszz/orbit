mod events;
mod ipc;

use anyhow::{Context, Result};
use orbit_protocol::ClientMessage;
use tracing::debug;

use crate::ipc::IpcClient;
use orbit_tui::app::{load_settings, App};
use orbit_tui::tui;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ORBIT_LOG_LEVEL")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    debug!("connecting to orbitd...");
    let (mut ipc, state) = IpcClient::connect()
        .await
        .context("failed to connect to orbitd — is the daemon running?")?;

    debug!("setting up terminal...");
    let mut terminal = tui::setup_terminal().context("failed to setup terminal")?;

    let term_cols = terminal.size()?.width;
    let term_rows = terminal.size()?.height;

    let sidebar_w: u16 = if term_cols < 80 {
        tui::SIDEBAR_COLLAPSED_W
    } else {
        tui::SIDEBAR_W
    };
    let total_cols = term_cols.saturating_sub(sidebar_w).max(20);
    let total_rows = term_rows.saturating_sub(3).max(5);

    let mut app = App::from_welcome(&state, total_cols, total_rows);

    let settings = load_settings();
    app.theme_name = settings.theme.clone();
    app.sidebar_visible = settings.sidebar_visible;
    app.agent_panel_visible = settings.agent_panel_visible;
    orbit_tui::tui::theme::set_theme(&app.theme_name);

    let pane_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: total_cols,
        height: total_rows,
    };
    let areas = tui::compute_leaf_areas(app.pane_tree(), pane_area);
    for (pid, rect) in areas {
        let pc = rect.width;
        let pr = rect.height.saturating_sub(2);
        if let Some(pane) = app.panes.get_mut(&pid) {
            pane.parser.grid.resize(pc, pr);
        }
        let _ = ipc
            .send(&ClientMessage::ResizePane {
                tab_id: app.active_tab_id,
                pane_id: pid,
                cols: pc,
                rows: pr,
            })
            .await;
    }

    let _ = ipc.send(&ClientMessage::RequestFullState).await;

    debug!("entering event loop");
    let run_result = events::run(&mut app, ipc, &mut terminal).await;

    let _ = tui::restore_terminal(&mut terminal);
    run_result
}
