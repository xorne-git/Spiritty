use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spiritty::agent::skills::{builtin_skills, SkillsManager};
use spiritty::app::{App, ChatMessage, MessageRole};
use spiritty::config::{Config, SkillSelectionMode};
use spiritty::ui::components::ModalState;

fn create_test_app() -> App {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    App::new(event_tx, 24, 80).expect("create test app")
}

#[test]
fn test_builtin_skills_catalog() {
    let skills = builtin_skills();
    assert_eq!(skills.len(), 5);

    let ids: Vec<&str> = skills.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"shell-guru"));
    assert!(ids.contains(&"git-cli"));
    assert!(ids.contains(&"process-triage"));
    assert!(ids.contains(&"text-processing"));
    assert!(ids.contains(&"network-tools"));

    for s in &skills {
        assert!(!s.name.is_empty());
        assert!(!s.description.is_empty());
        assert!(!s.triggers.is_empty());
        assert!(!s.content.is_empty());
    }
}

#[tokio::test]
async fn test_app_ctrl_s_toggles_skills_modal() {
    let mut app = create_test_app();
    assert!(matches!(app.modal, ModalState::None));

    // Press Ctrl+S
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::Skills(_)));

    // Press Ctrl+S again to close
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::None));
}

#[tokio::test]
async fn test_ctrl_s_in_config_modal_saves_and_does_not_open_skills() {
    let mut app = create_test_app();
    assert!(matches!(app.modal, ModalState::None));

    // Open Config modal with Ctrl+P
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::Config(_)));

    // Modify a setting inside the modal
    if let ModalState::Config(ref mut config_state) = app.modal {
        config_state.voice_enabled = true;
    }

    // Press Ctrl+S
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    // Must save and close, NOT switch to Skills modal
    assert!(matches!(app.modal, ModalState::None));
    assert!(app.config.voice.enabled);
}

#[tokio::test]
async fn test_ctrl_p_toggles_config_modal() {
    let mut app = create_test_app();
    assert!(matches!(app.modal, ModalState::None));

    // Open Config modal with Ctrl+P
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::Config(_)));

    // Press Ctrl+P again to close
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, ModalState::None));
}

#[tokio::test]
async fn test_slash_skills_commands_open_modal() {
    let mut app = create_test_app();
    assert!(matches!(app.modal, ModalState::None));

    // Type /skills
    app.chat_input = "/skills".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Skills(_)));

    // Close with Esc
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));

    // Type /skill
    app.chat_input = "/skill".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::Skills(_)));
}

#[tokio::test]
async fn test_skills_modal_mode_and_state_toggle() {
    let mut app = create_test_app();
    assert_eq!(app.config.skills.mode, SkillSelectionMode::Auto);

    // Open skills modal
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    // Toggle mode with Tab
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.config.skills.mode, SkillSelectionMode::Manual);

    // Toggle mode back with 'm'
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
    assert_eq!(app.config.skills.mode, SkillSelectionMode::Auto);

    // Toggle skill state with Space: Auto -> Disabled
    let first_id = {
        if let ModalState::Skills(ref st) = app.modal {
            st.skills[0].id.clone()
        } else {
            panic!("Expected Skills modal");
        }
    };
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(app.config.skills.disabled.contains(&first_id));

    // Space again: Disabled -> Enabled
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(app.config.skills.enabled.contains(&first_id));

    // Space again: Enabled -> Auto
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(!app.config.skills.disabled.contains(&first_id));
    assert!(!app.config.skills.enabled.contains(&first_id));
}

#[test]
fn test_skills_prompt_injection_auto_vs_manual() {
    let manager = SkillsManager::new();
    let config = Config::default();

    // Query mentioning git
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: "Je veux faire un git rebase interactif sur main".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    }];

    let prompt_section = manager.build_prompt_section(&config.skills, &messages);
    assert!(prompt_section.contains("git-cli"));
    assert!(prompt_section.contains("reflog"));
    assert!(prompt_section.contains("AVAILABLE STANDBY SKILLS"));

    // Query without special keywords
    let generic_messages = vec![ChatMessage {
        role: MessageRole::User,
        content: "Bonjour, comment ça va ?".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    }];
    let generic_section = manager.build_prompt_section(&config.skills, &generic_messages);
    assert!(!generic_section.contains("Follow these domain-specific directives"));
    assert!(generic_section.contains("AVAILABLE STANDBY SKILLS"));

    // In Manual mode with no enabled skills: nothing injected
    let mut manual_config = config.skills.clone();
    manual_config.mode = SkillSelectionMode::Manual;
    let manual_empty = manager.build_prompt_section(&manual_config, &messages);
    assert!(manual_empty.is_empty());

    // In Manual mode with git-cli explicitly enabled
    manual_config.enabled.push("git-cli".to_string());
    let manual_git = manager.build_prompt_section(&manual_config, &messages);
    assert!(manual_git.contains("git-cli"));
    assert!(!manual_git.contains("AVAILABLE STANDBY SKILLS"));
}
