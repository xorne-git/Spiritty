use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

use crate::event::AppEvent;
use crate::system::hosts::{HostProfile, HostsStore};
use crate::system::process_watcher::{
    detect_active_session, detect_current_working_dir, detect_git_branch, ActiveSession,
};
use crate::system::SystemContext;

/// Trait representing an inspectable terminal tab for process watching and remote context.
pub trait InspectableTab {
    fn child_pid(&self) -> Option<u32>;
    fn active_session(&self) -> &ActiveSession;
    fn set_active_session(&mut self, session: ActiveSession);
    fn set_remote_profile(&mut self, profile: Option<HostProfile>);
    fn set_current_dir(&mut self, dir: Option<String>);
    fn set_git_branch(&mut self, branch: Option<String>);
}

/// Encapsulates process watching, SSH host profiling, background probe tasks, and hosts storage.
pub struct SystemSupervisor {
    pub hosts_store: HostsStore,
    last_proc_scan: Instant,
    active_probes: HashSet<String>,
}

impl Default for SystemSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemSupervisor {
    pub fn new() -> Self {
        Self {
            hosts_store: HostsStore::load(),
            last_proc_scan: Instant::now(),
            active_probes: HashSet::new(),
        }
    }

    pub fn with_store(hosts_store: HostsStore) -> Self {
        Self {
            hosts_store,
            last_proc_scan: Instant::now(),
            active_probes: HashSet::new(),
        }
    }

    /// Checks whether the elapsed time since the last scan exceeds the given interval.
    pub fn should_scan(&self, interval: Duration) -> bool {
        self.last_proc_scan.elapsed() >= interval
    }

    /// Scans all tabs' child processes, updates their active sessions, and triggers SSH probes when needed.
    /// Returns an optional toast message if the active tab's session changed.
    pub fn scan_tabs<T: InspectableTab>(
        &mut self,
        tabs: &mut [T],
        active_idx: usize,
        system_context: &mut SystemContext,
        event_tx: &UnboundedSender<AppEvent>,
    ) -> Option<String> {
        self.last_proc_scan = Instant::now();
        let mut active_session_changed = None;
        let mut probes_to_trigger = Vec::new();

        for (idx, tab) in tabs.iter_mut().enumerate() {
            if let Some(child_pid) = tab.child_pid() {
                let new_session = detect_active_session(child_pid);
                if &new_session != tab.active_session() {
                    let was_ssh = tab.active_session().is_ssh();
                    tab.set_active_session(new_session.clone());
                    match &new_session {
                        ActiveSession::Ssh { target, .. } => {
                            if let Some(profile) = self.hosts_store.get(target) {
                                tab.set_remote_profile(Some(profile.clone()));
                            } else {
                                tab.set_remote_profile(None);
                                if !self.active_probes.contains(target) {
                                    probes_to_trigger.push(target.clone());
                                }
                            }
                        }
                        _ => {
                            tab.set_remote_profile(None);
                        }
                    }
                    if idx == active_idx {
                        active_session_changed = Some((was_ssh, new_session.clone()));
                    }
                }

                // Also refresh PWD and Git branch on local sessions
                if !tab.active_session().is_ssh() {
                    if let Some(cwd) = detect_current_working_dir(child_pid) {
                        let branch = detect_git_branch(&cwd);
                        tab.set_current_dir(Some(cwd));
                        tab.set_git_branch(branch);
                    }
                }
            }
        }

        // Trigger any pending background probes
        for target in probes_to_trigger {
            self.spawn_probe(target, event_tx);
        }

        // Sync system_context with the active tab if present
        if let Some(tab) = tabs.get(active_idx) {
            system_context.active_session = tab.active_session().clone();
            // Look up remote profile if SSH
            if let ActiveSession::Ssh { target, .. } = tab.active_session() {
                system_context.active_remote_profile = self.hosts_store.get(target).cloned();
            } else {
                system_context.active_remote_profile = None;
            }
        }

        // Generate toast notification if active session changed
        if let Some((was_ssh, new_session)) = active_session_changed {
            match new_session {
                ActiveSession::Ssh { target, .. } => {
                    if let Some(profile) = self.hosts_store.get(&target) {
                        Some(format!("🌐 SSH: {} ({})", target, profile.distro))
                    } else {
                        Some(format!("🌐 SSH: {}", target))
                    }
                }
                ActiveSession::Container {
                    runtime,
                    container_id,
                } => Some(format!("📦 {}: {}", runtime, container_id)),
                ActiveSession::Local { .. } => {
                    if was_ssh {
                        Some("🖥️ Retour à l'environnement local".to_string())
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    }

    /// Spawns a background task running the SSH probe command.
    pub fn spawn_probe(&mut self, target: String, event_tx: &UnboundedSender<AppEvent>) {
        if !self.active_probes.insert(target.clone()) {
            return;
        }

        let probe_target = target.clone();
        let tx = event_tx.clone();
        tokio::spawn(async move {
            let probe_cmd = HostsStore::generate_probe_command();
            let res = tokio::process::Command::new("ssh")
                .args([
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "ConnectTimeout=4",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    &probe_target,
                    probe_cmd,
                ])
                .output()
                .await;
            if let Ok(out) = res {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    let _ = tx.send(AppEvent::RemoteHostProbed {
                        target: probe_target,
                        output: text,
                    });
                }
            }
        });
    }

    /// Called when a remote host probe completes. Updates hosts_store and system_context if applicable.
    pub fn on_remote_host_probed(
        &mut self,
        target: &str,
        output: &str,
        system_context: &mut SystemContext,
    ) -> Option<HostProfile> {
        self.active_probes.remove(target);
        if let Some(profile) = HostsStore::parse_probe_output(target, output) {
            let _ = self.hosts_store.upsert(profile.clone());
            if let Some(active_target) = system_context.active_session.ssh_target() {
                if self
                    .hosts_store
                    .get(active_target)
                    .map(|p| p.target.as_str())
                    == Some(&profile.target)
                    || active_target == target
                    || target.contains(active_target)
                    || active_target.contains(target)
                {
                    system_context.active_remote_profile = Some(profile.clone());
                }
            }
            Some(profile)
        } else {
            None
        }
    }

    /// Triggers an immediate probe for the active SSH session if any.
    pub fn trigger_active_host_scan(
        &mut self,
        system_context: &SystemContext,
        event_tx: &UnboundedSender<AppEvent>,
    ) -> Result<String, &'static str> {
        if let Some(target) = system_context.active_session.ssh_target() {
            self.spawn_probe(target.to_string(), event_tx);
            Ok(target.to_string())
        } else {
            Err("ℹ️ Le scan est réservé aux sessions SSH distantes")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::sync::mpsc::unbounded_channel;

    struct MockTab {
        pid: Option<u32>,
        session: ActiveSession,
        remote_profile: Option<HostProfile>,
        current_dir: Option<String>,
        git_branch: Option<String>,
    }

    impl InspectableTab for MockTab {
        fn child_pid(&self) -> Option<u32> {
            self.pid
        }
        fn active_session(&self) -> &ActiveSession {
            &self.session
        }
        fn set_active_session(&mut self, session: ActiveSession) {
            self.session = session;
        }
        fn set_remote_profile(&mut self, profile: Option<HostProfile>) {
            self.remote_profile = profile;
        }
        fn set_current_dir(&mut self, dir: Option<String>) {
            self.current_dir = dir;
        }
        fn set_git_branch(&mut self, branch: Option<String>) {
            self.git_branch = branch;
        }
    }

    #[tokio::test]
    async fn test_supervisor_probe_and_profile_integration() {
        let dir = tempdir().unwrap();
        let store = HostsStore::load_from_path(dir.path().join("hosts.json"));
        let mut supervisor = SystemSupervisor::with_store(store);
        let mut sys_ctx = SystemContext::detect();
        sys_ctx.active_session = ActiveSession::Ssh {
            target: "prod.corp.net".to_string(),
            user: None,
            host: "prod.corp.net".to_string(),
            port: None,
        };

        let sample_output = r#"
SPIRITTY_PROBE_START
NAME="Ubuntu"
VERSION="24.04 LTS (Noble Numbat)"
ID=ubuntu
PRETTY_NAME="Ubuntu 24.04 LTS"
6.8.0-31-generic
ubuntu
prod.corp.net
/usr/bin/apt
/bin/systemctl
SPIRITTY_PROBE_END
"#;

        let profile =
            supervisor.on_remote_host_probed("prod.corp.net", sample_output, &mut sys_ctx);
        assert!(profile.is_some());
        let prof = profile.unwrap();
        assert_eq!(prof.distro, "Ubuntu 24.04 LTS");
        assert_eq!(prof.package_managers, vec!["apt".to_string()]);

        // Profile should be synchronized to system_context
        assert!(sys_ctx.active_remote_profile.is_some());
        assert_eq!(
            sys_ctx.active_remote_profile.as_ref().unwrap().distro,
            "Ubuntu 24.04 LTS"
        );

        // Active host scan should succeed for SSH
        let (tx, _rx) = unbounded_channel();
        let scan_res = supervisor.trigger_active_host_scan(&sys_ctx, &tx);
        assert!(scan_res.is_ok());
        assert_eq!(scan_res.unwrap(), "prod.corp.net");

        // Local session should return error for host scan
        sys_ctx.active_session = ActiveSession::Local {
            foreground_process: None,
        };
        let local_scan_res = supervisor.trigger_active_host_scan(&sys_ctx, &tx);
        assert!(local_scan_res.is_err());

        // Test scan_tabs with MockTab
        let mut mock_tabs = vec![MockTab {
            pid: None,
            session: ActiveSession::Local {
                foreground_process: None,
            },
            remote_profile: None,
            current_dir: None,
            git_branch: None,
        }];
        let toast = supervisor.scan_tabs(&mut mock_tabs, 0, &mut sys_ctx, &tx);
        assert!(toast.is_none());
    }
}
