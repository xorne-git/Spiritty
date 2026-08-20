use spiritty::system::{
    hosts::{HostProfile, HostsStore},
    process_watcher::{parse_session_from_cmdline, ActiveSession},
    SystemContext,
};
use tempfile::tempdir;

#[test]
fn test_parse_ssh_cmdline_variations() {
    // Basic ssh
    let cmd1 = vec!["ssh".to_string(), "root@vps-prod.internal".to_string()];
    assert_eq!(
        parse_session_from_cmdline(&cmd1),
        Some(ActiveSession::Ssh {
            target: "root@vps-prod.internal".to_string(),
            user: Some("root".to_string()),
            host: "vps-prod.internal".to_string(),
            port: None,
        })
    );

    // Custom port and identity file
    let cmd2 = vec![
        "ssh".to_string(),
        "-p".to_string(),
        "22022".to_string(),
        "-i".to_string(),
        "/home/user/.ssh/id_rsa".to_string(),
        "deploy@10.0.0.15".to_string(),
    ];
    assert_eq!(
        parse_session_from_cmdline(&cmd2),
        Some(ActiveSession::Ssh {
            target: "deploy@10.0.0.15:22022".to_string(),
            user: Some("deploy".to_string()),
            host: "10.0.0.15".to_string(),
            port: Some(22022),
        })
    );

    // Host without user, using -l flag
    let cmd3 = vec![
        "/usr/bin/ssh".to_string(),
        "-l".to_string(),
        "sysadmin".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "srv-db-backup".to_string(),
    ];
    assert_eq!(
        parse_session_from_cmdline(&cmd3),
        Some(ActiveSession::Ssh {
            target: "sysadmin@srv-db-backup".to_string(),
            user: Some("sysadmin".to_string()),
            host: "srv-db-backup".to_string(),
            port: None,
        })
    );
}

#[test]
fn test_parse_probe_output_ubuntu() {
    let sample = r#"
SPIRITTY_PROBE_START
NAME="Ubuntu"
VERSION="24.04 LTS (Noble Numbat)"
ID=ubuntu
PRETTY_NAME="Ubuntu 24.04 LTS"
6.8.0-31-generic
ubuntu
instance-aws-eu-central
/usr/bin/apt
/usr/bin/snap
/bin/systemctl
SPIRITTY_PROBE_END
"#;

    let profile = HostsStore::parse_probe_output("ubuntu@instance-aws-eu-central", sample)
        .expect("Should parse Ubuntu probe");
    assert_eq!(profile.target, "ubuntu@instance-aws-eu-central");
    assert_eq!(profile.distro, "Ubuntu 24.04 LTS");
    assert_eq!(profile.kernel, "6.8.0-31-generic");
    assert_eq!(profile.package_managers, vec!["apt"]);
    assert_eq!(profile.init_system, "systemd");
}

#[test]
fn test_parse_probe_output_arch() {
    let sample = r#"
SPIRITTY_PROBE_START
NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
6.10.3-arch1-1
archuser
arch-vps-hetzner
/usr/bin/pacman
/usr/bin/systemctl
SPIRITTY_PROBE_END
"#;

    let profile = HostsStore::parse_probe_output("archuser@arch-vps-hetzner", sample)
        .expect("Should parse Arch probe");
    assert_eq!(profile.target, "archuser@arch-vps-hetzner");
    assert_eq!(profile.distro, "Arch Linux");
    assert_eq!(profile.package_managers, vec!["pacman"]);
}

#[test]
fn test_hosts_store_persistence() {
    let tmp_dir = tempdir().expect("temp dir");
    let file_path = tmp_dir.path().join("hosts.json");

    let mut store = HostsStore::default();
    // Use upsert and save to custom location
    let profile = HostProfile {
        target: "root@srv-01.company.net".to_string(),
        hostname: Some("srv-01".to_string()),
        os_name: "Linux".to_string(),
        distro: "Debian GNU/Linux 12 (bookworm)".to_string(),
        kernel: "6.1.0-18-amd64".to_string(),
        user: "root".to_string(),
        package_managers: vec!["apt".to_string()],
        init_system: "systemd".to_string(),
        last_seen: "2026-08-20T21:00:00Z".to_string(),
    };

    store.profiles.insert(profile.target.clone(), profile.clone());
    let json = serde_json::to_string_pretty(&store).expect("serialize");
    std::fs::write(&file_path, json).expect("write");

    let read_json = std::fs::read_to_string(&file_path).expect("read");
    let loaded: HostsStore = serde_json::from_str(&read_json).expect("deserialize");
    assert_eq!(loaded.get("root@srv-01.company.net"), Some(&profile));
    // Test fuzzy port match
    assert_eq!(loaded.get("root@srv-01.company.net:22"), Some(&profile));
}

#[test]
fn test_system_context_prompt_switching() {
    let mut ctx = SystemContext::detect();
    let local_prompt = ctx.to_prompt_context();
    assert!(local_prompt.contains("Local Machine"));

    // Switch to unprofiled SSH
    ctx.active_session = ActiveSession::Ssh {
        target: "root@remote-vps.com".to_string(),
        user: Some("root".to_string()),
        host: "remote-vps.com".to_string(),
        port: None,
    };
    let ssh_unprofiled_prompt = ctx.to_prompt_context();
    assert!(ssh_unprofiled_prompt.contains("Active SSH Remote Environment"));
    assert!(ssh_unprofiled_prompt.contains("root@remote-vps.com"));

    // Add profiled host
    ctx.active_remote_profile = Some(HostProfile {
        target: "root@remote-vps.com".to_string(),
        hostname: Some("remote-vps".to_string()),
        os_name: "Linux".to_string(),
        distro: "Alpine Linux v3.19".to_string(),
        kernel: "6.6.14".to_string(),
        user: "root".to_string(),
        package_managers: vec!["apk".to_string()],
        init_system: "OpenRC".to_string(),
        last_seen: "2026-08-20T21:00:00Z".to_string(),
    });

    let ssh_profiled_prompt = ctx.to_prompt_context();
    assert!(ssh_profiled_prompt.contains("Alpine Linux v3.19"));
    assert!(ssh_profiled_prompt.contains("apk"));
    assert!(ssh_profiled_prompt.contains("OpenRC"));
}
