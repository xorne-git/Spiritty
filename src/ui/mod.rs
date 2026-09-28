pub mod chat_panel;
pub mod components;
pub mod terminal_panel;
pub mod theme;

pub use theme::{ThemeId, ThemePalette};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
    Frame,
};

use crate::{
    app::{App, Focus},
    config::SplitOrientation,
    i18n::{I18nKey, Language},
    voice::VoiceState,
};
use chat_panel::ChatPanel;
use terminal_panel::TerminalPanel;

/// Accent colour for the local voice-input shortcut in the footer.
const VOICE_COLOR: Color = Color::Cyan;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // 1. Split screen vertically into Main workspace, Footer Divider Line, and 1-line Info Footer
    let vertical_chunks = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(size);

    let workspace_area = vertical_chunks[0];
    let footer_divider_area = vertical_chunks[1];
    let footer_area = vertical_chunks[2];

    // 2. Split the workspace according to the active orientation:
    //    - Vertical: two columns (side by side)
    //    - Horizontal: two rows (stacked, best on narrow terminals)
    // The panel order (chat first vs terminal first) is then applied via `split_swapped`.
    // `split_ratio` always encodes the chat size, so the terminal gets the complement when
    // it is the first (left/top) panel.
    let first_ratio = if app.split_swapped {
        100 - app.split_ratio
    } else {
        app.split_ratio
    };
    let body_chunks = match app.split_orientation {
        SplitOrientation::Vertical => Layout::horizontal([
            Constraint::Percentage(first_ratio),
            Constraint::Percentage(100 - first_ratio),
        ])
        .split(workspace_area),
        SplitOrientation::Horizontal => Layout::vertical([
            Constraint::Percentage(first_ratio),
            Constraint::Percentage(100 - first_ratio),
        ])
        .split(workspace_area),
    };

    // Chat first by default (left/top); `F5` swaps the two panels.
    let first_area = body_chunks[0];
    let second_area = body_chunks[1];
    let (chat_area, terminal_area) = if app.split_swapped {
        (second_area, first_area)
    } else {
        (first_area, second_area)
    };

    app.chat_area = chat_area;
    app.terminal_area = terminal_area;

    let buf = frame.buffer_mut();
    let palette = app.theme.palette();
    let (g1_r, g1_g, g1_b) = palette.gradient_start;
    let (g2_r, g2_g, g2_b) = palette.gradient_end;

    // 2.1 Fill Chat Panel with Vertical Gradient
    let chat_h = chat_area.height.max(1) as f32;
    for y in chat_area.top()..chat_area.bottom() {
        let t = (y - chat_area.top()) as f32 / chat_h;
        let r = (g2_r as f32 * (1.0 - t) + g1_r as f32 * t).round() as u8;
        let g = (g2_g as f32 * (1.0 - t) + g1_g as f32 * t).round() as u8;
        let b = (g2_b as f32 * (1.0 - t) + g1_b as f32 * t).round() as u8;
        let bg_color = Color::Rgb(r, g, b);

        for x in chat_area.left()..chat_area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg_color);
            }
        }
    }

    // 2.2 Fill Terminal Panel with Inverted Vertical Gradient
    let term_h = terminal_area.height.max(1) as f32;
    for y in terminal_area.top()..terminal_area.bottom() {
        let t = (y - terminal_area.top()) as f32 / term_h;
        let r = (g1_r as f32 * (1.0 - t) + g2_r as f32 * t).round() as u8;
        let g = (g1_g as f32 * (1.0 - t) + g2_g as f32 * t).round() as u8;
        let b = (g1_b as f32 * (1.0 - t) + g2_b as f32 * t).round() as u8;
        let bg_color = Color::Rgb(r, g, b);

        for x in terminal_area.left()..terminal_area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg_color);
            }
        }
    }

    // Render Chat Panel (and get cursor)
    let chat_panel = ChatPanel::new(app);
    let chat_cursor = chat_panel.render_panel(chat_area, buf);

    // Render Terminal Panel (and get cursor)
    let terminal_panel = TerminalPanel::new(app);
    let term_cursor = terminal_panel.render_panel(terminal_area, buf);

    // 2.3 Permanent divider liseret between Chat and Terminal, matching the split orientation
    let split_style = if app.is_dragging_split {
        Style::default()
            .fg(palette.warning)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.border_unfocused)
    };
    match app.split_orientation {
        SplitOrientation::Vertical => {
            let split_x = first_area.right().saturating_sub(1);
            for y in workspace_area.top()..workspace_area.bottom() {
                buf.set_string(split_x, y, "│", split_style);
            }
        }
        SplitOrientation::Horizontal => {
            let split_y = first_area.bottom().saturating_sub(1);
            for x in workspace_area.left()..workspace_area.right() {
                buf.set_string(x, split_y, "─", split_style);
            }
        }
    }

    // 2.5 Apply mouse selection highlight and copy to clipboard on release
    if let Some(ref sel) = app.mouse_selection.clone() {
        let panel_area = match sel.panel {
            crate::app::SelectionPanel::Chat => chat_area,
            crate::app::SelectionPanel::Terminal => terminal_area,
        };

        let inner = Rect {
            x: panel_area.x,
            y: panel_area.y.saturating_add(1),
            width: panel_area.width,
            height: panel_area.height.saturating_sub(1),
        };

        if inner.width > 0 && inner.height > 0 {
            let s_x = sel
                .start
                .0
                .clamp(inner.left(), inner.right().saturating_sub(1));
            let s_y = sel
                .start
                .1
                .clamp(inner.top(), inner.bottom().saturating_sub(1));
            let e_x = sel
                .end
                .0
                .clamp(inner.left(), inner.right().saturating_sub(1));
            let e_y = sel
                .end
                .1
                .clamp(inner.top(), inner.bottom().saturating_sub(1));

            let (p1, p2) = if (s_y, s_x) <= (e_y, e_x) {
                ((s_x, s_y), (e_x, e_y))
            } else {
                ((e_x, e_y), (s_x, s_y))
            };

            let buf = frame.buffer_mut();
            let mut extracted_lines = Vec::new();

            for row in p1.1..=p2.1 {
                let (col_start, col_end) = if p1.1 == p2.1 {
                    (p1.0.min(p2.0), p1.0.max(p2.0))
                } else if row == p1.1 {
                    (p1.0, inner.right().saturating_sub(1))
                } else if row == p2.1 {
                    (inner.left(), p2.0)
                } else {
                    (inner.left(), inner.right().saturating_sub(1))
                };

                let mut row_str = String::new();
                for col in col_start..=col_end {
                    if let Some(cell) = buf.cell_mut((col, row)) {
                        row_str.push_str(cell.symbol());
                        if sel.is_selecting {
                            cell.set_style(
                                Style::default()
                                    .bg(palette.selection_bg)
                                    .fg(palette.text_primary),
                            );
                        }
                    }
                }
                extracted_lines.push(row_str.trim_end().to_string());
            }

            // If user just finished dragging (mouse was released), copy the text and clear selection!
            if !sel.is_selecting {
                let full_text = extracted_lines.join("\n").trim().to_string();
                // Only copy to the clipboard when at least 3 characters were selected,
                // so a stray 1-char click/drag doesn't pollute the clipboard.
                if full_text.chars().count() >= 3 {
                    crate::system::clipboard::copy_to_clipboard(&full_text);
                    app.clipboard_toast = Some((std::time::Instant::now(), full_text.len()));
                }
                app.mouse_selection = None;
            }
        }
    }

    // Render horizontal footer divider liseret (1px centered line)
    let footer_div_style = Style::default().fg(palette.accent_primary);
    for x in footer_divider_area.left()..footer_divider_area.right() {
        frame
            .buffer_mut()
            .set_string(x, footer_divider_area.top(), "─", footer_div_style);
    }

    // Render 1-line Info Footer at bottom
    render_footer(app, footer_area, frame.buffer_mut());

    let lang = app.config.get_language();

    // 3. Render Modal Overlays on top of the split screen if active
    if app.modal.is_open() {
        app.modal.render(size, frame.buffer_mut(), app.theme, lang);
    }

    // 3.5 Popup toast centrée (voix / presse-papier / notifications)
    render_toast_popup(app, size, frame.buffer_mut());

    // 4. Set Hardware Cursor on focused panel only when no modal is open
    if !app.modal.is_open() {
        let active_cursor = match app.focus {
            Focus::Chat => chat_cursor,
            Focus::Terminal => term_cursor,
        };

        if let Some((cx, cy)) = active_cursor {
            frame.set_cursor_position(ratatui::layout::Position { x: cx, y: cy });
        }
    }
}

const SPINNER_FRAMES: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// Returns the current spinner frame for smooth continuous rotation
pub fn get_spinner_char(frame: usize) -> &'static str {
    SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]
}

/// Renders the 1-line info bar at the bottom: Left metrics have full priority, right shortcuts adapt to remaining space
fn render_footer(app: &App, area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width < 5 {
        return;
    }

    let lang = app.config.get_language();
    let width = area.width as usize;

    // 1. Build Left Metrics with full priority (reserving a tiny minimum for essential shortcuts if window allows)
    let min_reserved_for_shortcuts = if width >= 30 {
        14
    } else if width >= 15 {
        6
    } else {
        0
    };
    let max_left_width = width.saturating_sub(min_reserved_for_shortcuts);

    let (left_spans, cost_range) = build_left_metrics(app, max_left_width);
    let left_width: usize = left_spans.iter().map(|s| s.width()).sum();

    // 2. Compute available space for right shortcuts (guaranteeing at least 2 spaces gap)
    let available_right_width = width.saturating_sub(left_width + 2);

    // 3. Build Right Shortcuts adapting to remaining space (prioritizing F1 Help and Ctrl+P Config)
    let right_spans = build_right_shortcuts(app, lang, available_right_width);
    let right_width: usize = right_spans.iter().map(|s| s.width()).sum();

    // 4. Combine Left Spans + Spaces + Right Spans
    let spaces = width.saturating_sub(left_width + right_width);
    let mut full_spans = Vec::new();
    full_spans.extend(left_spans);
    if spaces > 0 {
        full_spans.push(Span::raw(" ".repeat(spaces)));
    }
    full_spans.extend(right_spans);

    let line = Line::from(full_spans);
    buf.set_line(area.x, area.y, &line, area.width);

    // 5. Tooltip on hover over cost metric
    if let (Some((c_start, c_end)), Some((mx, my))) = (cost_range, app.mouse_pos) {
        let abs_start = area.x as usize + c_start;
        let abs_end = area.x as usize + c_end;
        if my == area.y && (mx as usize) >= abs_start && (mx as usize) < abs_end {
            render_pricing_tooltip(app, area, buf, abs_start as u16, lang);
        }
    }
}


/// Petite popup centrée pour les messages transitoires (enregistrement/transcription
/// vocale, copie presse-papier, notifications). Reste visible sur les fenêtres étroites,
/// là où la barre d'état d'une seule ligne les masquait.
fn render_toast_popup(app: &App, area: Rect, buf: &mut Buffer) {
    if area.width < 16 || area.height < 4 {
        return;
    }
    let lang = app.config.get_language();
    let (icon, text, color): (&str, String, Color) = match app.voice_state {
        VoiceState::Recording => (
            "🎙 ",
            lang.t(I18nKey::VoiceRecordingToast).to_string(),
            Color::LightRed,
        ),
        VoiceState::Transcribing => (
            "⏳ ",
            lang.t(I18nKey::VoiceTranscribingToast).to_string(),
            Color::LightYellow,
        ),
        _ => {
            if let Some((time, len)) = app.clipboard_toast {
                if time.elapsed().as_millis() < 2500 {
                    let msg = if lang == Language::Fr {
                        format!("Copié ({} car.)", len)
                    } else {
                        format!("Copied ({} chars)", len)
                    };
                    ("📋 ", msg, Color::LightGreen)
                } else {
                    return;
                }
            } else if let Some((time, ref msg)) = app.toast_message {
                if time.elapsed().as_millis() < 4500 {
                    ("", msg.clone(), Color::Cyan)
                } else {
                    return;
                }
            } else {
                return;
            }
        }
    };
    let span = Span::styled(
        format!("{icon}{text}"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    );
    let popup_w = ((span.width() as u16) + 4)
        .min(area.width.saturating_sub(2))
        .max(12);
    let popup_h = 3u16;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    Clear.render(popup, buf);
    let border = Style::default().fg(color).add_modifier(Modifier::BOLD);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .render(popup, buf);
    let inner = Rect::new(
        popup.x.saturating_add(2),
        popup.y.saturating_add(1),
        popup.width.saturating_sub(4),
        1,
    );
    Paragraph::new(Line::from(span))
        .alignment(ratatui::layout::Alignment::Center)
        .style(border)
        .render(inner, buf);
}

fn build_right_shortcuts(app: &App, lang: Language, available_width: usize) -> Vec<Span<'static>> {
    right_shortcut_spans(
        lang,
        app.config.auto_approve,
        app.voice_state,
        available_width,
    )
}

/// Builds the right-hand footer shortcuts in exact order:
/// **F3 approval, F7/F8 voice, F4 layout, F5 swap, F1 help**.
/// All `Ctrl+*` shortcuts have been removed. F1 is always present at the end of the line.
fn right_shortcut_spans(
    lang: Language,
    auto_approve: crate::config::AutoApproveLevel,
    voice_state: VoiceState,
    available_width: usize,
) -> Vec<Span<'static>> {
    use crate::config::AutoApproveLevel;

    if available_width < 4 {
        return Vec::new();
    }

    let (auto_badge_color, auto_badge_text) = match auto_approve {
        AutoApproveLevel::Safe => (Color::Green, "Safe"),
        AutoApproveLevel::Sudo => (Color::Yellow, "Sudo"),
        AutoApproveLevel::Yolo => (Color::Red, "YOLO"),
        AutoApproveLevel::Off => (Color::DarkGray, "Off"),
    };
    let help_label = if lang == Language::Fr { "Aide" } else { "Help" };
    let voice_label = lang.t(I18nKey::FooterVoiceLabel);

    struct Item {
        /// Rich: `[ F4/F5 ] Layout/Switch `.
        pill: Vec<Span<'static>>,
        /// Compact: `[F4/F5] Layout/Switch `.
        compact: Vec<Span<'static>>,
        /// Bare key: `[F4/F5]`.
        keys: Vec<Span<'static>>,
    }

    let pill = |key: String, color: Color, label: &str| -> Vec<Span<'static>> {
        let mut v = key_pill(key, color);
        v.push(Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        v
    };
    let compact = |key: &str, color: Color, label: &str| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                key.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {} ", label), Style::default().fg(Color::White)),
        ]
    };
    let bare = |key: &str, color: Color| -> Vec<Span<'static>> {
        vec![Span::styled(
            key.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]
    };

    // Voice shortcut: while dictating, the item becomes a live state badge so the user can
    // see the sentence is being recorded / transcribed; otherwise it advertises `F7/F8`.
    let (voice_pill, voice_compact, voice_keys) = match voice_state {
        VoiceState::Recording | VoiceState::Transcribing => {
            let recording = voice_state == VoiceState::Recording;
            let label = lang.t(if recording {
                I18nKey::VoiceBadgeRecording
            } else {
                I18nKey::VoiceBadgeTranscribing
            });
            let color = if recording {
                Color::Red
            } else {
                Color::Yellow
            };
            let mut spans = key_pill(label.to_string(), color);
            spans.push(Span::raw(" "));
            (spans.clone(), spans.clone(), spans)
        }
        _ => (
            pill("F7/F8".to_string(), VOICE_COLOR, voice_label),
            compact("[F7/F8]", VOICE_COLOR, voice_label),
            bare("[F7/F8]", VOICE_COLOR),
        ),
    };

    // Display order: F3 approval | F7/F8 voice | F4/F5 layout/switch | F1 help.
    let items: Vec<Item> = vec![
        Item {
            pill: {
                let mut v = vec![Span::styled(
                    lang.t(I18nKey::FooterApprovalLabel),
                    Style::default().fg(Color::DarkGray),
                )];
                v.extend(key_pill("F3", auto_badge_color));
                v.push(Span::styled(
                    format!(" {} ", auto_badge_text),
                    Style::default()
                        .fg(auto_badge_color)
                        .add_modifier(Modifier::BOLD),
                ));
                v
            },
            compact: vec![
                Span::styled(
                    format!("F3:{}", auto_badge_text),
                    Style::default()
                        .fg(auto_badge_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ],
            keys: vec![Span::styled(
                format!("F3:{}", auto_badge_text),
                Style::default()
                    .fg(auto_badge_color)
                    .add_modifier(Modifier::BOLD),
            )],
        },
        Item {
            pill: voice_pill.clone(),
            compact: voice_compact.clone(),
            keys: voice_keys.clone(),
        },
        Item {
            pill: pill("F4/F5".to_string(), Color::Cyan, "Layout/Switch"),
            compact: compact("[F4/F5]", Color::Cyan, "Layout/Switch"),
            keys: bare("[F4/F5]", Color::Cyan),
        },
        Item {
            pill: pill("F1".to_string(), Color::LightYellow, help_label),
            compact: compact("[F1]", Color::LightYellow, help_label),
            keys: bare("[F1]", Color::LightYellow),
        },
    ];

    let spans_for = |style: usize, idx: usize| -> &Vec<Span<'static>> {
        match style {
            0 => &items[idx].pill,
            1 => &items[idx].compact,
            _ => &items[idx].keys,
        }
    };
    let group_total = |style: usize, idxs: &[usize]| -> usize {
        let body: usize = idxs
            .iter()
            .map(|&i| spans_for(style, i).iter().map(|s| s.width()).sum::<usize>())
            .sum();
        body + idxs.len().saturating_sub(1)
    };

    let all_items = [0usize, 1, 2, 3]; // F3, F7/F8, F4/F5, F1

    // Richest style whose items fit; falls back to bare keys.
    let style = (0..3)
        .find(|&s| group_total(s, &all_items) <= available_width)
        .unwrap_or(2);

    let mut chosen: Vec<usize> = all_items.to_vec();

    // If even the bare keys do not fit, drop in order: F4/F5 then F7/F8 then F3.
    // F1 is ALWAYS kept at the end of the line.
    if group_total(style, &chosen) > available_width {
        for &drop in &[2usize, 1, 0] {
            if group_total(style, &chosen) <= available_width {
                break;
            }
            chosen.retain(|&i| i != drop);
        }
    }

    // Assemble in display order (ascending item index: 0=F3, 1=F7/F8, 2=F4/F5, 3=F1).
    chosen.sort_unstable();
    let mut right: Vec<Span<'static>> = Vec::new();
    for (n, &idx) in chosen.iter().enumerate() {
        if n > 0 {
            right.push(Span::raw(" "));
        }
        right.extend(spans_for(style, idx).iter().cloned());
    }

    right
}
fn build_left_metrics(
    app: &App,
    max_width: usize,
) -> (Vec<Span<'static>>, Option<(usize, usize)>) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0;
    let mut cost_col_range = None;

    let model_name = app.get_active_model_name();
    let tokens_used = if app.current_session.total_tokens > 0 {
        app.current_session.total_tokens
    } else {
        app.get_total_tokens_used()
    };
    let ctx_used = app.get_context_used_tokens();
    let ctx_total = app.get_context_window_limit();
    let ctx_pct = if ctx_total > 0 {
        (ctx_used as f64 / ctx_total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let is_generating = app.agent.is_generating;
    let is_active_generating = is_generating && app.pending_tool_approval.is_none();
    let spinner_char = get_spinner_char(app.spinner_frame);

    // 1. Model Name (the provider name/icon is intentionally omitted to leave room for the
    //    always-visible approval badge in the right-hand shortcuts).
    let m_span = if is_generating {
        if is_active_generating {
            Span::styled(
                format!("{} {}", spinner_char, model_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!("● {}", model_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        }
    } else {
        Span::styled(model_name, Style::default().fg(Color::LightCyan))
    };

    let m_w = m_span.width();
    if current_width + m_w <= max_width {
        current_width += m_w;
        spans.push(m_span);
    } else {
        // Try truncated model name
        let available = max_width.saturating_sub(current_width);
        if available >= 5 {
            // Split on a char boundary so a multi-byte model name can't panic.
            let cap = available.saturating_sub(2).min(m_span.content.len());
            let cut = m_span.content.floor_char_boundary(cap);
            let trunc = format!("{}…", &m_span.content[..cut]);
            let s = Span::styled(trunc, m_span.style);
            current_width += s.width();
            spans.push(s);
        }
    }

    // 2b. Thinking Level (directly to the right of model). When the configured effort is
    //     `Default`, show the provider's documented default level instead of a vague "Auto"
    //     (display only — no extra parameter is sent to the API).
    let reasoning = app.get_active_reasoning_effort();
    let effective = if reasoning == crate::config::ReasoningEffort::Default {
        app.resolved_default_reasoning()
    } else {
        Some(reasoning)
    };
    let (th_label, th_color) = match effective {
        Some(crate::config::ReasoningEffort::Off) => ("Off", Color::DarkGray),
        Some(crate::config::ReasoningEffort::Low) => ("Low", Color::Green),
        Some(crate::config::ReasoningEffort::Medium) => ("Med", Color::Yellow),
        Some(crate::config::ReasoningEffort::High) => ("High", Color::Magenta),
        _ => ("Model", Color::DarkGray),
    };
    let th_span = Span::styled(
        format!(" 🧠 {}", th_label),
        Style::default().fg(th_color).add_modifier(Modifier::BOLD),
    );
    let th_w = th_span.width();
    if current_width + th_w <= max_width {
        current_width += th_w;
        spans.push(th_span);
    }

    // 3. Tokens & Speed
    let t_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
    let t_icon = Span::styled("⚡ ", Style::default().fg(Color::Yellow));
    let tok_str = if let Some(tps) = app.get_tokens_per_sec() {
        format!("{} tok ({:.1} t/s)", format_token_count(tokens_used), tps)
    } else {
        format!("{} tok", format_token_count(tokens_used))
    };
    let t_val = Span::styled(tok_str, Style::default().fg(Color::White));
    let t_w = t_sep.width() + t_icon.width() + t_val.width();

    if current_width + t_w <= max_width {
        current_width += t_w;
        spans.push(t_sep);
        spans.push(t_icon);
        spans.push(t_val);
    }

    // 4. Cost estimation — shown ONLY when a tariff is actually configured for the active
    //    model; unknown models display nothing rather than an invented price.
    let known_cost = app
        .pricing_registry
        .try_read()
        .ok()
        .and_then(|guard| app.current_session.estimated_cost_opt(&guard));
    if let Some(cost) = known_cost {
        if cost >= 0.0005 {
            let is_deepseek = app
                .current_session
                .provider
                .to_lowercase()
                .contains("deepseek")
                || app
                    .current_session
                    .model
                    .to_lowercase()
                    .contains("deepseek")
                || matches!(
                    app.config.default_provider,
                    crate::config::ProviderType::DeepSeek
                );

            let is_offpeak = is_deepseek && crate::pricing::is_deepseek_offpeak(chrono::Utc::now());
            let cost_color = if is_deepseek {
                if is_offpeak {
                    Color::LightGreen
                } else {
                    Color::Rgb(240, 140, 40) // Orange for Peak
                }
            } else {
                Color::LightGreen
            };

            let c_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
            let c_icon = Span::styled("💵 ", Style::default().fg(cost_color));
            let c_val = Span::styled(
                format!("${:.3}", cost),
                Style::default().fg(cost_color).add_modifier(Modifier::BOLD),
            );
            let c_w = c_sep.width() + c_icon.width() + c_val.width();

            if current_width + c_w <= max_width {
                cost_col_range = Some((current_width, current_width + c_w));
                current_width += c_w;
                spans.push(c_sep);
                spans.push(c_icon);
                spans.push(c_val);
            }
        }
    }

    // 5. Context window usage
    let ctx_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
    let ctx_icon = Span::styled("📊 ", Style::default().fg(Color::Magenta));
    let ctx_str = format!(
        "Ctx: {} / {} ({:.0}%)",
        format_token_count(ctx_used),
        format_token_count(ctx_total),
        ctx_pct
    );
    let ctx_val = Span::styled(ctx_str, Style::default().fg(Color::Gray));
    let ctx_w = ctx_sep.width() + ctx_icon.width() + ctx_val.width();

    if current_width + ctx_w <= max_width {
        spans.push(ctx_sep);
        spans.push(ctx_icon);
        spans.push(ctx_val);
    }

    (spans, cost_col_range)
}

fn render_pricing_tooltip(
    app: &App,
    footer_area: Rect,
    buf: &mut Buffer,
    cost_x: u16,
    lang: Language,
) {
    let prov_str = app.get_active_provider_name();
    let model_str = app.get_active_model_name();
    let is_deepseek = prov_str.to_lowercase().contains("deepseek")
        || model_str.to_lowercase().contains("deepseek")
        || matches!(
            app.config.default_provider,
            crate::config::ProviderType::DeepSeek
        );

    // A rate is only ever displayed when one is actually configured for this model.
    let known_pricing = app
        .pricing_registry
        .try_read()
        .ok()
        .and_then(|guard| guard.get_pricing(prov_str, &model_str));
    let Some(pricing) = known_pricing else {
        render_unknown_pricing_tooltip(footer_area, buf, cost_x, lang, &model_str);
        return;
    };

    let is_offpeak = is_deepseek && crate::pricing::is_deepseek_offpeak(chrono::Utc::now());

    let (title, border_color, lines) = if is_deepseek {
        if is_offpeak {
            let title = if lang == Language::Fr {
                " 🟢 DeepSeek Tarification Off-Peak (-50%) ".to_string()
            } else {
                " 🟢 DeepSeek Off-Peak Pricing (-50%) ".to_string()
            };
            let mut l = Vec::new();
            if lang == Language::Fr {
                l.push(Line::from(vec![
                    Span::styled("• Statut  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Heures creuses actives (-50%)",
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Modèle  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Prompt  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok (réduit)", pricing.prompt),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Output  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok (réduit)", pricing.completion),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Horaires: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Week-end 24/24 & Semaine (12h-03h / 06h-08h FR)",
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            } else {
                l.push(Line::from(vec![
                    Span::styled("• Status  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Off-Peak Active (-50%)",
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Model   : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Prompt  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok (discounted)", pricing.prompt),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Output  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok (discounted)", pricing.completion),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Hours   : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Weekends 24/7 & Weekdays off-peak UTC",
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            (title, Color::LightGreen, l)
        } else {
            let title = if lang == Language::Fr {
                " 🟠 DeepSeek Tarification Peak (Plein tarif) ".to_string()
            } else {
                " 🟠 DeepSeek Peak Pricing (Standard) ".to_string()
            };
            let mut l = Vec::new();
            if lang == Language::Fr {
                l.push(Line::from(vec![
                    Span::styled("• Statut  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Heures pleines actives (Standard)",
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Modèle  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Prompt  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok", pricing.prompt),
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Output  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok", pricing.completion),
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Conseil : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Tarif -50% actif dès 12h00 FR (10:00 UTC)",
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            } else {
                l.push(Line::from(vec![
                    Span::styled("• Status  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Peak Hours (Standard)",
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Model   : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Prompt  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok", pricing.prompt),
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Output  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("${:.3} / 1M tok", pricing.completion),
                        Style::default()
                            .fg(Color::Rgb(240, 140, 40))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                l.push(Line::from(vec![
                    Span::styled("• Tip     : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "-50% discount resumes at 10:00 UTC",
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            }
            (title, Color::Rgb(240, 140, 40), l)
        }
    } else {
        let title = format!(" 💵 Tarification {} ", prov_str);
        let mut l = Vec::new();
        l.push(Line::from(vec![
            Span::styled("• Modèle  : ", Style::default().fg(Color::DarkGray)),
            Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
        ]));
        l.push(Line::from(vec![
            Span::styled("• Prompt  : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("${:.3} / 1M tok", pricing.prompt),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        l.push(Line::from(vec![
            Span::styled("• Output  : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("${:.3} / 1M tok", pricing.completion),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        (title, Color::Cyan, l)
    };

    let max_line_len = lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(50)
        .max(title.chars().count() + 2);
    let tooltip_width = ((max_line_len as u16) + 4)
        .clamp(52, 72)
        .min(footer_area.width.saturating_sub(2));
    let tooltip_height = (lines.len() as u16) + 2;
    let tooltip_x = cost_x
        .saturating_sub(2)
        .min(footer_area.right().saturating_sub(tooltip_width));
    let tooltip_y = footer_area.y.saturating_sub(tooltip_height);

    let popup_rect = Rect::new(tooltip_x, tooltip_y, tooltip_width, tooltip_height);
    Clear.render(popup_rect, buf);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_rect);
    block.render(popup_rect, buf);

    let p = Paragraph::new(lines);
    p.render(inner, buf);
}

/// Neutral pricing tooltip used when NO tariff is configured for the active model: it never
/// invents rates, and explains how to fetch the online multi-provider price list.
fn render_unknown_pricing_tooltip(
    footer_area: Rect,
    buf: &mut Buffer,
    cost_x: u16,
    lang: Language,
    model_str: &str,
) {
    let is_fr = lang == Language::Fr;
    let title = if is_fr {
        " 💵 Tarif non configuré ".to_string()
    } else {
        " 💵 Pricing unavailable ".to_string()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("• Modèle  : ", Style::default().fg(Color::DarkGray)),
            Span::styled(model_str.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• Tarif   : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if is_fr {
                    "Aucun tarif connu pour ce modèle — coût masqué"
                } else {
                    "No pricing known for this model — cost hidden"
                },
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("• Astuce  : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if is_fr {
                    "« Tarifs en ligne » dans la config (Ctrl+P) pour actualiser"
                } else {
                    "\"Update Pricing\" in config (Ctrl+P) to refresh"
                },
                Style::default().fg(Color::Gray),
            ),
        ]),
    ];

    let max_line_len = lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(50)
        .max(title.chars().count() + 2);
    let tooltip_width = ((max_line_len as u16) + 4)
        .clamp(52, 72)
        .min(footer_area.width.saturating_sub(2));
    let tooltip_height = (lines.len() as u16) + 2;
    let tooltip_x = cost_x
        .saturating_sub(2)
        .min(footer_area.right().saturating_sub(tooltip_width));
    let tooltip_y = footer_area.y.saturating_sub(tooltip_height);

    let popup_rect = Rect::new(tooltip_x, tooltip_y, tooltip_width, tooltip_height);
    Clear.render(popup_rect, buf);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_rect);
    block.render(popup_rect, buf);

    let p = Paragraph::new(lines);
    p.render(inner, buf);
}

pub fn key_pill<'a>(key: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> Vec<Span<'a>> {
    vec![
        Span::styled("[ ", Style::default().fg(color)),
        Span::styled(
            key.into(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ]", Style::default().fg(color)),
    ]
}

fn format_token_count(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}k", n / 1_000)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AutoApproveLevel;

    fn text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn footer_prioritizes_f3_f7_f8_f4_f5_f1() {
        let spans = right_shortcut_spans(Language::En, AutoApproveLevel::Safe, VoiceState::Idle, 240);
        let t = text(&spans);
        for expected in ["F3", "F7/F8", "F4/F5", "F1", "Layout/Switch"] {
            assert!(t.contains(expected), "footer missing `{expected}`: {t}");
        }
        assert!(!t.contains("Config"));
        assert!(!t.contains("Ctrl"));
        let i_f3 = t.find("F3").expect("F3");
        let i_voice = t.find("F7/F8").expect("F7/F8");
        let i_layout = t.find("F4/F5").expect("F4/F5");
        let i_f1 = t.find("F1").expect("F1");
        assert!(
            i_f3 < i_voice && i_voice < i_layout && i_layout < i_f1,
            "footer order must be F3 -> F7/F8 -> F4/F5 -> F1: {t}"
        );
    }

    #[test]
    fn footer_keeps_essentials_when_narrow() {
        // Mid width: all 4 shortcuts fit (compact style).
        let mid = right_shortcut_spans(Language::En, AutoApproveLevel::Safe, VoiceState::Idle, 90);
        let mt = text(&mid);
        for expected in ["F3", "F7/F8", "F4/F5", "F1"] {
            assert!(
                mt.contains(expected),
                "shortcut `{expected}` missing in mid-width: {mt}"
            );
        }
        assert!(!mt.contains("Ctrl"), "Ctrl+* must not be present: {mt}");

        // Very narrow: F1 and F3 always survive, F1 at the end.
        let tiny = text(&right_shortcut_spans(
            Language::En,
            AutoApproveLevel::Safe,
            VoiceState::Idle,
            5,
        ));
        assert!(tiny.contains("F1"), "F1 must always be kept: {tiny}");
        assert!(!tiny.contains("F4/F5"));

        // Below the floor: nothing is rendered.
        assert!(right_shortcut_spans(Language::En, AutoApproveLevel::Safe, VoiceState::Idle, 2)
            .is_empty());
    }


    #[test]
    fn footer_shows_voice_state_badge_while_dictating() {
        let recording = text(&right_shortcut_spans(
            Language::En,
            AutoApproveLevel::Safe,
            VoiceState::Recording,
            200,
        ));
        assert!(recording.contains("REC"), "expected REC badge: {recording}");
        assert!(recording.contains("F1"));

        let transcribing = text(&right_shortcut_spans(
            Language::En,
            AutoApproveLevel::Safe,
            VoiceState::Transcribing,
            200,
        ));
        assert!(
            transcribing.contains("Transcribing"),
            "expected Transcribing badge: {transcribing}"
        );
    }
}
