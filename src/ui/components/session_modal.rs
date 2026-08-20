use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, Widget},
};

use crate::{
    i18n::{I18nKey, Language},
    session::{SessionHeader, SessionStorage},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionModalAction {
    Load(String),
    NewSession,
    Close,
}

pub struct SessionModalState {
    pub sessions: Vec<SessionHeader>,
    pub selected_index: usize,
    pub confirm_delete_id: Option<String>,
    pub current_session_id: String,
    pub status_message: Option<(std::time::Instant, String)>,
}

impl SessionModalState {
    pub fn new(current_session_id: String) -> Self {
        let sessions = SessionStorage::list_sessions().unwrap_or_default();
        let selected_index = sessions
            .iter()
            .position(|s| s.id == current_session_id)
            .unwrap_or(0);

        Self {
            sessions,
            selected_index,
            confirm_delete_id: None,
            current_session_id,
            status_message: None,
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(sessions) = SessionStorage::list_sessions() {
            self.sessions = sessions;
            if self.selected_index >= self.sessions.len() && !self.sessions.is_empty() {
                self.selected_index = self.sessions.len() - 1;
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<SessionModalAction> {
        // Confirmation mode for deletion
        if let Some(id) = self.confirm_delete_id.clone() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter => {
                    let _ = SessionStorage::delete(&id);
                    self.confirm_delete_id = None;
                    self.refresh();
                    return None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_delete_id = None;
                    return None;
                }
                _ => return None,
            }
        }

        match key.code {
            KeyCode::Esc => Some(SessionModalAction::Close),
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.sessions.is_empty() {
                    if self.selected_index == 0 {
                        self.selected_index = self.sessions.len() - 1;
                    } else {
                        self.selected_index = self.selected_index.saturating_sub(1);
                    }
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.sessions.is_empty() {
                    if self.selected_index + 1 >= self.sessions.len() {
                        self.selected_index = 0;
                    } else {
                        self.selected_index += 1;
                    }
                }
                None
            }
            KeyCode::Enter => {
                self.sessions.get(self.selected_index).map(|sess| SessionModalAction::Load(sess.id.clone()))
            }
            KeyCode::Char('n') | KeyCode::Char('N') => Some(SessionModalAction::NewSession),
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                if let Some(sess) = self.sessions.get(self.selected_index) {
                    self.confirm_delete_id = Some(sess.id.clone());
                }
                None
            }
            _ => None,
        }
    }

    pub fn render_modal(&self, area: Rect, buf: &mut Buffer, lang: Language) {
        let popup_w = (area.width * 85 / 100).clamp(50, 110);
        let popup_h = (area.height * 80 / 100).clamp(16, 32);

        let popup_x = (area.width.saturating_sub(popup_w)) / 2;
        let popup_y = (area.height.saturating_sub(popup_h)) / 2;
        let modal_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

        Clear.render(modal_area, buf);

        let block = Block::default()
            .title(Span::styled(
                lang.t(I18nKey::SessionModalTitle),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::uniform(1));

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner_area.height < 4 || inner_area.width < 10 {
            return;
        }

        // Split vertically into: Header / Table list / Footer actions
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner_area);

        let header_area = chunks[0];
        let list_area = chunks[1];
        let footer_area = chunks[2];

        // 1. Render Count Header
        let count_text = format!(
            "Total : {} session(s) enregistrée(s)",
            self.sessions.len()
        );
        let header_line = Line::from(vec![
            Span::styled(count_text, Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]);
        Paragraph::new(header_line).render(header_area, buf);

        // 2. Render Sessions Table
        if self.sessions.is_empty() {
            let empty_text = lang.t(I18nKey::SessionEmptyList);
            let p = Paragraph::new(Line::from(vec![
                Span::styled(empty_text, Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
            ]));
            p.render(list_area, buf);
        } else {
            let visible_rows_count = list_area.height.saturating_sub(1) as usize;
            let scroll_offset = if self.selected_index >= visible_rows_count {
                self.selected_index - visible_rows_count + 1
            } else {
                0
            };

            let table_headers = Row::new(vec![
                Span::styled(lang.t(I18nKey::SessionHeaderTitle), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::styled(lang.t(I18nKey::SessionHeaderModel), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::styled(lang.t(I18nKey::SessionHeaderMessages), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::styled(lang.t(I18nKey::SessionHeaderTokens), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::styled(lang.t(I18nKey::SessionHeaderDate), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
            ])
            .bottom_margin(0);

            let rows: Vec<Row> = self
                .sessions
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(visible_rows_count)
                .map(|(idx, sess)| {
                    let is_selected = idx == self.selected_index;
                    let is_active = sess.id == self.current_session_id;

                    let mut title_spans = Vec::new();
                    if is_selected {
                        title_spans.push(Span::styled("❯ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                    } else {
                        title_spans.push(Span::raw("  "));
                    }

                    title_spans.push(Span::styled(
                        sess.title.clone(),
                        if is_selected {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ));

                    if is_active {
                        title_spans.push(Span::raw(" "));
                        title_spans.push(Span::styled(
                            lang.t(I18nKey::SessionTagActive),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                    }

                    let row_style = if is_selected {
                        Style::default().bg(Color::Rgb(30, 50, 80))
                    } else {
                        Style::default()
                    };

                    Row::new(vec![
                        Line::from(title_spans),
                        Line::from(Span::styled(sess.model.clone(), Style::default().fg(Color::Cyan))),
                        Line::from(Span::styled(format!("{}", sess.message_count), Style::default().fg(Color::Gray))),
                        Line::from(Span::styled(format_tokens(sess.total_tokens), Style::default().fg(Color::Yellow))),
                        Line::from(Span::styled(sess.updated_at.clone(), Style::default().fg(Color::DarkGray))),
                    ])
                    .style(row_style)
                })
                .collect();

            let widths = [
                Constraint::Percentage(42),
                Constraint::Percentage(20),
                Constraint::Percentage(8),
                Constraint::Percentage(12),
                Constraint::Percentage(18),
            ];

            let table = Table::new(rows, widths).header(table_headers);
            table.render(list_area, buf);
        }

        // 3. Render Footer Actions / Confirmation Prompt
        if let Some(ref _id) = self.confirm_delete_id {
            let confirm_line = Line::from(vec![
                Span::styled(" ⚠️  ", Style::default().fg(Color::Red)),
                Span::styled(
                    lang.t(I18nKey::SessionConfirmDelete),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]);
            Paragraph::new(confirm_line).render(footer_area, buf);
        } else {
            let mut footer_spans = Vec::new();
            footer_spans.extend(key_pill("↵", Color::Cyan));
            footer_spans.push(Span::styled(format!(" {}   ", lang.t(I18nKey::SessionActionLoad)), Style::default().fg(Color::White)));

            footer_spans.extend(key_pill("N", Color::Green));
            footer_spans.push(Span::styled(format!(" {}   ", lang.t(I18nKey::SessionActionNew)), Style::default().fg(Color::White)));

            footer_spans.extend(key_pill("D", Color::Red));
            footer_spans.push(Span::styled(format!(" {}   ", lang.t(I18nKey::SessionActionDelete)), Style::default().fg(Color::White)));

            footer_spans.extend(key_pill("Esc", Color::DarkGray));
            footer_spans.push(Span::styled(format!(" {}", lang.t(I18nKey::SessionActionClose)), Style::default().fg(Color::White)));

            Paragraph::new(Line::from(footer_spans)).render(footer_area, buf);
        }
    }
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

fn format_tokens(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}
