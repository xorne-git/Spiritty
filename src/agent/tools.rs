use crate::config::WebSearchConfig;
use anyhow::Result;
use std::env;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolInvocation {
    RunCommand(String),
    WebSearch(String),
    ReadFile(String),
    EditFile {
        path: String,
        old_string: String,
        new_string: String,
    },
    WriteFile {
        path: String,
        content: String,
    },
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

/// Reads a file asynchronously, truncating very large outputs (like PTY captures) and
/// showing a byte count so the model can decide to gate further reads.
pub async fn execute_read_file(path: &str) -> String {
    const MAX_BYTES: usize = 100 * 1024;
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            if bytes.is_empty() {
                return "(Fichier vide)".to_string();
            }
            let shown = if bytes.len() > MAX_BYTES {
                &bytes[..MAX_BYTES]
            } else {
                &bytes[..]
            };
            let text = String::from_utf8_lossy(shown);
            let mut out = text.to_string();
            if bytes.len() > MAX_BYTES {
                out.push_str(&format!(
                    "\n\n… [Sortie tronquée : {} octets affichés sur {}]",
                    MAX_BYTES,
                    bytes.len()
                ));
            }
            out
        }
        Err(err) => format!("Erreur de lecture de {} : {}", path, err),
    }
}

/// Overwrites a file with the given content. Fails if the target is a directory.
pub async fn execute_write_file(path: &str, content: &str) -> String {
    match tokio::fs::write(path, content).await {
        Ok(()) => format!("✅ Fichier écrit : {} ({} octets)", path, content.len()),
        Err(err) => format!("Erreur d'écriture de {} : {}", path, err),
    }
}

/// Applies a single exact-string replacement within a file. The `old_string` must occur
/// exactly once; zero or multiple occurrences is reported back (never a silent no-op or
/// an ambiguous partial replacement). The file is rewritten with the replacement applied.
pub async fn execute_edit_file(path: &str, old_string: &str, new_string: &str) -> String {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => {
            let occurrences = contents.matches(old_string).count();
            if occurrences == 0 {
                return format!(
                    "Erreur : le texte à remplacer est introuvable dans {}. Vérifiez le contenu exact (lecture avec tool:read_file) avant de réessayer.",
                    path
                );
            }
            if occurrences > 1 {
                return format!(
                    "Erreur : le texte à remplacer apparaît {} fois dans {} (il doit être unique). Fournissez un fragment plus précis incluant son contexte.",
                    occurrences, path
                );
            }
            let updated = contents.replace(old_string, new_string);
            match tokio::fs::write(path, updated.as_bytes()).await {
                Ok(()) => format!(
                    "✅ Fichier modifié : {} ({} → {} octets)",
                    path,
                    contents.len(),
                    updated.len()
                ),
                Err(err) => format!("Erreur d'écriture de {} : {}", path, err),
            }
        }
        Err(err) => format!("Erreur de lecture de {} : {}", path, err),
    }
}

/// Builds a shell command to read a remote file via base64 in the remote PTY.
pub fn build_remote_read_command(path: &str) -> String {
    let safe_path = path.replace('"', "\\\"");
    format!("(base64 -w 0 \"{safe_path}\" 2>/dev/null || base64 \"{safe_path}\" 2>/dev/null || cat \"{safe_path}\")")
}

/// Builds a shell command to write content via base64 in the remote PTY.
pub fn build_remote_write_command(path: &str, content_base64: &str, use_sudo: bool) -> String {
    let safe_path = path.replace('"', "\\\"");
    if use_sudo {
        format!("printf '%s' '{content_base64}' | base64 -d | sudo tee \"{safe_path}\" > /dev/null")
    } else {
        format!("printf '%s' '{content_base64}' | base64 -d > \"{safe_path}\"")
    }
}

/// Decodes base64 string bytes into Vec<u8> safely.
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(input).ok()
}

/// Decodes the output captured from a remote base64 read command.
pub fn decode_remote_read_output(output: &str, path: &str) -> String {
    const MAX_BYTES: usize = 100 * 1024;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "(Fichier vide)".to_string();
    }

    // Check for common shell error patterns
    if trimmed.contains("No such file or directory")
        || trimmed.contains("Permission denied")
        || trimmed.contains("Is a directory")
        || trimmed.contains("cannot open")
        || trimmed.contains("Aucun fichier ou dossier")
    {
        return format!("Erreur de lecture distante de {} : {}", path, trimmed);
    }

    // Try base64 decoding (strip whitespace/newlines that base64 command might output)
    let clean_b64: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if !clean_b64.is_empty() {
        if let Some(bytes) = base64_decode(&clean_b64) {
            if bytes.is_empty() {
                return "(Fichier vide)".to_string();
            }
            let shown = if bytes.len() > MAX_BYTES {
                &bytes[..MAX_BYTES]
            } else {
                &bytes[..]
            };
            let text = String::from_utf8_lossy(shown);
            let mut out = text.to_string();
            if bytes.len() > MAX_BYTES {
                out.push_str(&format!(
                    "\n\n… [Sortie tronquée : {} octets affichés sur {}]",
                    MAX_BYTES,
                    bytes.len()
                ));
            }
            return out;
        }
    }

    // Fallback: treat as plain text if cat fallback was used
    let bytes = trimmed.as_bytes();
    let shown = if bytes.len() > MAX_BYTES {
        &bytes[..MAX_BYTES]
    } else {
        bytes
    };
    let text = String::from_utf8_lossy(shown);
    let mut out = text.to_string();
    if bytes.len() > MAX_BYTES {
        out.push_str(&format!(
            "\n\n… [Sortie tronquée : {} octets affichés sur {}]",
            MAX_BYTES,
            bytes.len()
        ));
    }
    out
}

/// Applies replacement logic on a string, verifying uniqueness.
pub fn execute_remote_edit_logic(
    original_content: &str,
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<String, String> {
    let occurrences = original_content.matches(old_string).count();
    if occurrences == 0 {
        return Err(format!(
            "Erreur : le texte à remplacer est introuvable dans {}. Vérifiez le contenu exact (lecture avec tool:read_file) avant de réessayer.",
            path
        ));
    }
    if occurrences > 1 {
        return Err(format!(
            "Erreur : le texte à remplacer apparaît {} fois dans {} (il doit être unique). Fournissez un fragment plus précis incluant son contexte.",
            occurrences, path
        ));
    }
    Ok(original_content.replace(old_string, new_string))
}

/// Checks if text contains an explicit tool execution block ```tool:...``` or explicit XML/JSON function tags
pub fn parse_tool_call(text: &str) -> Option<ToolInvocation> {
    let stripped = strip_think_blocks(text);
    parse_tool_call_inner(&stripped)
}

fn parse_tool_call_inner(text: &str) -> Option<ToolInvocation> {
    // 1. Check for MCP tool call: ```tool:mcp:<server>:<tool_name>\n{...}\n```
    let mut mcp_search_from = 0;
    while let Some(rel) = text[mcp_search_from..].find("```tool:mcp:") {
        let start = mcp_search_from + rel;
        mcp_search_from = start + "```tool:mcp:".len();

        let line_start = text[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        if !text[line_start..start]
            .chars()
            .all(|c| c == ' ' || c == '\t')
        {
            continue;
        }

        let after = &text[start + "```tool:mcp:".len()..];
        let Some(first_line_end) = after.find('\n') else {
            continue;
        };
        let header = after[..first_line_end].trim();
        let parts: Vec<&str> = header.split(':').collect();
        if parts.len() < 2 {
            continue;
        }

        let server = parts[0].to_string();
        let tool = parts[1].to_string();
        let body_start = &after[first_line_end + 1..];

        let Some(end) = body_start.find("```") else {
            continue;
        };

        let json_str = body_start[..end].trim();
        let arguments = serde_json::from_str::<serde_json::Value>(json_str)
            .unwrap_or_else(|_| serde_json::json!({}));
        return Some(ToolInvocation::McpCall {
            server,
            tool,
            arguments,
        });
    }

    // 2. Check for Web Search with ```
    for prefix in &[
        "```tool:web_search",
        "```tool:search",
        "```tool:web",
        "```tool:google",
    ] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            search_from = start + prefix.len();

            let line_start = text[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            if !text[line_start..start]
                .chars()
                .all(|c| c == ' ' || c == '\t')
            {
                continue;
            }

            let after = &text[start + prefix.len()..];
            let Some(first_nl) = after.find('\n') else {
                continue;
            };
            let tag_rest = after[..first_nl].trim();
            let code_start = &after[first_nl + 1..];

            let query = if !tag_rest.is_empty() {
                tag_rest.to_string()
            } else if let Some(end_rel) = code_start.find("```") {
                code_start[..end_rel].trim().to_string()
            } else {
                continue;
            };

            if !query.is_empty() && query != "..." && query != "…" {
                return Some(ToolInvocation::WebSearch(query));
            }
        }
    }

    // 3. Command execution with ```
    for prefix in &["```tool:run_command", "```tool:execute_command"] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            search_from = start + prefix.len();

            let line_start = text[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            if !text[line_start..start]
                .chars()
                .all(|c| c == ' ' || c == '\t')
            {
                continue;
            }

            let after = &text[start + prefix.len()..];
            let Some(first_nl) = after.find('\n') else {
                continue;
            };
            let tag_rest = &after[..first_nl];
            if !tag_rest.trim().is_empty() {
                continue;
            }

            let code_start = &after[first_nl + 1..];
            let Some(end) = crate::app::find_closing_code_fence(code_start) else {
                continue;
            };
            let raw_block = code_start[..end].trim();

            if !raw_block.is_empty() {
                let cleaned = crate::app::sanitize_proposed_command(raw_block);
                if is_plausible_shell_command(&cleaned) {
                    return Some(ToolInvocation::RunCommand(cleaned));
                }
            }
        }
    }

    // 3.5 File-editing tools (```tool:read_file / edit_file / write_file```) — a proper
    //     structured alternative to the sed/awk/heredoc shell dance the model falls back to.
    for (prefix, tool) in [
        ("```tool:read_file", "read"),
        ("```tool:edit_file", "edit"),
        ("```tool:write_file", "write"),
    ] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            search_from = start + prefix.len();

            let line_start = text[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            if !text[line_start..start]
                .chars()
                .all(|c| c == ' ' || c == '\t')
            {
                continue;
            }

            let after = &text[start + prefix.len()..];
            let Some(first_nl) = after.find('\n') else {
                continue;
            };
            let tag_rest = &after[..first_nl];
            if !tag_rest.trim().is_empty() {
                continue;
            }

            let code_start = &after[first_nl + 1..];
            let Some(end) = crate::app::find_closing_code_fence(code_start) else {
                continue;
            };

            let body = &code_start[..end];
            if let Some(t) = parse_file_edit_fence(body.trim(), tool) {
                return Some(t);
            }
        }
    }

    // 3b. HTML-style tool tags some models emit instead of the fenced block —
    //     observed live from Gemini: `<tool:run_command>\ncmd\n``` ` (opening
    //     tag, command, then a stray fence, closing tag omitted).
    for open in &["<tool:run_command>", "<tool:execute_command>"] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(open) {
            let start = search_from + rel;
            search_from = start + open.len();

            let after = &text[start + open.len()..];
            let (raw, has_end): (&str, bool) = if let Some(end) = after
                .find("</tool:run_command>")
                .or_else(|| after.find("</tool:execute_command>"))
            {
                let inside = after[..end].trim();
                if let Some(rest) = inside.strip_prefix("```") {
                    let body = &rest[rest.find('\n').map(|i| i + 1).unwrap_or(0)..];
                    let fence_end = body.find("```").unwrap_or(body.len());
                    (&body[..fence_end], true)
                } else {
                    (inside, true)
                }
            } else {
                let trimmed_after = after.trim_start();
                if let Some(rest) = trimmed_after.strip_prefix("```") {
                    let body = &rest[rest.find('\n').map(|i| i + 1).unwrap_or(0)..];
                    if let Some(fence_end) = body.find("```") {
                        (&body[..fence_end], true)
                    } else {
                        ("", false)
                    }
                } else if let Some(end) = after.find("```") {
                    // Stray fence closing: <tool:run_command>\ncmd\n```
                    let inside = after[..end].trim();
                    (inside, true)
                } else {
                    ("", false)
                }
            };

            if !has_end {
                continue;
            }

            let cleaned = crate::app::sanitize_proposed_command(raw.trim());
            if is_plausible_shell_command(&cleaned) {
                return Some(ToolInvocation::RunCommand(cleaned));
            }
        }
    }

    // 4. XML / Function tags and direct JSON (e.g. DeepSeek/Qwen <tool_call> or <|tool_calls|>)
    if let Some(tool) = parse_json_or_xml_tool_call(text) {
        return Some(tool);
    }

    // 5. GLM/Z.ai hybrid DSML scaffolding emitted as plain text, e.g.
    //    `<｜｜DSML｜｜tool_calls><｜｜DSML｜｜invoke name="exec_command">…`
    //    (fullwidth pipes U+FF5C). Normalized then parsed as HTML-style tags.
    let normalized = normalize_model_tool_markup(text);
    if let Some(tool) = parse_html_style_tool_call(&normalized) {
        return Some(tool);
    }

    None
}

/// True when a command string looks like an actionable shell command, rather than
/// an ellipsis, placeholder (`<command>`, `<unit>`), or prose snippet.
pub fn is_plausible_shell_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "..."
        || trimmed == "…"
        || trimmed == "<command>"
        || trimmed == "commande"
        || trimmed == "cmd"
        || trimmed == "<action>"
        || trimmed == "<unit>"
    {
        return false;
    }
    if trimmed.starts_with('<') && trimmed.ends_with('>') && !trimmed.contains(' ') {
        return false;
    }
    if trimmed.lines().all(|l| l.trim().starts_with('#')) {
        return false;
    }
    true
}

/// Parses the body of a fenced file-edit tool block into a `ToolInvocation`.
///
/// Three layouts are supported, mirroring what the model is instructed to emit:
/// - `read`: the body is the absolute/expanded path (`/path/to/file`).
/// - `write`: the first line is the path, the remainder is the full verbatim content.
/// - `edit`: the first line is the path, then the literal `old_string` to replace, then
///   a separator line `---`, then the literal `new_string`. Splitting on a separator
///   (rather than line count) keeps multi-line old/new payloads intact.
fn parse_file_edit_fence(body: &str, kind: &str) -> Option<ToolInvocation> {
    if body.is_empty() {
        return None;
    }

    let line_end = body.find('\n').unwrap_or(body.len());
    let path_str = body[..line_end].trim();
    if path_str.is_empty() {
        return None;
    }
    let path = crate::app::expand_tilde(path_str);
    let path = path.to_string_lossy().into_owned();

    let payload = &body[line_end..].trim_start_matches(['\r', '\n']);

    match kind {
        "read" => Some(ToolInvocation::ReadFile(path)),
        "write" => Some(ToolInvocation::WriteFile {
            path,
            content: payload.to_string(),
        }),
        "edit" => {
            // A payload that immediately starts with the separator (or is empty) means the
            // old_string is missing — reject rather than corrupting a file with an ambiguous parse.
            if payload.is_empty() || payload.starts_with("---\n") || payload.trim() == "---" {
                return None;
            }
            let mut split = payload.splitn(2, "\n---\n");
            let old_string = split.next().unwrap_or("").trim_matches('\r').to_string();
            let new_string = split.next().unwrap_or("").trim_matches('\r').to_string();
            if old_string.is_empty() {
                return None;
            }
            Some(ToolInvocation::EditFile {
                path,
                old_string,
                new_string,
            })
        }
        _ => None,
    }
}

/// Finds the earliest occurrence of any tag in the slice, returning (byte_offset, tag_len).
pub(crate) fn find_earliest_tag(text: &str, tags: &[&str]) -> Option<(usize, usize)> {
    let mut earliest: Option<(usize, usize)> = None;
    for tag in tags {
        if let Some(pos) = text.find(tag) {
            if earliest.is_none_or(|(ep, _)| pos < ep) {
                earliest = Some((pos, tag.len()));
            }
        }
    }
    earliest
}

/// Removes reasoning regions (`<think>…</think>`, `<thought>…</thought>`, etc.) so
/// their internal deliberation is never parsed as an actionable tool call. Handles
/// typo variants emitted by real models (e.g. `</thunk>`, `</thinking>`, `</thought>`,
/// `</th`) and transitions where tool calls follow directly without a closing tag.
pub fn strip_think_blocks(text: &str) -> std::borrow::Cow<'_, str> {
    const OPEN_TAGS: &[&str] = &[
        "<think>",
        "<thought>",
        "<thinking>",
        "<reasoning>",
        "<reflection>",
        "<plan>",
        "<thought_process>",
    ];

    const CLOSE_TAGS: &[&str] = &[
        "</think>",
        "</thunk>",
        "</thought>",
        "</thinking>",
        "</thinking",
        "</reasoning>",
        "</reflection>",
        "</plan>",
        "</thought_process>",
    ];

    const TOOL_STARTS: &[&str] = &[
        "<｜｜DSML｜｜",
        "<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}",
        "<|DSML|",
        "<|tool_calls|>",
        "<tool_call>",
        "<tool_calls>",
        "<invoke",
        "<command>",
        "<tool:run_command>",
        "<tool:execute_command>",
        "```tool:",
        "```bash",
        "```sh",
        "```zsh",
    ];

    if !OPEN_TAGS.iter().any(|op| text.contains(op)) {
        return std::borrow::Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some((start, open_tag_len)) = find_earliest_tag(rest, OPEN_TAGS) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + open_tag_len..];

        if let Some((close_rel, close_tag_len)) = find_earliest_tag(after_open, CLOSE_TAGS) {
            rest = &after_open[close_rel + close_tag_len..];
        } else if let Some((tool_rel, _)) = find_earliest_tag(after_open, TOOL_STARTS) {
            rest = &after_open[tool_rel..];
        } else if after_open.ends_with("</th")
            || after_open.ends_with("</thi")
            || after_open.ends_with("</thin")
        {
            // Stream cut right as the closing tag was beginning at EOF
            return std::borrow::Cow::Owned(out);
        } else {
            // Stream cut mid-reasoning: drop to end
            return std::borrow::Cow::Owned(out);
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
/// `<parameter name="command" string="true">…</parameter>` or `<command>…</command>` children
/// (GLM/DeepSeek DSML scaffolding, Hermes/Mistral style). Returns the first
/// invocation that maps to a known tool; tolerates truncated streams.
fn parse_html_style_tool_call(text: &str) -> Option<ToolInvocation> {
    // 1. Direct <command>...</command> inside tool scaffolding
    if let Some(p) = text.find("<command>") {
        let after = &text[p + "<command>".len()..];
        if let Some(end) = after.find("</command>") {
            let val = after[..end].trim().to_string();
            if !val.is_empty() {
                return Some(ToolInvocation::RunCommand(val));
            }
        }
    }

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
        let name_lower = name.to_lowercase();

        let block_end = after_tag
            .find("</invoke>")
            .map_or(text.len(), |p| start + p);
        let block = &text[start..block_end];

        let param_value = |param: &str| -> Option<String> {
            let open_tag = format!("<parameter name=\"{param}\"");
            if let Some(p) = block.find(&open_tag) {
                if let Some(gt) = block[p..].find('>') {
                    let rest = &block[p + gt + 1..];
                    if let Some(end) = rest.find("</parameter>") {
                        let value = rest[..end].trim().to_string();
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                }
            }
            // Also support direct <tag>...</tag> (e.g. <command>...</command>)
            let direct_tag = format!("<{param}>");
            if let Some(p) = block.find(&direct_tag) {
                let after = &block[p + direct_tag.len()..];
                let close_tag = format!("</{param}>");
                if let Some(end) = after.find(&close_tag) {
                    let value = after[..end].trim().to_string();
                    if !value.is_empty() {
                        return Some(value);
                    }
                }
            }
            None
        };

        let tool = if name_lower.contains("command")
            || name_lower == "bash"
            || name_lower == "sh"
            || name_lower == "execute_command"
            || name_lower == "run_command"
            || name_lower == "terminal"
        {
            param_value("command")
                .or_else(|| param_value("cmd"))
                .map(ToolInvocation::RunCommand)
        } else if name_lower.contains("search") || name_lower.contains("web") {
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
    fn parses_deepseek_nested_dsml_skill_command() {
        let text = "<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<invoke name=\"Bash\">\n<skill name=\"Bash\">\n<command>sed -n '625,660p' /tmp/file.qml</command>\n</skill>\n</invoke>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand(
                "sed -n '625,660p' /tmp/file.qml".to_string()
            ))
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

    #[test]
    fn parses_read_file_fence() {
        let home = dirs::home_dir().expect("home dir");
        let text = "Je lis la config :\n```tool:read_file\n~/.config/niri/config.kdl\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::ReadFile(
                home.join(".config/niri/config.kdl")
                    .to_string_lossy()
                    .into_owned()
            ))
        );
    }

    #[test]
    fn parses_write_file_fence_with_verbatim_content() {
        let text = "Je crée le fichier :\n```tool:write_file\n/tmp/t.txt\nligne1\nligne2\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::WriteFile {
                path: "/tmp/t.txt".to_string(),
                content: "ligne1\nligne2".to_string(),
            })
        );
    }

    #[test]
    fn parses_edit_file_fence_with_multi_line_payload() {
        let text = "Je remplace un bloc :\n```tool:edit_file\n/tmp/t.txt\nold line one\nold line two\n---\nnew line one\nnew line two\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::EditFile {
                path: "/tmp/t.txt".to_string(),
                old_string: "old line one\nold line two".to_string(),
                new_string: "new line one\nnew line two".to_string(),
            })
        );
    }

    #[test]
    fn rejects_edit_file_with_empty_old_string() {
        let text = "```tool:edit_file\n/tmp/t.txt\n---\nnew\n```";
        assert_eq!(parse_tool_call(text), None);
    }

    #[test]
    fn file_edit_fence_inside_think_block_is_ignored() {
        let text = "<think>je vais éditer :\n```tool:edit_file\n/tmp/t.txt\nold\n---\nnew\n```</think>Réponse.";
        assert_eq!(parse_tool_call(text), None);
    }

    #[tokio::test]
    async fn write_edit_read_roundtrip() {
        use super::{execute_edit_file, execute_read_file, execute_write_file};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        let path_str = path.to_string_lossy().into_owned();

        let w = execute_write_file(&path_str, "line one\nline two\nline three\n").await;
        assert!(w.contains("✅ Fichier écrit"), "write: {}", w);

        let r = execute_read_file(&path_str).await;
        assert_eq!(r, "line one\nline two\nline three\n");

        // Unique single replacement.
        let e = execute_edit_file(&path_str, "line two", "line two EDITED").await;
        assert!(e.contains("✅ Fichier modifié"), "edit: {}", e);
        let r2 = execute_read_file(&path_str).await;
        assert_eq!(r2, "line one\nline two EDITED\nline three\n");

        // Ambiguous replacement (occurs twice) must be rejected, not silently applied.
        let dir2 = tempfile::tempdir().unwrap();
        let dup = dir2.path().join("dup.txt");
        let dup_str = dup.to_string_lossy().into_owned();
        execute_write_file(&dup_str, "x\ny\nx\n").await;
        let e2 = execute_edit_file(&dup_str, "x", "z").await;
        assert!(e2.contains("apparaît 2 fois"), "ambiguous edit: {}", e2);

        // Missing old_string must be reported, not a silent no-op.
        let e3 = execute_edit_file(&path_str, "nope", "z").await;
        assert!(e3.contains("introuvable"), "missing edit: {}", e3);
    }

    #[test]
    fn remote_file_commands_and_decode_tests() {
        use super::{
            build_remote_read_command, build_remote_write_command, decode_remote_read_output,
            execute_remote_edit_logic,
        };

        // Read command formatting
        let read_cmd = build_remote_read_command("/etc/nginx/nginx.conf");
        assert!(read_cmd.contains("base64 -w 0 \"/etc/nginx/nginx.conf\""));
        assert!(read_cmd.contains("cat \"/etc/nginx/nginx.conf\""));

        // Write command formatting without sudo
        let write_user = build_remote_write_command("/home/user/app.py", "cHJpbnQoMSkK", false);
        assert_eq!(
            write_user,
            "printf '%s' 'cHJpbnQoMSkK' | base64 -d > \"/home/user/app.py\""
        );

        // Write command formatting with sudo
        let write_sudo =
            build_remote_write_command("/etc/hosts", "MTI3LjAuMC4xIGxvY2FsaG9zdAo=", true);
        assert_eq!(
            write_sudo,
            "printf '%s' 'MTI3LjAuMC4xIGxvY2FsaG9zdAo=' | base64 -d | sudo tee \"/etc/hosts\" > /dev/null"
        );

        // Decode valid base64 output
        let decoded = decode_remote_read_output("aGVsbG8gd29ybGQ=", "/tmp/test.txt");
        assert_eq!(decoded, "hello world");

        // Decode empty output
        let empty = decode_remote_read_output("", "/tmp/empty.txt");
        assert_eq!(empty, "(Fichier vide)");

        // Decode shell error
        let err =
            decode_remote_read_output("cat: /etc/fake: No such file or directory", "/etc/fake");
        assert!(err.contains("Erreur de lecture distante"));

        // Remote edit logic
        let orig = "server {\n    listen 80;\n    server_name example.com;\n}\n";
        let edited = execute_remote_edit_logic(
            orig,
            "/etc/nginx/nginx.conf",
            "listen 80;",
            "listen 443 ssl;",
        )
        .unwrap();
        assert!(edited.contains("listen 443 ssl;"));
        assert!(!edited.contains("listen 80;"));

        // Remote edit logic - not found
        let err_missing =
            execute_remote_edit_logic(orig, "/etc/nginx/nginx.conf", "listen 8080;", "listen 443;");
        assert!(err_missing.is_err());
        assert!(err_missing.unwrap_err().contains("introuvable"));

        // Remote edit logic - ambiguous
        let orig_dup = "foo\nbar\nfoo\n";
        let err_dup = execute_remote_edit_logic(orig_dup, "test.txt", "foo", "baz");
        assert!(err_dup.is_err());
        assert!(err_dup.unwrap_err().contains("apparaît 2 fois"));
    }

    #[test]
    fn rejects_inline_tool_mention_without_block() {
        let text = "* Il doit émettre directement un bloc ```tool:run_command (ex: `systemctl --failed` ou `systemctl --user --failed`).\n* Suite du texte.";
        assert_eq!(parse_tool_call(text), None);
    }

    #[test]
    fn rejects_unclosed_tool_run_command() {
        let text = "```tool:run_command\nsystemctl --failed\nEt voici la suite de mes explications sans fermeture de bloc...";
        assert_eq!(parse_tool_call(text), None);
    }

    #[test]
    fn rejects_placeholder_commands() {
        assert_eq!(parse_tool_call("```tool:run_command\n...\n```"), None);
        assert_eq!(parse_tool_call("```tool:run_command\n<command>\n```"), None);
        assert_eq!(parse_tool_call("```tool:run_command\n<unit>\n```"), None);
        assert_eq!(parse_tool_call("```tool:run_command\ncommande\n```"), None);
        assert_eq!(parse_tool_call("```tool:run_command\ncmd\n```"), None);
    }

    #[test]
    fn parses_valid_block_preceded_by_inline_mention() {
        let text = "N'utilise pas ```tool:run_command``` dans les commentaires. Voici la commande :\n\n```tool:run_command\nuptime\n```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("uptime".to_string()))
        );
    }

    #[test]
    fn parses_indented_fenced_tool_block() {
        let text = "Exemple :\n   ```tool:run_command\n   df -h\n   ```";
        assert_eq!(
            parse_tool_call(text),
            Some(ToolInvocation::RunCommand("df -h".to_string()))
        );
    }

    #[test]
    fn rejects_unclosed_html_style_tag_mention() {
        let text = "N'utilisez pas de balise <tool:run_command> dans votre texte explicatif.";
        assert_eq!(parse_tool_call(text), None);
    }
}
