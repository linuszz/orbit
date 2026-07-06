use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use orbit_core::VtParser;
use orbit_protocol::{PaneId, ServerEvent};
use portable_pty::{CommandBuilder, PtySize};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

pub type SharedVtParser = Arc<Mutex<VtParser>>;
pub type SharedMaster = Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>;
pub type SharedChild = Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>;

pub struct PtyHandles {
    pub parser: SharedVtParser,
    pub master: SharedMaster,
    pub child: SharedChild,
    pub input_tx: mpsc::Sender<Vec<u8>>,
}

pub async fn spawn_pty(
    pane_id: PaneId,
    shell: &str,
    cwd: &str,
    cols: u16,
    rows: u16,
    event_bus: broadcast::Sender<ServerEvent>,
) -> Result<PtyHandles> {
    let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>(64);

    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open PTY pair")?;

    let mut cmd = CommandBuilder::new(shell);
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

    let shared_parser: SharedVtParser = Arc::new(Mutex::new(VtParser::new(cols, rows)));
    let shared_master: SharedMaster = Arc::new(Mutex::new(pair.master));
    let shared_child: SharedChild = Arc::new(Mutex::new(child));

    {
        let event_bus = event_bus.clone();
        let shared_parser = shared_parser.clone();
        let reader_tx = input_tx.clone();
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        let da1_queried = if let Ok(mut parser) = shared_parser.lock() {
                            parser.process(&data);
                            let _ = parser.grid.drain_scrolled_rows();
                            let q = parser.grid.da1_queried;
                            parser.grid.da1_queried = false;
                            q
                        } else {
                            false
                        };
                        if da1_queried {
                            let _ = reader_tx.blocking_send(b"\x1b[?62;4c".to_vec());
                        }
                        let _ = event_bus.send(ServerEvent::PaneOutput { pane_id, data });
                    }
                    Err(e) => {
                        warn!("PTY read error: {e}");
                        break;
                    }
                }
            }
            debug!("PTY reader exited for pane {:?}", pane_id);
        });
    }

    {
        let mut writer = writer;
        let mut input_rx = input_rx;
        tokio::task::spawn_blocking(move || {
            while let Some(data) = input_rx.blocking_recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
            debug!("PTY writer exited for pane {:?}", pane_id);
        });
    }

    Ok(PtyHandles {
        parser: shared_parser,
        master: shared_master,
        child: shared_child,
        input_tx,
    })
}
