use std::collections::{HashMap, VecDeque};

use orbit_core::VtParser;
use orbit_protocol::{
    Cell, CellGrid, FullState, PaneId, PaneLayout, ServerEvent, SpaceId, SplitDir, TabId,
};

// Fields consumed by Task 4 (sidebar rendering); suppressing dead_code until then.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SpaceEntry {
    pub space_id: SpaceId,
    pub name: String,
    pub cwd: String,
    pub tab_count: usize,
    pub pane_count: usize,
}

// Fields consumed by Tasks 4 and 5 (tab bar, mouse selection); suppressing dead_code until then.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Selection {
    pub pane_id: PaneId,
    pub start: (u16, u16), // (col, row) in cell coords within pane
    pub end: (u16, u16),
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    CommandPalette {
        search: String,
        selected: usize,
        search_focused: bool,
    },
    Scroll {
        offset: usize,
    },
}

pub struct CommandDef {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub shortcut: &'static str,
}

pub static COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: "split_h",
        label: "Split Horizontal",
        group: "Pane",
        shortcut: "h",
    },
    CommandDef {
        id: "split_v",
        label: "Split Vertical",
        group: "Pane",
        shortcut: "v",
    },
    CommandDef {
        id: "close_pane",
        label: "Close Pane",
        group: "Pane",
        shortcut: "x",
    },
    CommandDef {
        id: "scroll_mode",
        label: "Enter Scroll Mode",
        group: "Pane",
        shortcut: "[",
    },
    CommandDef {
        id: "new_tab",
        label: "New Tab",
        group: "Tab",
        shortcut: "c",
    },
    CommandDef {
        id: "next_tab",
        label: "Next Tab",
        group: "Tab",
        shortcut: "n",
    },
    CommandDef {
        id: "prev_tab",
        label: "Previous Tab",
        group: "Tab",
        shortcut: "p",
    },
    CommandDef {
        id: "toggle_sidebar",
        label: "Toggle Sidebar",
        group: "View",
        shortcut: "b",
    },
    CommandDef {
        id: "toggle_agent",
        label: "Toggle Agent Monitor",
        group: "View",
        shortcut: "a",
    },
    CommandDef {
        id: "detach",
        label: "Detach Session",
        group: "Session",
        shortcut: "d",
    },
    CommandDef {
        id: "help",
        label: "Show Help",
        group: "Help",
        shortcut: "?",
    },
];

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

    pub fn sync_from_server(&mut self, grid: &CellGrid) {
        self.parser.grid.cells = grid.cells.clone();
        self.parser.grid.cursor_x = grid.cursor_x;
        self.parser.grid.cursor_y = grid.cursor_y;
        self.parser.grid.resize(grid.cols, grid.rows);
        self.parser.reset_parser();
    }
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: TabId,
    pub name: String,
    pub pane_tree: PaneLayout,
}

pub struct App {
    pub panes: HashMap<PaneId, PaneState>,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub active_tab_id: TabId,
    pub active_pane: PaneId,
    pub pending_split: Option<(PaneId, SplitDir)>,
    pub mode: InputMode,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub server_connected: bool,
    pub sidebar_visible: bool,
    pub agent_panel_visible: bool,
    pub show_help: bool,
    pub context_menu: Option<ContextMenu>,
    pub space_name: String,
    pub space_path: String,
    pub spaces: Vec<SpaceEntry>,
    pub active_space_idx: usize,
    pub tab_hovered: Option<usize>,
    pub sidebar_hovered: Option<usize>,
    pub selection: Option<Selection>,
}

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Pane(PaneId),
    Space,
    Sidebar,
    Tab(TabId),
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub x: u16,
    pub y: u16,
    pub target: ContextMenuTarget,
    pub items: Vec<ContextMenuItem>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum ContextMenuItem {
    Action {
        label: String,
        shortcut: String,
        id: &'static str,
    },
    Separator,
}

impl App {
    pub fn from_welcome(state: &FullState, cols: u16, rows: u16) -> Self {
        let spaces: Vec<SpaceEntry> = state
            .spaces
            .iter()
            .map(|s| SpaceEntry {
                space_id: s.id,
                name: s.name.clone(),
                cwd: s.path.clone(),
                tab_count: s.tabs.len(),
                pane_count: s.panes.len(),
            })
            .collect();

        let active_space_idx = state
            .spaces
            .iter()
            .position(|s| s.id == state.active_space)
            .unwrap_or(0);

        let space = state.spaces.first();
        let mut panes = HashMap::new();
        let mut tabs = Vec::new();
        let mut active_tab_idx = 0;
        let mut active_tab_id = TabId(0);
        let mut active_pane = PaneId(0);

        if let Some(s) = space {
            for pane in &s.panes {
                let mut ps = PaneState::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                ps.parser.grid.cells = pane.cell_grid.cells.clone();
                ps.parser.grid.cursor_x = pane.cell_grid.cursor_x;
                ps.parser.grid.cursor_y = pane.cell_grid.cursor_y;
                ps.parser.grid.resize(cols, rows);
                panes.insert(pane.id, ps);
            }

            for (i, tab_info) in s.tabs.iter().enumerate() {
                tabs.push(Tab {
                    id: tab_info.id,
                    name: tab_info.name.clone(),
                    pane_tree: tab_info.layout.clone(),
                });
                if tab_info.id == s.active_tab {
                    active_tab_idx = i;
                    active_tab_id = tab_info.id;
                    active_pane = tab_info.active_pane;
                }
            }
        }

        let first_pane = space
            .and_then(|s| s.panes.first())
            .map(|p| p.id)
            .unwrap_or(PaneId(0));

        if tabs.is_empty() {
            let pane_tree = space
                .and_then(|s| s.tabs.first().map(|t| t.layout.clone()))
                .unwrap_or(PaneLayout::Leaf(first_pane));
            tabs.push(Tab {
                id: TabId(0),
                name: "dev".to_string(),
                pane_tree,
            });
        }

        Self {
            panes,
            tabs,
            active_tab: active_tab_idx,
            active_tab_id,
            active_pane,
            pending_split: None,
            mode: InputMode::Normal,
            should_quit: false,
            needs_redraw: true,
            server_connected: true,
            sidebar_visible: true,
            agent_panel_visible: false,
            show_help: false,
            context_menu: None,
            space_name: spaces
                .get(active_space_idx)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "orbit".to_string()),
            space_path: spaces
                .get(active_space_idx)
                .map(|s| s.cwd.clone())
                .unwrap_or_else(|| ".".to_string()),
            spaces,
            active_space_idx,
            tab_hovered: None,
            sidebar_hovered: None,
            selection: None,
        }
    }

    pub fn pane_tree(&self) -> &PaneLayout {
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
            self.selection = None;
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
            self.active_tab_id = self.tabs[self.active_tab].id;
            let leaves = self.pane_tree().leaves();
            if let Some(&first) = leaves.first() {
                self.active_pane = first;
            }
            self.needs_redraw = true;
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.selection = None;
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
            self.active_tab_id = self.tabs[self.active_tab].id;
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

    pub fn open_context_menu(&mut self, x: u16, y: u16, target: ContextMenuTarget) {
        let items = match &target {
            ContextMenuTarget::Pane(pane_id) => {
                let mut items = vec![
                    ContextMenuItem::Action {
                        label: "Split Horizontal".into(),
                        shortcut: "h".into(),
                        id: "split_h",
                    },
                    ContextMenuItem::Action {
                        label: "Split Vertical".into(),
                        shortcut: "v".into(),
                        id: "split_v",
                    },
                    ContextMenuItem::Separator,
                    ContextMenuItem::Action {
                        label: "Close Pane".into(),
                        shortcut: "x".into(),
                        id: "close_pane",
                    },
                    ContextMenuItem::Action {
                        label: "Maximize Pane".into(),
                        shortcut: "z".into(),
                        id: "maximize",
                    },
                ];
                if self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.pane_id == *pane_id)
                {
                    items.insert(
                        0,
                        ContextMenuItem::Action {
                            label: "Copy Selection".into(),
                            shortcut: String::new(),
                            id: "copy_selection",
                        },
                    );
                    items.insert(1, ContextMenuItem::Separator);
                }
                items
            }
            ContextMenuTarget::Space => vec![
                ContextMenuItem::Action {
                    label: "Rename".into(),
                    shortcut: "r".into(),
                    id: "rename_space",
                },
                ContextMenuItem::Separator,
                ContextMenuItem::Action {
                    label: "Move Up".into(),
                    shortcut: "".into(),
                    id: "move_up",
                },
                ContextMenuItem::Action {
                    label: "Move Down".into(),
                    shortcut: "".into(),
                    id: "move_down",
                },
                ContextMenuItem::Separator,
                ContextMenuItem::Action {
                    label: "Close".into(),
                    shortcut: "".into(),
                    id: "close_space",
                },
            ],
            ContextMenuTarget::Sidebar => vec![ContextMenuItem::Action {
                label: "New Space".into(),
                shortcut: "".into(),
                id: "new_space",
            }],
            ContextMenuTarget::Tab(_tab_id) => vec![
                ContextMenuItem::Action {
                    label: "New Tab".into(),
                    shortcut: "c".into(),
                    id: "new_tab",
                },
                ContextMenuItem::Separator,
                ContextMenuItem::Action {
                    label: "Close Tab".into(),
                    shortcut: "x".into(),
                    id: "close_tab",
                },
            ],
        };
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target,
            items,
            selected: 0,
        });
        self.needs_redraw = true;
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
        self.needs_redraw = true;
    }

    pub fn handle_server_event(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::Welcome { state, .. } => {
                if let Some(s) = state.spaces.first() {
                    for pane in &s.panes {
                        if let Some(existing) = self.panes.get_mut(&pane.id) {
                            existing.sync_from_server(&pane.cell_grid);
                        } else {
                            let ps = PaneState::new(
                                pane.cell_grid.cols.max(1),
                                pane.cell_grid.rows.max(1),
                            );
                            self.panes.insert(pane.id, ps);
                        }
                    }
                    if let Some(active_tab_info) = s.tabs.iter().find(|t| t.id == s.active_tab) {
                        self.active_pane = active_tab_info.active_pane;
                    }
                    self.space_name = s.name.clone();
                    self.space_path = s.path.clone();

                    let mut new_tabs = Vec::new();
                    let mut new_active_idx = 0;
                    let mut found_active = false;
                    for (i, tab_info) in s.tabs.iter().enumerate() {
                        new_tabs.push(Tab {
                            id: tab_info.id,
                            name: tab_info.name.clone(),
                            pane_tree: tab_info.layout.clone(),
                        });
                        if tab_info.id == s.active_tab {
                            new_active_idx = i;
                            found_active = true;
                        }
                    }
                    if !new_tabs.is_empty() {
                        self.tabs = new_tabs;
                        self.active_tab = if found_active { new_active_idx } else { 0 };
                        self.active_tab_id = s.active_tab;
                    }
                }
                self.spaces = state
                    .spaces
                    .iter()
                    .map(|s| SpaceEntry {
                        space_id: s.id,
                        name: s.name.clone(),
                        cwd: s.path.clone(),
                        tab_count: s.tabs.len(),
                        pane_count: s.panes.len(),
                    })
                    .collect();
                self.active_space_idx = state
                    .spaces
                    .iter()
                    .position(|s| s.id == state.active_space)
                    .unwrap_or(0);
                self.needs_redraw = true;
            }
            ServerEvent::PaneOutput { pane_id, data } => {
                if let Some(sel) = &self.selection {
                    if sel.pane_id == *pane_id {
                        self.selection = None;
                    }
                }
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
                    if old_ids.contains(&pane.id) {
                        if let Some(existing) = self.panes.get_mut(&pane.id) {
                            existing.sync_from_server(&pane.cell_grid);
                        }
                    } else {
                        let ps =
                            PaneState::new(pane.cell_grid.cols.max(1), pane.cell_grid.rows.max(1));
                        self.panes.insert(pane.id, ps);

                        if let Some((target, dir)) = self.pending_split.take() {
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
                    }
                }

                let mut new_tabs = Vec::new();
                let mut new_active_idx = 0;
                let mut found_active = false;
                for (i, tab_info) in info.tabs.iter().enumerate() {
                    new_tabs.push(Tab {
                        id: tab_info.id,
                        name: tab_info.name.clone(),
                        pane_tree: tab_info.layout.clone(),
                    });
                    if tab_info.id == info.active_tab {
                        new_active_idx = i;
                        found_active = true;
                    }
                }
                if !new_tabs.is_empty() {
                    self.tabs = new_tabs;
                    self.active_tab = if found_active { new_active_idx } else { 0 };
                    self.active_tab_id = info.active_tab;
                    if let Some(active_tab) = self.tabs.get(self.active_tab) {
                        let server_active = info
                            .tabs
                            .iter()
                            .find(|t| t.id == info.active_tab)
                            .map(|t| t.active_pane);
                        self.active_pane = server_active
                            .filter(|&pid| self.panes.contains_key(&pid))
                            .or_else(|| active_tab.pane_tree.leaves().first().copied())
                            .unwrap_or(self.active_pane);
                    }
                }

                if self.tabs.is_empty() {
                    self.should_quit = true;
                }
                self.needs_redraw = true;
            }
            ServerEvent::SpaceCreated(info) => {
                self.spaces.push(SpaceEntry {
                    space_id: info.id,
                    name: info.name.clone(),
                    cwd: info.path.clone(),
                    tab_count: info.tabs.len(),
                    pane_count: info.panes.len(),
                });
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
