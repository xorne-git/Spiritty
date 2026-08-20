use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};
use std::collections::HashMap;

use crate::{
    config::{Config, ProviderConfig, ProviderType},
    i18n::{I18nKey, Language},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Provider,
    AutoApprove,
    Model,
    BaseUrl,
    ApiKey,
    SaveButton,
}

impl ConfigField {
    pub fn next(&self) -> Self {
        match self {
            ConfigField::Provider => ConfigField::AutoApprove,
            ConfigField::AutoApprove => ConfigField::Model,
            ConfigField::Model => ConfigField::BaseUrl,
            ConfigField::BaseUrl => ConfigField::ApiKey,
            ConfigField::ApiKey => ConfigField::SaveButton,
            ConfigField::SaveButton => ConfigField::Provider,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ConfigField::Provider => ConfigField::SaveButton,
            ConfigField::AutoApprove => ConfigField::Provider,
            ConfigField::Model => ConfigField::AutoApprove,
            ConfigField::BaseUrl => ConfigField::Model,
            ConfigField::ApiKey => ConfigField::BaseUrl,
            ConfigField::SaveButton => ConfigField::ApiKey,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownAction {
    None,
    Adding(String, usize),
    Editing(String, usize),
}

pub struct ConfigModalState {
    pub selected_provider: ProviderType,
    pub auto_approve: crate::config::AutoApproveLevel,
    pub active_field: ConfigField,
    pub is_dropdown_open: bool,
    pub dropdown_selected_idx: usize,
    pub dropdown_action: DropdownAction,
    pub models_per_provider: HashMap<String, Vec<String>>,
    pub model_input: String,
    pub base_url_input: String,
    pub url_cursor: usize,
    pub api_key_input: String,
    pub api_key_cursor: usize,
}

fn key_pill<'a>(key: &'a str, color: Color) -> Vec<Span<'a>> {
    vec![
        Span::styled("", Style::default().fg(color)),
        Span::styled(key, Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::styled("", Style::default().fg(color)),
    ]
}

fn insert_char_at(s: &mut String, idx: usize, c: char) {
    let mut chars: Vec<char> = s.chars().collect();
    if idx <= chars.len() {
        chars.insert(idx, c);
        *s = chars.into_iter().collect();
    }
}

fn remove_char_at(s: &mut String, idx: usize) {
    let mut chars: Vec<char> = s.chars().collect();
    if idx < chars.len() {
        chars.remove(idx);
        *s = chars.into_iter().collect();
    }
}

fn render_editable_text<'a>(text: &'a str, cursor: usize, is_focused: bool, placeholder: &'a str) -> Vec<Span<'a>> {
    if !is_focused {
        if text.is_empty() {
            return vec![Span::styled(placeholder, Style::default().fg(Color::DarkGray))];
        }
        return vec![Span::styled(text, Style::default().fg(Color::White))];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();

    if chars.is_empty() {
        spans.push(Span::styled(" ", Style::default().bg(Color::Yellow).fg(Color::Black)));
        return spans;
    }

    let cursor = cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    if !before.is_empty() {
        spans.push(Span::styled(before, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    if cursor < chars.len() {
        let cur_char = chars[cursor].to_string();
        spans.push(Span::styled(cur_char, Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)));
        let after: String = chars[(cursor + 1)..].iter().collect();
        if !after.is_empty() {
            spans.push(Span::styled(after, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
    } else {
        spans.push(Span::styled(" ", Style::default().bg(Color::Yellow).fg(Color::Black)));
    }

    spans
}

impl ConfigModalState {
    pub fn from_config(config: &Config) -> Self {
        let provider = config.default_provider;
        let p_cfg = config.get_active_provider_config();

        let mut models_per_provider = HashMap::new();
        for p in ProviderType::all() {
            models_per_provider.insert(p.key_str().to_string(), config.get_models_for_provider(*p));
        }

        let current_models = models_per_provider.get(provider.key_str()).cloned().unwrap_or_default();
        let dropdown_selected_idx = current_models.iter().position(|m| *m == p_cfg.model).unwrap_or(0);

        let base_url_input = p_cfg.base_url.unwrap_or_else(|| provider.default_base_url().unwrap_or("").to_string());
        let url_cursor = base_url_input.chars().count();

        let api_key_input = p_cfg.api_key.unwrap_or_else(|| provider.default_env_var().map(|e| format!("ENV:{}", e)).unwrap_or_default());
        let api_key_cursor = api_key_input.chars().count();

        Self {
            selected_provider: provider,
            auto_approve: config.auto_approve,
            active_field: ConfigField::Provider,
            is_dropdown_open: false,
            dropdown_selected_idx,
            dropdown_action: DropdownAction::None,
            models_per_provider,
            model_input: p_cfg.model,
            base_url_input,
            url_cursor,
            api_key_input,
            api_key_cursor,
        }
    }

    pub fn set_provider(&mut self, provider: ProviderType, config: &Config) {
        self.selected_provider = provider;
        let key = provider.key_str();
        if let Some(existing) = config.providers.get(key) {
            self.model_input = existing.model.clone();
            self.base_url_input = existing.base_url.clone().unwrap_or_else(|| provider.default_base_url().unwrap_or("").to_string());
            self.api_key_input = existing.api_key.clone().unwrap_or_else(|| provider.default_env_var().map(|e| format!("ENV:{}", e)).unwrap_or_default());
        } else {
            self.model_input = provider.default_model().to_string();
            self.base_url_input = provider.default_base_url().unwrap_or("").to_string();
            self.api_key_input = provider.default_env_var().map(|e| format!("ENV:{}", e)).unwrap_or_default();
        }

        self.url_cursor = self.base_url_input.chars().count();
        self.api_key_cursor = self.api_key_input.chars().count();

        let models = self.models_per_provider.get(key).map(|v| v.as_slice()).unwrap_or(&[]);
        self.dropdown_selected_idx = models.iter().position(|m| *m == self.model_input).unwrap_or(0);
        self.is_dropdown_open = false;
        self.dropdown_action = DropdownAction::None;
    }

    pub fn handle_paste(&mut self, text: String) {
        let clean = text.replace(['\r', '\n'], "");
        if clean.is_empty() {
            return;
        }

        if self.is_dropdown_open {
            match &mut self.dropdown_action {
                DropdownAction::Adding(input, cursor) | DropdownAction::Editing(input, cursor) => {
                    for c in clean.chars() {
                        insert_char_at(input, *cursor, c);
                        *cursor += 1;
                    }
                    return;
                }
                DropdownAction::None => {}
            }
        }

        match self.active_field {
            ConfigField::Model => {
                self.model_input.push_str(&clean);
            }
            ConfigField::BaseUrl => {
                for c in clean.chars() {
                    insert_char_at(&mut self.base_url_input, self.url_cursor, c);
                    self.url_cursor += 1;
                }
            }
            ConfigField::ApiKey => {
                for c in clean.chars() {
                    insert_char_at(&mut self.api_key_input, self.api_key_cursor, c);
                    self.api_key_cursor += 1;
                }
            }
            _ => {}
        }
    }

    pub fn save_config(&self, config: &mut Config) -> bool {
        let key = self.selected_provider.key_str().to_string();
        let base_url = if self.base_url_input.trim().is_empty() {
            None
        } else {
            Some(self.base_url_input.trim().to_string())
        };
        let api_key = if self.api_key_input.trim().is_empty() {
            None
        } else {
            Some(self.api_key_input.trim().to_string())
        };

        let models = self.models_per_provider.get(&key).cloned().unwrap_or_default();
        let existing_ctx = config.providers.get(&key).and_then(|p| p.context_window);
        let updated_provider = ProviderConfig {
            model: self.model_input.trim().to_string(),
            models,
            base_url,
            api_key,
            context_window: existing_ctx,
        };

        config.default_provider = self.selected_provider;
        config.auto_approve = self.auto_approve;
        config.providers.insert(key, updated_provider);

        let _ = config.save();
        true // Close modal on save
    }

    pub fn handle_key(&mut self, key: KeyEvent, config: &mut Config) -> bool {
        // Global save shortcut anywhere in the modal: Shift+Enter, Ctrl+Enter, Ctrl+S, F2
        let is_save_shortcut = (matches!(key.code, KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r'))
            && (key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) || key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)))
            || (key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')))
            || key.code == KeyCode::F(2);

        let prov_key = self.selected_provider.key_str().to_string();

        if is_save_shortcut && !matches!(self.dropdown_action, DropdownAction::Adding(..) | DropdownAction::Editing(..)) {
            if self.is_dropdown_open {
                if let Some(models) = self.models_per_provider.get(&prov_key) {
                    if let Some(selected) = models.get(self.dropdown_selected_idx) {
                        self.model_input = selected.clone();
                    }
                }
                self.is_dropdown_open = false;
            }
            return self.save_config(config);
        }

        // 1. Dropdown is open
        if self.is_dropdown_open {
            match &mut self.dropdown_action {
                DropdownAction::Adding(input, cursor) => match key.code {
                    KeyCode::Enter => {
                        let new_model = input.trim().to_string();
                        if !new_model.is_empty() {
                            let models = self.models_per_provider.entry(prov_key).or_default();
                            if !models.contains(&new_model) {
                                models.push(new_model.clone());
                            }
                            self.dropdown_selected_idx = models.iter().position(|m| *m == new_model).unwrap_or(0);
                            self.model_input = new_model;
                        }
                        self.dropdown_action = DropdownAction::None;
                        return false;
                    }
                    KeyCode::Esc => {
                        self.dropdown_action = DropdownAction::None;
                        return false;
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                        return false;
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                        return false;
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                        return false;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                        return false;
                    }
                    KeyCode::Char(c) => {
                        insert_char_at(input, *cursor, c);
                        *cursor += 1;
                        return false;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            remove_char_at(input, *cursor - 1);
                            *cursor -= 1;
                        }
                        return false;
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            remove_char_at(input, *cursor);
                        }
                        return false;
                    }
                    _ => return false,
                },
                DropdownAction::Editing(input, cursor) => match key.code {
                    KeyCode::Enter => {
                        let edited = input.trim().to_string();
                        if !edited.is_empty() {
                            let idx = self.dropdown_selected_idx;
                            let models = self.models_per_provider.entry(prov_key).or_default();
                            if idx < models.len() {
                                models[idx] = edited.clone();
                                self.model_input = edited;
                            }
                        }
                        self.dropdown_action = DropdownAction::None;
                        return false;
                    }
                    KeyCode::Esc => {
                        self.dropdown_action = DropdownAction::None;
                        return false;
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                        return false;
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                        return false;
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                        return false;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                        return false;
                    }
                    KeyCode::Char(c) => {
                        insert_char_at(input, *cursor, c);
                        *cursor += 1;
                        return false;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            remove_char_at(input, *cursor - 1);
                            *cursor -= 1;
                        }
                        return false;
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            remove_char_at(input, *cursor);
                        }
                        return false;
                    }
                    _ => return false,
                },
                DropdownAction::None => match key.code {
                    KeyCode::Up => {
                        self.dropdown_selected_idx = self.dropdown_selected_idx.saturating_sub(1);
                        return false;
                    }
                    KeyCode::Down => {
                        let count = self.models_per_provider.get(&prov_key).map(|v| v.len()).unwrap_or(0);
                        if count > 0 && self.dropdown_selected_idx + 1 < count {
                            self.dropdown_selected_idx += 1;
                        }
                        return false;
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(models) = self.models_per_provider.get(&prov_key) {
                            if let Some(selected) = models.get(self.dropdown_selected_idx) {
                                self.model_input = selected.clone();
                            }
                        }
                        self.is_dropdown_open = false;
                        return false;
                    }
                    KeyCode::Char('a') | KeyCode::Char('+') => {
                        self.dropdown_action = DropdownAction::Adding(String::new(), 0);
                        return false;
                    }
                    KeyCode::Char('e') | KeyCode::F(2) => {
                        let current_name = self.models_per_provider
                            .get(&prov_key)
                            .and_then(|v| v.get(self.dropdown_selected_idx))
                            .cloned()
                            .unwrap_or_default();
                        let len = current_name.chars().count();
                        self.dropdown_action = DropdownAction::Editing(current_name, len);
                        return false;
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        let idx = self.dropdown_selected_idx;
                        let models = self.models_per_provider.entry(prov_key).or_default();
                        if models.len() > 1 && idx < models.len() {
                            models.remove(idx);
                            let new_idx = if idx >= models.len() { models.len() - 1 } else { idx };
                            self.dropdown_selected_idx = new_idx;
                            if let Some(new_sel) = models.get(new_idx) {
                                self.model_input = new_sel.clone();
                            }
                        }
                        return false;
                    }
                    KeyCode::Esc => {
                        self.is_dropdown_open = false;
                        return false;
                    }
                    _ => return false,
                },
            }
        }

        // 2. Main modal navigation
        match key.code {
            KeyCode::Esc => return true, // Close modal on Esc when dropdown is closed
            KeyCode::Tab | KeyCode::Down => {
                self.active_field = self.active_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.active_field = self.active_field.prev();
            }
            KeyCode::Left => match self.active_field {
                ConfigField::Provider => {
                    let all = ProviderType::all();
                    let current_idx = all.iter().position(|p| *p == self.selected_provider).unwrap_or(0);
                    let prev_idx = if current_idx == 0 { all.len() - 1 } else { current_idx - 1 };
                    self.set_provider(all[prev_idx], config);
                }
                ConfigField::AutoApprove => {
                    self.auto_approve = self.auto_approve.prev();
                }
                ConfigField::Model => {
                    if let Some(models) = self.models_per_provider.get(&prov_key) {
                        let len = models.len();
                        if len > 0 {
                            let new_idx = if self.dropdown_selected_idx == 0 { len - 1 } else { self.dropdown_selected_idx - 1 };
                            self.dropdown_selected_idx = new_idx;
                            self.model_input = models[new_idx].clone();
                        }
                    }
                }
                ConfigField::BaseUrl => {
                    self.url_cursor = self.url_cursor.saturating_sub(1);
                }
                ConfigField::ApiKey => {
                    self.api_key_cursor = self.api_key_cursor.saturating_sub(1);
                }
                _ => {}
            },
            KeyCode::Right => match self.active_field {
                ConfigField::Provider => {
                    let all = ProviderType::all();
                    let current_idx = all.iter().position(|p| *p == self.selected_provider).unwrap_or(0);
                    let next_idx = (current_idx + 1) % all.len();
                    self.set_provider(all[next_idx], config);
                }
                ConfigField::AutoApprove => {
                    self.auto_approve = self.auto_approve.next();
                }
                ConfigField::Model => {
                    if let Some(models) = self.models_per_provider.get(&prov_key) {
                        let len = models.len();
                        if len > 0 {
                            let new_idx = (self.dropdown_selected_idx + 1) % len;
                            self.dropdown_selected_idx = new_idx;
                            self.model_input = models[new_idx].clone();
                        }
                    }
                }
                ConfigField::BaseUrl => {
                    let max_len = self.base_url_input.chars().count();
                    if self.url_cursor < max_len {
                        self.url_cursor += 1;
                    }
                }
                ConfigField::ApiKey => {
                    let max_len = self.api_key_input.chars().count();
                    if self.api_key_cursor < max_len {
                        self.api_key_cursor += 1;
                    }
                }
                _ => {}
            },
            KeyCode::Home => match self.active_field {
                ConfigField::BaseUrl => self.url_cursor = 0,
                ConfigField::ApiKey => self.api_key_cursor = 0,
                _ => {}
            },
            KeyCode::End => match self.active_field {
                ConfigField::BaseUrl => self.url_cursor = self.base_url_input.chars().count(),
                ConfigField::ApiKey => self.api_key_cursor = self.api_key_input.chars().count(),
                _ => {}
            },
            KeyCode::Enter => {
                if self.active_field == ConfigField::Model {
                    self.is_dropdown_open = true;
                    return false;
                }
                if self.active_field == ConfigField::AutoApprove {
                    self.auto_approve = self.auto_approve.next();
                    return false;
                }

                return self.save_config(config);
            }
            KeyCode::Char(' ') => match self.active_field {
                ConfigField::AutoApprove => self.auto_approve = self.auto_approve.next(),
                ConfigField::Model => self.is_dropdown_open = true,
                ConfigField::BaseUrl => {
                    insert_char_at(&mut self.base_url_input, self.url_cursor, ' ');
                    self.url_cursor += 1;
                }
                ConfigField::ApiKey => {
                    insert_char_at(&mut self.api_key_input, self.api_key_cursor, ' ');
                    self.api_key_cursor += 1;
                }
                _ => {}
            },
            KeyCode::Char(c) if !c.is_control() && c != '\n' && c != '\r' => match self.active_field {
                ConfigField::BaseUrl => {
                    insert_char_at(&mut self.base_url_input, self.url_cursor, c);
                    self.url_cursor += 1;
                }
                ConfigField::ApiKey => {
                    insert_char_at(&mut self.api_key_input, self.api_key_cursor, c);
                    self.api_key_cursor += 1;
                }
                _ => {}
            },
            KeyCode::Backspace => match self.active_field {
                ConfigField::BaseUrl if self.url_cursor > 0 => {
                    remove_char_at(&mut self.base_url_input, self.url_cursor - 1);
                    self.url_cursor -= 1;
                }
                ConfigField::ApiKey if self.api_key_cursor > 0 => {
                    remove_char_at(&mut self.api_key_input, self.api_key_cursor - 1);
                    self.api_key_cursor -= 1;
                }
                _ => {}
            },
            KeyCode::Delete => match self.active_field {
                ConfigField::BaseUrl => {
                    remove_char_at(&mut self.base_url_input, self.url_cursor);
                }
                ConfigField::ApiKey => {
                    remove_char_at(&mut self.api_key_input, self.api_key_cursor);
                }
                _ => {}
            },
            _ => {}
        }
        false
    }

    pub fn render_modal(&self, area: Rect, buf: &mut Buffer, lang: Language) {
        let modal_width = (area.width * 85 / 100).clamp(76, 110);
        let modal_height = 20.min(area.height.saturating_sub(2));

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::new(3, 3, 1, 1))
            .title(Span::styled(
                lang.t(I18nKey::ConfigModalTitle),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let f_provider = self.active_field == ConfigField::Provider;
        let f_auto = self.active_field == ConfigField::AutoApprove;
        let f_model = self.active_field == ConfigField::Model;
        let f_url = self.active_field == ConfigField::BaseUrl;
        let f_key = self.active_field == ConfigField::ApiKey;
        let f_save = self.active_field == ConfigField::SaveButton;

        let mut lines = Vec::new();

        // 1. Provider Field with fixed width (Anthropic Claude = 16 chars max)
        let prov_color = if f_provider { Color::Yellow } else { Color::Cyan };
        let mut l1 = vec![
            Span::styled(lang.t(I18nKey::ConfigFieldProvider), Style::default().fg(if f_provider { Color::Cyan } else { Color::White }).add_modifier(Modifier::BOLD)),
        ];
        l1.extend(key_pill("←", prov_color));
        l1.push(Span::styled(
            format!(" {:^16} ", self.selected_provider.display_name()),
            Style::default().fg(if f_provider { Color::Yellow } else { Color::White }).add_modifier(Modifier::BOLD),
        ));
        l1.extend(key_pill("→", prov_color));
        lines.push(Line::from(l1));
        lines.push(Line::from(""));

        // 2. Auto-Approve Policy Field
        let auto_badge_color = match self.auto_approve {
            crate::config::AutoApproveLevel::Safe => Color::Green,
            crate::config::AutoApproveLevel::Sudo => Color::Yellow,
            crate::config::AutoApproveLevel::Yolo => Color::Red,
            crate::config::AutoApproveLevel::Off => Color::DarkGray,
        };
        let auto_arrow_color = if f_auto { Color::Yellow } else { Color::Cyan };
        let mut l_auto = vec![
            Span::styled(lang.t(I18nKey::ConfigFieldAutoApprove), Style::default().fg(if f_auto { Color::Cyan } else { Color::White }).add_modifier(Modifier::BOLD)),
        ];
        l_auto.extend(key_pill("←", auto_arrow_color));
        let auto_desc = format!(" {} ({}) ", self.auto_approve.display_name(), self.auto_approve.description(lang));
        l_auto.push(Span::styled(
            format!("{:^32}", auto_desc),
            Style::default().fg(if f_auto { Color::Yellow } else { auto_badge_color }).add_modifier(Modifier::BOLD),
        ));
        l_auto.extend(key_pill("→", auto_arrow_color));
        lines.push(Line::from(l_auto));
        lines.push(Line::from(""));

        // 3. Model Selection (with Dropdown trigger)
        let placeholder_model = lang.t(I18nKey::ConfigPlaceholderSelectModel);
        lines.push(Line::from(vec![
            Span::styled(lang.t(I18nKey::ConfigFieldModel), Style::default().fg(if f_model { Color::Cyan } else { Color::White }).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} ▾", if self.model_input.is_empty() { placeholder_model } else { &self.model_input }),
                Style::default().fg(if f_model { Color::Yellow } else { Color::White }).add_modifier(if f_model { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]));
        lines.push(Line::from(""));

        // 4. Server URL (Editable with cursor & arrow navigation)
        let mut l3 = vec![
            Span::styled(lang.t(I18nKey::ConfigFieldApiUrl), Style::default().fg(if f_url { Color::Cyan } else { Color::White }).add_modifier(Modifier::BOLD)),
        ];
        l3.extend(render_editable_text(&self.base_url_input, self.url_cursor, f_url, lang.t(I18nKey::ConfigPlaceholderDefaultUrl)));
        lines.push(Line::from(l3));
        lines.push(Line::from(""));

        // 5. Clé d'API (Editable with cursor & arrow navigation)
        let mut l4 = vec![
            Span::styled(lang.t(I18nKey::ConfigFieldApiKey), Style::default().fg(if f_key { Color::Cyan } else { Color::White }).add_modifier(Modifier::BOLD)),
        ];
        l4.extend(render_editable_text(&self.api_key_input, self.api_key_cursor, f_key, lang.t(I18nKey::ConfigPlaceholderNoKeyRequired)));
        lines.push(Line::from(l4));
        lines.push(Line::from(""));

        // 5. Save Button with square corners, Cyan background, and Yellow rollover
        let save_style = if f_save {
            Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
        };

        lines.push(Line::from(vec![
            Span::styled(lang.t(I18nKey::ConfigButtonSave), save_style),
        ]));

        let p_top = Paragraph::new(lines).alignment(Alignment::Left);
        p_top.render(inner_area, buf);

        // Full-Width Horizontal Separator Line (├─────────────────────────┤)
        let sep_y = modal_area.bottom().saturating_sub(5);
        let border_style = Style::default().fg(Color::Cyan);
        if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
            buf.set_string(modal_area.left(), sep_y, symbols::line::NORMAL.vertical_right, border_style);
            for x in (modal_area.left() + 1)..(modal_area.right().saturating_sub(1)) {
                buf.set_string(x, sep_y, symbols::line::NORMAL.horizontal, border_style);
            }
            buf.set_string(modal_area.right().saturating_sub(1), sep_y, symbols::line::NORMAL.vertical_left, border_style);
        }

        // Footer Guide below separator line (Vertically & Horizontally Centered with 1 row padding top and bottom)
        let mut footer = Vec::new();
        footer.extend(key_pill("Tab", Color::Cyan));
        footer.push(Span::raw(" "));
        footer.extend(key_pill("↑", Color::Cyan));
        footer.push(Span::raw(" "));
        footer.extend(key_pill("↓", Color::Cyan));
        footer.push(Span::raw(format!(" {}    ", lang.t(I18nKey::ConfigNavNavigate))));

        footer.extend(key_pill("Ctrl", Color::Yellow));
        footer.push(Span::styled("+", Style::default().fg(Color::Yellow)));
        footer.extend(key_pill("S", Color::Yellow));
        footer.push(Span::raw(format!(" {}    ", lang.t(I18nKey::ConfigButtonSave))));

        footer.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
        footer.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigNavClose))));

        let footer_area = Rect::new(
            modal_area.left() + 2,
            sep_y + 2,
            modal_area.width.saturating_sub(4),
            1,
        );
        let p_bottom = Paragraph::new(Line::from(footer)).alignment(Alignment::Center);
        p_bottom.render(footer_area, buf);

        // 3. Render Dropdown Overlay if open
        if self.is_dropdown_open {
            self.render_dropdown(modal_area, buf, lang);
        }
    }

    fn render_dropdown(&self, parent_area: Rect, buf: &mut Buffer, lang: Language) {
        let dd_width = (parent_area.width.saturating_sub(8)).clamp(50, 78);
        let prov_key = self.selected_provider.key_str();
        let models = self.models_per_provider.get(prov_key).map(|v| v.as_slice()).unwrap_or(&[]);
        let list_len = models.len() as u16;
        let content_lines = list_len + 2;
        let dd_height = (content_lines + 2).min(parent_area.height.saturating_sub(2));

        let dd_x = parent_area.left() + 20.min(parent_area.width.saturating_sub(dd_width + 3));
        let dd_y = parent_area.top() + 3;
        let dd_area = Rect::new(dd_x, dd_y, dd_width, dd_height);

        Clear.render(dd_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Yellow))
            .padding(Padding::horizontal(1))
            .title(Span::styled(
                lang.t(I18nKey::ConfigDropdownTitle),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(dd_area);
        block.render(dd_area, buf);

        let mut lines = Vec::new();

        match &self.dropdown_action {
            DropdownAction::Adding(input, cursor) => {
                lines.push(Line::from(Span::styled(
                    lang.t(I18nKey::ConfigDropdownAddTitle),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Green))];
                l.extend(render_editable_text(input, *cursor, true, ""));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                let mut f_add = Vec::new();
                f_add.extend(key_pill("Enter", Color::Green));
                f_add.push(Span::raw(format!(" {}   ", lang.t(I18nKey::ConfigDropdownConfirm))));
                f_add.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
                f_add.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigDropdownCancel))));
                lines.push(Line::from(f_add));
            }
            DropdownAction::Editing(input, cursor) => {
                lines.push(Line::from(Span::styled(
                    lang.t(I18nKey::ConfigDropdownEditTitle),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Cyan))];
                l.extend(render_editable_text(input, *cursor, true, ""));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                let mut f_edit = Vec::new();
                f_edit.extend(key_pill("Enter", Color::Cyan));
                f_edit.push(Span::raw(format!(" {}   ", lang.t(I18nKey::ConfigDropdownConfirm))));
                f_edit.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
                f_edit.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigDropdownCancel))));
                lines.push(Line::from(f_edit));
            }
            DropdownAction::None => {
                let available_for_models = (inner.height.saturating_sub(2)) as usize;
                let max_visible = available_for_models.max(1);
                let start_idx = if self.dropdown_selected_idx >= max_visible {
                    self.dropdown_selected_idx + 1 - max_visible
                } else {
                    0
                };

                let tag_active = lang.t(I18nKey::ConfigDropdownTagActive);

                for (i, m) in models.iter().enumerate().skip(start_idx).take(max_visible) {
                    let is_sel = i == self.dropdown_selected_idx;
                    let is_active = *m == self.model_input;

                    let prefix = if is_sel { "▶ " } else { "  " };
                    let tag = if is_active { tag_active } else { "" };

                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}{}", prefix, m, tag),
                            if is_sel {
                                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else if is_active {
                                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                    ]));
                }

                // Rounded Capsule Badges footer
                lines.push(Line::from(""));
                let mut dd_foot = Vec::new();
                dd_foot.extend(key_pill("a", Color::Yellow));
                dd_foot.push(Span::raw(format!(" {}  ", lang.t(I18nKey::ConfigDropdownActionAdd))));
                dd_foot.extend(key_pill("e", Color::Yellow));
                dd_foot.push(Span::raw(format!(" {}  ", lang.t(I18nKey::ConfigDropdownActionEdit))));
                dd_foot.extend(key_pill("d", Color::Red));
                dd_foot.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigDropdownActionDelete))));
                lines.push(Line::from(dd_foot));
            }
        }

        let p = Paragraph::new(lines);
        p.render(inner, buf);
    }
}
