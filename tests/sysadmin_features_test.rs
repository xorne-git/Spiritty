use spiritty::{
    app::{App, ChatMessage, MessageRole, ProactiveDiagnosis},
    event::AppEvent,
    system::{HostProfile, HostsStore},
};
use tempfile::tempdir;

#[tokio::test]
async fn test_bookmarks_store_crud_and_sorting() {
    let dir = tempdir().unwrap();
    let hosts_path = dir.path().join("hosts.json");

    let mut store = HostsStore::load_from_path(hosts_path);
    // Add bookmark 1
    store
        .add_bookmark(
            "root@vps-web.prod:22".to_string(),
            Some("Web Prod".to_string()),
        )
        .unwrap();
    // Add bookmark 2
    store
        .add_bookmark(
            "debian@db.internal".to_string(),
            Some("Database".to_string()),
        )
        .unwrap();

    // Cache a profile for vps-web
    store
        .upsert(HostProfile {
            target: "root@vps-web.prod:22".to_string(),
            hostname: Some("vps-web".to_string()),
            os_name: "Linux".to_string(),
            distro: "Debian GNU/Linux 12 (bookworm)".to_string(),
            kernel: "6.1.0-18-amd64".to_string(),
            user: "root".to_string(),
            package_managers: vec!["apt".to_string()],
            init_system: "systemd".to_string(),
            last_seen: "2026-08-24T00:00:00Z".to_string(),
        })
        .unwrap();

    let entries = store.list_all_entries();
    assert_eq!(entries.len(), 2);
    let web_entry = entries
        .iter()
        .find(|e| e.target == "root@vps-web.prod:22")
        .unwrap();
    assert_eq!(web_entry.alias.as_deref(), Some("Web Prod"));
    assert!(web_entry.is_favorite);
    assert!(web_entry.profile.is_some());

    // Toggle favorite
    store.toggle_favorite("debian@db.internal").unwrap();
    assert!(!store.is_favorite("debian@db.internal"));

    // Remove bookmark
    store.remove_bookmark("debian@db.internal").unwrap();
    let updated = store.list_all_entries();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].target, "root@vps-web.prod:22");
}

#[tokio::test]
async fn test_markdown_export_generation() {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    app.current_session.title = "Optimisation Nginx & PHP-FPM".to_string();
    app.system_context.current_dir = Some("~/Projets/Spiritty".to_string());
    app.system_context.git_branch = Some("main".to_string());

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Comment configurer php-fpm avec pm = ondemand ?".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Voici la configuration recommandée pour le pool www.conf :\n```ini\npm = ondemand\npm.max_children = 50\n```".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "💻 `systemctl restart php8.2-fpm`".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    let export_path = app
        .export_current_session_markdown()
        .expect("Markdown export failed");
    assert!(std::path::Path::new(&export_path).exists());

    let content = std::fs::read_to_string(&export_path).unwrap();
    assert!(content.contains("# 🧞 Rapport d'Intervention Spiritty — Optimisation Nginx & PHP-FPM"));
    assert!(content.contains("`~/Projets/Spiritty`"));
    assert!(content.contains("`main`"));
    assert!(content.contains("Comment configurer php-fpm"));
    assert!(content.contains("pm = ondemand"));
    assert!(content.contains("`systemctl restart php8.2-fpm`"));
}

#[tokio::test]
async fn test_chat_search_match_finding() {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Bonjour Spiritty, vérifie le port 80".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Le port 80 est bien ouvert par Nginx.".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Et pour le port 443 ?".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.chat_search_query = "port".to_string();
    let matches = app.find_search_matches();
    assert_eq!(matches, vec![0, 1, 2]);

    app.chat_search_query = "nginx".to_string();
    let matches_nginx = app.find_search_matches();
    assert_eq!(matches_nginx, vec![1]);

    app.chat_search_query = "inexistant".to_string();
    let no_matches = app.find_search_matches();
    assert!(no_matches.is_empty());
}

#[tokio::test]
async fn test_proactive_diagnosis_trigger() {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    app.proactive_error_diagnosis = Some(ProactiveDiagnosis {
        command: "systemctl start apache2".to_string(),
        error_message:
            "Job for apache2.service failed because the control process exited with error code."
                .to_string(),
    });

    app.trigger_proactive_diagnosis();

    // Verify diagnosis prompt was injected into messages
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, MessageRole::User);
    assert!(app.messages[0]
        .content
        .contains("La commande suivante a échoué"));
    assert!(app.messages[0].content.contains("systemctl start apache2"));
    assert!(app.messages[0]
        .content
        .contains("Job for apache2.service failed"));

    assert_eq!(app.messages[1].role, MessageRole::Assistant);
    assert!(app.proactive_error_diagnosis.is_none());
}

#[tokio::test]
async fn test_proactive_diagnosis_dismissal() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    app.proactive_error_diagnosis = Some(ProactiveDiagnosis {
        command: "cat /var/log/nginx/error.log".to_string(),
        error_message: "Permission denied".to_string(),
    });

    // Dismiss with Alt+X
    let alt_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
    app.handle_key(alt_x);
    assert!(app.proactive_error_diagnosis.is_none());

    // Dismiss with Esc in chat
    app.proactive_error_diagnosis = Some(ProactiveDiagnosis {
        command: "ls /root".to_string(),
        error_message: "Permission denied".to_string(),
    });
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.handle_key(esc);
    assert!(app.proactive_error_diagnosis.is_none());
}

#[tokio::test]
async fn test_proactive_diagnosis_only_for_manual_user_commands() {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // 1. When agent runs a tool and gets an error output, proactive error diagnosis is NOT triggered
    let (res_tx, _res_rx) = tokio::sync::oneshot::channel::<String>();
    let tab_id = app.active_tab().id;
    app.on_agent_pty_tool_execute("ls /forbidden".to_string(), res_tx, false);
    app.on_pty_output(
        tab_id,
        b"ls: cannot open directory '/forbidden': Permission denied\n",
    );
    assert!(
        app.proactive_error_diagnosis.is_none(),
        "Agent tools should NOT trigger proactive error toast"
    );

    // 2. When user types a manual command in shell and gets an error, proactive error diagnosis triggers
    app.active_pty_tool = None;
    app.last_user_terminal_command = Some("mkdir -p /opt/app".to_string());
    app.on_pty_output(
        tab_id,
        b"mkdir: cannot create directory '/opt/app': Permission denied\n",
    );
    assert!(
        app.proactive_error_diagnosis.is_some(),
        "Manual user command errors SHOULD trigger proactive diagnosis"
    );
    let diag = app.proactive_error_diagnosis.unwrap();
    assert_eq!(diag.command, "mkdir -p /opt/app");
    assert!(diag.error_message.contains("Permission denied"));
}

#[tokio::test]
async fn test_focus_preservation_on_chat_submit_and_execution() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::Focus;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // 1. When focus is Chat and user submits a prompt, focus MUST remain Chat
    app.focus = Focus::Chat;
    app.chat_input = "Vérifie les services actifs".to_string();
    app.cursor_pos = app.chat_input.len();

    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key(enter_key);

    assert_eq!(
        app.focus,
        Focus::Chat,
        "Focus should remain Chat after submitting prompt"
    );

    // 2. When executing a normal non-sudo command, focus remains where user set it
    app.focus = Focus::Chat;
    let (res_tx, _res_rx) = tokio::sync::oneshot::channel::<String>();
    app.on_agent_pty_tool_execute("uptime".to_string(), res_tx, false);
    assert_eq!(
        app.focus,
        Focus::Chat,
        "Focus should remain Chat during non-sudo PTY tool execution"
    );
}

#[tokio::test]
async fn test_sudo_password_detection_and_focus_switch() {
    use spiritty::app::{
        is_waiting_for_password, is_waiting_for_user_interaction, Focus, InteractionKind,
    };

    // 1. Password detection variations
    assert!(is_waiting_for_password("[sudo] password for xorne: "));
    assert!(is_waiting_for_password("[sudo] Mot de passe de user : "));
    assert!(is_waiting_for_password("Password: "));
    assert!(is_waiting_for_password(
        "Enter passphrase for key '/home/xorne/.ssh/id_ed25519': "
    ));
    assert_eq!(
        is_waiting_for_user_interaction("Enter passphrase for key: "),
        Some(InteractionKind::Password)
    );
    assert!(!is_waiting_for_password(
        "systemctl restart nginx\nSuccess\n[xorne@machine ~]$ "
    ));

    // 2. Interactive confirmation prompts ([y/n], continue connecting, etc.)
    assert_eq!(
        is_waiting_for_user_interaction("Proceed with installation? [Y/n] "),
        Some(InteractionKind::Confirmation)
    );
    assert_eq!(
        is_waiting_for_user_interaction("Voulez-vous continuer ? [O/n] "),
        Some(InteractionKind::Confirmation)
    );
    assert_eq!(
        is_waiting_for_user_interaction(
            "Are you sure you want to continue connecting (yes/no/[fingerprint])? "
        ),
        Some(InteractionKind::Confirmation)
    );
    assert_eq!(
        is_waiting_for_user_interaction("Press [Enter] to continue..."),
        Some(InteractionKind::Confirmation)
    );

    // 3. When a sudo command is executed, focus automatically switches to Terminal for password entry
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    app.focus = Focus::Chat;
    let (res_tx, _res_rx) = tokio::sync::oneshot::channel::<String>();
    app.on_agent_pty_tool_execute("sudo systemctl restart nginx".to_string(), res_tx, false);

    assert_eq!(
        app.focus,
        Focus::Terminal,
        "Focus should switch to Terminal for sudo commands"
    );

    // 4. Output with [Y/n] switches focus to Terminal
    let tab_id = app.active_tab().id;
    app.focus = Focus::Chat;
    app.on_pty_output(
        tab_id,
        b"Need to get 15.2 MB of archives.\nDo you want to continue? [Y/n] ",
    );
    assert_eq!(
        app.focus,
        Focus::Terminal,
        "Focus should switch to Terminal when an interactive [Y/n] prompt is detected"
    );
}

#[tokio::test]
async fn test_active_ssh_bookmarking_shortcut() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::ui::components::{AddHostState, BookmarksModalState};

    let dir = tempfile::tempdir().unwrap();
    let mut hosts_store = HostsStore::load_from_path(dir.path().join("hosts.json"));

    let mut state =
        BookmarksModalState::new(&hosts_store, Some("admin@192.168.1.200:2222".to_string()));
    assert_eq!(
        state.active_ssh_target.as_deref(),
        Some("admin@192.168.1.200:2222")
    );
    assert!(!state.is_target_bookmarked("admin@192.168.1.200:2222"));

    // Press 'S' to bookmark current active SSH session
    let s_key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    state.handle_key(s_key, &mut hosts_store);

    // Should prompt for alias
    match &state.add_state {
        AddHostState::EnteringAlias { target, .. } => {
            assert_eq!(target, "admin@192.168.1.200:2222");
        }
        _ => panic!("Expected AddHostState::EnteringAlias"),
    }

    // Type alias 'Server Backup' and hit Enter
    for c in "Server Backup".chars() {
        let char_key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        state.handle_key(char_key, &mut hosts_store);
    }
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    state.handle_key(enter_key, &mut hosts_store);

    assert_eq!(state.add_state, AddHostState::None);
    assert!(state.is_target_bookmarked("admin@192.168.1.200:2222"));

    let entry = state
        .entries
        .iter()
        .find(|e| e.target == "admin@192.168.1.200:2222")
        .unwrap();
    assert_eq!(entry.alias.as_deref(), Some("Server Backup"));
}

#[tokio::test]
async fn test_export_modal_and_custom_destination() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::ui::components::{ExportModalAction, ExportModalState};

    let dir = tempfile::tempdir().unwrap();
    let custom_file = dir.path().join("rapport_custom.md");
    let custom_str = custom_file.to_string_lossy().to_string();

    let mut state = ExportModalState::new("~/spiritty_rapport.md".to_string());
    assert_eq!(state.file_path_input, "~/spiritty_rapport.md");

    // Replace input with custom path
    state.file_path_input = custom_str.clone();
    state.cursor = state.file_path_input.len();

    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let action = state.handle_key(enter_key);

    assert_eq!(action, Some(ExportModalAction::Export(custom_str.clone())));

    // Verify App export to that path
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();
    app.current_session.title = "Audit Sécurité SSH".to_string();

    let exported = app.export_current_session_markdown_to(&custom_str).unwrap();
    assert_eq!(exported, custom_str);
    assert!(custom_file.exists());

    let content = std::fs::read_to_string(&custom_file).unwrap();
    assert!(content.contains("# 🧞 Rapport d'Intervention Spiritty — Audit Sécurité SSH"));
}

#[test]
fn test_cli_argument_parsing() {
    use spiritty::cli::CliOptions;

    // 1. Continue session (-c)
    let opts = CliOptions::parse_from_args(vec!["spiritty", "-c"]);
    assert!(opts.continue_last_session);

    // 2. Specific session ID (-s)
    let opts = CliOptions::parse_from_args(vec!["spiritty", "-s", "sess_20260824_0001"]);
    assert_eq!(opts.session_id.as_deref(), Some("sess_20260824_0001"));

    // 3. Positional prompt
    let opts = CliOptions::parse_from_args(vec!["spiritty", "analyse", "les", "logs", "nginx"]);
    assert_eq!(
        opts.initial_prompt.as_deref(),
        Some("analyse les logs nginx")
    );

    // 4. Combined options (-c -m model --yolo --ssh root@serv)
    let opts = CliOptions::parse_from_args(vec![
        "spiritty",
        "-c",
        "--model",
        "qwen2.5-coder:7b",
        "--yolo",
        "--ssh",
        "root@192.168.1.50:22",
    ]);
    assert!(opts.continue_last_session);
    assert_eq!(opts.model.as_deref(), Some("qwen2.5-coder:7b"));
    assert_eq!(opts.auto_approve.as_deref(), Some("yolo"));
    assert_eq!(opts.ssh_target.as_deref(), Some("root@192.168.1.50:22"));

    // 5. Help & Version
    let opts = CliOptions::parse_from_args(vec!["spiritty", "--help"]);
    assert!(opts.show_help);

    let opts = CliOptions::parse_from_args(vec!["spiritty", "-v"]);
    assert!(opts.show_version);
}

#[tokio::test]
async fn test_auto_host_scan_and_rescan_shortcut() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::system::ActiveSession;
    use spiritty::ui::components::{BookmarksModalAction, BookmarksModalState};

    let dir = tempfile::tempdir().unwrap();
    let mut hosts_store = HostsStore::load_from_path(dir.path().join("hosts.json"));

    // 1. Check flexible HostsStore::get matching
    let probe_output = r#"
SPIRITTY_PROBE_START
PRETTY_NAME="Debian GNU/Linux 12 (bookworm)"
NAME="Debian GNU/Linux"
6.1.0-21-amd64
xorne
xorne.net
/usr/bin/apt
/bin/systemctl
SPIRITTY_PROBE_END
"#;
    let profile = HostsStore::parse_probe_output("xorne@xorne.net:22", probe_output).unwrap();
    hosts_store.upsert(profile).unwrap();

    // Check that get("xorne.net") finds the profile
    let found = hosts_store
        .get("xorne.net")
        .expect("Should find profile by domain");
    assert_eq!(found.distro, "Debian GNU/Linux 12 (bookworm)");
    assert_eq!(found.user, "xorne");

    // 2. BookmarksModalState with active target and 'R' shortcut
    let mut state = BookmarksModalState::new(&hosts_store, Some("xorne.net".to_string()));
    let r_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    let action = state.handle_key(r_key, &mut hosts_store);
    assert_eq!(action, Some(BookmarksModalAction::TriggerScan));

    // 3. App auto-trigger scan and probe handling
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // Connect to unprobed host
    app.on_active_session_changed(ActiveSession::Ssh {
        target: "debian@new-server.internal".to_string(),
        user: Some("debian".to_string()),
        host: "new-server.internal".to_string(),
        port: None,
    });

    // Simulate probe output arrival via RemoteHostProbed event
    let new_probe = r#"
SPIRITTY_PROBE_START
PRETTY_NAME="Ubuntu 24.04 LTS"
NAME="Ubuntu"
6.8.0-31-generic
debian
new-server
/usr/bin/apt
/bin/systemctl
SPIRITTY_PROBE_END
"#;
    app.on_remote_host_probed(
        "debian@new-server.internal".to_string(),
        new_probe.to_string(),
    );
    assert!(app.system_context.active_remote_profile.is_some());
    let active_prof = app.system_context.active_remote_profile.unwrap();
    assert_eq!(active_prof.distro, "Ubuntu 24.04 LTS");
}

#[tokio::test]
async fn test_remote_file_tools_in_ssh_session() {
    use spiritty::agent::safety::{classify_file_edit, CommandRisk};
    use spiritty::agent::tools::{
        build_remote_read_command, build_remote_write_command, decode_remote_read_output,
        execute_remote_edit_logic,
    };

    // 1. Risk classification of remote paths
    assert_eq!(
        classify_file_edit("/etc/nginx/nginx.conf"),
        CommandRisk::Sudo
    );
    assert_eq!(classify_file_edit("/var/log/syslog"), CommandRisk::Sudo);
    assert_eq!(
        classify_file_edit("~/.ssh/authorized_keys"),
        CommandRisk::Risky
    );
    assert_eq!(
        classify_file_edit("/home/ubuntu/.bashrc"),
        CommandRisk::Risky
    );
    assert_eq!(
        classify_file_edit("/home/ubuntu/app/server.js"),
        CommandRisk::Standard
    );

    // 2. Command formatting
    let read_cmd = build_remote_read_command("/etc/caddy/Caddyfile");
    assert!(read_cmd.contains("/etc/caddy/Caddyfile"));

    let write_cmd = build_remote_write_command("/etc/caddy/Caddyfile", "ZGF0YQo=", true);
    assert!(write_cmd.contains("sudo tee \"/etc/caddy/Caddyfile\""));

    // 3. Decoding and editing
    let b64_input =
        spiritty::system::clipboard::base64_encode(b":80 {\n    respond \"Hello\"\n}\n");
    let decoded = decode_remote_read_output(&b64_input, "/etc/caddy/Caddyfile");
    assert!(decoded.contains("respond \"Hello\""));

    let edited = execute_remote_edit_logic(
        &decoded,
        "/etc/caddy/Caddyfile",
        "respond \"Hello\"",
        "reverse_proxy localhost:3000",
    )
    .unwrap();
    assert!(edited.contains("reverse_proxy localhost:3000"));
}

#[tokio::test]
async fn test_multi_tabs_lifecycle_and_shortcuts() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::Focus;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // 1. Initial state: 1 tab
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_index, 0);

    // 2. Open new tab via method
    let tab2_idx = app.new_tab().unwrap();
    assert_eq!(tab2_idx, 1);
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active_tab_index, 1);

    // 3. Open another tab via Ctrl + T key event
    let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_t);
    assert_eq!(app.tabs.len(), 3);
    assert_eq!(app.active_tab_index, 2);

    // 4. Tab navigation: next_tab and previous_tab
    app.next_tab();
    assert_eq!(app.active_tab_index, 0); // wrap around

    app.previous_tab();
    assert_eq!(app.active_tab_index, 2); // wrap back

    // 5. Direct tab selection with Alt + 2
    app.focus = Focus::Terminal;
    let alt_2 = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT);
    app.handle_key(alt_2);
    assert_eq!(app.active_tab_index, 1);

    // 6. Background tab output marks unread_activity
    let tab0_id = app.tabs[0].id;
    let tab1_id = app.tabs[1].id;
    assert!(!app.tabs[0].unread_activity);
    app.on_pty_output(tab0_id, b"background job finished\n");
    assert!(app.tabs[0].unread_activity);

    // Active tab output does NOT mark unread
    assert!(!app.tabs[1].unread_activity);
    app.on_pty_output(tab1_id, b"active tab prompt\n");
    assert!(!app.tabs[1].unread_activity);

    // Switching to tab 0 clears unread flag
    app.select_tab(0);
    assert_eq!(app.active_tab_index, 0);
    assert!(!app.tabs[0].unread_activity);

    // 7. Closing tab via Ctrl + Shift + W
    app.focus = Focus::Terminal;
    let ctrl_shift_w = KeyEvent::new(
        KeyCode::Char('w'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    app.handle_key(ctrl_shift_w);
    assert_eq!(app.tabs.len(), 2);

    // 8. Close another tab
    app.close_active_tab();
    assert_eq!(app.tabs.len(), 1);
    assert!(!app.should_quit);

    // 9. Closing the last remaining tab via close_active_tab does NOT quit
    app.close_active_tab();
    assert_eq!(app.tabs.len(), 1);
    assert!(!app.should_quit);

    // 10. Actual shell process exit on the last tab exits the app
    let last_tab_id = app.tabs[0].id;
    app.on_pty_exit(last_tab_id);
    assert!(app.should_quit);
}

#[tokio::test]
async fn test_tab_renaming_and_session_persistence() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::ModalState;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // 1. Initial tab display title is default
    assert_eq!(app.active_tab().display_title(), "💻 local");

    // 2. Alt + R opens the tab rename modal
    let alt_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT);
    app.handle_key(alt_r);
    assert!(matches!(app.modal, ModalState::RenameTab(_)));

    // 3. Type new title "production-db" and press Enter
    for c in "production-db".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, ModalState::None));
    assert_eq!(
        app.active_tab().custom_title.as_deref(),
        Some("production-db")
    );
    assert_eq!(app.active_tab().display_title(), "production-db");

    // 4. Test session persistence: save session with renamed tab
    app.messages.push(spiritty::app::ChatMessage {
        role: spiritty::app::MessageRole::User,
        content: "Test persistence".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.save_current_session();

    assert_eq!(app.current_session.tabs.len(), 1);
    assert_eq!(
        app.current_session.tabs[0].custom_title.as_deref(),
        Some("production-db")
    );

    // 5. Reset custom title with empty input
    app.handle_key(alt_r);
    // Backspace everything
    for _ in 0..20 {
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.active_tab().custom_title, None);
    assert_eq!(app.active_tab().display_title(), "💻 local");

    // Clean up test session
    let _ = spiritty::session::SessionStorage::delete(&app.current_session.id);
}
