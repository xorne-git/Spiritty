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
        Span::styled(key, Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::styled("", Style::default().fg(color)),
    ]
}

impl HelpModal {
    pub fn render_modal(area: Rect, buf: &mut Buffer, lang: Language) {
        let modal_width = 92.min(area.width.saturating_sub(4));
        let modal_height = 23.min(area.height.saturating_sub(2));

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
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));

        block.render(modal_area, buf);

        let border_style = Style::default().fg(Color::Cyan);
        let desc_style = Style::default().fg(Color::White);

        // Position of vertical separator (with generous spacing on both sides)
        let sep_x = modal_area.left() + 45;
        let sep_y = modal_area.bottom().saturating_sub(5);

        // 1. Full-Width Horizontal Separator Line
        if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
            buf.set_string(modal_area.left(), sep_y, symbols::line::NORMAL.vertical_right, border_style);
            for px in (modal_area.left() + 1)..(modal_area.right().saturating_sub(1)) {
                buf.set_string(px, sep_y, symbols::line::NORMAL.horizontal, border_style);
            }
            buf.set_string(modal_area.right().saturating_sub(1), sep_y, symbols::line::NORMAL.vertical_left, border_style);
        }

        // 2. Full Vertical Separator Line with clean junctions (┬ and ┴)
        if sep_x > modal_area.left() && sep_x < modal_area.right().saturating_sub(1) {
            buf.set_string(sep_x, modal_area.top(), symbols::line::NORMAL.horizontal_down, border_style);
            for py in (modal_area.top() + 1)..sep_y {
                buf.set_string(sep_x, py, symbols::line::NORMAL.vertical, border_style);
            }
            if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
                buf.set_string(sep_x, sep_y, symbols::line::NORMAL.horizontal_up, border_style);
            }
        }

        // 3. Left Column: Shortcut Keys (Starting at top + 2 for top padding)
        let mut left_lines = Vec::new();

        // 1. Shift Tab ou Ctrl Espace
        let mut l1 = key_pill(lang.t(I18nKey::HelpKeyShift), Color::Cyan);
        l1.push(Span::raw(" "));
        l1.extend(key_pill(lang.t(I18nKey::HelpKeyTab), Color::Cyan));
        l1.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
        l1.extend(key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan));
        l1.push(Span::raw(" "));
        l1.extend(key_pill(lang.t(I18nKey::HelpKeySpace), Color::Cyan));
        left_lines.push(Line::from(l1));
        left_lines.push(Line::from(""));

        // 2. Clic Souris
        left_lines.push(Line::from(key_pill(lang.t(I18nKey::HelpKeyMouseClick), Color::Cyan)));
        left_lines.push(Line::from(""));

        // 3. Molette ou PgUp / PgDn
        let mut l_scroll = key_pill(if lang == Language::Fr { "🖱 Molette" } else { "🖱 Scroll" }, Color::Cyan);
        l_scroll.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
        l_scroll.extend(key_pill("PgUp", Color::Cyan));
        l_scroll.push(Span::raw(" "));
        l_scroll.extend(key_pill("PgDn", Color::Cyan));
        left_lines.push(Line::from(l_scroll));
        left_lines.push(Line::from(""));

        // 4. Alt Left / Right ou Glisser
        let mut l3 = key_pill(lang.t(I18nKey::HelpKeyAlt), Color::Cyan);
        l3.push(Span::raw(" "));
        l3.extend(key_pill("←", Color::Cyan));
        l3.push(Span::raw(" "));
        l3.extend(key_pill("→", Color::Cyan));
        l3.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
        l3.extend(key_pill(lang.t(I18nKey::HelpKeyDrag), Color::Cyan));
        left_lines.push(Line::from(l3));
        left_lines.push(Line::from(""));

        // 5. F3 ou Ctrl Y (Auto-Approve)
        let mut l_f3 = key_pill("F3", Color::Green);
        l_f3.push(Span::raw(lang.t(I18nKey::HelpKeyOr)));
        l_f3.extend(key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Green));
        l_f3.push(Span::raw(" "));
        l_f3.extend(key_pill("Y", Color::Green));
        left_lines.push(Line::from(l_f3));
        left_lines.push(Line::from(""));

        // 6. Ctrl P
        let mut l4 = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
        l4.push(Span::raw(" "));
        l4.extend(key_pill("P", Color::Cyan));
        left_lines.push(Line::from(l4));
        left_lines.push(Line::from(""));

        // 7. Ctrl H
        let mut l_h = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
        l_h.push(Span::raw(" "));
        l_h.extend(key_pill("H", Color::Cyan));
        left_lines.push(Line::from(l_h));
        left_lines.push(Line::from(""));

        // 8. Ctrl N
        let mut l_n = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Cyan);
        l_n.push(Span::raw(" "));
        l_n.extend(key_pill("N", Color::Cyan));
        left_lines.push(Line::from(l_n));
        left_lines.push(Line::from(""));

        // 9. Alt S (Scan VPS)
        let mut l_s = key_pill("Alt", Color::Yellow);
        l_s.push(Span::raw(" "));
        l_s.extend(key_pill("S", Color::Yellow));
        left_lines.push(Line::from(l_s));
        left_lines.push(Line::from(""));

        // 10. F1
        left_lines.push(Line::from(key_pill("F1", Color::Cyan)));
        left_lines.push(Line::from(""));

        // 11. Ctrl Q
        let mut l6 = key_pill(lang.t(I18nKey::HelpKeyCtrl), Color::Red);
        l6.push(Span::raw(" "));
        l6.extend(key_pill("Q", Color::Red));
        left_lines.push(Line::from(l6));
        left_lines.push(Line::from(""));

        // 12. Échap
        left_lines.push(Line::from(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Cyan)));

        let left_area = Rect::new(
            modal_area.left() + 3,
            modal_area.top() + 2,
            sep_x.saturating_sub(modal_area.left() + 4),
            sep_y.saturating_sub(modal_area.top() + 2),
        );
        let p_left = Paragraph::new(left_lines).alignment(Alignment::Left);
        p_left.render(left_area, buf);

        // 4. Right Column: Descriptions (Starting at sep_x + 3 for left padding)
        let right_lines = vec![
            Line::from(Span::styled(lang.t(I18nKey::HelpDescToggleFocus), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescMouseClick), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescScroll), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescResizePanels), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescAutoApprove), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescConfigModal), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescSessionModal), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescNewSession), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescScanHost), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescToggleHelp), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescQuit), desc_style)),
            Line::from(""),
            Line::from(Span::styled(lang.t(I18nKey::HelpDescCloseModal), desc_style)),
        ];

        let right_area = Rect::new(
            sep_x + 3,
            modal_area.top() + 2,
            modal_area.right().saturating_sub(sep_x + 5),
            sep_y.saturating_sub(modal_area.top() + 2),
        );
        let p_right = Paragraph::new(right_lines).alignment(Alignment::Left);
        p_right.render(right_area, buf);

        // 5. Footer: Close Prompt (Vertically and Horizontally Centered)
        let mut footer = Vec::new();
        footer.push(Span::styled(lang.t(I18nKey::HelpFooterPromptPrefix), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)));
        footer.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::DarkGray));
        footer.push(Span::styled(lang.t(I18nKey::HelpFooterPromptMiddle), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)));
        footer.extend(key_pill("F1", Color::DarkGray));
        footer.push(Span::styled(lang.t(I18nKey::HelpFooterPromptSuffix), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)));

        let footer_area = Rect::new(
            modal_area.left() + 2,
            sep_y + 2,
            modal_area.width.saturating_sub(4),
            1,
        );
        let p_bottom = Paragraph::new(Line::from(footer)).alignment(Alignment::Center);
        p_bottom.render(footer_area, buf);
    }
}
