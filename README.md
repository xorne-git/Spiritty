<div align="center">
  <img src="assets/logo.png" alt="Spiritty Logo" width="220">
  <h1>Spiritty 🧞⚡</h1>
  <p><strong>Next-generation AI terminal assistant for sysadmins, DevOps engineers, and power users.</strong></p>
  <p><strong>English</strong> | <a href="README.fr.md">Français</a></p>
</div>

Spiritty is an ergonomic TUI (Terminal User Interface) application written in **Rust** that brings together in a seamless split-screen:
- **Left Panel:** A proactive, context-aware AI system assistant.
- **Right Panel:** Your native, fully interactive shell (bash, zsh, fish) powered by an embedded PTY.

---

## ⚡ Quick Install

Install Spiritty with a single command (Linux & macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash
```

*Or build from source:*
```bash
git clone https://github.com/xorne-git/Spiritty.git
cd Spiritty
cargo build --release
sudo cp target/release/spiritty /usr/local/bin/
```

---

## 🎯 Why Spiritty?

I have been administering Linux servers for nearly 25 years. 😊

Existing AI-assisted terminal tools simply didn't fit my workflow: bloated interfaces, overly complex setups trying to do everything while struggling with straightforward, everyday sysadmin tasks. In pure CLI/TUI, there was virtually nothing built for this purpose.

So I decided to learn Rust and build the tool I actually needed: SSH into a VPS, audit and optimize Apache, PHP-FPM, or MySQL with the assistance of an LLM, hop onto another server and tell the model "*do the same here*", and keep moving seamlessly without ever leaving a purpose-built terminal workspace.

**Spiritty** fills this gap by delivering a lightweight sysadmin co-pilot with strict **human-in-the-loop** safety, capable of understanding your OS environment, diagnosing errors, and executing actions cleanly and transparently.

> *This is an early beta release. If you find this tool useful, that's awesome! All constructive feedback, bug reports, and suggestions are warmly welcome.*

---

## 🏗️ Architecture & Layout

```
+-------------------------------------------------------------------------+
|                                SPIRITTY                                 |
+------------------------------------+------------------------------------+
|  🤖 AI AGENT (Left Panel)          |  💻 INTERACTIVE SHELL (Right Panel)
|                                    |                                    |
|  > "Configure a reverse proxy      |  $ caddy run --config ...          |
|     with Caddy for my app on :8080"|  2026/08/19 15:00:00 [INFO] admin  |
|                                    |  2026/08/19 15:00:00 [ERROR] bind  |
|  [Agent] Detected Arch Linux.      |  address already in use :80        |
|  Port 80 is currently busy.        |                                    |
|  Let's check active processes:     |  $ sudo ss -tulpn | grep :80       |
|                                    |                                    |
|  Proposed Command:                 |                                    |
|  `sudo ss -tulpn | grep :80`       |                                    |
|                                    |                                    |
|  [Alt+N: Execute Proposal N]       |                                    |
+------------------------------------+------------------------------------+
| [Ctrl+Space: Toggle Focus] [Ctrl+Q: Quit] [Ctrl+N: New Session]         |
+-------------------------------------------------------------------------+
```

> *The diagram above shows the default side-by-side (vertical) view. Spiritty starts in the **vertical** layout (chat on the left, shell on the right) — press `F4` to switch to the horizontal layout (chat on top) and `F5` to swap the two panels.*

---

## 🛠️ Tech Stack

- **Language:** [Rust](https://www.rust-lang.org/) (High performance, memory safety, zero-dependency standalone binary).
- **TUI Framework:** [`ratatui`](https://ratatui.rs/) & [`crossterm`](https://crates.io/crates/crossterm).
- **PTY Engine:** [`portable-pty`](https://crates.io/crates/portable-pty).
- **Terminal Emulation (VT100/ANSI):** [`vt100`](https://crates.io/crates/vt100).
- **Async Runtime:** [`tokio`](https://tokio.rs/).
- **LLM Connectivity:** Multi-provider support (Local Ollama, LM Studio, Google Gemini, Anthropic Claude, OpenAI, DeepSeek, xAI Grok, Z.ai GLM).

---

## 🚀 Key Features

- [x] **Ergonomic Split-Screen:** AI Agent and native interactive shell (`$SHELL`) in a **vertical layout by default** (side by side, chat on the left ~50%, shell on the right) — `F4` switches to the stacked layout, `F5` swaps the two panels (terminal left/right or top/bottom), resizable by mouse drag or `Alt+←/→` (vertical) / `Alt+↑/↓` (horizontal).
- [x] **Multi-Provider LLM Engine:** Native streaming support for LM Studio, Ollama, Google Gemini, Anthropic Claude, OpenAI, DeepSeek, xAI (Grok), and Z.ai (GLM) with dynamic context window auto-detection.
- [x] **Session Management & Smart Compaction:**
  - Full session persistence stored in `~/.config/spiritty/sessions/`.
  - Interactive session browser modal (`Ctrl + H`) and instant clean session creation (`Ctrl + N`).
  - Restores prompt history (`▲` / `▼`).
  - Full history is persisted and restored — scrolling back shows the complete conversation even after reloads (no more 9-message cap in the sessions list).
  - Smart context compaction applies **only to the LLM context**: older turns roll into a structured System summary while the 8 most recent turns are kept verbatim, so token cost stays bounded on long sessions.
- [x] **Human-in-the-Loop & Command Proposal Cards:**
  - Automatic command extraction with safety classification badges (🟢 Safe / 🟡 Standard / 🟣 Sudo / 🔴 Risky).
  - One-key execution via `Alt + 1..9`, and `F10` to approve a pending authorization.
  - Live capture and proactive analysis of terminal command output.
- [x] **Mouse Support & Keyboard Shortcuts:**
  - Click-to-focus (`🖱`), mouse wheel scrolling (`🖱 Scroll / PgUp/PgDn`).
  - Mouse text selection with automatic clipboard copy (Wayland / X11).
  - In-app interactive model & API configuration (`Ctrl + P`) and help modal (`F1`).
- [x] **System Awareness & Dynamic SSH Detection:**
  - Automatic remote server profiling (`hosts.json`) and instant System Prompt switching on SSH connections.
  - 100% silent execution without visual noise or escape sentinels in the terminal.
  - Live, non-blocking shell: type commands freely while the model generates its response.
  - One-keystroke SSH reconnect modal (`⏎ Reconnect`) when reloading a session that was remote on a local PTY, with the persistent `· 🔗 SSH (reprise)` title hint.
- [x] **Multiline Prompt Editor:**
  - `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter`, and `Ctrl + J` for easy multiline prompt drafting.
- [x] **Auto-Approve Policies:**
  - Fast cycling via `F3`: 🟢 Safe / 🟡 Sudo / 🔴 YOLO / ⚫ Off.
- [x] **Internationalization (i18n):** Native English and French with automatic system locale detection (`$LANG`).
- [x] **100% Local Voice Input:** speak into Spiritty and land the text in the prompt — `F7` for continuous, silence-detected dictation (transcribed and sent automatically) and `F8` for a manual segment. Audio is captured by `arecord` / `ffmpeg` / `sox` and transcribed offline by `whisper.cpp`, with a live `● REC` / `⟳ Transcription…` badge. Nothing ever leaves your machine.

---

## 🚀 Command-Line Options (CLI)

Spiritty provides rich command-line arguments to streamline your terminal workflow:

```bash
# Resume the most recent session
spiritty -c

# Resume a specific session by ID or title prefix
spiritty -s sess_20260824_0001
spiritty -s nginx

# Send an initial query directly at launch
spiritty "Analyze memory usage and error logs"

# Quick-connect directly to an SSH server
spiritty --ssh root@vps-web.prod:22

# Temporarily override model or auto-approve policy
spiritty --model qwen2.5-coder:7b --yolo

# List all saved sessions
spiritty --list-sessions

# Show full CLI help
spiritty --help
```

---

## 🎙️ Local Voice Input (100% offline)

Spiritty can dictate straight into the chat prompt without sending a single byte to the cloud.

> **One-line installer shortcut:** `install.sh` now asks *“Install local voice input?”* — answer **yes** and it detects or builds `whisper.cpp`, downloads the GGML model into `~/.config/spiritty/models/` and enables `[voice]` for you. The manual steps below are only needed otherwise.

**Prerequisites (one-time):** a recorder (`arecord` from *alsa-utils* on Linux, or `ffmpeg` / `sox`), plus [`whisper.cpp`](https://github.com/ggml-org/whisper.cpp) (`whisper-cli`) and a GGML model.

```bash
# Linux example
sudo pacman -S alsa-utils            # or: sudo apt install alsa-utils

git clone --depth 1 https://github.com/ggml-org/whisper.cpp ~/.local/opt/whisper.cpp
cmake -S ~/.local/opt/whisper.cpp -B ~/.local/opt/whisper.cpp/build -DCMAKE_BUILD_TYPE=Release
cmake --build ~/.local/opt/whisper.cpp/build -j

mkdir -p ~/.config/spiritty/models
curl -fL -o ~/.config/spiritty/models/ggml-small.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
```

**Configuration** (`~/.config/spiritty/config.toml`):

```toml
[voice]
enabled         = true
whisper_bin     = "~/.local/opt/whisper.cpp/build/bin/whisper-cli"  # or "whisper-cli" if on PATH
model_path      = "~/.config/spiritty/models/ggml-small.bin"
language        = "fr"     # empty = follow the UI language
silence_ms      = 1400     # continuous mode: pause that closes a sentence
vad_threshold   = 0.015    # speech / silence RMS threshold
auto_submit     = true     # send the transcript automatically
```

Press `F7` for hands-free dictation (pause to validate) or `F8` to record a single segment; a red `● REC` / yellow `⟳ Transcription…` badge shows the live state.

---

## ⌨️ Primary Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Enter` | Send prompt (Chat) or submit command (Terminal) |
| `Shift + Enter` / `Ctrl + J` | Insert a newline in the multiline prompt editor |
| `Ctrl + Space` or `Shift + Tab` | Toggle focus (Chat ↔ Terminal) |
| `Alt + 1` .. `Alt + 9` | Directly execute command proposal N |
| `F10` | Approve the pending command authorization (one-keystroke) |
| `F6` | Toggle focus (alias of `Ctrl + Space`) |
| `F3` | Cycle Auto-Approve policy (Safe / Sudo / YOLO / Off) |
| `F4` | Toggle split layout (horizontal: chat on top / vertical: side by side) |
| `F5` | Swap the two panels (chat ↔ terminal) within the active layout |
| `F7` | Toggle continuous, silence-detected voice dictation (100% local) |
| `F8` | Dictate a manual voice segment (record → transcribe) |
| `Ctrl + B` | Quick-Connect SSH servers & bookmarks manager |
| `Ctrl + E` | Export current session to formatted Markdown report |
| `Ctrl + F` | Search in chat history with real-time match navigation |
| `Alt + D` | Proactive error diagnosis and auto-healing suggestion |
| `Ctrl + H` | Open session manager modal |
| `Ctrl + N` | Start a new clean session |
| `Ctrl + P` | Open model & API key configuration |
| `F1` | Show keyboard shortcuts help modal |
| `Alt + ←` / `Alt + →` | Adjust split ratio (vertical layout) |
| `Alt + ↑` / `Alt + ↓` | Adjust split ratio (horizontal layout) |
| `Ctrl + Q` | Save and quit Spiritty |

---

## 📂 Project Documentation

- 📐 **[ARCHITECTURE.md](ARCHITECTURE.md)**: Technical architecture and subsystem designs.
- 🗺️ **[ROADMAP.md](ROADMAP.md)** : Development milestones and release plan.
- 🤖 **[AGENTS.md](AGENTS.md)** : Engineering guidelines and conventions for AI contributors.
- 📜 **[CHANGELOG.md](CHANGELOG.md)** : Detailed history of changes between releases.

---

## 📄 License

Dual-licensed under MIT or Apache 2.0.
