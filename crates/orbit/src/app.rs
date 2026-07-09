use std::collections::{HashMap, VecDeque};

use orbit_core::VtParser;
use orbit_protocol::{Cell, FullState, PaneId, ServerEvent, SplitDir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Prefix,
    Scroll { offset: usize },
}

pub struct PaneState {
    pub parser: VtParser,
    pub scrollback: VecDeque<Vec<Cell>>,
}

const SCROLLBACK_CAP: usize = 10_000;

impl PaneState {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: VtParser::new(cols, rows),
            scrollback: VecDeque::with_capacity(SCROLLBACK_CAP),
        }
    }

    pub fn process(&mut self, data: &[u8]) {
        self.parser.process(data);
        for row in self.parser.grid.drain_scrolled_rows() {
            if self.scrollback.len() >= SCROLLBACK_CAP {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(row);
        }
    }
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

#[derive(Debug, Clone)]
pub struct Tab {
    pub name: String,
    pub pane_tree: PaneNode,
}

pub struct App {
    pub panes: HashMap<PaneId, PaneState>,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub active_pane: PaneId,
    pub pending_split: Option<(PaneId, SplitDir)>,
    pub pending_new_tab: bool,
    pub tab_counter: usize,
    pub mode: InputMode,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub server_connected: bool,
    pub sidebar_visible: bool,
    pub agent_panel_visible: bool,
    pub show_help: bool,
    pub space_name: String,
}

impl App {
    pub fn from_welcome(state: &FullState, cols: u16, rows: u16) -> Self {
        let space = state.spaces.first();
        let mut panes = HashMap::new();

        if let Some(s) = space {
            for pane in &s.panes {
                let mut ps = PaneState::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                ps.parser.grid.cells = pane.cell_grid.cells.clone();
                ps.parser.grid.cursor_x = pane.cell_grid.cursor_x;
                ps.parser.grid.cursor_y = pane.cell_grid.cursor_y;
                ps.parser.grid.resize(cols, rows);
                panes.insert(pane.id, ps);
            }
        }

        let first_pane = space
            .and_then(|s| s.panes.first())
            .map(|p| p.id)
            .unwrap_or(PaneId(0));

        Self {
            panes,
            tabs: vec![Tab {
                name: "dev".to_string(),
                pane_tree: PaneNode::Leaf(first_pane),
            }],
            active_tab: 0,
            active_pane: first_pane,
            pending_split: None,
            pending_new_tab: false,
            tab_counter: 1,
            mode: InputMode::Normal,
            should_quit: false,
            needs_redraw: true,
            server_connected: true,
            sidebar_visible: true,
            agent_panel_visible: false,
            show_help: false,
            space_name: space
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "default".to_string()),
        }
    }

    pub fn pane_tree(&self) -> &PaneNode {
        &self.tabs[self.active_tab].pane_tree
    }

    pub fn current_tab_name(&self) -> &str {
        &self.tabs[self.active_tab].name
    }

    pub fn cycle_focus(&mut self) {
        let leaves = self.pane_tree().leaves();
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

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
            let leaves = self.pane_tree().leaves();
            if let Some(&first) = leaves.first() {
                self.active_pane = first;
            }
            self.needs_redraw = true;
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
            let leaves = self.pane_tree().leaves();
            if let Some(&first) = leaves.first() {
                self.active_pane = first;
            }
            self.needs_redraw = true;
        }
    }

    pub fn pane_in_current_tab(&self, pane_id: PaneId) -> bool {
        self.pane_tree().leaves().contains(&pane_id)
    }

    pub fn handle_server_event(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::PaneOutput { pane_id, data } => {
                if let Some(pane) = self.panes.get_mut(pane_id) {
                    pane.process(data);
                }
                if self.pane_in_current_tab(*pane_id) {
                    self.needs_redraw = true;
                }
            }
            ServerEvent::SpaceUpdated(info) => {
                let old_ids: std::collections::HashSet<PaneId> =
                    self.panes.keys().copied().collect();
                let new_ids: std::collections::HashSet<PaneId> =
                    info.panes.iter().map(|p| p.id).collect();

                for pane in &info.panes {
                    if !old_ids.contains(&pane.id) {
                        let ps =
                            PaneState::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                        self.panes.insert(pane.id, ps);

                        if self.pending_new_tab {
                            self.pending_new_tab = false;
                            self.tab_counter += 1;
                            self.tabs.push(Tab {
                                name: format!("tab{}", self.tab_counter),
                                pane_tree: PaneNode::Leaf(pane.id),
                            });
                            self.active_tab = self.tabs.len() - 1;
                            self.active_pane = pane.id;
                        } else if let Some((target, dir)) = self.pending_split.take() {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                                tab.pane_tree.split_leaf(target, dir, pane.id);
                            }
                            self.active_pane = pane.id;
                        }
                    }
                }

                for &id in old_ids.iter() {
                    if !new_ids.contains(&id) {
                        self.panes.remove(&id);
                        for tab in &mut self.tabs {
                            tab.pane_tree.remove_leaf(id);
                        }
                    }
                }

                self.tabs.retain(|t| !t.pane_tree.leaves().is_empty());
                if self.active_tab >= self.tabs.len() {
                    self.active_tab = self.tabs.len().saturating_sub(1);
                }

                if !info.panes.is_empty() {
                    self.active_pane = info.active_pane;
                }

                if self.tabs.is_empty() {
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
