use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Context;
use orbit_protocol::{
    CellGrid, FullState, PaneId, PaneInfo, PaneLayout, ServerEvent, SpaceId, SpaceInfo, SplitDir,
    TabId, TabInfo,
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

pub struct TabState {
    pub name: String,
    pub layout: PaneLayout,
    pub active_pane: PaneId,
}

pub struct SessionState {
    pub space_id: SpaceId,
    pub space_name: String,
    pub panes: RwLock<HashMap<PaneId, PaneEntry>>,
    pub tabs: RwLock<HashMap<TabId, TabState>>,
    pub tab_order: RwLock<Vec<TabId>>,
    pub active_tab: RwLock<TabId>,
    pub next_pane_id: AtomicU32,
    pub next_tab_id: AtomicU32,
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
        let tab_id = TabId(0);
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

        let mut tabs = HashMap::new();
        tabs.insert(
            tab_id,
            TabState {
                name: "dev".to_string(),
                layout: PaneLayout::Leaf(pane_id),
                active_pane: pane_id,
            },
        );

        Ok(Self {
            space_id: SpaceId(0),
            space_name: "default".to_string(),
            panes: RwLock::new(panes),
            tabs: RwLock::new(tabs),
            tab_order: RwLock::new(vec![tab_id]),
            active_tab: RwLock::new(tab_id),
            next_pane_id: AtomicU32::new(1),
            next_tab_id: AtomicU32::new(1),
            event_bus,
            shell,
            cwd,
        })
    }

    pub async fn split_pane(&self, tab_id: TabId, direction: SplitDir) -> anyhow::Result<PaneId> {
        let new_id = PaneId(self.next_pane_id.fetch_add(1, Ordering::Relaxed));
        let active = {
            let tabs = self.tabs.read().await;
            let tab = tabs
                .get(&tab_id)
                .ok_or_else(|| anyhow::anyhow!("tab not found"))?;
            tab.active_pane
        };
        let (cols, rows) = self.active_pane_size(&tab_id).await;

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
            let mut tabs = self.tabs.write().await;
            if let Some(tab) = tabs.get_mut(&tab_id) {
                tab.layout.split_leaf(active, direction, new_id);
                tab.active_pane = new_id;
            }
        }

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
        Ok(new_id)
    }

    pub async fn close_pane(&self, tab_id: TabId, pane_id: PaneId) {
        {
            let mut panes = self.panes.write().await;
            if let Some(entry) = panes.remove(&pane_id) {
                if let Ok(mut child) = entry.child.lock() {
                    let _ = child.kill();
                }
            }
        }

        let mut removed_tab = false;
        {
            let mut tabs = self.tabs.write().await;
            if let Some(tab) = tabs.get_mut(&tab_id) {
                tab.layout.remove_leaf(pane_id);
                let leaves = tab.layout.leaves();
                tab.active_pane = leaves.first().copied().unwrap_or(tab.active_pane);
                if leaves.is_empty() {
                    tabs.remove(&tab_id);
                    removed_tab = true;
                }
            }
        }

        if removed_tab {
            let mut order = self.tab_order.write().await;
            order.retain(|&id| id != tab_id);
            let mut active = self.active_tab.write().await;
            if *active == tab_id {
                *active = order.first().copied().unwrap_or(TabId(0));
            }
        }

        let total_panes: usize = {
            let tabs = self.tabs.read().await;
            tabs.values().map(|t| t.layout.leaves().len()).sum()
        };
        if total_panes == 0 {
            let _ = self.event_bus.send(ServerEvent::SpaceClosed(self.space_id));
            return;
        }

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
    }

    pub async fn new_tab(&self, name: Option<String>) -> anyhow::Result<TabId> {
        let new_id = TabId(self.next_tab_id.fetch_add(1, Ordering::Relaxed));
        let name = name.unwrap_or_else(|| format!("tab{}", new_id.0));
        let pane_id = PaneId(self.next_pane_id.fetch_add(1, Ordering::Relaxed));

        let (cols, rows) = {
            let active_tab_id = *self.active_tab.read().await;
            self.active_pane_size(&active_tab_id).await
        };

        let handles = pty::spawn_pty(
            pane_id,
            &self.shell,
            &self.cwd,
            cols,
            rows,
            self.event_bus.clone(),
        )
        .await
        .context("failed to spawn PTY for new tab")?;

        {
            let mut panes = self.panes.write().await;
            panes.insert(
                pane_id,
                PaneEntry {
                    input_tx: handles.input_tx,
                    vt_parser: handles.parser,
                    master: handles.master,
                    child: handles.child,
                },
            );
        }

        {
            let mut tabs = self.tabs.write().await;
            tabs.insert(
                new_id,
                TabState {
                    name,
                    layout: PaneLayout::Leaf(pane_id),
                    active_pane: pane_id,
                },
            );
        }
        {
            let mut order = self.tab_order.write().await;
            order.push(new_id);
        }
        {
            *self.active_tab.write().await = new_id;
        }

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
        Ok(new_id)
    }

    pub async fn close_tab(&self, tab_id: TabId) {
        {
            let mut tabs = self.tabs.write().await;
            if let Some(tab) = tabs.remove(&tab_id) {
                let mut panes = self.panes.write().await;
                for leaf in tab.layout.leaves() {
                    if let Some(entry) = panes.remove(&leaf) {
                        if let Ok(mut child) = entry.child.lock() {
                            let _ = child.kill();
                        }
                    }
                }
            }
        }
        {
            let mut order = self.tab_order.write().await;
            order.retain(|&id| id != tab_id);
        }
        {
            let mut active = self.active_tab.write().await;
            if *active == tab_id {
                *active = self
                    .tab_order
                    .read()
                    .await
                    .first()
                    .copied()
                    .unwrap_or(TabId(0));
            }
        }

        let total_panes: usize = {
            let tabs = self.tabs.read().await;
            tabs.values().map(|t| t.layout.leaves().len()).sum()
        };
        if total_panes == 0 {
            let _ = self.event_bus.send(ServerEvent::SpaceClosed(self.space_id));
            return;
        }

        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
    }

    pub async fn switch_tab(&self, tab_id: TabId) {
        {
            let mut active = self.active_tab.write().await;
            *active = tab_id;
        }
        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
    }

    pub async fn switch_space(&self, _space_id: orbit_protocol::SpaceId) {
        // Multi-space switching: future implementation.
        // For now the daemon runs a single space; this message is accepted but ignored.
    }

    pub async fn send_input(&self, _tab_id: TabId, pane_id: PaneId, data: Vec<u8>) {
        let panes = self.panes.read().await;
        if let Some(entry) = panes.get(&pane_id) {
            let _ = entry.input_tx.send(data).await;
        }
    }

    pub async fn resize_pane(&self, _tab_id: TabId, pane_id: PaneId, cols: u16, rows: u16) {
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

    pub async fn focus_pane(&self, tab_id: TabId, pane_id: PaneId) {
        {
            let mut tabs = self.tabs.write().await;
            if let Some(tab) = tabs.get_mut(&tab_id) {
                tab.active_pane = pane_id;
            }
        }
        {
            *self.active_tab.write().await = tab_id;
        }
        let _ = self
            .event_bus
            .send(ServerEvent::SpaceUpdated(self.collect_space_info().await));
    }

    pub async fn collect_full_state(&self) -> FullState {
        FullState {
            spaces: vec![self.collect_space_info().await],
            active_space: self.space_id,
            agents: vec![],
        }
    }

    async fn active_pane_size(&self, tab_id: &TabId) -> (u16, u16) {
        let tabs = self.tabs.read().await;
        if let Some(tab) = tabs.get(tab_id) {
            if let Some(entry) = self.panes.read().await.get(&tab.active_pane) {
                if let Ok(g) = entry.vt_parser.lock() {
                    return (g.grid.cols, g.grid.rows);
                }
            }
        }
        (80, 24)
    }

    async fn collect_space_info(&self) -> SpaceInfo {
        let tabs = self.tabs.read().await;
        let tab_order = self.tab_order.read().await;
        let active_tab = *self.active_tab.read().await;
        let panes = self.panes.read().await;

        let mut all_pane_ids = Vec::new();
        let mut tab_infos = Vec::new();
        for tab_id in tab_order.iter() {
            if let Some(tab) = tabs.get(tab_id) {
                all_pane_ids.push((*tab_id, tab.layout.leaves()));
                tab_infos.push(TabInfo {
                    id: *tab_id,
                    name: tab.name.clone(),
                    layout: tab.layout.clone(),
                    active_pane: tab.active_pane,
                });
            }
        }

        let mut pane_infos: Vec<PaneInfo> = Vec::new();
        for (tab_id, leaves) in &all_pane_ids {
            for &pid in leaves {
                if let Some(entry) = panes.get(&pid) {
                    let g = entry.vt_parser.lock().unwrap();
                    let grid = &g.grid;
                    pane_infos.push(PaneInfo {
                        id: pid,
                        tab_id: *tab_id,
                        title: "shell".to_string(),
                        cwd: self.cwd.clone(),
                        cell_grid: CellGrid {
                            cols: grid.cols,
                            rows: grid.rows,
                            cells: grid.cells.clone(),
                            cursor_x: grid.cursor_x,
                            cursor_y: grid.cursor_y,
                        },
                    });
                }
            }
        }

        SpaceInfo {
            id: self.space_id,
            name: self.space_name.clone(),
            path: self.cwd.clone(),
            tabs: tab_infos,
            active_tab,
            panes: pane_infos,
        }
    }
}
