//! Dump detailed pane state: cursor position, cursor_visible, cell content
//! around cursor, and raw escape sequences from live PaneOutput for 10s.
//!
//! Usage: cargo run --manifest-path tools/Cargo.toml --bin pane_dump [pane_id]

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

async fn send_msg(s: &mut interprocess::local_socket::tokio::Stream, msg: &ClientMessage) -> anyhow::Result<()> {
    let bytes = encode_message(msg)?;
    s.write_all(&bytes).await?;
    Ok(())
}

async fn read_one(s: &mut interprocess::local_socket::tokio::Stream) -> anyhow::Result<ServerEvent> {
    let mut len_buf = [0u8; 4];
    s.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    s.read_exact(&mut data).await?;
    let (msg, _): (ServerEvent, usize) = decode_message(&data)?;
    Ok(msg)
}

fn escape_bytes(data: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1b {
            out.push_str("\\e");
            i += 1;
        } else if b < 0x20 || b == 0x7f {
            out.push_str(&format!("\\x{b:02x}"));
            i += 1;
        } else {
            // try to decode utf-8
            let rest = &data[i..];
            match std::str::from_utf8(rest) {
                Ok(s) => {
                    let c = s.chars().next().unwrap();
                    out.push(c);
                    i += c.len_utf8();
                }
                Err(_) => {
                    out.push_str(&format!("\\x{b:02x}"));
                    i += 1;
                }
            }
        }
    }
    out
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let target_pane: Option<u32> = args.get(1).and_then(|s| s.parse().ok());

    let path = socket_path();
    let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
    use interprocess::local_socket::tokio::prelude::*;
    let mut stream = interprocess::local_socket::tokio::Stream::connect(name)
        .await
        .map_err(|e| anyhow::anyhow!("Cannot connect at {path:?}: {e}"))?;

    send_msg(&mut stream, &ClientMessage::Hello {
        client_version: "0.1.0-dump".to_string(),
        protocol_version: PROTOCOL_VERSION,
        capabilities: Capabilities::default(),
    }).await?;

    let state = match read_one(&mut stream).await? {
        ServerEvent::Welcome { state, .. } => state,
        other => anyhow::bail!("Expected Welcome, got {other:?}"),
    };

    // Find the active space
    let active_space = state.spaces.iter()
        .find(|s| s.id == state.active_space)
        .or_else(|| state.spaces.first())
        .ok_or_else(|| anyhow::anyhow!("No spaces"))?;

    println!("Active space: {:?} {:?}", active_space.id, active_space.name);

    for pane in &active_space.panes {
        let is_target = target_pane.map(|id| pane.id.0 == id).unwrap_or(true);
        if !is_target { continue; }

        let g = &pane.cell_grid;
        println!("\n=== Pane {:?}  {}x{}  cursor=({},{})  cursor_visible={} ===",
            pane.id, g.cols, g.rows, g.cursor_x, g.cursor_y, g.cursor_visible);

        // Dump rows around cursor
        let cy = g.cursor_y as usize;
        let start = cy.saturating_sub(3);
        let end = (cy + 4).min(g.rows as usize);

        for row in start..end {
            let row_start = row * g.cols as usize;
            let row_end = (row_start + g.cols as usize).min(g.cells.len());
            let cells = &g.cells[row_start..row_end];

            // Find last non-default cell
            let last_nonblank = cells.iter().rposition(|c| c.ch != ' ' && c.ch != '\0');
            let display_width = last_nonblank.map(|p| p + 1).unwrap_or(0).min(100);

            let cursor_marker = if row == cy { "<-- cursor" } else { "" };
            print!("  row {:3}: [", row);
            for (col, cell) in cells[..display_width].iter().enumerate() {
                if col == g.cursor_x as usize && row == cy {
                    print!("\x1b[7m"); // reverse video
                }
                let ch = cell.ch;
                if ch == '\0' { print!("·"); }
                else if ch == ' ' { print!(" "); }
                else { print!("{ch}"); }
                if col == g.cursor_x as usize && row == cy {
                    print!("\x1b[m");
                }
            }
            println!("] {cursor_marker}");

            // Also show raw byte values of the first non-space cells on cursor row
            if row == cy {
                print!("  row {:3}  bytes: ", row);
                for cell in cells[..display_width.min(40)].iter() {
                    let ch = cell.ch;
                    if ch == ' ' || ch == '\0' {
                        print!("  .  ");
                    } else {
                        print!(" {:04x}", ch as u32);
                    }
                }
                println!();
            }
        }
    }

    println!("\n=== Watching PaneOutput for 10s — raw escape sequences ===");
    println!("(interact with the pane now to see what bytes arrive)");

    let target_pane_id = target_pane.map(PaneId);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() { break; }
        match timeout(remaining, read_one(&mut stream)).await {
            Ok(Ok(ServerEvent::PaneOutput { pane_id, data })) => {
                let show = target_pane_id.map(|t| pane_id == t).unwrap_or(true);
                if show && !data.is_empty() {
                    let escaped = escape_bytes(&data);
                    // Show first 200 chars
                    let display = if escaped.len() > 200 { &escaped[..200] } else { &escaped };
                    println!("  PaneOutput {:?} ({} bytes): {}", pane_id, data.len(), display);
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => { eprintln!("read error: {e}"); break; }
            Err(_) => break,
        }
    }
    println!("Done.");
    Ok(())
}
