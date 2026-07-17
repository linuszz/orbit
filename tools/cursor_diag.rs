//! Cursor diagnostic: connects to running orbitd, dumps cursor_visible for
//! every pane in every space, then watches live PaneOutput for 10s to show
//! cursor-state transitions as they arrive.
//!
//! Usage: cargo run --manifest-path tools/Cargo.toml --bin cursor_diag

use interprocess::local_socket::GenericFilePath;
use orbit_protocol::*;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let rt = format!("/run/user/{uid}");
    if std::path::Path::new(&rt).exists() {
        return PathBuf::from(rt).join("orbit.sock");
    }
    std::env::temp_dir().join(format!("orbit-{uid}.sock"))
}

async fn send_msg(
    stream: &mut interprocess::local_socket::tokio::Stream,
    msg: &ClientMessage,
) -> anyhow::Result<()> {
    let bytes = encode_message(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn read_one(
    stream: &mut interprocess::local_socket::tokio::Stream,
) -> anyhow::Result<ServerEvent> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;
    let (msg, _): (ServerEvent, usize) = decode_message(&data)?;
    Ok(msg)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = socket_path();
    let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
    use interprocess::local_socket::tokio::prelude::*;
    let mut stream = interprocess::local_socket::tokio::Stream::connect(name)
        .await
        .map_err(|e| anyhow::anyhow!("Cannot connect at {path:?}: {e}"))?;

    let hello = ClientMessage::Hello {
        client_version: "0.1.0-diag".to_string(),
        protocol_version: PROTOCOL_VERSION,
        capabilities: Capabilities::default(),
    };
    send_msg(&mut stream, &hello).await?;

    let state = match read_one(&mut stream).await? {
        ServerEvent::Welcome { state, .. } => state,
        other => anyhow::bail!("Expected Welcome, got {other:?}"),
    };

    println!("=== Cursor Diagnostic ===");
    println!("active_space: {:?}", state.active_space);
    println!();

    for space in &state.spaces {
        let active_marker = if space.id == state.active_space { " [ACTIVE]" } else { "" };
        println!("Space {:?} {:?}{}", space.id, space.name, active_marker);
        println!("  active_tab: {:?}", space.active_tab);
        for tab in &space.tabs {
            let t_marker = if tab.id == space.active_tab { "*" } else { " " };
            println!("  {}Tab {:?} {:?}  active_pane: {:?}", t_marker, tab.id, tab.name, tab.active_pane);
        }
        for pane in &space.panes {
            println!(
                "    Pane {:?}  {}x{}  cursor=({},{})  cursor_visible={}",
                pane.id,
                pane.cell_grid.cols,
                pane.cell_grid.rows,
                pane.cell_grid.cursor_x,
                pane.cell_grid.cursor_y,
                pane.cell_grid.cursor_visible,
            );
            // Show last 5 rows of the pane to see what's on screen
            let rows = pane.cell_grid.rows as usize;
            let cols = pane.cell_grid.cols as usize;
            let start_row = rows.saturating_sub(5);
            println!("    --- last 5 rows ---");
            for row in start_row..rows {
                let mut line = String::new();
                for col in 0..cols {
                    let idx = row * cols + col;
                    if idx < pane.cell_grid.cells.len() {
                        let ch = pane.cell_grid.cells[idx].ch;
                        if ch == '\0' || ch == '\x00' {
                            line.push(' ');
                        } else {
                            line.push(ch);
                        }
                    }
                }
                println!("    {:2}: {}", row, line.trim_end());
            }
            println!("    -------------------");
        }
        println!();
    }

    println!("=== Watching live events for 15s ===");
    println!("(open claude code, type something, watch cursor_visible transitions)");
    println!();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, read_one(&mut stream)).await {
            Ok(Ok(ServerEvent::PaneOutput { pane_id, data })) => {
                // Check for CSI ?25h / CSI ?25l in the data
                let has_hide = contains_cursor_hide(&data);
                let has_show = contains_cursor_show(&data);
                if has_hide || has_show {
                    let s: String = data.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
                    println!(
                        "PaneOutput {:?}  {} bytes  HIDE={} SHOW={}  raw={}",
                        pane_id,
                        data.len(),
                        has_hide,
                        has_show,
                        &s[..s.len().min(80)]
                    );
                }
            }
            Ok(Ok(ServerEvent::SpaceUpdated(info))) => {
                println!("SpaceUpdated — active_tab: {:?}", info.active_tab);
                for pane in &info.panes {
                    println!(
                        "  Pane {:?} cursor_visible={}  cursor=({},{})",
                        pane.id,
                        pane.cell_grid.cursor_visible,
                        pane.cell_grid.cursor_x,
                        pane.cell_grid.cursor_y,
                    );
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                eprintln!("read error: {e}");
                break;
            }
            Err(_) => break, // timeout
        }
    }

    println!("Done.");
    Ok(())
}

fn contains_cursor_hide(data: &[u8]) -> bool {
    // ESC [ ? 2 5 l
    data.windows(6).any(|w| w == b"\x1b[?25l")
}

fn contains_cursor_show(data: &[u8]) -> bool {
    // ESC [ ? 2 5 h
    data.windows(6).any(|w| w == b"\x1b[?25h")
}
