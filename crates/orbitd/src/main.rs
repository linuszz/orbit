use std::sync::Arc;

use anyhow::{Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};

mod agent;
mod io;
mod ipc;
mod pty;
mod session;

use crate::session::SessionState;
use orbit_protocol::ServerEvent;

fn default_socket_path() -> std::path::PathBuf {
    let uid = unsafe { libc::getuid() };

    let runtime = format!("/run/user/{uid}");
    let runtime_dir = std::path::Path::new(&runtime);
    if runtime_dir.exists() {
        return runtime_dir.join("orbit.sock");
    }

    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::Path::new(&dir);
        if p.exists() {
            return p.join("orbit.sock");
        }
    }

    std::env::temp_dir().join(format!("orbit-{uid}.sock"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ORBIT_LOG_LEVEL")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    let socket_path = default_socket_path();
    let name = socket_path
        .to_str()
        .context("socket path is not valid UTF-8")?
        .to_fs_name::<GenericFilePath>()
        .context("failed to create socket name")?;

    if socket_path.exists() {
        std::fs::remove_file(&socket_path).ok();
    }

    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .with_context(|| format!("failed to bind socket at {}", socket_path.display()))?;
    info!("orbitd listening on {}", socket_path.display());

    let (event_bus, _rx) = broadcast::channel::<ServerEvent>(256);
    let (pty_input_tx, pty_input_rx) = mpsc::channel::<Vec<u8>>(64);

    let cols = 80u16;
    let rows = 24u16;

    let handles = pty::spawn_pty(
        orbit_protocol::PaneId(0),
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
        ".",
        cols,
        rows,
        event_bus.clone(),
        pty_input_rx,
    )
    .await
    .context("failed to spawn PTY")?;

    let session = Arc::new(SessionState::new(
        pty_input_tx,
        event_bus.clone(),
        handles.parser,
        handles.master,
    ));
    info!("orbitd ready — 1 space, 1 pane (bash)");

    tokio::select! {
        res = accept_loop(listener, session.clone()) => {
            if let Err(e) = res { error!("accept loop error: {e:#}"); }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("SIGINT received, shutting down");
        }
    }

    info!("orbitd stopped");
    let _ = std::fs::remove_file(&socket_path);
    Ok(())
}

async fn accept_loop(
    listener: interprocess::local_socket::tokio::Listener,
    session: Arc<SessionState>,
) -> Result<()> {
    loop {
        let stream = listener.accept().await?;
        let session = session.clone();
        tokio::spawn(async move {
            if let Err(e) = ipc::handle_client(stream, session).await {
                error!("client error: {e:#}");
            }
        });
    }
}
