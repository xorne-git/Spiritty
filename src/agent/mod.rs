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
use safety::{classify_file_edit, should_auto_approve_command, CommandRisk};
use tools::{
    build_remote_read_command, build_remote_write_command, decode_remote_read_output,
    execute_edit_file, execute_read_file, execute_remote_edit_logic, execute_web_search,
    execute_write_file, parse_tool_call, ToolInvocation,
};

/// Auto-approves a file-edit operation according to its pre-computed risk, without
/// re-classifying a path as a shell command (commands reuse `should_auto_approve_command`).
fn should_auto_approve_risk(risk: CommandRisk, level: crate::config::AutoApproveLevel) -> bool {
    match level {
        crate::config::AutoApproveLevel::Off => false,
        crate::config::AutoApproveLevel::Safe => risk == CommandRisk::Safe,
        crate::config::AutoApproveLevel::Sudo => matches!(
            risk,
            CommandRisk::Safe | CommandRisk::Standard | CommandRisk::Sudo
        ),
        crate::config::AutoApproveLevel::Yolo => true,
    }
}

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
        .filter(|m| {
            let trimmed = m.content.trim();
            !trimmed.starts_with("⚠️ Erreur") && !trimmed.starts_with("⚠️ Error")
        })
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
        // Keep a turn that carries ONLY a vision attachment (empty text but an image):
        // such a message must still reach the provider so the screenshot is forwarded.
        .filter(|m| !m.content.is_empty() || !m.attachments.is_empty())
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

        // File-editing tools act on localhost filesystem only. In a live SSH / container
        // session the model must NOT silently edit a local file that isn't the one the user
        // is looking at — it should fall back to shell commands. This flag is threaded into
        // the tool loop so read/write/edit are refused with an explicit message.
        let is_remote_session = matches!(
            sys_ctx.active_session,
            crate::system::ActiveSession::Ssh { .. }
                | crate::system::ActiveSession::Container { .. }
        );
        let is_remote_session =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(is_remote_session));

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
                                    attachments: Vec::new(),
                                });

                                let tool_msg = format!(
                                    "[RÉSULTAT DE L'OUTIL MCP '{}:{}']:\n{}\n[FIN DU RÉSULTAT MCP - Formulez maintenant votre diagnostic ou poursuivez votre analyse]",
                                    server, tool, mcp_result
                                );

                                conversation.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: tool_msg,
                                    command_proposal: None,
                                    attachments: Vec::new(),
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
                                    attachments: Vec::new(),
                                });

                                let tool_msg = format!(
                                    "[RÉSULTATS DE LA RECHERCHE WEB POUR '{}']:\n{}\n[FIN DES RÉSULTATS WEB - Utilisez ces informations pour formuler votre diagnostic ou poursuivre l'analyse]",
                                    query, search_result
                                );

                                conversation.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: tool_msg,
                                    command_proposal: None,
                                    attachments: Vec::new(),
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
                                        attachments: Vec::new(),
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
                                        attachments: Vec::new(),
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
                            ToolInvocation::ReadFile(path) => {
                                let (read_result, display_cmd) = if is_remote_session.load(std::sync::atomic::Ordering::SeqCst) {
                                    let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("📖 Lecture distante de {}", path)));
                                    let remote_cmd = build_remote_read_command(&path);
                                    let (pty_result_tx, pty_result_rx) = tokio::sync::oneshot::channel::<String>();
                                    let _ = forward_event_tx.send(AppEvent::AgentPtyToolExecute {
                                        command: remote_cmd,
                                        result_tx: pty_result_tx,
                                    });
                                    let raw_output = pty_result_rx
                                        .await
                                        .unwrap_or_else(|_| "(Erreur lors de l'exécution dans le PTY)".to_string());
                                    (decode_remote_read_output(&raw_output, &path), format!("📖 (distant) {}", path))
                                } else {
                                    let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("📖 Lecture de {}", path)));
                                    (execute_read_file(&path).await, format!("📖 {}", path))
                                };

                                let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                    command: display_cmd,
                                    output: read_result.clone(),
                                });

                                conversation.push(ChatMessage {
                                    role: MessageRole::Assistant,
                                    content: current_turn_text,
                                    command_proposal: None,
                                    attachments: Vec::new(),
                                });

                                let tool_msg = format!(
                                    "[RÉSULTAT DE LA LECTURE DU FICHIER '{}']:\n{}\n[FIN DU RÉSULTAT - Utilisez ces données pour votre diagnostic ou votre édition]",
                                    path, read_result
                                );
                                conversation.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: tool_msg,
                                    command_proposal: None,
                                    attachments: Vec::new(),
                                });

                                let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                continue;
                            }
                            ToolInvocation::EditFile { path, old_string, new_string } => {
                                let is_remote = is_remote_session.load(std::sync::atomic::Ordering::SeqCst);
                                let risk = classify_file_edit(&path);
                                let approved = if should_auto_approve_risk(risk, auto_approve) {
                                    true
                                } else {
                                    let old_preview = old_string.lines().next().unwrap_or(old_string.as_str());
                                    let new_preview = new_string.lines().next().unwrap_or(new_string.as_str());
                                    let preview = if is_remote {
                                        format!(
                                            "(Distant) Modifier le fichier `{}` en remplaçant \"{}\" par \"{}\"",
                                            path, old_preview, new_preview
                                        )
                                    } else {
                                        format!(
                                            "Modifier le fichier `{}` en remplaçant \"{}\" par \"{}\"",
                                            path, old_preview, new_preview
                                        )
                                    };
                                    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel::<bool>();
                                    let _ = forward_event_tx.send(AppEvent::AgentToolRequest {
                                        command: preview,
                                        approval_tx,
                                    });
                                    approval_rx.await.unwrap_or(false)
                                };

                                if approved {
                                    let (edit_result, display_cmd) = if is_remote {
                                        let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("✏️ Édition distante de {}", path)));
                                        // 1. Read remote file first via PTY
                                        let remote_read_cmd = build_remote_read_command(&path);
                                        let (pty_result_tx, pty_result_rx) = tokio::sync::oneshot::channel::<String>();
                                        let _ = forward_event_tx.send(AppEvent::AgentPtyToolExecute {
                                            command: remote_read_cmd,
                                            result_tx: pty_result_tx,
                                        });
                                        let raw_read = pty_result_rx
                                            .await
                                            .unwrap_or_else(|_| "(Erreur lors de l'exécution dans le PTY)".to_string());
                                        let original_content = decode_remote_read_output(&raw_read, &path);

                                        if original_content.starts_with("Erreur") {
                                            (format!("Erreur lors de la lecture préalable de {} : {}", path, original_content), format!("✏️ (distant) {}", path))
                                        } else {
                                            match execute_remote_edit_logic(&original_content, &path, &old_string, &new_string) {
                                                Ok(updated) => {
                                                    let use_sudo = matches!(risk, CommandRisk::Sudo | CommandRisk::Risky);
                                                    let b64 = crate::system::clipboard::base64_encode(updated.as_bytes());
                                                    let remote_write_cmd = build_remote_write_command(&path, &b64, use_sudo);
                                                    let (pty_write_tx, pty_write_rx) = tokio::sync::oneshot::channel::<String>();
                                                    let _ = forward_event_tx.send(AppEvent::AgentPtyToolExecute {
                                                        command: remote_write_cmd,
                                                        result_tx: pty_write_tx,
                                                    });
                                                    let raw_write = pty_write_rx
                                                        .await
                                                        .unwrap_or_else(|_| "(Erreur lors de l'exécution dans le PTY)".to_string());
                                                    let res = if raw_write.contains("Permission denied") || raw_write.contains("No such file") || raw_write.contains("cannot create") {
                                                        format!("Erreur d'écriture distante de {} : {}", path, raw_write.trim())
                                                    } else {
                                                        format!("✅ Fichier distant modifié : {} ({} → {} octets)", path, original_content.len(), updated.len())
                                                    };
                                                    (res, format!("✏️ (distant) {}", path))
                                                }
                                                Err(err) => (err, format!("✏️ (distant) {}", path)),
                                            }
                                        }
                                    } else {
                                        let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("✏️ Édition de {}", path)));
                                        (execute_edit_file(&path, &old_string, &new_string).await, format!("✏️ {}", path))
                                    };

                                    let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                        command: display_cmd,
                                        output: edit_result.clone(),
                                    });

                                    conversation.push(ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: current_turn_text,
                                        command_proposal: None,
                                        attachments: Vec::new(),
                                    });

                                    let tool_msg = format!(
                                        "[RÉSULTAT DE L'ÉDITION DU FICHIER '{}']:\n{}\n[FIN DU RÉSULTAT - Vérifiez le fichier si nécessaire ou poursuivez]",
                                        path, edit_result
                                    );
                                    conversation.push(ChatMessage {
                                        role: MessageRole::User,
                                        content: tool_msg,
                                        command_proposal: None,
                                        attachments: Vec::new(),
                                    });

                                    let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                    continue;
                                } else {
                                    let _ = forward_event_tx.send(AppEvent::AgentDone);
                                    break;
                                }
                            }
                            ToolInvocation::WriteFile { path, content } => {
                                let is_remote = is_remote_session.load(std::sync::atomic::Ordering::SeqCst);
                                let risk = classify_file_edit(&path);
                                let approved = if should_auto_approve_risk(risk, auto_approve) {
                                    true
                                } else {
                                    let preview = if is_remote {
                                        format!("(Distant) Écrire dans `{}` ({} octets)", path, content.len())
                                    } else {
                                        format!("Écrire dans `{}` ({} octets)", path, content.len())
                                    };
                                    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel::<bool>();
                                    let _ = forward_event_tx.send(AppEvent::AgentToolRequest {
                                        command: preview,
                                        approval_tx,
                                    });
                                    approval_rx.await.unwrap_or(false)
                                };

                                if approved {
                                    let (write_result, display_cmd) = if is_remote {
                                        let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("💾 Écriture distante de {}", path)));
                                        let use_sudo = matches!(risk, CommandRisk::Sudo | CommandRisk::Risky);
                                        let b64 = crate::system::clipboard::base64_encode(content.as_bytes());
                                        let remote_write_cmd = build_remote_write_command(&path, &b64, use_sudo);
                                        let (pty_result_tx, pty_result_rx) = tokio::sync::oneshot::channel::<String>();
                                        let _ = forward_event_tx.send(AppEvent::AgentPtyToolExecute {
                                            command: remote_write_cmd,
                                            result_tx: pty_result_tx,
                                        });
                                        let raw_output = pty_result_rx
                                            .await
                                            .unwrap_or_else(|_| "(Erreur lors de l'exécution dans le PTY)".to_string());
                                        let res = if raw_output.contains("Permission denied") || raw_output.contains("No such file") || raw_output.contains("cannot create") {
                                            format!("Erreur d'écriture distante de {} : {}", path, raw_output.trim())
                                        } else {
                                            format!("✅ Fichier distant écrit : {} ({} octets)", path, content.len())
                                        };
                                        (res, format!("💾 (distant) {}", path))
                                    } else {
                                        let _ = forward_event_tx.send(AppEvent::AgentToolStart(format!("💾 Écriture de {}", path)));
                                        (execute_write_file(&path, &content).await, format!("💾 {}", path))
                                    };

                                    let _ = forward_event_tx.send(AppEvent::AgentToolDone {
                                        command: display_cmd,
                                        output: write_result.clone(),
                                    });

                                    conversation.push(ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: current_turn_text,
                                        command_proposal: None,
                                        attachments: Vec::new(),
                                    });

                                    let tool_msg = format!(
                                        "[RÉSULTAT DE L'ÉCRITURE DU FICHIER '{}']:\n{}\n[FIN DU RÉSULTAT - Vérifiez le fichier si nécessaire ou poursuivez]",
                                        path, write_result
                                    );
                                    conversation.push(ChatMessage {
                                        role: MessageRole::User,
                                        content: tool_msg,
                                        command_proposal: None,
                                        attachments: Vec::new(),
                                    });

                                    let _ = forward_event_tx.send(AppEvent::AgentNewTurn);
                                    continue;
                                } else {
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
            attachments: Vec::new(),
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

    #[test]
    fn keeps_attachment_only_turn_for_vision() {
        // A user turn that carries ONLY an image (empty text) must survive preparation so
        // the provider can forward the screenshot, instead of being dropped as "empty".
        let mut m = msg(MessageRole::User, "");
        m.attachments = vec![crate::app::MessageAttachment {
            mime_type: "image/png".to_string(),
            data_base64: "iVBORw0KGgo".to_string(),
        }];
        let conv = prepare_conversation(vec![m]);
        assert_eq!(conv.len(), 1);
        assert_eq!(conv[0].attachments.len(), 1);
        assert_eq!(conv[0].attachments[0].mime_type, "image/png");
        // But a truly empty turn (no content, no attachment) is still dropped.
        let conv2 = prepare_conversation(vec![msg(MessageRole::User, "")]);
        assert_eq!(conv2.len(), 0);
    }
}
