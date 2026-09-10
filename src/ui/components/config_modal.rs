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
    config::{Config, ProviderConfig, ProviderType, ReasoningEffort},
    i18n::{I18nKey, Language},
    ui::theme::ThemeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Provider,
    AutoApprove,
    Theme,
    Model,
    Reasoning,
    BaseUrl,
    ApiKey,
    SaveButton,
}

impl ConfigField {
    pub fn next(&self) -> Self {
        match self {
            ConfigField::Provider => ConfigField::AutoApprove,
            ConfigField::AutoApprove => ConfigField::Theme,
            ConfigField::Theme => ConfigField::Model,
            ConfigField::Model => ConfigField::Reasoning,
            ConfigField::Reasoning => ConfigField::BaseUrl,
            ConfigField::BaseUrl => ConfigField::ApiKey,
            ConfigField::ApiKey => ConfigField::SaveButton,
            ConfigField::SaveButton => ConfigField::Provider,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ConfigField::Provider => ConfigField::SaveButton,
            ConfigField::AutoApprove => ConfigField::Provider,
            ConfigField::Theme => ConfigField::AutoApprove,
            ConfigField::Model => ConfigField::Theme,
            ConfigField::Reasoning => ConfigField::Model,
            ConfigField::BaseUrl => ConfigField::Reasoning,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigModalAction {
    None,
    Close,
    SaveAndClose,
    UpdatePricing,
    RefreshModelsAndPricing,
}

pub struct ConfigModalState {
    pub selected_provider: ProviderType,
    pub auto_approve: crate::config::AutoApproveLevel,
    pub theme: ThemeId,
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
    /// Secret currently stored in config for the edited provider, kept OUT of
    /// `api_key_input`: the modal never echoes an existing key back to the screen.
    /// An empty input at save-time means "preserve what was there".
    pub api_key_saved: Option<String>,
    pub reasoning_effort: ReasoningEffort,
    pub pricing_status: Option<(std::time::Instant, String, Color)>,
    pub provider_edits: HashMap<String, (String, String, String, Option<String>, ReasoningEffort)>,
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

fn spans_visual_len(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
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

fn render_editable_text<'a>(
    text: &'a str,
    cursor: usize,
    is_focused: bool,
    placeholder: &'a str,
) -> Vec<Span<'a>> {
    if text.is_empty() {
        if is_focused {
            return vec![Span::styled("█", Style::default().fg(Color::Cyan))];
        } else {
            return vec![Span::styled(
                placeholder,
                Style::default().fg(Color::DarkGray),
            )];
        }
    }

    if !is_focused {
        return vec![Span::styled(text, Style::default().fg(Color::White))];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();

    if cursor >= chars.len() {
        spans.push(Span::styled(text, Style::default().fg(Color::White)));
        spans.push(Span::styled("█", Style::default().fg(Color::Cyan)));
    } else {
        let before: String = chars[..cursor].iter().collect();
        let cursor_char = chars[cursor];
        let after: String = chars[cursor + 1..].iter().collect();

        if !before.is_empty() {
            spans.push(Span::styled(before, Style::default().fg(Color::White)));
        }
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default().bg(Color::Cyan).fg(Color::Black),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after, Style::default().fg(Color::White)));
        }
    }

    spans
}

impl ConfigModalState {
    pub fn from_config(config: &Config) -> Self {
        let selected_provider = config.default_provider;
        let mut models_per_provider = HashMap::new();

        for p in ProviderType::all() {
            let mut list = Vec::new();
            if let Some(p_cfg) = config.providers.get(p.key_str()) {
                if !p_cfg.models.is_empty() {
                    list = p_cfg.models.clone();
                }
            }
            if list.is_empty() {
                list = p.popular_models().iter().map(|s| s.to_string()).collect();
            }
            models_per_provider.insert(p.key_str().to_string(), list);
        }

        let prov_key = selected_provider.key_str();
        let (mut model_input, base_url_input, api_key_input_pre) =
            if let Some(p_cfg) = config.providers.get(prov_key) {
                (
                    p_cfg.model.clone(),
                    p_cfg.base_url.clone().unwrap_or_default(),
                    p_cfg.api_key.clone().unwrap_or_default(),
                )
            } else {
                (
                    selected_provider.default_model().to_string(),
                    String::new(),
                    String::new(),
                )
            };

        if selected_provider == ProviderType::DeepSeek && (model_input.contains("v4") || model_input.is_empty()) {
            model_input = "deepseek-flash".to_string();
        }

        // Never preload an actual secret into the editable field: stash it aside so the
        // modal can't leak it on screen. Empty field at save-time = keep stored key.
        // ENV: references are not secrets themselves and stay visible/editable.
        let api_key_saved = config
            .providers
            .get(prov_key)
            .and_then(|p| p.api_key.clone());
        let api_key_input = match &api_key_saved {
            Some(k) if !k.starts_with("ENV:") => String::new(),
            _ => api_key_input_pre,
        };

        let dropdown_selected_idx = models_per_provider
            .get(prov_key)
            .and_then(|models| models.iter().position(|m| *m == model_input))
            .unwrap_or(0);

        let url_len = base_url_input.chars().count();
        let key_len = api_key_input.chars().count();
        let theme = ThemeId::parse_or_default(config.theme.as_deref().unwrap_or("spiritty_dark"));
        let reasoning_effort = config
            .providers
            .get(prov_key)
            .map(|p| p.reasoning_effort)
            .unwrap_or_default();

        Self {
            selected_provider,
            active_field: ConfigField::Provider,
            auto_approve: config.auto_approve,
            theme,
            model_input,
            base_url_input,
            api_key_input,
            url_cursor: url_len,
            api_key_cursor: key_len,
            api_key_saved,
            reasoning_effort,
            is_dropdown_open: false,
            dropdown_selected_idx,
            dropdown_action: DropdownAction::None,
            models_per_provider,
            pricing_status: None,
            provider_edits: HashMap::new(),
        }
    }

    pub fn set_provider(&mut self, provider: ProviderType, config: &Config) {
        // 1. Stash current provider inputs
        let old_key = self.selected_provider.key_str().to_string();
        self.provider_edits.insert(
            old_key,
            (
                self.model_input.clone(),
                self.base_url_input.clone(),
                self.api_key_input.clone(),
                self.api_key_saved.clone(),
                self.reasoning_effort,
            ),
        );

        // 2. Switch provider
        self.selected_provider = provider;
        let prov_key = provider.key_str();

        if let Some((m, u, k_in, k_saved, r_effort)) = self.provider_edits.get(prov_key).cloned() {
            self.model_input = m;
            self.base_url_input = u;
            self.api_key_input = k_in;
            self.api_key_saved = k_saved;
            self.reasoning_effort = r_effort;
        } else if let Some(p_cfg) = config.providers.get(prov_key) {
            self.model_input = p_cfg.model.clone();
            self.base_url_input = p_cfg.base_url.clone().unwrap_or_default();
            self.api_key_saved = p_cfg.api_key.clone();
            // Same secret-hygiene as open(): never echo a stored key into the field.
            self.api_key_input = match &self.api_key_saved {
                Some(k) if !k.starts_with("ENV:") => String::new(),
                other => other.clone().unwrap_or_default(),
            };
            self.reasoning_effort = p_cfg.reasoning_effort;
        } else {
            self.model_input = provider.default_model().to_string();
            self.base_url_input = String::new();
            self.api_key_input = String::new();
            self.api_key_saved = None;
            self.reasoning_effort = ReasoningEffort::Default;
        }

        if provider == ProviderType::DeepSeek && (self.model_input.contains("v4") || self.model_input.is_empty()) {
            self.model_input = "deepseek-flash".to_string();
        }

        self.url_cursor = self.base_url_input.chars().count();
        self.api_key_cursor = self.api_key_input.chars().count();

        self.dropdown_selected_idx = self
            .models_per_provider
            .get(prov_key)
            .and_then(|models| models.iter().position(|m| *m == self.model_input))
            .unwrap_or(0);
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

    pub fn save_config(&mut self, config: &mut Config) -> ConfigModalAction {
        // 1. Stash current provider inputs
        let cur_key = self.selected_provider.key_str().to_string();
        self.provider_edits.insert(
            cur_key,
            (
                self.model_input.clone(),
                self.base_url_input.clone(),
                self.api_key_input.clone(),
                self.api_key_saved.clone(),
                self.reasoning_effort,
            ),
        );

        // 2. Persist all edited providers into config
        for (key, (model, base_url_str, api_key_in, api_key_saved, r_effort)) in &self.provider_edits {
            let base_url = if base_url_str.trim().is_empty() {
                None
            } else {
                Some(base_url_str.trim().to_string())
            };
            let api_key = if api_key_in.trim().is_empty() {
                api_key_saved.clone()
            } else {
                Some(api_key_in.trim().to_string())
            };

            let mut models = self
                .models_per_provider
                .get(key)
                .cloned()
                .unwrap_or_default();
            let mut model_trimmed = model.trim().to_string();
            if key == "deepseek" && (model_trimmed.contains("v4") || model_trimmed.is_empty()) {
                model_trimmed = "deepseek-flash".to_string();
            }
            if !model_trimmed.is_empty() && !models.contains(&model_trimmed) {
                models.push(model_trimmed.clone());
            }
            if key == "deepseek" {
                models.retain(|m| !m.contains("v4"));
                if !models.contains(&"deepseek-flash".to_string()) {
                    models.insert(0, "deepseek-flash".to_string());
                }
            }
            let existing_ctx = config.providers.get(key).and_then(|p| p.context_window);
            let updated_provider = ProviderConfig {
                model: model_trimmed,
                models,
                base_url,
                api_key,
                context_window: existing_ctx,
                reasoning_effort: *r_effort,
            };
            config.providers.insert(key.clone(), updated_provider);
        }

        config.default_provider = self.selected_provider;
        config.auto_approve = self.auto_approve;
        config.theme = Some(self.theme.key_str().to_string());

        let _ = config.save();
        ConfigModalAction::SaveAndClose
    }

    pub fn handle_key(&mut self, key: KeyEvent, config: &mut Config) -> ConfigModalAction {
        let is_save_shortcut = (matches!(
            key.code,
            KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
        ) && (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::SHIFT)
            || key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)))
            || (key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')))
            || key.code == KeyCode::F(2);

        let is_paste = (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            || key.modifiers.contains(
                crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
            ))
            && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'));

        if is_paste {
            if let Some(text) = crate::system::clipboard::get_clipboard_text_timeout(
                std::time::Duration::from_millis(1000),
            ) {
                self.handle_paste(text);
                return ConfigModalAction::None;
            }
        }

        let prov_key = self.selected_provider.key_str().to_string();

        let is_refresh_models = (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')))
            || key.code == KeyCode::F(5)
            || (matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
                && self.active_field != ConfigField::BaseUrl
                && self.active_field != ConfigField::ApiKey);

        if is_refresh_models
            && !self.is_dropdown_open
            && !matches!(
                self.dropdown_action,
                DropdownAction::Adding(..) | DropdownAction::Editing(..)
            )
        {
            let lang = config.get_language();
            self.pricing_status = Some((
                std::time::Instant::now(),
                lang.t(crate::i18n::I18nKey::ConfigRefreshInProgress).to_string(),
                Color::Cyan,
            ));
            return ConfigModalAction::RefreshModelsAndPricing;
        }

        let is_update_pricing = (key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('u') | KeyCode::Char('U')))
            || (matches!(key.code, KeyCode::Char('u') | KeyCode::Char('U'))
                && self.active_field != ConfigField::BaseUrl
                && self.active_field != ConfigField::ApiKey);

        if is_update_pricing
            && !self.is_dropdown_open
            && !matches!(
                self.dropdown_action,
                DropdownAction::Adding(..) | DropdownAction::Editing(..)
            )
        {
            self.pricing_status = Some((
                std::time::Instant::now(),
                if config.get_language() == crate::i18n::Language::Fr {
                    "⟳ Synchronisation des tarifs en ligne...".to_string()
                } else {
                    "⟳ Syncing online pricing...".to_string()
                },
                Color::Cyan,
            ));
            return ConfigModalAction::UpdatePricing;
        }

        if is_save_shortcut
            && !matches!(
                self.dropdown_action,
                DropdownAction::Adding(..) | DropdownAction::Editing(..)
            )
        {
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
                            self.dropdown_selected_idx =
                                models.iter().position(|m| *m == new_model).unwrap_or(0);
                            self.model_input = new_model;
                        }
                        self.dropdown_action = DropdownAction::None;
                        self.is_dropdown_open = false;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Esc => {
                        self.dropdown_action = DropdownAction::None;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                        return ConfigModalAction::None;
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                        return ConfigModalAction::None;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                        return ConfigModalAction::None;
                    }
                    KeyCode::Char(c) => {
                        insert_char_at(input, *cursor, c);
                        *cursor += 1;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            remove_char_at(input, *cursor - 1);
                            *cursor -= 1;
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            remove_char_at(input, *cursor);
                        }
                        return ConfigModalAction::None;
                    }
                    _ => return ConfigModalAction::None,
                },
                DropdownAction::Editing(input, cursor) => match key.code {
                    KeyCode::Enter => {
                        let edited_model = input.trim().to_string();
                        let idx = self.dropdown_selected_idx;
                        if !edited_model.is_empty() {
                            if let Some(models) = self.models_per_provider.get_mut(&prov_key) {
                                if idx < models.len() {
                                    models[idx] = edited_model.clone();
                                    self.model_input = edited_model;
                                }
                            }
                        }
                        self.dropdown_action = DropdownAction::None;
                        self.is_dropdown_open = false;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Esc => {
                        self.dropdown_action = DropdownAction::None;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                        return ConfigModalAction::None;
                    }
                    KeyCode::Right => {
                        if *cursor < input.chars().count() {
                            *cursor += 1;
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                        return ConfigModalAction::None;
                    }
                    KeyCode::End => {
                        *cursor = input.chars().count();
                        return ConfigModalAction::None;
                    }
                    KeyCode::Char(c) => {
                        insert_char_at(input, *cursor, c);
                        *cursor += 1;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            remove_char_at(input, *cursor - 1);
                            *cursor -= 1;
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Delete => {
                        if *cursor < input.chars().count() {
                            remove_char_at(input, *cursor);
                        }
                        return ConfigModalAction::None;
                    }
                    _ => return ConfigModalAction::None,
                },
                DropdownAction::None => match key.code {
                    KeyCode::Up => {
                        self.dropdown_selected_idx = self.dropdown_selected_idx.saturating_sub(1);
                        return ConfigModalAction::None;
                    }
                    KeyCode::Down => {
                        let count = self
                            .models_per_provider
                            .get(&prov_key)
                            .map(|v| v.len())
                            .unwrap_or(0);
                        if count > 0 && self.dropdown_selected_idx + 1 < count {
                            self.dropdown_selected_idx += 1;
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(models) = self.models_per_provider.get(&prov_key) {
                            if let Some(selected) = models.get(self.dropdown_selected_idx) {
                                let mut sel = selected.clone();
                                if self.selected_provider == ProviderType::DeepSeek && sel.contains("v4") {
                                    sel = "deepseek-flash".to_string();
                                }
                                self.model_input = sel;
                            }
                        }
                        self.is_dropdown_open = false;
                        return ConfigModalAction::None;
                    }
                    KeyCode::Char('a') | KeyCode::Char('+') => {
                        self.dropdown_action = DropdownAction::Adding(String::new(), 0);
                        return ConfigModalAction::None;
                    }
                    KeyCode::Char('e') | KeyCode::F(2) => {
                        let current_name = self
                            .models_per_provider
                            .get(&prov_key)
                            .and_then(|v| v.get(self.dropdown_selected_idx))
                            .cloned()
                            .unwrap_or_default();
                        let len = current_name.chars().count();
                        self.dropdown_action = DropdownAction::Editing(current_name, len);
                        return ConfigModalAction::None;
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        let idx = self.dropdown_selected_idx;
                        let models = self.models_per_provider.entry(prov_key).or_default();
                        if models.len() > 1 && idx < models.len() {
                            models.remove(idx);
                            let new_idx = if idx >= models.len() {
                                models.len() - 1
                            } else {
                                idx
                            };
                            self.dropdown_selected_idx = new_idx;
                            if let Some(new_sel) = models.get(new_idx) {
                                self.model_input = new_sel.clone();
                            }
                        }
                        return ConfigModalAction::None;
                    }
                    KeyCode::Esc => {
                        self.is_dropdown_open = false;
                        return ConfigModalAction::None;
                    }
                    _ => return ConfigModalAction::None,
                },
            }
        }

        match key.code {
            KeyCode::Esc => return ConfigModalAction::Close,
            KeyCode::Tab | KeyCode::Down => {
                self.active_field = self.active_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.active_field = self.active_field.prev();
            }
            KeyCode::Left => match self.active_field {
                ConfigField::Provider => {
                    let all = ProviderType::all();
                    let current_idx = all
                        .iter()
                        .position(|p| *p == self.selected_provider)
                        .unwrap_or(0);
                    let prev_idx = if current_idx == 0 {
                        all.len() - 1
                    } else {
                        current_idx - 1
                    };
                    self.set_provider(all[prev_idx], config);
                }
                ConfigField::AutoApprove => {
                    self.auto_approve = self.auto_approve.prev();
                }
                ConfigField::Theme => {
                    let all = ThemeId::all();
                    let current_idx = all.iter().position(|t| *t == self.theme).unwrap_or(0);
                    let prev_idx = if current_idx == 0 {
                        all.len() - 1
                    } else {
                        current_idx - 1
                    };
                    self.theme = all[prev_idx];
                }
                ConfigField::Model => {
                    if let Some(models) = self.models_per_provider.get(&prov_key) {
                        let len = models.len();
                        if len > 0 {
                            let new_idx = if self.dropdown_selected_idx == 0 {
                                len - 1
                            } else {
                                self.dropdown_selected_idx - 1
                            };
                            self.dropdown_selected_idx = new_idx;
                            let mut sel = models[new_idx].clone();
                            if self.selected_provider == ProviderType::DeepSeek && sel.contains("v4") {
                                sel = "deepseek-flash".to_string();
                            }
                            self.model_input = sel;
                        }
                    }
                }
                ConfigField::Reasoning => {
                    self.reasoning_effort = self.reasoning_effort.prev();
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
                    let current_idx = all
                        .iter()
                        .position(|p| *p == self.selected_provider)
                        .unwrap_or(0);
                    let next_idx = (current_idx + 1) % all.len();
                    self.set_provider(all[next_idx], config);
                }
                ConfigField::AutoApprove => {
                    self.auto_approve = self.auto_approve.next();
                }
                ConfigField::Theme => {
                    let all = ThemeId::all();
                    let current_idx = all.iter().position(|t| *t == self.theme).unwrap_or(0);
                    let next_idx = (current_idx + 1) % all.len();
                    self.theme = all[next_idx];
                }
                ConfigField::Model => {
                    if let Some(models) = self.models_per_provider.get(&prov_key) {
                        let len = models.len();
                        if len > 0 {
                            let new_idx = (self.dropdown_selected_idx + 1) % len;
                            self.dropdown_selected_idx = new_idx;
                            let mut sel = models[new_idx].clone();
                            if self.selected_provider == ProviderType::DeepSeek && sel.contains("v4") {
                                sel = "deepseek-flash".to_string();
                            }
                            self.model_input = sel;
                        }
                    }
                }
                ConfigField::Reasoning => {
                    self.reasoning_effort = self.reasoning_effort.next();
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
                    return ConfigModalAction::None;
                }
                if self.active_field == ConfigField::Reasoning {
                    self.reasoning_effort = self.reasoning_effort.next();
                    return ConfigModalAction::None;
                }
                if self.active_field == ConfigField::AutoApprove {
                    self.auto_approve = self.auto_approve.next();
                    return ConfigModalAction::None;
                }
                if self.active_field == ConfigField::Theme {
                    let all = ThemeId::all();
                    let current_idx = all.iter().position(|t| *t == self.theme).unwrap_or(0);
                    self.theme = all[(current_idx + 1) % all.len()];
                    return ConfigModalAction::None;
                }

                return self.save_config(config);
            }
            KeyCode::Char(' ') => match self.active_field {
                ConfigField::AutoApprove => self.auto_approve = self.auto_approve.next(),
                ConfigField::Theme => {
                    let all = ThemeId::all();
                    let current_idx = all.iter().position(|t| *t == self.theme).unwrap_or(0);
                    self.theme = all[(current_idx + 1) % all.len()];
                }
                ConfigField::Model => self.is_dropdown_open = true,
                ConfigField::Reasoning => self.reasoning_effort = self.reasoning_effort.next(),
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
            KeyCode::Char(c) if !c.is_control() && c != '\n' && c != '\r' => {
                match self.active_field {
                    ConfigField::BaseUrl => {
                        insert_char_at(&mut self.base_url_input, self.url_cursor, c);
                        self.url_cursor += 1;
                    }
                    ConfigField::ApiKey => {
                        insert_char_at(&mut self.api_key_input, self.api_key_cursor, c);
                        self.api_key_cursor += 1;
                    }
                    _ => {}
                }
            }
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
        ConfigModalAction::None
    }

    pub fn render_modal(&self, area: Rect, buf: &mut Buffer, lang: Language) {
        let modal_width = (area.width * 94 / 100)
            .clamp(88, 130)
            .min(area.width.saturating_sub(2));
        let modal_height = 23.min(area.height.saturating_sub(2));

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
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let f_provider = self.active_field == ConfigField::Provider;
        let f_auto = self.active_field == ConfigField::AutoApprove;
        let f_theme = self.active_field == ConfigField::Theme;
        let f_model = self.active_field == ConfigField::Model;
        let f_reasoning = self.active_field == ConfigField::Reasoning;
        let f_url = self.active_field == ConfigField::BaseUrl;
        let f_key = self.active_field == ConfigField::ApiKey;
        let f_save = self.active_field == ConfigField::SaveButton;
        let show_blank = modal_area.height >= 21;

        let mut lines = Vec::new();

        // 1. Provider Field with fixed width (Anthropic Claude = 16 chars max)
        let prov_color = if f_provider {
            Color::Yellow
        } else {
            Color::Cyan
        };
        let mut l1 = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldProvider),
            Style::default()
                .fg(if f_provider {
                    Color::Cyan
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        )];
        l1.extend(key_pill("←", prov_color));
        l1.push(Span::styled(
            format!(" {:^16} ", self.selected_provider.display_name()),
            Style::default()
                .fg(if f_provider {
                    Color::Yellow
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        ));
        l1.extend(key_pill("→", prov_color));
        lines.push(Line::from(l1));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 2. Auto-Approve Policy Field
        let auto_badge_color = match self.auto_approve {
            crate::config::AutoApproveLevel::Safe => Color::Green,
            crate::config::AutoApproveLevel::Sudo => Color::Yellow,
            crate::config::AutoApproveLevel::Yolo => Color::Red,
            crate::config::AutoApproveLevel::Off => Color::DarkGray,
        };
        let auto_arrow_color = if f_auto { Color::Yellow } else { Color::Cyan };
        let mut l_auto = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldAutoApprove),
            Style::default()
                .fg(if f_auto { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        )];
        l_auto.extend(key_pill("←", auto_arrow_color));
        let auto_desc = format!(
            " {} ({}) ",
            self.auto_approve.display_name(),
            self.auto_approve.description(lang)
        );
        l_auto.push(Span::styled(
            format!("{:^32}", auto_desc),
            Style::default()
                .fg(if f_auto {
                    Color::Yellow
                } else {
                    auto_badge_color
                })
                .add_modifier(Modifier::BOLD),
        ));
        l_auto.extend(key_pill("→", auto_arrow_color));
        lines.push(Line::from(l_auto));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 3. Theme Selector Field
        let theme_arrow_color = if f_theme { Color::Yellow } else { Color::Cyan };
        let mut l_theme = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldTheme),
            Style::default()
                .fg(if f_theme { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        )];
        l_theme.extend(key_pill("←", theme_arrow_color));
        let theme_name = format!(" {} ", self.theme.display_name());
        l_theme.push(Span::styled(
            format!("{:^32}", theme_name),
            Style::default()
                .fg(if f_theme { Color::Yellow } else { Color::Cyan })
                .add_modifier(Modifier::BOLD),
        ));
        l_theme.extend(key_pill("→", theme_arrow_color));
        lines.push(Line::from(l_theme));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 4. Model Selection (with Dropdown trigger)
        let placeholder_model = lang.t(I18nKey::ConfigPlaceholderSelectModel);
        lines.push(Line::from(vec![
            Span::styled(
                lang.t(I18nKey::ConfigFieldModel),
                Style::default()
                    .fg(if f_model { Color::Cyan } else { Color::White })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "{} ▾",
                    if self.model_input.is_empty() {
                        placeholder_model
                    } else {
                        &self.model_input
                    }
                ),
                Style::default()
                    .fg(if f_model { Color::Yellow } else { Color::White })
                    .add_modifier(if f_model {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 5. Reasoning Effort Field
        let reasoning_badge_color = match self.reasoning_effort {
            ReasoningEffort::Default => Color::DarkGray,
            ReasoningEffort::Off => Color::DarkGray,
            ReasoningEffort::Low => Color::Green,
            ReasoningEffort::Medium => Color::Yellow,
            ReasoningEffort::High => Color::Magenta,
        };
        let reasoning_arrow_color = if f_reasoning {
            Color::Yellow
        } else {
            Color::Cyan
        };
        let mut l_reasoning = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldReasoning),
            Style::default()
                .fg(if f_reasoning {
                    Color::Cyan
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        )];
        l_reasoning.extend(key_pill("←", reasoning_arrow_color));
        let reasoning_desc = format!(
            " {} ({}) ",
            self.reasoning_effort.display_name(),
            self.reasoning_effort.description(lang)
        );
        l_reasoning.push(Span::styled(
            format!("{:^32}", reasoning_desc),
            Style::default()
                .fg(if f_reasoning {
                    Color::Yellow
                } else {
                    reasoning_badge_color
                })
                .add_modifier(Modifier::BOLD),
        ));
        l_reasoning.extend(key_pill("→", reasoning_arrow_color));
        lines.push(Line::from(l_reasoning));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 4. Server URL (Editable with cursor & arrow navigation)
        let mut l3 = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldApiUrl),
            Style::default()
                .fg(if f_url { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        )];
        l3.extend(render_editable_text(
            &self.base_url_input,
            self.url_cursor,
            f_url,
            lang.t(I18nKey::ConfigPlaceholderDefaultUrl),
        ));
        lines.push(Line::from(l3));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 5. Clé d'API (masked while typing — same char count keeps the cursor in sync)
        let mut l4 = vec![Span::styled(
            lang.t(I18nKey::ConfigFieldApiKey),
            Style::default()
                .fg(if f_key { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        )];
        let masked_key: String = self.api_key_input.chars().map(|_| '•').collect();
        l4.extend(render_editable_text(
            &masked_key,
            self.api_key_cursor,
            f_key,
            lang.t(I18nKey::ConfigPlaceholderNoKeyRequired),
        ));
        // Explicit hint when an existing secret will be preserved by saving with the
        // field left empty (it is deliberately never displayed back).
        if self.api_key_saved.is_some() && self.api_key_input.is_empty() {
            l4.push(Span::styled(
                if lang == crate::i18n::Language::Fr {
                    "   ✔ conservée si vide"
                } else {
                    "   ✔ kept if left empty"
                },
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(l4));
        if show_blank {
            lines.push(Line::from(""));
        }

        // 5. Save Button with square corners, Cyan background, and Yellow rollover
        let save_style = if f_save {
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        };

        let mut save_spans = vec![Span::styled(lang.t(I18nKey::ConfigButtonSave), save_style)];

        if let Some((time, ref status_text, color)) = self.pricing_status {
            if time.elapsed().as_secs() < 8 {
                save_spans.push(Span::raw("   "));
                save_spans.push(Span::styled(
                    status_text.clone(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
        }

        lines.push(Line::from(save_spans));

        let p_top = Paragraph::new(lines).alignment(Alignment::Left);
        p_top.render(inner_area, buf);

        // Full-Width Horizontal Separator Line (├─────────────────────────┤)
        let sep_y = if modal_area.height >= 23 {
            modal_area.bottom().saturating_sub(5)
        } else {
            modal_area.bottom().saturating_sub(4)
        };
        let border_style = Style::default().fg(Color::Cyan);
        if sep_y > modal_area.top() && sep_y < modal_area.bottom().saturating_sub(1) {
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
        }

        // Footer Guide below separator line
        let mut nav_spans = Vec::new();
        nav_spans.extend(key_pill("Tab", Color::Cyan));
        nav_spans.push(Span::raw(" "));
        nav_spans.extend(key_pill("↑", Color::Cyan));
        nav_spans.push(Span::raw(" "));
        nav_spans.extend(key_pill("↓", Color::Cyan));
        nav_spans.push(Span::raw(format!(
            " {}",
            lang.t(I18nKey::ConfigNavNavigate)
        )));

        let mut save_spans = Vec::new();
        save_spans.extend(key_pill("Ctrl", Color::Yellow));
        save_spans.push(Span::styled("+", Style::default().fg(Color::Yellow)));
        save_spans.extend(key_pill("S", Color::Yellow));
        save_spans.push(Span::raw(format!(
            " {}",
            lang.t(I18nKey::ConfigButtonSave)
        )));

        let mut refresh_spans = Vec::new();
        refresh_spans.extend(key_pill("R", Color::Rgb(80, 200, 120)));
        refresh_spans.push(Span::raw(format!(
            " {}",
            lang.t(I18nKey::ConfigActionRefreshModels)
        )));

        let mut pricing_spans = Vec::new();
        pricing_spans.extend(key_pill("U", Color::Rgb(140, 100, 240)));
        pricing_spans.push(Span::raw(format!(
            " {}",
            lang.t(I18nKey::ConfigActionUpdatePricing)
        )));

        let mut close_spans = Vec::new();
        close_spans.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
        close_spans.push(Span::raw(format!(" {}", lang.t(I18nKey::ConfigNavClose))));

        let footer_width = modal_area.width.saturating_sub(4) as usize;
        let total_items_len = spans_visual_len(&nav_spans)
            + spans_visual_len(&save_spans)
            + spans_visual_len(&refresh_spans)
            + spans_visual_len(&pricing_spans)
            + spans_visual_len(&close_spans);

        let gap_len = if footer_width >= total_items_len + 12 {
            3
        } else {
            2
        };
        let is_multiline = footer_width < total_items_len + gap_len * 4;

        if is_multiline {
            let line1_len = spans_visual_len(&nav_spans)
                + spans_visual_len(&save_spans)
                + spans_visual_len(&close_spans);
            let gap1 = if footer_width >= line1_len + 8 {
                "    "
            } else if footer_width >= line1_len + 4 {
                "  "
            } else {
                " "
            };

            let mut line1 = Vec::new();
            line1.extend(nav_spans);
            line1.push(Span::raw(gap1));
            line1.extend(save_spans);
            line1.push(Span::raw(gap1));
            line1.extend(close_spans);

            let line2_len =
                spans_visual_len(&refresh_spans) + spans_visual_len(&pricing_spans);
            let gap2 = if footer_width >= line2_len + 8 {
                "    "
            } else if footer_width >= line2_len + 4 {
                "  "
            } else {
                " "
            };

            let mut line2 = Vec::new();
            line2.extend(refresh_spans);
            line2.push(Span::raw(gap2));
            line2.extend(pricing_spans);

            let footer_area = Rect::new(
                modal_area.left() + 2,
                sep_y + 1,
                modal_area.width.saturating_sub(4),
                2,
            );
            let p_bottom = Paragraph::new(vec![Line::from(line1), Line::from(line2)])
                .alignment(Alignment::Center);
            p_bottom.render(footer_area, buf);
        } else {
            let gap_str = if gap_len == 3 { "   " } else { "  " };
            let mut line = Vec::new();
            line.extend(nav_spans);
            line.push(Span::raw(gap_str));
            line.extend(save_spans);
            line.push(Span::raw(gap_str));
            line.extend(refresh_spans);
            line.push(Span::raw(gap_str));
            line.extend(pricing_spans);
            line.push(Span::raw(gap_str));
            line.extend(close_spans);

            let footer_area = Rect::new(
                modal_area.left() + 2,
                if modal_area.height >= 23 {
                    sep_y + 2
                } else {
                    sep_y + 1
                },
                modal_area.width.saturating_sub(4),
                1,
            );
            let p_bottom = Paragraph::new(Line::from(line)).alignment(Alignment::Center);
            p_bottom.render(footer_area, buf);
        }

        // 3. Render Dropdown Overlay if open
        if self.is_dropdown_open {
            self.render_dropdown(modal_area, buf, lang);
        }
    }

    fn render_dropdown(&self, parent_area: Rect, buf: &mut Buffer, lang: Language) {
        let dd_width = (parent_area.width.saturating_sub(8)).clamp(50, 78);
        let prov_key = self.selected_provider.key_str();
        let models = self
            .models_per_provider
            .get(prov_key)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
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
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(dd_area);
        block.render(dd_area, buf);

        let mut lines = Vec::new();

        match &self.dropdown_action {
            DropdownAction::Adding(input, cursor) => {
                lines.push(Line::from(Span::styled(
                    lang.t(I18nKey::ConfigDropdownAddTitle),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Green))];
                l.extend(render_editable_text(input, *cursor, true, ""));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                let mut f_add = Vec::new();
                f_add.extend(key_pill("Enter", Color::Green));
                f_add.push(Span::raw(format!(
                    " {}   ",
                    lang.t(I18nKey::ConfigDropdownConfirm)
                )));
                f_add.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
                f_add.push(Span::raw(format!(
                    " {}",
                    lang.t(I18nKey::ConfigDropdownCancel)
                )));
                lines.push(Line::from(f_add));
            }
            DropdownAction::Editing(input, cursor) => {
                lines.push(Line::from(Span::styled(
                    lang.t(I18nKey::ConfigDropdownEditTitle),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                let mut l = vec![Span::styled("❯ ", Style::default().fg(Color::Cyan))];
                l.extend(render_editable_text(input, *cursor, true, ""));
                lines.push(Line::from(l));
                lines.push(Line::from(""));
                let mut f_edit = Vec::new();
                f_edit.extend(key_pill("Enter", Color::Cyan));
                f_edit.push(Span::raw(format!(
                    " {}   ",
                    lang.t(I18nKey::ConfigDropdownConfirm)
                )));
                f_edit.extend(key_pill(lang.t(I18nKey::HelpKeyClose), Color::Red));
                f_edit.push(Span::raw(format!(
                    " {}",
                    lang.t(I18nKey::ConfigDropdownCancel)
                )));
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

                    lines.push(Line::from(vec![Span::styled(
                        format!("{}{}{}", prefix, m, tag),
                        if is_sel {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else if is_active {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    )]));
                }

                // Rounded Capsule Badges footer
                lines.push(Line::from(""));
                let mut dd_foot = Vec::new();
                dd_foot.extend(key_pill("A", Color::Yellow));
                dd_foot.push(Span::raw(format!(
                    " {}  ",
                    lang.t(I18nKey::ConfigDropdownActionAdd)
                )));
                dd_foot.extend(key_pill("E", Color::Yellow));
                dd_foot.push(Span::raw(format!(
                    " {}  ",
                    lang.t(I18nKey::ConfigDropdownActionEdit)
                )));
                dd_foot.extend(key_pill("D", Color::Red));
                dd_foot.push(Span::raw(format!(
                    " {}",
                    lang.t(I18nKey::ConfigDropdownActionDelete)
                )));
                lines.push(Line::from(dd_foot));
            }
        }

        let p = Paragraph::new(lines);
        p.render(inner, buf);
    }
}
