pub mod chat_panel;
pub mod components;
pub mod terminal_panel;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::{
    app::{App, Focus, ModalState},
    i18n::Language,
};
use chat_panel::ChatPanel;
use components::HelpModal;
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

    // 2.1 Fill Chat Panel with Vertical Gradient (Top Slate Navy -> Bottom Obsidian Midnight)
    let chat_h = chat_area.height.max(1) as f32;
    for y in chat_area.top()..chat_area.bottom() {
        let t = (y - chat_area.top()) as f32 / chat_h;
        let r = (24.0 * (1.0 - t) + 10.0 * t).round() as u8;
        let g = (34.0 * (1.0 - t) + 14.0 * t).round() as u8;
        let b = (48.0 * (1.0 - t) + 22.0 * t).round() as u8;
        let bg_color = Color::Rgb(r, g, b);

        for x in chat_area.left()..chat_area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg_color);
            }
        }
    }

    // 2.2 Fill Terminal Panel with Inverted Vertical Gradient (Top Obsidian Midnight -> Bottom Slate Navy)
    let term_h = terminal_area.height.max(1) as f32;
    for y in terminal_area.top()..terminal_area.bottom() {
        let t = (y - terminal_area.top()) as f32 / term_h;
        let r = (10.0 * (1.0 - t) + 24.0 * t).round() as u8;
        let g = (14.0 * (1.0 - t) + 34.0 * t).round() as u8;
        let b = (22.0 * (1.0 - t) + 48.0 * t).round() as u8;
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
        let drag_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
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
                            cell.set_style(Style::default().bg(Color::Rgb(40, 75, 130)).fg(Color::White));
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
    let footer_div_style = Style::default().fg(Color::Cyan);
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

/// Renders the 1-line info bar at the bottom: Provider, Model, Tokens, Context Window, and F1 Help button
fn render_footer(app: &App, area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    let lang = app.config.get_language();
    let provider_name = app.get_active_provider_name();
    let model_name = app.get_active_model_name();
    let tokens_used = app.get_total_tokens_used();
    let ctx_used = app.get_context_used_tokens();
    let ctx_total = app.get_context_window_limit();
    let ctx_pct = (ctx_used as f64 / ctx_total as f64 * 100.0).clamp(0.0, 100.0);
    let is_generating = app.agent.is_generating;
    let is_active_generating = is_generating && app.pending_tool_approval.is_none();
    let spinner_char = get_spinner_char(app.spinner_frame);

    let mut left_spans: Vec<Span<'static>> = Vec::new();
    left_spans.push(Span::styled(" 󰚩 ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    left_spans.push(Span::styled(provider_name.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));

    if is_generating {
        if is_active_generating {
            left_spans.push(Span::styled(
                format!("{} ", spinner_char),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            left_spans.push(Span::styled(
                model_name,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
        } else {
            // Waiting for user validation: keep icon, stop animation
            left_spans.push(Span::styled(
                "● ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
            left_spans.push(Span::styled(
                model_name,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }
    } else {
        left_spans.push(Span::styled(
            model_name,
            Style::default().fg(Color::LightCyan),
        ));
    }

    left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
    left_spans.push(Span::styled("📊 ", Style::default().fg(Color::Magenta)));
    left_spans.push(Span::styled(
        format!("Ctx: {} / {} ({:.0}%)", format_token_count(ctx_used), format_token_count(ctx_total), ctx_pct),
        Style::default().fg(Color::Gray),
    ));
    left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
    left_spans.push(Span::styled("⚡ ", Style::default().fg(Color::Yellow)));
    let tok_text = if let Some(tps) = app.get_tokens_per_sec() {
        format!("{} tok ({:.1} t/s)", format_token_count(tokens_used), tps)
    } else {
        format!("{} tokens", format_token_count(tokens_used))
    };
    left_spans.push(Span::styled(tok_text, Style::default().fg(Color::White)));

    if let Some((time, len)) = app.clipboard_toast {
        if time.elapsed().as_millis() < 2500 {
            left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            left_spans.push(Span::styled("📋 ", Style::default().fg(Color::Green)));
            left_spans.push(Span::styled(
                if lang == Language::Fr {
                    format!("Copié ({} car.)", len)
                } else {
                    format!("Copied ({} chars)", len)
                },
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        }
    }

    if let Some((time, ref msg)) = app.toast_message {
        if time.elapsed().as_millis() < 4000 {
            left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            left_spans.push(Span::styled(msg.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
    }

    let mut right_spans: Vec<Span<'static>> = Vec::new();

    // Auto-Approve badge with F3 shortcut
    use crate::config::AutoApproveLevel;
    let (auto_badge_color, auto_badge_text) = match app.config.auto_approve {
        AutoApproveLevel::Safe => (Color::Green, "Safe"),
        AutoApproveLevel::Sudo => (Color::Yellow, "Sudo"),
        AutoApproveLevel::Yolo => (Color::Red, "YOLO"),
        AutoApproveLevel::Off => (Color::DarkGray, "Off"),
    };
    right_spans.extend(key_pill("F3", auto_badge_color));
    right_spans.push(Span::styled(format!(" {} ", auto_badge_text), Style::default().fg(auto_badge_color).add_modifier(Modifier::BOLD)));

    right_spans.push(Span::raw(" "));
    right_spans.extend(key_pill("Ctrl", Color::LightCyan));
    right_spans.push(Span::raw(" "));
    right_spans.extend(key_pill("H", Color::LightCyan));
    right_spans.push(Span::styled(" Sessions ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

    right_spans.push(Span::raw(" "));
    right_spans.extend(key_pill("F1", Color::Cyan));
    right_spans.push(Span::styled(
        if lang == Language::Fr { " Aide " } else { " Help " },
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));

    let left_width: usize = left_spans.iter().map(|s| s.width()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.width()).sum();

    let mut full_spans = Vec::new();
    if left_width + right_width <= area.width as usize {
        let spaces = (area.width as usize).saturating_sub(left_width + right_width);
        full_spans.extend(left_spans);
        full_spans.push(Span::raw(" ".repeat(spaces)));
        full_spans.extend(right_spans);
    } else {
        full_spans.extend(left_spans);
    }

    let line = Line::from(full_spans);
    buf.set_line(area.x, area.y, &line, area.width);
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
