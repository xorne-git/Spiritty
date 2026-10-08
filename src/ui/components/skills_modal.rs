use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::agent::skills::{Skill, SkillSource, SkillState, SkillsManager};
use crate::config::{Config, SkillSelectionMode, SkillsConfig};
use crate::i18n::{I18nKey, Language};
use crate::ui::key_pill;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsModalAction {
    SkillsChanged,
    Close,
}

pub struct SkillsModalState {
    pub skills: Vec<Skill>,
    pub selected_index: usize,
    pub details_scroll: u16,
    pub creating_new: bool,
    pub new_id_input: String,
    pub status_message: Option<(std::time::Instant, String)>,
    pub skills_config: SkillsConfig,
    pub has_modified: bool,
}

impl SkillsModalState {
    pub fn new(skills_config: &SkillsConfig) -> Self {
        let manager = SkillsManager::new();
        Self {
            skills: manager.skills().to_vec(),
            selected_index: 0,
            details_scroll: 0,
            creating_new: false,
            new_id_input: String::new(),
            status_message: None,
            skills_config: skills_config.clone(),
            has_modified: false,
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        if self.creating_new {
            for c in text.chars() {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    self.new_id_input.push(c);
                }
            }
        }
    }

    pub fn refresh(&mut self, skills_manager: &SkillsManager) {
        self.skills = skills_manager.skills().to_vec();
        if self.selected_index >= self.skills.len() && !self.skills.is_empty() {
            self.selected_index = self.skills.len() - 1;
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        config: &mut Config,
    ) -> Option<SkillsModalAction> {
        // If in modal input mode for creating a new skill
        if self.creating_new {
            match key.code {
                KeyCode::Esc => {
                    self.creating_new = false;
                    self.new_id_input.clear();
                    return None;
                }
                KeyCode::Enter => {
                    let trimmed = self.new_id_input.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        let clean_id = trimmed
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                            .collect::<String>();
                        if !clean_id.is_empty() {
                            if let Some(config_dir) = dirs::config_dir() {
                                let dir = config_dir.join("spiritty").join("skills").join(&clean_id);
                                let _ = std::fs::create_dir_all(&dir);
                                let file_path = dir.join("SKILL.md");
                                let template = format!(
                                    r#"---
name: {}
description: Description personnalisée pour {}
triggers: [{}]
---
# Directives pour {}

- Vos directives personnalisées ici.
"#,
                                    clean_id, clean_id, clean_id, clean_id
                                );
                                let _ = std::fs::write(&file_path, template);
                                let mgr = SkillsManager::new();
                                self.refresh(&mgr);
                                self.status_message = Some((
                                    std::time::Instant::now(),
                                    clean_id,
                                ));
                                self.has_modified = true;
                            }
                        }
                    }
                    self.creating_new = false;
                    self.new_id_input.clear();
                    return Some(SkillsModalAction::SkillsChanged);
                }
                KeyCode::Backspace => {
                    self.new_id_input.pop();
                    return None;
                }
                KeyCode::Char(c) => {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        self.new_id_input.push(c);
                    }
                    return None;
                }
                _ => return None,
            }
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        {
            return Some(SkillsModalAction::Close);
        }

        match key.code {
            KeyCode::Esc => Some(SkillsModalAction::Close),
            KeyCode::Tab | KeyCode::Char('m') | KeyCode::Char('M') => {
                config.skills.mode = config.skills.mode.toggle();
                self.skills_config = config.skills.clone();
                self.has_modified = true;
                let _ = config.save();
                Some(SkillsModalAction::SkillsChanged)
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.details_scroll = self.details_scroll.saturating_sub(1);
                } else if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.details_scroll = 0;
                }
                None
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.details_scroll = self.details_scroll.saturating_add(1);
                } else if !self.skills.is_empty() && self.selected_index + 1 < self.skills.len() {
                    self.selected_index += 1;
                    self.details_scroll = 0;
                }
                None
            }
            KeyCode::PageUp => {
                self.details_scroll = self.details_scroll.saturating_sub(5);
                None
            }
            KeyCode::PageDown => {
                self.details_scroll = self.details_scroll.saturating_add(5);
                None
            }
            KeyCode::Char(' ') => {
                if let Some(skill) = self.skills.get(self.selected_index) {
                    let id = &skill.id;
                    match config.skills.mode {
                        SkillSelectionMode::Auto => {
                            // Cycle: Auto (neither in enabled nor disabled) -> Disabled -> Enabled -> Auto
                            if config.skills.disabled.contains(id) {
                                config.skills.disabled.retain(|s| s != id);
                                config.skills.enabled.push(id.clone());
                            } else if config.skills.enabled.contains(id) {
                                config.skills.enabled.retain(|s| s != id);
                            } else {
                                config.skills.disabled.push(id.clone());
                            }
                        }
                        SkillSelectionMode::Manual => {
                            if config.skills.enabled.contains(id) {
                                config.skills.enabled.retain(|s| s != id);
                            } else {
                                config.skills.enabled.push(id.clone());
                            }
                        }
                    }
                    self.skills_config = config.skills.clone();
                    self.has_modified = true;
                    let _ = config.save();
                    Some(SkillsModalAction::SkillsChanged)
                } else {
                    None
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.creating_new = true;
                self.new_id_input.clear();
                None
            }
            _ => None,
        }
    }
}

pub struct SkillsModal<'a> {
    state: &'a SkillsModalState,
    lang: Language,
}

impl<'a> SkillsModal<'a> {
    pub fn new(state: &'a SkillsModalState, lang: Language) -> Self {
        Self { state, lang }
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_width = (area.width * 90 / 100).clamp(80, 120).min(area.width);
        let modal_height = (area.height * 85 / 100).clamp(20, 32).min(area.height);

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                format!(" {} ", self.lang.t(I18nKey::SkillsModalTitle)),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        block.render(modal_area, buf);

        let inner = Rect {
            x: modal_area.x + 1,
            y: modal_area.y + 1,
            width: modal_area.width.saturating_sub(2),
            height: modal_area.height.saturating_sub(2),
        };

        if inner.height < 5 || inner.width < 10 {
            return;
        }

        // Layout: Subheader (mode) -> Main body (columns) -> Footer (actions)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Subheader with Mode
                Constraint::Min(5),    // Body
                Constraint::Length(1), // Separator
                Constraint::Length(1), // Footer
            ])
            .split(inner);

        // 1. Subheader: Active Mode
        let mode_label = match self.state.skills_config.mode {
            SkillSelectionMode::Auto => Span::styled(
                format!(" {} ", self.lang.t(I18nKey::SkillsModeAuto)),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            SkillSelectionMode::Manual => Span::styled(
                format!(" {} ", self.lang.t(I18nKey::SkillsModeManual)),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        };
        let switch_help = Span::styled(
            format!("  ({})", self.lang.t(I18nKey::SkillsModeHelp)),
            Style::default().fg(Color::DarkGray),
        );
        let header_line = if let Some((ts, ref id)) = self.state.status_message {
            if ts.elapsed().as_secs() < 4 {
                Line::from(vec![
                    mode_label,
                    Span::raw("  "),
                    Span::styled(
                        format!("✨ {} ({})", self.lang.t(I18nKey::SkillsCreatedSuccess), id),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                Line::from(vec![mode_label, switch_help])
            }
        } else {
            Line::from(vec![mode_label, switch_help])
        };
        let header_p = Paragraph::new(header_line);
        header_p.render(chunks[0], buf);

        // 2. Main body: 2 columns (Skills List vs Details)
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(chunks[1]);

        self.render_skills_list(body_chunks[0], buf);
        self.render_skill_details(body_chunks[1], buf);

        // 3. Horizontal separator line
        let sep_style = Style::default().fg(Color::Rgb(40, 55, 75));
        for cx in inner.left()..inner.right() {
            buf.set_string(cx, chunks[2].top(), "─", sep_style);
        }

        // 4. Footer shortcuts
        self.render_footer(chunks[3], buf);

        // Render Creation dialog if active
        if self.state.creating_new {
            self.render_create_dialog(modal_area, buf);
        }
    }

    fn render_skills_list(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(50, 65, 85)));
        block.render(area, buf);

        let list_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width.saturating_sub(1),
            height: area.height,
        };

        if self.state.skills.is_empty() {
            let p = Paragraph::new(Span::styled(
                self.lang.t(I18nKey::SkillsEmptyList),
                Style::default().fg(Color::DarkGray),
            ));
            p.render(list_area, buf);
            return;
        }

        for (i, skill) in self.state.skills.iter().enumerate() {
            if i >= list_area.height as usize {
                break;
            }
            let row_y = list_area.top() + i as u16;
            let is_selected = i == self.state.selected_index;

            let skill_state = if self.state.skills_config.disabled.contains(&skill.id) {
                SkillState::Disabled
            } else if self.state.skills_config.enabled.contains(&skill.id) {
                SkillState::Enabled
            } else {
                SkillState::Auto
            };

            let (state_text, state_color) = match (self.state.skills_config.mode, skill_state) {
                (SkillSelectionMode::Auto, SkillState::Disabled) => {
                    (self.lang.t(I18nKey::SkillsStateDisabled), Color::DarkGray)
                }
                (SkillSelectionMode::Auto, SkillState::Enabled) => {
                    (self.lang.t(I18nKey::SkillsStateEnabled), Color::Green)
                }
                (SkillSelectionMode::Auto, SkillState::Auto) => {
                    (self.lang.t(I18nKey::SkillsStateAuto), Color::Cyan)
                }
                (SkillSelectionMode::Manual, SkillState::Enabled) => {
                    (self.lang.t(I18nKey::SkillsStateEnabled), Color::Green)
                }
                (SkillSelectionMode::Manual, _) => {
                    (self.lang.t(I18nKey::SkillsStateDisabled), Color::DarkGray)
                }
            };

            let (source_badge, source_color) = match skill.source {
                SkillSource::Builtin => (" [B]", Color::Yellow),
                SkillSource::Global => (" [G]", Color::Magenta),
                SkillSource::Project => (" [P]", Color::LightBlue),
            };

            let prefix = if is_selected { "❯ " } else { "  " };

            let row_style = if is_selected {
                Style::default().bg(Color::Rgb(30, 42, 60))
            } else {
                Style::default()
            };

            // Clear row background
            for cx in list_area.left()..list_area.right() {
                if let Some(cell) = buf.cell_mut((cx, row_y)) {
                    cell.set_style(row_style);
                }
            }

            let line = Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default().fg(if is_selected {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("[{}] ", state_text),
                    Style::default()
                        .fg(state_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &skill.name,
                    Style::default()
                        .fg(if is_selected {
                            Color::White
                        } else {
                            Color::Gray
                        })
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(source_badge, Style::default().fg(source_color)),
            ]);

            buf.set_line(list_area.left(), row_y, &line, list_area.width);
        }
    }

    fn render_skill_details(&self, area: Rect, buf: &mut Buffer) {
        let Some(skill) = self.state.skills.get(self.state.selected_index) else {
            return;
        };

        let mut lines = Vec::new();

        // 1. Skill Title & ID
        lines.push(Line::from(vec![
            Span::styled(
                &skill.name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", skill.id),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        // 2. Source & Location
        let source_str = match skill.source {
            SkillSource::Builtin => self.lang.t(I18nKey::SkillsSourceBuiltin),
            SkillSource::Global => self.lang.t(I18nKey::SkillsSourceGlobal),
            SkillSource::Project => self.lang.t(I18nKey::SkillsSourceProject),
        };
        let location = skill
            .file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "spiritty-internal".to_string());
        lines.push(Line::from(vec![
            Span::styled(self.lang.t(I18nKey::SkillsTypeLabel), Style::default().fg(Color::DarkGray)),
            Span::styled(source_str, Style::default().fg(Color::Yellow)),
            Span::styled(format!(" ({})", location), Style::default().fg(Color::DarkGray)),
        ]));

        lines.push(Line::from(""));

        // 3. Description
        lines.push(Line::from(vec![
            Span::styled(
                self.lang.t(I18nKey::SkillsDetailDescription),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(&skill.description, Style::default().fg(Color::White)),
        ]));

        lines.push(Line::from(""));

        // 4. Triggers
        let triggers_joined = skill.triggers.join(", ");
        lines.push(Line::from(vec![
            Span::styled(
                self.lang.t(I18nKey::SkillsDetailTriggers),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(triggers_joined, Style::default().fg(Color::LightCyan)),
        ]));

        lines.push(Line::from(""));

        // 5. Directives Header
        lines.push(Line::from(Span::styled(
            self.lang.t(I18nKey::SkillsDetailDirectives),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));

        // 6. Directives Content
        for line in skill.content.lines() {
            let l_trimmed = line.trim();
            if l_trimmed.starts_with('#') {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
            } else if l_trimmed.starts_with('-') || l_trimmed.starts_with('*') {
                lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        line.trim_start_matches(['-', '*', ' ']),
                        Style::default().fg(Color::White),
                    ),
                ]));
            } else if l_trimmed.starts_with("```") {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Gray),
                )));
            }
        }

        let p = Paragraph::new(lines)
            .scroll((self.state.details_scroll, 0));
        p.render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let mut spans = Vec::new();

        let space_key = if self.lang == Language::Fr { "Espace" } else { "Space" };
        let esc_key = if self.lang == Language::Fr { "Échap" } else { "Esc" };

        spans.extend(key_pill(space_key, Color::Cyan));
        spans.push(Span::styled(
            format!(" {}  ", self.lang.t(I18nKey::SkillsHelpToggleState)),
            Style::default().fg(Color::White),
        ));

        spans.extend(key_pill("Tab/M", Color::Cyan));
        spans.push(Span::styled(
            format!(" {}  ", self.lang.t(I18nKey::SkillsHelpToggleMode)),
            Style::default().fg(Color::White),
        ));

        spans.extend(key_pill("N", Color::Yellow));
        spans.push(Span::styled(
            format!(" {}  ", self.lang.t(I18nKey::SkillsHelpNew)),
            Style::default().fg(Color::White),
        ));

        spans.extend(key_pill("Shift+↑/↓", Color::DarkGray));
        spans.push(Span::styled(
            format!(" {}  ", self.lang.t(I18nKey::SkillsHelpScroll)),
            Style::default().fg(Color::DarkGray),
        ));

        spans.extend(key_pill(esc_key, Color::DarkGray));
        spans.push(Span::styled(
            format!(" {}", self.lang.t(I18nKey::SkillsHelpClose)),
            Style::default().fg(Color::DarkGray),
        ));

        let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
        p.render(area, buf);
    }

    fn render_create_dialog(&self, area: Rect, buf: &mut Buffer) {
        let dialog_w = 60.min(area.width);
        let dialog_h = 7.min(area.height);
        let x = area.left() + (area.width.saturating_sub(dialog_w)) / 2;
        let y = area.top() + (area.height.saturating_sub(dialog_h)) / 2;
        let dialog_rect = Rect::new(x, y, dialog_w, dialog_h);

        Clear.render(dialog_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(
                format!(" {} ", self.lang.t(I18nKey::SkillsNewModalTitle)),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        block.render(dialog_rect, buf);

        let prompt_y = dialog_rect.top() + 2;
        let prompt_text = self.lang.t(I18nKey::SkillsNewPromptId);
        buf.set_string(dialog_rect.left() + 2, prompt_y, prompt_text, Style::default().fg(Color::White));

        let input_x = dialog_rect.left() + 2 + prompt_text.len() as u16;
        buf.set_string(
            input_x,
            prompt_y,
            format!("{}▌", self.state.new_id_input),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );

        let hint = self.lang.t(I18nKey::SkillsNewModalHint);
        buf.set_string(
            dialog_rect.left() + (dialog_w.saturating_sub(hint.len() as u16)) / 2,
            dialog_rect.bottom().saturating_sub(2),
            hint,
            Style::default().fg(Color::DarkGray),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_skills_modal_state_navigation_and_mode_toggle() {
        let mut config = Config::default();
        assert_eq!(config.skills.mode, SkillSelectionMode::Auto);

        let mut state = SkillsModalState::new(&config.skills);
        assert!(!state.skills.is_empty());
        assert_eq!(state.selected_index, 0);

        // Move down
        state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut config,
        );
        assert_eq!(state.selected_index, 1);

        // Move up
        state.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut config,
        );
        assert_eq!(state.selected_index, 0);

        // Toggle mode via Tab
        let action = state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut config,
        );
        assert_eq!(action, Some(SkillsModalAction::SkillsChanged));
        assert_eq!(config.skills.mode, SkillSelectionMode::Manual);
        assert_eq!(state.skills_config.mode, SkillSelectionMode::Manual);

        // Toggle mode back via 'm'
        let action = state.handle_key(
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
            &mut config,
        );
        assert_eq!(action, Some(SkillsModalAction::SkillsChanged));
        assert_eq!(config.skills.mode, SkillSelectionMode::Auto);
        assert_eq!(state.skills_config.mode, SkillSelectionMode::Auto);
    }

    #[test]
    fn test_skills_modal_space_toggle_cycle() {
        let mut config = Config::default();
        let mut state = SkillsModalState::new(&config.skills);
        let first_id = state.skills[0].id.clone();

        // 1. In Auto mode: initially neither disabled nor enabled -> Auto
        assert!(!config.skills.disabled.contains(&first_id));
        assert!(!config.skills.enabled.contains(&first_id));

        // Press space -> Disabled
        state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut config,
        );
        assert!(config.skills.disabled.contains(&first_id));
        assert!(!config.skills.enabled.contains(&first_id));

        // Press space -> Enabled
        state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut config,
        );
        assert!(!config.skills.disabled.contains(&first_id));
        assert!(config.skills.enabled.contains(&first_id));

        // Press space -> Auto (removed from both)
        state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut config,
        );
        assert!(!config.skills.disabled.contains(&first_id));
        assert!(!config.skills.enabled.contains(&first_id));
    }

    #[test]
    fn test_skills_modal_create_input_flow() {
        let mut config = Config::default();
        let mut state = SkillsModalState::new(&config.skills);

        // Press 'n' to enter creation mode
        state.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &mut config,
        );
        assert!(state.creating_new);

        // Type characters
        state.handle_key(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            &mut config,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE),
            &mut config,
        );
        state.handle_paste("-tool");
        assert_eq!(state.new_id_input, "k8-tool");

        // Backspace
        state.handle_key(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut config,
        );
        assert_eq!(state.new_id_input, "k8-too");

        // Esc cancels
        state.handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut config,
        );
        assert!(!state.creating_new);
        assert!(state.new_id_input.is_empty());
    }

    #[test]
    fn test_skills_modal_render_smoke() {
        let config = Config::default();
        let state = SkillsModalState::new(&config.skills);
        let modal = SkillsModal::new(&state, Language::Fr);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        modal.render(area, &mut buf);
    }
}
