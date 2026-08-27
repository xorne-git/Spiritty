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
            "none detected / POSIX standard".to_string()
        } else {
            self.package_managers.join(", ")
        };

        format!(
            "Active SSH Remote Environment (Target: {}):\n- Remote Host: {}\n- Distribution: {}\n- Kernel: {}\n- Remote User: {}\n- Available Remote Package Managers: {} (STRICTLY use these package managers for packages on this remote machine!)\n- Init System: {}\n- IMPORTANT: All command proposals and inspections execute on THIS REMOTE SERVER via SSH. Adapt commands to this distribution (e.g. apt on Debian/Ubuntu, apk on Alpine, dnf on Fedora/RHEL).",
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
        format!(
            "{} ({})",
            self.target,
            self.distro
                .split_whitespace()
                .next()
                .unwrap_or(&self.distro)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostBookmark {
    pub target: String,
    pub alias: Option<String>,
    #[serde(default)]
    pub is_favorite: bool,
    pub added_at: String,
}

#[derive(Debug, Clone)]
pub struct HostEntry {
    pub target: String,
    pub alias: Option<String>,
    pub is_favorite: bool,
    pub profile: Option<HostProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostsStore {
    pub profiles: HashMap<String, HostProfile>,
    #[serde(default)]
    pub bookmarks: Vec<HostBookmark>,
    #[serde(skip)]
    path: PathBuf,
}

impl HostsStore {
    pub fn load() -> Self {
        let path = Self::default_path().unwrap_or_else(|_| PathBuf::from("hosts.json"));
        Self::load_from_path(path)
    }

    pub fn load_from_path(path: PathBuf) -> Self {
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
            bookmarks: Vec::new(),
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

    pub fn add_bookmark(&mut self, target: String, alias: Option<String>) -> Result<()> {
        if let Some(bm) = self.bookmarks.iter_mut().find(|b| b.target == target) {
            bm.alias = alias;
            bm.is_favorite = true;
        } else {
            self.bookmarks.push(HostBookmark {
                target,
                alias,
                is_favorite: true,
                added_at: Utc::now().to_rfc3339(),
            });
        }
        self.save()
    }

    pub fn remove_bookmark(&mut self, target: &str) -> Result<()> {
        self.bookmarks.retain(|b| b.target != target);
        self.profiles.remove(target);
        self.save()
    }

    pub fn toggle_favorite(&mut self, target: &str) -> Result<()> {
        if let Some(bm) = self.bookmarks.iter_mut().find(|b| b.target == target) {
            bm.is_favorite = !bm.is_favorite;
        } else {
            self.bookmarks.push(HostBookmark {
                target: target.to_string(),
                alias: None,
                is_favorite: true,
                added_at: Utc::now().to_rfc3339(),
            });
        }
        self.save()
    }

    pub fn is_favorite(&self, target: &str) -> bool {
        self.bookmarks
            .iter()
            .any(|b| b.target == target && b.is_favorite)
    }

    /// Reverse-maps a remote HOSTNAME (e.g. `prod`, what a shell prompt shows) to
    /// the most recent known connection PROFILE (e.g. target `ducasse-seine.com`).
    /// Prompt remnants only reveal `user@hostname`, which is usually NOT directly
    /// connectable; the store knows which address actually reaches that machine.
    pub fn find_by_remote_hostname(&self, hostname: &str) -> Option<&HostProfile> {
        self.profiles
            .values()
            .filter(|p| p.hostname.as_deref() == Some(hostname))
            .max_by(|a, b| a.last_seen.cmp(&b.last_seen))
    }

    /// Turns a stored `last_ssh_target` into a connectable ssh argument:
    /// 1. an exact known profile target wins as-is;
    /// 2. otherwise `user@hostname` is resolved through the store — keeping the
    ///    stored user when it differs from the profile's (e.g. `root@prod`
    ///    → `root@ducasse-seine.com`), or the profile's canonical target;
    /// 3. otherwise the stored value is returned untouched (best effort).
    pub fn resolve_connectable_target(&self, stored: &str) -> String {
        if self.profiles.contains_key(stored) {
            return stored.to_string();
        }
        let (user, hostname) = stored.split_once('@').unwrap_or(("", stored));
        if let Some(profile) = self.find_by_remote_hostname(hostname) {
            if user.is_empty() || user == profile.user {
                return profile.target.clone();
            }
            return format!("{}@{}", user, profile.target);
        }
        stored.to_string()
    }

    pub fn get_alias(&self, target: &str) -> Option<&str> {
        self.bookmarks
            .iter()
            .find(|b| b.target == target)
            .and_then(|b| b.alias.as_deref())
    }

    pub fn list_all_entries(&self) -> Vec<HostEntry> {
        let mut entries = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. Add all bookmarks
        for bm in &self.bookmarks {
            let profile = self.get(&bm.target).cloned();
            seen.insert(bm.target.clone());
            entries.push(HostEntry {
                target: bm.target.clone(),
                alias: bm.alias.clone(),
                is_favorite: bm.is_favorite,
                profile,
            });
        }

        // 2. Add remaining cached profiles not in bookmarks
        for (target, profile) in &self.profiles {
            if !seen.contains(target) {
                entries.push(HostEntry {
                    target: target.clone(),
                    alias: None,
                    is_favorite: false,
                    profile: Some(profile.clone()),
                });
            }
        }

        // Sort favorites first, then alphabetically
        entries.sort_by(|a, b| {
            b.is_favorite
                .cmp(&a.is_favorite)
                .then_with(|| a.target.cmp(&b.target))
        });

        entries
    }

    pub fn get(&self, target: &str) -> Option<&HostProfile> {
        // 1. Exact match (e.g. root@vps-01:2222 or root@vps-01 or xorne.net)
        if let Some(profile) = self.profiles.get(target) {
            return Some(profile);
        }

        // 2. Try matching without port if target contains port
        if let Some(pos) = target.rfind(':') {
            let without_port = &target[..pos];
            if let Some(profile) = self.profiles.get(without_port) {
                return Some(profile);
            }
        }

        // 3. Try matching hostname only (if target is user@host or user@host:port)
        let host_part = if let Some(pos) = target.find('@') {
            let after_at = &target[pos + 1..];
            if let Some(p_pos) = after_at.rfind(':') {
                &after_at[..p_pos]
            } else {
                after_at
            }
        } else if let Some(p_pos) = target.rfind(':') {
            &target[..p_pos]
        } else {
            target
        };

        if let Some(profile) = self.profiles.get(host_part) {
            return Some(profile);
        }

        // 4. Flexible match across all stored profiles
        for (p_key, profile) in &self.profiles {
            if p_key == host_part
                || p_key.ends_with(&format!("@{}", host_part))
                || target.ends_with(&format!("@{}", p_key))
                || profile.hostname.as_deref() == Some(host_part)
                || profile.target.contains(host_part)
            {
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
        crate::config::restrict_file_permissions(&self.path);
        Ok(())
    }

    /// Generates the one-liner probe command to inspect a remote host via shell PTY
    pub fn generate_probe_command() -> &'static str {
        "printf 'SPIRITTY_PROBE_START\\n'; (command cat /etc/os-release || cat /etc/os-release) 2>/dev/null; uname -r 2>/dev/null; whoami 2>/dev/null; hostname 2>/dev/null; which apt pacman dnf yum apk brew zypper nix systemctl rc-service 2>/dev/null; printf 'SPIRITTY_PROBE_END\\n'"
    }

    /// Parses output from the probe command and creates a HostProfile
    pub fn parse_probe_output(target: &str, raw_output: &str) -> Option<HostProfile> {
        let start_marker = "SPIRITTY_PROBE_START";
        let end_marker = "SPIRITTY_PROBE_END";

        let clean_raw = strip_ansi_codes(raw_output);

        let section = if let Some(start_idx) = clean_raw.find(start_marker) {
            let after_start = &clean_raw[start_idx + start_marker.len()..];
            if let Some(end_idx) = after_start.find(end_marker) {
                &after_start[..end_idx]
            } else {
                after_start
            }
        } else {
            &clean_raw
        };

        let lines: Vec<&str> = section
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            return None;
        }

        let mut pretty_name: Option<String> = None;
        let mut name: Option<String> = None;
        let mut id_name: Option<String> = None;
        let mut version_str: Option<String> = None;
        let mut kernel = "unknown".to_string();
        let mut user = "root".to_string();
        let mut hostname = None;
        let mut package_managers = Vec::new();
        let mut init_system = "systemd".to_string();

        let mut non_os_release_lines = Vec::new();

        for &raw_l in &lines {
            if let Some(idx) = raw_l.find("PRETTY_NAME=") {
                let val = raw_l[idx + "PRETTY_NAME=".len()..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !val.is_empty() {
                    pretty_name = Some(val);
                }
            } else if let Some(idx) = raw_l.find("NAME=") {
                let val = raw_l[idx + "NAME=".len()..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !val.is_empty() {
                    name = Some(val);
                }
            } else if let Some(idx) = raw_l.find("ID=") {
                let val = raw_l[idx + "ID=".len()..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !val.is_empty() {
                    id_name = Some(val);
                }
            } else if let Some(idx) = raw_l.find("VERSION_ID=") {
                let val = raw_l[idx + "VERSION_ID=".len()..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !val.is_empty() {
                    version_str = Some(val);
                }
            } else if raw_l.contains('=') {
                // other os-release variables (VERSION_CODENAME, ID_LIKE, etc.)
                continue;
            } else {
                non_os_release_lines.push(raw_l);
            }
        }

        let distro = if let Some(pn) = pretty_name {
            pn
        } else if let Some(n) = name {
            if let Some(v) = version_str {
                format!("{} {}", n, v)
            } else {
                n
            }
        } else if let Some(id) = id_name {
            match id.to_lowercase().as_str() {
                "ubuntu" => "Ubuntu Linux".to_string(),
                "debian" => "Debian GNU/Linux".to_string(),
                "arch" => "Arch Linux".to_string(),
                "fedora" => "Fedora Linux".to_string(),
                "centos" | "rhel" | "rocky" | "almalinux" => format!("Enterprise Linux ({})", id),
                "alpine" => "Alpine Linux".to_string(),
                _ => format!("Linux ({})", id),
            }
        } else {
            "Linux (Unknown)".to_string()
        };

        // Process remaining lines for kernel, whoami, hostname, binaries
        for line in non_os_release_lines {
            if line.contains('/') {
                let bin = Path::new(line)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(line);
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
            } else if line
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
                && line.contains('.')
            {
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

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;
    for c in input.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
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

        let profile =
            HostsStore::parse_probe_output("root@vps-web-01", sample).expect("Parsed profile");
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

        let profile =
            HostsStore::parse_probe_output("admin@alpine-node-02", sample).expect("Parsed profile");
        assert_eq!(profile.distro, "Alpine Linux v3.19");
        assert_eq!(profile.package_managers, vec!["apk"]);
        assert_eq!(profile.init_system, "OpenRC");
    }

    #[test]
    fn test_parse_probe_output_ubuntu_numbered_and_ansi() {
        let sample = r#"
SPIRITTY_PROBE_START
1  PRETTY_NAME="Ubuntu 24.04.4 LTS"
2  NAME="Ubuntu"
3  VERSION_ID="24.04"
4  VERSION="24.04.4 LTS (Noble Numbat)"
5  VERSION_CODENAME=noble
6  ID=ubuntu
7  ID_LIKE=debian
6.8.0-137-generic
xorne
gg
/usr/bin/apt
/usr/bin/systemctl
SPIRITTY_PROBE_END
"#;

        let profile =
            HostsStore::parse_probe_output("gg.xorne.net", sample).expect("Parsed profile");
        assert_eq!(profile.target, "gg.xorne.net");
        assert_eq!(profile.distro, "Ubuntu 24.04.4 LTS");
        assert_eq!(profile.kernel, "6.8.0-137-generic");
        assert_eq!(profile.user, "xorne");
        assert_eq!(profile.package_managers, vec!["apt"]);
        assert_eq!(profile.init_system, "systemd");
    }
}

#[cfg(test)]
mod resolve_target_tests {
    use super::{HostProfile, HostsStore};

    fn profile(target: &str, hostname: &str, user: &str) -> HostProfile {
        HostProfile {
            target: target.to_string(),
            hostname: Some(hostname.to_string()),
            os_name: "Linux".to_string(),
            distro: "Debian".to_string(),
            kernel: "4.9".to_string(),
            user: user.to_string(),
            package_managers: vec!["apt".to_string()],
            init_system: "systemd".to_string(),
            last_seen: "2026-08-26T11:33:07+00:00".to_string(),
        }
    }

    fn store_with(entries: Vec<(&str, &str, &str, &str)>) -> HostsStore {
        let mut store = HostsStore::default();
        for (i, (target, hostname, user, seen)) in entries.into_iter().enumerate() {
            let mut p = profile(target, hostname, user);
            p.last_seen = format!("{}{:02}", seen, i);
            store.profiles.insert(target.to_string(), p);
        }
        store
    }

    #[test]
    fn resolves_inferred_hostname_to_real_target() {
        // Real-world case: the prompt shows `xorne@prod`, the store knows that
        // `prod` is reached via `ducasse-seine.com`.
        let store = store_with(vec![
            ("ib.xorne.net", "ib2", "xorne", "2026-08-24T12:25:30+00:0"),
            (
                "ducasse-seine.com",
                "prod",
                "xorne",
                "2026-08-26T11:33:07+00:0",
            ),
        ]);
        assert_eq!(
            store.resolve_connectable_target("xorne@prod"),
            "ducasse-seine.com"
        );
    }

    #[test]
    fn keeps_stored_user_when_it_differs_from_profile() {
        let store = store_with(vec![(
            "ducasse-seine.com",
            "prod",
            "xorne",
            "2026-08-26T11:33:07+00:00",
        )]);
        assert_eq!(
            store.resolve_connectable_target("root@prod"),
            "root@ducasse-seine.com"
        );
    }

    #[test]
    fn exact_profile_target_wins_as_is() {
        let store = store_with(vec![(
            "ducasse-seine.com",
            "prod",
            "xorne",
            "2026-08-26T11:33:07+00:00",
        )]);
        assert_eq!(
            store.resolve_connectable_target("ducasse-seine.com"),
            "ducasse-seine.com"
        );
    }

    #[test]
    fn unknown_target_returned_untouched() {
        let store = store_with(vec![]);
        assert_eq!(
            store.resolve_connectable_target("xorne@somewhere-else.net"),
            "xorne@somewhere-else.net"
        );
    }

    #[test]
    fn most_recent_profile_wins_on_hostname_collision() {
        let store = store_with(vec![
            (
                "old.example.com",
                "prod",
                "xorne",
                "2026-08-01T10:00:00+00:0",
            ),
            (
                "ducasse-seine.com",
                "prod",
                "xorne",
                "2026-08-26T11:33:07+00:0",
            ),
        ]);
        assert_eq!(
            store.resolve_connectable_target("xorne@prod"),
            "ducasse-seine.com"
        );
    }

    #[test]
    fn bare_hostname_gets_profile_target_without_user() {
        let store = store_with(vec![(
            "ducasse-seine.com",
            "prod",
            "xorne",
            "2026-08-26T11:33:07+00:00",
        )]);
        assert_eq!(
            store.resolve_connectable_target("prod"),
            "ducasse-seine.com"
        );
    }
}
