use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::i18n::{I18nKey, Language};

pub struct HelpModal;

fn key_pill<'a>(key: &'a str, color: Color) -> Vec<Span<'a>> {
    vec![
        Span::styled("", Style::default().fg(color)),
        Span::styled(
            key,
            Style::default()
                .bg(color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("", Style::default().fg(color)),
    ]
}

fn make_help_row<'a>(mut keys: Vec<Span<'a>>, desc: &'a str, key_col_width: usize) -> Line<'a> {
    let key_width: usize = keys.iter().map(|s| s.width()).sum();
    let pad = key_col_width.saturating_sub(key_width);
    if pad > 0 {
        keys.push(Span::raw(" ".repeat(pad)));
    }
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

impl HelpModal {
    pub fn render_modal(area: Rect, buf: &mut Buffer, lang: Language) {
        // Use 85% of screen width (min 72, max 145) and 88% of screen height
        let modal_width = ((area.width as u32 * 85) / 100).clamp(72, 145) as u16;
        let modal_height = ((area.height as u32 * 88) / 100).clamp(24, 40) as u16;

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                format!(" ⌨  {} ", lang.t(I18nKey::HelpModalTitle)),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        block.render(modal_area, buf);

        let border_style = Style::default().fg(Color::Cyan);

        // Footer separator position
        let sep_y = modal_area.bottom().saturating_sub(3);

        // Horizontal bottom divider
        if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
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

        // Center vertical divider
        let center_x = modal_area.left() + modal_width / 2;
        if modal_width >= 75
            && center_x > modal_area.left() + 25
            && center_x < modal_area.right().saturating_sub(25)
        {
            buf.set_string(
                center_x,
                modal_area.top(),
                symbols::line::NORMAL.horizontal_down,
                border_style,
            );
            for py in (modal_area.top() + 1)..sep_y {
                buf.set_string(center_x, py, symbols::line::NORMAL.vertical, border_style);
            }
            if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
                buf.set_string(
                    center_x,
                    sep_y,
                    symbols::line::NORMAL.horizontal_up,
                    border_style,
                );
            }
        }

        // Available vertical rows inside modal (excluding title, margins and footer)
        let inner_y = modal_area.top() + 1;
        let inner_height = sep_y.saturating_sub(inner_y);

        // Check if 2-column or 1-column layout fits best
        if modal_width >= 75 {
            // === 2-COLUMN SPACIOUS CATEGORIZED LAYOUT ===
            let left_col_width = (center_x.saturating_sub(modal_area.left() + 3)) as usize;
            let right_col_width = (modal_area.right().saturating_sub(center_x + 3)) as usize;
            let key_col_w_left = 32.min(left_col_width.saturating_sub(22));
            let key_col_w_right = 16.min(right_col_width.saturating_sub(22));

            // --- LEFT COLUMN ---
            let mut left_lines = Vec::new();

            // 1. Navigation & Interface
            left_lines.push(make_section_header(
                lang.t(I18nKey::HelpSectionNavigation),
                Color::Cyan,
                left_col_width,
            ));
            left_lines.push(Line::from(""));

            // Shift Tab ou Ctrl Espace
            let mut l_focus = key_pill(lang.t(I18nKey::HelpKeyShift), Color::Cyan);
            l_focus.push(Span::raw(" "));
            l_focus.extend(key_pill(lang.t(I18nKey::HelpKeyTab), Color::Cyan));
            l_focus.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
            l_focus.extend(key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan));
            l_focus.push(Span::raw(" "));
            l_focus.extend(key_pill(lang.t(I18nKey::HelpKeySpace), Color::Cyan));
            left_lines.push(make_help_row(
                l_focus,
                lang.t(I18nKey::HelpDescToggleFocus),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Clic Souris
            left_lines.push(make_help_row(
                key_pill(lang.t(I18nKey::HelpKeyMouseClick), Color::Cyan),
                lang.t(I18nKey::HelpDescMouseClick),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Molette / PgUp PgDn
            let mut l_scroll = key_pill(
                if lang == Language::Fr {
                    "🖱 Molette"
                } else {
                    "🖱 Scroll"
                },
                Color::Cyan,
            );
            l_scroll.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
            l_scroll.extend(key_pill("PgUp", Color::Cyan));
            l_scroll.push(Span::raw(" "));
            l_scroll.extend(key_pill("PgDn", Color::Cyan));
            left_lines.push(make_help_row(
                l_scroll,
                lang.t(I18nKey::HelpDescScroll),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Alt Left/Right ou Glisser
            let mut l_resize = key_pill(lang.t(I18nKey::HelpKeyAlt), Color::Cyan);
            l_resize.push(Span::raw(" "));
            l_resize.extend(key_pill("←/→", Color::Cyan));
            l_resize.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
            l_resize.extend(key_pill(lang.t(I18nKey::HelpKeyDrag), Color::Cyan));
            left_lines.push(make_help_row(
                l_resize,
                lang.t(I18nKey::HelpDescResizePanels),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Ctrl T (Nouvel onglet)
            let mut l_tab = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
            l_tab.push(Span::raw(" "));
            l_tab.extend(key_pill("T", Color::Cyan));
            left_lines.push(make_help_row(
                l_tab,
                lang.t(I18nKey::HelpDescNewTab),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Ctrl Tab (Changer d'onglet)
            let mut l_next_tab = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
            l_next_tab.push(Span::raw(" "));
            l_next_tab.extend(key_pill(lang.t(I18nKey::HelpKeyTab), Color::Cyan));
            left_lines.push(make_help_row(
                l_next_tab,
                lang.t(I18nKey::HelpDescNextTab),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Ctrl Shift W (Fermer l'onglet)
            let mut l_close_tab = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
            l_close_tab.push(Span::raw(" "));
            l_close_tab.extend(key_pill(lang.t(I18nKey::HelpKeyShift), Color::Cyan));
            l_close_tab.push(Span::raw(" "));
            l_close_tab.extend(key_pill("W", Color::Cyan));
            left_lines.push(make_help_row(
                l_close_tab,
                lang.t(I18nKey::HelpDescCloseTab),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Alt R (Renommer l'onglet)
            let mut l_rename_tab = key_pill(lang.t(I18nKey::HelpKeyAlt), Color::Cyan);
            l_rename_tab.push(Span::raw(" "));
            l_rename_tab.extend(key_pill("R", Color::Cyan));
            left_lines.push(make_help_row(
                l_rename_tab,
                lang.t(I18nKey::HelpDescRenameTab),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // 2. Agent IA & Diagnostic
            left_lines.push(make_section_header(
                lang.t(I18nKey::HelpSectionAgent),
                Color::Green,
                left_col_width,
            ));
            left_lines.push(Line::from(""));

            // F3 / Ctrl Y
            let mut l_f3 = key_pill("F3", Color::Green);
            l_f3.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
            l_f3.extend(key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Green));
            l_f3.push(Span::raw(" "));
            l_f3.extend(key_pill("Y", Color::Green));
            left_lines.push(make_help_row(
                l_f3,
                lang.t(I18nKey::HelpDescAutoApprove),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Alt D / Alt X
            let mut l_diag = key_pill(lang.t(I18nKey::HelpKeyAlt), Color::Yellow);
            l_diag.push(Span::raw(" "));
            l_diag.extend(key_pill("D", Color::Yellow));
            l_diag.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
            l_diag.extend(key_pill(lang.t(I18nKey::HelpKeyAlt), Color::DarkGray));
            l_diag.push(Span::raw(" "));
            l_diag.extend(key_pill("X", Color::DarkGray));
            left_lines.push(make_help_row(
                l_diag,
                lang.t(I18nKey::HelpDescDiagnoseError),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Ctrl M (MCP)
            let mut l_mcp = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Rgb(140, 100, 240));
            l_mcp.push(Span::raw(" "));
            l_mcp.extend(key_pill("M", Color::Rgb(140, 100, 240)));
            left_lines.push(make_help_row(
                l_mcp,
                lang.t(I18nKey::HelpDescMcpModal),
                key_col_w_left,
            ));
            left_lines.push(Line::from(""));

            // Ctrl F (Search)
            let mut l_srch = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Yellow);
            l_srch.push(Span::raw(" "));
            l_srch.extend(key_pill("F", Color::Yellow));
            left_lines.push(make_help_row(
                l_srch,
                lang.t(I18nKey::HelpDescChatSearch),
                key_col_w_left,
            ));

            let left_area = Rect::new(
                modal_area.left() + 2,
                inner_y,
                center_x.saturating_sub(modal_area.left() + 3),
                inner_height,
            );
            Paragraph::new(left_lines)
                .alignment(Alignment::Left)
                .render(left_area, buf);

            // --- RIGHT COLUMN ---
            let mut right_lines = Vec::new();

            // 3. Sessions & Hôtes
            right_lines.push(make_section_header(
                lang.t(I18nKey::HelpSectionSessions),
                Color::Yellow,
                right_col_width,
            ));
            right_lines.push(Line::from(""));

            // Ctrl P (Config)
            let mut l_cfg = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Magenta);
            l_cfg.push(Span::raw(" "));
            l_cfg.extend(key_pill("P", Color::Magenta));
            right_lines.push(make_help_row(
                l_cfg,
                lang.t(I18nKey::HelpDescConfigModal),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Ctrl B (Bookmarks)
            let mut l_bmk = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
            l_bmk.push(Span::raw(" "));
            l_bmk.extend(key_pill("B", Color::Cyan));
            right_lines.push(make_help_row(
                l_bmk,
                lang.t(I18nKey::HelpDescBookmarksModal),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Ctrl H (Sessions)
            let mut l_sess = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::LightCyan);
            l_sess.push(Span::raw(" "));
            l_sess.extend(key_pill("H", Color::LightCyan));
            right_lines.push(make_help_row(
                l_sess,
                lang.t(I18nKey::HelpDescSessionModal),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Ctrl E (Export)
            let mut l_exp = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Yellow);
            l_exp.push(Span::raw(" "));
            l_exp.extend(key_pill("E", Color::Yellow));
            right_lines.push(make_help_row(
                l_exp,
                lang.t(I18nKey::HelpDescExportSession),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Ctrl N (New session)
            let mut l_new = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
            l_new.push(Span::raw(" "));
            l_new.extend(key_pill("N", Color::Cyan));
            right_lines.push(make_help_row(
                l_new,
                lang.t(I18nKey::HelpDescNewSession),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // 4. Général & Contrôle
            right_lines.push(make_section_header(
                lang.t(I18nKey::HelpSectionGeneral),
                Color::LightCyan,
                right_col_width,
            ));
            right_lines.push(Line::from(""));

            // F1
            right_lines.push(make_help_row(
                key_pill("F1", Color::Cyan),
                lang.t(I18nKey::HelpDescToggleHelp),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Ctrl Q
            let mut l_quit = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Red);
            l_quit.push(Span::raw(" "));
            l_quit.extend(key_pill("Q", Color::Red));
            right_lines.push(make_help_row(
                l_quit,
                lang.t(I18nKey::HelpDescQuit),
                key_col_w_right,
            ));
            right_lines.push(Line::from(""));

            // Échap
            right_lines.push(make_help_row(
                key_pill(lang.t(I18nKey::HelpKeyClose), Color::DarkGray),
                lang.t(I18nKey::HelpDescCloseModal),
                key_col_w_right,
            ));

            let right_area = Rect::new(
                center_x + 2,
                inner_y,
                modal_area.right().saturating_sub(center_x + 3),
                inner_height,
            );
            Paragraph::new(right_lines)
                .alignment(Alignment::Left)
                .render(right_area, buf);
        } else {
            // === 1-COLUMN COMPACT LAYOUT FOR NARROW TERMINALS ===
            let col_width = modal_width.saturating_sub(4) as usize;
            let key_col_w = 18.min(col_width.saturating_sub(15));
            let lines = vec![
                make_help_row(
                    key_pill("Shift+Tab", Color::Cyan),
                    lang.t(I18nKey::HelpDescToggleFocus),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("F3", Color::Green),
                    lang.t(I18nKey::HelpDescAutoApprove),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+P", Color::Magenta),
                    lang.t(I18nKey::HelpDescConfigModal),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+B", Color::Cyan),
                    lang.t(I18nKey::HelpDescBookmarksModal),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+M", Color::Rgb(140, 100, 240)),
                    lang.t(I18nKey::HelpDescMcpModal),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+H", Color::LightCyan),
                    lang.t(I18nKey::HelpDescSessionModal),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+E", Color::Yellow),
                    lang.t(I18nKey::HelpDescExportSession),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+F", Color::Yellow),
                    lang.t(I18nKey::HelpDescChatSearch),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Alt+D", Color::Yellow),
                    lang.t(I18nKey::HelpDescDiagnoseError),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("F1", Color::Cyan),
                    lang.t(I18nKey::HelpDescToggleHelp),
                    key_col_w,
                ),
                make_help_row(
                    key_pill("Ctrl+Q", Color::Red),
                    lang.t(I18nKey::HelpDescQuit),
                    key_col_w,
                ),
            ];

            let content_area = Rect::new(
                modal_area.left() + 2,
                inner_y,
                modal_area.width.saturating_sub(4),
                inner_height,
            );
            Paragraph::new(lines)
                .alignment(Alignment::Left)
                .render(content_area, buf);
        }

        // --- FOOTER CLOSE PROMPT ---
        let mut footer = Vec::new();
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
