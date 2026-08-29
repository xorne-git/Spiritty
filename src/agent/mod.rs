pub mod mcp;
pub mod prompt;
pub mod providers;
pub mod safety;
pub mod tools;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::{
    app::{ChatMessage, MessageRole},
    config::Config,
    event::AppEvent,
};
use mcp::McpManager;
use prompt::build_system_prompt;
use providers::{create_provider, LlmProvider};
use safety::should_auto_approve_command;
use tools::{execute_web_search, parse_tool_call, ToolInvocation};

/// Prepares the UI message history for the LLM request: strips UI control pills
/// from assistant messages (command cards, tool banners) and drops every empty
/// message. The trailing empty `Assistant` placeholder (created by the UI as the
/// streaming target right before the request) MUST be dropped: sent as-is it ends
/// the request on a model turn, which the Gemini API rejects with HTTP 400
/// ("Requests ending with a model turn are not supported"). OpenAI-compatible
/// APIs merely tolerated it.
fn prepare_conversation(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .map(|mut m| {
            if m.role == MessageRole::Assistant {
                let mut clean_lines = Vec::new();
                for l in m.content.lines() {
                    let trimmed = l.trim();
                    if trimmed.starts_with("💻 `") {
                        if let Some(start) = trimmed.find("💻 `") {
                            let after = &trimmed[start + "💻 `".len()..];
                            if let Some(end) = after.find('`') {
                                let cmd = &after[..end];
                                clean_lines.push(format!("```bash\n{}\n```", cmd));
                            }
                        }
                    } else if !trimmed.starts_with("🌐 ")
                        && !trimmed.starts_with("⚡ Exécu")
                        && !trimmed.starts_with("⚡ Execu")
                    {
                        clean_lines.push(l.to_string());
                    }
                }
                m.content = clean_lines.join("\n").trim().to_string();
            }
            m
        })
        .filter(|m| !m.content.is_empty())
        .collect()
}

#[derive(Clone)]
pub struct AgentEngine {
    config: Config,
    provider: Arc<Box<dyn LlmProvider>>,
    pub mcp_manager: Arc<McpManager>,
    pub is_generating: bool,
    cancel_token: Option<CancellationToken>,
}

impl AgentEngine {
    pub fn new(config: Config) -> Self {
        Self::new_with_event_tx(config, None)
    }

    pub fn new_with_event_tx(
        config: Config,
        event_tx: Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>,
    ) -> Self {
        let provider = Arc::new(create_provider(&config));
        let mcp_manager = Arc::new(McpManager::load_from_config(&config, event_tx));
        Self {
            config,
            provider,
            mcp_manager,
            is_generating: false,
            cancel_token: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn reload_config(
        &mut self,
        config: Config,
        event_tx: Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>,
    ) {
        self.provider = Arc::new(create_provider(&config));
        let mcp_clone = self.mcp_manager.clone();
        let cfg_clone = config.clone();
        tokio::spawn(async move {
            mcp_clone.reload(&cfg_clone, event_tx).await;
        });
        self.config = config;
    }

    pub fn stop_generation(&mut self) {
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
        self.is_generating = false;
    }

    pub fn send_prompt(
        &mut self,
        messages: Vec<ChatMessage>,
        sys_ctx: &crate::system::SystemContext,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        self.stop_generation();

        let cancel_token = CancellationToken::new();
        self.cancel_token = Some(cancel_token.clone());
        self.is_generating = true;

        let provider = Arc::clone(&self.provider);
        let mcp_manager = Arc::clone(&self.mcp_manager);
        let config = self.config.clone();
        let lang = config.get_language();
        let auto_approve = config.auto_approve;
        let mut system_prompt = build_system_prompt(lang, sys_ctx, &config);

        tokio::spawn(async move {
            let mcp_prompt_summary = mcp_manager.get_tools_summary_for_prompt().await;
            if !mcp_prompt_summary.is_empty() {
                system_prompt.push_str(&mcp_prompt_summary);
            }

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    let _ = event_tx.send(AppEvent::AgentDone);
                }
                _ = async {
            let conversation = prepare_conversation(messages);

            // v0.5.2: compaction applies ONLY to the LLM context — the live UI
            // and the persisted session keep the full history. Older turns roll
            // into a single System summary so long sessions stay within a
            // bounded context budget regardless of conversation length.
            let compacted = crate::session::compact_chat_messages(&conversation);
            let mut conversation = compacted.messages;

            let mut tool_steps = 0;
            const MAX_TOOL_STEPS: usize = 6;

            loop {
                // Channel to intercept stream chunks for this turn
                let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
                let forward_event_tx = event_tx.clone();

                let provider_clone = Arc::clone(&provider);
                let conv_clone = conversation.clone();
                let sys_clone = system_prompt.clone();

                // Run LLM stream for current turn
                let cancel = cancel_token.clone();
                let stream_handle = tokio::spawn(async move {
                    provider_clone.stream_chat(&conv_clone, &sys_clone, turn_tx, cancel).await
                });

                let mut current_turn_text = String::new();

                while let Some(event) = turn_rx.recv().await {
                    match event {
                        AppEvent::AgentChunk(chunk) => {
                            current_turn_text.push_str(&chunk);
                            let _ = forward_event_tx.send(AppEvent::AgentChunk(chunk));
                        }
                        AppEvent::AgentUsage { prompt_tokens, completion_tokens, exact_speed } => {
                            let _ = forward_event_tx.send(AppEvent::AgentUsage { prompt_tokens, completion_tokens, exact_speed });
                        }
                        AppEvent::AgentError(err) => {
                            let _ = forward_event_tx.send(AppEvent::AgentError(err));
                            return;
                        }
                        AppEvent::AgentDone => {
                            break;
                        }
                        _ => {}
                    }
                }

                let stream_res = stream_handle.await;
                match stream_res {
                    Err(join_err) => {
                        // Provider task panicked or was aborted — surface it.
                        let _ = forward_event_tx.send(AppEvent::AgentError(format!(
                            "Erreur interne du fournisseur: {}",
                            join_err
                        )));
                        return;
                    }
                    Ok(Err(err)) => {
                        if current_turn_text.is_empty() {
                            let _ = forward_event_tx.send(AppEvent::AgentError(err.to_string()));
                            return;
                        }
                    }
                    Ok(Ok(())) => {}
                }

                // Check if the assistant requested a tool execution AND tool limit has not been exceeded
                if tool_steps < MAX_TOOL_STEPS {
                    if let Some(tool_call) = parse_tool_call(&current_turn_text) {
                        tool_steps += 1;

                        match tool_call {
                            ToolInvocation::McpCall { server, tool, arguments } => {
                                let label = format!("🔌 MCP {}:{}", server, tool);
                                let _ = forward_event_tx.send(AppEvent::AgentToolStart(label.clone()));

                                let mcp_result = mcp_manager
                                    .execute_mcp_tool(&server, &tool, arguments)
                                    .await
                                    .unwrap_or_else(|err| format!("Erreur MCP : {}", err));

                                let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                    command: label,
                                    output: mcp_result.clone(),
                                });

                                conversation.push(ChatMessage {
                                    role: MessageRole::Assistant,
                                    content: current_turn_text,
                                    command_proposal: None,
                                });

                                let tool_msg = format!(
                                    "[RÉSULTAT DE L'OUTIL MCP '{}:{}']:\n{}\n[FIN DU RÉSULTAT MCP - Formulez maintenant votre diagnostic ou poursuivez votre analyse]",
                                    server, tool, mcp_result
                                );

                                conversation.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: tool_msg,
                                    command_proposal: None,
                                });

                                let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                continue;
                            }
                            ToolInvocation::WebSearch(query) => {
                                let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("🌐 Recherche web : {}", query)));

                                let search_result = execute_web_search(&query, &config.web_search).await;

                                let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                    command: format!("🌐 Recherche web : {}", query),
                                    output: search_result.clone(),
                                });

                                // Append assistant turn & search result to history
                                conversation.push(ChatMessage {
                                    role: MessageRole::Assistant,
                                    content: current_turn_text,
                                    command_proposal: None,
                                });

                                let tool_msg = format!(
                                    "[RÉSULTATS DE LA RECHERCHE WEB POUR '{}']:\n{}\n[FIN DES RÉSULTATS WEB - Utilisez ces informations pour formuler votre diagnostic ou poursuivre l'analyse]",
                                    query, search_result
                                );

                                conversation.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: tool_msg,
                                    command_proposal: None,
                                });

                                let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                continue;
                            }
                            ToolInvocation::RunCommand(cmd) => {
                                let approved = if should_auto_approve_command(&cmd, auto_approve) {
                                    true
                                } else {
                                    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel::<bool>();
                                    let _ = forward_event_tx.send(AppEvent::AgentToolRequest {
                                        command: cmd.clone(),
                                        approval_tx,
                                    });
                                    approval_rx.await.unwrap_or(false)
                                };

                                if approved {
                                    let _ = forward_event_tx.send(AppEvent::AgentToolStart(cmd.clone()));

                                    // Execute the tool command directly in the live PTY terminal
                                    let (pty_result_tx, pty_result_rx) = tokio::sync::oneshot::channel::<String>();
                                    let _ = forward_event_tx.send(AppEvent::AgentPtyToolExecute {
                                        command: cmd.clone(),
                                        result_tx: pty_result_tx,
                                    });
                                    let tool_output = pty_result_rx
                                        .await
                                        .unwrap_or_else(|_| "(Erreur lors de l'exécution dans le PTY)".to_string());

                                    let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                        command: cmd.clone(),
                                        output: tool_output.clone(),
                                    });

                                    // Append assistant turn & tool result to history
                                    conversation.push(ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: current_turn_text,
                                        command_proposal: None,
                                    });

                                    let tool_msg = if tool_steps >= MAX_TOOL_STEPS {
                                        format!(
                                            "[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '{}']:\n{}\n[FIN DU RÉSULTAT - Formulez maintenant impérativement votre diagnostic final pour l'utilisateur sans nouvel outil]",
                                            cmd, tool_output
                                        )
                                    } else {
                                        format!(
                                            "[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '{}']:\n{}\n[FIN DU RÉSULTAT - Vous avez les données réelles du système. Formulez votre diagnostic direct ou lancez une dernière inspection si nécessaire]",
                                            cmd, tool_output
                                        )
                                    };

                                    conversation.push(ChatMessage {
                                        role: MessageRole::User,
                                        content: tool_msg,
                                        command_proposal: None,
                                    });

                                    // Signal UI to create a fresh turn for the next assistant response
                                    let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                    continue;
                                } else {
                                    // User declined execution (Esc): stop model generation immediately and let user type next prompt
                                    let _ = forward_event_tx.send(AppEvent::AgentDone);
                                    break;
                                }
                            }
                        }
                    }
                }

                // No tool call requested or final diagnostic reached: finish turn!
                let _ = forward_event_tx.send(AppEvent::AgentDone);
                break;
            }
                } => {}
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_conversation;
    use crate::app::{ChatMessage, MessageRole};

    fn msg(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
            command_proposal: None,
        }
    }

    #[test]
    fn drops_trailing_empty_assistant_placeholder() {
        // Regression: the UI pushes an empty Assistant message as streaming target
        // before every request. Sent as-is it ends the request on a model turn,
        // which the Gemini API rejects (HTTP 400).
        let conv = prepare_conversation(vec![
            msg(MessageRole::User, "quel noyau ?"),
            msg(MessageRole::Assistant, ""),
        ]);
        assert_eq!(conv.len(), 1);
        assert_eq!(conv.last().unwrap().role, MessageRole::User);
    }

    #[test]
    fn drops_empty_user_and_system_messages_but_keeps_content() {
        let conv = prepare_conversation(vec![
            msg(MessageRole::User, ""),
            msg(MessageRole::System, "résumé du contexte"),
            msg(MessageRole::User, "hello"),
        ]);
        assert_eq!(conv.len(), 2);
        assert_eq!(conv[0].content, "résumé du contexte");
        assert_eq!(conv[1].content, "hello");
    }

    #[test]
    fn converts_command_pills_to_bash_blocks_and_strips_tool_banners() {
        let assistant = "Voici :\n💻 `uname -r`\n🌐 Recherche web : test\nRésultat final.";
        let conv = prepare_conversation(vec![msg(MessageRole::Assistant, assistant)]);
        assert!(conv[0].content.contains("```bash\nuname -r\n```"));
        assert!(!conv[0].content.contains("🌐 "));
        assert!(conv[0].content.contains("Résultat final."));
    }
}
