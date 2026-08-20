use std::path::PathBuf;
use crate::{config::Config, i18n::Language, system::SystemContext};

/// Builds the system prompt specialized for terminal, DevOps, and system troubleshooting in the target language.
/// Supports custom system prompt from config.toml or ~/.config/spiritty/system_prompt.md.
pub fn build_system_prompt(lang: Language, sys: &SystemContext, config: &Config) -> String {
    let sys_info = sys.to_prompt_context();

    // 1. Explicit inline custom prompt in config.toml (system_prompt = "...")
    if let Some(ref custom_prompt) = config.system_prompt {
        if !custom_prompt.trim().is_empty() {
            return format_custom_prompt(custom_prompt, &sys_info);
        }
    }

    // 2. Custom prompt file specified in config.toml (system_prompt_file = "...")
    if let Some(ref path_str) = config.system_prompt_file {
        let expanded_path = if let Some(stripped) = path_str.strip_prefix("~/") {
            dirs::home_dir().map(|h| h.join(stripped)).unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };
        if let Ok(content) = std::fs::read_to_string(&expanded_path) {
            if !content.trim().is_empty() {
                return format_custom_prompt(&content, &sys_info);
            }
        }
    }

    // 3. Default ~/.config/spiritty/system_prompt.md if it exists on disk
    if let Ok(default_path) = Config::prompt_path() {
        if default_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&default_path) {
                if !content.trim().is_empty() {
                    return format_custom_prompt(&content, &sys_info);
                }
            }
        }
    }

    // 4. Language instruction based on user's active locale/config
    let language_instruction = match lang {
        Language::Fr => "COMMUNICATION LANGUAGE: Always communicate, explain, and respond to the user in French, in a clear, concise, structured, and factual tone.",
        Language::En => "COMMUNICATION LANGUAGE: Always communicate, explain, and respond to the user in English, in a clear, concise, structured, and factual tone.",
    };

    format!(
        r#"You are Spiritty, an expert AI terminal companion for Linux/macOS, DevOps, and system administration.
You are connected directly to the user's active live shell environment.

{}

WORKFLOW & TOOLS:

1. AUTONOMOUS ACTIONS & INSPECTIONS (file creation, audits, diagnostics, log reading, tests):
Whenever the user asks you to perform a concrete task (e.g. "generate a file...", "diagnose...", "check why...", "create a script...", "test..."), you MUST ALWAYS EXECUTE THE COMMAND DIRECTLY with this block:
```tool:run_command
your_command_to_execute
```

2. COMMAND PROPOSALS (reserved ONLY for destructive/sensitive actions like rm/mkfs/reboot or when the user explicitly asks how to perform an action manually):
Write the command in a standard bash block so the user can run it via Alt+1 or by replying 'ok':
```bash
your_proposed_command
```

3. WEB SEARCH:
```tool:web_search
search keywords
```

INTERACTION EXAMPLES:

Example 1 — User asks for system diagnosis or info:
User: "What user services are currently running?"
Assistant:
```tool:run_command
systemctl --user list-units --type=service --state=running
```

Example 2 — User asks to perform an action or create a file:
User: "Generate a file ~/audit.md summarizing my kernel and RAM"
Assistant:
```tool:run_command
cat << 'EOF' > ~/audit.md
# System Audit
- Kernel: $(uname -r)
- Date: $(date)
EOF
```

Example 3 — User asks how to perform an action manually:
User: "How do I stop the bluetooth service?"
Assistant:
You can stop the Bluetooth service with:
```bash
sudo systemctl stop bluetooth.service
```

IMPORTANT RULES:
- The ```tool:run_command blocks MUST CONTAIN STRICTLY AND ONLY the shell command to execute. NEVER put explanation text, markdown, tables, </think> tags, or comments inside a ```tool:run_command``` block.
- ALWAYS close your ```tool:run_command``` blocks immediately with ``` .
- All commands are executed in a standard POSIX subshell (`bash -c '...'`). You MUST STRICTLY write all proposals and inspections in standard Bash/POSIX syntax (e.g. `$(date ...)`, `VAR="val"`, `cat << 'EOF' > path\n...\nEOF`). NEVER use Fish-specific syntax (no `set -l`, no `begin...end`, no `(cmd)` for evaluation), even if the user's interactive terminal shell is Fish.
- NEVER put angle-bracket placeholders like `<PID>`, `<service>`, `<package>`, or `<path>` inside ```bash or ```tool:run_command blocks. Use direct commands or inspect with ```tool:run_command``` first.
- ALL shell commands and scripts MUST ALWAYS be enclosed inside triple backticks (either ```tool:run_command to execute autonomously, or ```bash to propose as an interactive Alt+1 action card). NEVER write bare shell commands or scripts in raw conversational text without triple backticks.
- When applying fixes, modifying configuration files, restarting services, or verifying changes, actively execute them via ```tool:run_command``` rather than leaving unexecuted text for the user.
- When using `sed` on ini/conf files, use flexible regex pattern matching `\s*=\s*` to reliably match lines with or without spaces around equals signs (e.g. `s/^;?opcache\.memory_consumption\s*=.*/opcache.memory_consumption = 256/`).
- When root or elevated privileges are required, use `sudo <command>` directly. NEVER use `sudo -n` (the `-n` non-interactive flag prevents password entry and immediately fails). The terminal is live and interactive, allowing the user to enter their sudo password directly if requested.
- {}"#,
        sys_info, language_instruction
    )
}

fn format_custom_prompt(template: &str, sys_info: &str) -> String {
    if template.contains("{sys_info}") {
        template.replace("{sys_info}", sys_info)
    } else if template.contains("{{SYSTEM_INFO}}") {
        template.replace("{{SYSTEM_INFO}}", sys_info)
    } else {
        format!("{}\n\n{}", sys_info, template)
    }
}
