pub mod agent;
pub mod io;
pub mod ipc;
pub mod pty;
pub mod session;

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use tokio::sync::broadcast;
use tracing::{error, info};

use self::session::SpaceManager;
use orbt_protocol::ServerEvent;

pub use orbt_protocol::default_socket_path;

fn lock_file_path() -> std::path::PathBuf {
    default_socket_path().with_extension("lock")
}

fn acquire_lock() -> Result<()> {
    let path = lock_file_path();
    if path.exists() {
        let pid_str = std::fs::read_to_string(&path)?;
        let pid: u32 = pid_str.trim().parse().unwrap_or(0);
        if pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0 {
            bail!("orbtd already running (PID {pid})");
        }
    }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

fn release_lock() {
    let _ = std::fs::remove_file(lock_file_path());
}

pub async fn run() -> Result<()> {
    acquire_lock().context("failed to acquire lock")?;

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
    info!("orbtd listening on {}", socket_path.display());

    let (event_bus, _rx) = broadcast::channel::<ServerEvent>(256);

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| ".".to_string());
    let space_manager = Arc::new(SpaceManager::new(event_bus, shell, cwd, 80, 24).await?);
    info!("orbtd ready — 1 space, 1 pane");

    {
        let sm = space_manager.clone();
        tokio::spawn(async move { sm.poll_cwd_changes(500).await });
    }

    tokio::select! {
        res = accept_loop(listener, space_manager.clone()) => {
            if let Err(e) = res { error!("accept loop error: {e:#}"); }
        }
        _ = wait_for_signal() => {}
    }

    info!("orbtd stopping...");
    let _ = std::fs::remove_file(&socket_path);
    release_lock();
    info!("orbtd stopped");
    Ok(())
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = term.recv() => info!("SIGTERM received, shutting down"),
            _ = tokio::signal::ctrl_c() => info!("SIGINT received, shutting down"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn accept_loop(
    listener: interprocess::local_socket::tokio::Listener,
    space_manager: Arc<SpaceManager>,
) -> Result<()> {
    loop {
        let stream = listener.accept().await?;
        let space_manager = space_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = ipc::handle_client(stream, space_manager).await {
                error!("client error: {e:#}");
            }
        });
    }
}
