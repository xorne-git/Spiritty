use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{Config, McpServerConfig};
use super::{process::McpProcess, McpToolDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpStatus {
    Connected(usize),
    Disabled,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct McpServerStatus {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub status: McpStatus,
    pub tools: Vec<McpToolDefinition>,
}

#[derive(Clone)]
pub struct McpManager {
    servers: Arc<RwLock<HashMap<String, Arc<McpProcess>>>>,
    statuses: Arc<RwLock<Vec<McpServerStatus>>>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            statuses: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn load_from_config(config: &Config, event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::event::AppEvent>>) -> Self {
        let manager = Self::new();
        let mgr_clone = manager.clone();
        let cfg_clone = config.clone();

        tokio::spawn(async move {
            mgr_clone.reload(&cfg_clone, event_tx).await;
        });

        manager
    }

    pub async fn reload(&self, config: &Config, event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::event::AppEvent>>) {
        let mut new_servers = HashMap::new();
        let mut new_statuses = Vec::new();

        for (name, s_cfg) in &config.mcp_servers {
            if !s_cfg.enabled {
                new_statuses.push(McpServerStatus {
                    name: name.clone(),
                    command: s_cfg.command.clone(),
                    args: s_cfg.args.clone(),
                    enabled: false,
                    status: McpStatus::Disabled,
                    tools: Vec::new(),
                });
                continue;
            }

            match McpProcess::spawn(name, &s_cfg.command, &s_cfg.args, &s_cfg.env).await {
                Ok(proc) => match proc.list_tools().await {
                    Ok(tools) => {
                        let tools_count = tools.len();
                        let arc_proc = Arc::new(proc);
                        new_servers.insert(name.clone(), arc_proc);

                        new_statuses.push(McpServerStatus {
                            name: name.clone(),
                            command: s_cfg.command.clone(),
                            args: s_cfg.args.clone(),
                            enabled: true,
                            status: McpStatus::Connected(tools_count),
                            tools,
                        });
                    }
                    Err(err) => {
                        // `proc` is dropped here → child process killed (kill_on_drop).
                        new_statuses.push(McpServerStatus {
                            name: name.clone(),
                            command: s_cfg.command.clone(),
                            args: s_cfg.args.clone(),
                            enabled: true,
                            status: McpStatus::Error(format!("Échec de découverte des outils: {}", err)),
                            tools: Vec::new(),
                        });
                    }
                },
                Err(err) => {
                    new_statuses.push(McpServerStatus {
                        name: name.clone(),
                        command: s_cfg.command.clone(),
                        args: s_cfg.args.clone(),
                        enabled: true,
                        status: McpStatus::Error(err.to_string()),
                        tools: Vec::new(),
                    });
                }
            }
        }

        {
            let mut s = self.servers.write().await;
            *s = new_servers;
        }
        {
            let mut st = self.statuses.write().await;
            *st = new_statuses;
        }

        if let Some(tx) = event_tx {
            let _ = tx.send(crate::event::AppEvent::McpServersUpdated);
        }
    }

    pub fn get_server_statuses_cached(&self) -> Vec<McpServerStatus> {
        if let Ok(st) = self.statuses.try_read() {
            st.clone()
        } else {
            Vec::new()
        }
    }

    pub async fn get_server_statuses(&self) -> Vec<McpServerStatus> {
        let st = self.statuses.read().await;
        st.clone()
    }

    pub async fn get_tools_summary_for_prompt(&self) -> String {
        let statuses = self.statuses.read().await;
        let mut lines = Vec::new();

        for s in statuses.iter() {
            if let McpStatus::Connected(_) = s.status {
                for t in &s.tools {
                    let desc = t.description.as_deref().unwrap_or("Sans description");
                    lines.push(format!("- `mcp:{}:{}` : {}", s.name, t.name, desc));
                }
            }
        }

        if lines.is_empty() {
            String::new()
        } else {
            format!(
                "\n### Outils MCP (Model Context Protocol) disponibles :\n{}\n\nPour appeler un outil MCP, utilise la syntaxe ```tool:mcp:<serveur>:<nom_outil>\n{{\"param\": \"valeur\"}}\n```\n",
                lines.join("\n")
            )
        }
    }

    pub async fn execute_mcp_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let proc = {
            let servers = self.servers.read().await;
            servers
                .get(server_name)
                .cloned()
                .with_context(|| format!("Serveur MCP '{}' non trouvé ou inactif", server_name))?
        };

        let res = proc.call_tool(tool_name, arguments).await?;
        Ok(res.to_plain_text())
    }

    pub async fn test_single_server(&self, name: &str, s_cfg: &McpServerConfig) -> Result<Vec<McpToolDefinition>> {
        let proc = McpProcess::spawn(name, &s_cfg.command, &s_cfg.args, &s_cfg.env).await?;
        let tools = proc.list_tools().await?;
        Ok(tools)
    }
}
