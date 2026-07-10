use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use orbit_protocol::{
    CellGrid, FullState, PaneId, PaneInfo, PaneLayout, ServerEvent, SpaceId, SpaceInfo, SplitDir,
};
use portable_pty::PtySize;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::pty::{self, SharedChild, SharedMaster, SharedVtParser};

pub struct PaneEntry {
    pub input_tx: mpsc::Sender<Vec<u8>>,
    pub vt_parser: SharedVtParser,
    pub master: SharedMaster,
    pub child: SharedChild,
}

pub struct SessionState {
    pub space_id: SpaceId,
    pub space_name: String,
    pub panes: RwLock<HashMap<PaneId, PaneEntry>>,
    pub pane_tree: RwLock<PaneLayout>,
    pub active_pane: RwLock<PaneId>,
    pub next_pane_id: AtomicU32,
    pub event_bus: broadcast::Sender<ServerEvent>,
    pub shell: String,
    pub cwd: String,
}

impl SessionState {
    pub async fn new(
        event_bus: broadcast::Sender<ServerEvent>,
        shell: String,
        cwd: String,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<Self> {
        let pane_id = PaneId(0);
        let handles = pty::spawn_pty(pane_id, &shell, &cwd, cols, rows, event_bus.clone()).await?;

        let mut panes = HashMap::new();
        panes.insert(
            pane_id,
            PaneEntry {
                input_tx: handles.input_tx,
                vt_parser: handles.parser,
                master: handles.master,
                child: handles.child,
            },
        );

        Ok(Self {
            space_id: SpaceId(0),
            space_name: "default".to_string(),
            panes: RwLock::new(panes),
            pane_tree: RwLock::new(PaneLayout::Leaf(pane_id)),
            active_pane: RwLock::new(pane_id),
            next_pane_id: AtomicU32::new(1),
            event_bus,
            shell,
            cwd,
        })
    }

    pub async fn split_pane(&self, direction: SplitDir) -> anyhow::Result<PaneId> {
        let new_id = PaneId(self.next_pane_id.fetch_add(1, Ordering::Relaxed));
        let active = *self.active_pane.read().await;
        let (cols, rows) = self.active_pane_size().await;

        let handles = pty::spawn_pty(
            new_id,
            &self.shell,
            &self.cwd,
            cols,
            rows,
            self.event_bus.clone(),
        )
        .await?;

        {
            let mut panes = self.panes.write().await;
            panes.insert(
                new_id,
                PaneEntry {
                    input_tx: handles.input_tx,
                    vt_parser: handles.parser,
                    master: handles.master,
                    child: handles.child,
                },
            );
        }

        {
            let mut tree = self.pane_tree.write().await;
            tree.split_leaf(active, direction, new_id);
        }

        *self.active_pane.write().await = new_id;

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
        Ok(new_id)
    }

    pub async fn close_pane(&self, pane_id: PaneId) {
        {
            let mut panes = self.panes.write().await;
            if let Some(entry) = panes.remove(&pane_id) {
                if let Ok(mut child) = entry.child.lock() {
                    let _ = child.kill();
                }
            }
        }

        {
            let mut tree = self.pane_tree.write().await;
            tree.remove_leaf(pane_id);
        }

        let tree_empty = self.pane_tree.read().await.leaves().is_empty();
        if tree_empty {
            let _ = self.event_bus.send(ServerEvent::SpaceClosed(self.space_id));
            return;
        }

        {
            let mut active = self.active_pane.write().await;
            let leaves = self.pane_tree.read().await.leaves();
            if *active == pane_id {
                if let Some(&first) = leaves.first() {
                    *active = first;
                }
            }
        }

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
    }

    pub async fn send_input(&self, pane_id: PaneId, data: Vec<u8>) {
        let panes = self.panes.read().await;
        if let Some(entry) = panes.get(&pane_id) {
            let _ = entry.input_tx.send(data).await;
        }
    }

    pub async fn resize_pane(&self, pane_id: PaneId, cols: u16, rows: u16) {
        let panes = self.panes.read().await;
        if let Some(entry) = panes.get(&pane_id) {
            if let Ok(master) = entry.master.lock() {
                let _ = master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
            if let Ok(mut parser) = entry.vt_parser.lock() {
                parser.grid.resize(cols, rows);
            }
        }
    }

    pub async fn focus_pane(&self, pane_id: PaneId) {
        *self.active_pane.write().await = pane_id;
    }

    pub async fn collect_full_state(&self) -> FullState {
        FullState {
            spaces: vec![self.collect_space_info().await],
            active_space: self.space_id,
            agents: vec![],
        }
    }

    async fn active_pane_size(&self) -> (u16, u16) {
        let panes = self.panes.read().await;
        let active = *self.active_pane.read().await;
        if let Some(entry) = panes.get(&active) {
            if let Ok(g) = entry.vt_parser.lock() {
                return (g.grid.cols, g.grid.rows);
            }
        }
        (80, 24)
    }

    async fn collect_space_info(&self) -> SpaceInfo {
        let panes = self.panes.read().await;
        let tree = self.pane_tree.read().await;
        let active = *self.active_pane.read().await;

        let pane_infos: Vec<PaneInfo> = tree
            .leaves()
            .iter()
            .filter_map(|&pid| {
                panes.get(&pid).map(|entry| {
                    let g = entry.vt_parser.lock().unwrap();
                    let grid = &g.grid;
                    PaneInfo {
                        id: pid,
                        title: "shell".to_string(),
                        cwd: self.cwd.clone(),
                        cell_grid: CellGrid {
                            cols: grid.cols,
                            rows: grid.rows,
                            cells: grid.cells.clone(),
                            cursor_x: grid.cursor_x,
                            cursor_y: grid.cursor_y,
                        },
                    }
                })
            })
            .collect();

        SpaceInfo {
            id: self.space_id,
            name: self.space_name.clone(),
            panes: pane_infos,
            active_pane: active,
            layout: tree.clone(),
        }
    }
}
