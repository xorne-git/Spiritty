use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthChar;

use crate::{
    app::{App, Focus, MessageRole},
    i18n::{I18nKey, Language},
    ui::get_spinner_char,
};

pub struct ChatPanel<'a> {
    app: &'a App,
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
        let palette = self.app.theme.palette();
        let is_focused = self.app.focus == Focus::Chat;
        // (spinner glyph resolved inside compose_assistant_message from the frame)
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
                .fg(if is_focused {
                    palette.accent_primary
                } else {
                    palette.border_unfocused
                })
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

        // 4. Render Messages History through the per-message render cache.
        //
        // Two-pass design (AGENTS.md: "Limiter les allocations inutiles dans la
        // boucle d'événement"): pass A validates cached compositions and
        // rebuilds only what actually changed; pass B clones just the visible
        // row window into the widget. Steady-state frame cost is O(visible
        // rows), not O(chat history).
        let generating = self.app.agent.is_generating;
        let has_pending_approval = self.app.pending_tool_approval.is_some();
        let total_messages = self.app.messages.len();
        let gen_key = crate::app::ChatGenKey {
            width: messages_area.width,
            flags: ((u8::from(lang == Language::Fr)) << 1) | u8::from(self.app.debug),
            theme: palette.id as u8,
        };

        // Elapsed since the model started the current turn — the live "deep diving"
        // reflection timer ("temps total de réflexion depuis la dernière interaction").
        let think_elapsed = if generating {
            self.app.generation_start_time.map(|t| t.elapsed())
        } else {
            None
        };

        // ---- Pass A: fill cache misses / invalidate stale entries -------------
        let mut history_rows_total: u16 = 0;
        let mut content_top: u16 = 0;
        self.app.chat_thought_hits.borrow_mut().clear();
        {
            let mut cache = self.app.chat_render_cache.borrow_mut();
            cache.begin_frame(gen_key, total_messages);

            for (idx, msg) in self.app.messages.iter().enumerate() {
                let is_last = idx + 1 == total_messages;
                let role_tag = match msg.role {
                    MessageRole::System => 0u8,
                    MessageRole::User => 1,
                    MessageRole::Assistant => 2,
                };
                let byte_len = msg.content.len();

                // Filter parity with the former inline loop: raw `[RÉSULTAT…]`
                // dumps stay hidden outside debug mode, whatever their role.
                let skipped = !self.app.debug && msg.content.starts_with("[RÉSULTAT");
                let is_tail_assistant = is_last && role_tag == 2;
                let want_expanded = self.app.expanded_thought == Some(idx);

                let cached_fresh = cache
                    .entry(idx)
                    .map(|e| {
                        e.generation == gen_key
                            && e.role_tag == role_tag
                            && e.byte_len == byte_len
                            && e.expanded == want_expanded
                            && !(is_tail_assistant
                                && (generating || e.pending_flag != has_pending_approval))
                    })
                    .unwrap_or(false);

                let rows_now: u16;
                if !cached_fresh {
                    let lines_built: Vec<Line<'static>> = if skipped {
                        Vec::new()
                    } else {
                        compose_single_message(
                            &msg.role,
                            &msg.content,
                            messages_area.width,
                            is_last,
                            generating,
                            has_pending_approval,
                            self.app
                                .pending_tool_approval
                                .as_ref()
                                .map(|p| p.command.as_str()),
                            self.app.spinner_frame,
                            think_elapsed,
                            want_expanded,
                            lang,
                            &palette,
                        )
                    };
                    // Authoritative wrapped row count from ratatui itself (the SIMULATED
                    // compute_wrapped_lines_count undercounted on tables/heredocs/wide
                    // glyphs, which both broke max_scroll and would break click hit-testing).
                    rows_now = Paragraph::new(lines_built.clone())
                        .wrap(Wrap { trim: false })
                        .line_count(messages_area.width) as u16;

                    // Always store, including the streaming tail: pass B clones
                    // widget input FROM the cache, so a missing tail entry would
                    // render as blank rows while the scroll counter still counts
                    // them ("scrolling into empty space" bug). Staleness is not a
                    // concern — `cached_fresh` above already recomposes the live
                    // tail on every frame while generating.
                    let slot = cache.slot_mut(idx);
                    *slot = Some(crate::app::ChatCacheEntry {
                        generation: gen_key,
                        role_tag,
                        byte_len,
                        pending_flag: has_pending_approval,
                        expanded: want_expanded,
                        rows: rows_now,
                        lines: lines_built,
                    });
                } else {
                    rows_now = cache.entry(idx).map(|e| e.rows).unwrap_or(0);
                }

                // Click target for expand/collapse: the "Think · " toggle line is the
                // FIRST row of the message content (assistant messages with reasoning).
                if role_tag == 2 && !skipped && has_visible_thought(&msg.content) {
                    self.app.chat_thought_hits.borrow_mut().push((
                        idx,
                        content_top,
                        content_top.saturating_add(1),
                    ));
                }

                content_top = content_top.saturating_add(rows_now);
                history_rows_total = history_rows_total.saturating_add(rows_now);
            }
        }

        // ---- Dynamic segment: pending permission card (kept out of the cache --
        // because its lifetime, footer pills and badge react to live consent state).
        let approval_lines: Vec<Line<'static>> = self
            .app
            .pending_tool_approval
            .as_ref()
            .map_or_else(Vec::new, |pending| {
                compose_approval_card(&pending.command, lang)
            });
        let approval_rows = compute_wrapped_lines_count(&approval_lines, messages_area.width);

        let visible_height = messages_area.height;

        // ---- Pass B: assemble the widget input from cached compositions -------
        //
        // Note: a finer-grained "clone only the visible row window" was tried
        // and rejected — logical lines span multiple *visual* rows once wrapped,
        // and mid-window slicing cannot reproduce Paragraph's context-dependent
        // word wrapping for a truncated line head. Legacy displayed exactly this
        // full-sequence strategy, so parity wins; the real win stays in pass A
        // (no markdown re-parse / re-wrap / re-allocation per frame).
        let mut visible_lines: Vec<Line<'static>> =
            Vec::with_capacity((history_rows_total + approval_rows) as usize);
        {
            let cache = self.app.chat_render_cache.borrow();
            for idx in 0..total_messages {
                if let Some(seg) = cache.entry_lines(idx) {
                    visible_lines.extend(seg.iter().cloned());
                }
            }
        }
        visible_lines.extend(approval_lines);

        // Authoritative visual height: let ratatui itself count the wrapped rows via
        // the SAME WordWrapper that `render` uses. The former manual sum of per-message
        // `compute_wrapped_lines_count` rows used a *simulated* wrap algorithm that
        // diverged from ratatui's real one on markdown tables / heredoc cards / wide
        // glyphs — each message added its own (under)counting error to `max_scroll`,
        // so the bottom rows of the conversation became UNREACHABLE by scrolling and
        // the tail of responses drifted out of view (user report: "le décalage
        // s'accentuait, je ne vois plus la fin des réponses", y compris au relancement
        // avec -c). One Paragraph serves both counting and rendering: zero extra clone.
        let messages_paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
        let content_visual_lines = messages_paragraph.line_count(messages_area.width) as u16;

        let max_scroll = content_visual_lines.saturating_sub(visible_height);
        let scroll_from_bottom = self.app.chat_scroll_from_bottom.min(max_scroll);
        let scroll_offset = max_scroll.saturating_sub(scroll_from_bottom);
        *self.app.chat_messages_geo.borrow_mut() = (messages_area, scroll_offset);

        messages_paragraph
            .scroll((scroll_offset, 0))
            .render(messages_area, buf);

        // Render scroll indicator badge on the top border (liseret)
        if area.width > 25 {
            let (badge_text, badge_style) = if scroll_from_bottom > 0 {
                (
                    format!(" ▲ -{} / {} l. ", scroll_from_bottom, content_visual_lines),
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    format!(" 📜 {} l. ", content_visual_lines),
                    Style::default().fg(if is_focused {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                )
            };

            let badge_len = unicode_width::UnicodeWidthStr::width(badge_text.as_str()) as u16;
            let badge_x = area.right().saturating_sub(badge_len + 1);
            let badge_y = area.top();
            buf.set_string(badge_x, badge_y, &badge_text, badge_style);
        }

        // 5. Render horizontal prompt divider liseret above input zone (1px centered line, 1 space margin at right)
        let prompt_div_style = Style::default().fg(Color::Rgb(40, 55, 75));
        for x in area.left()..area.right().saturating_sub(1) {
            buf.set_string(x, prompt_sep_y, "─", prompt_div_style);
        }

        // Render Search Bar Overlay if Ctrl+F is active
        if self.app.chat_search_active {
            let matches = self.app.find_search_matches();
            let match_text = if matches.is_empty() {
                if self.app.chat_search_query.is_empty() {
                    " (0) ".to_string()
                } else {
                    " (0 résultat) ".to_string()
                }
            } else {
                format!(
                    " ({}/{}) ",
                    self.app.chat_search_match_idx + 1,
                    matches.len()
                )
            };

            let search_line = Line::from(vec![
                Span::styled(" 🔍 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    &self.app.chat_search_query,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("▌", Style::default().fg(Color::Yellow)),
                Span::styled(
                    match_text,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [Enter: Suiv | Shift+Enter: Préc | Esc: Fermer] ",
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            buf.set_line(
                area.left() + 1,
                prompt_sep_y,
                &search_line,
                area.width.saturating_sub(2),
            );
        }

        // 6. Render Vertical Accent Bar (using left half block ▌)
        let bar_style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        for y in input_area.top()..input_area.bottom() {
            buf.set_string(area.left(), y, "▌", bar_style);
        }

        let input_scroll = cursor_row.saturating_sub(needed_input_height.saturating_sub(1));

        if self.app.chat_input.is_empty() {
            let (placeholder_line, alignment) = if self.app.pending_tool_approval.is_some() {
                // A tool is awaiting approval — invite the user to answer (no "Esc / Stop" here,
                // the input keeps focus so they can just type oui / non).
                (
                    Line::from(vec![
                        Span::styled(
                            format!("{} ", get_spinner_char(self.app.spinner_frame)),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            if lang == Language::Fr {
                                "Demande d'autorisation — tapez oui / non puis [Enter]"
                            } else {
                                "Permission request — type yes / no then [Enter]"
                            },
                            Style::default().fg(Color::Yellow),
                        ),
                    ]),
                    Alignment::Center,
                )
            } else if self.app.agent.is_generating {
                (
                    Line::from(vec![
                        Span::styled(
                            format!("{} ", get_spinner_char(self.app.spinner_frame)),
                            Style::default()
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "[Esc]",
                            Style::default()
                                .bg(Color::Rgb(65, 25, 25))
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            if lang == Language::Fr {
                                " Arrêter"
                            } else {
                                " Stop"
                            },
                            Style::default().fg(Color::LightRed),
                        ),
                    ]),
                    Alignment::Center,
                )
            } else if self.app.messages.is_empty() {
                (
                    Line::from(vec![Span::styled(
                        lang.t(I18nKey::ChatInputPlaceholder),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )]),
                    Alignment::Left,
                )
            } else {
                (Line::from(""), Alignment::Left)
            };
            let p = Paragraph::new(placeholder_line)
                .alignment(alignment)
                .wrap(Wrap { trim: false });
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

/// Composes the rendered lines of ONE System message (including its trailing
/// blank separator). Extracted verbatim from the former build-everything draw
/// loop so the render cache can fill misses per message.
fn compose_system_message(
    content: &str,
    palette: &crate::ui::theme::ThemePalette,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for l in content.lines() {
        lines.push(Line::from(vec![Span::styled(
            l.to_string(),
            Style::default().fg(palette.text_dim),
        )]));
    }
    push_blank_line(&mut lines);
    lines
}

/// Composes the rendered lines of ONE User message (tool result / refusal /
/// command echo / plain prompt variants included, trailing blank separator).
fn compose_user_message(
    content: &str,
    panel_width: u16,
    lang: Language,
    palette: &crate::ui::theme::ThemePalette,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if content.starts_with("[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '") {
        let cmd_name = extract_tool_cmd_name(content);
        lines.push(Line::from(vec![
            Span::styled("💻 ", Style::default().fg(palette.warning)),
            Span::styled(
                cmd_name.to_string(),
                Style::default()
                    .fg(palette.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                "✓ Exécution silencieuse",
                Style::default().fg(palette.success),
            ),
        ]));
    } else if content.starts_with("[L'utilisateur a refusé l'exécution") {
        lines.push(Line::from(vec![
            Span::styled("⚠️ ", Style::default().fg(palette.warning)),
            Span::styled(
                if lang == Language::Fr {
                    "Exécution refusée par l'utilisateur"
                } else {
                    "Execution declined by user"
                },
                Style::default()
                    .fg(palette.text_dim)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    } else if content.starts_with("💻 ") {
        let cmd_text = content.strip_prefix("💻 ").unwrap_or(content).trim();
        let clean_cmd = cmd_text.trim_matches('`');
        for (l_idx, line) in clean_cmd.lines().enumerate() {
            if l_idx == 0 {
                lines.push(Line::from(vec![
                    Span::styled("💻 ", Style::default().fg(palette.warning)),
                    Span::styled(
                        line.to_string(),
                        Style::default()
                            .fg(palette.warning)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        line.to_string(),
                        Style::default()
                            .fg(palette.warning)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }
    } else {
        render_user_message_block(content, &mut lines, panel_width, palette);
    }

    push_blank_line(&mut lines);
    lines
}

/// Composes the rendered lines of ONE Assistant message (thought blocks, ghost
/// prefix reacting to live state, markdown/cards; trailing blank separator).
///
/// Uses no palette: this branch bakes literal DarkGray/Cyan colors only, which
/// is why theme switches never need to touch these cache entries' contents.
/// Cyan gradient shimmer for the "Deep thinking" label: a wave of light sweeps
/// across the wording as `frame` advances (one step per spinner tick), from
/// dim slate to bright cyan and back. Grouped into as few spans as possible.
fn deep_thinking_shimmer_spans(wording: &str, frame: usize) -> Vec<Span<'static>> {
    /// Dim → bright cyan ramp (index 0 = resting color).
    const RAMP: [Color; 6] = [
        Color::DarkGray,
        Color::Rgb(18, 68, 78),
        Color::Rgb(26, 105, 120),
        Color::Rgb(58, 165, 180),
        Color::Cyan,
        Color::LightCyan,
    ];
    /// Number of characters lit around the wave head.
    const WAVE_WIDTH: i32 = 8;

    let chars: Vec<char> = wording.chars().collect();
    let n = chars.len() as i32;
    // The wave enters from the left, sweeps through, exits right, pauses.
    let cycle = n + WAVE_WIDTH * 2;
    let head = (frame % cycle as usize) as i32 - WAVE_WIDTH;

    let shade = |d: i32| -> Color {
        // d = distance to the wave head: 0 → brightest, ≥ WAVE_WIDTH → resting.
        let steps = RAMP.len() as i32 - 1;
        let idx = steps - (d * steps / WAVE_WIDTH).clamp(0, steps);
        RAMP[idx as usize]
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cur = String::new();
    let mut cur_color: Option<Color> = None;
    for (i, ch) in chars.iter().enumerate() {
        let color = shade((head - i as i32).abs());
        if cur_color == Some(color) {
            cur.push(*ch);
        } else {
            if !cur.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut cur),
                    Style::default()
                        .fg(cur_color.unwrap())
                        .add_modifier(Modifier::ITALIC),
                ));
            }
            cur.push(*ch);
            cur_color = Some(color);
        }
    }
    if !cur.is_empty() {
        spans.push(Span::styled(
            cur,
            Style::default()
                .fg(cur_color.unwrap())
                .add_modifier(Modifier::ITALIC),
        ));
    }
    spans
}

/// Collapses multi-line reasoning into ONE visual line, keeping the END (most recent
/// thought) and truncating with a leading ellipsis when it exceeds `max_w` visual
/// columns. This is the single-line "Think · …" view requested by the user so the
/// streaming thought no longer floods the response area.
fn collapse_thought_to_single_line(text: &str, max_w: usize) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if str_visual_width(&one_line) <= max_w {
        return one_line;
    }

    // Walk from the END, accumulating glyphs until we just fit max_w - 1 (leaving
    // room for the leading "…"), then prepend the ellipsis.
    let mut tail = String::new();
    let mut w = 0usize;
    for ch in one_line.chars().rev() {
        let cw = char_visual_width(ch);
        if w + cw + 1 > max_w {
            break;
        }
        tail.insert(0, ch);
        w += cw;
    }
    format!("…{}", tail)
}

/// Formats a duration as `m:ss` (e.g. `8m 29s`) for the "Deep diving..." reflection timer.
fn format_elapsed_min_sec(dur: std::time::Duration) -> String {
    let total = dur.as_secs();
    format!("{}m {:02}s", total / 60, total % 60)
}

#[allow(clippy::too_many_arguments)]
fn compose_assistant_message(
    content: &str,
    is_last: bool,
    is_generating: bool,
    has_pending_approval: bool,
    pending_command: Option<&str>,
    spinner_frame: usize,
    think_elapsed: Option<std::time::Duration>,
    panel_width: u16,
    thought_expanded: bool,
    lang: Language,
) -> Vec<Line<'static>> {
    let spinner_char = get_spinner_char(spinner_frame);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut card_counter: usize = 0;
    let parsed = extract_thought_block(content);

    let ghost_prefix = if is_generating && is_last {
        if has_pending_approval {
            "👻 ".to_string()
        } else {
            format!("{} 👻 ", spinner_char)
        }
    } else {
        "👻 ".to_string()
    };

    let has_valid_thought = parsed
        .thought
        .as_ref()
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false);

    if has_valid_thought {
        let thought = parsed.thought.as_ref().unwrap();
        let marker = if thought_expanded { "▾ " } else { "▸ " };
        let label = if lang == Language::Fr {
            "💭 Réflexion · "
        } else {
            "💭 Think · "
        };
        // The marker is the click target (expand/collapse). Rendered in cyan to hint
        // interactivity; ▸ = collapsed (expand), ▾ = expanded.
        lines.push(Line::from(vec![
            Span::styled(
                marker,
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                label,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ),
        ]));

        if thought_expanded {
            // Full reasoning block (click ▾ to collapse back to one line).
            for l in thought.lines() {
                let trimmed = l.trim();
                if trimmed.starts_with("```") {
                    continue;
                }
                if l.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(vec![Span::styled(
                        format!("  {}", l),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )]));
                }
            }
        } else {
            // Single-line view: collapse whitespace and show only the END of the
            // reasoning, riding on the SAME visual row as the marker/label.
            let prefix_w = str_visual_width(marker) + str_visual_width(label);
            let max_w = (panel_width as usize).saturating_sub(prefix_w + 2).max(10);
            let one_line = collapse_thought_to_single_line(thought, max_w);
            let style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
            lines[0].spans.push(Span::styled(one_line, style));
        }
        push_blank_line(&mut lines);

        // While the model is STILL reasoning (no response tokens yet), keep the
        // animated "Deep thinking…" status line with the live reflection timer
        // right below the thought line (user request: « garder la ligne deep
        // thinking en dessous avec le timer pendant le déroulé du thinking »).
        if is_generating && is_last && !has_pending_approval && parsed.response.trim().is_empty() {
            let wording = if lang == Language::Fr {
                "💭 Réflexion profonde…"
            } else {
                "💭 Deep thinking…"
            };
            let timer_suffix = think_elapsed
                .map(|e| format!(" {}", format_elapsed_min_sec(e)))
                .unwrap_or_default();
            let mut spans = deep_thinking_shimmer_spans(wording, spinner_frame);
            if !timer_suffix.is_empty() {
                spans.push(Span::styled(
                    timer_suffix,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(spans));
            push_blank_line(&mut lines);
        }

        if !parsed.response.is_empty() {
            render_markdown_blocks(
                &parsed.response,
                lang,
                &mut lines,
                Some(&ghost_prefix),
                &mut card_counter,
                pending_command,
            );
        }
    } else if !parsed.response.is_empty() {
        render_markdown_blocks(
            &parsed.response,
            lang,
            &mut lines,
            Some(&ghost_prefix),
            &mut card_counter,
            pending_command,
        );
    } else if is_generating && is_last && !has_pending_approval {
        // Long silent thinking phase (no tokens received yet): show the loader
        // with an explicit label and a cyan gradient shimmer sweeping the text,
        // so the user sees at a glance that the model is actively working. The
        // reflection timer rides on the SAME line, after the wording.
        let wording = if lang == Language::Fr {
            " 💭 Réflexion profonde…"
        } else {
            " 💭 Deep thinking…"
        };
        let timer_suffix = think_elapsed
            .map(|e| format!(" {}", format_elapsed_min_sec(e)))
            .unwrap_or_default();
        let mut spans = vec![Span::styled(
            ghost_prefix,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        spans.extend(deep_thinking_shimmer_spans(wording, spinner_frame));
        if !timer_suffix.is_empty() {
            spans.push(Span::styled(
                timer_suffix,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(spans));
    }

    push_blank_line(&mut lines);
    lines
}

/// Dispatches one chat message to its role composer (pure; no App access).
#[allow(clippy::too_many_arguments)]
fn compose_single_message(
    role: &MessageRole,
    content: &str,
    panel_width: u16,
    is_last: bool,
    is_generating: bool,
    has_pending_approval: bool,
    pending_command: Option<&str>,
    spinner_frame: usize,
    think_elapsed: Option<std::time::Duration>,
    thought_expanded: bool,
    lang: Language,
    palette: &crate::ui::theme::ThemePalette,
) -> Vec<Line<'static>> {
    match role {
        MessageRole::System => compose_system_message(content, palette),
        MessageRole::User => compose_user_message(content, panel_width, lang, palette),
        MessageRole::Assistant => compose_assistant_message(
            content,
            is_last,
            is_generating,
            has_pending_approval,
            pending_command,
            spinner_frame,
            think_elapsed,
            panel_width,
            thought_expanded,
            lang,
        ),
    }
}

/// Composes the ⚡ permission-request card shown above the input while an
/// execution awaits consent (dynamic segment, never part of the cache).
fn compose_approval_card(command: &str, lang: Language) -> Vec<Line<'static>> {
    // The badge comes from the single source of truth (`safety::classify_command`):
    // ad-hoc substring heuristics used to label `kill -9` / `chmod` / systemctl
    // restarts as a green "Safe" exactly when the user had to consent.
    let (badge_text, badge_color) = match crate::agent::safety::classify_command(command) {
        crate::agent::safety::CommandRisk::Safe => ("Safe", Color::Green),
        crate::agent::safety::CommandRisk::Standard => ("Standard", Color::Yellow),
        // Elevated-but-benign (sudo cat / ls / certbot certificates…): violet so it
        // never masquerades as green-Safe nor as red-destructive.
        crate::agent::safety::CommandRisk::Sudo => ("Sudo", Color::LightMagenta),
        crate::agent::safety::CommandRisk::Risky => (
            if lang == Language::Fr {
                "Risqué"
            } else {
                "Risky"
            },
            Color::Red,
        ),
    };

    let req_title = if lang == Language::Fr {
        "⚡ DEMANDE D'AUTORISATION "
    } else {
        "⚡ PERMISSION REQUEST "
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut title_spans = vec![Span::styled(
        req_title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )];
    title_spans.extend(key_pill(badge_text, badge_color));

    push_blank_line(&mut lines);
    lines.push(Line::from(title_spans));

    for l in command.lines() {
        if l.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", l),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
    }

    let mut footer_spans = vec![Span::raw("  ")];
    // Deliberately NOT "↵ Enter Autoriser": an accidental bare Enter must never
    // execute the pending command — approval requires a confirmation word or F10.
    footer_spans.extend(key_pill("F10", Color::Green));
    footer_spans.push(Span::styled(
        if lang == Language::Fr {
            " Autoriser · "
        } else {
            " Approve · "
        },
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    footer_spans.extend(key_pill(
        if lang == Language::Fr {
            "oui/ok + ↵"
        } else {
            "yes/ok + ↵"
        },
        Color::Green,
    ));
    footer_spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
    footer_spans.extend(key_pill("Esc", Color::Red));
    footer_spans.push(Span::styled(
        if lang == Language::Fr {
            " Refuser"
        } else {
            " Decline"
        },
        Style::default().fg(Color::White),
    ));

    lines.push(Line::from(footer_spans));
    push_blank_line(&mut lines);
    lines
}

/// Renders markdown response text and turns ```bash ... ``` code blocks into interactive Command Proposals.
/// `suppress_command`: when a tool consent is pending for this exact command, the matching
/// fence is demoted to an inert snippet — the approval card right below it stays the single
/// actionable surface (Alt+N on the duplicate would even bypass the pending authorization).
fn render_markdown_blocks(
    text: &str,
    lang: Language,
    lines: &mut Vec<Line<'static>>,
    mut leading_prefix: Option<&str>,
    card_counter: &mut usize,
    suppress_command: Option<&str>,
) {
    let repaired = crate::app::repair_prematurely_closed_code_blocks(text);
    let mut remaining = repaired.as_str();

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
                let is_pending_dup = suppress_command
                    .map(|pending| pending.trim() == code_content.trim())
                    .unwrap_or(false);
                if is_pending_dup {
                    // The same command is awaiting consent in the approval card
                    // right below: keep the code visible but demote it to an inert
                    // snippet so the command has exactly ONE actionable surface.
                    render_code_snippet_box(code_content, fence_tag, lines);
                } else {
                    render_command_card(*card_counter, code_content, fence_tag, lang, lines);
                }
            } else {
                render_code_snippet_box(code_content, fence_tag, lines);
            }
            remaining = &code_rest[end_idx + 3..];
        } else {
            // Streaming inside open code block (still incomplete):
            // Render as code snippet box while streaming; only promote to interactive Command Card (with Alt 1) once the block is closed!
            let code_content = code_rest.trim();
            if fence_tag.starts_with("tool:") {
                // Internal tool call block: skip rendering while streaming
            } else if fence_tag == "output" || fence_tag == "result" {
                render_tool_output_box(code_content, lines);
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

/// Renders user messages with vertical accent bar ▌ on every line and themed background
fn render_user_message_block(
    content: &str,
    lines: &mut Vec<Line<'static>>,
    width: u16,
    palette: &crate::ui::ThemePalette,
) {
    let user_bg = palette.selection_bg;
    let bar_style = Style::default()
        .fg(palette.accent_primary)
        .bg(user_bg)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(palette.text_primary).bg(user_bg);

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

            // Hard-wrap a single over-wide token (URL / hash) so it can't overflow the panel.
            if word_w > max_text_width {
                if current_width > 0 {
                    let pad_len = target_width.saturating_sub(2 + current_width);
                    let mut spans = vec![Span::styled("▌ ", bar_style)];
                    spans.extend(parse_inline_spans(&current_line, text_style));
                    if pad_len > 0 {
                        spans.push(Span::styled(" ".repeat(pad_len), text_style));
                    }
                    lines.push(Line::from(spans));
                    current_line.clear();
                    current_width = 0;
                }
                for chunk in wrap_wide_word(word, max_text_width) {
                    let chunk_w = chunk.chars().map(|c| c.width().unwrap_or(1)).sum::<usize>();
                    let pad_len = target_width.saturating_sub(2 + chunk_w);
                    let mut spans = vec![Span::styled("▌ ", bar_style)];
                    spans.extend(parse_inline_spans(&chunk, text_style));
                    if pad_len > 0 {
                        spans.push(Span::styled(" ".repeat(pad_len), text_style));
                    }
                    lines.push(Line::from(spans));
                }
                continue;
            }

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

/// Splits a single word into chunks no wider than `max_w` display columns.
fn wrap_wide_word(word: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![word.to_string()];
    }
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for c in word.chars() {
        let cw = c.width().unwrap_or(0).max(1);
        if cur_w + cw > max_w && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(c);
        cur_w += cw;
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

/// Renders a sequence of markdown lines with automatic grouping of markdown tables
fn render_text_lines(text: &str, lines: &mut Vec<Line<'static>>, mut leading_prefix: Option<&str>) {
    let mut current_table_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Check if line looks like a table row (starts and contains pipe '|')
        if (trimmed.starts_with('|')
            || trimmed.starts_with("├─")
            || trimmed.starts_with("┌─")
            || trimmed.starts_with("└─"))
            && trimmed.contains('|')
        {
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
        if (trimmed.starts_with("---") || trimmed.starts_with("***"))
            && trimmed.chars().all(|c| c == '-' || c == '*' || c == ' ')
        {
            lines.push(Line::from(vec![Span::styled(
                "  ──────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )]));
            push_blank_line(lines);
            continue;
        }

        // Regular line
        let p_span = leading_prefix.take().map(|p| {
            Span::styled(
                p.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
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
        if inner
            .chars()
            .all(|c| c == '-' || c == '|' || c == ':' || c == ' ')
        {
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

    let max_col_w = 30usize;
    let mut col_widths = vec![0usize; num_cols];
    for row in &rows {
        for (col_idx, cell) in row.iter().enumerate() {
            let w = visual_cell_width(cell).min(max_col_w);
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
            let (truncated_cell, cell_w) = truncate_table_cell(cell, w);
            let pad_right = w.saturating_sub(cell_w);

            let cell_style = if is_header {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            spans.extend(parse_inline_spans(&truncated_cell, cell_style));
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

fn truncate_table_cell(text: &str, max_w: usize) -> (String, usize) {
    let w = visual_cell_width(text);
    if w <= max_w {
        return (text.to_string(), w);
    }
    let mut out = String::new();
    let mut cur_w = 0;
    for c in text.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        if cur_w + cw + 1 > max_w {
            break;
        }
        out.push(c);
        cur_w += cw;
    }
    out.push('…');
    (out, cur_w + 1)
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
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
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

            spans.push(Span::styled(
                text[start_idx..start_idx + delim_type.len()].to_string(),
                base_style,
            ));
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
    if let Some(rest) = trimmed
        .strip_prefix("###### ")
        .or_else(|| trimmed.strip_prefix("######"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::ITALIC),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed
        .strip_prefix("##### ")
        .or_else(|| trimmed.strip_prefix("#####"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed
        .strip_prefix("#### ")
        .or_else(|| trimmed.strip_prefix("####"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed
        .strip_prefix("### ")
        .or_else(|| trimmed.strip_prefix("###"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed
        .strip_prefix("## ")
        .or_else(|| trimmed.strip_prefix("##"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed
        .strip_prefix("# ")
        .or_else(|| trimmed.strip_prefix("#"))
    {
        push_blank_line(lines);
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.extend(parse_inline_spans(
            rest.trim_start(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("💻 ") {
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.push(Span::styled("💻 ", Style::default().fg(Color::Yellow)));
        let clean_cmd = rest.trim().trim_matches('`');
        spans.push(Span::styled(
            clean_cmd.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(spans));
        return;
    } else if let Some(rest) = trimmed.strip_prefix("🌐 ") {
        let mut spans = Vec::new();
        if let Some(p) = leading_prefix {
            spans.push(p);
        }
        spans.push(Span::styled("🌐 ", Style::default().fg(Color::Cyan)));
        let clean_text = rest.trim();
        spans.push(Span::styled(
            clean_text.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(spans));
        return;
    }

    let mut spans = Vec::new();
    if let Some(p) = leading_prefix {
        spans.push(p);
    }

    if let Some(rest) = trimmed.strip_prefix("- ") {
        spans.push(Span::styled(
            "• ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.extend(parse_inline_spans(rest, base_style));
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        spans.push(Span::styled(
            "• ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.extend(parse_inline_spans(rest, base_style));
    } else {
        let is_numbered = trimmed.len() >= 3
            && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
            && trimmed.chars().nth(1) == Some('.')
            && trimmed.chars().nth(2) == Some(' ');

        if is_numbered {
            let num_prefix = &trimmed[..3];
            let rest = &trimmed[3..];
            spans.push(Span::styled(
                num_prefix.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.extend(parse_inline_spans(rest, base_style));
        } else if trimmed.starts_with('|')
            && trimmed
                .chars()
                .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
        {
            // Markdown table separator row
            spans.push(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
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
        if l.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", l),
                Style::default().fg(Color::LightCyan),
            )]));
        }
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
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", tag),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
    }

    for l in code.lines() {
        if l.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", l),
                Style::default().fg(Color::LightCyan),
            )]));
        }
    }
    push_blank_line(lines);
}

/// Renders a command proposal with sleek key pills (consistent with Help/Config modals)
fn render_command_card(
    card_idx: usize,
    cmd: &str,
    _tag: &str,
    lang: Language,
    lines: &mut Vec<Line<'static>>,
) {
    if cmd.is_empty() {
        return;
    }

    let risk = crate::agent::safety::classify_command(cmd);
    let (badge_text, badge_color) = match risk {
        crate::agent::safety::CommandRisk::Safe => ("Safe", Color::Green),
        crate::agent::safety::CommandRisk::Standard => ("Standard", Color::Yellow),
        // Violet for elevated-but-benign, consistent with the permission card badge.
        crate::agent::safety::CommandRisk::Sudo => ("Sudo", Color::LightMagenta),
        crate::agent::safety::CommandRisk::Risky => (
            if lang == Language::Fr {
                "Risqué"
            } else {
                "Risky"
            },
            Color::Red,
        ),
    };

    let mut title_spans = vec![Span::styled(
        format!("⚡ COMMANDE #{} ", card_idx),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];
    title_spans.extend(key_pill(badge_text, badge_color));

    push_blank_line(lines);
    lines.push(Line::from(title_spans));

    for l in cmd.lines() {
        if l.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", l),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
    }

    let mut footer_spans = vec![Span::raw("  ")];
    footer_spans.extend(key_combo_pills("Alt", &card_idx.to_string(), Color::Cyan));
    footer_spans.push(Span::styled(
        if lang == Language::Fr {
            " Exécuter"
        } else {
            " Run"
        },
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
                let thought_opt = if thought.is_empty() {
                    None
                } else {
                    Some(thought)
                };
                return ParsedThought {
                    thought: thought_opt,
                    is_completed: true,
                    response: remaining,
                };
            } else {
                // Still streaming inside thought block
                let thought = after_start.trim().to_string();
                let thought_opt = if thought.is_empty() {
                    None
                } else {
                    Some(thought)
                };
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

/// True when a message carries reasoning with non-empty content (its "Think · "
/// toggle line is rendered and clickable).
fn has_visible_thought(content: &str) -> bool {
    extract_thought_block(content)
        .thought
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false)
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

    // Strip raw XML-like tool-call blocks some models emit inline in the visible
    // stream instead of via the tool protocol (<tool_calls>/<DSML>/<invoke>/<parameter>).
    strip_tool_call_xml(&result).trim().to_string()
}

/// Removes raw XML-like tool-call blocks leaked into the visible stream. Handles
/// both the standard form (`<tool_calls>…<invoke …>…</invoke></tool_calls>`) and the
/// hybrid/mangled delimiters some models emit — fullwidth-pipe tags without angle
/// brackets (`｜DSML｜｜tool_calls>` with U+FF5C pipes), and `<|DSML|>`-style opens.
/// A marker without any recognised closing tag (a stream cut mid-block) drops the
/// rest of the string; text before the block (and any rightmost trailing text) is kept.
fn strip_tool_call_xml(text: &str) -> String {
    // Opening markers: ASCII XML opens plus fullwidth-pipe / `<|…|>` variants.
    const OPENERS: &[&str] = &[
        "<tool_calls",
        "<DSML",
        "<invoke",
        "<parameter",
        "\u{ff5c}DSML",
        "\u{ff5c}tool_calls",
        "\u{ff5c}invoke",
        "\u{ff5c}parameter",
        "<|tool_calls",
        "<|DSML",
        "<|invoke",
        "<|parameter",
    ];
    // Outermost closing tag preferred (cuts after it, preserving trailing text).
    const CLOSES: &[&str] = &["</tool_calls>", "</DSML>", "</invoke>", "</parameter>"];

    // Normalize the hybrid GLM/Z.ai DSML scaffolding first: every variant maps to
    // clean tags. A marker WITH a preceding bracket dissolves (`<｜｜DSML｜｜invoke …>`
    // → `<invoke …>`); a bracket-LESS marker BECOMES the bracket (`｜DSML｜｜tool_calls>`
    // → `<|tool_calls>`, matched by the `<|…` openers below). Without this, the
    // `\u{ff5c}DSML` opener only matched at the second fullwidth pipe and left a
    // stray `<｜` residue on screen.
    let normalized;
    let text = if text.contains('\u{ff5c}') {
        normalized = text
            .replace('\u{ff5c}', "|")
            .replace("<||DSML||", "<")
            .replace("</||DSML||", "</")
            .replace("||DSML||", "<")
            .replace("<|DSML|", "<")
            .replace("</|DSML|", "</")
            .replace("|DSML|", "<");
        normalized.as_str()
    } else {
        text
    };

    let mut out = String::new();
    let mut remaining = text;
    loop {
        let mut best: Option<usize> = None;
        for o in OPENERS {
            if let Some(p) = remaining.find(o) {
                if best.is_none_or(|b| p < b) {
                    best = Some(p);
                }
            }
        }
        let Some(pos) = best else {
            out.push_str(remaining);
            break;
        };
        out.push_str(&remaining[..pos]);
        let tail = &remaining[pos..];
        // Cut after the OUTERMOST closing tag (e.g. </tool_calls>, not the inner
        // </invoke>); if none, drop to the end (a stream cut mid-block).
        let mut cut: Option<usize> = None;
        for c in CLOSES {
            if let Some(i) = tail.find(c) {
                let after = pos + i + c.len();
                cut = Some(cut.map_or(after, |c0| c0.max(after)));
            }
        }
        remaining = &remaining[cut.unwrap_or(remaining.len())..];
    }

    // Drop a dangling partial-tag fragment at the very end (stream cut mid-marker,
    // e.g. a lone `<|` or `</`): never legitimate prose at end-of-message.
    let trimmed = out.trim_end();
    let frag_len = if trimmed.ends_with("</") || trimmed.ends_with("<|") {
        Some(2)
    } else if trimmed.ends_with('<') {
        Some(1)
    } else {
        None
    };
    if let Some(n) = frag_len {
        out.truncate(trimmed.len() - n);
    }

    out.trim().to_string()
}

// =====================================================================
// Helpers (recovered after an accidental truncation): these are UNMODIFIED
// from the v0.4.5 baseline, restored verbatim; prompt_visual_rows is
// reconstructed to satisfy the contract used by app.rs and the prompt
// cursor/compose segmentation.
// =====================================================================

pub(crate) fn char_visual_width(c: char) -> usize {
    match c {
        '\t' => 4,
        '\n' | '\r' => 0,
        // Private use area glyphs (Nerd font powerline icons like  ,  )
        '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}' => 1,
        // Zero-width characters (variation selectors, zero-width joiner)
        '\u{fe00}'..='\u{fe0f}' | '\u{200b}'..='\u{200d}' => 0,
        _ => c.width().unwrap_or(1).max(1),
    }
}

pub(crate) fn str_visual_width(s: &str) -> usize {
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
        let is_empty =
            last.spans.is_empty() || last.spans.iter().all(|s| s.content.trim().is_empty());
        if !is_empty {
            lines.push(Line::from(""));
        }
    }
}

/// A visual row of the prompt input: half-open byte range [byte_start, byte_end).
/// A row's end may include the terminating '\n' (see `prompt_move_cursor_vertical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PromptRow {
    pub byte_start: usize,
    pub byte_end: usize,
}

/// Splits a prompt into visual rows matching the display/compose word-wrap at
/// `max_w` cells. Hard '\n' breaks start a new row; the last row of a subline
/// absorbs the terminating newline into its `byte_end` (so callers can strip it).
pub(crate) fn prompt_visual_rows(text: &str, max_w: usize) -> Vec<PromptRow> {
    let mut rows: Vec<PromptRow> = Vec::new();
    if max_w == 0 {
        if !text.is_empty() {
            rows.push(PromptRow {
                byte_start: 0,
                byte_end: text.len(),
            });
        }
        return rows;
    }

    let mut byte_offset = 0usize;
    let mut sub_iter = text.split('\n').peekable();
    while let Some(subline) = sub_iter.next() {
        let is_last = sub_iter.peek().is_none();
        let sub_start = byte_offset;
        let sub_end = sub_start + subline.len();
        byte_offset = sub_end + 1; // +1 for the separating '\n'

        if subline.is_empty() {
            rows.push(PromptRow {
                byte_start: sub_start,
                byte_end: sub_end,
            });
            continue;
        }

        let flush = |rows: &mut Vec<PromptRow>, a: usize, b: usize| {
            if b > a || a == sub_start {
                rows.push(PromptRow {
                    byte_start: a,
                    byte_end: b,
                });
            }
        };

        let mut row_start_rel = 0usize;
        let mut col = 0usize;
        let mut pos = 0usize;

        for word in subline.split_inclusive(' ') {
            let trimmed = word.trim_end_matches(' ');
            let word_w = str_visual_width(trimmed);
            let trailing = word.len() - trimmed.len();

            if col > 0 && col + word_w > max_w {
                flush(&mut rows, sub_start + row_start_rel, sub_start + pos);
                row_start_rel = pos;
                col = 0;
            }

            if word_w == 0 {
                col += trailing;
            } else if col + word_w + trailing <= max_w {
                col += word_w + trailing;
            } else if col + word_w <= max_w {
                // word fits, trailing spaces overflow to the next visual row
                flush(
                    &mut rows,
                    sub_start + row_start_rel,
                    sub_start + pos + word.len(),
                );
                row_start_rel = pos + word.len();
                col = 0;
            } else {
                // word too wide for one row: hard-split by cell width
                if col > 0 {
                    flush(&mut rows, sub_start + row_start_rel, sub_start + pos);
                    row_start_rel = pos;
                }
                let mut chunk_start = 0usize;
                let mut chunk_w = 0usize;
                for (i, ch) in trimmed.char_indices() {
                    let cw = char_visual_width(ch);
                    if chunk_w > 0 && chunk_w + cw > max_w {
                        flush(
                            &mut rows,
                            sub_start + row_start_rel + chunk_start,
                            sub_start + row_start_rel + i,
                        );
                        chunk_start = i;
                        chunk_w = 0;
                    }
                    chunk_w += cw;
                }
                col = chunk_w + trailing;
                row_start_rel = pos + word.len();
            }
            pos += word.len();
        }

        let row_end = if is_last { sub_end } else { byte_offset };
        rows.push(PromptRow {
            byte_start: sub_start + row_start_rel,
            byte_end: row_end,
        });
    }

    rows
}

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

            if !cursor_found
                && cursor_byte_pos >= byte_offset
                && cursor_byte_pos <= byte_offset + word_bytes
            {
                let inside_offset = (cursor_byte_pos - byte_offset).min(word.len());
                let inside_offset = word.floor_char_boundary(inside_offset);
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

#[cfg(test)]
mod recovered_regression_tests {
    use super::{
        clean_response, compute_prompt_cursor_and_lines, prompt_visual_rows,
        render_markdown_blocks, str_visual_width,
    };
    use ratatui::{text::Text, widgets::Paragraph, widgets::Wrap};

    fn spans_text(lines: &[ratatui::text::Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// While a tool consent is pending for a command, the matching fence in the
    /// assistant text must be demoted to an inert snippet — the approval card is the
    /// single actionable surface (duplicate « Alt+N » vs « F10 » report). A fence that
    /// does NOT match the pending command keeps its interactive card.
    #[test]
    fn pending_approval_demotes_matching_command_card() {
        let text = "voici :\n```bash\necho hello\n```\n";
        let mut lines = Vec::new();
        let mut counter = 0usize;
        render_markdown_blocks(
            text,
            crate::i18n::Language::Fr,
            &mut lines,
            None,
            &mut counter,
            Some("echo hello"),
        );
        let joined = spans_text(&lines);
        assert!(!joined.contains("COMMANDE #"), "{joined}");
        assert!(!joined.contains("Exécuter"), "{joined}");
        assert!(joined.contains("echo hello"), "{joined}");

        // A different pending command leaves the proposal card untouched.
        let mut lines2 = Vec::new();
        let mut counter2 = 0usize;
        render_markdown_blocks(
            text,
            crate::i18n::Language::Fr,
            &mut lines2,
            None,
            &mut counter2,
            Some("echo other"),
        );
        let joined2 = spans_text(&lines2);
        assert!(joined2.contains("COMMANDE #1"), "{joined2}");
        assert!(joined2.contains("Exécuter"), "{joined2}");
    }

    /// The reconstructed visual-row splitter must agree with ratatui's own wrapped
    /// line count for the same text/width (the authoritative oracle).
    #[test]
    fn prompt_visual_rows_matches_ratatui_line_count() {
        let cases = [
            ("hello world", 20),
            ("hello world this is a longer prompt that must wrap", 18),
            ("a b c d e f g", 8),
            ("line one\nline two\nline three", 15),
            ("  leading and trailing  spaces", 12),
            ("", 10),
        ];
        for (text, w) in cases {
            let n_rows = prompt_visual_rows(text, w).len();
            let par = Paragraph::new(Text::raw(text)).wrap(Wrap { trim: false });
            let expected = par.line_count(w as u16);
            assert_eq!(
                n_rows, expected,
                "row-count mismatch for {text:?} at width {w}"
            );
        }
    }

    #[test]
    fn prompt_visual_rows_ranges_are_contiguous_and_cover_text() {
        let text = "abc def ghi";
        let rows = prompt_visual_rows(text, 5);
        assert!(!rows.is_empty());
        assert_eq!(rows.first().unwrap().byte_start, 0);
        for pair in rows.windows(2) {
            assert_eq!(
                pair[0].byte_end, pair[1].byte_start,
                "gap/overlap between rows"
            );
        }
        assert_eq!(rows.last().unwrap().byte_end, text.len());
    }

    #[test]
    fn strip_balanced_tool_calls_block() {
        let msg = "Je relance.\n\n```bash\ncd /var/www/xorne/resa-prod\n```\n\n<tool_calls>\n<invoke name=\"exec_command\">\n<parameter name=\"command\" string=\"true\">cd /var/www && echo ok</parameter>\n</invoke>\n</tool_calls>";
        let cleaned = clean_response(msg);
        assert!(!cleaned.contains("<tool_calls>"));
        assert!(!cleaned.contains("exec_command"));
        assert!(cleaned.contains("Je relance."));
        assert!(cleaned.contains("```bash"));
    }

    #[test]
    fn strip_fullwidth_pipe_hybrid_marker() {
        // Exact hybrid delimiter some models emit: fullwidth-pipe open, ASCII close.
        let msg = "Vérif en cours.\n\n\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<invoke name=\"exec_command\">\n<parameter name=\"command\" string=\"true\">ls</parameter>\n</invoke>\n</tool_calls>";
        let cleaned = clean_response(msg);
        assert!(
            !cleaned.contains("DSML"),
            "fullwidth-pipe DSML marker must vanish"
        );
        assert!(!cleaned.contains("tool_calls"));
        assert_eq!(cleaned, "Vérif en cours.");
    }

    #[test]
    fn strip_unbalanced_drops_to_end() {
        let msg = "Réponse courte.\n\n<tool_calls>\n<invoke name=\"exec_command\">";
        assert_eq!(clean_response(msg), "Réponse courte.");
    }

    #[test]
    fn leaves_normal_markdown_untouched() {
        let msg = "Voici du code:\n\n```bash\necho hi\n```";
        assert_eq!(clean_response(msg), msg);
    }

    #[test]
    fn cursor_and_rows_are_self_consistent() {
        let text = "hello world wrapped prompt";
        let w = 12;
        let rows = prompt_visual_rows(text, w).len();
        let (_, _, total) = compute_prompt_cursor_and_lines(text, 0, w);
        assert_eq!(
            total,
            rows.max(1) as u16,
            "cursor total must match visual rows"
        );
    }

    #[test]
    fn strip_hybrid_dsml_block_leaves_no_residue() {
        // Exact live bytes: `<｜｜DSML｜｜…>` — the old opener `｜DSML` matched at the
        // SECOND fullwidth pipe and left a stray `<｜` (seen as `< |`) on screen.
        let msg = "Je relance :\n\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"command\" string=\"true\">uptime</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>";
        let cleaned = clean_response(msg);
        assert_eq!(cleaned, "Je relance :");
        assert!(!cleaned.contains('\u{ff5c}'), "fullwidth pipe residue");
        assert!(!cleaned.contains('<'), "angle-bracket residue");
    }

    #[test]
    fn strip_dangling_partial_marker_at_end() {
        // Stream cut right after the marker started: lone fragment at end-of-message.
        assert_eq!(
            clean_response("Texte et fin.\n\n<\u{ff5c}"),
            "Texte et fin."
        );
        assert_eq!(clean_response("Texte et fin.\n\n<|"), "Texte et fin.");
        assert_eq!(clean_response("Texte et fin.\n\n</"), "Texte et fin.");
        assert_eq!(clean_response("Texte et fin.\n\n<"), "Texte et fin.");
    }

    #[test]
    fn legit_lt_sign_mid_text_is_preserved() {
        assert_eq!(clean_response("compare a < b ici"), "compare a < b ici");
    }

    #[test]
    fn str_visual_width_counts_correctly() {
        assert_eq!(str_visual_width("abc"), 3);
        assert_eq!(str_visual_width("a b"), 3);
        assert_eq!(str_visual_width(""), 0);
    }
}
