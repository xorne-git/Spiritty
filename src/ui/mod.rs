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
    widgets::Widget,
    Frame,
};

use crate::{
    app::{App, Focus, ModalState},
    i18n::{I18nKey, Language},
};
use chat_panel::ChatPanel;
use components::{BookmarksModal, ExportModal, HelpModal};
use terminal_panel::TerminalPanel;

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

    // 2. Pure full-height horizontal split (Left: Chat, Right: Terminal)
    let body_chunks = Layout::horizontal([
        Constraint::Percentage(app.split_ratio),
        Constraint::Percentage(100 - app.split_ratio),
    ])
    .split(workspace_area);

    let chat_area = body_chunks[0];
    let terminal_area = body_chunks[1];

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

    // 2.3 Show subtle split drag guide only when actively dragging
    if app.is_dragging_split {
        let split_x = chat_area.right().saturating_sub(1);
        let drag_style = Style::default().fg(palette.warning).add_modifier(Modifier::BOLD);
        for y in workspace_area.top()..workspace_area.bottom() {
            buf.set_string(split_x, y, "│", drag_style);
        }
    }

    // Render Chat Panel (and get cursor)
    let chat_panel = ChatPanel::new(app);
    let chat_cursor = chat_panel.render_panel(chat_area, buf);

    // Render Terminal Panel (and get cursor)
    let terminal_panel = TerminalPanel::new(app);
    let term_cursor = terminal_panel.render_panel(terminal_area, buf);

    // 2.5 Apply mouse selection highlight and copy to clipboard on release
    if let Some(ref sel) = app.mouse_selection.clone() {
        let panel_area = match sel.panel {
            crate::app::SelectionPanel::Chat => chat_area,
            crate::app::SelectionPanel::Terminal => terminal_area,
        };

        let inner = Rect {
            x: panel_area.x.saturating_add(1),
            y: panel_area.y.saturating_add(1),
            width: panel_area.width.saturating_sub(2),
            height: panel_area.height.saturating_sub(2),
        };

        if inner.width > 0 && inner.height > 0 {
            let s_x = sel.start.0.clamp(inner.left(), inner.right().saturating_sub(1));
            let s_y = sel.start.1.clamp(inner.top(), inner.bottom().saturating_sub(1));
            let e_x = sel.end.0.clamp(inner.left(), inner.right().saturating_sub(1));
            let e_y = sel.end.1.clamp(inner.top(), inner.bottom().saturating_sub(1));

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
                            cell.set_style(Style::default().bg(palette.selection_bg).fg(palette.text_primary));
                        }
                    }
                }
                extracted_lines.push(row_str.trim_end().to_string());
            }

            // If user just finished dragging (mouse was released), copy the text and clear selection!
            if !sel.is_selecting {
                let full_text = extracted_lines.join("\n").trim().to_string();
                if !full_text.is_empty() {
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
        frame.buffer_mut().set_string(x, footer_divider_area.top(), "─", footer_div_style);
    }

    // Render 1-line Info Footer at bottom
    render_footer(app, footer_area, frame.buffer_mut());

    let lang = app.config.get_language();

    // 3. Render Modal Overlays on top of the split screen if active
    match &app.modal {
        ModalState::Help => {
            HelpModal::render_modal(size, frame.buffer_mut(), lang);
        }
        ModalState::Config(config_state) => {
            config_state.render_modal(size, frame.buffer_mut(), lang);
        }
        ModalState::Sessions(session_state) => {
            session_state.render_modal(size, frame.buffer_mut(), lang);
        }
        ModalState::Bookmarks(bookmarks_state) => {
            BookmarksModal::render_modal(size, frame.buffer_mut(), bookmarks_state, lang);
        }
        ModalState::Export(export_state) => {
            ExportModal::render_modal(size, frame.buffer_mut(), export_state, lang);
        }
        ModalState::Mcp(mcp_state) => {
            crate::ui::components::McpModal::new(mcp_state, lang).render(size, frame.buffer_mut());
        }
        ModalState::None => {
            // Position cursor on the active pane only when no modal is open
            match app.focus {
                Focus::Chat => {
                    if let Some((cx, cy)) = chat_cursor {
                        frame.set_cursor_position((cx, cy));
                    }
                }
                Focus::Terminal => {
                    if let Some((cx, cy)) = term_cursor {
                        frame.set_cursor_position((cx, cy));
                    }
                }
            }
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
    let min_reserved_for_shortcuts = if width >= 30 { 14 } else if width >= 15 { 6 } else { 0 };
    let max_left_width = width.saturating_sub(min_reserved_for_shortcuts);

    let left_spans = build_left_metrics(app, lang, max_left_width);
    let left_width: usize = left_spans.iter().map(|s| s.width()).sum();

    // 2. Compute available space for right shortcuts
    let available_right_width = width.saturating_sub(left_width + 1);

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
}

fn build_right_shortcuts(app: &App, lang: Language, available_width: usize) -> Vec<Span<'static>> {
    use crate::config::AutoApproveLevel;
    let (auto_badge_color, auto_badge_text) = match app.config.auto_approve {
        AutoApproveLevel::Safe => (Color::Green, "Safe"),
        AutoApproveLevel::Sudo => (Color::Yellow, "Sudo"),
        AutoApproveLevel::Yolo => (Color::Red, "YOLO"),
        AutoApproveLevel::Off => (Color::DarkGray, "Off"),
    };

    let mut right = Vec::new();

    if available_width >= 82 {
        // Tier 1: Full Powerline pills with all shortcuts
        right.push(Span::styled(lang.t(I18nKey::FooterApprovalLabel), Style::default().fg(Color::DarkGray)));
        right.extend(key_pill("F3", auto_badge_color));
        right.push(Span::styled(format!(" {} ", auto_badge_text), Style::default().fg(auto_badge_color).add_modifier(Modifier::BOLD)));

        right.push(Span::raw(" "));
        right.extend(key_pill("Ctrl", Color::Magenta));
        right.push(Span::raw(" "));
        right.extend(key_pill("P", Color::Magenta));
        right.push(Span::styled(" Config ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        right.push(Span::raw(" "));
        right.extend(key_pill("Ctrl", Color::Cyan));
        right.push(Span::raw(" "));
        right.extend(key_pill("B", Color::Cyan));
        right.push(Span::styled(" Hosts ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        right.push(Span::raw(" "));
        right.extend(key_pill("Ctrl", Color::Rgb(140, 100, 240)));
        right.push(Span::raw(" "));
        right.extend(key_pill("M", Color::Rgb(140, 100, 240)));
        right.push(Span::styled(" MCP ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        right.push(Span::raw(" "));
        right.extend(key_pill("Ctrl", Color::LightCyan));
        right.push(Span::raw(" "));
        right.extend(key_pill("H", Color::LightCyan));
        right.push(Span::styled(" Sessions ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        right.push(Span::raw(" "));
        right.extend(key_pill("F1", Color::Cyan));
        right.push(Span::styled(
            if lang == Language::Fr { " Aide " } else { " Help " },
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    } else if available_width >= 50 {
        // Tier 2: Compact badges with all shortcuts
        right.push(Span::styled(format!("F3:{}", auto_badge_text), Style::default().fg(auto_badge_color).add_modifier(Modifier::BOLD)));
        right.push(Span::raw("  "));

        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Config  ", Style::default().fg(Color::White)));

        right.push(Span::styled("^B", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Hosts  ", Style::default().fg(Color::White)));

        right.push(Span::styled("^M", Style::default().fg(Color::Rgb(140, 100, 240)).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" MCP  ", Style::default().fg(Color::White)));

        right.push(Span::styled("^H", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Sess  ", Style::default().fg(Color::White)));

        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(if lang == Language::Fr { " Aide" } else { " Help" }, Style::default().fg(Color::White)));
    } else if available_width >= 36 {
        // Tier 3: F3, ^P Config, ^B Hosts, F1 Aide
        right.push(Span::styled(format!("F3:{}", auto_badge_text), Style::default().fg(auto_badge_color).add_modifier(Modifier::BOLD)));
        right.push(Span::raw("  "));

        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Config  ", Style::default().fg(Color::White)));

        right.push(Span::styled("^B", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Hosts  ", Style::default().fg(Color::White)));

        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(if lang == Language::Fr { " Aide" } else { " Help" }, Style::default().fg(Color::White)));
    } else if available_width >= 24 {
        // Tier 4: F3, ^P Config, F1 Aide
        right.push(Span::styled(format!("F3:{}", auto_badge_text), Style::default().fg(auto_badge_color).add_modifier(Modifier::BOLD)));
        right.push(Span::raw("  "));

        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Config  ", Style::default().fg(Color::White)));

        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(if lang == Language::Fr { " Aide" } else { " Help" }, Style::default().fg(Color::White)));
    } else if available_width >= 18 {
        // Tier 5: Essential ^P Config + F1 Aide
        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Config  ", Style::default().fg(Color::White)));

        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(if lang == Language::Fr { " Aide" } else { " Help" }, Style::default().fg(Color::White)));
    } else if available_width >= 12 {
        // Tier 6: ^P Config F1
        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" Config ", Style::default().fg(Color::White)));
        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    } else if available_width >= 6 {
        // Tier 7: ^P F1
        right.push(Span::styled("^P", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        right.push(Span::raw(" "));
        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    } else if available_width >= 2 {
        // Tier 8: F1
        right.push(Span::styled("F1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    }

    right
}

fn build_left_metrics(app: &App, lang: Language, max_width: usize) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0;

    let provider_name = app.get_active_provider_name();
    let model_name = app.get_active_model_name();
    let tokens_used = if app.current_session.total_tokens > 0 {
        app.current_session.total_tokens
    } else {
        app.get_total_tokens_used()
    };
    let ctx_used = app.get_context_used_tokens();
    let ctx_total = app.get_context_window_limit();
    let ctx_pct = (ctx_used as f64 / ctx_total as f64 * 100.0).clamp(0.0, 100.0);
    let is_generating = app.agent.is_generating;
    let is_active_generating = is_generating && app.pending_tool_approval.is_none();
    let spinner_char = get_spinner_char(app.spinner_frame);

    // 1. Provider
    let p_icon = Span::styled(" 󰚩 ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let p_name = Span::styled(provider_name.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let p_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));

    let p_w = p_icon.width() + p_name.width() + p_sep.width();
    if current_width + p_w <= max_width {
        current_width += p_w;
        spans.push(p_icon);
        spans.push(p_name);
        spans.push(p_sep);
    }

    // 2. Model Name
    let m_span = if is_generating {
        if is_active_generating {
            Span::styled(format!("{} {}", spinner_char, model_name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(format!("● {}", model_name), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
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
            let trunc = format!("{}…", &m_span.content[..available.saturating_sub(2).min(m_span.content.len())]);
            let s = Span::styled(trunc, m_span.style);
            current_width += s.width();
            spans.push(s);
        }
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

    // 4. Cost estimation (if > $0.0001)
    let cost = if let Ok(guard) = app.pricing_registry.try_read() {
        app.current_session.estimated_cost_with_pricing(&guard)
    } else {
        app.current_session.estimated_cost_usd()
    };
    if cost > 0.00005 {
        let c_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
        let c_icon = Span::styled("💵 ", Style::default().fg(Color::LightGreen));
        let c_val = Span::styled(format!("${:.4}", cost), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
        let c_w = c_sep.width() + c_icon.width() + c_val.width();

        if current_width + c_w <= max_width {
            current_width += c_w;
            spans.push(c_sep);
            spans.push(c_icon);
            spans.push(c_val);
        }
    }

    // 5. Context window usage
    let ctx_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
    let ctx_icon = Span::styled("📊 ", Style::default().fg(Color::Magenta));
    let ctx_str = format!("Ctx: {} / {} ({:.0}%)", format_token_count(ctx_used), format_token_count(ctx_total), ctx_pct);
    let ctx_val = Span::styled(ctx_str, Style::default().fg(Color::Gray));
    let ctx_w = ctx_sep.width() + ctx_icon.width() + ctx_val.width();

    if current_width + ctx_w <= max_width {
        current_width += ctx_w;
        spans.push(ctx_sep);
        spans.push(ctx_icon);
        spans.push(ctx_val);
    }

    // 6. Toasts (Clipboard or Notification)
    if let Some((time, len)) = app.clipboard_toast {
        if time.elapsed().as_millis() < 2500 {
            let toast_sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));
            let toast_icon = Span::styled("📋 ", Style::default().fg(Color::Green));
            let msg = if lang == Language::Fr {
                format!("Copié ({} car.)", len)
            } else {
                format!("Copied ({} chars)", len)
            };
            let toast_val = Span::styled(msg, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
            let toast_w = toast_sep.width() + toast_icon.width() + toast_val.width();
            if current_width + toast_w <= max_width {
                spans.push(toast_sep);
                spans.push(toast_icon);
                spans.push(toast_val);
            }
        }
    }

    spans
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
