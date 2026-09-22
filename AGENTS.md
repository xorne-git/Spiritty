# AGENTS.md — Development Directives for Spiritty

Spiritty: a Rust TUI binary (ratatui + crossterm + tokio) combining an AI sysadmin agent (left panel) and an interactive PTY shell (right panel) in split-screen. Reference docs: [ARCHITECTURE.md](ARCHITECTURE.md) (design invariants), [ROADMAP.md](ROADMAP.md) (milestones), [CHANGELOG.md](CHANGELOG.md).

---

## 🛠️ Verification Commands

```bash
cargo check                # fast compilation
cargo clippy -- -D warnings   # zero warnings tolerated
cargo test                 # full suite (pure logic, runs headless)
cargo test --test session_test   # a single integration file
cargo build --release      # standalone binary in target/release/spiritty
```

- No `rustfmt.toml` or custom clippy config: default values.
- Tests do not automate the TUI or the PTY: they are pure-logic tests (unit `#[cfg(test)]` inside modules + per-domain integration in `tests/`). Visual TUI validation (resizing, focus, shortcuts) is done manually.

---

## 🏛️ Non-Negotiable Invariants

1. **Never block the event loop**: any long-running processing (LLM streaming, PTY I/O, system inspection) goes into a `tokio::spawn` task. Communication between subsystems (UI, PTY, agent) goes exclusively through typed `tokio::sync::mpsc` messages (see `src/event.rs`).
2. **Human-in-the-loop**: no command is injected into the PTY without explicit consent (outside user-enabled YOLO mode). Risk classification lives in `src/agent/safety.rs` (`CommandRisk`: Safe / Standard / Sudo / Risky), and the active policy is `ApprovalLevel` (`Safe / Standard / Sudo / YOLO / Off`, cycled with `F3`).
3. **Mandatory i18n**: every visible string (UI, modals, statuses, error messages) and every system prompt goes through `src/i18n/`. The `I18nKey` catalog is typed: adding a string = adding the enum variant **and** the entry in `fr.rs` **and** `en.rs`, otherwise `cargo check` fails. ⚠️ The default fallback is **French** (`Language::Fr` in `src/i18n/mod.rs`).
4. **PTY responsiveness**: typing in the right-hand shell must remain indistinguishable from a native terminal (zero unnecessary allocations in the `crossterm::event` loop).
5. **Errors**: no `unwrap()`/`expect()` outside tests; propagate via `thiserror`/`anyhow`. Runtime errors become typed events, never a TUI crash.

---

## 🗺️ Module Map (actual state)

```
src/
├── main.rs            # Terminal bootstrap + panic/signal hooks + event loop (no business logic)
├── lib.rs             # Exposes all modules (integration tests go through the lib)
├── app.rs / event.rs  # Global state + central event router (keyboard, PTY, LLM, timers)
├── cli.rs             # Hand-rolled CLI parsing (no clap): -S/--ssh, -m/--model, --yolo, -c/--continue
├── brand.rs           # Brand identity / in-TUI glyph + embedded icon assets
├── agent/             # AI agent: prompt.rs, tools.rs, safety.rs (command classification)
│   ├── providers/     # ollama.rs, gemini.rs, anthropic.rs, openai.rs
│   └── mcp/           # MCP client (manager + process)
├── pty/               # mod.rs (abstraction), process.rs ($SHELL lifecycle), vt.rs (vt100→ratatui bridge), capture.rs (tool output capture)
├── ui/                # mod.rs (layout), chat_panel.rs, terminal_panel.rs, theme.rs, components/ (modals, mod.rs)
├── system/            # hosts.rs (SSH profiles/hosts.json), process_watcher.rs, supervisor.rs, clipboard.rs
├── session/           # Persistence ~/.config/spiritty/sessions/ + context compaction; storage.rs
├── pricing/           # Token costs (assets/pricing.json)
├── i18n/              # mod.rs (I18nKey enum), fr.rs, en.rs
└── config/            # ~/.config/spiritty/config.toml + system_prompt.md override
```

- **LLM providers**: every OpenAI-compatible brand (DeepSeek, Z.ai/GLM, Grok, LM Studio...) goes through `providers/openai.rs` — do not create one file per brand. "Reasoning" models handle the `reasoning_content` field there (folded into `<think>…</think>` blocks).
- **SSH context**: profiling of remote servers (`/proc` detection, `hosts.json` cache) adapts the system prompt to the remote distribution — see `src/system/hosts.rs`.

---

## 🚀 Releases (exact ritual)

1. Record changes in the **"Unreleased"** section of `CHANGELOG.md` as you go (categories: `Added` · `Changed` · `Fixed` · `Performance` · `Security`).
2. At release: bump `version` in `Cargo.toml`, rename "Unreleased" to `## vX.Y.Z — YYYY-MM-DD`, open a new empty "Unreleased" section, commit `chore(release): vX.Y.Z — version bump, CHANGELOG and installer fallback`.
3. A `v*` tag triggers `.github/workflows/release.yml`: Linux x86_64 + aarch64 builds (via `cross`) and macOS aarch64 only (Intel macOS runners have been removed — do not re-add `x86_64-apple-darwin`).
4. Commit messages in conventional convention with scope: `feat(session):`, `fix(agent):`, `ci(release):`, `docs(roadmap):`...

---

## 🔄 Workflow Rules

1. Consult [ARCHITECTURE.md](ARCHITECTURE.md) before any redesign; detailed implementation plans live in `docs/plans/YYYY-MM-DD_name.md`.
2. Keep [ROADMAP.md](ROADMAP.md) and the "Unreleased" section of the CHANGELOG up to date as tasks progress.
3. **NEVER run `git commit` or `git push` on your own initiative** — only at the user's express request.
