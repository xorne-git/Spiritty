use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::sync::{Arc, Mutex};
use vt100::Parser;

/// Wrapper around `vt100::Parser` providing thread-safe screen parsing
/// and conversion to Ratatui buffer cells.
#[derive(Clone)]
pub struct VtScreen {
    parser: Arc<Mutex<Parser>>,
}

impl VtScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = Parser::new(rows, cols, 10_000);
        Self {
            parser: Arc::new(Mutex::new(parser)),
        }
    }

    pub fn process(&self, bytes: &[u8]) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.process(bytes);
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.set_size(rows.max(1), cols.max(1));
            parser.set_scrollback(0);
        }
    }

    pub fn scroll_up(&self, lines: usize) {
        if let Ok(mut parser) = self.parser.lock() {
            let current = parser.screen().scrollback();
            let screen_rows = parser.screen().size().0 as usize;
            let max_safe = screen_rows.saturating_sub(1);
            let next = current.saturating_add(lines).min(max_safe);
            parser.set_scrollback(next);
        }
    }

    pub fn scroll_down(&self, lines: usize) {
        if let Ok(mut parser) = self.parser.lock() {
            let current = parser.screen().scrollback();
            parser.set_scrollback(current.saturating_sub(lines));
        }
    }

    pub fn reset_scroll(&self) {
        if let Ok(mut parser) = self.parser.lock() {
            parser.set_scrollback(0);
        }
    }

    pub fn scroll_offset(&self) -> usize {
        if let Ok(parser) = self.parser.lock() {
            parser.screen().scrollback()
        } else {
            0
        }
    }

    /// Returns `(current_scroll_offset, total_terminal_lines)`
    pub fn scroll_info(&self) -> (usize, usize) {
        if let Ok(mut parser) = self.parser.lock() {
            let current = parser.screen().scrollback();
            let screen_rows = parser.screen().size().0 as usize;
            parser.set_scrollback(usize::MAX);
            let max_scrollback = parser.screen().scrollback();
            parser.set_scrollback(current);
            (current, screen_rows.saturating_add(max_scrollback))
        } else {
            (0, 0)
        }
    }

    /// Returns `(col, row, is_visible)` for the terminal cursor.
    pub fn cursor_position(&self) -> (u16, u16, bool) {
        if let Ok(parser) = self.parser.lock() {
            let screen = parser.screen();
            let (row, col) = screen.cursor_position();
            let hide_cursor = screen.hide_cursor();
            (col, row, !hide_cursor)
        } else {
            (0, 0, false)
        }
    }

    /// Renders the virtual VT100 screen buffer directly onto a Ratatui `Buffer`.
    pub fn render_to_buffer(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let Ok(parser) = self.parser.lock() else {
            return;
        };
        let screen = parser.screen();

        for row in 0..area.height {
            let screen_row = row;
            for col in 0..area.width {
                let screen_col = col;
                let buf_x = area.left() + col;
                let buf_y = area.top() + row;

                if buf_x >= area.right() || buf_y >= area.bottom() {
                    continue;
                }

                if let Some(cell) = screen.cell(screen_row, screen_col) {
                    let contents = cell.contents();
                    let symbol = if cell.has_contents() {
                        contents.as_str()
                    } else {
                        " "
                    };

                    let mut style = Style::default();

                    // Convert colors
                    if let Some(fg) = convert_vt_color(cell.fgcolor()) {
                        style = style.fg(fg);
                    }
                    if let Some(bg) = convert_vt_color(cell.bgcolor()) {
                        style = style.bg(bg);
                    }

                    // Convert text attributes
                    let mut modifier = Modifier::empty();
                    if cell.bold() {
                        modifier |= Modifier::BOLD;
                    }
                    if cell.italic() {
                        modifier |= Modifier::ITALIC;
                    }
                    if cell.underline() {
                        modifier |= Modifier::UNDERLINED;
                    }
                    if cell.inverse() {
                        modifier |= Modifier::REVERSED;
                    }
                    style = style.add_modifier(modifier);

                    buf.set_string(buf_x, buf_y, symbol, style);
                } else {
                    buf.set_string(buf_x, buf_y, " ", Style::default());
                }
            }
        }
    }
}

fn convert_vt_color(color: vt100::Color) -> Option<Color> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(idx) => Some(Color::Indexed(idx)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}
