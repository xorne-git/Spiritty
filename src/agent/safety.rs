use crate::config::AutoApproveLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    /// Read-only inspection commands (ps, grep, cat, systemctl status/list, journalctl, pacman -Q, etc.)
    Safe,
    /// User-level modifying commands without root privileges (mkdir, cp, git, etc.)
    Standard,
    /// Elevated, destructive, or process-terminating operations (sudo, rm, kill, systemctl stop/restart, pacman -S, etc.)
    Risky,
}

/// Classifies a shell command string into a safety risk category.
pub fn classify_command(cmd: &str) -> CommandRisk {
    let clean = cmd.trim();
    if clean.is_empty() {
        return CommandRisk::Safe;
    }

    let lower = clean.to_lowercase();

    // 1. Root / elevated execution
    if lower.starts_with("sudo ")
        || lower.contains(" sudo ")
        || lower.starts_with("doas ")
        || lower.contains(" doas ")
        || lower.starts_with("su ")
        || lower.starts_with("su -")
    {
        return CommandRisk::Risky;
    }

    // 2. Destructive or process-killing operations (direct invocation)
    let risky_binaries = [
        "rm ", "rmdir ", "unlink ", "shred ",
        "kill ", "killall ", "pkill ", "xkill ",
        "reboot", "shutdown", "poweroff", "init ",
        "dd ", "mkfs", "fdisk", "parted", "gparted",
        "chmod ", "chown ", "chgrp ",
        "iptables", "ufw", "firewalld",
    ];
    for &bin in &risky_binaries {
        if lower.starts_with(bin) || lower.contains(&format!(" {}", bin)) || lower.contains(&format!(";{}", bin)) || lower.contains(&format!("&&{}", bin)) || lower.contains(&format!("|{}", bin)) {
            return CommandRisk::Risky;
        }
    }

    // 2b. Destructive commands hidden behind a shell wrapper, command substitution or quoting
    //     (e.g. `bash -c 'rm -rf ~'`, `x=$(rm -rf /)`, `sh -c "rm ..."`, `` `rm -rf /` ``).
    const WRAPPER_MARKERS: &[&str] = &[
        "bash -c", "sh -c", "zsh -c", "dash -c", "ash -c", "ksh -c", "fish -c",
        "eval ", "$(", "`",
    ];
    for marker in WRAPPER_MARKERS {
        if let Some(pos) = lower.find(marker) {
            let after = &lower[pos + marker.len()..];
            if has_risky_token(after) {
                return CommandRisk::Risky;
            }
        }
    }

    // 3. systemd service modifications (only when the subcommand is the action, not a unit name)
    if systemctl_action_is_risky(&lower) {
        return CommandRisk::Risky;
    }

    // 4. Package manager install / remove / upgrade commands (but not read-only queries)
    if pacman_has_risky_flag(clean) {
        return CommandRisk::Risky;
    }
    let pm_risky = [
        "apt install", "apt remove", "apt purge", "apt upgrade", "apt-get",
        "dnf install", "dnf remove", "dnf upgrade",
        "zypper in", "zypper rm", "zypper dup",
        "flatpak install", "flatpak uninstall", "flatpak update",
    ];
    for pm in &pm_risky {
        if lower.contains(pm) {
            return CommandRisk::Risky;
        }
    }

    // 5. Chained safe commands (e.g. cmd1 && cmd2 || cmd3)
    if lower.contains("&&") || lower.contains(';') {
        let parts: Vec<&str> = if lower.contains("&&") {
            lower.split("&&").collect()
        } else {
            lower.split(';').collect()
        };
        let all_safe = parts.iter().all(|part| is_single_command_safe(part.trim()));
        if all_safe && !parts.is_empty() {
            return CommandRisk::Safe;
        }
    }

    if is_single_command_safe(&lower) {
        return CommandRisk::Safe;
    }

    // Default to Standard risk for regular user commands
    CommandRisk::Standard
}

/// Returns true if `s` contains a destructive/process-killing binary as a standalone token.
fn has_risky_token(s: &str) -> bool {
    const RISKY: &[&str] = &[
        "rm", "rmdir", "unlink", "shred", "kill", "killall", "pkill", "xkill",
        "dd", "mkfs", "fdisk", "parted", "gparted", "chmod", "chown", "chgrp",
        "iptables", "ufw", "firewalld", "reboot", "shutdown", "poweroff", "halt",
    ];
    s.split(|c: char| !c.is_alphanumeric()).any(|t| RISKY.contains(&t))
}

/// Detects whether a `systemctl …` command has a modifying action (stop/restart/…),
/// matching the action *token* rather than any substring in a unit name.
fn systemctl_action_is_risky(lower: &str) -> bool {
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    let mut saw_systemctl = false;
    for tok in &tokens {
        if *tok == "systemctl" {
            saw_systemctl = true;
            continue;
        }
        if saw_systemctl {
            if *tok == "--user" || *tok == "--system" || tok.starts_with("--") {
                continue;
            }
            let action = tok.trim_start_matches('-');
            return matches!(
                action,
                "stop" | "restart" | "reload" | "disable" | "mask" | "unmask"
                    | "edit" | "daemon-reload" | "poweroff" | "reboot" | "halt"
            );
        }
    }
    false
}

/// Detects risky pacman/yay/paru flags (`-S`/`-R`/`-U` and their compounds) while
/// keeping read-only query/search flags (`-Ss`, `-Si`, `-Sl`, `-Sw`, `-Q*`) safe.
fn pacman_has_risky_flag(cmd: &str) -> bool {
    let is_pacman_family = cmd
        .split_whitespace()
        .next()
        .map(|b| matches!(b, "pacman" | "yay" | "paru") || b.ends_with("/pacman") || b.ends_with("/yay") || b.ends_with("/paru"))
        .unwrap_or(false);

    if !is_pacman_family {
        return false;
    }

    for tok in cmd.split_whitespace() {
        if !tok.starts_with('-') {
            continue;
        }
        let t = tok.trim_start_matches('-');
        // Read-only flags
        if t == "Ss" || t == "Si" || t == "Sl" || t == "Sw" || t == "Sg"
            || t == "Qs" || t == "Qi" || t == "Ql" || t == "Qo" || t == "Qg"
            || t.starts_with('Q')
        {
            continue;
        }
        if t.contains('S') || t.contains('R') || t.contains('U') {
            return true;
        }
    }
    false
}

fn is_single_command_safe(lower: &str) -> bool {
    // `psql` is a database client, not the `ps` process lister.
    if lower.starts_with("psql") {
        return false;
    }

    let safe_prefixes = [
        // Systemd read-only
        "systemctl status", "systemctl --user status",
        "systemctl is-active", "systemctl --user is-active",
        "systemctl is-enabled", "systemctl --user is-enabled",
        "systemctl is-failed", "systemctl --user is-failed",
        "systemctl list-units", "systemctl --user list-units",
        "systemctl list-unit-files", "systemctl --user list-unit-files",
        "systemctl list-sockets", "systemctl --user list-sockets",
        "systemctl list-timers", "systemctl --user list-timers",
        "systemctl cat", "systemctl --user cat",
        "systemctl show", "systemctl --user show",
        // Containers (Docker / Podman) read-only
        "docker ps", "docker inspect", "docker logs", "docker stats", "docker port", "docker top", "docker version", "docker info", "docker images",
        "podman ps", "podman inspect", "podman logs", "podman stats", "podman port", "podman top", "podman version", "podman info", "podman images",
        "docker compose ps", "docker compose logs", "docker compose config", "docker compose top",
        "docker-compose ps", "docker-compose logs", "docker-compose config", "docker-compose top",
        // Logs & journal
        "journalctl",
        // Processes
        "ps ", "ps -", "ps", "pgrep", "top -b", "pstree",
        // File inspection
        "cat ", "head ", "tail ", "less ", "more ", "bat ",
        "ls ", "ls -", "ls", "dir ", "vdir ", "tree ", "find ", "fd ", "locate ", "which ", "whereis ", "type ",
        "file ", "stat ",
        // Text processing
        "grep ", "grep -", "egrep ", "fgrep ", "rg ", "ag ", "awk ", "cut ", "sort ", "uniq ", "wc ", "wc -", "diff ", "cmp ", "column ", "jq", "jq ",
        // Package queries
        "pacman -q", "pacman -qs", "pacman -qi", "pacman -ql", "pacman -qo",
        "pacman -ss", "pacman -si", "pacman -sl", "pacman -sw", "pacman -sg",
        "paru -q", "paru -ss", "paru -si", "paru -sl",
        "yay -q", "yay -ss", "yay -si", "yay -sl",
        "apt list", "dpkg -l", "dpkg -s", "rpm -qa", "dnf list", "zypper se",
        "brew list", "flatpak list", "flatpak info",
        // System & Hardware info
        "uname", "hostname", "hostnamectl", "uptime", "id", "who", "whoami", "w", "env", "printenv", "locale", "timedatectl",
        "df", "df -", "du", "du -", "free", "free -", "lsblk", "blkid", "mount", "findmnt", "lsof", "fuser",
        "dmesg", "lspci", "lsusb", "lscpu", "lshw", "inxi", "neofetch", "fastfetch",
        "glxinfo", "vulkaninfo", "nvidia-smi", "wlrctl",
        // Network queries
        "ip ", "ip -", "ifconfig", "ss ", "ss -", "netstat", "ping -c", "traceroute", "dig", "nslookup", "curl -i", "curl -s", "ethtool",
        // Git queries
        "git status", "git log", "git diff", "git branch", "git show", "git remote",
        // Safe echo / printf without redirects
        "echo ", "printf ", "echo", "printf",
    ];

    // Ignore harmless /dev/null and fd redirects when checking for file write redirects
    let stripped_redirects = lower
        .replace("2>/dev/null", "")
        .replace(">/dev/null", "")
        .replace("&>/dev/null", "")
        .replace("1>/dev/null", "")
        .replace("2>&1", "")
        .replace("1>&2", "");

    // curl/wget downloading to a file (`-o`/`--output`/`-O`) is a write operation.
    let curl_wget_write = (lower.starts_with("curl ") || lower.starts_with("wget "))
        && (lower.contains(" -o ") || lower.contains(" -o=") || lower.contains(" --output"));

    let has_file_write_redirect = stripped_redirects.contains('>')
        || stripped_redirects.contains(">>")
        || lower.contains(" | tee ")
        || curl_wget_write;

    if !has_file_write_redirect {
        for prefix in &safe_prefixes {
            if lower.starts_with(prefix) || lower == *prefix {
                return true;
            }
        }
    }
    false
}

/// Determines whether a command should be auto-approved based on current AutoApproveLevel and command risk.
pub fn should_auto_approve_command(cmd: &str, level: AutoApproveLevel) -> bool {
    let risk = classify_command(cmd);
    match level {
        AutoApproveLevel::Off => false,
        AutoApproveLevel::Safe => risk == CommandRisk::Safe,
        AutoApproveLevel::Sudo => risk == CommandRisk::Safe || risk == CommandRisk::Standard,
        AutoApproveLevel::Yolo => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_safe_commands() {
        assert_eq!(classify_command("systemctl status dms"), CommandRisk::Safe);
        assert_eq!(classify_command("systemctl --user status dms.service"), CommandRisk::Safe);
        assert_eq!(classify_command("journalctl -u dms -b -n 50"), CommandRisk::Safe);
        assert_eq!(classify_command("ps -ef | grep dms | grep -v grep"), CommandRisk::Safe);
        assert_eq!(classify_command("cat ~/.config/niri/config.kdl"), CommandRisk::Safe);
        assert_eq!(classify_command("pacman -Qs dank"), CommandRisk::Safe);
        assert_eq!(classify_command("df -h"), CommandRisk::Safe);
        assert_eq!(classify_command("systemctl --user list-units --type=service --state=running 2>/dev/null"), CommandRisk::Safe);
        assert_eq!(classify_command("glxinfo 2>&1 | grep -E OpenGL"), CommandRisk::Safe);
    }

    #[test]
    fn test_classify_risky_commands() {
        assert_eq!(classify_command("sudo systemctl restart dms"), CommandRisk::Risky);
        assert_eq!(classify_command("sudo kill -9 1234"), CommandRisk::Risky);
        assert_eq!(classify_command("pkill -f dms"), CommandRisk::Risky);
        assert_eq!(classify_command("sudo pacman -S dank"), CommandRisk::Risky);
        assert_eq!(classify_command("rm -rf ~/.cache/dms"), CommandRisk::Risky);
    }

    #[test]
    fn test_auto_approve_policies() {
        assert!(should_auto_approve_command("systemctl status dms", AutoApproveLevel::Safe));
        assert!(!should_auto_approve_command("sudo systemctl restart dms", AutoApproveLevel::Safe));

        assert!(should_auto_approve_command("systemctl status dms", AutoApproveLevel::Yolo));
        assert!(should_auto_approve_command("sudo systemctl restart dms", AutoApproveLevel::Yolo));

        assert!(!should_auto_approve_command("systemctl status dms", AutoApproveLevel::Off));
    }

    #[test]
    fn test_classify_wrapper_bypass() {
        // Destructive commands hidden behind shell wrappers / substitution must be Risky.
        assert_eq!(classify_command("bash -c 'rm -rf ~'"), CommandRisk::Risky);
        assert_eq!(classify_command("sh -c \"rm -rf /\""), CommandRisk::Risky);
        assert_eq!(classify_command("x=$(rm -rf /)"), CommandRisk::Risky);
        assert_eq!(classify_command("eval 'kill -9 1'"), CommandRisk::Risky);
        assert_eq!(classify_command("`rm -rf /tmp/x`"), CommandRisk::Risky);
    }

    #[test]
    fn test_classify_write_detection() {
        // curl/wget downloading to a file must not be auto-approved as Safe (they become Standard).
        assert_eq!(classify_command("curl -s -o /etc/passwd http://x"), CommandRisk::Standard);
        assert_eq!(classify_command("wget -O /etc/passwd http://x"), CommandRisk::Standard);
        // But a read-only curl GET remains Safe.
        assert_eq!(classify_command("curl -s https://example.com"), CommandRisk::Safe);
    }

    #[test]
    fn test_classify_readonly_not_risky() {
        // Read-only pacman search & systemctl status with "restart" in a unit name must stay Safe.
        assert_eq!(classify_command("pacman -Ss linux"), CommandRisk::Safe);
        assert_eq!(classify_command("pacman -Qs linux"), CommandRisk::Safe);
        assert_eq!(classify_command("systemctl status my-restart-unit.service"), CommandRisk::Safe);
        // psql must not be treated as the safe `ps` lister.
        assert_ne!(classify_command("psql -c 'DROP TABLE users'"), CommandRisk::Safe);
    }
}
