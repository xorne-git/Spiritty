use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthChar;

use crate::{
    app::{App, Focus, MessageRole},
    i18n::{I18nKey, Language},
};

pub struct ChatPanel<'a> {
    app: &'a App,
}

fn key_pill(key: &str, color: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled("", Style::default().fg(color)),
        Span::styled(
            key.to_string(),
            Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::styled("", Style::default().fg(color)),
    ]
}

fn key_combo_pills(mod_key: &str, key: &str, color: Color) -> Vec<Span<'static>> {
    let mut spans = key_pill(mod_key, color);
    spans.push(Span::raw(" "));
    spans.extend(key_pill(key, color));
    spans
}

impl<'a> ChatPanel<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_panel(self, area: Rect, buf: &mut Buffer) -> Option<(u16, u16)> {
        if area.height < 5 || area.width < 6 {
            return None;
        }

        let lang = self.app.config.get_language();
        let is_focused = self.app.focus == Focus::Chat;
        let spinner_char = crate::ui::get_spinner_char(self.app.spinner_frame);
        let title_text = format!("👻 Spiritty v{} ", env!("CARGO_PKG_VERSION"));

        // 1. Dynamic prompt input sizing & line wrapping (2 lines minimum, with padding top/bot)
        let prompt_pad_x = area.left() + 2;
        let prompt_text_width = area.width.saturating_sub(4);
        let cursor_byte_pos = self.app.cursor_pos.min(self.app.chat_input.len());

        let (cursor_row, cursor_col, total_input_lines) = compute_prompt_cursor_and_lines(
            &self.app.chat_input,
            cursor_byte_pos,
            prompt_text_width as usize,
        );

        let max_input_height = (area.height.saturating_sub(6) / 2).clamp(2, 8);
        let needed_input_height = total_input_lines.clamp(2, max_input_height);

        // 2. Render Floating Header Title on Chat Panel (1 char padding)
        buf.set_string(
            area.left() + 1,
            area.top(),
            &title_text,
            Style::default()
                .fg(if is_focused { Color::Cyan } else { Color::DarkGray })
                .add_modifier(Modifier::BOLD),
        );

        // 3. Define content areas (no left, right or bottom borders)
        let prompt_total_zone = needed_input_height.saturating_add(1);
        let messages_box_height = area.height.saturating_sub(prompt_total_zone + 1);

        let messages_area = Rect {
            x: prompt_pad_x,
            y: area.top() + 1,
            width: prompt_text_width,
            height: messages_box_height,
        };

        let prompt_sep_y = area.bottom().saturating_sub(needed_input_height + 1);
        let prompt_y = area.bottom().saturating_sub(needed_input_height);
        let input_area = Rect {
            x: prompt_pad_x,
            y: prompt_y,
            width: prompt_text_width,
            height: needed_input_height,
        };

        // 4. Render Messages History with airy spacing
        let mut lines: Vec<Line<'static>> = Vec::new();

        let total_messages = self.app.messages.len();
        for (idx, msg) in self.app.messages.iter().enumerate() {
            let is_last = idx + 1 == total_messages;

            match msg.role {
                MessageRole::System => {
                    for l in msg.content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled(l.to_string(), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                    push_blank_line(&mut lines);
                }
                MessageRole::User => {
                    if msg.content.starts_with("[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '") {
                        let cmd_name = extract_tool_cmd_name(&msg.content);
                        lines.push(Line::from(vec![
                            Span::styled("💻 ", Style::default().fg(Color::Yellow)),
                            Span::styled(cmd_name.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        ]));
                    } else if msg.content.starts_with("[L'utilisateur a refusé l'exécution") {
                        lines.push(Line::from(vec![
                            Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
                            Span::styled(
                                if lang == Language::Fr { "Exécution refusée par l'utilisateur" } else { "Execution declined by user" },
                                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                            ),
                        ]));
                    } else if msg.content.starts_with("💻 ") {
                        let cmd_text = msg.content.strip_prefix("💻 ").unwrap_or(&msg.content).trim();
                        let clean_cmd = cmd_text.trim_matches('`');
                        for (l_idx, line) in clean_cmd.lines().enumerate() {
                            if l_idx == 0 {
                                lines.push(Line::from(vec![
                                    Span::styled("💻 ", Style::default().fg(Color::Yellow)),
                                    Span::styled(line.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                                ]));
                            } else {
                                lines.push(Line::from(vec![
                                    Span::raw("   "),
                                    Span::styled(line.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                                ]));
                            }
                        }
                    } else {
                        render_user_message_block(&msg.content, &mut lines, messages_area.width);
                    }
                    push_blank_line(&mut lines);
                }
                MessageRole::Assistant => {
                    let mut card_counter: usize = 0;
                    let content = &msg.content;
                    let parsed = extract_thought_block(content);

                    let ghost_prefix = if self.app.agent.is_generating && is_last {
                        if self.app.pending_tool_approval.is_some() {
                            "👻 ".to_string()
                        } else {
                            format!("{} 👻 ", spinner_char)
                        }
                    } else {
                        "👻 ".to_string()
                    };

                    let has_valid_thought = parsed.thought.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(false);

                    if has_valid_thought {
                        let thought = parsed.thought.as_ref().unwrap();
                        let thought_label = if self.app.agent.is_generating && is_last && self.app.pending_tool_approval.is_none() {
                            if lang == Language::Fr {
                                format!("{} 💭 Réflexion :", spinner_char)
                            } else {
                                format!("{} 💭 Thinking:", spinner_char)
                            }
                        } else {
                            if lang == Language::Fr {
                                "💭 Réflexion :".to_string()
                            } else {
                                "💭 Thinking:".to_string()
                            }
                        };
                        lines.push(Line::from(vec![
                            Span::styled(thought_label, Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD | Modifier::ITALIC)),
                        ]));
                        for l in thought.lines() {
                            let trimmed = l.trim();
                            if trimmed.starts_with("```") {
                                continue;
                            }
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {}", l), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                            ]));
                        }
                        push_blank_line(&mut lines);
                        if !parsed.response.is_empty() {
                            render_markdown_blocks(&parsed.response, lang, &mut lines, Some(&ghost_prefix), &mut card_counter);
                        }
                    } else if !parsed.response.is_empty() {
                        render_markdown_blocks(&parsed.response, lang, &mut lines, Some(&ghost_prefix), &mut card_counter);
                    } else if self.app.agent.is_generating && is_last && self.app.pending_tool_approval.is_none() {
                        lines.push(Line::from(vec![
                            Span::styled(ghost_prefix, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        ]));
                    }
                    push_blank_line(&mut lines);
                }
            }
        }

        // 4.1. If there is a pending tool execution approval, render the Permission Request Card
        if let Some(ref pending) = self.app.pending_tool_approval {
            let is_sudo = pending.command.contains("sudo") || pending.command.contains("root");
            let is_danger = pending.command.contains("rm -rf") || pending.command.contains("dd ") || pending.command.contains("mkfs");

            let (badge_text, badge_color) = if is_danger {
                (if lang == Language::Fr { "Risqué" } else { "Risky" }, Color::Red)
            } else if is_sudo {
                ("Sudo", Color::Yellow)
            } else {
                ("Safe", Color::Green)
            };

            let req_title = if lang == Language::Fr { "⚡ DEMANDE D'AUTORISATION " } else { "⚡ PERMISSION REQUEST " };
            let mut title_spans = vec![
                Span::styled(req_title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];
            title_spans.extend(key_pill(badge_text, badge_color));

            push_blank_line(&mut lines);
            lines.push(Line::from(title_spans));

            for l in pending.command.lines() {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}", l), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]));
            }

            let mut footer_spans = vec![Span::raw("  ")];
            footer_spans.extend(key_pill("↵ Enter", Color::Green));
            footer_spans.push(Span::styled(if lang == Language::Fr { " Autoriser   " } else { " Approve   " }, Style::default().fg(Color::White)));
            footer_spans.extend(key_pill("Esc", Color::Red));
            footer_spans.push(Span::styled(if lang == Language::Fr { " Refuser" } else { " Decline" }, Style::default().fg(Color::White)));

            lines.push(Line::from(footer_spans));
            push_blank_line(&mut lines);
        }

        // Always add 4 trailing blank lines at the very bottom so the last message and action buttons have generous breathing room
        for _ in 0..4 {
            lines.push(Line::from(""));
        }

        let total_visual_lines = compute_wrapped_lines_count(&lines, messages_area.width);
        let visible_height = messages_area.height;
        let max_scroll = total_visual_lines.saturating_sub(visible_height);

        let scroll_from_bottom = self.app.chat_scroll_from_bottom.min(max_scroll);
        let scroll_offset = max_scroll.saturating_sub(scroll_from_bottom);

        let messages_paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0));
        messages_paragraph.render(messages_area, buf);

        // Render scroll indicator badge on the top border (liseret)
        if area.width > 25 {
            let (badge_text, badge_style) = if scroll_from_bottom > 0 {
                (
                    format!(" ▲ -{} / {} l. ", scroll_from_bottom, total_visual_lines),
                    Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    format!(" 📜 {} l. ", total_visual_lines),
                    Style::default().fg(if is_focused { Color::Cyan } else { Color::DarkGray }),
                )
            };

            let badge_len = badge_text.chars().count() as u16;
            let badge_x = area.right().saturating_sub(badge_len + 1);
            let badge_y = area.top();
            buf.set_string(badge_x, badge_y, &badge_text, badge_style);
        }

        // 5. Render horizontal prompt divider liseret above input zone (1px centered line, 1 space margin at right)
        let prompt_div_style = Style::default().fg(Color::Rgb(40, 55, 75));
        for x in area.left()..area.right().saturating_sub(1) {
            buf.set_string(x, prompt_sep_y, "─", prompt_div_style);
        }

        // 6. Render Vertical Accent Bar (using left half block ▌)
        let bar_style = if is_focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        for y in input_area.top()..input_area.bottom() {
            buf.set_string(area.left(), y, "▌", bar_style);
        }

        let input_scroll = cursor_row.saturating_sub(needed_input_height.saturating_sub(1));

        if self.app.chat_input.is_empty() {
            let placeholder_line = Line::from(vec![
                Span::styled(
                    lang.t(I18nKey::ChatInputPlaceholder),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                ),
            ]);
            let p = Paragraph::new(placeholder_line).wrap(Wrap { trim: false });
            p.render(input_area, buf);
        } else {
            let input_paragraph = Paragraph::new(self.app.chat_input.as_str())
                .wrap(Wrap { trim: false })
                .scroll((input_scroll, 0))
                .style(Style::default().fg(Color::White));
            input_paragraph.render(input_area, buf);
        }

        // 6. Return cursor position for native hardware cursor rendering
        if is_focused && input_area.width > 0 && input_area.height > 0 {
            let cursor_x = if self.app.chat_input.is_empty() {
                input_area.x
            } else {
                input_area.x + cursor_col.min(input_area.width.saturating_sub(1))
            };
            let cursor_y = if self.app.chat_input.is_empty() {
                input_area.y
            } else {
                input_area.y + cursor_row.saturating_sub(input_scroll)
            };

            if cursor_x < input_area.right() && cursor_y < input_area.bottom() {
                return Some((cursor_x, cursor_y));
            }
        }

        None
    }
}

impl<'a> Widget for ChatPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_panel(area, buf);
    }
}

/// Renders markdown response text and turns ```bash ... ``` code blocks into interactive Command Proposals
fn render_markdown_blocks(
    text: &str,
    lang: Language,
    lines: &mut Vec<Line<'static>>,
    mut leading_prefix: Option<&str>,
    card_counter: &mut usize,
) {
    let mut remaining = text;

    while let Some(start_idx) = remaining.find("```") {
        let text_before = &remaining[..start_idx];
        if !text_before.trim().is_empty() {
            render_text_lines(text_before, lines, leading_prefix.take());
            push_blank_line(lines);
        }

        let after_fence = &remaining[start_idx + 3..];
        let (fence_tag, code_rest) = if let Some(first_nl) = after_fence.find('\n') {
            (after_fence[..first_nl].trim(), &after_fence[first_nl + 1..])
        } else {
            (after_fence.trim(), "")
        };

        if let Some(end_idx) = code_rest.find("```") {
            let code_content = code_rest[..end_idx].trim();
            if fence_tag.starts_with("tool:") {
                // Internal tool call block: skip rendering
            } else if fence_tag == "output" || fence_tag == "result" {
                render_tool_output_box(code_content, lines);
            } else if crate::app::is_executable_command_block(fence_tag, code_content) {
                *card_counter += 1;
                render_command_card(*card_counter, code_content, fence_tag, lang, lines);
            } else {
                render_code_snippet_box(code_content, fence_tag, lines);
            }
            remaining = &code_rest[end_idx + 3..];
        } else {
            // Streaming inside open code block
            let code_content = code_rest.trim();
            if fence_tag.starts_with("tool:") {
                // Internal tool call block: skip rendering while streaming
            } else if fence_tag == "output" || fence_tag == "result" {
                render_tool_output_box(code_content, lines);
            } else if crate::app::is_executable_command_block(fence_tag, code_content) {
                *card_counter += 1;
                render_command_card(*card_counter, code_content, fence_tag, lang, lines);
            } else {
                render_code_snippet_box(code_content, fence_tag, lines);
            }
            remaining = "";
            break;
        }
    }

    if !remaining.trim().is_empty() {
        render_text_lines(remaining, lines, leading_prefix.take());
    }
}

/// Renders user messages with vertical Cyan accent bar ▌ on every line and full-width solid dark navy background
fn render_user_message_block(content: &str, lines: &mut Vec<Line<'static>>, width: u16) {
    let user_bg = Color::Rgb(26, 36, 58);
    let bar_style = Style::default().fg(Color::Cyan).bg(user_bg).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White).bg(user_bg);

    let target_width = width as usize;
    let max_text_width = target_width.saturating_sub(2).max(1);

    for raw_line in content.lines() {
        if raw_line.trim().is_empty() {
            let pad_len = target_width.saturating_sub(2);
            lines.push(Line::from(vec![
                Span::styled("▌ ", bar_style),
                Span::styled(" ".repeat(pad_len), text_style),
            ]));
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0;

        for word in raw_line.split_whitespace() {
            let word_w = word.chars().map(|c| c.width().unwrap_or(1)).sum::<usize>();

            if current_width == 0 {
                current_line.push_str(word);
                current_width = word_w;
            } else if current_width + 1 + word_w <= max_text_width {
                current_line.push(' ');
                current_line.push_str(word);
                current_width += 1 + word_w;
            } else {
                let pad_len = target_width.saturating_sub(2 + current_width);
                let mut spans = vec![Span::styled("▌ ", bar_style)];
                spans.extend(parse_inline_spans(&current_line, text_style));
                if pad_len > 0 {
                    spans.push(Span::styled(" ".repeat(pad_len), text_style));
                }
                lines.push(Line::from(spans));

                current_line = word.to_string();
                current_width = word_w;
            }
        }

        if !current_line.is_empty() {
            let pad_len = target_width.saturating_sub(2 + current_width);
            let mut spans = vec![Span::styled("▌ ", bar_style)];
            spans.extend(parse_inline_spans(&current_line, text_style));
            if pad_len > 0 {
                spans.push(Span::styled(" ".repeat(pad_len), text_style));
            }
            lines.push(Line::from(spans));
        }
    }
}

/// Renders a sequence of markdown lines with automatic grouping of markdown tables
fn render_text_lines(
    text: &str,
    lines: &mut Vec<Line<'static>>,
    mut leading_prefix: Option<&str>,
) {
    let mut current_table_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Check if line looks like a table row (starts and contains pipe '|')
        if (trimmed.starts_with('|') || trimmed.starts_with("├─") || trimmed.starts_with("┌─") || trimmed.starts_with("└─")) && trimmed.contains('|') {
            current_table_lines.push(line);
            continue;
        }

        // Flush any accumulated table lines
        if !current_table_lines.is_empty() {
            render_table_block(&current_table_lines, lines);
            current_table_lines.clear();
        }

        if trimmed.is_empty() {
            push_blank_line(lines);
            continue;
        }

        // Section separator: --- or ***
        if (trimmed.starts_with("---") || trimmed.starts_with("***")) && trimmed.chars().all(|c| c == '-' || c == '*' || c == ' ') {
            lines.push(Line::from(vec![
                Span::styled("  ──────────────────────────────────────────", Style::default().fg(Color::DarkGray)),
            ]));
            push_blank_line(lines);
            continue;
        }

        // Regular line
        let p_span = leading_prefix.take().map(|p| {
            Span::styled(p.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        });
        render_markdown_line(line, p_span, lines);
    }

    if !current_table_lines.is_empty() {
        render_table_block(&current_table_lines, lines);
    }
}

/// Renders a markdown table with beautiful borders, cyan headers, and aligned columns
fn render_table_block(raw_table_lines: &[&str], lines: &mut Vec<Line<'static>>) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut has_separator = false;

    for line in raw_table_lines {
        let trimmed = line.trim();
        if trimmed.starts_with('├') || trimmed.starts_with('┌') || trimmed.starts_with('└') {
            continue;
        }
        let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
        if inner.chars().all(|c| c == '-' || c == '|' || c == ':' || c == ' ') {
            has_separator = true;
            continue;
        }
        let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return;
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return;
    }

    let mut col_widths = vec![0usize; num_cols];
    for row in &rows {
        for (col_idx, cell) in row.iter().enumerate() {
            let w = visual_cell_width(cell);
            col_widths[col_idx] = col_widths[col_idx].max(w);
        }
    }

    push_blank_line(lines);
    let border_style = Style::default().fg(Color::DarkGray);

    for (row_idx, row) in rows.iter().enumerate() {
        let is_header = has_separator && row_idx == 0;
        let mut spans = Vec::new();

        spans.push(Span::styled("│ ", border_style));

        for (col_idx, &w) in col_widths.iter().enumerate() {
            let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let cell_w = visual_cell_width(cell);
            let pad_right = w.saturating_sub(cell_w);

            let cell_style = if is_header {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            spans.extend(parse_inline_spans(cell, cell_style));
            if pad_right > 0 {
                spans.push(Span::raw(" ".repeat(pad_right)));
            }

            if col_idx + 1 < num_cols {
                spans.push(Span::styled(" │ ", border_style));
            } else {
                spans.push(Span::styled(" │", border_style));
            }
        }

        lines.push(Line::from(spans));

        if is_header {
            let mut sep_spans = Vec::new();
            sep_spans.push(Span::styled("├─", border_style));
            for (col_idx, &w) in col_widths.iter().enumerate() {
                sep_spans.push(Span::styled("─".repeat(w), border_style));
                if col_idx + 1 < num_cols {
                    sep_spans.push(Span::styled("─┼─", border_style));
                } else {
                    sep_spans.push(Span::styled("─┤", border_style));
                }
            }
            lines.push(Line::from(sep_spans));
        }
    }

    push_blank_line(lines);
}

fn visual_cell_width(cell: &str) -> usize {
    let clean = strip_inline_markdown(cell);
    str_visual_width(&clean)
}

fn strip_inline_markdown(text: &str) -> String {
    text.replace("**", "").replace(['`', '*'], "")
}

/// Parses inline markdown syntax (**bold**, `code`, *italic*) into styled Ratatui Spans
fn parse_inline_spans(mut text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    while !text.is_empty() {
        let next_bold = text.find("**");
        let next_code = text.find('`');
        let next_italic = text.find('*');

        let mut min_idx = None;
        let mut delim_type = "";

        if let Some(idx) = next_bold {
            min_idx = Some(idx);
            delim_type = "**";
        }
        if let Some(idx) = next_code {
            if min_idx.is_none_or(|m| idx < m) {
                min_idx = Some(idx);
                delim_type = "`";
            }
        }
        if let Some(idx) = next_italic {
            if (delim_type != "**" || min_idx != Some(idx)) && min_idx.is_none_or(|m| idx < m) {
                min_idx = Some(idx);
                delim_type = "*";
            }
        }

        if let Some(start_idx) = min_idx {
            if start_idx > 0 {
                spans.push(Span::styled(text[..start_idx].to_string(), base_style));
            }

            let after_delim = &text[start_idx + delim_type.len()..];

            if delim_type == "**" {
                if let Some(end_idx) = after_delim.find("**") {
                    let content = &after_delim[..end_idx];
                    spans.push(Span::styled(
                        content.to_string(),
                        base_style.fg(Color::White).add_modifier(Modifier::BOLD),
                    ));
                    text = &after_delim[end_idx + 2..];
                    continue;
                }
            } else if delim_type == "`" {
                if let Some(end_idx) = after_delim.find('`') {
                    let content = &after_delim[..end_idx];
                    spans.push(Span::styled(
                        content.to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                    text = &after_delim[end_idx + 1..];
                    continue;
                }
            } else if delim_type == "*" {
                if let Some(end_idx) = after_delim.find('*') {
                    let content = &after_delim[..end_idx];
                    spans.push(Span::styled(
                        content.to_string(),
                        base_style.fg(Color::Gray).add_modifier(Modifier::ITALIC),
                    ));
                    text = &after_delim[end_idx + 1..];
                    continue;
                }
            }

            spans.push(Span::styled(text[start_idx..start_idx + delim_type.len()].to_string(), base_style));
            text = after_delim;
        } else {
            spans.push(Span::styled(text.to_string(), base_style));
            break;
        }
    }

    spans
}

/// Renders a single markdown text line with headers, bullets, and inline styles without raw markdown tags
fn render_markdown_line(
    raw_line: &str,
    leading_prefix: Option<Span<'static>>,
    lines: &mut Vec<Line<'static>>,
) {
    let trimmed = raw_line.trim_start();
    let base_style = Style::default().fg(Color::White);

    // Strips ######, #####, ####, ###, ##, # and renders formatted heading without literal markdown hashes
    if let Some(rest) = trimmed.strip_prefix("###### ").or_else(|| trimmed.strip_prefix("######")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::ITALIC)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("##### ").or_else(|| trimmed.strip_prefix("#####")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD | Modifier::ITALIC)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("#### ").or_else(|| trimmed.strip_prefix("####")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("### ").or_else(|| trimmed.strip_prefix("###")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("## ").or_else(|| trimmed.strip_prefix("##")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("# ").or_else(|| trimmed.strip_prefix("#")) {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(rest.trim_start(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("💻 ") {
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.push(Span::styled("💻 ", Style::default().fg(Color::Yellow)));
        let clean_cmd = rest.trim().trim_matches('`');
        spans.push(Span::styled(clean_cmd.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("🌐 ") {
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.push(Span::styled("🌐 ", Style::default().fg(Color::Cyan)));
        let clean_text = rest.trim();
        spans.push(Span::styled(clean_text.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(spans));
        return;
    }

    let mut spans = Vec::new();
    if let Some(p) = leading_prefix {
        spans.push(p);
    }

    if let Some(rest) = trimmed.strip_prefix("- ") {
        spans.push(Span::styled("• ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        spans.extend(parse_inline_spans(rest, base_style));
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        spans.push(Span::styled("• ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        spans.extend(parse_inline_spans(rest, base_style));
    } else {
        let is_numbered = trimmed.len() >= 3
            && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
            && trimmed.chars().nth(1) == Some('.')
            && trimmed.chars().nth(2) == Some(' ');

        if is_numbered {
            let num_prefix = &trimmed[..3];
            let rest = &trimmed[3..];
            spans.push(Span::styled(num_prefix.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            spans.extend(parse_inline_spans(rest, base_style));
        } else if trimmed.starts_with('|') && trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ') {
            // Markdown table separator row
            spans.push(Span::styled(raw_line.to_string(), Style::default().fg(Color::DarkGray)));
        } else {
            spans.extend(parse_inline_spans(raw_line, base_style));
        }
    }

    lines.push(Line::from(spans));
}

/// Renders terminal output with clean subtle indentation
fn render_tool_output_box(output: &str, lines: &mut Vec<Line<'static>>) {
    if output.is_empty() {
        return;
    }

    push_blank_line(lines);
    for l in output.lines() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", l), Style::default().fg(Color::LightCyan)),
        ]));
    }
    push_blank_line(lines);
}

/// Renders passive code snippets, logs, or process trees without noisy text headers
fn render_code_snippet_box(code: &str, tag: &str, lines: &mut Vec<Line<'static>>) {
    if code.is_empty() {
        return;
    }

    let is_generic_text = matches!(
        tag.to_lowercase().as_str(),
        "" | "text" | "txt" | "output" | "result" | "log" | "logs" | "tree"
    );

    push_blank_line(lines);
    if !is_generic_text {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", tag), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]));
    }

    for l in code.lines() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", l), Style::default().fg(Color::LightCyan)),
        ]));
    }
    push_blank_line(lines);
}

/// Renders a command proposal with sleek key pills (consistent with Help/Config modals)
fn render_command_card(card_idx: usize, cmd: &str, _tag: &str, lang: Language, lines: &mut Vec<Line<'static>>) {
    if cmd.is_empty() {
        return;
    }

    let risk = crate::agent::safety::classify_command(cmd);
    let (badge_text, badge_color) = match risk {
        crate::agent::safety::CommandRisk::Safe => ("Safe", Color::Green),
        crate::agent::safety::CommandRisk::Standard => ("Standard", Color::Yellow),
        crate::agent::safety::CommandRisk::Risky => (
            if lang == Language::Fr { "Risqué" } else { "Risky" },
            Color::Red,
        ),
    };

    let mut title_spans = vec![
        Span::styled(format!("⚡ COMMANDE #{} ", card_idx), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ];
    title_spans.extend(key_pill(badge_text, badge_color));

    push_blank_line(lines);
    lines.push(Line::from(title_spans));

    for l in cmd.lines() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", l), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
    }

    let mut footer_spans = vec![Span::raw("  ")];
    footer_spans.extend(key_combo_pills("Alt", &card_idx.to_string(), Color::Cyan));
    footer_spans.push(Span::styled(
        if lang == Language::Fr { " Exécuter" } else { " Run" },
        Style::default().fg(Color::White),
    ));

    lines.push(Line::from(footer_spans));
    push_blank_line(lines);
}

#[allow(dead_code)]
struct ParsedThought {
    thought: Option<String>,
    is_completed: bool,
    response: String,
}

/// Extracts <think>...</think>, <thought>...</thought>, or <reasoning>...</reasoning> reasoning blocks if present.
fn extract_thought_block(text: &str) -> ParsedThought {
    let delimiters = [
        ("<think>", "</think>"),
        ("<thought>", "</thought>"),
        ("<reasoning>", "</reasoning>"),
    ];

    for (open_tag, close_tag) in delimiters {
        if let Some(start) = text.find(open_tag) {
            let before = &text[..start];
            let after_start = &text[start + open_tag.len()..];

            if let Some(end) = after_start.find(close_tag) {
                let thought = after_start[..end].trim().to_string();
                let after_end = after_start[end + close_tag.len()..].trim();
                let remaining = if before.trim().is_empty() {
                    clean_response(after_end)
                } else {
                    clean_response(&format!("{}\n\n{}", before.trim(), after_end))
                };
                let thought_opt = if thought.is_empty() { None } else { Some(thought) };
                return ParsedThought {
                    thought: thought_opt,
                    is_completed: true,
                    response: remaining,
                };
            } else {
                // Still streaming inside thought block
                let thought = after_start.trim().to_string();
                let thought_opt = if thought.is_empty() { None } else { Some(thought) };
                return ParsedThought {
                    thought: thought_opt,
                    is_completed: false,
                    response: clean_response(before.trim()),
                };
            }
        }
    }

    ParsedThought {
        thought: None,
        is_completed: true,
        response: clean_response(text),
    }
}

/// Strips raw ChatML artifacts or raw tool call blocks if returned by local models
fn clean_response(text: &str) -> String {
    let mut cleaned = text.trim();
    if let Some(rest) = cleaned.strip_prefix("assistant\n") {
        cleaned = rest.trim();
    } else if let Some(rest) = cleaned.strip_prefix("assistant") {
        cleaned = rest.trim();
    }

    // Strip tool call blocks (```tool:...```) completely
    let mut result = String::new();
    let mut remaining = cleaned;
    while let Some(start) = remaining.find("```tool:") {
        result.push_str(&remaining[..start]);
        let after = &remaining[start + 8..];
        if let Some(end) = after.find("```") {
            remaining = &after[end + 3..];
        } else {
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);

    result.trim().to_string()
}

fn char_visual_width(c: char) -> usize {
    match c {
        '\t' => 4,
        '\n' | '\r' => 0,
        // Private use area glyphs (Nerd font powerline icons like , )
        '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}' => 1,
        // Zero-width characters (variation selectors, zero-width joiner)
        '\u{fe00}'..='\u{fe0f}' | '\u{200b}'..='\u{200d}' => 0,
        _ => c.width().unwrap_or(1).max(1),
    }
}

fn str_visual_width(s: &str) -> usize {
    s.chars().map(char_visual_width).sum()
}

/// Computes the exact number of visual rendered lines after wrapping at a given width,
/// accurately matching Ratatui's Paragraph word wrapping (Wrap { trim: false }).
fn compute_wrapped_lines_count(lines: &[Line], width: u16) -> u16 {
    if width == 0 {
        return lines.len() as u16;
    }
    let max_w = width as usize;
    let mut total: u16 = 0;

    for line in lines {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if text.is_empty() {
            total = total.saturating_add(1);
            continue;
        }

        // Handle possible internal newlines inside spans
        for subline in text.split('\n') {
            if subline.is_empty() {
                total = total.saturating_add(1);
                continue;
            }

            let mut line_rows: u16 = 1;
            let mut current_col: usize = 0;

            for word in subline.split_inclusive(' ') {
                let trimmed = word.trim_end_matches(' ');
                let word_w = str_visual_width(trimmed);
                let trailing_spaces = word.len() - trimmed.len();

                if word_w == 0 {
                    if current_col + trailing_spaces <= max_w {
                        current_col += trailing_spaces;
                    } else {
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                    continue;
                }

                if current_col + word_w <= max_w {
                    if current_col + word_w + trailing_spaces <= max_w {
                        current_col += word_w + trailing_spaces;
                    } else {
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                } else {
                    if current_col > 0 {
                        line_rows = line_rows.saturating_add(1);
                    }

                    if word_w > max_w {
                        let mut rem = word_w;
                        while rem > max_w {
                            line_rows = line_rows.saturating_add(1);
                            rem -= max_w;
                        }
                        if rem + trailing_spaces <= max_w {
                            current_col = rem + trailing_spaces;
                        } else {
                            line_rows = line_rows.saturating_add(1);
                            current_col = 0;
                        }
                    } else if word_w + trailing_spaces <= max_w {
                        current_col = word_w + trailing_spaces;
                    } else {
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                }
            }

            total = total.saturating_add(line_rows);
        }
    }

    total
}

fn extract_tool_cmd_name(content: &str) -> &str {
    if let Some(start) = content.find('\'') {
        if let Some(end) = content[start + 1..].find('\'') {
            return &content[start + 1..start + 1 + end];
        }
    }
    ""
}

/// Pushes a single blank line only if the previous line is not already blank
fn push_blank_line(lines: &mut Vec<Line<'static>>) {
    if let Some(last) = lines.last() {
        let is_empty = last.spans.is_empty() || last.spans.iter().all(|s| s.content.trim().is_empty());
        if !is_empty {
            lines.push(Line::from(""));
        }
    }
}

/// Accurately computes cursor (row, col) and total wrapped line count matching Ratatui's Paragraph word wrapping
fn compute_prompt_cursor_and_lines(
    text: &str,
    cursor_byte_pos: usize,
    max_w: usize,
) -> (u16, u16, u16) {
    if max_w == 0 || text.is_empty() {
        return (0, 0, 1);
    }

    let mut current_row: u16 = 0;
    let mut current_col: usize = 0;
    let mut cursor_row: u16 = 0;
    let mut cursor_col: usize = 0;
    let mut cursor_found = false;

    let mut byte_offset: usize = 0;

    let sublines: Vec<&str> = text.split('\n').collect();

    for (sub_idx, subline) in sublines.iter().enumerate() {
        if sub_idx > 0 {
            if !cursor_found && byte_offset == cursor_byte_pos {
                cursor_row = current_row;
                cursor_col = current_col;
                cursor_found = true;
            }
            byte_offset += 1; // for '\n'
            current_row = current_row.saturating_add(1);
            current_col = 0;
        }

        if subline.is_empty() {
            if !cursor_found && byte_offset == cursor_byte_pos {
                cursor_row = current_row;
                cursor_col = current_col;
                cursor_found = true;
            }
            continue;
        }

        for word in subline.split_inclusive(' ') {
            let word_bytes = word.len();
            let word_trimmed = word.trim_end_matches(' ');
            let word_w = str_visual_width(word_trimmed);
            let trailing_spaces = word.len() - word_trimmed.len();

            if !cursor_found && cursor_byte_pos >= byte_offset && cursor_byte_pos <= byte_offset + word_bytes {
                let inside_offset = cursor_byte_pos - byte_offset;
                let inside_str = &word[..inside_offset];
                let inside_w = str_visual_width(inside_str);

                if current_col + word_w <= max_w || current_col == 0 {
                    cursor_row = current_row;
                    cursor_col = current_col + inside_w;
                } else {
                    cursor_row = current_row.saturating_add(1);
                    cursor_col = inside_w;
                }
                cursor_found = true;
            }

            if word_w == 0 {
                if current_col + trailing_spaces <= max_w {
                    current_col += trailing_spaces;
                } else {
                    current_row = current_row.saturating_add(1);
                    current_col = 0;
                }
            } else if current_col + word_w <= max_w {
                if current_col + word_w + trailing_spaces <= max_w {
                    current_col += word_w + trailing_spaces;
                } else {
                    current_row = current_row.saturating_add(1);
                    current_col = 0;
                }
            } else {
                if current_col > 0 {
                    current_row = current_row.saturating_add(1);
                }

                if word_w > max_w {
                    let mut rem = word_w;
                    while rem > max_w {
                        current_row = current_row.saturating_add(1);
                        rem -= max_w;
                    }
                    if rem + trailing_spaces <= max_w {
                        current_col = rem + trailing_spaces;
                    } else {
                        current_row = current_row.saturating_add(1);
                        current_col = 0;
                    }
                } else if word_w + trailing_spaces <= max_w {
                    current_col = word_w + trailing_spaces;
                } else {
                    current_row = current_row.saturating_add(1);
                    current_col = 0;
                }
            }

            byte_offset += word_bytes;
        }
    }

    if !cursor_found {
        cursor_row = current_row;
        cursor_col = current_col;
    }

    let total_rows = current_row.saturating_add(1);
    (cursor_row, cursor_col as u16, total_rows)
}
