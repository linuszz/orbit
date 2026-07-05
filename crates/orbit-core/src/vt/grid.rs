pub struct CellGrid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<orbit_protocol::Cell>,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub cursor_visible: bool,
    scroll_top: u16,
    scroll_bottom: u16,
    pub title: Option<String>,
}

impl CellGrid {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cells: vec![orbit_protocol::Cell::default(); cols as usize * rows as usize],
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            title: None,
        }
    }

    pub fn row(&self, n: u16) -> &[orbit_protocol::Cell] {
        let start = n as usize * self.cols as usize;
        let end = start + self.cols as usize;
        &self.cells[start..end]
    }

    pub fn scroll_top(&self) -> u16 {
        self.scroll_top
    }

    pub fn scroll_bottom(&self) -> u16 {
        self.scroll_bottom
    }

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        self.scroll_top = top.min(self.rows.saturating_sub(1));
        self.scroll_bottom = bottom.min(self.rows.saturating_sub(1));
    }
}
