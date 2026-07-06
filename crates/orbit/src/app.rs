use orbit_core::VtParser;
use orbit_protocol::{CellGrid, PaneId, ServerEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Prefix,
}

pub struct App {
    pub parser: VtParser,
    pub mode: InputMode,
    pub pane_id: PaneId,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub server_connected: bool,
    pub sidebar_visible: bool,
    pub agent_panel_visible: bool,
    pub space_name: String,
}

impl App {
    pub fn new(cols: u16, rows: u16, pane_id: PaneId) -> Self {
        Self {
            parser: VtParser::new(cols, rows),
            mode: InputMode::Normal,
            pane_id,
            should_quit: false,
            needs_redraw: true,
            server_connected: true,
            sidebar_visible: true,
            agent_panel_visible: false,
            space_name: "default".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn apply_snapshot(&mut self, grid: &CellGrid) {
        self.parser.grid.cols = grid.cols;
        self.parser.grid.rows = grid.rows;
        self.parser.grid.cells = grid.cells.clone();
        self.parser.grid.cursor_x = grid.cursor_x;
        self.parser.grid.cursor_y = grid.cursor_y;
        self.needs_redraw = true;
    }

    pub fn handle_server_event(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::PaneOutput { data, .. } => {
                self.parser.process(data);
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
