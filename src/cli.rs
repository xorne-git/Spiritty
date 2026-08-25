use anyhow::Result;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub continue_last_session: bool,
    pub session_id: Option<String>,
    pub initial_prompt: Option<String>,
    pub ssh_target: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub auto_approve: Option<String>,
    pub list_sessions: bool,
    pub show_help: bool,
    pub show_version: bool,
    /// Debug mode: show raw tool-result blocks (`[RÉSULTAT…]`) and extra diagnostics in the UI.
    pub debug: bool,
}

impl CliOptions {
    /// Returns the next token as a value only if it is not itself a flag
    /// (prevents `spiritty -s -p foo` from consuming `-p` as the session id).
    fn take_value(raw_args: &[String], i: &mut usize) -> Option<String> {
        if *i + 1 < raw_args.len() && !raw_args[*i + 1].starts_with('-') {
            *i += 1;
            Some(raw_args[*i].clone())
        } else {
            None
        }
    }

    pub fn parse_from_args<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let raw_args: Vec<String> = args.into_iter().map(|a| a.into()).collect();
        let mut opts = Self::default();
        let mut positional = Vec::new();

        let mut i = 1; // skip binary name
        while i < raw_args.len() {
            let arg = &raw_args[i];
            match arg.as_str() {
                "-h" | "--help" => {
                    opts.show_help = true;
                }
                "-v" | "-V" | "--version" => {
                    opts.show_version = true;
                }
                "-d" | "--debug" => {
                    opts.debug = true;
                }
                "-c" | "--continue" => {
                    opts.continue_last_session = true;
                }
                "-s" | "--session" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.session_id = Some(v);
                    }
                }
                "-p" | "--prompt" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.initial_prompt = Some(v);
                    }
                }
                "-S" | "--ssh" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.ssh_target = Some(v);
                    }
                }
                "-m" | "--model" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.model = Some(v);
                    }
                }
                "--provider" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.provider = Some(v);
                    }
                }
                "--auto-approve" => {
                    if let Some(v) = Self::take_value(&raw_args, &mut i) {
                        opts.auto_approve = Some(v);
                    }
                }
                "--safe" => {
                    opts.auto_approve = Some("safe".to_string());
                }
                "--yolo" => {
                    opts.auto_approve = Some("yolo".to_string());
                }
                "-l" | "--list-sessions" => {
                    opts.list_sessions = true;
                }
                _ => {
                    if arg.starts_with("--session=") {
                        opts.session_id = Some(arg.trim_start_matches("--session=").to_string());
                    } else if arg.starts_with("-s=") {
                        opts.session_id = Some(arg.trim_start_matches("-s=").to_string());
                    } else if arg.starts_with("--prompt=") {
                        opts.initial_prompt = Some(arg.trim_start_matches("--prompt=").to_string());
                    } else if arg.starts_with("-p=") {
                        opts.initial_prompt = Some(arg.trim_start_matches("-p=").to_string());
                    } else if arg.starts_with("--ssh=") {
                        opts.ssh_target = Some(arg.trim_start_matches("--ssh=").to_string());
                    } else if arg.starts_with("-S=") {
                        opts.ssh_target = Some(arg.trim_start_matches("-S=").to_string());
                    } else if arg.starts_with("--model=") {
                        opts.model = Some(arg.trim_start_matches("--model=").to_string());
                    } else if arg.starts_with("-m=") {
                        opts.model = Some(arg.trim_start_matches("-m=").to_string());
                    } else if arg.starts_with("--provider=") {
                        opts.provider = Some(arg.trim_start_matches("--provider=").to_string());
                    } else if arg.starts_with("--auto-approve=") {
                        opts.auto_approve = Some(arg.trim_start_matches("--auto-approve=").to_string());
                    } else if !arg.starts_with('-') {
                        positional.push(arg.clone());
                    }
                }
            }
            i += 1;
        }

        if opts.initial_prompt.is_none() && !positional.is_empty() {
            opts.initial_prompt = Some(positional.join(" "));
        }

        opts
    }

    pub fn print_help() {
        let version = env!("CARGO_PKG_VERSION");
        println!(
            r#"👻 Spiritty v{version}
AI-powered split-screen terminal companion for Sysadmins & DevOps

USAGE:
    spiritty [OPTIONS] [PROMPT]

ARGUMENTS:
    [PROMPT]                  Initial question or task for the AI agent

OPTIONS:
    -c, --continue            Resume the most recent chat session
    -s, --session <ID>        Resume a specific session by ID or title prefix
    -p, --prompt <TEXT>       Send an initial prompt to the AI agent
    -S, --ssh <TARGET>        Automatically connect to an SSH target on launch (e.g. user@host:22)
    -m, --model <NAME>        Override the AI model (e.g. qwen2.5-coder:7b, claude-3-7-sonnet)
        --provider <NAME>     Override the LLM provider (ollama, gemini, anthropic, openai, etc.)
        --safe                Start in Safe mode (auto-approve read-only inspection commands)
        --yolo                Start in YOLO mode (auto-execute all suggested commands)
        --auto-approve <LVL>  Set auto-approve policy (off, safe, sudo, yolo)
    -l, --list-sessions       List all saved sessions and exit
    -d, --debug               Debug mode (show raw tool-result [RÉSULTAT…] blocks in the chat)
    -v, --version             Display version information and exit
    -h, --help                Display this help message and exit

KEYBOARD SHORTCUTS (TUI):
    Ctrl + Space / Shift + Tab   Switch focus between Chat and Terminal
    Alt + 1..9 / AZERTY          Execute proposed command card in live shell
    Alt + D                      Proactively diagnose last command failure
    Alt + X / Alt + C / Esc      Dismiss proactive error diagnosis card
    Ctrl + B                     SSH Server Bookmarks & Quick-Connect (with [S] to bookmark active server)
    Ctrl + H                     Session History & Switcher
    Ctrl + N                     New Chat Session
    Ctrl + E                     Export Session to Markdown Report (interactive destination)
    Ctrl + F                     Search in Chat History
    F1                           Interactive Help & Shortcut Cheatsheet
    Ctrl + Q                     Quit Spiritty
"#
        );
    }

    pub fn print_version() {
        println!("Spiritty v{}", env!("CARGO_PKG_VERSION"));
    }

    pub fn print_sessions_list() -> Result<()> {
        let headers = crate::session::SessionStorage::list_sessions()?;
        if headers.is_empty() {
            println!("Aucune session enregistrée dans ~/.config/spiritty/sessions/.");
            return Ok(());
        }

        println!("📜 Sessions enregistrées ({} au total) :\n", headers.len());
        println!("{:<24}  {:<18}  {:<32}  TITRE", "ID SESSION", "DERNIÈRE MAJ", "MODÈLE / PROVIDER");
        println!("{}", "─".repeat(95));

        for h in headers {
            let prov_model = format!("{} ({})", h.model, h.provider);
            println!("{:<24}  {:<18}  {:<32}  {}", h.id, h.updated_at, prov_model, h.title);
        }
        println!("\nPour reprendre une session : spiritty -s <ID>");
        Ok(())
    }
}
