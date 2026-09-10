use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    symbols,
    text::Span,
    widgets::{Block, Borders, Clear, Widget},
};

use crate::i18n::{I18nKey, Language};
use crate::ui::theme::ThemeId;

#[derive(Debug, Clone)]
pub struct RenameTabModalState {
    pub tab_index: usize,
    pub input: String,
}

pub enum RenameTabAction {
    Close,
    Save(Option<String>),
}

impl RenameTabModalState {
    pub fn new(tab_index: usize, initial_title: String) -> Self {
        Self {
            tab_index,
            input: initial_title,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<RenameTabAction> {
        match key.code {
            KeyCode::Esc => Some(RenameTabAction::Close),
            KeyCode::Enter => {
                let trimmed = self.input.trim();
                let title = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                Some(RenameTabAction::Save(title))
            }
            KeyCode::Backspace => {
                self.input.pop();
                None
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.input.push(c);
                None
            }
            _ => None,
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        self.input.push_str(text);
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: ThemeId, lang: Language) {
        let palette = theme.palette();
        let modal_width = 54u16.min(area.width.saturating_sub(4));
        let modal_height = 7u16;

        let modal_x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let title = format!(
            " 🏷️ {} #{} ",
            lang.t(I18nKey::TabRenameTitle),
            self.tab_index + 1
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(palette.accent_primary))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(palette.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner.height >= 3 && inner.width >= 10 {
            // Label
            let label = lang.t(I18nKey::TabRenamePrompt);
            buf.set_string(
                inner.x + 1,
                inner.y,
                label,
                Style::default().fg(palette.text_secondary),
            );

            // Text input field box
            let input_area = Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1);
            let input_style = Style::default()
                .bg(palette.selection_bg)
                .fg(palette.text_primary);
            buf.set_style(input_area, input_style);

            let display_text = if self.input.is_empty() {
                " "
            } else {
                &self.input
            };
            buf.set_string(
                input_area.x + 1,
                input_area.y,
                display_text,
                input_style.add_modifier(Modifier::BOLD),
            );
            // Render cursor symbol
            let cursor_x = (input_area.x
                + 1
                + unicode_width::UnicodeWidthStr::width(self.input.as_str()) as u16)
                .min(input_area.right().saturating_sub(1));
            buf.set_string(
                cursor_x,
                input_area.y,
                "█",
                Style::default().fg(palette.accent_primary),
            );

            // Help line
            let help_text = lang.t(I18nKey::TabRenameHelp);
            buf.set_string(
                inner.x + 1,
                inner.y + 3,
                help_text,
                Style::default().fg(palette.text_secondary),
            );
        }
    }
}
