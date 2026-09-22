use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::{
    i18n::{I18nKey, Language},
    ui::key_pill,
};

/// Scroll state for the single-column help overlay.
///
/// `max_scroll` is recomputed on every render from the actual viewport height (stored in a
/// `Cell` so the render pass only needs `&self`), letting `handle_key` clamp the offset
/// without knowing the terminal size.
#[derive(Default)]
pub struct HelpModalState {
    pub scroll: u16,
    max_scroll: std::cell::Cell<u16>,
}

impl HelpModalState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct HelpModal;

impl HelpModal {
    /// Handles a key. Returns `true` when the modal must close (`Esc` / `Enter`).
    /// Arrow keys, `PgUp`/`PgDn`, `Home`/`End` (and vim `j`/`k`) scroll the list instead.
    pub fn handle_key(key: KeyEvent, state: &mut HelpModalState) -> bool {
        let max = state.max_scroll.get();
        match key.code {
            KeyCode::Esc | KeyCode::Enter => return true,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                state.scroll = state.scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                state.scroll = state.scroll.saturating_add(1).min(max);
            }
            KeyCode::PageUp => {
                state.scroll = state.scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                state.scroll = state.scroll.saturating_add(8).min(max);
            }
            KeyCode::Home => state.scroll = 0,
            KeyCode::End => state.scroll = max,
            _ => {}
        }
        false
    }

    pub fn render_modal(area: Rect, buf: &mut Buffer, lang: Language, state: &HelpModalState) {
        // 88% of the screen width / 90% of the height, kept inside the terminal bounds.
        let modal_width = ((area.width as u32 * 88) / 100)
            .clamp(56, 150)
            .min(area.width as u32) as u16;
        let modal_height = ((area.height as u32 * 90) / 100)
            .clamp(10, 44)
            .min(area.height as u32) as u16;

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                lang.t(I18nKey::HelpModalTitle),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        block.render(modal_area, buf);

        let border_style = Style::default().fg(Color::Cyan);

        // Footer separator (2 lines above the bottom border).
        let sep_y = modal_area.bottom().saturating_sub(3);
        let draw_separator =
            sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1);
        if draw_separator {
            buf.set_string(
                modal_area.left(),
                sep_y,
                symbols::line::NORMAL.vertical_right,
                border_style,
            );
            for px in (modal_area.left() + 1)..(modal_area.right().saturating_sub(1)) {
                buf.set_string(px, sep_y, symbols::line::NORMAL.horizontal, border_style);
            }
            buf.set_string(
                modal_area.right().saturating_sub(1),
                sep_y,
                symbols::line::NORMAL.vertical_left,
                border_style,
            );
        }

        // Single full-width column: every entry keeps its key(s) and description on one line.
        // Two columns on the right are reserved for the scrollbar.
        let content_x = modal_area.left() + 2;
        let content_w = modal_area.width.saturating_sub(6);
        let content_y = modal_area.top() + 1;
        let content_h = sep_y.saturating_sub(content_y);

        let lines = build_help_lines(lang, content_w as usize);
        let total_lines = lines.len() as u16;
        let max_scroll = total_lines.saturating_sub(content_h);
        state.max_scroll.set(max_scroll);
        let offset = state.scroll.min(max_scroll);

        let content_area = Rect::new(content_x, content_y, content_w, content_h);
        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .scroll((offset, 0))
            .render(content_area, buf);

        // Scrollbar track + thumb, only when the content actually overflows.
        if max_scroll > 0 && content_h > 0 {
            let track_x = modal_area.right().saturating_sub(2);
            let track_style = Style::default().fg(Color::DarkGray);
            for row in content_y..(content_y + content_h) {
                buf.set_string(track_x, row, "│", track_style);
            }
            let thumb_h =
                ((content_h as u32 * content_h as u32) / total_lines as u32).max(1) as u16;
            let thumb_pos = ((offset as u32 * (content_h.saturating_sub(thumb_h)) as u32)
                / max_scroll as u32) as u16;
            for row in 0..thumb_h {
                buf.set_string(
                    track_x,
                    content_y + thumb_pos + row,
                    "█",
                    Style::default().fg(Color::Cyan),
                );
            }
        }

        // --- FOOTER CLOSE PROMPT (+ scroll hint when relevant) ---
        let mut footer = Vec::new();
        if max_scroll > 0 {
            footer.extend(key_pill("↑ / ↓", Color::Cyan));
            footer.push(Span::styled(
                format!(" {}  ·  ", lang.t(I18nKey::HelpFooterScroll)),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        footer.push(Span::styled(
            lang.t(I18nKey::HelpFooterPromptPrefix),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
        footer.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::DarkGray));
        footer.push(Span::styled(
            lang.t(I18nKey::HelpFooterPromptMiddle),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
        footer.extend(key_pill("F1", Color::DarkGray));
        footer.push(Span::styled(
            lang.t(I18nKey::HelpFooterPromptSuffix),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));

        let footer_area = Rect::new(
            modal_area.left() + 2,
            sep_y + 1,
            modal_area.width.saturating_sub(4),
            1,
        );
        Paragraph::new(Line::from(footer))
            .alignment(Alignment::Center)
            .render(footer_area, buf);
    }
}

fn make_help_row<'a>(mut keys: Vec<Span<'a>>, desc: &'a str, key_col_width: usize) -> Line<'a> {
    let key_width: usize = keys.iter().map(|s| s.width()).sum();
    // Always leave at least one separating space, even when the key(s) overflow the column.
    let pad = key_col_width.saturating_sub(key_width).max(1);
    keys.push(Span::raw(" ".repeat(pad)));
    keys.push(Span::styled(desc, Style::default().fg(Color::White)));
    Line::from(keys)
}

fn make_section_header<'a>(title: &'a str, color: Color, col_width: usize) -> Line<'a> {
    let title_len = title.chars().count();
    let remaining = col_width.saturating_sub(title_len + 4);
    let right_dashes = "─".repeat(remaining.max(2));

    Line::from(vec![
        Span::styled("─ ", Style::default().fg(color)),
        Span::styled(
            title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", right_dashes), Style::default().fg(color)),
    ])
}

/// Builds the flat, single-column list of help entries (sections + one line per shortcut).
fn build_help_lines(lang: Language, width: usize) -> Vec<Line<'static>> {
    let key_col_w = 42.min(width.saturating_sub(24)).max(14);
    let header_w = width.saturating_sub(3);
    let mut lines: Vec<Line<'static>> = Vec::new();

    // --- Navigation & Interface ---
    lines.push(Line::from(""));
    lines.push(make_section_header(
        lang.t(I18nKey::HelpSectionNavigation),
        Color::Cyan,
        header_w,
    ));

    let mut focus = key_pill(
        format!(
            "{} + {}",
            lang.t(I18nKey::HelpKeyShift),
            lang.t(I18nKey::HelpKeyTab)
        ),
        Color::Cyan,
    );
    focus.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
    focus.extend(key_pill(
        format!(
            "{} + {}",
            lang.t(I18nKey::HelpKeyCtrl),
            lang.t(I18nKey::HelpKeySpace)
        ),
        Color::Cyan,
    ));
    lines.push(make_help_row(
        focus,
        lang.t(I18nKey::HelpDescToggleFocus),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(lang.t(I18nKey::HelpKeyMouseClick), Color::Cyan),
        lang.t(I18nKey::HelpDescMouseClick),
        key_col_w,
    ));

    let mut scroll = key_pill(lang.t(I18nKey::HelpKeyScroll), Color::Cyan);
    scroll.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
    scroll.extend(key_pill("PgUp / PgDn", Color::Cyan));
    lines.push(make_help_row(
        scroll,
        lang.t(I18nKey::HelpDescScroll),
        key_col_w,
    ));

    let mut resize = key_pill(
        format!("{} + ←/→ ↑/↓", lang.t(I18nKey::HelpKeyAlt)),
        Color::Cyan,
    );
    resize.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
    resize.extend(key_pill(lang.t(I18nKey::HelpKeyDrag), Color::Cyan));
    lines.push(make_help_row(
        resize,
        lang.t(I18nKey::HelpDescResizePanels),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill("F4", Color::Cyan),
        lang.t(I18nKey::HelpDescToggleOrientation),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(format!("{} + T", lang.t(I18nKey::HelpKeyCtrl)), Color::Cyan),
        lang.t(I18nKey::HelpDescNewTab),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!(
                "{} + {}",
                lang.t(I18nKey::HelpKeyCtrl),
                lang.t(I18nKey::HelpKeyTab)
            ),
            Color::Cyan,
        ),
        lang.t(I18nKey::HelpDescNextTab),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!(
                "{} + {} + W",
                lang.t(I18nKey::HelpKeyCtrl),
                lang.t(I18nKey::HelpKeyShift)
            ),
            Color::Cyan,
        ),
        lang.t(I18nKey::HelpDescCloseTab),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(format!("{} + R", lang.t(I18nKey::HelpKeyAlt)), Color::Cyan),
        lang.t(I18nKey::HelpDescRenameTab),
        key_col_w,
    ));

    // --- Agent & Actions ---
    lines.push(Line::from(""));
    lines.push(make_section_header(
        lang.t(I18nKey::HelpSectionAgent),
        Color::Green,
        header_w,
    ));

    let mut f3 = key_pill("F3", Color::Green);
    f3.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
    f3.extend(key_pill(
        format!("{} + Y", lang.t(I18nKey::HelpKeyCtrl)),
        Color::Green,
    ));
    lines.push(make_help_row(
        f3,
        lang.t(I18nKey::HelpDescAutoApprove),
        key_col_w,
    ));

    let mut diag = key_pill(
        format!("{} + D", lang.t(I18nKey::HelpKeyAlt)),
        Color::Yellow,
    );
    diag.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
    diag.extend(key_pill(
        format!("{} + X", lang.t(I18nKey::HelpKeyAlt)),
        Color::DarkGray,
    ));
    lines.push(make_help_row(
        diag,
        lang.t(I18nKey::HelpDescDiagnoseError),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + M", lang.t(I18nKey::HelpKeyCtrl)),
            Color::Rgb(140, 100, 240),
        ),
        lang.t(I18nKey::HelpDescMcpModal),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + F", lang.t(I18nKey::HelpKeyCtrl)),
            Color::Yellow,
        ),
        lang.t(I18nKey::HelpDescChatSearch),
        key_col_w,
    ));

    // --- Sessions & Hosts ---
    lines.push(Line::from(""));
    lines.push(make_section_header(
        lang.t(I18nKey::HelpSectionSessions),
        Color::Yellow,
        header_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + P", lang.t(I18nKey::HelpKeyCtrl)),
            Color::Magenta,
        ),
        lang.t(I18nKey::HelpDescConfigModal),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(format!("{} + B", lang.t(I18nKey::HelpKeyCtrl)), Color::Cyan),
        lang.t(I18nKey::HelpDescBookmarksModal),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + H", lang.t(I18nKey::HelpKeyCtrl)),
            Color::LightCyan,
        ),
        lang.t(I18nKey::HelpDescSessionModal),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + E", lang.t(I18nKey::HelpKeyCtrl)),
            Color::Yellow,
        ),
        lang.t(I18nKey::HelpDescExportSession),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(format!("{} + N", lang.t(I18nKey::HelpKeyCtrl)), Color::Cyan),
        lang.t(I18nKey::HelpDescNewSession),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(
            format!("{} + S", lang.t(I18nKey::HelpKeyAlt)),
            Color::Yellow,
        ),
        lang.t(I18nKey::HelpDescScanHost),
        key_col_w,
    ));

    // --- General & Control ---
    lines.push(Line::from(""));
    lines.push(make_section_header(
        lang.t(I18nKey::HelpSectionGeneral),
        Color::LightCyan,
        header_w,
    ));

    lines.push(make_help_row(
        key_pill("F1", Color::Cyan),
        lang.t(I18nKey::HelpDescToggleHelp),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(format!("{} + Q", lang.t(I18nKey::HelpKeyCtrl)), Color::Red),
        lang.t(I18nKey::HelpDescQuit),
        key_col_w,
    ));

    lines.push(make_help_row(
        key_pill(lang.t(I18nKey::HelpKeyClose), Color::DarkGray),
        lang.t(I18nKey::HelpDescCloseModal),
        key_col_w,
    ));

    lines
}
