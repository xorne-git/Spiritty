use crate::config::WebSearchConfig;
use anyhow::Result;
use std::env;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolInvocation {
    RunCommand(String),
    WebSearch(String),
    McpCall {
        server: String,
        tool: String,
        arguments: serde_json::Value,
    },
}

/// Executes a shell command asynchronously and captures its combined stdout and stderr.
#[allow(dead_code)]
pub async fn execute_shell_command(cmd: &str) -> String {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let timeout_duration = Duration::from_secs(15);

    let child_res = tokio::process::Command::new(&shell)
        .args(["-c", cmd])
        .output();

    match timeout(timeout_duration, child_res).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();

            if !stdout.trim().is_empty() {
                result.push_str(stdout.trim());
            }
            if !stderr.trim().is_empty() {
                if !result.is_empty() {
                    result.push_str("\n--- STDERR ---\n");
                }
                result.push_str(stderr.trim());
            }

            if result.trim().is_empty() {
                if output.status.success() {
                    "(Commande exécutée avec succès sans sortie texte / code 0)".to_string()
                } else {
                    format!(
                        "(Commande terminée avec code d'erreur {:?})",
                        output.status.code()
                    )
                }
            } else if result.len() > 3000 {
                let cut = result.floor_char_boundary(3000);
                format!(
                    "{}\n... [Sortie tronquée à 3000 caractères]",
                    &result[..cut]
                )
            } else {
                result
            }
        }
        Ok(Err(err)) => {
            format!("Erreur lors de l'exécution : {}", err)
        }
        Err(_) => "Erreur : La commande a dépassé le délai d'attente (timeout de 15s).".to_string(),
    }
}

/// Checks if text contains an explicit tool execution block ```tool:...``` or explicit XML/JSON function tags
pub fn parse_tool_call(text: &str) -> Option<ToolInvocation> {
    // Reasoning is not action: models stream their private deliberation as
    // <think>…</think> (folded from reasoning_content), which may contain
    // command *examples* (```bash blocks, hypothetical syntax). Never parse a
    // tool call out of it — only the visible answer is actionable.
    let text = strip_think_blocks(text);

    // 1. Check for MCP tool call: ```tool:mcp:<server>:<tool_name>\n{...}\n```
    if let Some(start) = text.find("```tool:mcp:") {
        let after = &text[start + "```tool:mcp:".len()..];
        let first_line_end = after.find('\n').unwrap_or(after.len());
        let header = after[..first_line_end].trim();
        let parts: Vec<&str> = header.split(':').collect();
        if parts.len() >= 2 {
            let server = parts[0].to_string();
            let tool = parts[1].to_string();
            let body_start = if first_line_end < after.len() {
                &after[first_line_end..]
            } else {
                ""
            };
            let json_str = if let Some(end) = body_start.find("```") {
                &body_start[..end]
            } else {
                body_start
            }
            .trim();
            let arguments = serde_json::from_str::<serde_json::Value>(json_str)
                .unwrap_or_else(|_| serde_json::json!({}));
            return Some(ToolInvocation::McpCall {
                server,
                tool,
                arguments,
            });
        }
    }

    // 2. Check for Web Search with ```
    for prefix in &[
        "```tool:web_search",
        "```tool:search",
        "```tool:web",
        "```tool:google",
    ] {
        if let Some(start) = text.find(prefix) {
            let after = &text[start + prefix.len()..];
            let code_start = after.strip_prefix('\n').unwrap_or(after);
            let query_str = if let Some(end) = code_start.find("```") {
                &code_start[..end]
            } else {
                code_start
            };
            let query = query_str.trim().to_string();
            if !query.is_empty() {
                return Some(ToolInvocation::WebSearch(query));
            }
        }
    }

    // 3. Command execution with ```
    for prefix in &["```tool:run_command", "```tool:execute_command"] {
        if let Some(start) = text.find(prefix) {
            let after = &text[start + prefix.len()..];
            let code_start = after.strip_prefix('\n').unwrap_or(after);
            let raw_block = if let Some(end) = code_start.find("```") {
                code_start[..end].trim()
            } else {
                code_start.trim()
            };

            if !raw_block.is_empty() {
                // Same cleanup as markdown proposals: drop interpreter framing (`bash`,
                // shebangs) and dangling `exit` so the command runs as intended in the PTY.
                let cleaned = crate::app::sanitize_proposed_command(raw_block);
                if !cleaned.is_empty() {
                    return Some(ToolInvocation::RunCommand(cleaned));
                }
            }
        }
    }

    // 3b. HTML-style tool tags some models emit instead of the fenced block —
    //     observed live from Gemini: `<tool:run_command>\ncmd\n``` ` (opening
    //     tag, command, then a stray fence, closing tag omitted). The payload
    //     ends at the closing tag, a stray markdown fence, or end of text; a
    //     leading fence wrapping the body (```bash … ```) is unwrapped too.
    for open in &["<tool:run_command>", "<tool:execute_command>"] {
        if let Some(start) = text.find(open) {
            let after = &text[start + open.len()..];
            let end = after
                .find("</tool:run_command>")
                .or_else(|| after.find("</tool:execute_command>"))
                .unwrap_or(after.len());
            let trimmed = after[..end].trim();
            let raw = if let Some(rest) = trimmed.strip_prefix("```") {
                let body = &rest[rest.find('\n').map(|i| i + 1).unwrap_or(0)..];
                &body[..body.find("```").unwrap_or(body.len())]
            } else {
                &trimmed[..trimmed.find("```").unwrap_or(trimmed.len())]
            };
            if !raw.trim().is_empty() {
                let cleaned = crate::app::sanitize_proposed_command(raw.trim());
                if !cleaned.is_empty() {
                    return Some(ToolInvocation::RunCommand(cleaned));
                }
            }
        }
    }

    // 4. XML / Function tags and direct JSON (e.g. DeepSeek/Qwen <tool_call> or <|tool_calls|>)
    if let Some(tool) = parse_json_or_xml_tool_call(&text) {
        return Some(tool);
    }

    // 5. GLM/Z.ai hybrid DSML scaffolding emitted as plain text, e.g.
    //    `<｜｜DSML｜｜tool_calls><｜｜DSML｜｜invoke name="exec_command">…`
    //    (fullwidth pipes U+FF5C). Normalized then parsed as HTML-style tags.
    let normalized = normalize_model_tool_markup(&text);
    if let Some(tool) = parse_html_style_tool_call(&normalized) {
        return Some(tool);
    }

    None
}

/// Removes `<think>…</think>` reasoning regions so their content is never
/// parsed as an actionable tool call. An unterminated `<think>` (stream cut
/// mid-reasoning) strips everything to the end — nothing after it was visible
/// to the user anyway.
fn strip_think_blocks(text: &str) -> std::borrow::Cow<'_, str> {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    if !text.contains(OPEN) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + OPEN.len()..];
        match after_open.find(CLOSE) {
            Some(end) => rest = &after_open[end + CLOSE.len()..],
            None => return std::borrow::Cow::Owned(out), // stream cut mid-reasoning
        }
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// Normalizes the hybrid DSML tool scaffolding some GLM/Z.ai models emit as plain
/// text (fullwidth pipes U+FF5C) into clean XML tags so the HTML-style extractor
/// can read it: `<｜｜DSML｜｜invoke …>` becomes `<invoke …>`.
fn normalize_model_tool_markup(text: &str) -> String {
    let t = text.replace('\u{ff5c}', "|");
    let t = t.replace("||DSML||", "");
    let t = t.replace("|DSML|", "");
    // Stray markers around tags (e.g. `<|tool_calls>` leftovers).
    t.replace("<|", "<").replace("|>", ">")
}

/// Extracts HTML-style tool invocations: `<invoke name="…">` with
/// `<parameter name="command" string="true">…</parameter>` children
/// (GLM DSML scaffolding, Hermes/Mistral style). Returns the first
/// invocation that maps to a known tool; tolerates truncated streams.
fn parse_html_style_tool_call(text: &str) -> Option<ToolInvocation> {
    let mut search_from = 0usize;
    while let Some(rel) = text[search_from..].find("<invoke") {
        let start = search_from + rel;
        let after_tag = &text[start..];

        let name = after_tag
            .find("name=\"")
            .and_then(|p| {
                let rest = &after_tag[p + "name=\"".len()..];
                rest.find('"').map(|e| rest[..e].to_string())
            })
            .unwrap_or_default();

        let block_end = after_tag
            .find("</invoke>")
            .map_or(text.len(), |p| start + p);
        let block = &text[start..block_end];

        let param_value = |param: &str| -> Option<String> {
            let open_tag = format!("<parameter name=\"{param}\"");
            let p = block.find(&open_tag)?;
            let after = &block[p..];
            let gt = after.find('>')?;
            let rest = &after[gt + 1..];
            let end = rest.find("</parameter>")?;
            let value = rest[..end].trim().to_string();
            (!value.is_empty()).then_some(value)
        };

        let tool = if name.contains("command")
            || name == "bash"
            || name == "sh"
            || name == "execute_command"
            || name == "run_command"
        {
            param_value("command").map(ToolInvocation::RunCommand)
        } else if name.contains("search") || name.contains("web") {
            param_value("query")
                .or_else(|| param_value("search_query"))
                .map(ToolInvocation::WebSearch)
        } else {
            None
        };
        if tool.is_some() {
            return tool;
        }
        search_from = start + "<invoke".len();
    }
    None
}

fn parse_json_or_xml_tool_call(text: &str) -> Option<ToolInvocation> {
    let xml_patterns = [
        ("<tool_call>", "</tool_call>"),
        ("<function=", "</function>"),
        ("<|tool_calls|>", "<|/tool_calls|>"),
    ];

    for (open, close) in xml_patterns {
        if let Some(start) = text.find(open) {
            let after_open = &text[start + open.len()..];
            let payload = if let Some(end) = after_open.find(close) {
                &after_open[..end]
            } else {
                after_open
            };

            if let Some(tool) = extract_tool_from_json_or_str(payload) {
                return Some(tool);
            }
        }
    }

    if let Some(start) = text.find("{\"name\"") {
        if let Some(tool) = extract_tool_from_json_or_str(&text[start..]) {
            return Some(tool);
        }
    }

    None
}

fn extract_tool_from_json_or_str(input: &str) -> Option<ToolInvocation> {
    let trimmed = input.trim();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(arr) = val.as_array() {
            if let Some(first) = arr.first() {
                return parse_json_tool_value(first);
            }
        } else {
            return parse_json_tool_value(&val);
        }
    }
    None
}

fn parse_json_tool_value(val: &serde_json::Value) -> Option<ToolInvocation> {
    let name = val.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = val.get("arguments").or_else(|| val.get("parameters"));

    if name.contains("command")
        || name == "bash"
        || name == "sh"
        || name == "execute_command"
        || name == "run_command"
    {
        let cmd = if let Some(a) = args {
            if let Some(s) = a.as_str() {
                s.to_string()
            } else {
                a.get("command")
                    .or_else(|| a.get("cmd"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string()
            }
        } else {
            val.get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        };

        if !cmd.trim().is_empty() {
            return Some(ToolInvocation::RunCommand(cmd.trim().to_string()));
        }
    } else if name.contains("search") || name.contains("web") {
        let query = if let Some(a) = args {
            if let Some(s) = a.as_str() {
                s.to_string()
            } else {
                a.get("query")
                    .or_else(|| a.get("search_query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or("")
                    .to_string()
            }
        } else {
            val.get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .to_string()
        };

        if !query.trim().is_empty() {
            return Some(ToolInvocation::WebSearch(query.trim().to_string()));
        }
    }

    None
}

/// Executes an internet web search asynchronously across multiple sources
pub async fn execute_web_search(query: &str, config: &WebSearchConfig) -> String {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) Spiritty/0.2")
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Erreur initialisation client HTTP : {}", e),
    };

    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return "Requête de recherche vide.".to_string();
    }

    // 1. Custom configured providers
    if let Some(ref brave_key) = config.brave_api_key {
        if !brave_key.trim().is_empty() {
            if let Ok(res) = search_brave(&client, trimmed_query, brave_key.trim()).await {
                if !res.trim().is_empty() {
                    return res;
                }
            }
        }
    }

    if let Some(ref searx_url) = config.searxng_url {
        if !searx_url.trim().is_empty() {
            if let Ok(res) = search_searxng(&client, trimmed_query, searx_url.trim()).await {
                if !res.trim().is_empty() {
                    return res;
                }
            }
        }
    }

    if let Some(ref tavily_key) = config.tavily_api_key {
        if !tavily_key.trim().is_empty() {
            if let Ok(res) = search_tavily(&client, trimmed_query, tavily_key.trim()).await {
                if !res.trim().is_empty() {
                    return res;
                }
            }
        }
    }

    // 2. Free Built-in Multi-Source Aggregator (DuckDuckGo + Wikipedia + ArchWiki)
    let mut sections = Vec::new();

    if let Ok(ddg_res) = search_duckduckgo_api(&client, trimmed_query).await {
        if !ddg_res.trim().is_empty() {
            sections.push(ddg_res);
        }
    }

    if let Ok(wiki_res) = search_wikipedia(&client, trimmed_query).await {
        if !wiki_res.trim().is_empty() {
            sections.push(wiki_res);
        }
    }

    if let Ok(arch_res) = search_archwiki(&client, trimmed_query).await {
        if !arch_res.trim().is_empty() {
            sections.push(arch_res);
        }
    }

    if sections.is_empty() {
        format!(
            "Aucun résultat web direct trouvé pour \"{}\".",
            trimmed_query
        )
    } else {
        sections.join("\n\n")
    }
}

async fn search_duckduckgo_api(client: &reqwest::Client, query: &str) -> Result<String> {
    let url = "https://api.duckduckgo.com/";
    let resp = client
        .get(url)
        .query(&[
            ("q", query),
            ("format", "json"),
            ("no_html", "1"),
            ("skip_disambig", "1"),
        ])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut out = Vec::new();

    if let Some(heading) = json.get("Heading").and_then(|v| v.as_str()) {
        if !heading.is_empty() {
            let abstract_text = json
                .get("AbstractText")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let abstract_url = json
                .get("AbstractURL")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !abstract_text.is_empty() {
                out.push(format!(
                    "### DuckDuckGo : {}\n{}\nSource: {}",
                    heading, abstract_text, abstract_url
                ));
            }
        }
    }

    if let Some(related) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
        let mut topics = Vec::new();
        for item in related.iter().take(4) {
            if let Some(text) = item.get("Text").and_then(|v| v.as_str()) {
                let url = item.get("FirstURL").and_then(|v| v.as_str()).unwrap_or("");
                topics.push(format!("- {} ({})", text, url));
            }
        }
        if !topics.is_empty() && out.is_empty() {
            out.push(format!(
                "### DuckDuckGo Sujets Associés :\n{}",
                topics.join("\n")
            ));
        }
    }

    Ok(out.join("\n\n"))
}

async fn search_wikipedia(client: &reqwest::Client, query: &str) -> Result<String> {
    let url = "https://en.wikipedia.org/w/api.php";
    let resp = client
        .get(url)
        .query(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", query),
            ("format", "json"),
            ("utf8", "1"),
        ])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut articles = Vec::new();

    if let Some(search_list) = json
        .get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
    {
        for item in search_list.iter().take(3) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                let clean_snippet = snippet
                    .replace("<span class=\"searchmatch\">", "")
                    .replace("</span>", "");
                let page_url = format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"));
                articles.push(format!(
                    "- **{}** : {}\n  URL: {}",
                    title, clean_snippet, page_url
                ));
            }
        }
    }

    if articles.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("### Wikipédia :\n{}", articles.join("\n")))
    }
}

async fn search_archwiki(client: &reqwest::Client, query: &str) -> Result<String> {
    let url = "https://wiki.archlinux.org/api.php";
    let resp = client
        .get(url)
        .query(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", query),
            ("format", "json"),
            ("utf8", "1"),
        ])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut articles = Vec::new();

    if let Some(search_list) = json
        .get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
    {
        for item in search_list.iter().take(3) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                let clean_snippet = snippet
                    .replace("<span class=\"searchmatch\">", "")
                    .replace("</span>", "");
                let page_url = format!(
                    "https://wiki.archlinux.org/title/{}",
                    title.replace(' ', "_")
                );
                articles.push(format!(
                    "- **{}** : {}\n  URL: {}",
                    title, clean_snippet, page_url
                ));
            }
        }
    }

    if articles.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("### ArchWiki :\n{}", articles.join("\n")))
    }
}

async fn search_brave(client: &reqwest::Client, query: &str, api_key: &str) -> Result<String> {
    let url = "https://api.search.brave.com/res/v1/web/search";
    let resp = client
        .get(url)
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .query(&[("q", query), ("count", "4")])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut results = Vec::new();

    if let Some(web_results) = json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
    {
        for item in web_results.iter().take(4) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let desc = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let link = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                results.push(format!("- **{}** : {}\n  URL: {}", title, desc, link));
            }
        }
    }

    if results.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("### Brave Search :\n{}", results.join("\n")))
    }
}

async fn search_searxng(client: &reqwest::Client, query: &str, base_url: &str) -> Result<String> {
    let clean_url = format!("{}/search", base_url.trim_end_matches('/'));
    let resp = client
        .get(&clean_url)
        .query(&[("q", query), ("format", "json")])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut results = Vec::new();

    if let Some(res_array) = json.get("results").and_then(|r| r.as_array()) {
        for item in res_array.iter().take(4) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                results.push(format!("- **{}** : {}\n  URL: {}", title, content, url));
            }
        }
    }

    if results.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("### SearXNG :\n{}", results.join("\n")))
    }
}

async fn search_tavily(client: &reqwest::Client, query: &str, api_key: &str) -> Result<String> {
    let url = "https://api.tavily.com/search";
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "search_depth": "basic",
        "max_results": 4
    });

    let resp = client.post(url).json(&body).send().await?;
    let json: serde_json::Value = resp.json().await?;
    let mut results = Vec::new();

    if let Some(res_array) = json.get("results").and_then(|r| r.as_array()) {
        for item in res_array.iter().take(4) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                results.push(format!("- **{}** : {}\n  URL: {}", title, content, url));
            }
        }
    }

    if results.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("### Tavily Search :\n{}", results.join("\n")))
    }
}

#[cfg(test)]
mod tool_parse_tests {
    use super::{parse_tool_call, ToolInvocation};

    /// Exact hybrid scaffolding captured from a live GLM/Z.ai session: fullwidth
    /// pipes U+FF5C, `<｜｜DSML｜｜…>` markers, HTML-style invoke/parameter children.
    #[test]
    fn parses_glm_dsml_hybrid_tool_call() {
        let text = "D'abord la r\u{e9}sa en base :\n\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"command\" string=\"true\">cd /var/www && echo ok</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand(
                "cd /var/www && echo ok".to_string()
            ))
        );
    }

    #[test]
    fn parses_truncated_stream_tool_call() {
        // Stream cut mid-block: no closing tags at all.
        let text = "Je v\u{e9}rifie.\n\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"command\" string=\"true\">uptime</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("uptime".to_string()))
        );
    }

    #[test]
    fn parses_single_pipe_dsml_variant() {
        let text = "<\u{ff5c}DSML\u{ff5c}invoke name=\"exec_command\"><\u{ff5c}DSML\u{ff5c}parameter name=\"command\" string=\"true\">ls -la</\u{ff5c}DSML\u{ff5c}parameter></\u{ff5c}DSML\u{ff5c}invoke>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("ls -la".to_string()))
        );
    }

    #[test]
    fn clean_xml_tool_call_also_parses() {
        let text = "<tool_calls>\n<invoke name=\"exec_command\">\n<parameter name=\"command\" string=\"true\">df -h</parameter>\n</invoke>\n</tool_calls>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("df -h".to_string()))
        );
    }

    #[test]
    fn web_search_tool_call_parses() {
        let text = "<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"web_search\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"query\" string=\"true\">rust ratatui</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::WebSearch("rust ratatui".to_string()))
        );
    }

    #[test]
    fn prose_without_tool_call_stays_none() {
        let text = "Voici mon analyse du probl\u{e8}me, rien \u{e0} ex\u{e9}cuter.";
        assert_eq!(parse_tool_call(text), None);
    }

    #[test]
    fn parses_gemini_html_style_tool_tag_with_stray_fence() {
        // Exact malformed payload from a live Gemini session: opening HTML-style
        // tag, command, then a stray markdown fence, closing tag omitted.
        let text = "Je relance SDDM pour revenir au greeter proprement :\n\n<tool:run_command>\nsudo systemctl restart sddm\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand(
                "sudo systemctl restart sddm".to_string()
            ))
        );
    }

    #[test]
    fn parses_html_style_tag_with_fenced_body() {
        let text = "<tool:run_command>\n```bash\ndf -h\n```\n";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("df -h".to_string()))
        );
    }

    #[test]
    fn parses_closed_html_style_tag() {
        let text = "<tool:run_command>uptime</tool:run_command>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("uptime".to_string()))
        );
    }

    #[test]
    fn never_parses_tool_call_inside_think_block() {
        // Reasoning is not action: a ```tool: block or XML call inside the
        // model's private <think> deliberation must not trigger execution.
        let text = "<think>Je devrais lancer :\n```tool:run_command\nrm -rf /\n```\nou via <tool_call>{\"name\":\"run_command\",\"arguments\":\"ls\"}</tool_call></think>Voici mon analyse.";
        assert_eq!(parse_tool_call(text), None);
    }

    #[test]
    fn parses_tool_call_after_closed_think_block() {
        let text = "<think>La commande est :\n```bash\nsudo reboot\n```\nAllons-y.</think>Je relance le service :\n<tool:run_command>\nsudo systemctl restart nginx\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand(
                "sudo systemctl restart nginx".to_string()
            ))
        );
    }
}
