use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};
use vt100::Parser;

/// Smallest dimension ever handed to the `vt100` parser.
///
/// The crate panics on degenerate screens: in `vt100::grid::Grid::col_wrap` the
/// `prev_pos.row -= scrolled` subtraction underflows (u16 wrap) on a one-row /
/// one-line-scroll-region grid, and the following `drawing_row_mut(..).unwrap()`
/// then panics. Every entry point clamps to at least this size, and the parser
/// is additionally driven behind a `catch_unwind` guard.
const MIN_DIM: u16 = 2;

/// Wrapper around `vt100::Parser` providing thread-safe screen parsing
/// and conversion to Ratatui buffer cells.
#[derive(Clone)]
pub struct VtScreen {
    parser: Arc<Mutex<Parser>>,
}

impl VtScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = Parser::new(rows.max(MIN_DIM), cols.max(MIN_DIM), 10_000);
        Self {
            parser: Arc::new(Mutex::new(parser)),
        }
    }

    /// Locks the parser, transparently recovering from a poisoned mutex.
    ///
    /// A panic inside `vt100` would otherwise poison the lock and freeze the
    /// terminal panel forever (every later `lock()` returns `Err`).
    fn lock(&self) -> MutexGuard<'_, Parser> {
        self.parser
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn process(&self, bytes: &[u8]) {
        let mut parser = self.lock();
        // `vt100` 0.15/0.16 can panic on tiny screens (`grid.rs::col_wrap`).
        // Left unguarded, that panic unwinds through the PTY reader thread,
        // trips the global panic hook and tears down the terminal while the UI
        // is still running. Swallow it: worst case the panel shows one stale
        // cell, never a crash or a corrupted tty.
        let _ = catch_unwind(AssertUnwindSafe(|| {
            parser.process(bytes);
        }));
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let mut parser = self.lock();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            parser.set_size(rows.max(MIN_DIM), cols.max(MIN_DIM));
            parser.set_scrollback(0);
        }));
    }

    pub fn scroll_up(&self, lines: usize) {
        let mut parser = self.lock();
        let current = parser.screen().scrollback();
        let screen_rows = parser.screen().size().0 as usize;
        let max_safe = screen_rows.saturating_sub(1);
        let next = current.saturating_add(lines).min(max_safe);
        parser.set_scrollback(next);
    }

    pub fn scroll_down(&self, lines: usize) {
        let mut parser = self.lock();
        let current = parser.screen().scrollback();
        parser.set_scrollback(current.saturating_sub(lines));
    }

    pub fn reset_scroll(&self) {
        let mut parser = self.lock();
        parser.set_scrollback(0);
    }

    pub fn scroll_offset(&self) -> usize {
        self.lock().screen().scrollback()
    }

    /// Returns `(current_scroll_offset, total_terminal_lines)`
    pub fn scroll_info(&self) -> (usize, usize) {
        let mut parser = self.lock();
        let current = parser.screen().scrollback();
        let screen_rows = parser.screen().size().0 as usize;
        parser.set_scrollback(usize::MAX);
        let max_scrollback = parser.screen().scrollback();
        parser.set_scrollback(current);
        (current, screen_rows.saturating_add(max_scrollback))
    }

    /// Returns `(col, row, is_visible)` for the terminal cursor.
    pub fn cursor_position(&self) -> (u16, u16, bool) {
        let parser = self.lock();
        let screen = parser.screen();
        let (row, col) = screen.cursor_position();
        let hide_cursor = screen.hide_cursor();
        (col, row, !hide_cursor)
    }

    /// Whether the terminal child is currently using the alternate screen buffer (e.g. vim, htop, less).
    pub fn is_alternate_screen(&self) -> bool {
        self.lock().screen().alternate_screen()
    }

    /// Whether the terminal child expects application cursor keys (DECCKM).
    pub fn application_cursor(&self) -> bool {
        self.lock().screen().application_cursor()
    }

    /// Active mouse tracking protocol mode (None, Press, PressRelease, ButtonMotion, AnyMotion).
    pub fn mouse_protocol_mode(&self) -> vt100::MouseProtocolMode {
        self.lock().screen().mouse_protocol_mode()
    }

    /// Active mouse protocol encoding (Default, Utf8, Sgr).
    pub fn mouse_protocol_encoding(&self) -> vt100::MouseProtocolEncoding {
        self.lock().screen().mouse_protocol_encoding()
    }

    /// Whether the terminal child enabled bracketed paste mode (\x1b[?2004h).
    pub fn bracketed_paste(&self) -> bool {
        self.lock().screen().bracketed_paste()
    }

    /// Renders the virtual VT100 screen buffer directly onto a Ratatui `Buffer`.
    pub fn render_to_buffer(&self, area: Rect, buf: &mut Buffer) {
        // Clamp to the actual buffer: a stale/over-large layout area must never
        // make `set_string` write outside the buffer (which panics in ratatui).
        let area = area.intersection(buf.area);
        if area.width == 0 || area.height == 0 {
            return;
        }

        let parser = self.lock();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression for the crash users hit on small terminals: `vt100` panics in
    /// `grid.rs::col_wrap` when a line wraps on a degenerate (0/1-row) screen.
    /// `VtScreen` must clamp the size and swallow the panic instead of letting
    /// it unwind into the PTY reader thread.
    #[test]
    fn degenerate_screen_never_panics() {
        // Constructor must clamp a 1x1 request to a safe size.
        let screen = VtScreen::new(1, 1);

        // Long wrapping line on a tiny screen...
        screen.process(b"abcdefghijklmnopqrstuvwxyz0123456789\n\n\n");
        // ...and a one-line scroll region (DECSTBM), the known trigger.
        screen.process(b"\x1b[1;1rABC\r\nDEF\r\nGHI\r\n");

        // Mutating entry points must be panic-safe too.
        screen.resize(1, 1);
        let _ = screen.scroll_info();
    }
}
