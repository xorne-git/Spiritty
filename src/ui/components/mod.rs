pub mod bookmarks_modal;
pub mod config_modal;
pub mod export_modal;
pub mod help_modal;
pub mod mcp_modal;
pub mod rename_tab_modal;
pub mod session_modal;
pub mod ssh_reconnect_modal;

pub use bookmarks_modal::{
    AddHostState, BookmarksModal, BookmarksModalAction, BookmarksModalState,
};
pub use config_modal::{ConfigModalAction, ConfigModalState};
pub use export_modal::{ExportModal, ExportModalAction, ExportModalState};
pub use help_modal::HelpModal;
pub use mcp_modal::{AddMcpState, McpModal, McpModalAction, McpModalState};
pub use rename_tab_modal::{RenameTabAction, RenameTabModalState};
pub use session_modal::{SessionModalAction, SessionModalState};
pub use ssh_reconnect_modal::{SshReconnectAction, SshReconnectModal};

use ratatui::widgets::Widget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalOutcome {
    None,
    Close,
    LoadSession(String),
    NewSession,
    ConnectSsh(String),
    TriggerHostScan,
    ExportMarkdown(String),
    McpServersChanged,
    SaveConfigAndClose,
    UpdatePricing,
    RefreshProviderModels,
    SetTabTitle {
        tab_index: usize,
        title: Option<String>,
    },
}

#[derive(Default)]
pub enum ModalState {
    #[default]
    None,
    Help,
    Config(ConfigModalState),
    Sessions(SessionModalState),
    Bookmarks(BookmarksModalState),
    Export(ExportModalState),
    Mcp(McpModalState),
    /// Small prompt offering to reconnect to the SSH host of a `-c`-resumed session
    /// whose PTY is currently local.
    SshReconnect {
        target: String,
    },
    /// Modal allowing the user to set or reset the custom title of a terminal tab (Alt+R)
    RenameTab(RenameTabModalState),
}

impl ModalState {
    pub fn is_open(&self) -> bool {
        !matches!(self, ModalState::None)
    }

    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        config: &mut crate::config::Config,
        hosts_store: &mut crate::system::HostsStore,
    ) -> ModalOutcome {
        match self {
            ModalState::None => ModalOutcome::None,
            ModalState::Help => {
                if HelpModal::handle_key(key) {
                    ModalOutcome::Close
                } else {
                    ModalOutcome::None
                }
            }
            ModalState::Config(config_state) => match config_state.handle_key(key, config) {
                ConfigModalAction::SaveAndClose => ModalOutcome::SaveConfigAndClose,
                ConfigModalAction::Close => ModalOutcome::Close,
                ConfigModalAction::UpdatePricing => ModalOutcome::UpdatePricing,
                ConfigModalAction::RefreshModelsAndPricing => ModalOutcome::RefreshProviderModels,
                ConfigModalAction::None => ModalOutcome::None,
            },
            ModalState::Sessions(session_state) => match session_state.handle_key(key) {
                Some(SessionModalAction::Load(id)) => ModalOutcome::LoadSession(id),
                Some(SessionModalAction::NewSession) => ModalOutcome::NewSession,
                Some(SessionModalAction::Close) => ModalOutcome::Close,
                None => ModalOutcome::None,
            },
            ModalState::Bookmarks(bm_state) => match bm_state.handle_key(key, hosts_store) {
                Some(BookmarksModalAction::Connect(target)) => ModalOutcome::ConnectSsh(target),
                Some(BookmarksModalAction::TriggerScan) => ModalOutcome::TriggerHostScan,
                Some(BookmarksModalAction::Close) => ModalOutcome::Close,
                None => ModalOutcome::None,
            },
            ModalState::Export(export_state) => match export_state.handle_key(key) {
                Some(ExportModalAction::Export(path)) => ModalOutcome::ExportMarkdown(path),
                Some(ExportModalAction::Close) => ModalOutcome::Close,
                None => ModalOutcome::None,
            },
            ModalState::Mcp(mcp_state) => match mcp_state.handle_key(key, config) {
                Some(McpModalAction::ServersChanged) => ModalOutcome::McpServersChanged,
                Some(McpModalAction::Close) => ModalOutcome::Close,
                None => ModalOutcome::None,
            },
            ModalState::SshReconnect { target } => {
                match SshReconnectModal::handle_key(key, target) {
                    Some(SshReconnectAction::Connect(t)) => ModalOutcome::ConnectSsh(t),
                    Some(SshReconnectAction::Close) => ModalOutcome::Close,
                    None => ModalOutcome::None,
                }
            }
            ModalState::RenameTab(rename_state) => match rename_state.handle_key(key) {
                Some(RenameTabAction::Save(title)) => ModalOutcome::SetTabTitle {
                    tab_index: rename_state.tab_index,
                    title,
                },
                Some(RenameTabAction::Close) => ModalOutcome::Close,
                None => ModalOutcome::None,
            },
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        match self {
            ModalState::Config(config_state) => config_state.handle_paste(text.to_string()),
            ModalState::Bookmarks(bm_state) => bm_state.handle_paste(text.to_string()),
            ModalState::Export(export_state) => export_state.handle_paste(text.to_string()),
            ModalState::Mcp(mcp_state) => mcp_state.handle_paste(text.to_string()),
            ModalState::RenameTab(rename_state) => rename_state.handle_paste(text),
            ModalState::None
            | ModalState::Help
            | ModalState::Sessions(_)
            | ModalState::SshReconnect { .. } => {}
        }
    }

    pub fn render(
        &self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: crate::ui::theme::ThemeId,
        lang: crate::i18n::Language,
    ) {
        match self {
            ModalState::None => {}
            ModalState::Help => HelpModal::render_modal(area, buf, lang),
            ModalState::Config(config_state) => config_state.render_modal(area, buf, lang),
            ModalState::Sessions(session_state) => session_state.render_modal(area, buf, lang),
            ModalState::Bookmarks(bm_state) => {
                BookmarksModal::render_modal(area, buf, bm_state, lang)
            }
            ModalState::Export(export_state) => {
                ExportModal::render_modal(area, buf, export_state, lang)
            }
            ModalState::Mcp(mcp_state) => McpModal::new(mcp_state, lang).render(area, buf),
            ModalState::SshReconnect { target } => {
                SshReconnectModal::render_modal(area, buf, target, lang)
            }
            ModalState::RenameTab(rename_state) => rename_state.render(area, buf, theme, lang),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_modal_state_lifecycle_and_outcomes() {
        let mut config = crate::config::Config::default();
        let mut hosts_store = crate::system::HostsStore::default();
        let key = |c| KeyEvent::new(c, KeyModifiers::NONE);

        // 1. None
        let mut modal = ModalState::None;
        assert!(!modal.is_open());
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter), &mut config, &mut hosts_store),
            ModalOutcome::None
        );

        // 2. Help
        modal = ModalState::Help;
        assert!(modal.is_open());
        assert_eq!(
            modal.handle_key(key(KeyCode::Esc), &mut config, &mut hosts_store),
            ModalOutcome::Close
        );

        // 3. SshReconnect
        modal = ModalState::SshReconnect {
            target: "user@remote.host".to_string(),
        };
        assert!(modal.is_open());
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter), &mut config, &mut hosts_store),
            ModalOutcome::ConnectSsh("user@remote.host".to_string())
        );

        modal = ModalState::SshReconnect {
            target: "user@remote.host".to_string(),
        };
        assert_eq!(
            modal.handle_key(key(KeyCode::Char('n')), &mut config, &mut hosts_store),
            ModalOutcome::Close
        );

        // 4. RenameTab
        modal = ModalState::RenameTab(RenameTabModalState::new(2, "old-name".to_string()));
        assert!(modal.is_open());
        // Type chars
        modal.handle_key(key(KeyCode::Backspace), &mut config, &mut hosts_store);
        modal.handle_paste(" new");
        assert_eq!(
            modal.handle_key(key(KeyCode::Enter), &mut config, &mut hosts_store),
            ModalOutcome::SetTabTitle {
                tab_index: 2,
                title: Some("old-nam new".to_string()),
            }
        );
    }
}
