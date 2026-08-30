use crate::config::AutoApproveLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    /// Read-only inspection commands (ps, grep, cat, systemctl status/list, journalctl, pacman -Q, etc.)
    Safe,
    /// User-level modifying commands without root privileges (mkdir, cp, git commit, cargo build, etc.)
    Standard,
    /// Elevated (sudo/doas/su) operations that are NOT destructive — e.g. `sudo cat /var/log/syslog`,
    /// `sudo ls /root`, `sudo certbot certificates`, `sudo grep …`. Auto-approved only at the
    /// explicit "Sudo" policy level; still prompts at the "Safe" level.
    Sudo,
    /// Destructive, process-terminating or system-impacting operations (rm -rf, dd, mkfs,
    /// kill, chmod/chown, firewall edits, service restarts/stops, package installs) whether
    /// elevated or not. Only auto-approved in YOLO mode.
    Risky,
}

/// Classifies a shell command string into a safety risk category.
pub fn classify_command(cmd: &str) -> CommandRisk {
    let clean = cmd.trim();
    if clean.is_empty() {
        return CommandRisk::Safe;
    }

    let lower = clean.to_lowercase();

    // 1. Destructive or process-killing operations (direct invocation)
    let risky_binaries = [
        "rm ",
        "rmdir ",
        "unlink ",
        "shred ",
        "kill ",
        "killall ",
        "pkill ",
        "xkill ",
        "reboot",
        "shutdown",
        "poweroff",
        "init ",
        "dd ",
        "mkfs",
        "fdisk",
        "parted",
        "gparted",
        "chmod ",
        "chown ",
        "chgrp ",
        "iptables",
        "ufw",
        "firewalld",
    ];
    for &bin in &risky_binaries {
        if lower.starts_with(bin)
            || lower.contains(&format!(" {}", bin))
            || lower.contains(&format!(";{}", bin))
            || lower.contains(&format!("&&{}", bin))
            || lower.contains(&format!("|{}", bin))
        {
            return CommandRisk::Risky;
        }
    }

    // 2. Destructive commands hidden behind a shell wrapper, command substitution or quoting
    //     (e.g. `bash -c 'rm -rf ~'`, `x=$(rm -rf /)`, `sh -c "rm ..."`, `` `rm -rf /` ``).
    const WRAPPER_MARKERS: &[&str] = &[
        "bash -c", "sh -c", "zsh -c", "dash -c", "ash -c", "ksh -c", "fish -c", "eval ", "$(", "`",
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
        "apt install",
        "apt remove",
        "apt purge",
        "apt upgrade",
        "apt-get",
        "dnf install",
        "dnf remove",
        "dnf upgrade",
        "zypper in",
        "zypper rm",
        "zypper dup",
        "flatpak install",
        "flatpak uninstall",
        "flatpak update",
    ];
    for pm in &pm_risky {
        if lower.contains(pm) {
            return CommandRisk::Risky;
        }
    }

    // 5. Privilege escalation with no destructive pattern matched above: the command is
    //    elevated but read-only/benign as root (`sudo cat`, `sudo ls`, `sudo grep`,
    //    `sudo certbot certificates`, `sudo systemctl status …`). This is its own class so
    //    the "Sudo" approval level can allow it while "Safe" keeps prompting for it, and
    //    destructive elevated commands were already caught by rules 1-4 and stay Risky.
    if lower.starts_with("sudo ")
        || lower.contains(" sudo ")
        || lower.starts_with("doas ")
        || lower.contains(" doas ")
        || lower.starts_with("su ")
        || lower.starts_with("su -")
    {
        return CommandRisk::Sudo;
    }

    // 6. Chained safe commands (e.g. cmd1 && cmd2 || cmd3)
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
        "rm",
        "rmdir",
        "unlink",
        "shred",
        "kill",
        "killall",
        "pkill",
        "xkill",
        "dd",
        "mkfs",
        "fdisk",
        "parted",
        "gparted",
        "chmod",
        "chown",
        "chgrp",
        "iptables",
        "ufw",
        "firewalld",
        "reboot",
        "shutdown",
        "poweroff",
        "halt",
    ];
    s.split(|c: char| !c.is_alphanumeric())
        .any(|t| RISKY.contains(&t))
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
                "stop"
                    | "restart"
                    | "reload"
                    | "disable"
                    | "mask"
                    | "unmask"
                    | "edit"
                    | "daemon-reload"
                    | "poweroff"
                    | "reboot"
                    | "halt"
            );
        }
    }
    false
}

/// Detects risky pacman/yay/paru flags (`-S`/`-R`/`-U` and their compounds) while
/// keeping read-only query/search flags (`-Ss`, `-Si`, `-Sl`, `-Sw`, `-Q*`) safe.
fn pacman_has_risky_flag(cmd: &str) -> bool {
    let mut tokens = cmd.split_whitespace();
    let mut bin = tokens.next().unwrap_or("");
    // Tolerate an elevation prefix so `sudo pacman -Syu` / `doas yay -S x` stay Risky.
    if matches!(bin, "sudo" | "doas") {
        bin = tokens.next().unwrap_or("");
    }
    let is_pacman_family = matches!(bin, "pacman" | "yay" | "paru")
        || bin.ends_with("/pacman")
        || bin.ends_with("/yay")
        || bin.ends_with("/paru");

    if !is_pacman_family {
        return false;
    }

    for tok in cmd.split_whitespace() {
        if !tok.starts_with('-') {
            continue;
        }
        let t = tok.trim_start_matches('-');
        // Read-only flags
        if t == "Ss"
            || t == "Si"
            || t == "Sl"
            || t == "Sw"
            || t == "Sg"
            || t == "Qs"
            || t == "Qi"
            || t == "Ql"
            || t == "Qo"
            || t == "Qg"
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
        "systemctl status",
        "systemctl --user status",
        "systemctl is-active",
        "systemctl --user is-active",
        "systemctl is-enabled",
        "systemctl --user is-enabled",
        "systemctl is-failed",
        "systemctl --user is-failed",
        "systemctl list-units",
        "systemctl --user list-units",
        "systemctl list-unit-files",
        "systemctl --user list-unit-files",
        "systemctl list-sockets",
        "systemctl --user list-sockets",
        "systemctl list-timers",
        "systemctl --user list-timers",
        "systemctl cat",
        "systemctl --user cat",
        "systemctl show",
        "systemctl --user show",
        // Containers (Docker / Podman) read-only
        "docker ps",
        "docker inspect",
        "docker logs",
        "docker stats",
        "docker port",
        "docker top",
        "docker version",
        "docker info",
        "docker images",
        "podman ps",
        "podman inspect",
        "podman logs",
        "podman stats",
        "podman port",
        "podman top",
        "podman version",
        "podman info",
        "podman images",
        "docker compose ps",
        "docker compose logs",
        "docker compose config",
        "docker compose top",
        "docker-compose ps",
        "docker-compose logs",
        "docker-compose config",
        "docker-compose top",
        // Logs & journal
        "journalctl",
        // Processes
        "ps ",
        "ps -",
        "ps",
        "pgrep",
        "top -b",
        "pstree",
        // File inspection
        "cat ",
        "head ",
        "tail ",
        "less ",
        "more ",
        "bat ",
        "ls ",
        "ls -",
        "ls",
        "dir ",
        "vdir ",
        "tree ",
        "find ",
        "fd ",
        "locate ",
        "which ",
        "whereis ",
        "type ",
        "file ",
        "stat ",
        // Text processing
        "grep ",
        "grep -",
        "egrep ",
        "fgrep ",
        "rg ",
        "ag ",
        "awk ",
        "cut ",
        "sort ",
        "uniq ",
        "wc ",
        "wc -",
        "diff ",
        "cmp ",
        "column ",
        "jq",
        "jq ",
        // Package queries
        "pacman -q",
        "pacman -qs",
        "pacman -qi",
        "pacman -ql",
        "pacman -qo",
        "pacman -ss",
        "pacman -si",
        "pacman -sl",
        "pacman -sw",
        "pacman -sg",
        "paru -q",
        "paru -ss",
        "paru -si",
        "paru -sl",
        "yay -q",
        "yay -ss",
        "yay -si",
        "yay -sl",
        "apt list",
        "dpkg -l",
        "dpkg -s",
        "rpm -qa",
        "dnf list",
        "zypper se",
        "brew list",
        "flatpak list",
        "flatpak info",
        // System & Hardware info
        "uname",
        "hostname",
        "hostnamectl",
        "uptime",
        "id",
        "who",
        "whoami",
        "w",
        "env",
        "printenv",
        "locale",
        "timedatectl",
        "df",
        "df -",
        "du",
        "du -",
        "free",
        "free -",
        "lsblk",
        "blkid",
        "mount",
        "findmnt",
        "lsof",
        "fuser",
        "dmesg",
        "lspci",
        "lsusb",
        "lscpu",
        "lshw",
        "inxi",
        "neofetch",
        "fastfetch",
        "glxinfo",
        "vulkaninfo",
        "nvidia-smi",
        "wlrctl",
        // Network queries
        "ip ",
        "ip -",
        "ifconfig",
        "ss ",
        "ss -",
        "netstat",
        "ping -c",
        "traceroute",
        "dig",
        "nslookup",
        "curl -i",
        "curl -s",
        "ethtool",
        // Git queries
        "git status",
        "git log",
        "git diff",
        "git branch",
        "git show",
        "git remote",
        // Safe echo / printf without redirects
        "echo ",
        "printf ",
        "echo",
        "printf",
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
        // "Sudo" level: everything up to elevated-but-benign operations. Destructive
        // commands (Risky) still require explicit consent until YOLO is enabled.
        AutoApproveLevel::Sudo => matches!(
            risk,
            CommandRisk::Safe | CommandRisk::Standard | CommandRisk::Sudo
        ),
        AutoApproveLevel::Yolo => true,
    }
}

/// Classifies a file-edit operation by target path. A write/edit that only touches the
/// user's own config/desktop files is a normal user-level change (`Standard`); anything
/// under system, root-owned or service locations (`/etc`, `/usr`, `/var`, `/root`,
/// `/boot`, systemd units…) is `Sudo` and always asks beyond the explicit Sudo level;
/// a truly sensitive path (shell rc files, ssh keys, passwd, fstab…) stays `Risky` so it
/// is only ever auto-approved in YOLO mode.
pub fn classify_file_edit(path: &str) -> CommandRisk {
    let p = path.trim();
    if p.is_empty() {
        return CommandRisk::Risky;
    }

    // Sensitive: shell auth/config, login, mounts. Conservative, never auto-approved
    // beyond YOLO.
    const SENSITIVE_MARKERS: &[&str] = &[
        "/.ssh/",
        "/.gnupg/",
        "/authorized_keys",
        "/.bashrc",
        "/.zshrc",
        "/.profile",
        "/.bash_profile",
        "/.zshenv",
        "/.zprofile",
        "/etc/passwd",
        "/etc/shadow",
        "/etc/fstab",
        "/etc/sudoers",
        "/etc/group",
        "/etc/sudoers.d/",
        "/etc/ssh/",
        "/etc/pacman.conf",
        "/etc/apt/sources",
        "/etc/dnf/",
        "/etc/zypp/",
        "/etc/mkinitcpio",
        "/etc/X11/",
        "/etc/ld.so.conf",
    ];
    if SENSITIVE_MARKERS.iter().any(|m| p.contains(m)) {
        return CommandRisk::Risky;
    }

    // System / service locations: elevated, non-destructive edits.
    const SYSTEM_PREFIXES: &[&str] = &[
        "/etc/",
        "/usr/",
        "/var/",
        "/boot/",
        "/opt/",
        "/srv/",
        "/root/",
        "/sbin/",
        "/usr/local/",
        "/run/",
    ];
    if SYSTEM_PREFIXES.iter().any(|m| p.starts_with(m)) {
        return CommandRisk::Sudo;
    }

    // Everything else (home, ~/.config, ~/.local, project files, /mnt, /media…) is a
    // regular user-level change.
    CommandRisk::Standard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_safe_commands() {
        assert_eq!(classify_command("systemctl status dms"), CommandRisk::Safe);
        assert_eq!(
            classify_command("systemctl --user status dms.service"),
            CommandRisk::Safe
        );
        assert_eq!(
            classify_command("journalctl -u dms -b -n 50"),
            CommandRisk::Safe
        );
        assert_eq!(
            classify_command("ps -ef | grep dms | grep -v grep"),
            CommandRisk::Safe
        );
        assert_eq!(
            classify_command("cat ~/.config/niri/config.kdl"),
            CommandRisk::Safe
        );
        assert_eq!(classify_command("pacman -Qs dank"), CommandRisk::Safe);
        assert_eq!(classify_command("df -h"), CommandRisk::Safe);
        assert_eq!(
            classify_command(
                "systemctl --user list-units --type=service --state=running 2>/dev/null"
            ),
            CommandRisk::Safe
        );
        assert_eq!(
            classify_command("glxinfo 2>&1 | grep -E OpenGL"),
            CommandRisk::Safe
        );
    }

    #[test]
    fn test_classify_risky_commands() {
        assert_eq!(
            classify_command("sudo systemctl restart dms"),
            CommandRisk::Risky
        );
        assert_eq!(classify_command("sudo kill -9 1234"), CommandRisk::Risky);
        assert_eq!(classify_command("pkill -f dms"), CommandRisk::Risky);
        assert_eq!(classify_command("sudo pacman -S dank"), CommandRisk::Risky);
        assert_eq!(classify_command("rm -rf ~/.cache/dms"), CommandRisk::Risky);
    }

    #[test]
    fn test_auto_approve_policies() {
        assert!(should_auto_approve_command(
            "systemctl status dms",
            AutoApproveLevel::Safe
        ));
        assert!(!should_auto_approve_command(
            "sudo systemctl restart dms",
            AutoApproveLevel::Safe
        ));

        assert!(should_auto_approve_command(
            "systemctl status dms",
            AutoApproveLevel::Yolo
        ));
        assert!(should_auto_approve_command(
            "sudo systemctl restart dms",
            AutoApproveLevel::Yolo
        ));

        assert!(!should_auto_approve_command(
            "systemctl status dms",
            AutoApproveLevel::Off
        ));
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
        assert_eq!(
            classify_command("curl -s -o /etc/passwd http://x"),
            CommandRisk::Standard
        );
        assert_eq!(
            classify_command("wget -O /etc/passwd http://x"),
            CommandRisk::Standard
        );
        // But a read-only curl GET remains Safe.
        assert_eq!(
            classify_command("curl -s https://example.com"),
            CommandRisk::Safe
        );
    }

    #[test]
    fn test_classify_readonly_not_risky() {
        // Read-only pacman search & systemctl status with "restart" in a unit name must stay Safe.
        assert_eq!(classify_command("pacman -Ss linux"), CommandRisk::Safe);
        assert_eq!(classify_command("pacman -Qs linux"), CommandRisk::Safe);
        assert_eq!(
            classify_command("systemctl status my-restart-unit.service"),
            CommandRisk::Safe
        );
        // psql must not be treated as the safe `ps` lister.
        assert_ne!(
            classify_command("psql -c 'DROP TABLE users'"),
            CommandRisk::Safe
        );
    }

    #[test]
    fn test_sudo_class_separates_elevated_readonly_from_destructive() {
        // Elevated but benign (read-only as root) -> dedicated Sudo class.
        assert_eq!(
            classify_command("sudo ls -la /etc/letsencrypt/live/"),
            CommandRisk::Sudo
        );
        assert_eq!(
            classify_command("sudo grep -rE 'SetHandler|\\.sock' /etc/apache2/sites-enabled/"),
            CommandRisk::Sudo
        );
        assert_eq!(
            classify_command("sudo cat /var/log/syslog | tail -50"),
            CommandRisk::Sudo
        );
        assert_eq!(
            classify_command("sudo certbot certificates"),
            CommandRisk::Sudo
        );
        assert_eq!(
            classify_command("sudo systemctl status apache2"),
            CommandRisk::Sudo
        );
        assert_eq!(
            classify_command("doas cat /etc/hosts.allow"),
            CommandRisk::Sudo
        );
        // Elevated read-only segment inside a large audit chain stays Sudo.
        assert_eq!(
            classify_command(
                "echo \"== SOCKETS ==\" && systemctl list-units --type=service --state=running --no-pager | grep -Ei 'php|apache'; sudo ls -la /run/php/"
            ),
            CommandRisk::Sudo
        );
        // Destructive, elevated or not: still Risky (never auto-approved below YOLO).
        assert_eq!(classify_command("sudo rm -rf /tmp/x"), CommandRisk::Risky);
        assert_eq!(
            classify_command("sudo systemctl restart apache2"),
            CommandRisk::Risky
        );
        assert_eq!(
            classify_command("sudo apt install nginx"),
            CommandRisk::Risky
        );
        assert_eq!(classify_command("sudo pacman -Syu"), CommandRisk::Risky);
        assert_eq!(
            classify_command("sudo chmod 777 /etc/passwd"),
            CommandRisk::Risky
        );
        assert_eq!(classify_command("sudo kill -9 4242"), CommandRisk::Risky);
    }

    #[test]
    fn test_auto_approve_sudo_level_matrix() {
        use crate::config::AutoApproveLevel;
        let lvl = AutoApproveLevel::Sudo;
        // Safe & Standard & Sudo are allowed…
        assert!(should_auto_approve_command("df -h", lvl));
        assert!(should_auto_approve_command("git commit -m \"msg\"", lvl));
        assert!(should_auto_approve_command("sudo ls /root", lvl));
        // …destructive ones never are.
        assert!(!should_auto_approve_command("sudo rm -rf /tmp/x", lvl));
        assert!(!should_auto_approve_command("pkill -f dms", lvl));
        // Safe level keeps prompting for anything elevated.
        assert!(!should_auto_approve_command(
            "sudo ls /root",
            AutoApproveLevel::Safe
        ));
        assert!(!should_auto_approve_command(
            "sudo ls /root",
            AutoApproveLevel::Off
        ));
    }

    #[test]
    fn test_classify_file_edit_by_path() {
        // User-level config/home files: normal change.
        assert_eq!(classify_file_edit("~/.config/niri/config.kdl"), CommandRisk::Standard);
        assert_eq!(classify_file_edit("/home/x/project/src/main.rs"), CommandRisk::Standard);
        assert_eq!(classify_file_edit("/mnt/data/backup.txt"), CommandRisk::Standard);
        // System / service locations: elevated.
        assert_eq!(classify_file_edit("/etc/systemd/system/acpi.service"), CommandRisk::Sudo);
        assert_eq!(classify_file_edit("/usr/share/applications/x.desktop"), CommandRisk::Sudo);
        assert_eq!(classify_file_edit("/var/www/site/index.html"), CommandRisk::Sudo);
        // Sensitive / conservative: never auto-approved below YOLO.
        assert_eq!(classify_file_edit("/home/x/.ssh/authorized_keys"), CommandRisk::Risky);
        assert_eq!(classify_file_edit("/home/x/.zshrc"), CommandRisk::Risky);
        assert_eq!(classify_file_edit("/etc/fstab"), CommandRisk::Risky);
        // Empty path is conservative.
        assert_eq!(classify_file_edit("   "), CommandRisk::Risky);
    }
}
