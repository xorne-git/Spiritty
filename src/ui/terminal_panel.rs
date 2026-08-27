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
        let (title_text, title_style) = match self.app.system_context.active_session {
            crate::system::ActiveSession::Ssh { ref target, .. } => {
                if let Some(ref profile) = self.app.system_context.active_remote_profile {
                    (
                        format!(
                            "🌐 SSH: {} ({})",
                            target,
                            profile
                                .distro
                                .split_whitespace()
                                .next()
                                .unwrap_or(&profile.distro)
                        ),
                        Style::default()
                            .fg(if is_focused {
                                palette.warning
                            } else {
                                palette.border_unfocused
                            })
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (
                        format!("🌐 SSH: {}", target),
                        Style::default()
                            .fg(if is_focused {
                                palette.warning
                            } else {
                                palette.border_unfocused
                            })
                            .add_modifier(Modifier::BOLD),
                    )
                }
            }
            crate::system::ActiveSession::Container {
                ref runtime,
                ref container_id,
            } => (
                format!("📦 {}: {}", runtime, container_id),
                Style::default()
                    .fg(if is_focused {
                        palette.accent_secondary
                    } else {
                        palette.border_unfocused
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            crate::system::ActiveSession::Local {
                ref foreground_process,
            } => {
                let base = if let Some(proc) = foreground_process {
                    if proc != "fish" && proc != "bash" && proc != "zsh" && proc != "sh" {
                        format!(
                            "💻 {} ({})",
                            self.app.system_context.terminal_emulator, proc
                        )
                    } else {
                        format!("💻 {}", self.app.system_context.terminal_emulator)
                    }
                } else {
                    format!("💻 {}", self.app.system_context.terminal_emulator)
                };

                let cwd_info = match (
                    &self.app.system_context.current_dir,
                    &self.app.system_context.git_branch,
                ) {
                    (Some(cwd), Some(branch)) => format!(" [ {} ] ( {})", cwd, branch),
                    (Some(cwd), None) => format!(" [ {} ]", cwd),
                    _ => String::new(),
                };

                let max_available = (area.width.saturating_sub(25)) as usize;
                let full = format!("{}{}", base, cwd_info);
                let text = if full.len() > max_available && !cwd_info.is_empty() {
                    if let Some(ref cwd) = self.app.system_context.current_dir {
                        let short_cwd = cwd.split('/').next_back().unwrap_or(cwd);
                        if let Some(ref branch) = self.app.system_context.git_branch {
                            format!("{} [{}] ( {})", base, short_cwd, branch)
                        } else {
                            format!("{} [{}]", base, short_cwd)
                        }
                    } else {
                        base
                    }
                } else {
                    full
                };

                let style = Style::default()
                    .fg(if is_focused {
                        palette.accent_primary
                    } else {
                        palette.border_unfocused
                    })
                    .add_modifier(Modifier::BOLD);

                // Resumed-SSH hint: the session was continued with `-c` and it WAS
                // remote, but the PTY is currently local (user must reconnect). The
                // live-SSH case already shows the 🌐 title above, so no hint there.
                let lang = self.app.config.get_language();
                let resumed_word = if lang == Language::Fr {
                    "reprise"
                } else {
                    "resumed"
                };
                // Graded hint variants, widest first — the hint (transient, tells
                // the user to reconnect) ranks ABOVE the cwd/branch info, and the
                // fit is measured in TERMINAL CELLS (byte length lies for emojis).
                let target_opt = self.app.current_session.last_ssh_target.clone();
                // Display rule: the hint appears ONLY when this session has a known
                // SSH history (resumed with -c). A None/empty target = never remote
                // (or brand-new session) => NO hint at all. The live-SSH case shows
                // the 🌐 title instead, so no hint there either.
                let hints: Vec<String> = match target_opt.as_deref() {
                    Some(t) if !t.is_empty() => vec![
                        format!(" · 🔗 SSH {} ({})", t, resumed_word),
                        format!(" · 🔗 {} ({})", t, resumed_word),
                        format!(" · 🔗 SSH ({})", resumed_word),
                        // Last-resort tier without the emoji: 3 cells saved, matters
                        // on narrow splits where even the compact hint would overflow.
                        format!(" · SSH ({})", resumed_word),
                    ],
                    _ => Vec::new(),
                };
                let cell_w = |s: &str| unicode_width::UnicodeWidthStr::width(s);
                let base_part = text.split(" [").next().unwrap_or(&text).to_string();
                let base_cell_w = cell_w(&base_part);
                let text = hints
                    .iter()
                    .find(|h| cell_w(&text) + cell_w(h) <= max_available)
                    .map(|h| format!("{}{}", text, h))
                    .or_else(|| {
                        // Drop the cwd part and retry with the widest hint that fits.
                        hints
                            .iter()
                            .find(|h| base_cell_w + cell_w(h) <= max_available)
                            .map(|h| format!("{}{}", base_part, h))
                    })
                    .unwrap_or(text);

                (text, style)
            }
        };

        // 1. Icon + Title on the LEFT of terminal panel (1 char padding)
        buf.set_string(area.left() + 1, area.top(), &title_text, title_style);

        let (scroll_offset, total_lines) = self.app.pty.scroll_info();

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
        self.app.pty.screen().render_to_buffer(inner_area, buf);

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
