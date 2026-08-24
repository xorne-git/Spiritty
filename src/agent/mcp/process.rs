use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;

use super::{
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpToolCallResult, McpToolDefinition,
    McpToolsListResult,
};

pub struct McpProcess {
    pub name: String,
    stdin: Arc<Mutex<ChildStdin>>,
    req_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    _child: Arc<Mutex<Child>>,
}

impl McpProcess {
    pub async fn spawn(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.envs(env);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // Kill the child process when the handle is dropped (no orphan/zombie leak on reload/shutdown).
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server '{}' with command '{}'", name, command))?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to open stdin for MCP server")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to open stdout for MCP server")?;
        let stderr = child.stderr.take();

        let stdin = Arc::new(Mutex::new(stdin));
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> = Arc::new(Mutex::new(HashMap::new()));
        let child = Arc::new(Mutex::new(child));
        let last_stderr = Arc::new(Mutex::new(String::new()));

        if let Some(err_pipe) = stderr {
            let last_stderr_clone = last_stderr.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err_pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let mut guard = last_stderr_clone.lock().await;
                        if guard.is_empty() {
                            *guard = trimmed.to_string();
                        } else {
                            *guard = format!("{}\n{}", guard, trimmed);
                        }
                        // Bound the buffer to the most recent bytes to avoid unbounded growth.
                        const MAX_STDERR: usize = 4096;
                        if guard.len() > MAX_STDERR {
                            let cut_from = guard.len() - MAX_STDERR;
                            if let Some(nl) = guard[..cut_from].rfind('\n') {
                                guard.drain(..=nl);
                            } else {
                                guard.drain(..cut_from);
                            }
                        }
                    }
                }
            });
        }

        // Background task to read stdout and dispatch JSON-RPC responses
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let line_trim = line.trim();
                if line_trim.is_empty() {
                    continue;
                }
                // Distinguish responses (`result`/`error`) from server→client requests/notifications
                // (which carry a `method` field) so we never correlate a request as a response.
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line_trim) {
                    if val.get("method").is_some() {
                        continue;
                    }
                }
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(line_trim) {
                    if let Some(id) = resp.id {
                        let mut map = pending_clone.lock().await;
                        if let Some(tx) = map.remove(&id) {
                            let _ = tx.send(resp);
                        }
                    }
                }
            }
        });

        let process = Self {
            name: name.to_string(),
            stdin,
            req_id: AtomicU64::new(1),
            pending,
            _child: child,
        };

        // Perform initialization handshake with 12s timeout
        match timeout(Duration::from_secs(12), process.initialize()).await {
            Ok(Ok(())) => Ok(process),
            Ok(Err(e)) => {
                let err_msg = last_stderr.lock().await.clone();
                if !err_msg.is_empty() {
                    let relevant = err_msg
                        .lines()
                        .find(|l| l.contains("error") || l.contains("Error") || l.contains("not found"))
                        .unwrap_or_else(|| err_msg.lines().next().unwrap_or(&err_msg));
                    anyhow::bail!("{}: {}", e, relevant);
                } else {
                    Err(e)
                }
            }
            Err(_) => {
                let err_msg = last_stderr.lock().await.clone();
                if !err_msg.is_empty() {
                    let relevant = err_msg
                        .lines()
                        .find(|l| l.contains("error") || l.contains("Error") || l.contains("not found"))
                        .unwrap_or_else(|| err_msg.lines().next().unwrap_or(&err_msg));
                    anyhow::bail!("MCP error: {}", relevant);
                } else {
                    anyhow::bail!("MCP server handshake timed out after 12s");
                }
            }
        }
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let id = self.req_id.fetch_add(1, Ordering::Relaxed);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let json_line = serde_json::to_string(&req)? + "\n";
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        {
            let mut sin = self.stdin.lock().await;
            sin.write_all(json_line.as_bytes()).await?;
            sin.flush().await?;
        }

        let resp = timeout(Duration::from_secs(20), rx).await;
        if resp.is_err() {
            // Timed out: drop the pending entry to avoid leaking the oneshot sender.
            let mut map = self.pending.lock().await;
            map.remove(&id);
        }
        let resp = resp
            .context("MCP request timed out")?
            .context("MCP response channel closed unexpectedly")?;

        if let Some(err) = resp.error {
            anyhow::bail!("MCP error ({}): {}", err.code, err.message);
        }

        resp.result.context("Empty result in MCP response")
    }

    async fn send_notification(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        let json_line = serde_json::to_string(&notif)? + "\n";
        let mut sin = self.stdin.lock().await;
        sin.write_all(json_line.as_bytes()).await?;
        sin.flush().await?;
        Ok(())
    }

    async fn initialize(&self) -> Result<()> {
        let init_params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "clientInfo": {
                "name": "Spiritty",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let _ = self.send_request("initialize", Some(init_params)).await?;
        self.send_notification("notifications/initialized", None).await?;
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        let result = self.send_request("tools/list", None).await?;
        let parsed: McpToolsListResult = serde_json::from_value(result)?;
        Ok(parsed.tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<McpToolCallResult> {
        let params = json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.send_request("tools/call", Some(params)).await?;
        let parsed: McpToolCallResult = serde_json::from_value(result)?;
        Ok(parsed)
    }
}
