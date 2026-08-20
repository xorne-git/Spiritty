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

    // 1. Check for dangerous chaining or subshells containing risky elements
    let lower = clean.to_lowercase();

    // Check for root / elevated execution
    if lower.starts_with("sudo ")
        || lower.contains(" sudo ")
        || lower.starts_with("doas ")
        || lower.contains(" doas ")
        || lower.starts_with("su ")
        || lower.starts_with("su -")
    {
        return CommandRisk::Risky;
    }

    // Check for destructive or process-killing operations
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

    // Check systemd modifications
    if lower.contains("systemctl") {
        let risky_actions = [
            "stop", "restart", "reload", "disable", "mask", "unmask",
            "edit", "daemon-reload", "poweroff", "reboot", "halt",
        ];
        for action in &risky_actions {
            if lower.contains(action) {
                return CommandRisk::Risky;
            }
        }
    }

    // Check package manager install / remove / upgrade commands
    let pm_risky = [
        "pacman -s", "pacman -r", "pacman -u", "pacman -syu", "pacman -syyu",
        "paru -s", "paru -r", "paru -u", "paru -syu",
        "yay -s", "yay -r", "yay -u", "yay -syu",
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

    // 2. Check for Safe Read-Only commands
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
        // Logs & journal
        "journalctl",
        // Processes
        "ps ", "ps -", "ps", "pgrep", "top -b", "pstree",
        // File inspection
        "cat ", "head ", "tail ", "less ", "more ", "bat ",
        "ls ", "ls -", "ls", "dir ", "vdir ", "tree ", "find ", "fd ", "locate ", "which ", "whereis ", "type ",
        "file ", "stat ",
        // Text processing
        "grep ", "grep -", "egrep ", "fgrep ", "rg ", "ag ", "awk ", "cut ", "sort ", "uniq ", "wc ", "wc -", "diff ", "cmp ", "column ",
        // Package queries
        "pacman -q", "pacman -qs", "pacman -qi", "pacman -ql", "pacman -qo",
        "paru -q", "yay -q",
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
        "echo ", "printf ",
    ];

    // Ignore harmless /dev/null and fd redirects (2>/dev/null, >/dev/null, &>/dev/null, 2>&1, 1>&2) when checking for file write redirects
    let stripped_redirects = lower
        .replace("2>/dev/null", "")
        .replace(">/dev/null", "")
        .replace("&>/dev/null", "")
        .replace("1>/dev/null", "")
        .replace("2>&1", "")
        .replace("1>&2", "");

    let has_file_write_redirect = stripped_redirects.contains('>') || stripped_redirects.contains(">>") || lower.contains(" | tee ");

    if !has_file_write_redirect {
        for prefix in &safe_prefixes {
            if lower.starts_with(prefix) || lower == *prefix {
                return CommandRisk::Safe;
            }
        }
    }

    // Default to Standard risk for regular user commands
    CommandRisk::Standard
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
}
