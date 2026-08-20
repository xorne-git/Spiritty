pub mod prompt;
pub mod providers;
pub mod safety;
pub mod tools;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{ChatMessage, MessageRole},
    config::Config,
    event::AppEvent,
};
use prompt::build_system_prompt;
use providers::{create_provider, LlmProvider};
use safety::should_auto_approve_command;
use tools::{execute_web_search, parse_tool_call, ToolInvocation};

#[derive(Clone)]
pub struct AgentEngine {
    config: Config,
    provider: Arc<Box<dyn LlmProvider>>,
    pub is_generating: bool,
}

impl AgentEngine {
    pub fn new(config: Config) -> Self {
        let provider = Arc::new(create_provider(&config));
        Self {
            config,
            provider,
            is_generating: false,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn reload_config(&mut self, config: Config) {
        self.provider = Arc::new(create_provider(&config));
        self.config = config;
    }

    pub fn send_prompt(
        &mut self,
        messages: Vec<ChatMessage>,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        self.is_generating = true;
        let provider = Arc::clone(&self.provider);
        let config = self.config.clone();
        let lang = config.get_language();
        let auto_approve = config.auto_approve;
        let sys_ctx = crate::system::SystemContext::detect();
        let system_prompt = build_system_prompt(lang, &sys_ctx, &config);

        tokio::spawn(async move {
            // Format history cleanly for the LLM without UI control pills but preserving command blocks
            let mut conversation: Vec<ChatMessage> = messages
                .into_iter()
                .map(|mut m| {
                    if m.role == MessageRole::Assistant {
                        let mut clean_lines = Vec::new();
                        for l in m.content.lines() {
                            let trimmed = l.trim();
                            if trimmed.starts_with("💻 `") {
                                if let Some(start) = trimmed.find("💻 `") {
                                    let after = &trimmed[start + 4..];
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
                .filter(|m| !m.content.is_empty() || m.role == MessageRole::Assistant)
                .collect();

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
                let stream_handle = tokio::spawn(async move {
                    provider_clone.stream_chat(&conv_clone, &sys_clone, turn_tx).await
                });

                let mut current_turn_text = String::new();

                while let Some(event) = turn_rx.recv().await {
                    match event {
                        AppEvent::AgentChunk(chunk) => {
                            current_turn_text.push_str(&chunk);
                            let _ = forward_event_tx.send(AppEvent::AgentChunk(chunk));
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

                let _ = stream_handle.await;

                // Check if the assistant requested a tool execution AND tool limit has not been exceeded
                if tool_steps < MAX_TOOL_STEPS {
                    if let Some(tool_call) = parse_tool_call(&current_turn_text) {
                        tool_steps += 1;

                        match tool_call {
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
        });

        Ok(())
    }
}
