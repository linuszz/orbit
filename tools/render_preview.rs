//! Headless TUI render preview: connects to orbitd, fetches full state, renders
//! the complete Orbit UI into a TestBackend, then dumps the result as ANSI-colored
//! text to stdout so you can inspect the live UI without a real terminal.
//!
//! Usage:
//!   cargo run --bin render_preview                     # default 160x45
//!   cargo run --bin render_preview -- 120 30           # custom size
//!   cargo run --bin render_preview -- 120 30 --plain   # no ANSI colors

use interprocess::local_socket::GenericFilePath;
use orbit_protocol::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cols: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(160);
    let rows: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(45);
    let plain = args.iter().any(|a| a == "--plain");

    // Connect to orbitd
    let path = socket_path();
    let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
    use interprocess::local_socket::tokio::prelude::*;
    let mut stream = interprocess::local_socket::tokio::Stream::connect(name).await
        .map_err(|e| anyhow::anyhow!("Could not connect to orbitd at {:?}: {e}\nIs the daemon running?", path))?;

    let hello = ClientMessage::Hello {
        client_version: "0.1.0-preview".to_string(),
        protocol_version: PROTOCOL_VERSION,
        capabilities: Capabilities::default(),
    };
    let bytes = encode_message(&hello)?;
    stream.write_all(&bytes).await?;

    // Read Welcome response
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;
    let (event, _): (ServerEvent, usize) = decode_message(&data)?;

    let state = match event {
        ServerEvent::Welcome { state, .. } => state,
        other => anyhow::bail!("Expected Welcome, got {other:?}"),
    };

    // Build App from state (same logic as the real orbit client)
    let sidebar_w: u16 = if cols < 80 { 5 } else { 24 };
    let total_cols = cols.saturating_sub(sidebar_w).max(20);
    let total_rows = rows.saturating_sub(3).max(5);

    let mut app = orbit_tui::app::App::from_welcome(&state, total_cols, total_rows);

    let settings = orbit_tui::app::load_settings();
    app.theme_name = settings.theme.clone();
    app.sidebar_visible = settings.sidebar_visible;
    app.agent_panel_visible = settings.agent_panel_visible;
    orbit_tui::tui::theme::set_theme(&app.theme_name);

    // Render into TestBackend
    let backend = TestBackend::new(cols, rows);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| orbit_tui::render(f, &app))?;

    // Output the rendered buffer as ANSI text
    let buf = terminal.backend().buffer().clone();
    eprintln!("=== Orbit TUI Preview ({}x{}) ===\n", cols, rows);

    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if plain {
                print!("{}", cell.symbol());
            } else {
                let fg_code = color_to_ansi_fg(cell.fg);
                let bg_code = color_to_ansi_bg(cell.bg);
                print!("{}{}{}\x1b[0m", fg_code, bg_code, cell.symbol());
            }
        }
        println!();
    }

    if !plain {
        eprintln!("\n(Rendered with ANSI colors. Use --plain for no colors.)");
    }

    Ok(())
}

#[allow(unreachable_patterns)]
fn color_to_ansi_fg(color: ratatui::style::Color) -> String {
    match color {
        ratatui::style::Color::Reset => String::new(),
        ratatui::style::Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
        ratatui::style::Color::Indexed(n) => format!("\x1b[38;5;{n}m"),
        ratatui::style::Color::Black => "\x1b[30m".to_string(),
        ratatui::style::Color::Red => "\x1b[31m".to_string(),
        ratatui::style::Color::Green => "\x1b[32m".to_string(),
        ratatui::style::Color::Yellow => "\x1b[33m".to_string(),
        ratatui::style::Color::Blue => "\x1b[34m".to_string(),
        ratatui::style::Color::Magenta => "\x1b[35m".to_string(),
        ratatui::style::Color::Cyan => "\x1b[36m".to_string(),
        ratatui::style::Color::White => "\x1b[37m".to_string(),
        ratatui::style::Color::Gray => "\x1b[90m".to_string(),
        ratatui::style::Color::DarkGray => "\x1b[90m".to_string(),
        ratatui::style::Color::LightRed => "\x1b[91m".to_string(),
        ratatui::style::Color::LightGreen => "\x1b[92m".to_string(),
        ratatui::style::Color::LightYellow => "\x1b[93m".to_string(),
        ratatui::style::Color::LightBlue => "\x1b[94m".to_string(),
        ratatui::style::Color::LightMagenta => "\x1b[95m".to_string(),
        ratatui::style::Color::LightCyan => "\x1b[96m".to_string(),
        _ => String::new(),
    }
}

#[allow(unreachable_patterns)]
fn color_to_ansi_bg(color: ratatui::style::Color) -> String {
    match color {
        ratatui::style::Color::Reset => String::new(),
        ratatui::style::Color::Rgb(r, g, b) => format!("\x1b[48;2;{r};{g};{b}m"),
        ratatui::style::Color::Indexed(n) => format!("\x1b[48;5;{n}m"),
        ratatui::style::Color::Black => "\x1b[40m".to_string(),
        ratatui::style::Color::Red => "\x1b[41m".to_string(),
        ratatui::style::Color::Green => "\x1b[42m".to_string(),
        ratatui::style::Color::Yellow => "\x1b[43m".to_string(),
        ratatui::style::Color::Blue => "\x1b[44m".to_string(),
        ratatui::style::Color::Magenta => "\x1b[45m".to_string(),
        ratatui::style::Color::Cyan => "\x1b[46m".to_string(),
        ratatui::style::Color::White => "\x1b[47m".to_string(),
        _ => String::new(),
    }
}
