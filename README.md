# Spiritty 👻⚡

**English** | [Français](README.fr.md)

> **Next-generation AI terminal assistant for sysadmins, DevOps engineers, and power users.**

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
|  [Enter: Execute | Tab: Edit]      |                                    |
+------------------------------------+------------------------------------+
| [Ctrl+Tab: Toggle Focus] [Ctrl+Q: Quit] [Ctrl+N: New Session]           |
+-------------------------------------------------------------------------+
```

---

## 🛠️ Tech Stack

- **Language:** [Rust](https://www.rust-lang.org/) (High performance, memory safety, zero-dependency standalone binary).
- **TUI Framework:** [`ratatui`](https://ratatui.rs/) & [`crossterm`](https://crates.io/crates/crossterm).
- **PTY Engine:** [`portable-pty`](https://crates.io/crates/portable-pty).
- **Terminal Emulation (VT100/ANSI):** [`vt100`](https://crates.io/crates/vt100).
- **Async Runtime:** [`tokio`](https://tokio.rs/).
- **LLM Connectivity:** Multi-provider support (Local Ollama, LM Studio, Google Gemini, Anthropic Claude, OpenAI, DeepSeek, xAI Grok).

---

## 🚀 Key Features

- [x] **Ergonomic Split-Screen:** AI Agent on the left, native interactive shell (`$SHELL`) on the right with interactive resizing (mouse drag or `Alt+Left/Right`).
- [x] **Multi-Provider LLM Engine:** Native streaming support for LM Studio, Ollama, Google Gemini, Anthropic Claude, OpenAI, DeepSeek, and xAI (Grok) with dynamic context window auto-detection.
- [x] **Session Management & Smart Compaction:**
  - Full session persistence stored in `~/.config/spiritty/sessions/`.
  - Interactive session browser modal (`Ctrl + H`) and instant clean session creation (`Ctrl + N`).
  - Restores prompt history (`▲` / `▼`).
  - Automatic memory compaction of older turns to save token costs while preserving critical context.
- [x] **Human-in-the-Loop & Command Proposal Cards:**
  - Automatic command extraction with safety classification badges (🟢 Safe / 🟡 Sudo / 🔴 Risky).
  - One-key execution via `Alt + 1..9` or `Enter`.
  - Live capture and proactive analysis of terminal command output.
- [x] **Mouse Support & Keyboard Shortcuts:**
  - Click-to-focus (`🖱`), mouse wheel scrolling (`🖱 Scroll / PgUp/PgDn`).
  - Mouse text selection with automatic clipboard copy (Wayland / X11).
  - In-app interactive model & API configuration (`Ctrl + P`) and help modal (`F1`).
- [x] **System Awareness & Dynamic SSH Detection:**
  - Automatic remote server profiling (`hosts.json`) and instant System Prompt switching on SSH connections.
  - 100% silent execution without visual noise or escape sentinels in the terminal.
  - Live, non-blocking shell: type commands freely while the model generates its response.
- [x] **Multiline Prompt Editor:**
  - `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter`, and `Ctrl + J` for easy multiline prompt drafting.
- [x] **Auto-Approve Policies:**
  - Fast cycling via `F3`: 🟢 Safe / 🟡 Sudo / 🔴 YOLO / ⚫ Off.
- [x] **Internationalization (i18n):** Native English and French with automatic system locale detection (`$LANG`).

---

## ⌨️ Primary Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Enter` | Send prompt (Chat) or submit command (Terminal) |
| `Shift + Enter` / `Ctrl + J` | Insert a newline in the multiline prompt editor |
| `Ctrl + Space` or `Shift + Tab` | Toggle focus (Chat ↔ Terminal) |
| `Alt + 1` .. `Alt + 9` | Directly execute command proposal N |
| `F3` | Cycle Auto-Approve policy (Safe / Sudo / YOLO / Off) |
| `Ctrl + H` | Open session manager modal |
| `Ctrl + N` | Start a new clean session |
| `Ctrl + P` | Open model & API key configuration |
| `F1` | Show keyboard shortcuts help modal |
| `Alt + ←` / `Alt + →` | Adjust split screen ratio |
| `Ctrl + Q` | Save and quit Spiritty |

---

## 📂 Project Documentation

- 📐 **[ARCHITECTURE.md](file:///home/xorne/Projets/Spiritty/ARCHITECTURE.md)**: Technical architecture and subsystem designs.
- 🗺️ **[ROADMAP.md](file:///home/xorne/Projets/Spiritty/ROADMAP.md)** : Development milestones and release plan.
- 🤖 **[AGENTS.md](file:///home/xorne/Projets/Spiritty/AGENTS.md)** : Engineering guidelines and conventions for AI contributors.

---

## 📄 License

Dual-licensed under MIT or Apache 2.0.
