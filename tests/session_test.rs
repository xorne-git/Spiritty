use spiritty::{
    app::{ChatMessage, MessageRole},
    session::{Session, SessionStorage},
};

#[test]
fn test_session_creation_and_title_generation() {
    let mut session = Session::new("Google Gemini", "gemini-2.5-flash");
    assert_eq!(session.title, "Nouvelle session");
    assert_eq!(session.provider, "Google Gemini");
    assert_eq!(session.model, "gemini-2.5-flash");
    assert!(session.messages.is_empty());

    let messages = vec![
        ChatMessage {
            role: MessageRole::User,
            content: "Comment vérifier l'utilisation de la RAM avec free -h ?".to_string(),
            command_proposal: None,
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Voici la commande :\n```bash\nfree -h\n```".to_string(),
            command_proposal: Some("free -h".to_string()),
        },
    ];

    let prompt_history = vec!["Comment vérifier l'utilisation de la RAM avec free -h ?".to_string()];
    session.update_from_chat(&messages, &prompt_history, 120, "Google Gemini", "gemini-2.5-flash");
    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.prompt_history.len(), 1);
    assert_eq!(session.total_tokens, 120);
    assert_eq!(session.title, "Comment vérifier l'utilisation de la RAM avec");
}

#[test]
fn test_session_compaction() {
    let mut session = Session::new("LM Studio", "qwen2.5-coder-7b");
    
    // With 4 or fewer messages, compact() does not shrink history
    let short_messages = vec![
        ChatMessage {
            role: MessageRole::User,
            content: "Première question".to_string(),
            command_proposal: None,
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Première réponse".to_string(),
            command_proposal: None,
        },
    ];
    let short_history = vec!["Première question".to_string()];
    session.update_from_chat(&short_messages, &short_history, 50, "LM Studio", "qwen2.5-coder-7b");
    assert!(!session.compact());
    assert_eq!(session.messages.len(), 2);

    // With more than 4 messages, compact() summarizes older turns and keeps the last 4 messages
    let mut long_messages = Vec::new();
    let mut long_history = Vec::new();
    for i in 1..=8 {
        long_messages.push(ChatMessage {
            role: MessageRole::User,
            content: format!("Question numéro {}", i),
            command_proposal: None,
        });
        long_history.push(format!("Question numéro {}", i));
        long_messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("💻 `echo {}`", i),
            command_proposal: Some(format!("echo {}", i)),
        });
    }
    // Total 16 messages
    session.update_from_chat(&long_messages, &long_history, 500, "LM Studio", "qwen2.5-coder-7b");
    assert_eq!(session.messages.len(), 16);
    assert_eq!(session.prompt_history.len(), 8);

    let compacted = session.compact();
    assert!(compacted);
    assert!(session.compacted_summary.is_some());
    // Retains 1 summary message + 8 recent messages = 9 messages
    assert_eq!(session.messages.len(), 9);
    assert_eq!(session.messages[0].role, MessageRole::System);
    assert!(session.messages[0].content.contains("Contexte précédent"));
}

#[test]
fn test_session_greeting_refinement() {
    let mut session = Session::new("DeepSeek", "deepseek-v4-flash");
    
    // First message is a simple greeting
    let msgs_greeting = vec![
        ChatMessage {
            role: MessageRole::User,
            content: "salut".to_string(),
            command_proposal: None,
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Salut ! Comment puis-je vous aider ?".to_string(),
            command_proposal: None,
        },
    ];
    session.update_from_chat(&msgs_greeting, &["salut".to_string()], 50, "DeepSeek", "deepseek-v4-flash");
    assert_eq!(session.title, "Nouvelle session");

    // Next message is a real substantive question
    let mut msgs_full = msgs_greeting;
    msgs_full.push(ChatMessage {
        role: MessageRole::User,
        content: "ma session dms+niri ne démarre plus".to_string(),
        command_proposal: None,
    });
    session.update_from_chat(&msgs_full, &["salut".to_string(), "ma session dms+niri ne démarre plus".to_string()], 100, "DeepSeek", "deepseek-v4-flash");
    assert_eq!(session.title, "ma session dms+niri ne démarre plus");
}

#[test]
fn test_session_storage_roundtrip() {
    let mut session = Session::new("Anthropic", "claude-3-5-sonnet");
    session.id = "test_session_roundtrip_123".to_string();
    session.title = "Test de persistance de session".to_string();
    session.prompt_history = vec!["df -h /".to_string()];
    session.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "df -h /".to_string(),
        command_proposal: None,
    });
    session.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Espace disque vérifié.".to_string(),
        command_proposal: None,
    });

    // 1. Save
    let save_res = SessionStorage::save(&session);
    assert!(save_res.is_ok());

    // 2. Load
    let load_res = SessionStorage::load(&session.id);
    assert!(load_res.is_ok());
    let loaded = load_res.unwrap();
    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.title, session.title);
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.prompt_history.len(), 1);
    assert_eq!(loaded.prompt_history[0], "df -h /");

    // 3. List
    let list_res = SessionStorage::list_sessions();
    assert!(list_res.is_ok());
    let list = list_res.unwrap();
    assert!(list.iter().any(|s| s.id == session.id));

    // 4. Delete
    let del_res = SessionStorage::delete(&session.id);
    assert!(del_res.is_ok());

    // 5. Verify deleted
    let verify_del = SessionStorage::load(&session.id);
    assert!(verify_del.is_err());
}

#[tokio::test]
async fn test_app_new_session_shortcut() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::App;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Première question".to_string(),
        command_proposal: None,
    });
    app.chat_input = "Draft en cours".to_string();

    let initial_id = app.current_session.id.clone();

    // Trigger Ctrl+N keypress
    let key_ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
    app.handle_key(key_ctrl_n);

    // New session must have been created
    assert_ne!(app.current_session.id, initial_id);
    assert!(app.messages.is_empty());
    assert!(app.chat_input.is_empty());
    assert!(app.toast_message.is_some());
}

#[tokio::test]
async fn test_app_load_session_shortcut() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::App;
    use spiritty::session::{Session, SessionStorage};

    let mut saved_session = Session::new("DeepSeek", "deepseek-v4-flash");
    saved_session.id = "test_load_123".to_string();
    saved_session.title = "Ma session de test".to_string();
    saved_session.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Question sauvegardée".to_string(),
        command_proposal: None,
    });
    saved_session.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Réponse sauvegardée".to_string(),
        command_proposal: None,
    });
    let _ = SessionStorage::save(&saved_session);

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    // Open sessions modal with Ctrl+H
    app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, spiritty::app::ModalState::Sessions(_)));

    // In modal, find and select "test_load_123"
    if let spiritty::app::ModalState::Sessions(ref mut state) = app.modal {
        if let Some(pos) = state.sessions.iter().position(|s| s.id == "test_load_123") {
            state.selected_index = pos;
        }
    }

    // Press Enter to load
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, spiritty::app::ModalState::None));

    assert_eq!(app.current_session.id, "test_load_123");
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].content, "Question sauvegardée");

    let _ = SessionStorage::delete("test_load_123");
}

