use orbit_protocol::{
    Cell, CellGrid, FullState, PaneId, PaneInfo, ServerEvent, SpaceId, SpaceInfo,
};
use tokio::sync::{broadcast, mpsc};

pub struct SessionState {
    pub space_id: SpaceId,
    pub pane_id: PaneId,
    pub pty_input_tx: mpsc::Sender<Vec<u8>>,
    pub event_bus: broadcast::Sender<ServerEvent>,
    pub cols: u16,
    pub rows: u16,
}

impl SessionState {
    pub fn new(
        pty_input_tx: mpsc::Sender<Vec<u8>>,
        event_bus: broadcast::Sender<ServerEvent>,
        cols: u16,
        rows: u16,
    ) -> Self {
        Self {
            space_id: SpaceId(0),
            pane_id: PaneId(0),
            pty_input_tx,
            event_bus,
            cols,
            rows,
        }
    }

    pub fn collect_full_state(&self) -> FullState {
        let pane = PaneInfo {
            id: self.pane_id,
            title: "bash".to_string(),
            cwd: ".".to_string(),
            cell_grid: CellGrid {
                cols: self.cols,
                rows: self.rows,
                cells: vec![Cell::default(); self.cols as usize * self.rows as usize],
                cursor_x: 0,
                cursor_y: 0,
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
}
