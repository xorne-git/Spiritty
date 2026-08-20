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
            ActiveSession::Container { runtime, container_id } => {
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

    // Fallback: scan /proc for any process with PPID == root_pid
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

    if let Some(&last_child) = direct_children.last() {
        return find_foreground_leaf_pid(last_child).or(Some(last_child));
    }

    Some(root_pid)
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
        if matches!(arg.as_str(), "-i" | "-F" | "-o" | "-c" | "-b" | "-E" | "-J" | "-W" | "-w" | "-B" | "-S") {
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
}
