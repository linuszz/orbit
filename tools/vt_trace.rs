//! VT trace: connect to running orbtd, watch a pane's raw output for N seconds,
//! decode and display all escape sequences for diagnosis.
//!
//! Usage: cargo run --manifest-path tools/Cargo.toml --bin vt_trace [pane_id] [seconds]

use interprocess::local_socket::GenericFilePath;
use orbt_protocol::*;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let rt = format!("/run/user/{uid}");
    if std::path::Path::new(&rt).exists() {
        return PathBuf::from(rt).join("orbt.sock");
    }
    std::env::temp_dir().join(format!("orbt-{uid}.sock"))
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

fn decode_escapes(data: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1b {
            i += 1;
            if i >= data.len() {
                out.push_str("ESC");
                break;
            }
            match data[i] {
                b'[' => {
                    // CSI sequence
                    i += 1;
                    let mut params = String::new();
                    let mut intermediates = String::new();
                    while i < data.len() {
                        let c = data[i];
                        if c >= 0x30 && c <= 0x3f {
                            params.push(c as char);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    while i < data.len() {
                        let c = data[i];
                        if c >= 0x20 && c <= 0x2f {
                            intermediates.push(c as char);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    if i < data.len() {
                        let final_byte = data[i] as char;
                        i += 1;
                        let desc = describe_csi(&params, &intermediates, final_byte);
                        out.push_str(&format!("CSI({params}{intermediates}{final_byte}={desc}) "));
                    }
                }
                b']' => {
                    // OSC
                    i += 1;
                    let start = i;
                    while i < data.len() && !(data[i] == 0x07 || (data[i] == 0x1b && i + 1 < data.len() && data[i+1] == b'\\')) {
                        i += 1;
                    }
                    let content = std::str::from_utf8(&data[start..i]).unwrap_or("?");
                    if i < data.len() { i += 1; } // ST or BEL
                    out.push_str(&format!("OSC({}) ", &content[..content.len().min(30)]));
                }
                b'(' | b')' => {
                    i += 1;
                    if i < data.len() { i += 1; }
                    out.push_str("ESC(charsetignored) ");
                }
                c => {
                    out.push_str(&format!("ESC({}) ", c as char));
                    i += 1;
                }
            }
        } else if b < 0x20 {
            match b {
                0x07 => { out.push_str("BEL "); i += 1; }
                0x08 => { out.push_str("BS "); i += 1; }
                0x09 => { out.push_str("TAB "); i += 1; }
                0x0a => { out.push_str("LF "); i += 1; }
                0x0d => { out.push_str("CR "); i += 1; }
                _ => { out.push_str(&format!("^{:02x} ", b)); i += 1; }
            }
        } else {
            // printable text run
            let start = i;
            while i < data.len() && data[i] >= 0x20 && data[i] != 0x7f {
                i += 1;
            }
            if let Ok(s) = std::str::from_utf8(&data[start..i]) {
                if s.len() > 40 {
                    out.push_str(&format!("\"{}...\"({}) ", &s[..40], s.len()));
                } else {
                    out.push_str(&format!("\"{}\" ", s));
                }
            } else {
                out.push_str(&format!("[{} bytes] ", i - start));
            }
        }
    }
    out
}

fn describe_csi(params: &str, _inter: &str, final_byte: char) -> &'static str {
    match final_byte {
        'A' => "CUU",
        'B' => "CUD",
        'C' => "CUF",
        'D' => "CUB",
        'H' | 'f' => "CUP",
        'J' => "ED",
        'K' => "EL",
        'L' => "IL",
        'M' => "DL",
        'P' => "DCH",
        'S' => "SU",
        'T' => "SD",
        'X' => "ECH",
        'h' if params.starts_with('?') => "DEC-SET",
        'l' if params.starts_with('?') => "DEC-RST",
        'm' => "SGR",
        'n' => "DSR",
        'r' => "DECSTBM",
        's' => "DECSC",
        'u' => "DECRC",
        _ => "?",
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let target_pane: Option<u32> = args.get(1).and_then(|s| s.parse().ok());
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);

    let path = socket_path();
    let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
    use interprocess::local_socket::tokio::prelude::*;
    let mut stream = interprocess::local_socket::tokio::Stream::connect(name).await
        .map_err(|e| anyhow::anyhow!("Cannot connect: {e}"))?;

    send_msg(&mut stream, &ClientMessage::Hello {
        client_version: "0.1.0-vt-trace".to_string(),
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

    println!("Active space: {:?} {:?}", active.id, active.name);
    println!("Watching pane {:?} for {secs}s...", target_pane.map(PaneId).unwrap_or(PaneId(999)));
    println!("(interact with the app now)\n");

    let target_pane_id = target_pane.map(PaneId);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() { break; }
        match timeout(remaining, read_one(&mut stream)).await {
            Ok(Ok(ServerEvent::PaneOutput { pane_id, data })) => {
                let show = target_pane_id.map(|t| pane_id == t).unwrap_or(true);
                if show && !data.is_empty() {
                    let decoded = decode_escapes(&data);
                    println!("[{:?} {} bytes] {}", pane_id, data.len(), decoded);
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => { eprintln!("read error: {e}"); break; }
            Err(_) => break,
        }
    }
    println!("\nDone.");
    Ok(())
}
