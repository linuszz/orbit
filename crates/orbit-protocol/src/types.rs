//! Shared types carried by the IPC protocol. See `06_tech-design/03-ipc-protocol.md` §3
//! and `06_tech-design/05-vt-emulation.md` §3 for `Cell`/`CellFlags`/`TermColor` size analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u32);

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
    pub path: String,
    pub tabs: Vec<TabInfo>,
    pub active_tab: TabId,
    pub panes: Vec<PaneInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneLayout {
    Leaf(PaneId),
    Split {
        direction: SplitDir,
        first: Box<PaneLayout>,
        second: Box<PaneLayout>,
        #[serde(default = "default_ratio")]
        ratio: f32,
    },
}

fn default_ratio() -> f32 {
    0.5
}

impl PaneLayout {
    pub fn split_leaf(&mut self, target: PaneId, direction: SplitDir, new_id: PaneId) -> bool {
        match self {
            PaneLayout::Leaf(id) if *id == target => {
                *self = PaneLayout::Split {
                    direction,
                    first: Box::new(PaneLayout::Leaf(target)),
                    second: Box::new(PaneLayout::Leaf(new_id)),
                    ratio: 0.5,
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

    pub fn set_split_ratio(&mut self, first_pane: PaneId, second_pane: PaneId, ratio: f32) -> bool {
        let ratio = if ratio.is_finite() {
            ratio.clamp(0.1, 0.9)
        } else {
            0.5
        };
        match self {
            PaneLayout::Leaf(_) => false,
            PaneLayout::Split {
                first,
                second,
                ratio: r,
                ..
            } => {
                let first_leaf = first.leaves().first().copied();
                let second_leaf = second.leaves().first().copied();
                if first_leaf == Some(first_pane) && second_leaf == Some(second_pane) {
                    *r = ratio;
                    return true;
                }
                first.set_split_ratio(first_pane, second_pane, ratio)
                    || second.set_split_ratio(first_pane, second_pane, ratio)
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

    pub fn find_pane_in_direction(
        &self,
        current: PaneId,
        split_dir: SplitDir,
        positive: bool,
    ) -> Option<PaneId> {
        match self {
            PaneLayout::Leaf(_) => None,
            PaneLayout::Split {
                direction,
                first,
                second,
                ..
            } => {
                let in_first = first.leaves().contains(&current);
                let in_second = second.leaves().contains(&current);

                if *direction == split_dir && ((positive && in_first) || (!positive && in_second)) {
                    let target = if positive { second } else { first };
                    target.leaves().first().copied()
                } else {
                    first
                        .find_pane_in_direction(current, split_dir, positive)
                        .or_else(|| second.find_pane_in_direction(current, split_dir, positive))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: PaneId,
    pub tab_id: TabId,
    pub title: String,
    pub cwd: String,
    pub cell_grid: CellGrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: TabId,
    pub name: String,
    pub layout: PaneLayout,
    pub active_pane: PaneId,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_split_ratio_simple_split() {
        let mut layout = PaneLayout::Split {
            direction: SplitDir::Horizontal,
            first: Box::new(PaneLayout::Leaf(PaneId(1))),
            second: Box::new(PaneLayout::Leaf(PaneId(2))),
            ratio: 0.5,
        };
        assert!(layout.set_split_ratio(PaneId(1), PaneId(2), 0.3));
        match layout {
            PaneLayout::Split { ratio, .. } => assert!((ratio - 0.3).abs() < f32::EPSILON),
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn set_split_ratio_clamps_and_rejects_nan() {
        let mut layout = PaneLayout::Split {
            direction: SplitDir::Horizontal,
            first: Box::new(PaneLayout::Leaf(PaneId(1))),
            second: Box::new(PaneLayout::Leaf(PaneId(2))),
            ratio: 0.5,
        };
        assert!(layout.set_split_ratio(PaneId(1), PaneId(2), f32::NAN));
        match layout {
            PaneLayout::Split { ratio, .. } => {
                assert!(ratio.is_finite());
                assert!((0.1..=0.9).contains(&ratio));
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn set_split_ratio_leaf_no_op() {
        let mut layout = PaneLayout::Leaf(PaneId(1));
        assert!(!layout.set_split_ratio(PaneId(1), PaneId(2), 0.3));
    }

    #[test]
    fn set_split_ratio_nested_inner_split() {
        let mut layout = PaneLayout::Split {
            direction: SplitDir::Horizontal,
            first: Box::new(PaneLayout::Split {
                direction: SplitDir::Horizontal,
                first: Box::new(PaneLayout::Leaf(PaneId(1))),
                second: Box::new(PaneLayout::Leaf(PaneId(2))),
                ratio: 0.5,
            }),
            second: Box::new(PaneLayout::Leaf(PaneId(3))),
            ratio: 0.5,
        };
        assert!(layout.set_split_ratio(PaneId(1), PaneId(2), 0.7));
        match layout {
            PaneLayout::Split {
                first: inner,
                second,
                ratio: outer_ratio,
                ..
            } => {
                assert!((outer_ratio - 0.5).abs() < f32::EPSILON);
                assert_eq!(second.leaves(), vec![PaneId(3)]);
                match *inner {
                    PaneLayout::Split { ratio, .. } => {
                        assert!((ratio - 0.7).abs() < f32::EPSILON)
                    }
                    _ => panic!("expected inner split"),
                }
            }
            _ => panic!("expected outer split"),
        }
    }

    #[test]
    fn set_split_ratio_nested_outer_split() {
        let mut layout = PaneLayout::Split {
            direction: SplitDir::Horizontal,
            first: Box::new(PaneLayout::Split {
                direction: SplitDir::Horizontal,
                first: Box::new(PaneLayout::Leaf(PaneId(1))),
                second: Box::new(PaneLayout::Leaf(PaneId(2))),
                ratio: 0.5,
            }),
            second: Box::new(PaneLayout::Leaf(PaneId(3))),
            ratio: 0.5,
        };
        assert!(layout.set_split_ratio(PaneId(1), PaneId(3), 0.2));
        match layout {
            PaneLayout::Split { ratio, .. } => {
                assert!((ratio - 0.2).abs() < f32::EPSILON)
            }
            _ => panic!("expected outer split"),
        }
    }
}
