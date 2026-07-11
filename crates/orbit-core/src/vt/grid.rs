use orbit_protocol::{Cell, CellFlags, TermColor};

pub struct CellGrid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub cursor_visible: bool,
    pub scroll_top: u16,
    pub scroll_bottom: u16,
    pub title: Option<String>,

    pub da1_queried: bool,

    pub scrolled_off_rows: Vec<Vec<Cell>>,

    current_fg: TermColor,
    current_bg: TermColor,
    current_bold: bool,
    current_italic: bool,
    current_underline: bool,
    current_dim: bool,

    saved_cells: Option<Vec<Cell>>,
    saved_cursor_x: u16,
    saved_cursor_y: u16,
    saved_cursor_visible: bool,
    saved_scroll_top: u16,
    saved_scroll_bottom: u16,
    pub in_alternate_screen: bool,

    // DECSC / DECRC cursor save-restore (ESC 7 / ESC 8, CSI s / CSI u)
    cursor_saved_x: u16,
    cursor_saved_y: u16,
}

impl CellGrid {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = cols as usize * rows as usize;
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); size],
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            title: None,
            da1_queried: false,
            scrolled_off_rows: Vec::new(),
            current_fg: TermColor::Default,
            current_bg: TermColor::Default,
            current_bold: false,
            current_italic: false,
            current_underline: false,
            current_dim: false,
            saved_cells: None,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            saved_cursor_visible: true,
            saved_scroll_top: 0,
            saved_scroll_bottom: rows.saturating_sub(1),
            in_alternate_screen: false,
            cursor_saved_x: 0,
            cursor_saved_y: 0,
        }
    }

    pub fn row(&self, n: u16) -> &[Cell] {
        let start = n as usize * self.cols as usize;
        let end = start + self.cols as usize;
        &self.cells[start..end]
    }

    pub fn put_char(&mut self, ch: char) {
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.line_feed();
        }
        let idx = self.cell_index(self.cursor_x, self.cursor_y);
        let mut flags = 0u8;
        if self.current_bold {
            flags |= CellFlags::BOLD;
        }
        if self.current_italic {
            flags |= CellFlags::ITALIC;
        }
        if self.current_underline {
            flags |= CellFlags::UNDERLINE;
        }
        if self.current_dim {
            flags |= CellFlags::DIM;
        }
        self.cells[idx] = Cell {
            ch,
            fg: self.current_fg,
            bg: self.current_bg,
            flags: CellFlags(flags),
        };
        self.cursor_x += 1;
    }

    pub fn carriage_return(&mut self) {
        self.cursor_x = 0;
    }

    pub fn line_feed(&mut self) {
        if self.cursor_y == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.cursor_y < self.rows.saturating_sub(1) {
            self.cursor_y += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    pub fn reverse_index(&mut self) {
        if self.cursor_y == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
        }
    }

    pub fn cursor_set(&mut self, col: u16, row: u16) {
        self.cursor_x = col.min(self.cols.saturating_sub(1));
        self.cursor_y = row.min(self.rows.saturating_sub(1));
    }

    pub fn cursor_up(&mut self, n: u16) {
        self.cursor_y = self.cursor_y.saturating_sub(n).max(self.scroll_top);
    }

    pub fn cursor_down(&mut self, n: u16) {
        self.cursor_y = (self.cursor_y + n).min(self.scroll_bottom);
    }

    pub fn cursor_right(&mut self, n: u16) {
        self.cursor_x = (self.cursor_x + n).min(self.cols.saturating_sub(1));
    }

    pub fn cursor_left(&mut self, n: u16) {
        self.cursor_x = self.cursor_x.saturating_sub(n);
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn erase_display(&mut self, mode: u16) {
        let cursor_pos = self.cell_index(self.cursor_x, self.cursor_y);
        match mode {
            0 => self.cells[cursor_pos..].fill(Cell::default()),
            1 => self.cells[..=cursor_pos].fill(Cell::default()),
            2 | 3 => self.cells.fill(Cell::default()),
            _ => {}
        }
    }

    pub fn erase_line(&mut self, mode: u16) {
        let row_start = self.cursor_y as usize * self.cols as usize;
        let cursor_col = self.cursor_x as usize;
        let cols = self.cols as usize;
        match mode {
            0 => self.cells[row_start + cursor_col..row_start + cols].fill(Cell::default()),
            1 => self.cells[row_start..=row_start + cursor_col].fill(Cell::default()),
            2 => self.cells[row_start..row_start + cols].fill(Cell::default()),
            _ => {}
        }
    }

    pub fn insert_lines(&mut self, n: u16) {
        let row = self.cursor_y as usize;
        let bottom = self.scroll_bottom as usize;
        let cols = self.cols as usize;
        let n = n as usize;
        for r in (row..=bottom.saturating_sub(n)).rev() {
            let src = r * cols;
            let dst = (r + n) * cols;
            self.cells.copy_within(src..src + cols, dst);
        }
        for r in row..row + n {
            let start = r * cols;
            if start + cols <= self.cells.len() {
                self.cells[start..start + cols].fill(Cell::default());
            }
        }
    }

    pub fn delete_lines(&mut self, n: u16) {
        let row = self.cursor_y as usize;
        let bottom = self.scroll_bottom as usize;
        let cols = self.cols as usize;
        let n = n as usize;
        for r in row..=bottom.saturating_sub(n) {
            let src = (r + n) * cols;
            let dst = r * cols;
            self.cells.copy_within(src..src + cols, dst);
        }
        for r in (bottom + 1).saturating_sub(n)..=bottom {
            let start = r * cols;
            if start + cols <= self.cells.len() {
                self.cells[start..start + cols].fill(Cell::default());
            }
        }
    }

    pub fn insert_chars(&mut self, n: u16) {
        let row_start = self.cursor_y as usize * self.cols as usize;
        let col = self.cursor_x as usize;
        let cols = self.cols as usize;
        let n = n as usize;
        if col + n >= cols {
            self.cells[row_start + col..row_start + cols].fill(Cell::default());
            return;
        }
        self.cells
            .copy_within(row_start + col..row_start + cols - n, row_start + col + n);
        self.cells[row_start + col..row_start + col + n].fill(Cell::default());
    }

    pub fn delete_chars(&mut self, n: u16) {
        let row_start = self.cursor_y as usize * self.cols as usize;
        let col = self.cursor_x as usize;
        let cols = self.cols as usize;
        let n = n as usize;
        if col + n >= cols {
            self.cells[row_start + col..row_start + cols].fill(Cell::default());
            return;
        }
        self.cells
            .copy_within(row_start + col + n..row_start + cols, row_start + col);
        self.cells[row_start + cols - n..row_start + cols].fill(Cell::default());
    }

    pub fn set_sgr(&mut self, params: &[u16]) {
        let default = [0u16];
        let params = if params.is_empty() {
            &default[..]
        } else {
            params
        };
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.current_fg = TermColor::Default;
                    self.current_bg = TermColor::Default;
                    self.current_bold = false;
                    self.current_italic = false;
                    self.current_underline = false;
                    self.current_dim = false;
                }
                1 => self.current_bold = true,
                2 => self.current_dim = true,
                3 => self.current_italic = true,
                4 => self.current_underline = true,
                22 => {
                    self.current_bold = false;
                    self.current_dim = false;
                }
                23 => self.current_italic = false,
                24 => self.current_underline = false,
                30..=37 => self.current_fg = TermColor::Ansi(params[i] as u8 - 30),
                38 => {
                    if params.get(i + 1) == Some(&5) && i + 2 < params.len() {
                        self.current_fg = TermColor::Ansi256(params[i + 2] as u8);
                        i += 2;
                    } else if params.get(i + 1) == Some(&2) && i + 4 < params.len() {
                        self.current_fg = TermColor::Rgb(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                39 => self.current_fg = TermColor::Default,
                40..=47 => self.current_bg = TermColor::Ansi(params[i] as u8 - 40),
                48 => {
                    if params.get(i + 1) == Some(&5) && i + 2 < params.len() {
                        self.current_bg = TermColor::Ansi256(params[i + 2] as u8);
                        i += 2;
                    } else if params.get(i + 1) == Some(&2) && i + 4 < params.len() {
                        self.current_bg = TermColor::Rgb(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                49 => self.current_bg = TermColor::Default,
                90..=97 => self.current_fg = TermColor::Ansi(params[i] as u8 - 90 + 8),
                100..=107 => self.current_bg = TermColor::Ansi(params[i] as u8 - 100 + 8),
                _ => {}
            }
            i += 1;
        }
    }

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        let top = if top == 0 { 1 } else { top };
        let bottom = if bottom == 0 { self.rows } else { bottom };
        if top < bottom && bottom <= self.rows {
            self.scroll_top = top - 1;
            self.scroll_bottom = bottom - 1;
            self.cursor_x = 0;
            self.cursor_y = 0;
        }
    }

    pub fn enter_alternate_screen(&mut self) {
        if self.in_alternate_screen {
            return;
        }
        self.saved_cells = Some(self.cells.clone());
        self.saved_cursor_x = self.cursor_x;
        self.saved_cursor_y = self.cursor_y;
        self.saved_cursor_visible = self.cursor_visible;
        // Save and reset scroll region so apps entering alt screen get a clean region
        self.saved_scroll_top = self.scroll_top;
        self.saved_scroll_bottom = self.scroll_bottom;
        let size = self.cols as usize * self.rows as usize;
        self.cells = vec![Cell::default(); size];
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.cursor_visible = true;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows.saturating_sub(1);
        self.in_alternate_screen = true;
    }

    pub fn exit_alternate_screen(&mut self) {
        if !self.in_alternate_screen {
            return;
        }
        if let Some(saved) = self.saved_cells.take() {
            self.cells = saved;
            self.cursor_x = self.saved_cursor_x;
            self.cursor_y = self.saved_cursor_y;
            self.cursor_visible = self.saved_cursor_visible;
        }
        self.scroll_top = self.saved_scroll_top;
        self.scroll_bottom = self.saved_scroll_bottom;
        self.in_alternate_screen = false;
    }

    pub fn resize(&mut self, new_cols: u16, new_rows: u16) {
        let old_cols = self.cols as usize;
        let new_cols_usize = new_cols as usize;
        let new_rows_usize = new_rows as usize;
        let mut new_cells = vec![Cell::default(); new_cols_usize * new_rows_usize];
        let copy_rows = (self.rows as usize).min(new_rows_usize);
        let copy_cols = old_cols.min(new_cols_usize);
        for r in 0..copy_rows {
            let src = r * old_cols;
            let dst = r * new_cols_usize;
            new_cells[dst..dst + copy_cols].clone_from_slice(&self.cells[src..src + copy_cols]);
        }
        self.cols = new_cols;
        self.rows = new_rows;
        self.cells = new_cells;
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self.cursor_y.min(new_rows.saturating_sub(1));
        self.scroll_top = 0;
        self.scroll_bottom = new_rows.saturating_sub(1);
    }

    pub fn save_cursor(&mut self) {
        self.cursor_saved_x = self.cursor_x;
        self.cursor_saved_y = self.cursor_y;
    }

    pub fn restore_cursor(&mut self) {
        self.cursor_x = self.cursor_saved_x.min(self.cols.saturating_sub(1));
        self.cursor_y = self.cursor_saved_y.min(self.rows.saturating_sub(1));
    }

    pub fn erase_chars(&mut self, n: u16) {
        let start = self.cell_index(self.cursor_x, self.cursor_y);
        let row_end = (self.cursor_y as usize + 1) * self.cols as usize;
        let end = (start + n as usize).min(row_end);
        self.cells[start..end].fill(Cell::default());
    }

    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    #[inline]
    fn cell_index(&self, col: u16, row: u16) -> usize {
        row as usize * self.cols as usize + col as usize
    }

    fn scroll_up(&mut self, n: usize) {
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        let cols = self.cols as usize;
        let full_screen = top == 0 && bottom + 1 == self.rows as usize;

        if full_screen && !self.in_alternate_screen {
            for r in top..top + n.min(bottom.saturating_sub(top) + 1) {
                let start = r * cols;
                if start + cols <= self.cells.len() {
                    self.scrolled_off_rows
                        .push(self.cells[start..start + cols].to_vec());
                }
            }
        }

        for r in top..=bottom.saturating_sub(n) {
            let src = (r + n) * cols;
            let dst = r * cols;
            self.cells.copy_within(src..src + cols, dst);
        }
        for r in (bottom + 1).saturating_sub(n)..=bottom {
            let start = r * cols;
            if start + cols <= self.cells.len() {
                self.cells[start..start + cols].fill(Cell::default());
            }
        }
    }

    pub fn drain_scrolled_rows(&mut self) -> Vec<Vec<Cell>> {
        std::mem::take(&mut self.scrolled_off_rows)
    }

    fn scroll_down(&mut self, n: usize) {
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        let cols = self.cols as usize;
        for r in (top..=bottom.saturating_sub(n)).rev() {
            let src = r * cols;
            let dst = (r + n) * cols;
            self.cells.copy_within(src..src + cols, dst);
        }
        for r in top..top + n {
            let start = r * cols;
            if start + cols <= self.cells.len() {
                self.cells[start..start + cols].fill(Cell::default());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_char_advances_cursor() {
        let mut grid = CellGrid::new(10, 5);
        grid.put_char('A');
        assert_eq!(grid.cells[0].ch, 'A');
        assert_eq!(grid.cursor_x, 1);
    }

    #[test]
    fn put_char_wraps_at_eol() {
        let mut grid = CellGrid::new(3, 5);
        grid.cursor_x = 2;
        grid.put_char('X');
        grid.put_char('Y');
        assert_eq!(grid.cursor_x, 1);
        assert_eq!(grid.cursor_y, 1);
    }

    #[test]
    fn line_feed_scrolls_at_bottom() {
        let mut grid = CellGrid::new(5, 3);
        grid.put_char('A');
        grid.carriage_return();
        grid.line_feed();
        grid.put_char('B');
        grid.carriage_return();
        grid.line_feed();
        grid.put_char('C');
        grid.carriage_return();
        grid.line_feed();
        assert_eq!(grid.cells[0].ch, 'B');
        assert_eq!(grid.cells[5].ch, 'C');
    }

    #[test]
    fn sgr_bold_reset() {
        let mut grid = CellGrid::new(5, 5);
        grid.set_sgr(&[1]);
        grid.put_char('B');
        assert!(grid.cells[0].flags.bold());
        grid.set_sgr(&[0]);
        grid.put_char('N');
        assert!(!grid.cells[1].flags.bold());
    }

    #[test]
    fn sgr_truecolor() {
        let mut grid = CellGrid::new(5, 5);
        grid.set_sgr(&[38, 2, 255, 0, 0]);
        grid.put_char('R');
        assert_eq!(grid.cells[0].fg, TermColor::Rgb(255, 0, 0));
    }

    #[test]
    fn alternate_screen_roundtrip() {
        let mut grid = CellGrid::new(5, 3);
        grid.put_char('M');
        grid.enter_alternate_screen();
        assert_eq!(grid.cells[0].ch, '\0');
        grid.put_char('A');
        assert_eq!(grid.cells[0].ch, 'A');
        grid.exit_alternate_screen();
        assert_eq!(grid.cells[0].ch, 'M');
    }

    #[test]
    fn cursor_position_absolute() {
        let mut grid = CellGrid::new(10, 10);
        grid.cursor_set(5, 3);
        assert_eq!(grid.cursor_x, 5);
        assert_eq!(grid.cursor_y, 3);
    }

    #[test]
    fn erase_display_all() {
        let mut grid = CellGrid::new(5, 3);
        for c in grid.cells.iter_mut() {
            c.ch = 'X';
        }
        grid.erase_display(2);
        for c in &grid.cells {
            assert_eq!(c.ch, '\0');
        }
    }

    #[test]
    fn resize_preserves_content() {
        let mut grid = CellGrid::new(5, 3);
        grid.put_char('A');
        grid.resize(10, 5);
        assert_eq!(grid.cols, 10);
        assert_eq!(grid.rows, 5);
        assert_eq!(grid.cells[0].ch, 'A');
    }
}
