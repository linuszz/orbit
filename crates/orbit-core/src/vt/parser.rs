//! VT parser — wraps `vte::Parser` and applies escape sequences to a `CellGrid`.
//!
//! SKELETON ONLY: the actual `Perform` implementation lives in
//! `06_tech-design/05-vt-emulation.md` §2-§3. The integration requires a
//! split-struct pattern (`VtParser` owns the `Parser`; a separate `Performer`
//! owns the `CellGrid` + SGR state) to satisfy the borrow checker — `vte`'s
//! `Parser::advance(&mut self, &mut impl Perform)` cannot borrow `self`
//! twice. That pattern will be implemented in Phase 1 week 1 alongside the
//! VT golden-file tests (v5 GAP 2).

use crate::vt::grid::CellGrid;

pub struct VtParser {
    pub grid: CellGrid,
    parser: vte::Parser,
}

impl VtParser {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            grid: CellGrid::new(cols, rows),
            parser: vte::Parser::new(),
        }
    }

    /// Push raw PTY bytes through the VT state machine.
    ///
    /// TODO(phase1): implement via the split-struct `Performer` pattern.
    /// For now this is a no-op so the skeleton compiles; the grid is
    /// populated only via `ServerEvent::PaneSnapshot` in Phase 1 Vertical
    /// Slice 0 step 5.
    pub fn process(&mut self, _bytes: &[u8]) {
        // Real implementation will:
        //   let mut performer = Performer::new(&mut self.grid);
        //   for byte in bytes { self.parser.advance(&mut performer, *byte); }
    }

    pub fn reset_parser(&mut self) {
        self.parser = vte::Parser::new();
    }
}
