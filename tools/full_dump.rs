//! Full pane dump: show ALL cells including bg/fg colors, useful for diagnosing
//! missing selection highlights, wrong background rendering, etc.
//!
//! Usage: cargo run --manifest-path tools/Cargo.toml --bin full_dump [pane_id] [start_row] [end_row]

use interprocess::local_socket::GenericFilePath;
use orbit_protocol::*;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

fn color_name(c: &orbit_protocol::TermColor) -> String {
    match c {
        orbit_protocol::TermColor::Default => "def".to_string(),
        orbit_protocol::TermColor::Ansi(n) => format!("ansi{n}"),
        orbit_protocol::TermColor::Ansi256(n) => format!("256-{n}"),
        orbit_protocol::TermColor::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let target_pane: Option<u32> = args.get(1).and_then(|s| s.parse().ok());
    let start_row: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let end_row: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);

    let path = socket_path();
    let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
    use interprocess::local_socket::tokio::prelude::*;
    let mut stream = interprocess::local_socket::tokio::Stream::connect(name).await
        .map_err(|e| anyhow::anyhow!("Cannot connect: {e}"))?;

    send_msg(&mut stream, &ClientMessage::Hello {
        client_version: "0.1.0-full-dump".to_string(),
        protocol_version: PROTOCOL_VERSION,
        capabilities: Capabilities::default(),
    }).await?;

    let state = match read_one(&mut stream).await? {
        ServerEvent::Welcome { state, .. } => state,
        other => anyhow::bail!("Expected Welcome, got {other:?}"),
    };

    let active = state.spaces.iter().find(|s| s.id == state.active_space)
        .or_else(|| state.spaces.first())
        .ok_or_else(|| anyhow::anyhow!("No spaces"))?;

    for pane in &active.panes {
        let is_target = target_pane.map(|id| pane.id.0 == id).unwrap_or(true);
        if !is_target { continue; }

        let g = &pane.cell_grid;
        let cols = g.cols as usize;
        let rows = g.rows as usize;
        let actual_end = end_row.min(rows);

        println!("=== Pane {:?}  {}x{}  cursor=({},{})  cursor_visible={} ===",
            pane.id, cols, rows, g.cursor_x, g.cursor_y, g.cursor_visible);

        for row in start_row..actual_end {
            let row_start = row * cols;
            let row_end = (row_start + cols).min(g.cells.len());
            let cells = &g.cells[row_start..row_end];

            // Check if row has any non-default background
            let has_bg = cells.iter().any(|c| !matches!(c.bg, TermColor::Default));
            let _has_fg = cells.iter().any(|c| !matches!(c.fg, TermColor::Default));

            // Content line
            let content: String = cells.iter().map(|c| {
                if c.ch == '\0' { '·' } else if c.ch == ' ' { ' ' } else { c.ch }
            }).collect();
            let cursor_marker = if row == g.cursor_y as usize { " <cursor" } else { "" };
            // char-boundary safe truncation
            let display: String = content.chars().take(cols).collect();
            println!("  r{row:03}: |{}|{}", display, cursor_marker);

            // Color annotation for rows with interesting colors
            if has_bg {
                let mut annotation = String::from("  colors: ");
                let mut prev_bg = String::new();
                let mut run_start = 0;
                for (col, cell) in cells.iter().enumerate() {
                    let bg = color_name(&cell.bg);
                    if bg != prev_bg {
                        if !prev_bg.is_empty() && &prev_bg != "def" {
                            annotation.push_str(&format!("  bg={} cols {}-{}", prev_bg, run_start, col.saturating_sub(1)));
                        }
                        prev_bg = bg;
                        run_start = col;
                    }
                }
                if !prev_bg.is_empty() && &prev_bg != "def" {
                    annotation.push_str(&format!("  bg={} cols {}-{}", prev_bg, run_start, cells.len().saturating_sub(1)));
                }
                if annotation != "  colors: " {
                    println!("{annotation}");
                }
            }
        }
    }

    Ok(())
}
