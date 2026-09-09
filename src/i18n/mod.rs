pub mod en;
pub mod fr;

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Fr,
    En,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[Language::Fr, Language::En]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::Fr => "fr",
            Language::En => "en",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Fr => "Français",
            Language::En => "English (US)",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let clean = code.trim().to_lowercase();
        if clean.starts_with("fr") {
            Some(Language::Fr)
        } else if clean.starts_with("en") {
            Some(Language::En)
        } else {
            None
        }
    }

    pub fn detect_system() -> Self {
        for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = env::var(var) {
                if let Some(lang) = Self::from_code(&val) {
                    return lang;
                }
            }
        }
        // Fallback default
        Language::Fr
    }

    pub fn t(&self, key: I18nKey) -> &'static str {
        match self {
            Language::Fr => fr::translate(key),
            Language::En => en::translate(key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum I18nKey {
    // Config Modal
    ConfigModalTitle,
    ConfigFieldProvider,
    ConfigFieldAutoApprove,
    ConfigFieldTheme,
    ConfigFieldModel,
    ConfigFieldReasoning,
    ConfigFieldApiUrl,
    ConfigFieldApiKey,
    ConfigButtonSave,
    ConfigNavNavigate,
    ConfigNavValidateOrOpen,
    ConfigNavClose,
    ConfigPlaceholderSelectModel,
    ConfigPlaceholderDefaultUrl,
    ConfigPlaceholderNoKeyRequired,
    ConfigDropdownTitle,
    ConfigDropdownAddTitle,
    ConfigDropdownEditTitle,
    ConfigDropdownConfirm,
    ConfigDropdownCancel,
    ConfigDropdownActionAdd,
    ConfigDropdownActionEdit,
    ConfigDropdownActionDelete,
    ConfigDropdownTagActive,
    ConfigActionUpdatePricing,
    ConfigActionRefreshModels,
    ConfigRefreshInProgress,
    ConfigRefreshSuccess,
    ConfigRefreshFailed,
    ConfigRefreshMissingKey,
    PricingUpdateSuccess,
    PricingUpdateFailed,

    // Help Modal
    HelpModalTitle,
    HelpSectionNavigation,
    HelpSectionAgent,
    HelpSectionSessions,
    HelpSectionGeneral,
    HelpKeyShift,
    HelpKeyCtrl,
    HelpKeyAlt,
    HelpKeyTab,
    HelpKeySpace,
    HelpKeyOr,
    HelpKeyMouseClick,
    HelpKeyScroll,
    HelpKeyDrag,
    HelpKeyClose,
    HelpDescToggleFocus,
    HelpDescMouseClick,
    HelpDescScroll,
    HelpDescResizePanels,
    HelpDescConfigModal,
    HelpDescSessionModal,
    HelpDescNewSession,
    HelpDescAutoApprove,
    HelpDescBookmarksModal,
    HelpDescExportSession,
    HelpDescMcpModal,
    HelpDescChatSearch,
    HelpDescDiagnoseError,
    HelpDescNewTab,
    HelpDescCloseTab,
    HelpDescNextTab,
    HelpDescPrevTab,
    FooterApprovalLabel,
    HelpDescToggleHelp,
    HelpDescQuit,
    HelpDescCloseModal,
    HelpFooterPromptPrefix,
    HelpFooterPromptMiddle,
    HelpFooterPromptSuffix,

    // Sessions Modal
    SessionModalTitle,
    // SSH reconnect modal
    SshReconnectTitle,
    SshReconnectBody,
    SshReconnectConfirm,
    SshReconnectLater,
    SessionHeaderTitle,
    SessionHeaderModel,
    SessionHeaderMessages,
    SessionHeaderTokens,
    SessionHeaderDate,
    SessionTagActive,
    SessionActionLoad,
    SessionActionNew,
    SessionActionCompact,
    SessionActionDelete,
    SessionActionClose,
    SessionEmptyList,
    SessionConfirmDelete,

    // Chat Panel & UI
    ChatHeaderTitle,
    TerminalHeaderTitle,
    ChatInputPlaceholder,
    ChatThinking,
    ChatThoughtCompleted,
    ChatThoughtStreaming,
    ChatWelcomeTitle,
    ChatWelcomeSubtitle,

    // SSH & Host Management
    SshDetected,
    SshProfileLoaded,
    SshScanPrompt,
    SshScanSuccess,
    SshScanFailed,
    SshLocalRestored,
    HelpDescScanHost,
    TabRenameTitle,
    TabRenamePrompt,
    TabRenameHelp,
    HelpDescRenameTab,

    // Agent System Prompts
    AgentLanguageInstruction,
}

pub fn t(key: I18nKey, lang: Language) -> &'static str {
    lang.t(key)
}
