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

    pub fn process(&mut self, bytes: &[u8]) {
        let parser = &mut self.parser;
        let grid = &mut self.grid;
        let mut performer = Performer { grid };
        for byte in bytes {
            parser.advance(&mut performer, *byte);
        }
    }

    pub fn reset_parser(&mut self) {
        self.parser = vte::Parser::new();
    }
}

struct Performer<'a> {
    grid: &'a mut CellGrid,
}

impl<'a> vte::Perform for Performer<'a> {
    fn print(&mut self, c: char) {
        self.grid.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x0D => self.grid.carriage_return(),
            0x0A => self.grid.line_feed(),
            0x08 => self.grid.backspace(),
            0x07 => {}
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let p: Vec<u16> = params
            .iter()
            .map(|sub| sub.iter().next().copied().unwrap_or(0))
            .collect();
        let p0 = p.first().copied().unwrap_or(0);
        let p1 = p.get(1).copied().unwrap_or(0);

        match (intermediates, action) {
            ([], 'A') => self.grid.cursor_up(p0.max(1)),
            ([], 'B') => self.grid.cursor_down(p0.max(1)),
            ([], 'C') => self.grid.cursor_right(p0.max(1)),
            ([], 'D') => self.grid.cursor_left(p0.max(1)),
            ([], 'H') | ([], 'f') => {
                let row = p0.saturating_sub(1);
                let col = p1.saturating_sub(1);
                self.grid.cursor_set(col, row);
            }
            ([], 'G') => self
                .grid
                .cursor_set(p0.saturating_sub(1), self.grid.cursor_y),
            ([], 'd') => self
                .grid
                .cursor_set(self.grid.cursor_x, p0.saturating_sub(1)),
            ([], 'J') => self.grid.erase_display(p0),
            ([], 'K') => self.grid.erase_line(p0),
            ([], 'L') => self.grid.insert_lines(p0.max(1)),
            ([], 'M') => self.grid.delete_lines(p0.max(1)),
            ([], '@') => self.grid.insert_chars(p0.max(1)),
            ([], 'P') => self.grid.delete_chars(p0.max(1)),
            ([], 'm') => self.grid.set_sgr(&p),
            ([], 'r') => self.grid.set_scroll_region(p0, p1),
            ([], 'c') => self.grid.da1_queried = true,
            ([b'?'], 'h') => self.handle_dec_private_set(p0),
            ([b'?'], 'l') => self.handle_dec_private_reset(p0),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        match (params.first(), params.get(1)) {
            (Some(first), Some(title_bytes)) if *first == b"0" || *first == b"2" => {
                if let Ok(title) = std::str::from_utf8(title_bytes) {
                    self.grid.set_title(title.to_string());
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        if byte == b'M' {
            self.grid.reverse_index();
        }
    }
}

impl<'a> Performer<'a> {
    fn handle_dec_private_set(&mut self, mode: u16) {
        match mode {
            25 => self.grid.set_cursor_visible(true),
            1049 => self.grid.enter_alternate_screen(),
            _ => {}
        }
    }

    fn handle_dec_private_reset(&mut self, mode: u16) {
        match mode {
            25 => self.grid.set_cursor_visible(false),
            1049 => self.grid.exit_alternate_screen(),
            _ => {}
        }
    }
}
