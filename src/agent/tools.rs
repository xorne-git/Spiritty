use std::env;
use std::time::Duration;
use anyhow::Result;
use tokio::time::timeout;
use crate::config::WebSearchConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolInvocation {
    RunCommand(String),
    WebSearch(String),
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
                    format!("(Commande terminée avec code d'erreur {:?})", output.status.code())
                }
            } else if result.len() > 3000 {
                format!("{}\n... [Sortie tronquée à 3000 caractères]", &result[..3000])
            } else {
                result
            }
        }
        Ok(Err(err)) => {
            format!("Erreur lors de l'exécution : {}", err)
        }
        Err(_) => {
            "Erreur : La commande a dépassé le délai d'attente (timeout de 15s).".to_string()
        }
    }
}

/// Checks if text contains a tool execution block ```tool:...``` or loose tool:run_command
pub fn parse_tool_call(text: &str) -> Option<ToolInvocation> {
    // 1. Check for Web Search with ```
    for prefix in &["```tool:web_search", "```tool:search", "```tool:web", "```tool:google"] {
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

    // 2. Check for Web Search without ``` (e.g. "tool:web_search arch linux")
    for prefix in &["tool:web_search", "tool:search", "tool:web"] {
        if let Some(start) = text.find(prefix) {
            let after = &text[start + prefix.len()..];
            let clean = after.trim_start_matches([':', ' ', '\n']);
            let query = clean.lines().next().unwrap_or(clean).trim().to_string();
            if !query.is_empty() {
                return Some(ToolInvocation::WebSearch(query));
            }
        }
    }

    // 3. Command execution with ```
    for prefix in &["```tool:run_command", "```tool:execute_command", "```tool:bash", "```tool:sh"] {
        if let Some(start) = text.find(prefix) {
            let after = &text[start + prefix.len()..];
            let code_start = after.strip_prefix('\n').unwrap_or(after);
            let raw_block = if let Some(end) = code_start.find("```") {
                code_start[..end].trim()
            } else {
                code_start.trim()
            };
            let valid_lines: Vec<&str> = raw_block
                .lines()
                .map(|l| l.trim())
                .filter(|l| is_executable_shell_line(l))
                .collect();

            if !valid_lines.is_empty() {
                return Some(ToolInvocation::RunCommand(valid_lines.join("\n")));
            }
        }
    }

    // 4. Command execution without ``` (e.g. "tool:run_command\nsystemctl ...")
    for prefix in &["tool:run_command", "tool:execute_command"] {
        if let Some(start) = text.find(prefix) {
            let after = &text[start + prefix.len()..];
            let clean = after.trim_start_matches([':', ' ', '\n']);
            let valid_lines: Vec<&str> = clean
                .lines()
                .map(|l| l.trim())
                .filter(|l| is_executable_shell_line(l))
                .collect();

            if !valid_lines.is_empty() {
                return Some(ToolInvocation::RunCommand(valid_lines.join("\n")));
            }
        }
    }

    // 5. XML / Function tags and direct JSON (e.g. DeepSeek/Qwen <tool_call> or <|tool_calls|>)
    if let Some(tool) = parse_json_or_xml_tool_call(text) {
        return Some(tool);
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

    if name.contains("command") || name == "bash" || name == "sh" || name == "execute_command" || name == "run_command" {
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
            val.get("command").and_then(|c| c.as_str()).unwrap_or("").to_string()
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
            val.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string()
        };

        if !query.trim().is_empty() {
            return Some(ToolInvocation::WebSearch(query.trim().to_string()));
        }
    }

    None
}

fn is_executable_shell_line(line: &str) -> bool {
    let l = line.trim();
    if l.is_empty()
        || l.starts_with('#')
        || l.starts_with("//")
        || l.starts_with('(')
        || l.starts_with('>')
        || l.starts_with('|')
        || l.starts_with("</")
        || l.starts_with('<')
        || l.starts_with("```")
        || l.starts_with("---")
        || l.starts_with("===")
        || l.starts_with("• ")
        || l.starts_with("? ")
        || l.starts_with("! ")
        || l.starts_with("📌")
        || l.starts_with("Cas ")
        || l.starts_with("Type de ")
        || l.eq_ignore_ascii_case("bash")
        || l.eq_ignore_ascii_case("sh")
        || l.eq_ignore_ascii_case("zsh")
        || l.eq_ignore_ascii_case("fish")
    {
        return false;
    }

    let lower = l.to_lowercase();
    if lower.starts_with("pour ")
        || lower.starts_with("si ")
        || lower.starts_with("voici ")
        || lower.starts_with("l'utilisateur ")
        || lower.starts_with("vous pouvez ")
        || lower.starts_with("cette commande ")
        || lower.starts_with("in order to ")
        || lower.starts_with("if you ")
        || lower.starts_with("here is ")
    {
        return false;
    }

    true
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
        format!("Aucun résultat web direct trouvé pour \"{}\".", trimmed_query)
    } else {
        sections.join("\n\n")
    }
}

async fn search_duckduckgo_api(client: &reqwest::Client, query: &str) -> Result<String> {
    let url = "https://api.duckduckgo.com/";
    let resp = client
        .get(url)
        .query(&[("q", query), ("format", "json"), ("no_html", "1"), ("skip_disambig", "1")])
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let mut out = Vec::new();

    if let Some(heading) = json.get("Heading").and_then(|v| v.as_str()) {
        if !heading.is_empty() {
            let abstract_text = json.get("AbstractText").and_then(|v| v.as_str()).unwrap_or("");
            let abstract_url = json.get("AbstractURL").and_then(|v| v.as_str()).unwrap_or("");
            if !abstract_text.is_empty() {
                out.push(format!("### DuckDuckGo : {}\n{}\nSource: {}", heading, abstract_text, abstract_url));
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
            out.push(format!("### DuckDuckGo Sujets Associés :\n{}", topics.join("\n")));
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

    if let Some(search_list) = json.get("query").and_then(|q| q.get("search")).and_then(|s| s.as_array()) {
        for item in search_list.iter().take(3) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                let clean_snippet = snippet.replace("<span class=\"searchmatch\">", "").replace("</span>", "");
                let page_url = format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"));
                articles.push(format!("- **{}** : {}\n  URL: {}", title, clean_snippet, page_url));
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

    if let Some(search_list) = json.get("query").and_then(|q| q.get("search")).and_then(|s| s.as_array()) {
        for item in search_list.iter().take(3) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                let clean_snippet = snippet.replace("<span class=\"searchmatch\">", "").replace("</span>", "");
                let page_url = format!("https://wiki.archlinux.org/title/{}", title.replace(' ', "_"));
                articles.push(format!("- **{}** : {}\n  URL: {}", title, clean_snippet, page_url));
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

    if let Some(web_results) = json.get("web").and_then(|w| w.get("results")).and_then(|r| r.as_array()) {
        for item in web_results.iter().take(4) {
            if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                let desc = item.get("description").and_then(|v| v.as_str()).unwrap_or("");
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
