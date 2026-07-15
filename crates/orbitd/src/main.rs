use std::sync::Arc;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use tokio::sync::broadcast;
use tracing::{error, info};

mod agent;
mod io;
mod ipc;
mod pty;
mod session;

use crate::session::SpaceManager;
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

fn lock_file_path() -> std::path::PathBuf {
    default_socket_path().with_extension("lock")
}

fn acquire_lock() -> Result<()> {
    let path = lock_file_path();
    if path.exists() {
        let pid_str = std::fs::read_to_string(&path)?;
        let pid: u32 = pid_str.trim().parse().unwrap_or(0);
        if pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0 {
            bail!("orbitd already running (PID {pid})");
        }
    }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

fn release_lock() {
    let _ = std::fs::remove_file(lock_file_path());
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
    info!("orbitd listening on {}", socket_path.display());

    let (event_bus, _rx) = broadcast::channel::<ServerEvent>(256);

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| ".".to_string());
    let space_manager = Arc::new(SpaceManager::new(event_bus, shell, cwd, 80, 24).await?);
    info!("orbitd ready — 1 space, 1 pane");

    tokio::select! {
        res = accept_loop(listener, space_manager.clone()) => {
            if let Err(e) = res { error!("accept loop error: {e:#}"); }
        }
        _ = wait_for_signal() => {}
    }

    info!("orbitd stopping...");
    let _ = std::fs::remove_file(&socket_path);
    release_lock();
    info!("orbitd stopped");
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
