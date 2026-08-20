pub mod clipboard;
pub mod hosts;
pub mod process_watcher;

pub use hosts::{HostProfile, HostsStore};
pub use process_watcher::{detect_active_session, ActiveSession};

use std::env;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SystemContext {
    pub os_name: String,
    pub distro: String,
    pub kernel: String,
    pub shell: String,
    pub terminal_emulator: String,
    pub package_managers: Vec<String>,
    pub desktop_env: Option<String>,
    pub active_session: ActiveSession,
    pub active_remote_profile: Option<HostProfile>,
}

impl SystemContext {
    pub fn detect() -> Self {
        let os_name = env::consts::OS.to_string();
        let mut distro = "Linux".to_string();
        let mut package_managers = Vec::new();

        // 1. Read /etc/os-release on Linux
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    distro = line.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string();
                    break;
                } else if line.starts_with("NAME=") && distro == "Linux" {
                    distro = line.trim_start_matches("NAME=").trim_matches('"').to_string();
                }
            }
        }

        // 2. Detect package managers in PATH
        for pm in &["pacman", "paru", "yay", "apt", "dnf", "zypper", "brew", "flatpak", "snap", "nix"] {
            if which(pm) {
                package_managers.push(pm.to_string());
            }
        }

        // 3. Detect Kernel
        let kernel = match Command::new("uname").arg("-r").output() {
            Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
            Err(_) => "unknown".to_string(),
        };

        // 4. Shell
        let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());

        // 5. Detect Terminal Emulator and Version
        let terminal_emulator = detect_terminal_emulator(&shell);

        // 6. Desktop / Wayland / X11
        let desktop_env = env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| env::var("DESKTOP_SESSION"))
            .or_else(|_| env::var("WAYLAND_DISPLAY").map(|_| "Wayland".to_string()))
            .ok();

        Self {
            os_name,
            distro,
            kernel,
            shell,
            terminal_emulator,
            package_managers,
            desktop_env,
            active_session: ActiveSession::Local { foreground_process: None },
            active_remote_profile: None,
        }
    }

    pub fn to_prompt_context(&self) -> String {
        if let Some(ref remote_profile) = self.active_remote_profile {
            return remote_profile.to_prompt_context();
        }

        if let ActiveSession::Ssh { ref target, .. } = self.active_session {
            return format!(
                "Active SSH Remote Environment (Target: {}):\n- The user is currently connected via SSH to remote server '{}'.\n- Full host profile not scanned yet. Use universal POSIX commands and suggest checking OS details (e.g. cat /etc/os-release or uname -a) if needed.\n- IMPORTANT: All command proposals and inspections execute on THIS REMOTE SERVER via SSH.",
                target, target
            );
        }

        let pms = if self.package_managers.is_empty() {
            "none detected".to_string()
        } else {
            self.package_managers.join(", ")
        };

        let wm = self.desktop_env.as_deref().unwrap_or("Terminal/Console");

        format!(
            "User's Detected System Environment (Local Machine):\n- OS / Distribution: {}\n- Kernel: {}\n- Interactive Terminal Shell: {} (IMPORTANT: the command execution subshell is standard Bash/POSIX. All proposed commands and inspections must be strictly valid Bash syntax, never Fish syntax).\n- Available Package Managers: {} (NEVER use unlisted package managers like dpkg/rpm/apt if not present!)\n- Desktop Environment / Window Manager: {}\n- Init & Services: systemd (always check both 'systemctl' and 'systemctl --user' for user-level services like dms, pipewire, etc.)",
            self.distro, self.kernel, self.shell, pms, wm
        )
    }
}

fn which(binary: &str) -> bool {
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let p = dir.join(binary);
            if p.is_file() {
                return true;
            }
        }
    }
    false
}

fn detect_terminal_emulator(shell: &str) -> String {
    // 1. Check Ghostty
    if env::var("GHOSTTY_BIN_DIR").is_ok()
        || env::var("TERMINAL").map(|t| t.to_lowercase().contains("ghostty")).unwrap_or(false)
        || env::var("TERM_PROGRAM").map(|t| t.to_lowercase().contains("ghostty")).unwrap_or(false)
    {
        if let Ok(out) = Command::new("ghostty").arg("--version").output() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = out_str.lines().next() {
                let ver = first_line.trim_start_matches("Ghostty").trim();
                let clean_ver = ver.split('-').next().unwrap_or(ver);
                return format!("Ghostty v{}", clean_ver);
            }
        }
        return "Ghostty".to_string();
    }

    // 2. Check Kitty
    if env::var("KITTY_PID").is_ok() || env::var("KITTY_WINDOW_ID").is_ok() {
        if let Ok(out) = Command::new("kitty").arg("--version").output() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = out_str.lines().next() {
                let ver = first_line.trim_start_matches("kitty").trim();
                return format!("Kitty v{}", ver);
            }
        }
        return "Kitty".to_string();
    }

    // 3. Check Alacritty
    if env::var("ALACRITTY_SOCKET").is_ok()
        || env::var("ALACRITTY_LOG").is_ok()
        || env::var("ALACRITTY_WINDOW_ID").is_ok()
    {
        if let Ok(out) = Command::new("alacritty").arg("--version").output() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = out_str.lines().next() {
                let ver = first_line.trim_start_matches("alacritty").trim();
                return format!("Alacritty v{}", ver);
            }
        }
        return "Alacritty".to_string();
    }

    // 4. Check Foot
    if env::var("FOOT_SERVER_PID").is_ok() || env::var("TERM").map(|t| t == "foot").unwrap_or(false) {
        if let Ok(out) = Command::new("foot").arg("--version").output() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = out_str.lines().next() {
                let ver = first_line.trim_start_matches("foot version:").trim_start_matches("foot").trim();
                return format!("Foot v{}", ver);
            }
        }
        return "Foot".to_string();
    }

    // 5. Check WezTerm
    if env::var("WEZTERM_EXECUTABLE").is_ok()
        || env::var("WEZTERM_PANE").is_ok()
        || env::var("TERM_PROGRAM").map(|t| t == "WezTerm").unwrap_or(false)
    {
        if let Ok(ver) = env::var("WEZTERM_VERSION") {
            return format!("WezTerm v{}", ver);
        }
        return "WezTerm".to_string();
    }

    // 6. Generic TERM_PROGRAM & TERM_PROGRAM_VERSION
    if let Ok(term_prog) = env::var("TERM_PROGRAM") {
        if let Ok(term_ver) = env::var("TERM_PROGRAM_VERSION") {
            return format!("{} v{}", capitalize(&term_prog), term_ver);
        }
        return capitalize(&term_prog);
    }

    // 7. Fallback to Shell Name & Version (e.g. Fish v4.8.1)
    let shell_name = shell.rsplit('/').next().unwrap_or(shell);
    if let Ok(out) = Command::new(shell_name).arg("--version").output() {
        let out_str = String::from_utf8_lossy(&out.stdout);
        if let Some(first_line) = out_str.lines().next() {
            for word in first_line.split_whitespace() {
                if word.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                    if !clean.is_empty() {
                        return format!("{} v{}", capitalize(shell_name), clean);
                    }
                }
            }
        }
    }

    capitalize(shell_name)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
