use orbit_protocol::{CellGrid, FullState, PaneId, PaneInfo, ServerEvent, SpaceId, SpaceInfo};
use portable_pty::PtySize;
use tokio::sync::{broadcast, mpsc};

use crate::pty::{SharedMaster, SharedVtParser};

pub struct SessionState {
    pub space_id: SpaceId,
    pub pane_id: PaneId,
    pub pty_input_tx: mpsc::Sender<Vec<u8>>,
    pub event_bus: broadcast::Sender<ServerEvent>,
    pub vt_parser: SharedVtParser,
    pub master: SharedMaster,
}

impl SessionState {
    pub fn new(
        pty_input_tx: mpsc::Sender<Vec<u8>>,
        event_bus: broadcast::Sender<ServerEvent>,
        vt_parser: SharedVtParser,
        master: SharedMaster,
    ) -> Self {
        Self {
            space_id: SpaceId(0),
            pane_id: PaneId(0),
            pty_input_tx,
            event_bus,
            vt_parser,
            master,
        }
    }

    pub fn collect_full_state(&self) -> FullState {
        let (cols, rows, cells, cursor_x, cursor_y) = {
            let parser = self.vt_parser.lock().unwrap();
            let g = &parser.grid;
            (g.cols, g.rows, g.cells.clone(), g.cursor_x, g.cursor_y)
        };
        let pane = PaneInfo {
            id: self.pane_id,
            title: "shell".to_string(),
            cwd: ".".to_string(),
            cell_grid: CellGrid {
                cols,
                rows,
                cells,
                cursor_x,
                cursor_y,
            },
        };
        FullState {
            spaces: vec![SpaceInfo {
                id: self.space_id,
                name: "default".to_string(),
                panes: vec![pane],
                active_pane: self.pane_id,
            }],
            active_space: self.space_id,
            agents: vec![],
        }
    }

    pub fn resize_pty(&self, cols: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        if let Ok(mut parser) = self.vt_parser.lock() {
            parser.grid.resize(cols, rows);
        }
    }
}
