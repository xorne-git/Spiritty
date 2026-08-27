use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};

use crate::i18n::Language;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportModalAction {
    Export(String),
    Close,
}

pub struct ExportModalState {
    pub file_path_input: String,
    pub cursor: usize,
}

impl ExportModalState {
    pub fn new(default_path: String) -> Self {
        let cursor = default_path.chars().count();
        Self {
            file_path_input: default_path,
            cursor,
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        let clean = text.replace(['\r', '\n'], "");
        if clean.is_empty() {
            return;
        }
        for c in clean.chars() {
            if !c.is_control() {
                let mut chars: Vec<char> = self.file_path_input.chars().collect();
                if self.cursor <= chars.len() {
                    chars.insert(self.cursor, c);
                    self.file_path_input = chars.into_iter().collect();
                    self.cursor += 1;
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ExportModalAction> {
        let is_paste = (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            || key.modifiers.contains(
                crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
            ))
            && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'));

        if is_paste {
            if let Some(text) = crate::system::clipboard::get_clipboard_text() {
                self.handle_paste(text);
                return None;
            }
        }

        match key.code {
            KeyCode::Esc => Some(ExportModalAction::Close),
            KeyCode::Enter => {
                let trimmed = self.file_path_input.trim().to_string();
                if !trimmed.is_empty() {
                    Some(ExportModalAction::Export(trimmed))
                } else {
                    None
                }
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let mut chars: Vec<char> = self.file_path_input.chars().collect();
                    chars.remove(self.cursor - 1);
                    self.file_path_input = chars.into_iter().collect();
                    self.cursor -= 1;
                }
                None
            }
            KeyCode::Delete => {
                if self.cursor < self.file_path_input.chars().count() {
                    let mut chars: Vec<char> = self.file_path_input.chars().collect();
                    chars.remove(self.cursor);
                    self.file_path_input = chars.into_iter().collect();
                }
                None
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            KeyCode::Right => {
                if self.cursor < self.file_path_input.chars().count() {
                    self.cursor += 1;
                }
                None
            }
            KeyCode::Home => {
                self.cursor = 0;
                None
            }
            KeyCode::End => {
                self.cursor = self.file_path_input.chars().count();
                None
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
            {
                let mut chars: Vec<char> = self.file_path_input.chars().collect();
                chars.insert(self.cursor, c);
                self.file_path_input = chars.into_iter().collect();
                self.cursor += 1;
                None
            }
            _ => None,
        }
    }
}

pub struct ExportModal;

impl ExportModal {
    pub fn render_modal(
        parent_area: Rect,
        buf: &mut Buffer,
        state: &ExportModalState,
        lang: Language,
    ) {
        let modal_width = (parent_area.width.saturating_sub(10)).clamp(64, 90);
        let modal_height = 10;

        let modal_x = parent_area.left() + (parent_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = parent_area.top() + (parent_area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let title = if lang == Language::Fr {
            " 📝 Exporter la Session en Markdown "
        } else {
            " 📝 Export Session to Markdown "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::horizontal(1))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(1), // Prompt label
            Constraint::Length(2), // Input field
            Constraint::Length(1), // Hint text
            Constraint::Length(1), // Separator line
            Constraint::Length(1), // Footer actions
        ])
        .split(inner_area);

        // 1. Prompt label
        let prompt_text = if lang == Language::Fr {
            "Chemin du fichier de destination :"
        } else {
            "Destination file path:"
        };
        let p_label = Paragraph::new(Line::from(vec![
            Span::styled(
                "❯ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                prompt_text,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        p_label.render(chunks[0], buf);

        // 2. Input field
        let input_box = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Yellow));
        let input_inner = input_box.inner(chunks[1]);
        input_box.render(chunks[1], buf);

        let input_spans =
            render_editable_text(&state.file_path_input, state.cursor, true, "~/session.md");
        let p_input = Paragraph::new(Line::from(input_spans));
        p_input.render(input_inner, buf);

        // 3. Hint text
        let hint_text = if lang == Language::Fr {
            "💡 Par défaut dans votre dossier personnel (~) ou répertoire configuré."
        } else {
            "💡 Defaults to your home directory (~) or configured export folder."
        };
        let p_hint = Paragraph::new(Line::from(Span::styled(
            hint_text,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
        p_hint.render(chunks[2], buf);

        // 4. Separator Line
        let sep_y = chunks[3].top();
        let border_style = Style::default().fg(Color::Cyan);
        buf.set_string(
            modal_area.left(),
            sep_y,
            symbols::line::NORMAL.vertical_right,
            border_style,
        );
        for x in (modal_area.left() + 1)..(modal_area.right().saturating_sub(1)) {
            buf.set_string(x, sep_y, symbols::line::NORMAL.horizontal, border_style);
        }
        buf.set_string(
            modal_area.right().saturating_sub(1),
            sep_y,
            symbols::line::NORMAL.vertical_left,
            border_style,
        );

        // 5. Footer actions
        let mut footer = Vec::new();
        footer.extend(key_pill("Enter", Color::Green));
        footer.push(Span::raw(if lang == Language::Fr {
            " Valider et Exporter   "
        } else {
            " Confirm & Export   "
        }));

        footer.extend(key_pill("Esc", Color::DarkGray));
        footer.push(Span::raw(if lang == Language::Fr {
            " Annuler"
        } else {
            " Cancel"
        }));

        let p_footer = Paragraph::new(Line::from(footer)).alignment(Alignment::Center);
        p_footer.render(chunks[4], buf);
    }
}

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

fn render_editable_text<'a>(
    text: &'a str,
    cursor: usize,
    is_focused: bool,
    placeholder: &'a str,
) -> Vec<Span<'a>> {
    if !is_focused {
        if text.is_empty() {
            return vec![Span::styled(
                placeholder,
                Style::default().fg(Color::DarkGray),
            )];
        }
        return vec![Span::styled(text, Style::default().fg(Color::White))];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();

    if chars.is_empty() {
        spans.push(Span::styled(
            " ",
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ));
        return spans;
    }

    let cursor = cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    if !before.is_empty() {
        spans.push(Span::styled(
            before,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if cursor < chars.len() {
        let cur_char = chars[cursor].to_string();
        spans.push(Span::styled(
            cur_char,
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        let after: String = chars[(cursor + 1)..].iter().collect();
        if !after.is_empty() {
            spans.push(Span::styled(
                after,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    } else {
        spans.push(Span::styled(
            " ",
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ));
    }

    spans
}
