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
            attachments: Vec::new(),
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Voici la commande :\n```bash\nfree -h\n```".to_string(),
            command_proposal: Some("free -h".to_string()),
            attachments: Vec::new(),
        },
    ];

    let prompt_history =
        vec!["Comment vérifier l'utilisation de la RAM avec free -h ?".to_string()];
    session.update_from_chat(
        &messages,
        &prompt_history,
        120,
        "Google Gemini",
        "gemini-2.5-flash",
    );
    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.prompt_history.len(), 1);
    assert_eq!(session.total_tokens, 120);
    assert_eq!(
        session.title,
        "Comment vérifier l'utilisation de la RAM avec"
    );
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
            attachments: Vec::new(),
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Première réponse".to_string(),
            command_proposal: None,
            attachments: Vec::new(),
        },
    ];
    let short_history = vec!["Première question".to_string()];
    session.update_from_chat(
        &short_messages,
        &short_history,
        50,
        "LM Studio",
        "qwen2.5-coder-7b",
    );
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
            attachments: Vec::new(),
        });
        long_history.push(format!("Question numéro {}", i));
        long_messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("💻 `echo {}`", i),
            command_proposal: Some(format!("echo {}", i)),
            attachments: Vec::new(),
        });
    }
    // Total 16 messages
    session.update_from_chat(
        &long_messages,
        &long_history,
        500,
        "LM Studio",
        "qwen2.5-coder-7b",
    );
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
            attachments: Vec::new(),
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Salut ! Comment puis-je vous aider ?".to_string(),
            command_proposal: None,
            attachments: Vec::new(),
        },
    ];
    session.update_from_chat(
        &msgs_greeting,
        &["salut".to_string()],
        50,
        "DeepSeek",
        "deepseek-v4-flash",
    );
    assert_eq!(session.title, "Nouvelle session");

    // Next message is a real substantive question
    let mut msgs_full = msgs_greeting;
    msgs_full.push(ChatMessage {
        role: MessageRole::User,
        content: "ma session dms+niri ne démarre plus".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    session.update_from_chat(
        &msgs_full,
        &[
            "salut".to_string(),
            "ma session dms+niri ne démarre plus".to_string(),
        ],
        100,
        "DeepSeek",
        "deepseek-v4-flash",
    );
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
        attachments: Vec::new(),
    });
    session.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Espace disque vérifié.".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
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
        attachments: Vec::new(),
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

    let _ = SessionStorage::delete(&initial_id);
    let _ = SessionStorage::delete(&app.current_session.id);
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
        attachments: Vec::new(),
    });
    saved_session.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Réponse sauvegardée".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    let _ = SessionStorage::save(&saved_session);

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    let initial_app_id = app.current_session.id.clone();

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
    assert_eq!(
        app.config.default_provider,
        spiritty::config::ProviderType::DeepSeek
    );
    assert_eq!(
        app.config.providers.get("deepseek").unwrap().model,
        "deepseek-v4-flash"
    );

    let _ = SessionStorage::delete("test_load_123");
    let _ = SessionStorage::delete(&initial_app_id);
}

#[tokio::test]
async fn test_session_auto_approve_persistence_and_restoration() {
    use spiritty::app::App;
    use spiritty::config::AutoApproveLevel;
    use spiritty::session::{Session, SessionStorage};

    let session_id = "test_auto_approve_persist_999";
    let mut saved_session = Session::new("DeepSeek", "deepseek-v4-pro");
    saved_session.id = session_id.to_string();
    saved_session.title = "Session avec mode YOLO".to_string();
    saved_session.auto_approve = Some(AutoApproveLevel::Yolo);
    saved_session.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "apt update".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    let save_res = SessionStorage::save(&saved_session);
    assert!(save_res.is_ok());

    // Verify storage roundtrip contains auto_approve
    let loaded = SessionStorage::load(session_id).expect("load saved session");
    assert_eq!(loaded.auto_approve, Some(AutoApproveLevel::Yolo));

    // Verify list_sessions contains auto_approve
    let list = SessionStorage::list_sessions().expect("list sessions");
    let header = list.iter().find(|h| h.id == session_id).expect("found in list");
    assert_eq!(header.auto_approve, Some(AutoApproveLevel::Yolo));

    // Create App instance and test load_session
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 50, 100).expect("create app");
    let init_id = app.current_session.id.clone();
    app.config.auto_approve = AutoApproveLevel::Safe;

    app.load_session(session_id);
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Yolo);
    assert_eq!(app.current_session.auto_approve, Some(AutoApproveLevel::Yolo));

    // Cycle auto approve (Yolo -> Off -> Safe -> Sudo -> Yolo)
    let next = app.cycle_auto_approve();
    assert_eq!(next, AutoApproveLevel::Off);
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Off);
    assert_eq!(app.current_session.auto_approve, Some(AutoApproveLevel::Off));

    // Cleanup
    let _ = SessionStorage::delete(session_id);
    let _ = SessionStorage::delete(&init_id);
    let _ = SessionStorage::delete(&app.current_session.id);
}

#[tokio::test]
async fn test_cli_auto_approve_override_precedence_on_session_continue() {
    use spiritty::app::App;
    use spiritty::config::AutoApproveLevel;
    use spiritty::session::{Session, SessionStorage};

    let session_id = "test_cli_override_yolo_session";
    let mut saved_session = Session::new("OpenAI", "gpt-5.6-sol");
    saved_session.id = session_id.to_string();
    saved_session.title = "Session Yolo".to_string();
    saved_session.auto_approve = Some(AutoApproveLevel::Yolo);
    saved_session.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "uptime".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    let _ = SessionStorage::save(&saved_session);

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 50, 100).expect("create app");
    let init_id = app.current_session.id.clone();

    // 1. When no CLI override is given, loaded session keeps its Yolo level
    app.load_session(session_id);
    app.apply_cli_overrides(None, None, None, None);
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Yolo);

    // 2. When CLI specifies --safe (or --auto-approve safe), CLI overrides session
    app.apply_cli_overrides(None, None, Some("safe".to_string()), None);
    assert_eq!(app.config.auto_approve, AutoApproveLevel::Safe);

    // Cleanup
    let _ = SessionStorage::delete(session_id);
    let _ = SessionStorage::delete(&init_id);
    let _ = SessionStorage::delete(&app.current_session.id);
}

#[test]
fn test_compaction_filters_errors_and_bounds_giant_outputs() {
    use spiritty::session::compact_chat_messages;

    let mut messages = Vec::new();
    for i in 1..=5 {
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: format!("Question {}", i),
            command_proposal: None,
            attachments: Vec::new(),
        });
        messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("Réponse {}", i),
            command_proposal: None,
            attachments: Vec::new(),
        });
    }

    // Add a transient timeout error
    messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "⚠️ Erreur : Délai d'inactivité de 25s dépassé sur le flux du modèle (timeout SSE).".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    // Add a giant command output (10,000 chars)
    let giant_content = "X".repeat(10000);
    messages.push(ChatMessage {
        role: MessageRole::User,
        content: format!("[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE 'grep -rn test']: {}", giant_content),
        command_proposal: None,
        attachments: Vec::new(),
    });

    let compacted = compact_chat_messages(&messages);
    // Transient error was filtered out
    assert!(!compacted.messages.iter().any(|m| m.content.contains("⚠️ Erreur")));

    // Giant message was bounded (should not be 10,000 chars)
    let last_msg = compacted.messages.last().unwrap();
    assert!(last_msg.content.len() < 7000);
    assert!(last_msg.content.contains("sortie tronquée pour le contexte LLM"));
}

#[tokio::test]
async fn test_alt_1_on_resumed_session() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::App;

    let (event_tx, mut _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 24, 80).expect("create app");

    app.load_session("sess_20260830_172917_888963_001");
    println!("Loaded messages count: {}", app.messages.len());
    println!("Proposals: {:?}", app.all_command_proposals());

    // Send Alt + 1
    let alt_1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT);
    app.handle_key(alt_1);

    println!("After Alt+1: should_quit={}", app.should_quit);
    println!("Active pty tool: {:?}", app.active_pty_tool.is_some());
    assert!(!app.should_quit);
}


