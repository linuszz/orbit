use std::io::{Read, Write};

use anyhow::{Context, Result};
use orbit_protocol::{PaneId, ServerEvent};
use portable_pty::{CommandBuilder, PtySize};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

pub async fn spawn_pty(
    pane_id: PaneId,
    shell: String,
    cwd: &str,
    cols: u16,
    rows: u16,
    event_bus: broadcast::Sender<ServerEvent>,
    mut input_rx: mpsc::Receiver<Vec<u8>>,
) -> Result<()> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open PTY pair")?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("ORBIT_PANE_ID", pane_id.0.to_string());

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn shell")?;
    drop(pair.slave);

    let writer = pair
        .master
        .take_writer()
        .context("failed to take PTY writer")?;
    let reader = pair
        .master
        .try_clone_reader()
        .context("failed to clone PTY reader")?;

    {
        let event_bus = event_bus.clone();
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        let _ = event_bus.send(ServerEvent::PaneOutput { pane_id, data });
                    }
                    Err(e) => {
                        warn!("PTY read error: {e}");
                        break;
                    }
                }
            }
            debug!("PTY reader exited for pane {:?}", pane_id);
            let _ = event_bus.send(ServerEvent::SpaceClosed(orbit_protocol::SpaceId(0)));
        });
    }

    {
        let mut writer = writer;
        tokio::task::spawn_blocking(move || {
            while let Some(data) = input_rx.blocking_recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
            debug!("PTY writer exited");
        });
    }

    std::mem::forget(pair.master);
    std::mem::forget(child);

    Ok(())
}
