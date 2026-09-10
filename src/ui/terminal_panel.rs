use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};

use crate::{
    app::{App, Focus},
    i18n::Language,
};

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
        let palette = self.app.theme.palette();

        // Clear hit regions for this frame
        self.app.terminal_tab_hits.borrow_mut().clear();

        let top_y = area.top();
        let right_bound = area.right().saturating_sub(18); // Leave space for line badge
        let mut curr_x = area.left() + 1;

        let num_tabs = self.app.tabs.len();
        for (i, tab) in self.app.tabs.iter().enumerate() {
            if curr_x >= right_bound {
                break;
            }

            let is_active = i == self.app.active_tab_index;
            let title = tab.display_title();
            let unread = if tab.unread_activity && !is_active {
                " ●"
            } else {
                ""
            };

            let max_tab_len = 20;
            let clean_title = if title.len() > max_tab_len {
                let cut = title.floor_char_boundary(max_tab_len - 1);
                format!("{}…", &title[..cut])
            } else {
                title
            };

            let tab_num = i + 1;
            let tab_label = if is_active && num_tabs > 1 {
                format!(" {} {}{} × ", tab_num, clean_title, unread)
            } else {
                format!(" {} {}{} ", tab_num, clean_title, unread)
            };

            let tab_width = unicode_width::UnicodeWidthStr::width(tab_label.as_str()) as u16;
            if curr_x + tab_width > right_bound && i > 0 {
                break;
            }

            let tab_style = if is_active {
                Style::default()
                    .bg(if is_focused {
                        palette.accent_primary
                    } else {
                        palette.border_unfocused
                    })
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else if tab.unread_activity {
                Style::default()
                    .fg(palette.warning)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.text_secondary)
            };

            buf.set_string(curr_x, top_y, &tab_label, tab_style);

            let tab_end_x =
                curr_x + tab_width.saturating_sub(if is_active && num_tabs > 1 { 3 } else { 0 });
            self.app
                .terminal_tab_hits
                .borrow_mut()
                .push(crate::app::TerminalTabHit {
                    tab_index: i,
                    start_x: curr_x,
                    end_x: tab_end_x,
                    y: top_y,
                    is_close: false,
                    is_plus: false,
                });

            if is_active && num_tabs > 1 {
                let close_start_x = tab_end_x;
                let close_end_x = curr_x + tab_width;
                self.app
                    .terminal_tab_hits
                    .borrow_mut()
                    .push(crate::app::TerminalTabHit {
                        tab_index: i,
                        start_x: close_start_x,
                        end_x: close_end_x,
                        y: top_y,
                        is_close: true,
                        is_plus: false,
                    });
            }

            curr_x += tab_width + 1;
        }

        // Plus button [+]
        if curr_x + 3 < right_bound {
            let plus_str = " + ";
            let plus_style = Style::default()
                .fg(if is_focused {
                    palette.accent_primary
                } else {
                    palette.text_secondary
                })
                .add_modifier(Modifier::BOLD);
            buf.set_string(curr_x, top_y, plus_str, plus_style);
            self.app
                .terminal_tab_hits
                .borrow_mut()
                .push(crate::app::TerminalTabHit {
                    tab_index: 0,
                    start_x: curr_x,
                    end_x: curr_x + 3,
                    y: top_y,
                    is_close: false,
                    is_plus: true,
                });
            curr_x += 3;
        }

        // Resumed-SSH hint: the session was continued with `-c` and it WAS
        // remote, but the PTY is currently local (user must reconnect).
        if !self.app.active_tab().active_session.is_ssh() {
            if let Some(ref target) = self.app.current_session.last_ssh_target {
                if !target.is_empty() {
                    let lang = self.app.config.get_language();
                    let resumed_word = if lang == Language::Fr {
                        "reprise"
                    } else {
                        "resumed"
                    };
                    let max_hint_available = right_bound.saturating_sub(curr_x) as usize;
                    let hints: Vec<String> = vec![
                        format!(" · 🔗 SSH {} ({})", target, resumed_word),
                        format!(" · 🔗 {} ({})", target, resumed_word),
                        format!(" · 🔗 SSH ({})", resumed_word),
                        format!(" · SSH ({})", resumed_word),
                    ];
                    let cell_w = |s: &str| unicode_width::UnicodeWidthStr::width(s);
                    if let Some(hint) = hints.iter().find(|h| cell_w(h) <= max_hint_available) {
                        let hint_style = Style::default()
                            .fg(palette.warning)
                            .add_modifier(Modifier::BOLD);
                        buf.set_string(curr_x, top_y, hint, hint_style);
                    }
                }
            }
        }

        // Active PTY tool running indicator with Shift+Tab hint
        if self.app.active_pty_tool.is_some() {
            let lang = self.app.config.get_language();
            let max_running_available = right_bound.saturating_sub(curr_x) as usize;
            let hints: Vec<String> = if is_focused {
                vec![
                    if lang == Language::Fr {
                        " · ⚡ En cours"
                    } else {
                        " · ⚡ Running"
                    }
                    .to_string(),
                    " · ⚡".to_string(),
                ]
            } else {
                vec![
                    if lang == Language::Fr {
                        " · ⚡ En cours (Shift+Tab pour interagir)"
                    } else {
                        " · ⚡ Running (Shift+Tab to interact)"
                    }
                    .to_string(),
                    " · ⚡ Shift+Tab".to_string(),
                    " · ⚡".to_string(),
                ]
            };
            let cell_w = |s: &str| unicode_width::UnicodeWidthStr::width(s);
            if let Some(hint) = hints.iter().find(|h| cell_w(h) <= max_running_available) {
                let hint_style = Style::default()
                    .fg(palette.accent_primary)
                    .add_modifier(Modifier::BOLD);
                buf.set_string(curr_x, top_y, hint, hint_style);
            }
        }

        let (scroll_offset, total_lines) = self.app.active_tab().pty.scroll_info();

        // 2. Line count badge on the RIGHT of terminal panel (1 char padding)
        if area.width > 25 {
            let (badge_text, badge_style) = if scroll_offset > 0 {
                (
                    format!("▲ -{} / {} l.", scroll_offset, total_lines),
                    Style::default()
                        .bg(palette.accent_primary)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    format!("📜 {} l.", total_lines),
                    Style::default().fg(if is_focused {
                        palette.accent_primary
                    } else {
                        palette.border_unfocused
                    }),
                )
            };

            let badge_len = unicode_width::UnicodeWidthStr::width(badge_text.as_str()) as u16;
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
        self.app
            .active_tab()
            .pty
            .screen()
            .render_to_buffer(inner_area, buf);

        // 3. Proactive Error Diagnosis floating modal in the terminal panel
        if let Some(ref diag) = self.app.proactive_error_diagnosis {
            let lang = self.app.config.get_language();
            let toast_width = (inner_area.width.saturating_sub(4)).clamp(36, 56);
            let toast_height = 5u16;

            if inner_area.width >= toast_width && inner_area.height >= toast_height + 2 {
                let toast_x = inner_area.right().saturating_sub(toast_width + 1);
                let toast_y = inner_area.bottom().saturating_sub(toast_height + 1);
                let toast_area = Rect::new(toast_x, toast_y, toast_width, toast_height);

                Clear.render(toast_area, buf);

                let title = if lang == Language::Fr {
                    " ⚡ Erreur détectée "
                } else {
                    " ⚡ Error Detected "
                };

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(Style::default().fg(palette.warning))
                    .padding(Padding::horizontal(1))
                    .title(Span::styled(
                        title,
                        Style::default()
                            .fg(palette.warning)
                            .add_modifier(Modifier::BOLD),
                    ));

                let inner_toast = block.inner(toast_area);
                block.render(toast_area, buf);

                let max_text_len = (inner_toast.width.saturating_sub(6)) as usize;
                let cmd_short = if diag.command.len() > max_text_len {
                    let cut = diag
                        .command
                        .floor_char_boundary(max_text_len.saturating_sub(1));
                    format!("{}…", &diag.command[..cut])
                } else {
                    diag.command.clone()
                };

                let line1 = Line::from(vec![
                    Span::styled("Cmd: ", Style::default().fg(palette.text_secondary)),
                    Span::styled(
                        cmd_short,
                        Style::default()
                            .fg(palette.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);

                let line2 = Line::from(vec![Span::styled(
                    if lang == Language::Fr {
                        "Diagnostiquer avec l'agent IA ?"
                    } else {
                        "Diagnose with AI agent?"
                    },
                    Style::default().fg(palette.text_secondary),
                )]);

                let mut btn_spans = Vec::new();
                btn_spans.extend(key_pill("Alt + D", palette.success));
                btn_spans.push(Span::styled(
                    " OK  ",
                    Style::default()
                        .fg(palette.text_primary)
                        .add_modifier(Modifier::BOLD),
                ));
                btn_spans.extend(key_pill("Alt + X", palette.text_secondary));
                btn_spans.push(Span::styled(
                    " Cancel",
                    Style::default().fg(palette.text_secondary),
                ));

                let p = Paragraph::new(vec![line1, line2, Line::from(btn_spans)]);
                p.render(inner_toast, buf);
            }
        }

        // Cursor calculation: only show live hardware cursor when on live screen
        if is_focused && scroll_offset == 0 {
            let (cursor_col, cursor_row, visible) =
                self.app.active_tab().pty.screen().cursor_position();
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

fn key_pill(key: &str, color: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled("", Style::default().fg(color)),
        Span::styled(
            key.to_string(),
            Style::default()
                .bg(color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("", Style::default().fg(color)),
    ]
}
