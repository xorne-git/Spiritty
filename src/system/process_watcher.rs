use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveSession {
    Local {
        foreground_process: Option<String>,
    },
    Ssh {
        target: String,
        user: Option<String>,
        host: String,
        port: Option<u16>,
    },
    Container {
        runtime: String,
        container_id: String,
    },
}

impl ActiveSession {
    pub fn is_ssh(&self) -> bool {
        matches!(self, ActiveSession::Ssh { .. })
    }

    pub fn display_label(&self) -> String {
        match self {
            ActiveSession::Local { foreground_process } => {
                if let Some(proc) = foreground_process {
                    if proc != "fish" && proc != "bash" && proc != "zsh" && proc != "sh" {
                        return format!("Local ({})", proc);
                    }
                }
                "Local".to_string()
            }
            ActiveSession::Ssh { target, .. } => {
                format!("SSH: {}", target)
            }
            ActiveSession::Container {
                runtime,
                container_id,
            } => {
                format!("{}: {}", runtime, container_id)
            }
        }
    }

    pub fn ssh_target(&self) -> Option<&str> {
        match self {
            ActiveSession::Ssh { target, .. } => Some(target.as_str()),
            _ => None,
        }
    }
}

/// Detects the active foreground session under the PTY shell process
pub fn detect_active_session(pty_child_pid: u32) -> ActiveSession {
    #[cfg(target_os = "linux")]
    {
        if let Some(leaf_pid) = find_foreground_leaf_pid(pty_child_pid) {
            if let Ok(cmdline_raw) = fs::read(format!("/proc/{}/cmdline", leaf_pid)) {
                let args: Vec<String> = cmdline_raw
                    .split(|&b| b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .collect();

                if let Some(session) = parse_session_from_cmdline(&args) {
                    return session;
                }
            }

            // If not SSH/Container, capture foreground process name
            let comm = fs::read_to_string(format!("/proc/{}/comm", leaf_pid))
                .map(|s| s.trim().to_string())
                .ok();

            return ActiveSession::Local {
                foreground_process: comm,
            };
        }
    }

    let _ = pty_child_pid;
    ActiveSession::Local {
        foreground_process: None,
    }
}

/// Recursively find the deepest active child process of the shell
#[cfg(target_os = "linux")]
fn find_foreground_leaf_pid(root_pid: u32) -> Option<u32> {
    let children_path = format!("/proc/{}/task/{}/children", root_pid, root_pid);
    if let Ok(children_str) = fs::read_to_string(&children_path) {
        let pids: Vec<u32> = children_str
            .split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect();

        if let Some(&last_child) = pids.last() {
            // Check if this child has further grandchildren
            return find_foreground_leaf_pid(last_child).or(Some(last_child));
        }
    }

    // Fallback: scan /proc for any process with PPID == root_pid.
    //
    // This full-directory scan runs on the UI event thread (called from `on_tick` twice
    // per poll), and the kernel's `children` file can transiently disappear when a
    // foreground process is exiting — which would previously trigger it constantly.
    // Memoize per root pid with a short TTL: shell-tree topology simply does not change
    // fast enough to justify re-scanning every ~360 ms, and the children-file fast path
    // above still handles the common case instantly.
    const FALLBACK_TTL: std::time::Duration = std::time::Duration::from_millis(1500);
    if let Ok(cached) = fallback_scan_cache().lock() {
        if let Some(entry) = cached.as_ref() {
            if entry.root == root_pid && entry.at.elapsed() < FALLBACK_TTL {
                return entry.result;
            }
        }
    }

    let mut direct_children = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if let Ok(pid) = file_name.parse::<u32>() {
                    if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
                        // format: pid (comm) state ppid ...
                        if let Some(ppid_str) = stat.split(')').nth(1) {
                            let parts: Vec<&str> = ppid_str.split_whitespace().collect();
                            if parts.len() >= 2 && parts[1].parse::<u32>().ok() == Some(root_pid) {
                                direct_children.push(pid);
                            }
                        }
                    }
                }
            }
        }
    }

    let fallback_result = if let Some(&last_child) = direct_children.last() {
        find_foreground_leaf_pid(last_child).or(Some(last_child))
    } else {
        Some(root_pid)
    };

    if let Ok(mut guard) = fallback_scan_cache().lock() {
        *guard = Some(FallbackScanEntry {
            at: std::time::Instant::now(),
            root: root_pid,
            result: fallback_result,
        });
    }
    fallback_result
}

struct FallbackScanEntry {
    at: std::time::Instant,
    root: u32,
    result: Option<u32>,
}

fn fallback_scan_cache() -> &'static std::sync::Mutex<Option<FallbackScanEntry>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<FallbackScanEntry>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Detects current working directory of the process
pub fn detect_current_working_dir(pty_child_pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let target_pid = find_foreground_leaf_pid(pty_child_pid).unwrap_or(pty_child_pid);
        if let Ok(link) = fs::read_link(format!("/proc/{}/cwd", target_pid)) {
            let path_str = link.to_string_lossy().to_string();
            return Some(format_compact_path(&path_str));
        }
    }
    let _ = pty_child_pid;
    None
}

/// Formats a path by replacing $HOME with ~
pub fn format_compact_path(raw_path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if raw_path == home {
            return "~".to_string();
        } else if let Some(rest) = raw_path.strip_prefix(&home) {
            if rest.starts_with('/') {
                return format!("~{}", rest);
            }
        }
    }
    raw_path.to_string()
}

/// Detects the active Git branch if the directory is inside a Git repository
pub fn detect_git_branch(dir: &str) -> Option<String> {
    let current_path = if dir.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            std::path::PathBuf::from(dir.replacen('~', &home, 1))
        } else {
            std::path::PathBuf::from(dir)
        }
    } else {
        std::path::PathBuf::from(dir)
    };

    let mut path = current_path;
    // Traverse upwards up to 5 levels to find .git directory
    for _ in 0..5 {
        let git_dir = path.join(".git");
        if git_dir.is_dir() {
            let head_file = git_dir.join("HEAD");
            if let Ok(content) = fs::read_to_string(head_file) {
                let trimmed = content.trim();
                if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                } else if trimmed.len() >= 7 {
                    return Some(trimmed[..7].to_string());
                }
            }
            break;
        } else if git_dir.is_file() {
            // Worktree or submodule: "gitdir: <path>"
            if let Ok(content) = fs::read_to_string(&git_dir) {
                if let Some(git_ref_path) = content.trim().strip_prefix("gitdir:") {
                    let ref_str = git_ref_path.trim();
                    let full_git_ref = if std::path::Path::new(ref_str).is_absolute() {
                        std::path::PathBuf::from(ref_str)
                    } else {
                        path.join(ref_str)
                    };
                    if let Ok(head_content) = fs::read_to_string(full_git_ref.join("HEAD")) {
                        let trimmed = head_content.trim();
                        if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                            return Some(branch.to_string());
                        } else if trimmed.len() >= 7 {
                            return Some(trimmed[..7].to_string());
                        }
                    }
                }
            }
            break;
        }

        if !path.pop() {
            break;
        }
    }

    None
}

/// Parses commandline arguments of a process to detect SSH or Container sessions
pub fn parse_session_from_cmdline(args: &[String]) -> Option<ActiveSession> {
    if args.is_empty() {
        return None;
    }

    let bin_path = &args[0];
    let bin_name = Path::new(bin_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(bin_path);

    // 1. Detect SSH (ssh, sftp, scp)
    if bin_name == "ssh" || bin_name == "sftp" {
        return parse_ssh_args(&args[1..]);
    }

    // 2. Detect Mosh
    if bin_name == "mosh-client" || bin_name == "mosh" {
        return parse_ssh_args(&args[1..]);
    }

    // 3. Detect Docker / Podman exec or attach
    if (bin_name == "docker" || bin_name == "podman") && args.len() >= 3 {
        let subcmd = &args[1];
        if subcmd == "exec" || subcmd == "attach" || subcmd == "run" {
            let mut container_id = None;
            for arg in &args[2..] {
                if !arg.starts_with('-') {
                    container_id = Some(arg.clone());
                    break;
                }
            }
            if let Some(cid) = container_id {
                return Some(ActiveSession::Container {
                    runtime: bin_name.to_string(),
                    container_id: cid,
                });
            }
        }
    }

    None
}

/// Parses SSH flags and extracts the target host/user/port
fn parse_ssh_args(args: &[String]) -> Option<ActiveSession> {
    let mut explicit_user: Option<String> = None;
    let mut explicit_port: Option<u16> = None;
    let mut target_arg: Option<String> = None;

    let mut skip_next = false;
    for (idx, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        if arg == "-l" && idx + 1 < args.len() {
            explicit_user = Some(args[idx + 1].clone());
            skip_next = true;
            continue;
        }

        if arg == "-p" && idx + 1 < args.len() {
            explicit_port = args[idx + 1].parse::<u16>().ok();
            skip_next = true;
            continue;
        }

        // Flags that take an argument
        if matches!(
            arg.as_str(),
            "-i" | "-F" | "-o" | "-c" | "-b" | "-E" | "-J" | "-W" | "-w" | "-B" | "-S"
        ) {
            skip_next = true;
            continue;
        }

        if arg.starts_with('-') {
            // Other boolean flags (e.g. -v, -X, -Y, -A, -C, -N, -f, -q, -t, -T)
            continue;
        }

        // First non-flag argument is the target (e.g. user@host or host)
        if target_arg.is_none() {
            target_arg = Some(arg.clone());
            break;
        }
    }

    let raw_target = target_arg?;
    let (user, host) = if let Some(pos) = raw_target.find('@') {
        let (u, h) = raw_target.split_at(pos);
        (Some(u.to_string()), h[1..].to_string())
    } else {
        (explicit_user, raw_target.clone())
    };

    // A valid SSH target is a hostname, IPv4, or bracketed IPv6 — never a bare number
    // or a pure flag value. The detection walks the PTY process tree; a foreground leaf
    // can transiently be a process whose argv[0] resolves to `ssh` but whose args are not
    // a real target (e.g. a numeric leftover, a reaper, a stray `ssh N`). Reject those so
    // a Local session is never mislabelled as a bogus `Ssh { target: "30", host: "30" }`.
    if !is_valid_ssh_host(&host) {
        return None;
    }

    let target_display = match (&user, explicit_port) {
        (Some(u), Some(p)) => format!("{}@{}:{}", u, host, p),
        (Some(u), None) => format!("{}@{}", u, host),
        (None, Some(p)) => format!("{}:{}", host, p),
        (None, None) => host.clone(),
    };

    Some(ActiveSession::Ssh {
        target: target_display,
        user,
        host,
        port: explicit_port,
    })
}

/// True when `host` looks like a usable SSH destination: a hostname (letters/digits/dots
/// and `-`/`_`, not purely numeric), an IPv4 address, or a bracketed IPv6. Rejects bare
/// integers, empty strings, and flag-value leftovers so the detector never fabricates a
/// fake `Ssh` session from an unrelated process.
fn is_valid_ssh_host(host: &str) -> bool {
    let h = host.trim();
    if h.is_empty() {
        return false;
    }
    // Bracketed IPv6, e.g. `[::1]`.
    if h.starts_with('[') && h.ends_with(']') && h.len() > 2 {
        return h[1..h.len() - 1].contains(':');
    }
    // Bare IPv6 containing colons (e.g. `::1`).
    if h.contains(':') {
        return true;
    }
    // Purely numeric (a bare port value, a sleep duration, a PID…) is never a host.
    if h.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // Hostname or IPv4: at least one alphanumeric, and only hostname-safe characters.
    h.chars().any(|c| c.is_ascii_alphanumeric())
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_commands() {
        let cmd1 = vec!["ssh".to_string(), "root@vps-01.com".to_string()];
        let res1 = parse_session_from_cmdline(&cmd1);
        assert_eq!(
            res1,
            Some(ActiveSession::Ssh {
                target: "root@vps-01.com".to_string(),
                user: Some("root".to_string()),
                host: "vps-01.com".to_string(),
                port: None,
            })
        );

        let cmd2 = vec![
            "ssh".to_string(),
            "-p".to_string(),
            "2222".to_string(),
            "-i".to_string(),
            "~/.ssh/id_ed25519".to_string(),
            "admin@192.168.1.50".to_string(),
        ];
        let res2 = parse_session_from_cmdline(&cmd2);
        assert_eq!(
            res2,
            Some(ActiveSession::Ssh {
                target: "admin@192.168.1.50:2222".to_string(),
                user: Some("admin".to_string()),
                host: "192.168.1.50".to_string(),
                port: Some(2222),
            })
        );

        let cmd3 = vec![
            "/usr/bin/ssh".to_string(),
            "-l".to_string(),
            "ubuntu".to_string(),
            "aws-ec2-instance".to_string(),
        ];
        let res3 = parse_session_from_cmdline(&cmd3);
        assert_eq!(
            res3,
            Some(ActiveSession::Ssh {
                target: "ubuntu@aws-ec2-instance".to_string(),
                user: Some("ubuntu".to_string()),
                host: "aws-ec2-instance".to_string(),
                port: None,
            })
        );
    }

    #[test]
    fn test_parse_docker_exec() {
        let cmd = vec![
            "docker".to_string(),
            "exec".to_string(),
            "-it".to_string(),
            "my-nginx-container".to_string(),
            "bash".to_string(),
        ];
        let res = parse_session_from_cmdline(&cmd);
        assert_eq!(
            res,
            Some(ActiveSession::Container {
                runtime: "docker".to_string(),
                container_id: "my-nginx-container".to_string(),
            })
        );
    }

    #[test]
    fn test_ssh_without_valid_target_is_rejected() {
        // A foreground leaf whose argv[0] resolves to `ssh` but carries a non-target
        // argument must NOT forge a fake Ssh session (e.g. `ssh 30` from a leaked numeric
        // arg, or `ssh -N` with no destination). Otherwise a Local session gets mislabelled.
        assert_eq!(
            parse_session_from_cmdline(&["ssh".to_string(), "30".to_string()]),
            None
        );
        assert_eq!(
            parse_session_from_cmdline(&["ssh".to_string(), "-N".to_string()]),
            None
        );
        assert_eq!(parse_session_from_cmdline(&["ssh".to_string()]), None);
        // A numeric-looking host is also rejected.
        assert_eq!(
            parse_session_from_cmdline(&["ssh".to_string(), "1234".to_string()]),
            None
        );
    }

    #[test]
    fn test_ssh_with_valid_targets_is_accepted() {
        let res = parse_session_from_cmdline(&["ssh".to_string(), "vps-prod".to_string()]);
        assert!(matches!(res, Some(ActiveSession::Ssh { ref host, .. }) if host == "vps-prod"));

        let res =
            parse_session_from_cmdline(&["ssh".to_string(), "root@vps.prod.internal".to_string()]);
        assert!(
            matches!(res, Some(ActiveSession::Ssh { ref host, .. }) if host == "vps.prod.internal")
        );

        let res = parse_session_from_cmdline(&["ssh".to_string(), "10.0.0.8".to_string()]);
        assert!(matches!(res, Some(ActiveSession::Ssh { ref host, .. }) if host == "10.0.0.8"));
    }
}
