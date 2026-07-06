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
    let (mut ipc, state) = IpcClient::connect()
        .await
        .context("failed to connect to orbitd — is the daemon running?")?;

    debug!("setting up terminal...");
    let mut terminal = tui::setup_terminal().context("failed to setup terminal")?;

    let term_cols = terminal.size()?.width;
    let term_rows = terminal.size()?.height;

    let sidebar_w: u16 = 14;
    let pane_cols = term_cols.saturating_sub(sidebar_w).max(20);
    let pane_rows = term_rows.saturating_sub(3).max(5);

    let mut app = App::from_welcome(&state, pane_cols, pane_rows);

    for &pid in &app.pane_tree.leaves() {
        let _ = ipc
            .send(&ClientMessage::ResizePane {
                pane_id: pid,
                cols: pane_cols,
                rows: pane_rows,
            })
            .await;
    }

    debug!("entering event loop");
    let run_result = events::run(&mut app, ipc, &mut terminal).await;

    let _ = tui::restore_terminal(&mut terminal);
    run_result
}
