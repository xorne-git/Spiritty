use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, Widget},
};

use crate::{
    i18n::{I18nKey, Language},
    system::{HostEntry, HostsStore},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarksModalAction {
    Connect(String),
    TriggerScan,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddHostState {
    None,
    EnteringTarget {
        input: String,
        cursor: usize,
    },
    EnteringAlias {
        target: String,
        input: String,
        cursor: usize,
    },
}

pub struct BookmarksModalState {
    pub entries: Vec<HostEntry>,
    pub selected_index: usize,
    pub add_state: AddHostState,
    pub confirm_delete_target: Option<String>,
    pub active_ssh_target: Option<String>,
}

impl BookmarksModalState {
    pub fn new(hosts_store: &HostsStore, active_ssh_target: Option<String>) -> Self {
        let entries = hosts_store.list_all_entries();
        Self {
            entries,
            selected_index: 0,
            add_state: AddHostState::None,
            confirm_delete_target: None,
            active_ssh_target,
        }
    }

    pub fn is_target_bookmarked(&self, target: &str) -> bool {
        self.entries.iter().any(|e| e.target == target)
    }

    pub fn refresh(&mut self, hosts_store: &HostsStore) {
        self.entries = hosts_store.list_all_entries();
        if self.selected_index >= self.entries.len() && !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        let clean = text.replace(['\r', '\n'], " ");
        if clean.is_empty() {
            return;
        }

        match &mut self.add_state {
            AddHostState::EnteringTarget { input, cursor }
            | AddHostState::EnteringAlias { input, cursor, .. } => {
                for c in clean.chars() {
                    if !c.is_control() {
                        let mut chars: Vec<char> = input.chars().collect();
                        if *cursor <= chars.len() {
                            chars.insert(*cursor, c);
                            *input = chars.into_iter().collect();
                            *cursor += 1;
                        }
                    }
                }
            }
            AddHostState::None => {}
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        hosts_store: &mut HostsStore,
    ) -> Option<BookmarksModalAction> {
        // Confirmation for delete
        if let Some(target) = self.confirm_delete_target.clone() {
            match key.code {
                KeyCode::Char('y')
                | KeyCode::Char('Y')
                | KeyCode::Char('o')
                | KeyCode::Char('O')
                | KeyCode::Enter => {
                    let _ = hosts_store.remove_bookmark(&target);
                    self.confirm_delete_target = None;
                    self.refresh(hosts_store);
                    return None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_delete_target = None;
                    return None;
                }
                _ => return None,
            }
        }

        // Check for Ctrl+V / Shift+Ctrl+V clipboard paste in add wizard
        let is_paste = (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            || key.modifiers.contains(
                crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
            ))
            && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'));

        if is_paste && !matches!(self.add_state, AddHostState::None) {
            if let Some(text) = crate::system::clipboard::get_clipboard_text_timeout(
                std::time::Duration::from_millis(1000),
            ) {
                self.handle_paste(text);
                return None;
            }
        }

        // Add Host Wizard
        match &mut self.add_state {
            AddHostState::EnteringTarget { input, cursor } => {
                match key.code {
                    KeyCode::Esc => {
                        self.add_state = AddHostState::None;
                        return None;
                    }
                    KeyCode::Enter => {
                        let target = input.trim().to_string();
                        if !target.is_empty() {
                            self.add_state = AddHostState::EnteringAlias {
                                target,
                                input: String::new(),
                                cursor: 0,
                            };
                        }
                        return None;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            let mut chars: Vec<char> = input.chars().collect();
                            chars.remove(*cursor - 1);
                            *input = chars.into_iter().collect();
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            let mut chars: Vec<char> = input.chars().collect();
                            chars.remove(*cursor);
                            *input = chars.into_iter().collect();
                        }
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                            && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                    {
                        let mut chars: Vec<char> = input.chars().collect();
                        chars.insert(*cursor, c);
                        *input = chars.into_iter().collect();
                        *cursor += 1;
                    }
                    _ => {}
                }
                return None;
            }
            AddHostState::EnteringAlias {
                target,
                input,
                cursor,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.add_state = AddHostState::None;
                        return None;
                    }
                    KeyCode::Enter => {
                        let alias = if input.trim().is_empty() {
                            None
                        } else {
                            Some(input.trim().to_string())
                        };
                        let _ = hosts_store.add_bookmark(target.clone(), alias);
                        self.add_state = AddHostState::None;
                        self.refresh(hosts_store);
                        return None;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            let mut chars: Vec<char> = input.chars().collect();
                            chars.remove(*cursor - 1);
                            *input = chars.into_iter().collect();
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            let mut chars: Vec<char> = input.chars().collect();
                            chars.remove(*cursor);
                            *input = chars.into_iter().collect();
                        }
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                            && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                    {
                        let mut chars: Vec<char> = input.chars().collect();
                        chars.insert(*cursor, c);
                        *input = chars.into_iter().collect();
                        *cursor += 1;
                    }
                    _ => {}
                }
                return None;
            }
            AddHostState::None => {}
        }

        // Navigation mode
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                Some(BookmarksModalAction::Close)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else if !self.entries.is_empty() {
                    self.selected_index = self.entries.len() - 1;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.entries.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.entries.len();
                }
                None
            }
            KeyCode::Enter => self
                .entries
                .get(self.selected_index)
                .map(|entry| BookmarksModalAction::Connect(entry.target.clone())),
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.add_state = AddHostState::EnteringTarget {
                    input: String::new(),
                    cursor: 0,
                };
                None
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Some(ref target) = self.active_ssh_target {
                    if self.is_target_bookmarked(target) {
                        let _ = hosts_store.toggle_favorite(target);
                        self.refresh(hosts_store);
                    } else {
                        self.add_state = AddHostState::EnteringAlias {
                            target: target.clone(),
                            input: String::new(),
                            cursor: 0,
                        };
                    }
                }
                None
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if self.active_ssh_target.is_some() {
                    return Some(BookmarksModalAction::TriggerScan);
                }
                None
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if let Some(entry) = self.entries.get(self.selected_index) {
                    let _ = hosts_store.toggle_favorite(&entry.target);
                    self.refresh(hosts_store);
                }
                None
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                if let Some(entry) = self.entries.get(self.selected_index) {
                    self.confirm_delete_target = Some(entry.target.clone());
                }
                None
            }
            _ => None,
        }
    }
}

pub struct BookmarksModal;

impl BookmarksModal {
    pub fn render_modal(
        parent_area: Rect,
        buf: &mut Buffer,
        state: &BookmarksModalState,
        lang: Language,
    ) {
        let modal_width = (parent_area.width.saturating_sub(4)).clamp(64, 110);
        let modal_height = (parent_area.height.saturating_sub(4)).clamp(16, 26);

        let modal_x = parent_area.left() + (parent_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = parent_area.top() + (parent_area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let title = if lang == Language::Fr {
            " 🌐 Serveurs SSH & Favoris "
        } else {
            " 🌐 SSH Servers & Bookmarks "
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

        let has_active_ssh = state.active_ssh_target.is_some();
        let (banner_area, table_area, sep_area, footer_area) = if has_active_ssh {
            let chunks = Layout::vertical([
                Constraint::Length(1), // Active server banner
                Constraint::Min(4),    // Table
                Constraint::Length(1), // Separator
                Constraint::Length(1), // Footer
            ])
            .split(inner_area);
            (Some(chunks[0]), chunks[1], chunks[2], chunks[3])
        } else {
            let chunks = Layout::vertical([
                Constraint::Min(4),    // Table
                Constraint::Length(1), // Separator
                Constraint::Length(1), // Footer
            ])
            .split(inner_area);
            (None, chunks[0], chunks[1], chunks[2])
        };

        if let (Some(b_area), Some(ref target)) = (banner_area, &state.active_ssh_target) {
            let is_bookmarked = state.is_target_bookmarked(target);
            let mut banner_spans = vec![
                Span::styled(" 🌐 ", Style::default().fg(Color::LightGreen)),
                Span::styled(
                    if lang == Language::Fr {
                        "Hôte actif : "
                    } else {
                        "Active host: "
                    },
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    target.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            banner_spans.extend(key_pill("S", Color::Yellow));
            banner_spans.push(Span::styled(
                if is_bookmarked {
                    if lang == Language::Fr {
                        " Déjà dans vos favoris ★"
                    } else {
                        " Already in bookmarks ★"
                    }
                } else if lang == Language::Fr {
                    " Sauvegarder l'hôte"
                } else {
                    " Save host to bookmarks"
                },
                Style::default().fg(Color::White),
            ));
            let p_banner = Paragraph::new(Line::from(banner_spans));
            p_banner.render(b_area, buf);
        }

        // 1. Check if adding or deleting
        if let Some(ref target) = state.confirm_delete_target {
            let msg = if lang == Language::Fr {
                format!("⚠️ Supprimer le serveur '{}' des favoris ? (O/n)", target)
            } else {
                format!("⚠️ Delete server '{}' from bookmarks? (Y/n)", target)
            };
            let p = Paragraph::new(Line::from(vec![Span::styled(
                msg,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]))
            .alignment(Alignment::Center);
            p.render(table_area, buf);
            return;
        }

        match &state.add_state {
            AddHostState::EnteringTarget { input, cursor } => {
                let mut lines = Vec::new();
                lines.push(Line::from(Span::styled(
                    if lang == Language::Fr { "＋ Ajouter un serveur SSH (ex: root@192.168.1.100 ou debian@vps.com:22022) :" }
                    else { "＋ Add SSH Server (e.g. root@192.168.1.100 or debian@vps.com:22022) :" },
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Cyan))];
                l.extend(render_editable_text(input, *cursor, true, "user@host"));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![Span::styled(
                    "[Enter] Suivant    [Esc] Annuler",
                    Style::default().fg(Color::DarkGray),
                )]));
                let p = Paragraph::new(lines);
                p.render(table_area, buf);
                return;
            }
            AddHostState::EnteringAlias {
                target,
                input,
                cursor,
            } => {
                let mut lines = Vec::new();
                lines.push(Line::from(Span::styled(
                    if lang == Language::Fr {
                        format!(
                            "✎ Donner un alias/nom à '{}' (laisser vide si aucun) :",
                            target
                        )
                    } else {
                        format!("✎ Set an alias/label for '{}' (optional) :", target)
                    },
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Cyan))];
                l.extend(render_editable_text(
                    input,
                    *cursor,
                    true,
                    "prod-web, vps-db, etc.",
                ));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![Span::styled(
                    "[Enter] Enregistrer    [Esc] Annuler",
                    Style::default().fg(Color::DarkGray),
                )]));
                let p = Paragraph::new(lines);
                p.render(table_area, buf);
                return;
            }
            AddHostState::None => {}
        }

        // Table of entries
        if state.entries.is_empty() {
            let msg = if lang == Language::Fr {
                "Aucun serveur SSH enregistré. Appuyez sur [A] pour en ajouter un."
            } else {
                "No SSH servers saved yet. Press [A] to add one."
            };
            let p = Paragraph::new(Line::from(vec![Span::styled(
                msg,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]))
            .alignment(Alignment::Center);
            p.render(table_area, buf);
        } else {
            let header_row = Row::new(vec![
                Span::styled(
                    "Fav",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if lang == Language::Fr {
                        "Alias / Nom"
                    } else {
                        "Alias / Label"
                    },
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if lang == Language::Fr {
                        "Cible SSH"
                    } else {
                        "SSH Target"
                    },
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if lang == Language::Fr {
                        "Distribution / OS"
                    } else {
                        "Distro / OS"
                    },
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
            .bottom_margin(1);

            // Scroll so the selected row stays visible when the list exceeds the table height.
            let total = state.entries.len();
            let visible_rows = (table_area.height.saturating_sub(2)).max(1) as usize;
            let sel = state.selected_index.min(total.saturating_sub(1));
            let mut offset = 0usize;
            if total > visible_rows {
                if sel >= visible_rows {
                    offset = sel - visible_rows + 1;
                }
                offset = offset.min(total - visible_rows);
            }

            let rows: Vec<Row> = state
                .entries
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible_rows)
                .map(|(idx, entry)| {
                    let is_sel = idx == state.selected_index;
                    let fav_icon = if entry.is_favorite { " ★ " } else { " ☆ " };
                    let fav_color = if entry.is_favorite {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    };

                    let alias_text = entry.alias.clone().unwrap_or_else(|| "-".to_string());
                    let distro_text = if let Some(ref prof) = entry.profile {
                        prof.distro.clone()
                    } else {
                        "Non scanné".to_string()
                    };

                    let style = if is_sel {
                        Style::default()
                            .bg(Color::Cyan)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    Row::new(vec![
                        Span::styled(
                            fav_icon,
                            if is_sel {
                                style
                            } else {
                                Style::default().fg(fav_color)
                            },
                        ),
                        Span::styled(alias_text, style),
                        Span::styled(entry.target.clone(), style),
                        Span::styled(distro_text, style),
                    ])
                })
                .collect();

            let widths = [
                Constraint::Length(5),
                Constraint::Percentage(25),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ];

            let table = Table::new(rows, widths).header(header_row);
            table.render(table_area, buf);
        }

        // Full-Width Horizontal Separator Line (├─────────────────────────┤)
        let sep_y = sep_area.top();
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

        // Footer Help Line
        let mut footer = Vec::new();
        footer.extend(key_pill("Enter", Color::Green));
        footer.push(Span::raw(if lang == Language::Fr {
            " Connecter  "
        } else {
            " Connect  "
        }));

        if state.active_ssh_target.is_some() {
            footer.extend(key_pill("S", Color::Yellow));
            footer.push(Span::raw(if lang == Language::Fr {
                " Sauvegarder hôte  "
            } else {
                " Save host  "
            }));

            footer.extend(key_pill("R", Color::Cyan));
            footer.push(Span::raw(" Scan  "));
        }

        footer.extend(key_pill("A", Color::Yellow));
        footer.push(Span::raw(if lang == Language::Fr {
            " Ajouter  "
        } else {
            " Add  "
        }));

        footer.extend(key_pill("F", Color::Yellow));
        footer.push(Span::raw(if lang == Language::Fr {
            " Favori ★  "
        } else {
            " Favorite ★  "
        }));

        footer.extend(key_pill("D", Color::Red));
        footer.push(Span::raw(if lang == Language::Fr {
            " Suppr  "
        } else {
            " Delete  "
        }));

        footer.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::DarkGray));
        footer.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigNavClose))));

        let p_bottom = Paragraph::new(Line::from(footer)).alignment(Alignment::Center);
        p_bottom.render(footer_area, buf);
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
