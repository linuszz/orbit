mod app;
mod events;
mod ipc;
mod tui;

use anyhow::{Context, Result};
use orbit_protocol::ClientMessage;
use tracing::debug;

use crate::app::App;
use crate::ipc::IpcClient;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ORBIT_LOG_LEVEL")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    debug!("connecting to orbitd...");
    let ipc = IpcClient::connect()
        .await
        .context("failed to connect to orbitd — is the daemon running?")?;

    let (mut ipc, state) = ipc;

    let space = state
        .spaces
        .first()
        .context("orbitd returned no spaces in Welcome")?;
    let pane = space
        .panes
        .first()
        .context("orbitd returned no panes in Welcome")?;
    let pane_id = pane.id;

    debug!("setting up terminal...");
    let mut terminal = tui::setup_terminal().context("failed to setup terminal")?;

    let term_cols = terminal.size()?.width;
    let term_rows = terminal.size()?.height;

    let sidebar_w: u16 = 14;
    let overhead_rows: u16 = 3;

    let pane_cols = term_cols.saturating_sub(sidebar_w).max(20);
    let pane_rows = term_rows.saturating_sub(overhead_rows).max(5);

    let mut app = App::new(pane_cols, pane_rows, pane_id);
    app.space_name = space.name.clone();

    if let Some(pane_info) = state.spaces.first().and_then(|s| s.panes.first()) {
        app.apply_snapshot(&pane_info.cell_grid);
        app.parser.grid.resize(pane_cols, pane_rows);
    }

    let _ = ipc
        .send(&ClientMessage::ResizePane {
            pane_id,
            cols: pane_cols,
            rows: pane_rows,
        })
        .await;

    let _ = ipc
        .send(&ClientMessage::PaneInput {
            pane_id,
            data: b"\x0c".to_vec(),
        })
        .await;

    debug!("entering event loop (pane: {pane_cols}x{pane_rows})");
    let run_result = events::run(&mut app, ipc, &mut terminal).await;

    debug!("restoring terminal...");
    let _ = tui::restore_terminal(&mut terminal);

    run_result
}
