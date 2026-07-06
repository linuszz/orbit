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

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDir,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    pub fn split_leaf(&mut self, target: PaneId, direction: SplitDir, new_id: PaneId) -> bool {
        match self {
            PaneNode::Leaf(id) if *id == target => {
                *self = PaneNode::Split {
                    direction,
                    first: Box::new(PaneNode::Leaf(target)),
                    second: Box::new(PaneNode::Leaf(new_id)),
                };
                true
            }
            PaneNode::Leaf(_) => false,
            PaneNode::Split { first, second, .. } => {
                first.split_leaf(target, direction, new_id)
                    || second.split_leaf(target, direction, new_id)
            }
        }
    }

    pub fn remove_leaf(&mut self, target: PaneId) -> bool {
        match self {
            PaneNode::Leaf(id) => *id != target,
            PaneNode::Split { first, second, .. } => {
                if let PaneNode::Leaf(id) = **first {
                    if id == target {
                        *self = (**second).clone();
                        return true;
                    }
                }
                if let PaneNode::Leaf(id) = **second {
                    if id == target {
                        *self = (**first).clone();
                        return true;
                    }
                }
                first.remove_leaf(target);
                second.remove_leaf(target);
                true
            }
        }
    }

    pub fn leaves(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(id) => vec![*id],
            PaneNode::Split { first, second, .. } => {
                let mut v = first.leaves();
                v.extend(second.leaves());
                v
            }
        }
    }
}

pub struct App {
    pub panes: HashMap<PaneId, PaneState>,
    pub pane_tree: PaneNode,
    pub active_pane: PaneId,
    pub pending_split: Option<(PaneId, SplitDir)>,
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
        let mut pane_tree = PaneNode::Leaf(PaneId(0));

        if let Some(s) = space {
            for pane in &s.panes {
                let mut parser =
                    VtParser::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                parser.grid.cells = pane.cell_grid.cells.clone();
                parser.grid.cursor_x = pane.cell_grid.cursor_x;
                parser.grid.cursor_y = pane.cell_grid.cursor_y;
                parser.grid.resize(cols, rows);
                panes.insert(pane.id, PaneState { parser });
            }
            if let Some(first_pane) = s.panes.first() {
                pane_tree = PaneNode::Leaf(first_pane.id);
            }
        }

        let active_pane = space
            .and_then(|s| s.panes.first())
            .map(|p| p.id)
            .unwrap_or(PaneId(0));

        Self {
            panes,
            pane_tree,
            active_pane,
            pending_split: None,
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

    pub fn cycle_focus(&mut self) {
        let leaves = self.pane_tree.leaves();
        if leaves.len() < 2 {
            return;
        }
        let idx = leaves
            .iter()
            .position(|&p| p == self.active_pane)
            .unwrap_or(0);
        self.active_pane = leaves[(idx + 1) % leaves.len()];
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

                        if let Some((target, dir)) = self.pending_split.take() {
                            self.pane_tree.split_leaf(target, dir, pane.id);
                        }
                    }
                }

                for &id in old_ids.iter() {
                    if !new_ids.contains(&id) {
                        self.panes.remove(&id);
                        self.pane_tree.remove_leaf(id);
                    }
                }

                self.active_pane = info.active_pane;
                if self.pane_tree.leaves().is_empty() {
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
