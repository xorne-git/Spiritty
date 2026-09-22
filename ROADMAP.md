# Roadmap — Spiritty

This document defines the key milestones in the development of **Spiritty**, from the initial prototype up to version 1.0.

---

## 🎯 Global Milestone Overview

```
[Phase 1: TUI & PTY Foundations] ──> [Phase 2: Agent Engine & LLM] ──> [Phase 3: Human-in-the-Loop & Actions]
                                                                                    │
[Phase 5: v1.0 Release & Distribution] <── [Phase 4: System Context & Polishing] <──┘
```

---

## 📌 Phase 1: TUI & PTY Terminal Foundations (v0.1.0) [COMPLETE ✅]
*Objective: Have a fluid TUI application with a functional split-screen and a native interactive shell in the right panel.*

- [x] **Cargo project initialization:**
  - Configuration of the `Cargo.toml` with dependencies (`ratatui`, `crossterm`, `tokio`, `portable-pty`, `vt100`, `anyhow`, `serde`).
- [x] **Event loop & base layout:**
  - 40/60 horizontal split-screen layout.
  - Keyboard focus management: quick toggle (`Ctrl + Space`) between Chat and Terminal.
  - Status bar (Header & Footer) with shortcuts and system status.
- [x] **PTY integration into Ratatui:**
  - Spawn of the default shell (`$SHELL`) via `portable-pty`.
  - Capture and parsing of ANSI/VT100 bytes via `vt100`.
  - Rendering of the virtual buffer into `ratatui::buffer::Buffer` cells.
  - Transmission of keystrokes to the master PTY in raw mode.
  - Dynamic handling of window resizing (`SIGWINCH` / `pty.resize`).

---

## 📌 Phase 2: Agent Engine & LLM Integration (v0.2.0) [COMPLETE ✅]
*Objective: Connect an LLM to the left panel with response streaming, multi-provider configuration (Ollama, LM Studio, Gemini, Grok, DeepSeek, OpenAI, Claude) and interactive modals.*

- [x] **Interactive Chat Panel & Streaming:**
  - Multi-line input area, message history, hardware cursor.
  - Asynchronous streaming without blocking the interactive PTY shell.
  - Visual status indicator (`🧞 Spiritty is thinking...`).
- [x] **LLM Providers (Multi-Provider):**
  - Ollama client (local models such as `qwen2.5-coder`, `deepseek-r1`).
  - LM Studio client (local OpenAI-compatible server).
  - Grok / xAI client (`api.x.ai/v1`).
  - Google Gemini client (REST SSE streaming).
  - DeepSeek & OpenAI client.
  - Anthropic Claude client.
- [x] **Configuration Management & Modals:**
  - TOML configuration file (`~/.config/spiritty/config.toml`).
  - Interactive configuration modal (`Ctrl+P`) to change provider/model/key.
  - Shortcut help modal (`F1`).
- [x] **Internationalization (i18n) Engine:**
  - Automatic detection of the system language (`$LANG`) and French/English support.
  - Language override in the configuration (`language = "fr"`).
  - Translation of all modals, help footers, statuses and system prompts.

---

## 📌 Phase 3: Human Validation, Command Execution & Sessions (v0.3.0) [COMPLETE ✅]
*Objective: Allow the agent to propose commands and the user to execute them in one gesture in the right terminal, with session persistence and compaction.*

- [x] **"Command Proposal Card" Component:**
  - Automatic detection of command blocks proposed by the AI and filtering of explanations.
  - Interactive multi-proposal action cards (`Alt + 1..9`).
- [x] **Keyboard Actions & Live PTY Execution:**
  - `[Enter]`: Direct send to the model or injection into the PTY.
  - `[Alt + N]`: Execution of proposal N with capture and analysis of the result.
- [x] **Session Manager & Context Compaction:**
  - Structured JSON storage in `~/.config/spiritty/sessions/`.
  - Interactive session navigation modal (`Ctrl + H`) with reload, creation (`Ctrl + N`), deletion and manual compaction.
  - Intelligent automatic compaction of old dialogue turns to preserve tokens.
  - Systematic auto-save of the current session on application close.

---

## 📌 Phase 4: Advanced System Context, SSH Detection & Auto-Remediation (v0.4.0) [COMPLETE ✅]
*Objective: Give the agent a keen awareness of the host machine (local or remote SSH server), silent execution without terminal pollution and a fully interactive shell at all times.*

- [x] **Dynamic SSH Session Detection & Multi-Host Profiling:**
  - Non-blocking real-time monitoring of the process tree under the PTY (`/proc/<pid>/...`).
  - Automatic detection of `ssh`, `sftp`, `mosh-client`, `docker`, `podman` connections.
  - Persistent cache of server profiles in `~/.config/spiritty/hosts.json` (OS, distribution, kernel, package managers, init system).
  - Instant and automatic switching of the AI *System Prompt* on SSH connections/disconnections.
  - Clean header and status indicators (`🌐 SSH: user@host (Distro)`).
- [x] **Silent Execution and Continuously Interactive Shell:**
  - 100% clean execution without any visible sentinel (`printf "\033]..."`) in the PTY terminal.
  - Continuous and non-blocking shell input during model thinking and streaming.
  - Automatic focus switch to the shell on prompt submission for optimal ergonomics.
  - Prevention of shell history pollution (leading space for Fish, Bash, Zsh).
- [x] **Multi-Line Prompt Editor & Markdown Self-Repair:**
  - Fluid line breaks via `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter` and `Ctrl + J`.
  - On-the-fly self-repair of code blocks closed prematurely by LLMs.
  - Filtering of fake command blocks (transition arrows, descriptions).
- [x] **Local System Context Extractor:**
  - Automatic detection of the Linux distribution (Arch, CachyOS, Ubuntu, Debian, Fedora, Alpine) or macOS.
  - Detection of installed package managers (`apt`, `pacman`, `dnf`, `brew`, `nix`, `cargo`, `yay`, `paru`, `flatpak`, `snap`).
  - Capture of the active shell, the host terminal emulator and the graphical environment (`Wayland`/`X11`/`niri`/`hyprland`).
- [x] **Proactive Error Capture & Diagnosis (`Alt + D`):**
  - Automatic detection of failed commands and execution errors in the PTY.
  - Alert card and automatic remediation in one shortcut (`Alt + D`).
- [x] **Current Directory (PWD) & Git Branch Indicator:**
  - Instant display of the active folder and Git branch in the terminal header.
  - Dynamic injection of the PWD and Git branch into the agent's system context.
- [x] **Session Export as Markdown Report (`Ctrl + E`):**
  - One-key generation of a structured report with timestamp, machine metadata, prompt history and commands in `~/.config/spiritty/exports/`.
- [x] **Favorite SSH Server Manager (`Ctrl + B`):**
  - Interactive SSH favorites modal with quick add, search, favorite stars and one-key connection.
- [x] **Real-Time Search in Chat History (`Ctrl + F`):**
  - Search bar with dynamic highlighting and quick navigation between occurrences (`Enter` / `Shift + Enter`).

---

## 📌 Phase 5: Advanced Protocols, Precise Metrics & Distribution (v1.0.0) [IN PROGRESS 🚀]
*Objective: Integrate the MCP ecosystem, guarantee absolute metric precision for tokens and throughput, and produce an ultra-fast and stable binary.*

- [x] **MCP (Model Context Protocol) Support & Dedicated TUI Modal (`Ctrl + M`):**
  - Asynchronous MCP stdio client engine (JSON-RPC 2.0) with initialization, capability negotiation and dynamic tool discovery (`tools/list`).
  - Dynamic discovery and exposure of MCP tools in the agent's system prompt (`mcp:<server>:<tool>`).
  - Interception and asynchronous execution of MCP tool calls by the agent (`ToolInvocation::McpCall`).
  - Interactive TUI modal (`Ctrl + M`): server list, tool inspector, one-key activation/deactivation (`Space`), reload (`R`), add (`A`) and delete (`D`).
- [x] **Precise Token & Throughput (tokens/s) Calculation & Dynamic Cost Registry:**
  - Use of native metrics returned by LLM APIs (`eval_count`/`eval_duration` in Ollama, `stream_options.include_usage` in OpenAI/DeepSeek/Grok, `message_delta.usage` in Anthropic, `usageMetadata` in Gemini).
  - Elimination of throughput calculation artifacts: streaming timer started at the first useful chunk, deduction of network latencies and tool execution pauses.
  - Real-time estimation and display of the session cost in dollars (`💵 $0.0042`) in the footer and session modals for cloud models.
  - **Dynamic LLM pricing registry (`src/pricing/`):** support for custom overrides in `~/.config/spiritty/config.toml` (`[pricing."model_name"]`), persistence of the local cache in `~/.config/spiritty/pricing.json`, and one-key update/synchronization from the Internet (`Ctrl + P` then `[U]`).
- [x] **Non-Intrusive Toast Modal & Targeted Proactive Diagnosis (`Alt + D`):**
  - Shell error notification as an elegant floating toast at the bottom right of the terminal panel with a rounded border (`Alt + D` Diagnose / `Alt + X` Close).
  - Strict restriction of proactive diagnosis to only commands typed manually by the user in the interactive shell.
  - Absolute respect for the cleanliness of the chat panel: 0 pollution on manual command errors.
- [x] **Ergonomics & Themes:**
  - 7 predefined themes with vertical gradients and coordinated palettes: *Spiritty Dark*, *Catppuccin Mocha*, *Tokyo Night*, *Nord Arctic*, *Gruvbox Dark*, *Dracula*, *Monokai Pro*.
  - Interactive theme selector in the `Ctrl + P` modal with instant dynamic preview.
  - Fluid resizing of the left/right split with the mouse (drag-and-drop) or the keyboard (`Alt + ←` / `Alt + →`).
  - Automatic persistence of panel sizes (`split_ratio`), MCP configuration and the active theme in `~/.config/spiritty/config.toml`.
  - Explicit error diagnostics on LLM connection failures (local servers off, timeout, network errors).
- [x] **Hardening v0.4.5 — Security, Perf & Reliability (full audit):** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - Human-in-the-loop security: "empty Enter approves the pending command" flaw closed; 4-level risk taxonomy (`Safe / Standard / Sudo / Risky`) with `Sudo` auto-approval covering elevated read-only commands.
  - Secrets hygiene: `config.toml`/`hosts.json`/sessions/pricing written in 0600, API key never redisplayed in the modal (masking + retention if empty).
  - PTY lifecycle: clean exit on shell `exit` (`PtyExit` + reaper thread), no more zombies or frozen panels.
  - UI thread never blocked: asynchronous Ctrl+V, memoized ENV probe, bounded `/proc` scan (1.5 s TTL).
  - Incremental PTY capture: end of O(n²) on verbose commands (UTF-8 decoding with carry, bounded windows, sentinel watermark).
  - Cleanup of LLM proposals: stray interpreter lines (`bash`, shebangs, orphaned `exit`) removed; prose/output tab characters no longer become ⚡ cards.
  - Z.ai (GLM/Zhipu) provider added with built-in pricing and alias detection.
- [x] **Hardening v0.5.0 — Reliable Tool Loop & Indestructible UI:** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - Textual tool loop actually executed again (hybrid DeepSeek/GLM DSML markup parsed); corrupted echoes from SSH redraws recognized and removed — no more "saboteur" hallucinations fed by the echo.
  - Reliability: off-thread PTY writes (no more UI freeze on stalled SSH), capture capped at 1 MiB with lazy cleanup + truncation notice, terminal restoration on SIGTERM/SIGINT/SIGHUP.
  - UX: "💭 Deep thinking…" timer re-armed on each thinking segment, no more double `Alt+N`/`F10` prompt on the same command (snippet made inert + Alt+N blocked during consent or capture).
- [x] **Hardening v0.5.1 — SSH Reconnection on In-App Reload:** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - The SSH reconnection modal was set by `load_session` but **immediately closed** by the session list handler (`modal = None` after the `Load` action): it only survived on the `-c` path (startup). The list no longer closes if the `SshReconnect` offer has just been set. Validated E2E on a real SSH session reloaded in-app.
- [x] **Hardening v0.5.2 — Full History Persisted, Context-Only Compaction (option C):** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - `save_current_session` no longer compacts: the session JSON keeps **all** exchanges and reloading restores the entire conversation (no more the "1 summary + 8 turns = 9 messages" ceiling inherited from compaction at save time in the session list).
  - Compaction moved **to request time**: `agent::send_prompt` applies `compact_chat_messages` (old turns → System summary, 8 most recent verbatim) — the live context stays bounded and is compressed at every turn. Histories already compacted by previous versions cannot be reconstructed. Validated E2E in an isolated HOME (15-message session → reload → 15 messages on disk, no persisted summary).
- [x] **Hardening v0.5.3 — System Prompt: Forbid Abbreviating Commands:** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - Fix for the case where GLM (`glm-5.3-flash`) shortened its long commands to `...` (or a `BASE64`, `[content]` placeholder) then blamed the breakage on a "client truncation": new rule in `IMPORTANT RULES` — always paste the full content of a heredoc/script/file, never a placeholder, the block is executed as-is. Verified client-side: `extract_all_command_proposals` truncates nothing (no ceiling or `...` insertion), the UI's `...` were only display indicators.
- [x] **Hardening v0.5.4 — Fixed Ctx Footer & Cleaned-Up Release CI:** *(full detail in [CHANGELOG.md](CHANGELOG.md))*
  - `get_context_used_tokens` summed the entire session history (remnant of option C) → absurd "Ctx: 178k / 131k (100%)" (used > window, clamped to 100%). It now estimates the **actually sent compacted** context (summary + last 8 turns verbatim), consistent with request-time compaction; the total token counter remains separate.
  - Release CI: removal of the `x86_64-apple-darwin` target (Intel macOS hosted runners `macos-13` retired by GitHub — the job stayed `queued` and never built). Matrix reduced to Linux `x86_64` / `aarch64` + macOS Apple Silicon `aarch64`.
- [x] **Robustness Tests & Advanced Terminal Ergonomics:**
  - [x] Robust handling of ncurses & interactive TUI applications in the right PTY (`vim`, `nano`, `htop`, `fzf`, `lazygit`, `less`): SGR mouse tracking, shift bypass, alternate-screen navigation, xterm modifiers, bracketed paste.
  - [x] Enriched multi-tabs: tab renaming (`Alt+R`), session persistence of labels and SSH targets.
  - [x] Dynamic discovery of LLM models and live pricing (`R` in `F2`) and configurable thinking level (`ReasoningEffort` with 5 levels, visual badges and multi-provider API support).
  - [x] Official DeepSeek V4.1 Flash integration (`deepseek-flash`), auto-approval of Safe/Sudo commands and hermetic isolation of test sessions.
  - [x] Architectural deepening (Deep Modules):
    - Unification of TUI modals (`ModalState` / `ModalOutcome` in `src/ui/components/`), centralizing keystrokes, paste and rendering.
    - System supervisor and SSH contexts (`SystemSupervisor` and `InspectableTab` in `src/system/supervisor.rs`), isolating `/proc` process detection, distribution profiling and deduplication of background probes.
  - [x] Clean handling of `SIGINT`, `SIGTERM`, `SIGHUP` signals (full terminal restoration + `128+signal` exit, even with a frozen UI).
- [x] **Packaging & Automated Distribution:**
  - Universal one-line `install.sh` installation script (`curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash`) with automatic OS and architecture detection (`x86_64`, `aarch64`, macOS).
  - Automated multi-target GitHub Actions publishing pipeline (`release.yml`) generating stripped (`strip`) binaries and tarball archives on each `v*` tag.
  - Static and universal binary ready for `cargo install`, AUR and Homebrew.
