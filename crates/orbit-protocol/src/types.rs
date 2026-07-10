//! Shared types carried by the IPC protocol. See `06_tech-design/03-ipc-protocol.md` §3
//! and `06_tech-design/05-vt-emulation.md` §3 for `Cell`/`CellFlags`/`TermColor` size analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Error,
    Done,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentDetail {
    pub task: Option<String>,
    pub block_msg: Option<String>,
    pub progress: Option<f32>,
    pub duration_s: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u32>,
    pub recent_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CellFlags(pub u8);

impl CellFlags {
    pub const BOLD: u8 = 0b0001;
    pub const ITALIC: u8 = 0b0010;
    pub const UNDERLINE: u8 = 0b0100;
    pub const DIM: u8 = 0b1000;

    pub fn bold(self) -> bool {
        self.0 & Self::BOLD != 0
    }
    pub fn italic(self) -> bool {
        self.0 & Self::ITALIC != 0
    }
    pub fn underline(self) -> bool {
        self.0 & Self::UNDERLINE != 0
    }
    pub fn dim(self) -> bool {
        self.0 & Self::DIM != 0
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TermColor {
    #[default]
    Default,
    Ansi(u8),
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: TermColor,
    pub bg: TermColor,
    pub flags: CellFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellGrid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub cursor_x: u16,
    pub cursor_y: u16,
}

impl CellGrid {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols as usize * rows as usize],
            cursor_x: 0,
            cursor_y: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FullState {
    pub spaces: Vec<SpaceInfo>,
    pub active_space: SpaceId,
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: SpaceId,
    pub name: String,
    pub panes: Vec<PaneInfo>,
    pub active_pane: PaneId,
    pub layout: PaneLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneLayout {
    Leaf(PaneId),
    Split {
        direction: SplitDir,
        first: Box<PaneLayout>,
        second: Box<PaneLayout>,
    },
}

impl PaneLayout {
    pub fn split_leaf(&mut self, target: PaneId, direction: SplitDir, new_id: PaneId) -> bool {
        match self {
            PaneLayout::Leaf(id) if *id == target => {
                *self = PaneLayout::Split {
                    direction,
                    first: Box::new(PaneLayout::Leaf(target)),
                    second: Box::new(PaneLayout::Leaf(new_id)),
                };
                true
            }
            PaneLayout::Leaf(_) => false,
            PaneLayout::Split { first, second, .. } => {
                first.split_leaf(target, direction, new_id)
                    || second.split_leaf(target, direction, new_id)
            }
        }
    }

    pub fn remove_leaf(&mut self, target: PaneId) -> bool {
        match self {
            PaneLayout::Leaf(id) => *id != target,
            PaneLayout::Split { first, second, .. } => {
                if let PaneLayout::Leaf(id) = **first {
                    if id == target {
                        *self = (**second).clone();
                        return true;
                    }
                }
                if let PaneLayout::Leaf(id) = **second {
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
            PaneLayout::Leaf(id) => vec![*id],
            PaneLayout::Split { first, second, .. } => {
                let mut v = first.leaves();
                v.extend(second.leaves());
                v
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: PaneId,
    pub title: String,
    pub cwd: String,
    pub cell_grid: CellGrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    pub space_id: SpaceId,
    pub pane_id: Option<PaneId>,
    pub model: String,
    pub status: AgentStatus,
    pub detail: Option<AgentDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLaunchRequest {
    pub name: String,
    pub model: String,
    pub cwd: String,
    pub space_id: SpaceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbackLine {
    pub cells: Vec<Cell>,
    pub width: u16,
    pub seq: u64,
}
