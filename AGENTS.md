# AGENTS.md — Development Directives for Spiritty

Spiritty is a Rust TUI binary (Ratatui + Crossterm + Tokio) pairing an AI sysadmin agent (left panel) with an interactive PTY shell (right panel) in split-screen.
Reference docs: [ARCHITECTURE.md](ARCHITECTURE.md) (design & subsystems), [ROADMAP.md](ROADMAP.md) (milestones), [CHANGELOG.md](CHANGELOG.md).

---

## 🛠️ Verification Commands

Run in this order after modifying code:

```bash
cargo check                        # Fast compilation check
cargo clippy -- -D warnings       # Strict linting (zero warnings tolerated)
cargo test                         # Full test suite (pure logic, runs headless)
cargo test --test session_test     # Single integration test suite in tests/
cargo test agent::safety::tests    # Single unit test module
cargo test <test_name>             # Single focused test
cargo build --release              # Standalone binary in target/release/spiritty
```

- **Toolchain config:** Uses standard Rust defaults; no custom `rustfmt.toml` or `clippy.toml`.
- **Testing scope:** Tests are pure logic (`#[cfg(test)]` modules + `tests/*.rs`). TUI rendering, PTY interactions, and window resizing are verified manually.

---

## 🏛️ Non-Negotiable Invariants

1. **Never block the event loop:** Long-running operations (LLM streaming, PTY I/O, system checks) MUST run in a `tokio::spawn` task. All inter-subsystem communication (UI, PTY, agent, voice) flows exclusively through typed `tokio::sync::mpsc` messages (see `src/event.rs` `AppEvent`).
2. **Human-in-the-loop safety:** No command executes in the PTY without explicit user consent (unless permitted by the active `ApprovalLevel`).
   - Risk classification lives in `src/agent/safety.rs` (`CommandRisk`: `Safe` / `Standard` / `Sudo` / `Risky`).
   - Active policy is `ApprovalLevel` (`Safe` / `Standard` / `Sudo` / `YOLO` / `Off`, toggled via `F3`).
3. **Mandatory typed i18n:** Every user-visible string, toast, modal, and system prompt MUST use `src/i18n/`.
   - Adding a string requires adding a variant to `I18nKey` in `src/i18n/mod.rs` AND implementations in BOTH `src/i18n/fr.rs` and `src/i18n/en.rs` (enforced at compile time).
   - Default fallback is **French** (`Language::Fr`).
4. **PTY responsiveness:** Shell typing in the terminal panel must feel native. Avoid allocations and heavy work in the `crossterm::event` loop.
5. **Robust error handling:** No `unwrap()` or `expect()` in application code (tests only). Bubble errors up via `anyhow`/`thiserror` or emit typed `AppEvent` errors to display in the UI without crashing.

---

## 🏗️ Architecture & Component Boundaries

- **Entrypoints:**
  - `src/main.rs`: Terminal initialization, raw mode, enhanced keyboard protocol, signal/panic hooks, and top-level event loop. Contains no business logic.
  - `src/lib.rs`: Exposes crate modules for integration tests in `tests/`.
- **LLM Providers (`src/agent/providers/`):**
  - All OpenAI-compatible APIs (DeepSeek, Grok/xAI, Z.ai/GLM, LM Studio) share `providers/openai.rs` via configuration. Do not create separate provider files for OpenAI-compatible APIs.
  - "Reasoning" models fold `reasoning_content` into `<think>...</think>` blocks inside `providers/openai.rs`.
- **PTY & VT100 (`src/pty/`):**
  - Spawns `$SHELL` via `portable-pty`.
  - ANSI/VT100 escape codes are parsed by `vt100::Parser` into a virtual screen, then rendered into Ratatui buffer cells.
  - `capture.rs` captures command outputs for the agent using completion sentinels (OSC 777) or prompt settling.
- **UI & Layout (`src/ui/`):**
  - Supports vertical and horizontal split layouts (`F4` toggles orientation, `F5` swaps panels).
- **CLI Parsing (`src/cli.rs`):** Hand-rolled argument parsing (no `clap` dependency).
- **Voice STT (`src/voice/`):**
  - 100% offline local dictation handled by `VoiceController`.
  - Shells out to external audio recorders (`arecord`/`ffmpeg`/`sox`) and `whisper-cli`. No C/C++ build dependencies in the Rust crate.
  - Keybindings: `F7` (continuous VAD dictation), `F8` (manual segment).
- **Config & Session Storage:**
  - Config: `~/.config/spiritty/config.toml` (never overwritten on parse error; backed up to `config.toml.bak`).
  - Sessions: JSON persistence in `~/.config/spiritty/sessions/`. Smart compaction keeps recent turns verbatim while summarizing older history in LLM context.
  - Remote SSH profiles: cached in `~/.config/spiritty/hosts.json` to adapt system prompts to remote environments.

---

## 🚀 Release Ritual

1. Accumulate changes in the `## Unreleased` section of `CHANGELOG.md` (`Added` · `Changed` · `Fixed` · `Performance` · `Security`).
2. When releasing:
   - Bump `version` in `Cargo.toml`.
   - Update the `LATEST_TAG` fallback in `install.sh`.
   - Rename `## Unreleased` in `CHANGELOG.md` to `## vX.Y.Z — YYYY-MM-DD` and open a fresh empty `## Unreleased` section.
   - Commit: `chore(release): vX.Y.Z — version bump, CHANGELOG and installer fallback`.
3. Pushing a tag `v*` triggers `.github/workflows/release.yml`:
   - Builds Linux `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (via `cross`).
   - Builds macOS `aarch64-apple-darwin` (Apple Silicon only; Intel macOS is not supported).
4. Commit messages follow Conventional Commits with scope: `feat(session):`, `fix(ui):`, `chore(release):`, etc.

---

## 🔄 Workflow Rules

1. Consult [ARCHITECTURE.md](ARCHITECTURE.md) before architectural changes; detailed specs live in `docs/plans/`.
2. Keep [ROADMAP.md](ROADMAP.md) and `CHANGELOG.md` updated as features progress.
3. **NEVER run `git commit` or `git push` on your own initiative** — only when explicitly requested by the user.
