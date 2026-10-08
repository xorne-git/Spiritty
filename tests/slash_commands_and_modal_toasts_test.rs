use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spiritty::app::App;
use spiritty::config::AutoApproveLevel;
use spiritty::i18n::{I18nKey, Language};
use spiritty::ui::components::ModalState;

fn create_test_app() -> App {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    App::new(event_tx, 24, 80).expect("create test app")
}

#[tokio::test]
async fn test_slash_commands_open_modals() {
    let mut app = create_test_app();

    // 1. /help
    app.chat_input = "/help".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Help(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 2. /config and /settings
    app.chat_input = "/config".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Config(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    app.chat_input = "/settings".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Config(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 3. /sessions and /history
    app.chat_input = "/sessions".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Sessions(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    app.chat_input = "/history".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Sessions(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 4. /bookmarks and /hosts
    app.chat_input = "/bookmarks".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Bookmarks(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 5. /mcp
    app.chat_input = "/mcp".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Mcp(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 6. /export
    app.chat_input = "/export".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Export(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // 7. /skills
    app.chat_input = "/skills".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Skills(_)));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));
}

#[tokio::test]
async fn test_slash_commands_actions() {
    let mut app = create_test_app();

    // /layout toggles split orientation
    let initial_orientation = app.split_orientation;
    app.chat_input = "/layout".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_ne!(app.split_orientation, initial_orientation);
    app.chat_input = "/split".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.split_orientation, initial_orientation);

    // /swap toggles split swapped
    let initial_swapped = app.split_swapped;
    app.chat_input = "/swap".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.split_swapped, !initial_swapped);

    // /approve commands
    app.chat_input = "/approve yolo".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Yolo);

    app.chat_input = "/approve safe".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Safe);

    // /rename with argument directly renames tab
    app.chat_input = "/rename my-custom-tab".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.active_tab().custom_title.as_deref(),
        Some("my-custom-tab")
    );
    assert!(app.has_active_toast());

    // /search activates chat search
    assert!(!app.chat_search_active);
    app.chat_input = "/search error".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.chat_search_active);
    assert_eq!(app.chat_search_query, "error");

    // Close search with Esc
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.chat_search_active);

    // Unknown command displays toast and clears input without crash
    app.chat_input = "/notacommand".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.chat_input.is_empty());
    assert!(app.has_active_toast());
}

#[tokio::test]
async fn test_slash_commands_tab_autocompletion() {
    let mut app = create_test_app();

    // 1. Unique match: /sk -> /skills 
    app.chat_input = "/sk".to_string();
    app.cursor_pos = 3;
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.chat_input, "/skills ");

    // 2. Multiple matches and suggestions toast: /c
    app.chat_input = "/c".to_string();
    app.cursor_pos = 2;
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    // /clear, /closetab, /config -> common prefix is /c, displays toast
    assert_eq!(app.chat_input, "/c");
    assert!(app.has_active_toast());

    // 3. Unique match: /exp -> /export 
    app.chat_input = "/exp".to_string();
    app.cursor_pos = 4;
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.chat_input, "/export ");
}

#[tokio::test]
async fn test_modal_save_toasts() {
    let mut app = create_test_app();

    // 1. Config modal save produces ToastConfigSaved
    app.open_config_modal();
    assert!(matches!(app.modal, ModalState::Config(_)));
    // Save via Ctrl+S in config modal
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::None));
    assert!(app.has_active_toast());
    assert_eq!(
        app.toast_message.as_ref().map(|(_, msg)| msg.as_str()),
        Some(Language::Fr.t(I18nKey::ToastConfigSaved))
    );

    // 2. Tab rename modal save produces ToastTabRenamed
    app.open_rename_tab_modal();
    assert!(matches!(app.modal, ModalState::RenameTab(_)));
    // Type a character and hit Enter to save
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));
    assert!(app.has_active_toast());
    assert_eq!(
        app.toast_message.as_ref().map(|(_, msg)| msg.as_str()),
        Some(Language::Fr.t(I18nKey::ToastTabRenamed))
    );

    // 3. Skills modal save produces ToastSkillsSaved when modified
    app.open_skills_modal();
    assert!(matches!(app.modal, ModalState::Skills(_)));
    // Toggle a state with Space
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    // Close with Esc
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));
    assert!(app.has_active_toast());
    assert_eq!(
        app.toast_message.as_ref().map(|(_, msg)| msg.as_str()),
        Some(Language::Fr.t(I18nKey::ToastSkillsSaved))
    );
}

#[tokio::test]
async fn test_help_modal_renders_slash_commands() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::ui::components::{HelpModal, HelpModalState};

    let state = HelpModalState::new();
    let mut term = Terminal::new(TestBackend::new(100, 70)).unwrap();
    term.draw(|f| {
        HelpModal::render_modal(f.area(), f.buffer_mut(), Language::Fr, &state);
    })
    .unwrap();

    let buffer = term.backend().buffer();
    let mut full_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            full_text.push_str(buffer[(x, y)].symbol());
        }
        full_text.push('\n');
    }

    assert!(
        full_text.contains("Commandes Slash"),
        "Help modal should render slash commands section header"
    );
    assert!(
        full_text.contains("/config"),
        "Help modal should render /config slash command"
    );
    assert!(
        full_text.contains("/skills"),
        "Help modal should render /skills slash command"
    );
}

#[tokio::test]
async fn test_double_ctrl_c_quits_app() {
    let mut app = create_test_app();
    app.focus = spiritty::app::Focus::Chat;
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_none());

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

    // 1st press: arms quit, displays toast
    app.handle_key(ctrl_c);
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_some());
    assert!(app.has_active_toast());
    assert_eq!(
        app.toast_message.as_ref().map(|(_, msg)| msg.as_str()),
        Some(Language::Fr.t(I18nKey::ToastPressCtrlCAgainToQuit))
    );

    // 2nd press within window: quits
    app.handle_key(ctrl_c);
    assert!(app.should_quit);
}

#[tokio::test]
async fn test_single_ctrl_c_clears_prompt_input_and_arms() {
    let mut app = create_test_app();
    app.focus = spiritty::app::Focus::Chat;
    app.chat_input = "systemctl restart nginx".to_string();
    app.cursor_pos = app.chat_input.len();

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_c);

    assert_eq!(app.chat_input, "");
    assert_eq!(app.cursor_pos, 0);
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_some());
}

#[tokio::test]
async fn test_ctrl_c_resets_on_other_keystroke() {
    let mut app = create_test_app();
    app.focus = spiritty::app::Focus::Chat;

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_c);
    assert!(app.last_ctrl_c.is_some());

    // Type another key (e.g. 'a')
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(app.last_ctrl_c.is_none());
    assert!(!app.should_quit);

    // Next Ctrl+C is treated as 1st press again
    app.handle_key(ctrl_c);
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_some());
}

#[tokio::test]
async fn test_ctrl_c_in_terminal_focus_does_not_quit_or_arm() {
    let mut app = create_test_app();
    app.focus = spiritty::app::Focus::Terminal;

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_c);
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_none());

    app.handle_key(ctrl_c);
    assert!(!app.should_quit);
    assert!(app.last_ctrl_c.is_none());
}

