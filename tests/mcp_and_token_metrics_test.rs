use spiritty::{
    agent::mcp::{
        manager::{McpManager, McpServerStatus, McpStatus},
        McpContent, McpToolCallResult, McpToolDefinition,
    },
    agent::tools::{parse_tool_call, ToolInvocation},
    config::{Config, McpServerConfig},
    session::Session,
    ui::components::{AddMcpState, McpModalAction, McpModalState},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[test]
fn test_mcp_tool_call_parsing() {
    let raw_text = r#"Je vais lire le fichier demandé avec l'outil MCP.

```tool:mcp:filesystem:read_file
{"path": "/etc/hosts"}
```
"#;

    let tool_call = parse_tool_call(raw_text).expect("Should parse MCP tool call");
    match tool_call {
        ToolInvocation::McpCall { server, tool, arguments } => {
            assert_eq!(server, "filesystem");
            assert_eq!(tool, "read_file");
            assert_eq!(arguments["path"], "/etc/hosts");
        }
        _ => panic!("Expected ToolInvocation::McpCall, got {:?}", tool_call),
    }
}

#[test]
fn test_mcp_tool_call_result_formatting() {
    let res = McpToolCallResult {
        content: vec![
            McpContent {
                content_type: "text".to_string(),
                text: Some("127.0.0.1 localhost".to_string()),
                data: None,
            },
        ],
        is_error: false,
    };

    assert_eq!(res.to_plain_text(), "127.0.0.1 localhost");

    let err_res = McpToolCallResult {
        content: vec![],
        is_error: true,
    };
    assert_eq!(err_res.to_plain_text(), "(Erreur MCP sans description)");
}

#[tokio::test]
async fn test_mcp_manager_tools_summary_formatting() {
    let manager = McpManager::new();
    let mut config = Config::default();

    config.mcp_servers.insert(
        "git".to_string(),
        McpServerConfig {
            command: "uvx".to_string(),
            args: vec!["mcp-server-git".to_string()],
            env: HashMap::new(),
            enabled: true,
        },
    );

    // Initial summary without active connected processes is empty
    let summary = manager.get_tools_summary_for_prompt().await;
    assert!(summary.is_empty());
}

#[test]
fn test_session_cost_estimation_and_token_metrics() {
    let mut session = Session::new("DeepSeek", "deepseek-chat");
    session.prompt_tokens = 10_000;
    session.completion_tokens = 2_000;
    session.total_tokens = 12_000;

    // DeepSeek pricing: $0.14 / 1M prompt ($0.0014), $0.28 / 1M completion ($0.00056)
    let cost = session.estimated_cost_usd();
    assert!(cost > 0.0019 && cost < 0.0020, "Cost was {}", cost);

    // Ollama / LM Studio (local) should cost $0.00
    let mut local_session = Session::new("Ollama", "qwen2.5:7b");
    local_session.prompt_tokens = 50_000;
    local_session.completion_tokens = 10_000;
    assert_eq!(local_session.estimated_cost_usd(), 0.0);
}

#[test]
fn test_mcp_modal_wizard_and_crud() {
    let statuses = vec![
        McpServerStatus {
            name: "filesystem".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), "/tmp".to_string()],
            enabled: true,
            status: McpStatus::Connected(3),
            tools: vec![
                McpToolDefinition {
                    name: "read_file".to_string(),
                    description: Some("Read file contents".to_string()),
                    input_schema: None,
                },
                McpToolDefinition {
                    name: "write_file".to_string(),
                    description: Some("Write file contents".to_string()),
                    input_schema: None,
                },
                McpToolDefinition {
                    name: "list_dir".to_string(),
                    description: Some("List directory contents".to_string()),
                    input_schema: None,
                },
            ],
        },
    ];

    let mut modal_state = McpModalState::new(statuses);
    let mut config = Config::default();
    config.mcp_servers.insert(
        "filesystem".to_string(),
        McpServerConfig {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), "/tmp".to_string()],
            env: HashMap::new(),
            enabled: true,
        },
    );

    // 1. Space toggles enabled state
    let action = modal_state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &mut config);
    assert_eq!(action, Some(McpModalAction::ServersChanged));
    assert!(!config.mcp_servers.get("filesystem").unwrap().enabled);

    // 2. Press 'a' starts add wizard
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &mut config);
    assert!(matches!(modal_state.add_state, AddMcpState::EnteringName { .. }));

    // Type server name 'docker'
    for c in "docker".chars() {
        let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), &mut config);
    }
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);
    assert!(matches!(modal_state.add_state, AddMcpState::EnteringCommand { .. }));

    // Type command 'docker-mcp'
    for c in "docker-mcp".chars() {
        let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), &mut config);
    }
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);
    assert!(matches!(modal_state.add_state, AddMcpState::EnteringArgs { .. }));

    // Type args '--all' and validate
    for c in "--all".chars() {
        let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), &mut config);
    }
    let action = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);
    assert_eq!(action, Some(McpModalAction::ServersChanged));
    assert!(config.mcp_servers.contains_key("docker"));
    assert_eq!(config.mcp_servers.get("docker").unwrap().command, "docker-mcp");
    assert_eq!(config.mcp_servers.get("docker").unwrap().args, vec!["--all"]);
}

#[test]
fn test_mcp_modal_paste() {
    let mut modal_state = McpModalState::new(Vec::new());
    let mut config = Config::default();

    // Start add wizard
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &mut config);

    // Paste name
    modal_state.handle_paste("fetch-service".to_string());
    if let AddMcpState::EnteringName { ref input, .. } = modal_state.add_state {
        assert_eq!(input, "fetch-service");
    } else {
        panic!("Expected EnteringName state");
    }
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);

    // Paste command
    modal_state.handle_paste("npx".to_string());
    if let AddMcpState::EnteringCommand { ref input, .. } = modal_state.add_state {
        assert_eq!(input, "npx");
    } else {
        panic!("Expected EnteringCommand state");
    }
    let _ = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);

    // Paste args
    modal_state.handle_paste("-y @modelcontextprotocol/server-fetch".to_string());
    if let AddMcpState::EnteringArgs { ref input, .. } = modal_state.add_state {
        assert_eq!(input, "-y @modelcontextprotocol/server-fetch");
    } else {
        panic!("Expected EnteringArgs state");
    }
    let action = modal_state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut config);
    assert_eq!(action, Some(McpModalAction::ServersChanged));

    let srv = config.mcp_servers.get("fetch-service").expect("Server must be saved in config");
    assert_eq!(srv.command, "npx");
    assert_eq!(srv.args, vec!["-y", "@modelcontextprotocol/server-fetch"]);

    // Verify modal state has the new server immediately
    assert!(modal_state.servers.iter().any(|s| s.name == "fetch-service"));

    // Verify sync_with_config retains the new server
    modal_state.sync_with_config(&config);
    assert!(modal_state.servers.iter().any(|s| s.name == "fetch-service"));
}


