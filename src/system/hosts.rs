use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProfile {
    pub target: String,
    pub hostname: Option<String>,
    pub os_name: String,
    pub distro: String,
    pub kernel: String,
    pub user: String,
    pub package_managers: Vec<String>,
    pub init_system: String,
    pub last_seen: String,
}

impl HostProfile {
    pub fn to_prompt_context(&self) -> String {
        let pms = if self.package_managers.is_empty() {
            "non détecté / POSIX standard".to_string()
        } else {
            self.package_managers.join(", ")
        };

        format!(
            "Environnement distant SSH actif de l'utilisateur (Cible : {}) :\n- Hôte distant : {}\n- Distribution : {}\n- Noyau : {}\n- Utilisateur distant : {}\n- Gestionnaires de paquets distants : {} (Utilisez STRICTEMENT ces gestionnaires pour les paquets sur cette machine distante !)\n- Système d'init : {}\n- IMPORTANT : Toutes vos propositions de commandes et inspections s'exécutent sur CE SERVEUR DISTANT via SSH. Adaptez vos commandes à cette distribution (ex: apt sur Debian/Ubuntu, apk sur Alpine, dnf sur Fedora/RHEL).",
            self.target,
            self.hostname.as_deref().unwrap_or(&self.target),
            self.distro,
            self.kernel,
            self.user,
            pms,
            self.init_system
        )
    }

    pub fn display_badge(&self) -> String {
        format!("{} ({})", self.target, self.distro.split_whitespace().next().unwrap_or(&self.distro))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostsStore {
    pub profiles: HashMap<String, HostProfile>,
    #[serde(skip)]
    path: PathBuf,
}

impl HostsStore {
    pub fn load() -> Self {
        let path = Self::default_path().unwrap_or_else(|_| PathBuf::from("hosts.json"));
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut store) = serde_json::from_str::<HostsStore>(&content) {
                    store.path = path;
                    return store;
                }
            }
        }

        Self {
            profiles: HashMap::new(),
            path,
        }
    }

    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("spiritty");
        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("hosts.json"))
    }

    pub fn get(&self, target: &str) -> Option<&HostProfile> {
        // Try exact match first (e.g. root@vps-01:2222 or root@vps-01)
        if let Some(profile) = self.profiles.get(target) {
            return Some(profile);
        }

        // Try matching without port if target contains port
        if let Some(pos) = target.rfind(':') {
            let without_port = &target[..pos];
            if let Some(profile) = self.profiles.get(without_port) {
                return Some(profile);
            }
        }

        // Try matching hostname only
        if let Some(pos) = target.find('@') {
            let host_only = &target[pos + 1..];
            if let Some(profile) = self.profiles.get(host_only) {
                return Some(profile);
            }
        }

        None
    }

    pub fn upsert(&mut self, profile: HostProfile) -> Result<()> {
        self.profiles.insert(profile.target.clone(), profile);
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize hosts store to JSON")?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, json).context("Failed to write hosts.json")?;
        Ok(())
    }

    /// Generates the one-liner probe command to inspect a remote host via shell PTY
    pub fn generate_probe_command() -> &'static str {
        "printf 'SPIRITTY_PROBE_START\\n'; cat /etc/os-release 2>/dev/null; uname -r 2>/dev/null; whoami 2>/dev/null; hostname 2>/dev/null; which apt pacman dnf yum apk brew zypper nix systemctl rc-service 2>/dev/null; printf 'SPIRITTY_PROBE_END\\n'"
    }

    /// Parses output from the probe command and creates a HostProfile
    pub fn parse_probe_output(target: &str, raw_output: &str) -> Option<HostProfile> {
        let start_marker = "SPIRITTY_PROBE_START";
        let end_marker = "SPIRITTY_PROBE_END";

        let section = if let Some(start_idx) = raw_output.find(start_marker) {
            let after_start = &raw_output[start_idx + start_marker.len()..];
            if let Some(end_idx) = after_start.find(end_marker) {
                &after_start[..end_idx]
            } else {
                after_start
            }
        } else {
            raw_output
        };

        let lines: Vec<&str> = section.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            return None;
        }

        let mut distro = "Linux (Unknown)".to_string();
        let mut kernel = "unknown".to_string();
        let mut user = "root".to_string();
        let mut hostname = None;
        let mut package_managers = Vec::new();
        let mut init_system = "systemd".to_string();

        let mut non_os_release_lines = Vec::new();

        for line in &lines {
            if line.starts_with("PRETTY_NAME=") {
                distro = line.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string();
            } else if line.starts_with("NAME=") && distro == "Linux (Unknown)" {
                distro = line.trim_start_matches("NAME=").trim_matches('"').to_string();
            } else if line.contains('=') {
                // other os-release variables (ID, VERSION_ID, etc.)
                continue;
            } else {
                non_os_release_lines.push(*line);
            }
        }

        // Process remaining lines for kernel, whoami, hostname, binaries
        for line in non_os_release_lines {
            if line.contains('/') {
                let bin = Path::new(line).file_name().and_then(|n| n.to_str()).unwrap_or(line);
                match bin {
                    "apt" | "pacman" | "dnf" | "yum" | "apk" | "brew" | "zypper" | "nix" => {
                        if !package_managers.contains(&bin.to_string()) {
                            package_managers.push(bin.to_string());
                        }
                    }
                    "systemctl" => init_system = "systemd".to_string(),
                    "rc-service" => init_system = "OpenRC".to_string(),
                    _ => {}
                }
            } else if line.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && line.contains('.') {
                kernel = line.to_string();
            } else if user == "root" && !line.is_empty() && !line.contains(' ') {
                if hostname.is_none() {
                    user = line.to_string();
                }
            } else if hostname.is_none() && !line.is_empty() && !line.contains(' ') {
                hostname = Some(line.to_string());
            }
        }

        Some(HostProfile {
            target: target.to_string(),
            hostname,
            os_name: "Linux".to_string(),
            distro,
            kernel,
            user,
            package_managers,
            init_system,
            last_seen: Utc::now().to_rfc3339(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_probe_output_debian() {
        let sample = r#"
SPIRITTY_PROBE_START
PRETTY_NAME="Debian GNU/Linux 12 (bookworm)"
NAME="Debian GNU/Linux"
VERSION_ID="12"
VERSION="12 (bookworm)"
6.1.0-18-amd64
root
vps-web-01
/usr/bin/apt
/bin/systemctl
SPIRITTY_PROBE_END
"#;

        let profile = HostsStore::parse_probe_output("root@vps-web-01", sample).expect("Parsed profile");
        assert_eq!(profile.target, "root@vps-web-01");
        assert_eq!(profile.distro, "Debian GNU/Linux 12 (bookworm)");
        assert_eq!(profile.kernel, "6.1.0-18-amd64");
        assert_eq!(profile.package_managers, vec!["apt"]);
        assert_eq!(profile.init_system, "systemd");
    }

    #[test]
    fn test_parse_probe_output_alpine() {
        let sample = r#"
SPIRITTY_PROBE_START
NAME="Alpine Linux"
ID=alpine
VERSION_ID=3.19.1
PRETTY_NAME="Alpine Linux v3.19"
6.6.14-0-virt
admin
alpine-node-02
/sbin/apk
/sbin/rc-service
SPIRITTY_PROBE_END
"#;

        let profile = HostsStore::parse_probe_output("admin@alpine-node-02", sample).expect("Parsed profile");
        assert_eq!(profile.distro, "Alpine Linux v3.19");
        assert_eq!(profile.package_managers, vec!["apk"]);
        assert_eq!(profile.init_system, "OpenRC");
    }
}
