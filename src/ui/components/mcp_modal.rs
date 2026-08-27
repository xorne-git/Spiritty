use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, Widget},
};

use crate::agent::mcp::manager::{McpServerStatus, McpStatus};
use crate::config::{Config, McpServerConfig};
use crate::i18n::Language;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpModalAction {
    ServersChanged,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddMcpState {
    None,
    EnteringName {
        input: String,
        cursor: usize,
    },
    EnteringCommand {
        name: String,
        input: String,
        cursor: usize,
    },
    EnteringArgs {
        name: String,
        command: String,
        input: String,
        cursor: usize,
    },
}

pub struct McpModalState {
    pub servers: Vec<McpServerStatus>,
    pub selected_index: usize,
    pub add_state: AddMcpState,
    pub confirm_delete_name: Option<String>,
    pub status_message: Option<(std::time::Instant, String)>,
}

impl McpModalState {
    pub fn new(statuses: Vec<McpServerStatus>) -> Self {
        Self {
            servers: statuses,
            selected_index: 0,
            add_state: AddMcpState::None,
            confirm_delete_name: None,
            status_message: None,
        }
    }

    pub fn update_statuses(&mut self, statuses: Vec<McpServerStatus>) {
        for new_s in statuses {
            if let Some(pos) = self.servers.iter().position(|s| s.name == new_s.name) {
                self.servers[pos] = new_s;
            } else {
                self.servers.push(new_s);
            }
        }
        if self.selected_index >= self.servers.len() && !self.servers.is_empty() {
            self.selected_index = self.servers.len() - 1;
        }
    }

    pub fn sync_with_config(&mut self, config: &Config) {
        self.servers
            .retain(|s| config.mcp_servers.contains_key(&s.name));
        for (name, s_cfg) in &config.mcp_servers {
            if let Some(existing) = self.servers.iter_mut().find(|s| s.name == *name) {
                existing.command = s_cfg.command.clone();
                existing.args = s_cfg.args.clone();
                existing.enabled = s_cfg.enabled;
            } else {
                self.servers.push(McpServerStatus {
                    name: name.clone(),
                    command: s_cfg.command.clone(),
                    args: s_cfg.args.clone(),
                    enabled: s_cfg.enabled,
                    status: if s_cfg.enabled {
                        McpStatus::Connected(0)
                    } else {
                        McpStatus::Disabled
                    },
                    tools: Vec::new(),
                });
            }
        }
        if self.selected_index >= self.servers.len() && !self.servers.is_empty() {
            self.selected_index = self.servers.len() - 1;
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        let clean = text.replace(['\r', '\n'], " ");
        if clean.is_empty() {
            return;
        }

        match &mut self.add_state {
            AddMcpState::EnteringName { input, cursor } => {
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
            AddMcpState::EnteringCommand { input, cursor, .. } => {
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
            AddMcpState::EnteringArgs { input, cursor, .. } => {
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
            AddMcpState::None => {}
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, config: &mut Config) -> Option<McpModalAction> {
        // Confirmation for delete
        if let Some(name) = self.confirm_delete_name.clone() {
            match key.code {
                KeyCode::Char('y')
                | KeyCode::Char('Y')
                | KeyCode::Char('o')
                | KeyCode::Char('O')
                | KeyCode::Enter => {
                    config.mcp_servers.remove(&name);
                    let _ = config.save();
                    self.servers.retain(|s| s.name != name);
                    if self.selected_index >= self.servers.len() && !self.servers.is_empty() {
                        self.selected_index = self.servers.len() - 1;
                    }
                    self.confirm_delete_name = None;
                    self.status_message = Some((
                        std::time::Instant::now(),
                        format!("Serveur MCP '{}' supprimé", name),
                    ));
                    return Some(McpModalAction::ServersChanged);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_delete_name = None;
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

        if is_paste && !matches!(self.add_state, AddMcpState::None) {
            if let Some(text) = crate::system::clipboard::get_clipboard_text() {
                self.handle_paste(text);
                return None;
            }
        }

        // Add Wizard
        let mut next_add_state = None;
        let mut server_changed = false;

        match &mut self.add_state {
            AddMcpState::EnteringName { input, cursor } => match key.code {
                KeyCode::Esc => {
                    next_add_state = Some(AddMcpState::None);
                }
                KeyCode::Enter => {
                    let name = input.trim().to_string();
                    if !name.is_empty() {
                        next_add_state = Some(AddMcpState::EnteringCommand {
                            name,
                            input: String::new(),
                            cursor: 0,
                        });
                    }
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
            },
            AddMcpState::EnteringCommand {
                name,
                input,
                cursor,
            } => match key.code {
                KeyCode::Esc => {
                    next_add_state = Some(AddMcpState::None);
                }
                KeyCode::Enter => {
                    let command = input.trim().to_string();
                    if !command.is_empty() {
                        next_add_state = Some(AddMcpState::EnteringArgs {
                            name: name.clone(),
                            command,
                            input: String::new(),
                            cursor: 0,
                        });
                    }
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
            },
            AddMcpState::EnteringArgs {
                name,
                command,
                input,
                cursor,
            } => match key.code {
                KeyCode::Esc => {
                    next_add_state = Some(AddMcpState::None);
                }
                KeyCode::Enter => {
                    let raw_args = input.trim();
                    let args: Vec<String> = if raw_args.is_empty() {
                        Vec::new()
                    } else {
                        raw_args.split_whitespace().map(|s| s.to_string()).collect()
                    };

                    let new_status = McpServerStatus {
                        name: name.clone(),
                        command: command.clone(),
                        args: args.clone(),
                        enabled: true,
                        status: McpStatus::Connected(0),
                        tools: Vec::new(),
                    };

                    config.mcp_servers.insert(
                        name.clone(),
                        McpServerConfig {
                            command: command.clone(),
                            args,
                            env: std::collections::HashMap::new(),
                            enabled: true,
                        },
                    );
                    let _ = config.save();

                    if let Some(pos) = self.servers.iter().position(|s| s.name == *name) {
                        self.servers[pos] = new_status;
                        self.selected_index = pos;
                    } else {
                        self.servers.push(new_status);
                        self.selected_index = self.servers.len().saturating_sub(1);
                    }

                    self.status_message = Some((
                        std::time::Instant::now(),
                        format!("Serveur MCP '{}' ajouté", name),
                    ));
                    next_add_state = Some(AddMcpState::None);
                    server_changed = true;
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
            },
            AddMcpState::None => {}
        }

        if let Some(next) = next_add_state {
            self.add_state = next;
            if server_changed {
                return Some(McpModalAction::ServersChanged);
            }
            return None;
        }

        if server_changed {
            return Some(McpModalAction::ServersChanged);
        }

        if self.add_state != AddMcpState::None {
            return None;
        }

        // Standard Navigation & Actions
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(McpModalAction::Close),
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.servers.is_empty() {
                    if self.selected_index == 0 {
                        self.selected_index = self.servers.len() - 1;
                    } else {
                        self.selected_index -= 1;
                    }
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.servers.is_empty() {
                    if self.selected_index + 1 >= self.servers.len() {
                        self.selected_index = 0;
                    } else {
                        self.selected_index += 1;
                    }
                }
                None
            }
            KeyCode::Char(' ') => {
                if let Some(srv) = self.servers.get_mut(self.selected_index) {
                    if let Some(cfg) = config.mcp_servers.get_mut(&srv.name) {
                        cfg.enabled = !cfg.enabled;
                        srv.enabled = cfg.enabled;
                        srv.status = if srv.enabled {
                            McpStatus::Connected(srv.tools.len())
                        } else {
                            McpStatus::Disabled
                        };
                        let _ = config.save();
                        return Some(McpModalAction::ServersChanged);
                    }
                }
                None
            }
            KeyCode::Char('r') | KeyCode::Char('R') => Some(McpModalAction::ServersChanged),
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.add_state = AddMcpState::EnteringName {
                    input: String::new(),
                    cursor: 0,
                };
                None
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(srv) = self.servers.get(self.selected_index) {
                    self.confirm_delete_name = Some(srv.name.clone());
                }
                None
            }
            _ => None,
        }
    }
}

pub struct McpModal<'a> {
    state: &'a McpModalState,
    _language: Language,
}

/// Renders an editable input with the cursor block positioned at `cursor` (a char index).
fn wizard_input_spans<'a>(input: &'a str, cursor: usize) -> Vec<Span<'a>> {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    let after: String = chars[cursor..].iter().collect();
    vec![
        Span::styled(
            before,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(Color::Yellow)),
        Span::styled(
            after,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]
}

impl<'a> McpModal<'a> {
    pub fn new(state: &'a McpModalState, language: Language) -> Self {
        Self {
            state,
            _language: language,
        }
    }
}

impl<'a> Widget for McpModal<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.clamp(60, 100);
        let height = area.height.clamp(20, 34);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        Clear.render(modal_area, buf);

        let main_block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Rgb(140, 100, 240)))
            .title(Span::styled(
                " 🔌 Serveurs MCP (Model Context Protocol) ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Left)
            .padding(Padding::uniform(1));

        let inner_area = main_block.inner(modal_area);
        main_block.render(modal_area, buf);

        let chunks = Layout::default()
            .constraints([
                Constraint::Length(9), // Servers list
                Constraint::Min(6),    // Tools inspector
                Constraint::Length(2), // Status / Wizard
                Constraint::Length(1), // Shortcuts footer
            ])
            .split(inner_area);

        // 1. Table of Servers (with scrolling so the selected row stays visible)
        let total = self.state.servers.len();
        let visible = (chunks[0].height.saturating_sub(1)).max(1) as usize;
        let sel = self.state.selected_index.min(total.saturating_sub(1));
        let mut offset = 0usize;
        if total > visible {
            if sel >= visible {
                offset = sel - visible + 1;
            }
            offset = offset.min(total - visible);
        }

        let mut rows = Vec::new();
        for (i, s) in self
            .state
            .servers
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
        {
            let is_selected = i == self.state.selected_index;
            let check_icon = if s.enabled { " [x] " } else { " [ ] " };
            let (status_text, status_style) = match &s.status {
                McpStatus::Connected(count) => (
                    format!("🟢 Actif ({} outils)", count),
                    Style::default().fg(Color::Green),
                ),
                McpStatus::Disabled => (
                    "⚪ Désactivé".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                McpStatus::Error(e) => {
                    let short_e = if e.len() > 25 {
                        let cut = e.floor_char_boundary(25);
                        format!("{}…", &e[..cut])
                    } else {
                        e.clone()
                    };
                    (
                        format!("🔴 Erreur : {}", short_e),
                        Style::default().fg(Color::Red),
                    )
                }
            };

            let prefix = if is_selected { "▶ " } else { "  " };
            let row_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(40, 45, 65))
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let cmd_str = format!("{} {}", s.command, s.args.join(" "));
            let cmd_display = if cmd_str.len() > 32 {
                let cut = cmd_str.floor_char_boundary(32);
                format!("{}…", &cmd_str[..cut])
            } else {
                cmd_str
            };

            rows.push(Row::new(vec![
                Span::styled(format!("{}{}", prefix, check_icon), row_style),
                Span::styled(&s.name, row_style),
                Span::styled(cmd_display, Style::default().fg(Color::Gray)),
                Span::styled(status_text, status_style),
            ]));
        }

        let table_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Serveurs configurés ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

        let table = Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Length(18),
                Constraint::Min(25),
                Constraint::Length(24),
            ],
        )
        .header(
            Row::new(vec![
                Span::styled(" Actif", Style::default().fg(Color::DarkGray)),
                Span::styled("Nom", Style::default().fg(Color::DarkGray)),
                Span::styled("Commande", Style::default().fg(Color::DarkGray)),
                Span::styled("Statut", Style::default().fg(Color::DarkGray)),
            ])
            .bottom_margin(0),
        )
        .block(table_block);

        table.render(chunks[0], buf);

        // 2. Tools Inspector for selected server
        let detail_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Outils exposés au modèle IA ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let detail_inner = detail_block.inner(chunks[1]);
        detail_block.render(chunks[1], buf);

        if let Some(selected) = self.state.servers.get(self.state.selected_index) {
            if selected.tools.is_empty() {
                let msg = match &selected.status {
                    McpStatus::Connected(_) => "Aucun outil exposé par ce serveur.",
                    McpStatus::Disabled => {
                        "Serveur désactivé. Appuyez sur [Espace] pour l'activer."
                    }
                    McpStatus::Error(e) => e.as_str(),
                };
                Paragraph::new(Span::styled(
                    format!("  {}", msg),
                    Style::default().fg(Color::DarkGray),
                ))
                .render(detail_inner, buf);
            } else {
                let mut tool_lines = Vec::new();
                for t in &selected.tools {
                    let desc = t.description.as_deref().unwrap_or("Sans description");
                    tool_lines.push(Line::from(vec![
                        Span::styled(
                            format!("  • mcp:{}:{}", selected.name, t.name),
                            Style::default()
                                .fg(Color::LightCyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" - "),
                        Span::styled(desc, Style::default().fg(Color::White)),
                    ]));
                }
                Paragraph::new(tool_lines).render(detail_inner, buf);
            }
        } else {
            Paragraph::new(Span::styled(
                "  Aucun serveur MCP configuré. Appuyez sur [A] pour ajouter un serveur.",
                Style::default().fg(Color::DarkGray),
            ))
            .render(detail_inner, buf);
        }

        // 3. Status or Add Wizard Area
        if let Some(ref name) = self.state.confirm_delete_name {
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "⚠️ Supprimer le serveur MCP '",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    name,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "' ? [O]ui / [N]on",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]))
            .render(chunks[2], buf);
        } else {
            match &self.state.add_state {
                AddMcpState::EnteringName { input, cursor } => {
                    let mut spans = vec![
                        Span::styled(
                            " [Ajout 1/3] ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("Nom du serveur : "),
                    ];
                    spans.extend(wizard_input_spans(input, *cursor));
                    spans.push(Span::styled(
                        " (Entrée pour valider, Echap annuler)",
                        Style::default().fg(Color::DarkGray),
                    ));
                    Paragraph::new(Line::from(spans)).render(chunks[2], buf);
                }
                AddMcpState::EnteringCommand {
                    name,
                    input,
                    cursor,
                } => {
                    let mut spans = vec![
                        Span::styled(
                            " [Ajout 2/3] ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("Commande pour '{}' : ", name)),
                    ];
                    spans.extend(wizard_input_spans(input, *cursor));
                    spans.push(Span::styled(
                        " (ex: npx, uvx, docker...)",
                        Style::default().fg(Color::DarkGray),
                    ));
                    Paragraph::new(Line::from(spans)).render(chunks[2], buf);
                }
                AddMcpState::EnteringArgs {
                    command,
                    input,
                    cursor,
                    ..
                } => {
                    let mut spans = vec![
                        Span::styled(
                            " [Ajout 3/3] ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("Arguments pour '{}' : ", command)),
                    ];
                    spans.extend(wizard_input_spans(input, *cursor));
                    spans.push(Span::styled(
                        " (ex: -y @modelcontextprotocol/server-filesystem /tmp)",
                        Style::default().fg(Color::DarkGray),
                    ));
                    Paragraph::new(Line::from(spans)).render(chunks[2], buf);
                }
                AddMcpState::None => {
                    if let Some((_, ref msg)) = self.state.status_message {
                        Paragraph::new(Span::styled(
                            format!(" ℹ️ {}", msg),
                            Style::default().fg(Color::LightGreen),
                        ))
                        .render(chunks[2], buf);
                    }
                }
            }
        }

        // 4. Footer shortcuts
        let footer_spans = vec![
            Span::styled(" [↑/↓] ", Style::default().fg(Color::Yellow)),
            Span::raw("Naviguer "),
            Span::styled(" [Espace] ", Style::default().fg(Color::Yellow)),
            Span::raw("Activer/Désactiver "),
            Span::styled(" [R] ", Style::default().fg(Color::Yellow)),
            Span::raw("Recharger "),
            Span::styled(" [A] ", Style::default().fg(Color::Yellow)),
            Span::raw("Ajouter "),
            Span::styled(" [D] ", Style::default().fg(Color::Yellow)),
            Span::raw("Supprimer "),
            Span::styled(" [Échap] ", Style::default().fg(Color::Yellow)),
            Span::raw("Fermer"),
        ];

        Paragraph::new(Line::from(footer_spans))
            .alignment(Alignment::Center)
            .render(chunks[3], buf);
    }
}
