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
            ([], 'm') => {
                // Expand colon sub-params so that `38:2:r:g:b` (a single vte param
                // with sub-params) is treated the same as `38;2;r;g;b` (separate params).
                let sgr_p: Vec<u16> = params.iter().flat_map(|sub| sub.iter().copied()).collect();
                self.grid.set_sgr(&sgr_p);
            }
            ([], 'r') => self.grid.set_scroll_region(p0, p1),
            ([], 'c') => self.grid.da1_queried = true,
            ([], 's') => self.grid.save_cursor(),
            ([], 'u') => self.grid.restore_cursor(),
            ([], 'X') => self.grid.erase_chars(p0.max(1)),
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
        match byte {
            b'7' => self.grid.save_cursor(),
            b'8' => self.grid.restore_cursor(),
            b'M' => self.grid.reverse_index(),
            _ => {}
        }
    }
}

impl<'a> Performer<'a> {
    fn handle_dec_private_set(&mut self, mode: u16) {
        match mode {
            25 => self.grid.set_cursor_visible(true),
            47 | 1047 | 1049 => self.grid.enter_alternate_screen(),
            // Mouse reporting modes
            1000 | 1002 | 1003 => self.grid.mouse_reporting = true,
            1006 => self.grid.mouse_sgr = true,
            _ => {}
        }
    }

    fn handle_dec_private_reset(&mut self, mode: u16) {
        match mode {
            25 => self.grid.set_cursor_visible(false),
            47 | 1047 | 1049 => self.grid.exit_alternate_screen(),
            // Mouse reporting modes
            1000 | 1002 | 1003 => self.grid.mouse_reporting = false,
            1006 => self.grid.mouse_sgr = false,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbt_protocol::TermColor;

    fn parse(seq: &[u8]) -> VtParser {
        let mut p = VtParser::new(80, 24);
        p.process(seq);
        p
    }

    #[test]
    fn sgr_reverse_erase_to_eol_fills_background() {
        // ESC[32m (fg=green), ESC[7m (reverse), "hello", ESC[K (erase to EOL), ESC[m (reset)
        // With REVERSE flag approach: cells store raw fg=Ansi(2), bg=Default with REVERSE set.
        // The renderer applies Modifier::REVERSED which swaps fg/bg at draw time.
        let seq = b"\x1b[3;1H\x1b[32m\x1b[7mhello\x1b[K\x1b[m";
        let p = parse(seq);
        let row = 2usize;
        let cols = 80usize;
        for col in 0..5usize {
            let cell = &p.grid.cells[row * cols + col];
            assert!(
                cell.flags.reverse(),
                "hello char at col {col} should have REVERSE flag set"
            );
            assert_eq!(
                cell.fg,
                TermColor::Ansi(2),
                "hello char at col {col} should have fg=Ansi(2) (green)"
            );
        }
        for col in 5..80usize {
            let cell = &p.grid.cells[row * cols + col];
            assert!(
                cell.flags.reverse(),
                "erased col {col} should have REVERSE flag set"
            );
            assert_eq!(
                cell.fg,
                TermColor::Ansi(2),
                "erased col {col} should have fg=Ansi(2)"
            );
        }
    }

    #[test]
    fn sgr_bg_color_erase_to_eol() {
        // SGR 44 sets bg=Ansi(4) — no reverse, erased cells should have bg=Ansi(4) directly.
        let seq = b"\x1b[2;1H\x1b[44mtext\x1b[K\x1b[m";
        let p = parse(seq);
        let row = 1usize;
        let cols = 80usize;
        for col in 4..80usize {
            let cell = &p.grid.cells[row * cols + col];
            assert_eq!(
                cell.bg,
                TermColor::Ansi(4),
                "erased col {col} should have bg=Ansi(4)"
            );
            assert!(
                !cell.flags.reverse(),
                "erased col {col} should not have REVERSE flag"
            );
        }
    }

    #[test]
    fn sgr_reverse_default_colors_stores_flag() {
        // The key case: reverse on Default fg/bg — pre-swapping loses the reversal
        // since Default^Default = Default. REVERSE flag must be stored in the cell.
        let mut p = VtParser::new(80, 24);
        p.process(b"\x1b[7mA\x1b[m");
        let cell = &p.grid.cells[0];
        assert!(cell.flags.reverse(), "A with SGR 7 must have REVERSE flag");
        assert_eq!(cell.fg, TermColor::Default);
        assert_eq!(cell.bg, TermColor::Default);
    }

    #[test]
    fn cursor_show_hide() {
        let mut p = VtParser::new(80, 24);
        assert!(p.grid.cursor_visible);
        p.process(b"\x1b[?25l");
        assert!(!p.grid.cursor_visible);
        p.process(b"\x1b[?25h");
        assert!(p.grid.cursor_visible);
    }

    #[test]
    fn sgr_colon_subparam_rgb_foreground() {
        // ESC[38:2:255:0:128m — colon-separated truecolor fg (yazi/kitty style)
        // vte delivers this as one param with sub-params [38, 2, 255, 0, 128]
        // Our fix must expand this into a flat [38, 2, 255, 0, 128] for set_sgr.
        let mut p = VtParser::new(80, 24);
        p.process(b"\x1b[38:2:255:0:128mA");
        assert_eq!(p.grid.cells[0].fg, TermColor::Rgb(255, 0, 128));
    }

    #[test]
    fn sgr_colon_subparam_rgb_background() {
        // ESC[48:2:0:200:50m — colon-separated truecolor bg
        let mut p = VtParser::new(80, 24);
        p.process(b"\x1b[48:2:0:200:50mA");
        assert_eq!(p.grid.cells[0].bg, TermColor::Rgb(0, 200, 50));
    }
}
