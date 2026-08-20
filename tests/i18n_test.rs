use spiritty::i18n::{I18nKey, Language};

#[test]
fn test_language_detection_and_fallback() {
    assert_eq!(Language::from_code("fr"), Some(Language::Fr));
    assert_eq!(Language::from_code("fr_FR.UTF-8"), Some(Language::Fr));
    assert_eq!(Language::from_code("en"), Some(Language::En));
    assert_eq!(Language::from_code("en_US.UTF-8"), Some(Language::En));
    assert_eq!(Language::from_code("de"), None);
}

#[test]
fn test_i18n_catalog_completeness() {
    let keys = [
        I18nKey::ConfigModalTitle,
        I18nKey::ConfigFieldProvider,
        I18nKey::ConfigFieldModel,
        I18nKey::ConfigFieldApiUrl,
        I18nKey::ConfigFieldApiKey,
        I18nKey::ConfigButtonSave,
        I18nKey::ConfigNavNavigate,
        I18nKey::ConfigNavValidateOrOpen,
        I18nKey::ConfigNavClose,
        I18nKey::ConfigPlaceholderSelectModel,
        I18nKey::ConfigPlaceholderDefaultUrl,
        I18nKey::ConfigPlaceholderNoKeyRequired,
        I18nKey::ConfigDropdownTitle,
        I18nKey::ConfigDropdownAddTitle,
        I18nKey::ConfigDropdownEditTitle,
        I18nKey::ConfigDropdownConfirm,
        I18nKey::ConfigDropdownCancel,
        I18nKey::ConfigDropdownActionAdd,
        I18nKey::ConfigDropdownActionEdit,
        I18nKey::ConfigDropdownActionDelete,
        I18nKey::ConfigDropdownTagActive,
        I18nKey::HelpModalTitle,
        I18nKey::HelpKeyShift,
        I18nKey::HelpKeyCtrl,
        I18nKey::HelpKeyAlt,
        I18nKey::HelpKeyTab,
        I18nKey::HelpKeySpace,
        I18nKey::HelpKeyOr,
        I18nKey::HelpKeyMouseClick,
        I18nKey::HelpKeyDrag,
        I18nKey::HelpKeyClose,
        I18nKey::HelpDescToggleFocus,
        I18nKey::HelpDescMouseClick,
        I18nKey::HelpDescResizePanels,
        I18nKey::HelpDescConfigModal,
        I18nKey::HelpDescToggleHelp,
        I18nKey::HelpDescQuit,
        I18nKey::HelpDescCloseModal,
        I18nKey::HelpFooterPromptPrefix,
        I18nKey::HelpFooterPromptMiddle,
        I18nKey::HelpFooterPromptSuffix,
        I18nKey::ChatHeaderTitle,
        I18nKey::TerminalHeaderTitle,
        I18nKey::ChatInputPlaceholder,
        I18nKey::ChatThinking,
        I18nKey::ChatThoughtCompleted,
        I18nKey::ChatThoughtStreaming,
        I18nKey::ChatWelcomeTitle,
        I18nKey::ChatWelcomeSubtitle,
        I18nKey::AgentLanguageInstruction,
    ];

    for key in keys {
        let fr_text = Language::Fr.t(key);
        let en_text = Language::En.t(key);

        assert!(!fr_text.is_empty(), "French translation missing for {:?}", key);
        assert!(!en_text.is_empty(), "English translation missing for {:?}", key);
    }
}
