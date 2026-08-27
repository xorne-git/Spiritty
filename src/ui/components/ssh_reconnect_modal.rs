use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};

use crate::i18n::{I18nKey, Language};

/// Small centered modal offering to reconnect to the SSH host of a `-c`-resumed
/// session while the PTY is still local. Enter accepts, Esc dismisses (the 🏷️
/// resumed hint stays in the terminal title either way).
pub struct SshReconnectModal;

impl SshReconnectModal {
    pub fn render_modal(area: Rect, buf: &mut Buffer, target: &str, lang: Language) {
        // Compact box: wide enough for the target, never larger than the screen.
        let modal_width = 48u16.min(area.width.saturating_sub(4)).max(24);
        let modal_height = 7u16.min(area.height.saturating_sub(2)).max(5);
        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let border_style = Style::default().fg(Color::Cyan);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(border_style)
            .padding(Padding::horizontal(1))
            .title(Span::styled(
                lang.t(I18nKey::SshReconnectTitle),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        // Target line: truncated to the inner width to never overflow the box.
        let inner_w = (modal_width.saturating_sub(4)) as usize;
        let target_disp: String = if target.chars().count() > inner_w {
            target
                .chars()
                .take(inner_w.saturating_sub(1))
                .collect::<String>()
                + "…"
        } else {
            target.to_string()
        };

        // Confirm hint stands out in green (the action we want); dismiss stays dim.
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                lang.t(I18nKey::SshReconnectBody),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                target_disp,
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    lang.t(I18nKey::SshReconnectConfirm),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   ·   {}", lang.t(I18nKey::SshReconnectLater)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];

        Paragraph::new(lines).block(block).render(modal_area, buf);
    }
}
