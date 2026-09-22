# Implementation Plan — Dynamic SSH Session Detection & Multi-Server Adaptation (Phase 4)

**Date:** August 20, 2026  
**Branch:** `feat/ssh-host-profiling`  
**Status:** Complete — shipped in Phase 4 (v0.4.0)

---

## 🎯 Context & Objective
When administering a fleet of remote servers/VPS through Spiritty's integrated shell, the AI agent must know in real time which machine the user is operating on.
This plan sets up:
1. **Live detection of active SSH sessions** in the PTY via the `/proc/<pid>` process tree.
2. **A persistent cache of server profiles** in `~/.config/spiritty/hosts.json` (OS, distribution, kernel, package managers, services).
3. **Dynamic adaptation of the system context** injected into the LLM agent's *System Prompt* (favoring the remote distribution's commands, e.g. `apt` on Debian instead of `pacman` on Arch/CachyOS).
4. **Visual feedback in the terminal header** (`🌐 SSH: root@vps-01 (Debian 12)`).
5. **A fast fingerprint scan command (`Alt + S`)** to instantly profile and remember a new server.

---

## 📐 Architecture & Modules

### 1. Process Detection Module (`src/system/process_watcher.rs`) [NEW]
* Detects the active foreground process under the PTY shell (`child_pid`).
* Identifies whether an `ssh`, `mosh-client`, or `sftp` process is active.
* Extracts the target (`[user@]hostname`, optional port).
* State type `ActiveSession`: `Local` or `Ssh { target: String, command: Option<String> }`.

### 2. Host Profile Manager & Cache (`src/system/hosts.rs`) [NEW]
* `HostProfile` struct: `target`, `hostname`, `distro`, `kernel`, `package_managers`, `init_system`, `user`, `last_seen`.
* `HostsStore` struct with JSON serialization/deserialization (`~/.config/spiritty/hosts.json`).
* System probe output parser (`parse_probe_output`) to turn collected metadata into a structured profile.

### 3. Dynamic System Context (`src/system/mod.rs`) [MODIFY]
* Integration of `ActiveSession` and `Option<HostProfile>` into `SystemContext`.
* Update of `to_prompt_context()` to format the context either for the local machine or the connected remote server.

### 4. PTY Process (`src/pty/process.rs`) [MODIFY]
* Exposure of `pub fn child_pid(&self) -> Option<u32>`.

### 5. Application State & Events (`src/app.rs` & `src/event.rs`) [MODIFY]
* Periodic monitoring of the active session (every ~400ms on tick or on command execution).
* Handling of environment switching `Local <-> SSH`.
* Host scan trigger `Alt + S` (`AppEvent::ScanHost` / `scan_remote_host()`).
* Toast notification on detection or discovery of a server profile.

### 6. UI Rendering (`src/ui/terminal_panel.rs` & `src/ui/mod.rs`) [MODIFY]
* Shell window header:
  * If local: `💻 Ghostty`
  * If SSH: `🌐 SSH: user@host (Distro)` with a highlighted badge.
* Status bar (Footer): Displays the active target and the `[Alt+S] Scan VPS` help if the server has not yet been profiled.

### 7. Translation Keys (`src/i18n/`) [MODIFY]
* Addition of keys for SSH notifications, server scan, and headers.

---

## 🧪 Test Plan
- `tests/hosts_test.rs`:
  - `hosts.json` serialization / deserialization test.
  - Probe output parser test (`parse_probe_output`).
  - `SystemContext` switching test (Local vs SSH).
- `cargo test`: Verification of the 21+ existing unit tests + new tests.
- `cargo clippy -- -D warnings`: 0 warnings.
