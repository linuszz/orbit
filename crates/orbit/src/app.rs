use std::collections::HashMap;

use orbit_core::VtParser;
use orbit_protocol::{FullState, PaneId, ServerEvent, SplitDir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Prefix,
}

pub struct PaneState {
    pub parser: VtParser,
}

pub struct App {
    pub panes: HashMap<PaneId, PaneState>,
    pub pane_order: Vec<PaneId>,
    pub active_pane: PaneId,
    pub layout: SplitDir,
    pub mode: InputMode,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub server_connected: bool,
    pub sidebar_visible: bool,
    pub agent_panel_visible: bool,
    pub space_name: String,
    pub bytes_received: u64,
}

impl App {
    pub fn from_welcome(state: &FullState, cols: u16, rows: u16) -> Self {
        let space = state.spaces.first();
        let mut panes = HashMap::new();
        let mut pane_order = Vec::new();

        if let Some(s) = space {
            for pane in &s.panes {
                let mut parser =
                    VtParser::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                parser.grid.cells = pane.cell_grid.cells.clone();
                parser.grid.cursor_x = pane.cell_grid.cursor_x;
                parser.grid.cursor_y = pane.cell_grid.cursor_y;
                parser.grid.resize(cols, rows);
                panes.insert(pane.id, PaneState { parser });
                pane_order.push(pane.id);
            }
        }

        let active_pane = space
            .and_then(|s| s.panes.first())
            .map(|p| p.id)
            .unwrap_or(PaneId(0));

        Self {
            panes,
            pane_order,
            active_pane,
            layout: SplitDir::Horizontal,
            mode: InputMode::Normal,
            should_quit: false,
            needs_redraw: true,
            server_connected: true,
            sidebar_visible: true,
            agent_panel_visible: false,
            space_name: space
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "default".to_string()),
            bytes_received: 0,
        }
    }

    #[allow(dead_code)]
    pub fn active_pane_id(&self) -> PaneId {
        self.active_pane
    }

    pub fn cycle_focus(&mut self) {
        if self.pane_order.len() < 2 {
            return;
        }
        let idx = self
            .pane_order
            .iter()
            .position(|&p| p == self.active_pane)
            .unwrap_or(0);
        let next = (idx + 1) % self.pane_order.len();
        self.active_pane = self.pane_order[next];
        self.needs_redraw = true;
    }

    pub fn handle_server_event(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::PaneOutput { pane_id, data } => {
                self.bytes_received += data.len() as u64;
                if let Some(pane) = self.panes.get_mut(pane_id) {
                    pane.parser.process(data);
                }
                self.needs_redraw = true;
            }
            ServerEvent::SpaceUpdated(info) => {
                let old_ids: std::collections::HashSet<PaneId> =
                    self.panes.keys().copied().collect();
                let new_ids: std::collections::HashSet<PaneId> =
                    info.panes.iter().map(|p| p.id).collect();

                for pane in &info.panes {
                    if !old_ids.contains(&pane.id) {
                        let parser =
                            VtParser::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                        self.panes.insert(pane.id, PaneState { parser });
                    }
                }

                for &id in old_ids.iter() {
                    if !new_ids.contains(&id) {
                        self.panes.remove(&id);
                    }
                }

                self.pane_order = info.panes.iter().map(|p| p.id).collect();
                self.active_pane = info.active_pane;
                if self.pane_order.is_empty() {
                    self.should_quit = true;
                }
                self.needs_redraw = true;
            }
            ServerEvent::SpaceClosed(_) => {
                self.should_quit = true;
                self.needs_redraw = true;
            }
            _ => {}
        }
    }
}
