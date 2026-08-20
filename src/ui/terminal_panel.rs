use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::app::{App, Focus};

pub struct TerminalPanel<'a> {
    app: &'a mut App,
}

impl<'a> TerminalPanel<'a> {
    pub fn new(app: &'a mut App) -> Self {
        Self { app }
    }
}

impl<'a> TerminalPanel<'a> {
    pub fn render_panel(self, area: Rect, buf: &mut Buffer) -> Option<(u16, u16)> {
        let is_focused = self.app.focus == Focus::Terminal;
        let title_text = format!("💻 {}", self.app.system_context.terminal_emulator);

        // 1. Icon + Title on the LEFT of terminal panel (1 char padding)
        buf.set_string(
            area.left() + 1,
            area.top(),
            &title_text,
            Style::default()
                .fg(if is_focused { Color::Cyan } else { Color::DarkGray })
                .add_modifier(Modifier::BOLD),
        );

        let (scroll_offset, total_lines) = self.app.pty.scroll_info();

        // 2. Line count badge on the RIGHT of terminal panel (1 char padding)
        if area.width > 25 {
            let (badge_text, badge_style) = if scroll_offset > 0 {
                (
                    format!("▲ -{} / {} l.", scroll_offset, total_lines),
                    Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    format!("📜 {} l.", total_lines),
                    Style::default().fg(if is_focused { Color::Cyan } else { Color::DarkGray }),
                )
            };

            let badge_len = badge_text.chars().count() as u16;
            let badge_x = area.right().saturating_sub(badge_len + 1);
            buf.set_string(badge_x, area.top(), &badge_text, badge_style);
        }

        // Inner area for VT100: begins below top liseret, leaves 1 char margin on left
        let inner_area = Rect {
            x: area.left() + 1,
            y: area.top() + 1,
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };

        if inner_area.width == 0 || inner_area.height == 0 {
            return None;
        }

        // Notify PTY of current inner render area
        self.app.update_terminal_size(inner_area);

        // Render VT100 screen buffer
        self.app.pty.screen().render_to_buffer(inner_area, buf);

        // Cursor calculation: only show live hardware cursor when on live screen
        if is_focused && scroll_offset == 0 {
            let (cursor_col, cursor_row, visible) = self.app.pty.screen().cursor_position();
            if visible {
                let abs_x = inner_area.left() + cursor_col;
                let abs_y = inner_area.top() + cursor_row;
                if abs_x < inner_area.right() && abs_y < inner_area.bottom() {
                    return Some((abs_x, abs_y));
                }
            }
        }

        None
    }
}
