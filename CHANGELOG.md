# Changelog — Spiritty

All notable changes to Spiritty are documented in this file.

## 📌 Convention

This changelog follows the [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) format.

- **Between two pushes**: changes accumulate in the **"Unreleased"** section.
- **At release time (tag `vX.Y.Z`)**: the "Unreleased" section is renamed to
  `## vX.Y.Z — YYYY-MM-DD` and a new empty "Unreleased" section is opened above it.
- The categories used: `Added` · `Changed` · `Fixed` · `Performance` · `Security`.

---

## Unreleased

### Added

- **Adjustable split orientation — horizontal layout by default (`F4`)**:
  - The chat/terminal split now defaults to a **horizontal layout** (chat on top at ~70% height, interactive PTY shell below), which stays readable on narrow terminals; press `F4` to switch back to the classic side-by-side (vertical) layout.
  - Each orientation remembers its own ratio: vertical defaults to 50% chat width, horizontal to 70% chat height. The orientation and both ratios are persisted in `~/.config/spiritty/config.toml` (`split_orientation`, `split_ratio`, `split_ratio_horizontal`).
  - `Alt + ←/→` resizes the split in vertical mode, `Alt + ↑/↓` in horizontal mode; the divider is drawn as a vertical `│` or horizontal `─` line accordingly and stays draggable with the mouse. The shell tabs, mouse selection and PTY sizing all follow the active orientation.
- **Direct validation of a pending command with a single press of [ Enter ]**:
  - When a command awaits user confirmation (`pending_tool_approval`), a single press of `[ Enter ]` validates and runs the command directly if the input field (prompt) is empty (or contains an affirmative sentence such as `ok` / `oui`).
  - If the user types text (question, new directive) before pressing `Enter`, the pending command is cleanly denied to unblock the agent, and the new message is immediately submitted to the model.
  - Update of the footer of the approval command cards and of the prompt hint message to `[ Enter ] Allow  ·  [ Esc ] Deny`.

### Fixed

- **Reasoning turns no longer end silently (thinking swallowed the command)**:
  - Spiritty now wraps a provider's streamed `reasoning_content` in a dedicated, unambiguous marker (`<spiritty:think>…</spiritty:think>`) instead of `<think>…</think>`. A model that literally writes `<think>`/`</think>` *inside* its reasoning (e.g. while debugging this very feature) can no longer split Spiritty's own reasoning boundary and corrupt parsing.
  - `strip_think_blocks` strips residual reasoning closing tags (a leaked `</think>` used to pollute the answer and could sit on a code fence's closing line — ` ```</think> ` — making `find_closing_code_fence` reject the block and silently drop the whole command). Legacy `<think>` handling is preserved for older sessions.
  - When a reasoning block is left **unclosed** and is immediately followed by the model's real command (` ```bash ` / ` ```tool: `…), the command is now preserved instead of being swallowed (this alone recovered ~8+3 command blocks across two real sessions).
  - New **dead-turn recovery**: if a turn produces no visible answer but its reasoning carried an executable command, the command is promoted to a visible ```` ```bash ```` block (and a proposal) so the normal card / approval flow handles it — requiring user consent — instead of ending the turn with nothing.
  - The model's private reasoning is now dropped from the conversation sent back to the API (it is no longer echoed, saving context).
- **Elimination of the 100% CPU freeze during model streaming and terminal output**:
  - **Batched event draining**: the main loop [`run_loop`](file:///home/xorne/Projets/Spiritty/src/main.rs) now immediately drains all pending events in memory (`try_recv()`) before triggering a render, eliminating queue congestion where each individual chunk or PTY byte triggered a full terminal redraw.
  - **Frame rate throttling (~30 FPS)**: the periodic background redraw is limited to 30 ms for continuous streams (LLM chunks, PTY output), while keeping an instant 0 ms render for interactive keyboard input, pastes and mouse clicks.
  - **Removal of spurious redraws on mouse hover (`Mouse(Moved)`)**: micro-movements of the mouse no longer cause a continuous full-window redraw when no split resize is active.
- **Prevention of chunking panics on command cards (`chunks(0)`)**:
  - Hardening of command line splitting in `compose_approval_card` and `render_command_card` (systematic `.max(1)` on `max_chunk_w`) preventing any crash when resizing very narrow windows.
- **Update of the standalone production binary**:
  - Clean release rebuild in `target/release/spiritty` incorporating all the UI overhauls and stability fixes.

### Performance

- **Caching of the thinking status (`has_thought`) in `ChatCacheEntry`**:
  - Avoids the costly re-parsing and string allocations on all history messages (notably the large blocks of tens of thousands of characters) during the chat display pass.
  - Ultra-fast detection of `<think>` tags without useless allocation.
- **Optimization of `collapse_thought_to_single_line`**:
  - Bounded analysis on the end of the reasoning stream (`max_w * 4` bytes) and $O(N)$ construction instead of a full split/recombine of blocks of several tens of thousands of characters.
- **Filtering of command proposals during streaming**:
  - Suspension of regex analysis and code-block parsing on pure reasoning fragments until a code or tool tag actually appears (` ``` `, `<tool:`...).

### Changed

- **Status bar shortcut priorities & help modal readability**:
  - The footer's right-hand shortcuts now prioritize `F3` (approval), `Ctrl + P` Config, `F4` (layout) and `F1` (help); `Ctrl + B` / `Ctrl + M` / `Ctrl + H` (Hosts/MCP/Sessions) appear only when space allows, and the rendering degrades from bracketed pills to compact badges to bare keys instead of overflowing.
  - The `F1` help modal is now a **single full-width column** (one shortcut per line, a blank line before each section title, no more descriptions truncated by a two-column split) and **scrolls** (`↑`/`↓`, `PgUp`/`PgDn`, `Home`/`End`, `j`/`k`) with a scrollbar when the list exceeds the viewport.
- **General harmonization of buttons and shortcut keys (`key_pill`)**:
  - Replacement of the solid Powerline pills with rounded edges (`...`) by the framed key format `[ key ]` (colored brackets and bold label without a solid background), guaranteeing a clean, universal rendering with no dependency on Nerd Fonts glyphs.
  - Systematic unification of key combinations as a single key `[ Ctrl + Key ]` (instead of `[ Ctrl ] + [ Key ]` or `[ Ctrl ] [ Key ]`) in the F1 help modal, the configuration modal, the footer and the previews.
  - Centralization of `key_pill` in [`src/ui/mod.rs`](file:///home/xorne/Projets/Spiritty/src/ui/mod.rs) with support for `Cow<'a, str>` (static or dynamically composed strings) and direct reuse in the terminal panel and all modals.
- **Addition of a permanent vertical divider between the two windows (Chat and Terminal)**:
  - Drawing of a thin continuous `│` line over the entire height of the workspace separating the left panel (AI chat) and the right panel (PTY terminal), in a sober shade (`palette.border_unfocused`), highlighted in bold yellow (`palette.warning`) during a mouse drag (`is_dragging_split`).
  - Straight, clean drawing preserving the continuity of the horizontal divider without a jarring T-junction.
- **Visual harmonization of passive code blocks and tool output in the chat**:
  - Systematic framing of passive Markdown snippets and terminal outputs in [`src/ui/chat_panel.rs`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) via a double rounded border container (`render_framed_snippet_box`), reusing the dark bluish frame of the command cards (`Color::Rgb(40, 56, 80)` and `Color::Rgb(28, 42, 62)`).
  - Typed internal header with an adapted icon (`📄` CODE or `⚙` OUTPUT), a framed language or stream badge (`[ RUST ]`, `[ BASH ]`, `[ STDOUT ]`) and an inner box dedicated to the code in bright cyan (`Color::LightCyan`).
  - Modernization of the interactive expand/collapse button for reasoning (`[ ▾ ]` / `[ ▸ ]`) and of the prompt interrupt button (`[ Esc ]`).
- **Realignment of `ARCHITECTURE.md` with the actual implementation (v0.7.x)**:
  - §4: four-level risk taxonomy (`Safe / Standard / Sudo / Risky`) and actual controls (`Alt + 1..9`, `F10`, `Enter` empty field, `F3` levels), replacing the obsolete 3 levels and `[Tab]`=copy.
  - §5: "Planned Providers" → "Implemented Providers", addition of Z.ai/GLM and of the rule "any OpenAI format goes through `providers/openai.rs`" (`reasoning_content` → `<think>`).
  - §6: language fallback corrected to **French** (`fr`) and order of the environment variables aligned with `Language::detect_system()` (`$LC_ALL` → `$LC_MESSAGES` → `$LANG`).
  - §8: compaction described at request time (`compact_chat_messages`, 8 verbatim turns), saving preserving the full history, in place of the old 4-turn compaction executed at save time.
- **Full English harmonization of the project documentation**:
  - `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `CHANGELOG.md` and `docs/plans/2026-08-20_phase4_ssh_detection_and_hosts_cache.md` translated to English and aligned on the code (module map completed, Phase 4 plan marked complete, `Ctrl + Space` focus shortcut, 4-level risk taxonomy, French i18n fallback).
  - `README.fr.md` intentionally stays in French; `README.md`, `CONTEXT.md` and the ADRs were already English.


---

## v0.7.3 — 2026-09-10

### Added

- **New official logo and multi-resolution desktop icon (Spiritty celestial genie)**:
  - Integration of the new high-resolution official logo in [`assets/logo.png`](file:///home/xorne/Projets/Spiritty/assets/logo.png) and [`assets/icons/spiritty.png`](file:///home/xorne/Projets/Spiritty/assets/icons/spiritty.png) (blue-and-gold celestial genie with starry scrolls shaped like an "S").
  - Deployment of the desktop icon in all standard FreeDesktop sizes (`16x16`, `32x32`, `48x48`, `64x64`, `128x128`, `256x256`, `512x512` and the vector SVG `scalable`).
  - Direct embedding in [`src/brand.rs`](file:///home/xorne/Projets/Spiritty/src/brand.rs) (`ICON_PNG` and `ICON_SVG`).
  - Visual integration in `README.md`, `README.fr.md`, the `install.sh` install script and the release CI pipeline.
- **Visual redesign and framed container for command cards (faithful to the website design)**:
  - Complete redesign of [`render_command_card`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) and [`compose_approval_card`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs):
    - Outer surrounding container with rounded corners (`╭─...─╮`, `│...│`, `╰─...─╯`).
    - Internal header: amber lightning bolt `⚡` and `COMMAND #N` with a framed sober risk badge (`[ SAFE ]`, `[ RISKY ]`).
    - Inner framed box dedicated to the code with text in bright cyan (`Color::LightCyan`).
    - Clean card footer: status label (`"Validation required before execution"`) on the left and amber `[ Alt + N ]` key button on the right (without the verbose `"Execute"` suffix).
    - Harmonization of the tool approval card with framed keys `[ F10 ]` and `[ Esc ]`.

### Fixed

- **Mouse selection and copy of the last line of the multiline prompt**:
  - Fix in [`src/ui/mod.rs`](file:///home/xorne/Projets/Spiritty/src/ui/mod.rs) of the selection rectangle calculation (`inner.height = panel_area.height.saturating_sub(1)` instead of `saturating_sub(2)`).
  - Previously, the double border subtraction (`height - 2` then `clamp(..., bottom - 1)`) truncated the selection at `panel_area.bottom() - 2`, preventing selection and copy of the 3rd line of a multiline prompt (or of any lower line of the prompt and of the terminal).
  - Mouse selection can now cover, without truncation, the entire multiline prompt down to the last line (`panel_area.bottom() - 1`).

---

## v0.7.2 — 2026-09-10

### Fixed

- **Isolation of test sessions and elimination of session history pollution**:
  - Fix of a side effect where running the test suite (`cargo test`) recorded temporary test sessions directly in `~/.config/spiritty/sessions/` without deleting them.
  - The `spiritty -c` command then resumed these orphan test sessions (e.g. `Session #141311` with the dummy command `sed -n '1,10p' Cargo.toml`) instead of the last real user conversation, and polluted the session manager.
  - Addition of support for the `SPIRITTY_SESSIONS_DIR` variable in [`src/session/storage.rs`](file:///home/xorne/Projets/Spiritty/src/session/storage.rs) to isolate the tests, systematic deletion of test artifacts, and purging of orphan test sessions.
- **Robustness of Markdown code block splitting and elimination of false command proposals**:
  - Fix of a subtle bug in [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) and [`src/ui/chat_panel.rs`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) where the presence of triple backticks in a string literal inside a code block (e.g. `remaining.find("```")`) or of inline backticks in the conversational text prematurely truncated the block and interpreted ordinary words of the explanatory text (e.g. the word "Pour") as executable shell command proposals.
  - Implementation of rigorous syntactic parsers `find_opening_code_fence` and `find_closing_code_fence` verifying that an opening or closing fence starts at the beginning of a line (with optional spaces) and has a valid info tag, ignoring inline backticks or those contained in string literals.
  - Improvement of `repair_prematurely_closed_code_blocks`: when a prematurely closed empty block is detected, if the following text already contains valid fences, the empty fence is removed without re-wrapping the following textual explanations in a false bash block.
  - Strengthened filtering in `is_clean_command_line` rejecting isolated conversational linking words ("pour", "suite", "attention", "voici", "cela").
- **Auto-approval of command proposals according to the configured level (Safe / Sudo / Yolo)**:
  - Fix of an inconsistency where shell commands proposed by the assistant as an interactive Markdown block (`⚡ COMMAND #1`) systematically enforced manual validation (`Alt + 1`), even when the active approval mode (`F3 Safe` or `F3 Sudo`) explicitly authorized the command's risk level (e.g. read-only `Safe` commands such as `sed`, `cat`, `grep`, `df` or administrative `Sudo` commands).
  - Automatic evaluation in [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) (`on_agent_done`) of eligible single proposals via `should_auto_approve_command(&cmd, level)` with injection and direct execution in the PTY terminal.
  - Strict preservation of user control: destructive commands (`Risky`) always require manual validation (except in `Yolo` mode), multiple alternative proposals wait for the user's choice, and a guardrail caps automatic sequences at a maximum of 10 consecutive executions with an i18n toast notification (`AutoApproveMaxConsecutiveReached`).
- **Restoration of the system prompt for OpenAI-compatible providers (DeepSeek, Grok, GLM, LM Studio)**:
  - Fix of a critical variable-shadowing bug (`let mut api_messages = Vec::new()`) in [`src/agent/providers/openai.rs`](file:///home/xorne/Projets/Spiritty/src/agent/providers/openai.rs) that overwrote and reset the message list to empty right after inserting the system message.
  - DeepSeek models again receive Spiritty's full system prompt (identifying the role, the split-screen terminal, the `tool:run_command`, `tool:web_search`, `tool:read_file`, etc. tools) and can browse the web and inspect the system instead of refusing while claiming they have no Internet access.
  - Extraction of the pure function `build_api_messages` covered by a dedicated unit test suite.
- **Context window detection for the entire DeepSeek range (131k tokens)**:
  - Fix of the maximum context window detection in [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs): the test restrictively targeted `model.contains("deepseek-v4")`, causing the degradation of `deepseek-flash`, `deepseek-chat` and `deepseek-reasoner` to the default local window of 8.2k tokens.
  - Broadening to `model.contains("deepseek")` guaranteeing the full window of 131,072 tokens (131k) for all models in the DeepSeek family.
- **Hardening and securing of tool block parsing (`tool:run_command`, `web_search`, `read/write/edit_file`)**:
  - Fix of a critical behavior in [`src/agent/tools.rs`](file:///home/xorne/Projets/Spiritty/src/agent/tools.rs) where the absence of a closing delimiter ``` or the inline mention of a tool name in an explanatory sentence caused the capture of the entire remainder of the Markdown response as a shell command executed live in the PTY.
  - Strict requirement of a start of line (with optional indentation), of a clean header line terminated by a newline, and of a mandatory closing delimiter ``` (or `</tool:...>`).
  - Filtering of false positives: systematic rejection of placeholder commands (`...`, `<command>`, `<unit>`, `cmd`, `commande`) and of blocks truncated midway.
  - Adjustment in [`src/ui/chat_panel.rs`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) and [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) to preserve conversational text mentioning tools without arbitrarily truncating it.

### Added

- **Support for the DeepSeek V4.1 Flash model (`deepseek-flash`) and pricing alignment (Sept. 10, 2026)**:
  - Alignment with the real DeepSeek API identifier: although the announcement is titled "V4.1 Flash", the official gateway `api.deepseek.com` only accepts `deepseek-flash`, `deepseek-v4-flash` and `deepseek-v4-pro` (rejecting `deepseek-v4.1-flash` with an HTTP 400).
  - Definition of `deepseek-flash` as the official default model in [`src/config/mod.rs`](file:///home/xorne/Projets/Spiritty/src/config/mod.rs).
  - Automatic and systematic mapping of any selection or entry of a `v4*` model (`deepseek-v4`, `deepseek-v4-flash`, `deepseek-v4-pro`, `deepseek-v4.1-flash`) to `deepseek-flash` on configuration load, in the configuration modal (F2/Ctrl+P) and on the network in [`src/agent/providers/openai.rs`](file:///home/xorne/Projets/Spiritty/src/agent/providers/openai.rs).
  - Update of the official rates in [`assets/pricing.json`](file:///home/xorne/Projets/Spiritty/assets/pricing.json) and [`src/pricing/mod.rs`](file:///home/xorne/Projets/Spiritty/src/pricing/mod.rs) ($0.30 / 1M input cache-miss, $1.20 / 1M output at full peak rate, with a dynamic reduction to $0.15 / $0.60 during off-peak hours and weekends).

---

## v0.7.1 — 2026-09-09

### Added

- **Configurable thinking / reasoning level per provider (`ReasoningEffort`)**:
  - **Interactive selector in the configuration modal (`F2`)**: new field `5. AI Reasoning ❯ [←] Badge [→]` allowing the model's reasoning level to be adjusted on the fly among 5 notches: `Default` (model default), `Off` (disabled for maximum speed), `Low` (low, ~1k token budget), `Medium` (medium, ~4k token budget) and `High` (high, ~16k token budget), with dynamic colors and localized descriptions (FR/EN).
  - **Multi-provider API integration**:
    - *Google Gemini*: transmission of `generationConfig.thinkingConfig.thinkingBudget` (0 to disable, 1024, 4096, 16384 or omitted by default).
    - *OpenAI / Compatible*: transmission of the standard `reasoning_effort` parameter (`low`, `medium`, `high`) in completion requests.
    - *Anthropic Claude*: activation of `thinking: { type: "enabled", budget_tokens: ... }` with automatic calculation of the `max_tokens` ceiling (up to 20480 tokens) and transparent folding of `thinking_delta` blocks into reasoning tags `<think>...</think>`.
  - **Per-provider persistence**: clean recording in `~/.config/spiritty/config.toml` under each provider (`reasoning_effort = "..."`), automatically omitted when set to `default`.
  - **Visual indicator in the status bar**: real-time display of the reasoning level directly to the right of the model name (` 🧠 Auto `, ` 🧠 Off `, ` 🧠 Low `, ` 🧠 Med `, ` 🧠 High `) with a dedicated color code and responsive adaptation to the terminal width.

- **Dynamic live refresh of models and LLM pricing (`R` / `Ctrl+R` / `F5` in the Configuration modal `F2`)**:
  - **Dynamic multi-provider discovery**: non-blocking asynchronous querying of each provider's official APIs (Google Gemini via `/v1beta/models`, Anthropic via `/v1/models`, Ollama via `/api/tags`, OpenAI-compatible providers via `/v1/models`) with instant consideration of the API keys and custom URLs entered in the modal.
  - **Automatic update and persistence**: immediate recording of the newly discovered models in the user's configuration (`~/.config/spiritty/config.toml`), update of the interactive model selector and simultaneous triggering of the token pricing grid update.
  - **Visual status feedback and full i18n**: interactive badge `[ R ] Refresh models` in the modal footer with dynamic status messages (query in progress, success with the number of models discovered, or explicit cause of failure in case of a missing API key or a network error) in French and English.
  - **Responsive and widened display of the modal footer**: widening of the modal width (`clamp(88, 130)` columns) and reactive layout (airy single line on a wide screen, or balanced two lines on a compact screen) guaranteeing full visibility of all buttons and shortcuts (`Tab / ↑↓ Navigate`, `Ctrl+S Save`, `R Refresh models`, `U Online pricing`, `Esc Close`) without any overflow or truncation.
- **Support for the `gemini-3.8-flash` model for Google Gemini**:
  - Addition of `gemini-3.8-flash` as the default model and at the top of the list of recommended models for Google Gemini.
  - Integration of the pricing grid ($0.75 input / $3.75 output per million tokens) in the dynamic registry and the embedded fallback (`assets/pricing.json`).
- **Robustness of interactive ncurses & full-screen applications (`vim`, `nano`, `htop`, `fzf`, `lazygit`, `less`)**:
  - **Native SGR Mouse Reporting**: dynamic detection of the xterm/SGR mouse protocol (`\x1b[?1000h` / `\x1b[?1006h`) and instant retransmission of clicks, releases, drags and wheels to the PTY process (`htop`, `vim` with `:set mouse=a`, `fzf`). Holding the `Shift` key bypasses the application mouse to select and copy text locally.
  - **Smart wheel scrolling in the alternate screen**: when a full-screen application runs without a mouse protocol (`less`, `man`, `vim`), the mouse wheel automatically emits arrow keys to the PTY instead of scrolling an empty scrollback history.
  - **Extended xterm encoding of navigation keys with modifiers**: full support for `Ctrl+Arrows` (word jump in readline/zsh/nano), `Shift+Arrows`, `Alt+Arrows`, `Ctrl+Home`/`End`, `Ctrl+Delete`, and function keys `F1..F12` with modifiers.
  - **No interception of `PageUp` / `PageDown` and `Ctrl+V` in the alternate screen**: the `PageUp` and `PageDown` keys now reach the active application directly. `Ctrl+V` (without Shift) is transmitted to `vim` to trigger visual block selection (`^V`), while `Ctrl+Shift+V` remains dedicated to pasting.
  - **Native support for Bracketed Paste (`\x1b[?2004h`)**: automatic wrapping of pasted text with the VT markers (`\x1b[200~` ... `\x1b[201~`) avoiding staircase indentation effects in `vim`/`nano` and the premature execution of multiline commands.
- **Tab renaming and multi-tab persistence in sessions (`Alt+R`)**:
  - **Dedicated rename modal (`Alt+R`)**: allows assigning a clear business label to each tab (e.g. `bdd-prod`, `logs-nginx`) with full i18n (FR/EN) or returning to the default dynamic title with an empty input. Shortcut documented in the help modal (`F1` / `?`).
  - **Persistence of custom titles and SSH contexts**: tabs and their labels are saved transparently within the session file (`Session.tabs`) and faithfully reapplied when a session is reloaded.

### Changed

- **Ultra-compact display of sessions in the status bar and guaranteed spacing**:
  - Replacement of the old verbose session restore message (which contained the full title, the message count, the SSH status and the redundant `⚡ Sudo` approval reminder) by an ultra-short label: `📂 Session #151237` (~17 characters in total).
  - Elimination of text collisions in the footer: reservation of a guaranteed minimal spacing of at least 2 spaces between the left metrics and the right shortcuts (`build_right_shortcuts`), avoiding any overlap or text adjacency (`SudoApproval`).
- **Architecture: unification and deepening of the TUI modals ([`src/ui/components/`](file:///home/xorne/Projets/Spiritty/src/ui/components/))**:
  - Centralization of key handling (`handle_key`), clipboard pasting (`handle_paste`) and visual rendering (`render`) of all 7 modals (`Help`, `Config`, `Sessions`, `Bookmarks`, `Export`, `Mcp`, `RenameTab`, `SshReconnect`) within the unified enum `ModalState` and the transition state machine `ModalOutcome`.
  - Elimination of more than 280 lines of logic scattered between `src/app.rs` and `src/ui/mod.rs`, reducing ad-hoc blocks to simple composable and isolatedly testable delegations.
- **Architecture: creation of the system and SSH hosts supervisor ([`src/system/supervisor.rs`](file:///home/xorne/Projets/Spiritty/src/system/supervisor.rs))**:
  - Complete encapsulation of the `/proc` monitoring of foreground processes (`SystemSupervisor`), of SSH/Docker/local session detection, of current directory and Git branch synchronization, as well as the background execution of distribution inspection probes (`ssh -o BatchMode=yes ...`).
  - Decoupling of the tabs via the `InspectableTab` trait allowing testing of process detection, multi-tab state and system profiling without depending on the TUI loop or the real PTY. Active deduplication of in-flight SSH probes to avoid any network saturation.
- **Architecture: extraction and deepening of the `ToolCapture` subsystem ([`src/pty/capture.rs`](file:///home/xorne/Projets/Spiritty/src/pty/capture.rs))**
  — substantial lightening of `src/app.rs` (-690 lines) by the complete encapsulation of tool capture, incremental UTF-8 decoding with carry buffer, sentinel scanning (`OSC 777`), the 1 MiB overflow ceiling, detection of interactive prompts (`InteractionKind`) and clean `SIGINT` cancellation within a pure `ToolCaptureSession` state machine that is 100% testable in headless mode.

- **Elimination of hangs on interactive pagers (`systemctl`, `journalctl`, `git`, `less`)**:
  - **Automatic injection of `--no-pager` (`ensure_non_interactive_command`)**: systematic detection and injection of the `--no-pager` option when executing system commands (`systemctl`, `journalctl`, `git log/diff/show/branch`) both locally and in an SSH session or under `sudo`. Prevents status or inspection commands from launching `less` in the background and blocking the terminal on a `lines ... (END)` prompt.
  - **Detection and auto-acknowledgement of pagers (`InteractionKind::Pager`)**: extension of interactive prompt detection in `src/pty/capture.rs` (`lines ... (END)`, `--More--`, etc.) and immediate automatic sending of the `q` character to the PTY to release the terminal without requiring manual user intervention.
  - **Filtering of status residue in PTY output**: removal of pager status lines (`lines 1-25/25 (END)`) in `clean_pty_output` so as not to pollute the context returned to the AI agent.
  - **Explicit rule in the system prompt**: formal instruction to the AI agent to avoid interactive commands and to always append `--no-pager` or redirect to `cat`/`head`.
- **Streaming reasoning display for Google Gemini (`includeThoughts: true`)**:
  - Transmission of the `includeThoughts: true` parameter within `thinkingConfig` in requests to the Google Gemini API (when `ReasoningEffort` is configured to `Low`, `Medium`, `High` or `Default`). Without this explicit flag, the Gemini API omitted the reasoning fragments (`thought: true`) from the SSE stream, preventing the display of the animated line `💭 Reasoning · ...` in the chat window.
  - Clean closure of the reasoning block via `bracket.finish()` at the end of the stream when a response ends with a reasoning phase.
- **Auto-approval of multiline and chained diagnostic commands (`Safe` / `Sudo`)**:
  - **Intelligent splitting of command chains (`split_chained_commands`)**: full support for newlines (`\n`), logical operators `&&`, `||`, semicolons `;` as well as subshell parentheses `(...)` commonly generated by LLM models during complex diagnostics.
  - **Enrichment of the safe inspection catalog (`safe_prefixes`)**: automatic classification as `Safe` of the usual diagnostic binaries and options (`nproc`, `free`, `php -v/-m/-i`, `apache2 -v`, `apachectl -M/-S/-v`, `httpd`, `mysql --version`, `mariadb --version`, `dpkg -l/-s/--list`, `awk`, `sed` read-only without `-i`). System audits are no longer falsely downgraded to commands subject to manual validation.
- **Secure-by-default auto-approval at `Safe`**:
  - Replacement of the legacy behavior where `auto_approve = true`, `all` or `auto` activated `Yolo` mode. The boolean value `true` or the string `auto` now strictly switches to `Safe` (read-only commands auto-approved, system modifications subject to confirmation). Only the explicit value `yolo` activates YOLO mode.
  - Resuming a session containing a YOLO level no longer overwrites the global configuration file `config.toml`, protecting the user's default setting between sessions.
- **Persistence and memorization of the last selected model**:
  - The model refresh (`R`) immediately synchronizes the model currently entered or selected in the configuration before writing to disk, preventing any regression to a previous model.
  - Navigation between different providers (`←`/`→`) within the `F2` modal keeps the models and parameters chosen for each provider in a buffer, persisting all the modifications on save.
- **Consideration of manual modifications in `config.toml`**: removal of the unconditional reassignment of the model/provider from the last session at startup. `config.toml` becomes the source of truth again at Spiritty startup, session restoration being reserved for the explicit `-c` / `--continue` option.
- **Automatic synchronization of recommended models**: `Config::load` now automatically merges the new popular models (such as `gemini-3.8-flash`) at the top of the list in existing configuration files, while preserving the user's custom models.
- **Addition of a custom model in the configuration modal (`F2`)**: confirmation of the addition (`Enter`) cleanly closes the input submenu to directly display the selected model, and saving (`Ctrl+S` / `F2`) automatically records the model in the provider's `models` array.

---

## v0.7.0 — 2026-09-03

### Added

- **Interactive multi-tabs in the split-screen terminal (`Ctrl+T` / `Ctrl+W` / `Ctrl+Tab` / `Alt+1..9`)**
  — native support for several PTY sessions and servers in parallel in the right panel:
  - **Tab management**: quick creation with `Ctrl+T` (or click on `[+]`), closing with `Ctrl+W` (or click on `×`), sequential navigation `Ctrl+Tab` / `Ctrl+Shift+Tab` / `Ctrl+PgUp` / `Ctrl+PgDn`, and direct selection `Alt+1..9`.
  - **Dynamic tab bar & indicators**: display of each tab with its contextual title (`1: 💻 local`, `2: 🌐 vps-prod`, etc.) and `●` dot in case of activity or background output.
  - **Contextual synchronization with the AI agent**: switching tabs immediately updates the active system context (`SSH`, `Docker`, `local`, working directory, Git branch), ensuring the AI agent always assists the environment visible on screen.
  - **Non-blocking multitasking**: each tab maintains its own PTY process and its own VT100 virtual screen in the background without blocking the interface.

- **Persistence and restoration of the approval mode (`auto_approve`) per session**
  — the active automatic approval level (`Safe`, `Sudo`, `YOLO`, `Off`) is now saved with the session (`~/.config/spiritty/sessions/`) and faithfully restored during a `spiritty -c` or when loading a session via `Ctrl+H`. The sessions modal displays the associated visual indicator and the command-line options (`--yolo`, `--safe`, `--auto-approve <lvl>`) keep absolute priority in case of forced resume.

- **Native support for the Gemini 3.7 / 2.5 reasoning stream (`thought: true`)**
  — automatic wrapping of the Google Gemini provider's reasoning chunks into `<think>…</think>` blocks for a collapsible and animated visual rendering identical to the DeepSeek / OpenAI models.

- **Automatic logging of crashes and panics in `~/.config/spiritty/crash.log`**
  — systematic recording of fatal errors with timestamp and full backtrace.

- **Editing and management of remote files in SSH session & containers (`tool:read_file` / `edit_file` / `write_file`)**
  — the dedicated file tools now work transparently and securely
  on remote machines during an active `SSH`, `Docker` or `Podman` session:
  - `tool:read_file`: remote reading via silent `base64` pipeline in the PTY stream with
    memory decoding and handling of the safety ceiling (100 KB).
  - `tool:edit_file`: prior reading of the remote file, strict validation of the uniqueness
    of the `old_string` fragment in memory on the Rust side, and atomic rewriting via `base64 -d | [sudo tee]`.
  - `tool:write_file`: full remote write/overwrite with Base64 encoding and automatic `sudo tee`
    elevation on system paths (`/etc/`, `/var/`, `/usr/`, etc.).
  - Preservation of human-in-the-loop safety: risk classification (`Safe`, `Standard`,
    `Sudo`, `Risky`) and user confirmation requests respected on all remote paths.

- **Automatic detection of interactive prompts and focus switching (`[y/n]`, confirmation, passwords)**
  — extension of the PTY stream detector to identify not only `sudo`/password requests, but also common interactive confirmations (`[y/n]`, `[o/n]`, `(yes/no)`, `Press [Enter] to continue`, `Are you sure you want to continue connecting`). When the AI agent triggers a command requiring a human response, the keyboard focus automatically switches to the terminal with an explicit toast and the inactivity delay is increased to 120s to give the user time to respond.

- **In-progress execution indicator and interaction prompt (`⚡ In progress · Shift+Tab`) in the terminal**
  — during execution of a tool by the AI agent, the terminal header bar now displays a visible badge signaling that a command is active and recalling the universal `Shift+Tab` shortcut to instantly switch into the shell and interact.

### Fixed

- **Automatic scrolling to the bottom of the terminal when injecting tools and commands**
  — if the terminal was scrolled up (history), the terminal display remained frozen on the old lines during execution of the agent's tool commands. Scrolling is now automatically reset (`reset_scroll`) as soon as a command is injected, immediately making the live output visible.

- **Clean interruption of suspended PTY processes when stopping generation (`Esc` / `Ctrl+C`)**
  — when a command was blocking in the terminal (for example an `ssh` waiting or an interactive script), pressing `Esc` or stopping generation cancelled the task in Spiritty but left the process active in the background in the terminal. Spiritty now injects an interrupt signal `\x03` (SIGINT) into the PTY to cleanly kill the command and immediately return the prompt to the user.

- **Fix of the tab close shortcut and preservation of word erase (`Ctrl+W`)**
  — in the terminal and the chat input, the combination `Ctrl+W` traditionally serves to erase the previous word (`werase`). The global interception of `Ctrl+W` caused Spiritty to close unexpectedly when only one tab was active. The tab close shortcut is now `Ctrl+Shift+W` (or click on `×`), `Ctrl+W` performs word erase, and closing a tab no longer quits the application when the last tab is active.

- **Isolation of multiline scripts and commands containing `exit` / `set -e` in a subshell (`bash -c '...'`)**
  — when an AI script proposal contained `exit 1` or `set -e` (for example a test script with `if [ -z "$KEY" ]; then exit 1; fi`), its direct execution in the interactive shell killed the root process of the PTY (`$SHELL`), causing Spiritty to close suddenly. These scripts are now automatically wrapped in an isolated subshell, preserving the interactive session and cleanly capturing the output and return code without quitting Spiritty.

- **Full support for DeepSeek DSML tool markup (`<skill>`, `<command>`)**
  — DeepSeek models emitting custom XML tool calls are now correctly interpreted as interactive command proposals and their technical tags are filtered from the chat view.

- **Robustness of reasoning block splitting and tag variants (`</thunk>`, `</thought>`, `</thinking`, `</th`)**
  — some reasoning models (DeepSeek, GLM, Grok) sometimes emit variants of end-of-thought tags (typo `</thunk>`, `</thought>`, `</thinking` without a closing chevron or partial cut `</th`) while placing a closing `</think>` at the very end of the message after the tool call. The parser considered the entire message (including the response text and the DSML tool invocation) as part of the private reasoning, hiding the response and leaving the TUI frozen on *Deep thinking*. Thought extraction and tool splitting now handle all these formatting anomalies and guarantee the immediate extraction of command proposals.

- **Support for compound approval phrases (`oui vas y`, `ok vas y`, `oui stp`) and unblocking of tools**
  — when a tool approval request was pending, common compound expressions such as `oui vas y` or `ok vas y` were not recognized as a validation, and sending a new message left the background task blocked waiting for consent. The natural approval parser now handles all common locutions and cleanly releases the task in progress if a new directive is entered.

- **Restoration of the universal `💭 Deep thinking…` indicator and the active reasoning shimmer**
  — the thinking animation and the real-time reasoning timer remain visible throughout the model computation (including before the first token is received and during the reasoning).

- **Resolution of SSE timeouts (increased to 45s connection / 90s stream) for reasoning models**
  — reasoning models (DeepSeek-R1 / V3, Gemini 3.7 Thinking, Claude 3.7 Thinking, o3-mini) and long sessions under heavy load caused premature errors `25s inactivity timeout exceeded on the model stream (SSE timeout)`. The timeouts have been raised to 45s for the connection and 90s for the thought streaming on all providers (OpenAI, DeepSeek, Gemini, Anthropic, Ollama).

- **Optimization of LLM context compaction for very long sessions (200+ turns)**
  — large sessions saturated the token budget and lengthened the TTFT:
  - **Filtering of transient errors**: automatic removal of residual error messages (`⚠️ Error: ...`) during preparation of the conversation sent to the API.
  - **Trimming of giant command outputs**: large raw outputs (> 6,000 characters) are automatically summarized while preserving the beginning and end of the output (`[output truncated for the LLM context]`).
  - **Capping of the history summary**: limiting the summary of older turns to 25 key points to guarantee minimal latency.

## v0.6.4 — 2026-08-30

### Fixed

- **Resolution of API key environment variables at GUI launch**
  — when launching Spiritty via a desktop application launcher (without going
  through an existing interactive terminal), the API keys declared in `~/.zshrc` or
  `~/.bashrc` (`export GEMINI_API_KEY=...`, `DEEPSEEK_API_KEY`, etc.) were not
  loaded because the probe executed the shell in non-interactive login mode (`-l`). The probe
  now executes the shell in interactive login mode (`-l -i`) with tight delimiters,
  guaranteeing transparent loading of the keys configured in your shell rc.
- **Install script (`install.sh`): absolute path of the executable in the XDG launcher (`Exec`)**
  — the generated `spiritty.desktop` file contained a relative `Exec=spiritty`. When Spiritty
  is installed in `~/.local/bin` (user installation without sudo), the desktop launchers
  and terminal emulators (Ghostty, etc.) failed with the error `Failed to find executable spiritty`
  because `~/.local/bin` is not present in the global `$PATH` of the graphical session. The script
  now uses the exact absolute path `${INSTALL_DIR}/${BINARY_NAME}`.

## v0.6.3 — 2026-08-30

### Added

- **Install script (`install.sh`): desktop detection and creation of the XDG launcher**
  — on Linux, the installer now detects the active desktop environment
  (GNOME, KDE Plasma, XFCE, Hyprland, Sway, DankMaterialShell / DMS, etc.) and
  interactively offers to install:
  - The SVG icon in `~/.local/share/icons/hicolor/scalable/apps/spiritty.svg`
  - The launcher `~/.local/share/applications/spiritty.desktop`
  - The automatic refresh of the launcher databases and icon
    caches (`update-desktop-database`, `gtk-update-icon-cache`, and restart
    of the `dms` service if active).
- **CI packaging (`release.yml`)**: the release tarball archive now includes
  the icon `assets/icons/spiritty.svg`.

## v0.6.2 — 2026-08-30

### Added

- **Official embedded SVG icon + `brand` module** — the "genie lamp"
  icon `assets/icons/spiritty.svg` is pushed as the repository's brand asset
  and **embedded** in the binary via `include_str!` (new `src/brand.rs`:
  `BRAND_GLYPH`, `brand_title()`, `ICON_SVG`). The chat panel title uses
  `brand::brand_title()` and the whole app shares a single marker. The TUI
  does not display the SVG (a terminal cannot draw a vector) — the emoji
  🧞 remains the in-TUI marker.
- Documentation aligned with the 🧞 glyph (install.sh, README, README.fr,
  ROADMAP).

## v0.6.1 — 2026-08-30

### Changed

- **Brand emoji: ghost → blue lamp 🧞** — the ghost `👻` becomes the
  **blue lamp 🧞** (reference to the "genie lamp"), everywhere in the app:
  chat panel title, assistant response prefix, CLI help, exported Markdown
  reports and session summaries.

## v0.6.0 — 2026-08-30

### Added

- **Image / screenshot pasting for vision models (`Ctrl+Shift+V`)**
  — image reading from the clipboard (arboard `get_image` → RGBA pixels →
  PNG → base64), attached to the next user prompt. The flow:
  - `Ctrl+Shift+V` triggers an asynchronous read (dedicated thread, anti-stacking
    guard) and stores the image in `pending_image` — the toast
    "🖼️ Image attached…" confirms, then the image is attached to the next send.
  - `ChatMessage.attachments` (`Vec<MessageAttachment>`, `mime_type` +
    `data_base64`) carries the image; `.data_uri()` exposes the form
    `data:<mime>;base64,…`. ⚠️ backward-compatible: JSON sessions without the
    `attachments` key still deserialize (`#[serde(default)]` attribute).
  - `prepare_conversation` now keeps an "image-only" turn (empty text +
    attachment) instead of discarding it as empty — the provider does receive
    the capture.
  - **Three vision providers**: `openai.rs` (`image_url` block + data-URI),
    `gemini.rs` (`inline_data` part, base64 without prefix), `anthropic.rs`
    (`image.source` block base64). The text remains a `text` part to satisfy
    APIs that reject a 100% image turn.
  - New dependencies: `base64` and `image` (feature `png`).
  - Tests: RGBA PNG + base64 encoding, JSON schema of the three vision blocks,
    data-URI, and survival of an attachment-only turn in `prepare_conversation`.

- **TUI preview of the pasted image (half-blocks)** — the pending image is
  rendered live above the input area with half-blocks (`▀` = 2 pixels
  per cell, top = fg, bottom = bg), nearest-neighbour downsampling
  (letterboxed, ratio preserved, ~32×8 cells). No image widget or dependency
  added. A status line displays the dimensions and the shortcuts:
  `[Enter] sends it` · `[Ctrl+Shift+⌫] removes it`. `pending_image` now carries
  a `PendingImage` (the `MessageAttachment` for sending + the RGBA pixels
  decoded once at paste time, no re-decoding per frame). Alpha
  flattened on a dark background. Tests `render_halfblock_marks_cells…` /
  `flatten_over_dark…`.

- **Smart image/text `Ctrl+V` + fixed Wayland image reading** — image
  pasting is now done via **`Ctrl+V`** (global, active from both
  panels), and not `Ctrl+Shift+V` which Ghostty and most terminal emulators
  capture before the app. Three fixes:
  - **`wayland-data-control` feature enabled** on `arboard` (pulls
    `wl-clipboard-rs`) — under Wayland, `arboard::get_image()` failed
    silently for lack of this feature (arboard fell back to the X11 backend,
    which does not see the Wayland copy), hence the paste of the path instead
    of the thumbnail.
  - **`Ctrl+V` moved into `handle_key`** (global) instead of the chat panel
    only: in Terminal focus it was sent as-is to the PTY, injecting the image
    path into the shell (local or **remote SSH**) — worse on a VPS.
    `handle_terminal_key` no longer sends `Ctrl+V` to the PTY.
  - **`spawn_smart_paste_request`**: reads the clipboard once (arboard),
    priority to the image (`get_image`) → `PasteImage` (preview), otherwise text →
    `Paste` (pasted into the active panel). Confirmed diagnostics: the clipboards
    contain both image pixels (392×575) and a file URI
    — the image is indeed read first.
  - **`Ctrl+Shift+V` captured by the terminal = image anyway**: Ghostty
    converts `Ctrl+Shift+V` into a **text** paste (the file URI/path).
    `handle_paste` now detects that the pasted text is an image path/URI
    (`looks_like_image_path`) and re-reads the clipboard to attach
    the image instead of pasting the path into the panel (worse on a VPS).
    New `PasteInto` event (insertion without re-detection) to avoid any
    recursion. Tests `image_path_detection`.

### Added

- **Dedicated file editing tools** (`tool:read_file` / `tool:edit_file` /
  `tool:write_file`) — the model fell back on `sed`/`awk`/heredoc pipelines
  to modify a file, a source of corruption (mangled heredocs) and
  of 127 errors on real sessions. Three structured tools, dispatched
  in the tool loop (`src/agent/mod.rs`) and documented in the system
  prompt (`src/agent/prompt.rs`):
  - `tool:read_file`: displays the file (truncated to 100 KB with a counter), auto-approved (read-only).
  - `tool:edit_file`: replacement of a unique exact string (`old`→`new`, separator `---`), atomic — fails loudly if the text cannot be found **or** appears several times, instead of applying a partial substitute.
  - `tool:write_file`: write/overwrite of a complete file, verbatim content (never ellipsis/placeholder).
  - The parser (`parse_file_edit_fence`, `src/agent/tools.rs`) refuses an `edit_file` with an empty `old_string` and ignores fences inside `<think>` blocks.
  - **Classification by path** (`classify_file_edit`, `src/agent/safety.rs`):
    user files/`~/.config`/projects → Standard (auto-approved), `/etc`,
    `/usr`, `/var`, `/root`, `/boot`... → Sudo (requests confirmation outside the Sudo level),
    sensitive paths (`.ssh`, `.zshrc`, `fstab`, `sudoers`, `ssh` config) → Risky
    (only auto-approved in YOLO).
  - Tests: fence parsing (read/write/edit, multiline, `<think>`, empty old),
    classification by path, and read/write/edit round-trip (unique,
    ambiguous, not-found replacement).
  - **Refusal in remote session (SSH/container)**: the editing tools act on
    Spiritty's **local** system, not on the remote server. The tool loop
    now detects the session via `sys_ctx.active_session` (SSH/container) and refuses
    `read_file`/`edit_file`/`write_file` with an explicit message inviting a return
    to shell commands (`cat`/`sed`/`tee`/heredoc/`scp`) via `tool:run_command` —
    rather than silently editing a local file that is not the one the
    user is viewing. Rule added to the system prompt.

### Fixed

- **SSH detection from the shell panel: false `Ssh` forged on an invalid target**
  — `parse_ssh_args` (`src/system/process_watcher.rs`) accepted any
  non-flag first token as an ssh target. A foreground process whose
  `argv[0]` resolves to `ssh` but with a non-target argument (duration/`sleep`,
  isolated number, `-N` without destination, reaper) produced a false
  `ActiveSession::Ssh { target: "30", host: "30" }`, skewing the
  Local↔SSH detection in the race. Addition of a strict `is_valid_ssh_host` validation: the
  target must be a hostname (letters/digits/`.`/`-`/`_`, not purely
  numeric), an IPv4, or an IPv6 (between `[]` or with `:`). Tests
  `test_ssh_without_valid_target_is_rejected` / `test_ssh_with_valid_targets_is_accepted`.

- **SSH reconnect modal wrongly offered on a local session** — the case
  where the model *emits* an `ssh …` command (proposal/example) caused the
  reconnect modal to appear when reloading a session that was in fact
  local. The `extract_ssh_target` heuristic (`src/session/mod.rs`) considered
  any message `💻 \`ssh …\`` as proof of a remote session, whereas it
  proves nothing (the command may simply have run, or the `ssh` followed
  by an `exit`). The only reliable proof that a session ended on a remote
  shell is the **prompt remainder** (`user@host:~$`): the
  "bare `ssh` command" signal is removed, the inference is now made only via a
  remote prompt. Test `infers_target_from_ssh_command_message` renamed
  `bare_ssh_command_message_does_not_infer_target`, and `newest_message_wins`
  rewritten on remote prompts.


- **`<think><think>` nested in the displayed reasoning** (session audit
  20260830) — some reasoning models (GLM/Z.ai/DeepSeek) emit their
  own `<think>` **inside** the visible content, in addition to the wrapper that Spiritty
  adds for `reasoning_content`: the reasoning extractor
  (`extract_thought_block`, `src/ui/chat_panel.rs`) then captured `<think>…`
  with the literal tag at the head. The extracted `thought` is now passed through a
  `strip_residual_reasoning_tags` cleanup that removes all residual
  reasoning delimiters (`<think>`/`<thought>`/`<reasoning>` and their closings)
  — the deliberation is displayed verbatim, the normal case of a single block remains
  unchanged. Test `nested_think_keeps_reasoning_verbatim`.

- **Multiline heredocs mangled by the local interactive line-editor (zsh/bash)**
  (session audit 20260830) — heredoc scripts (`sudo tee … <<'EOF'` with a
  body over several physical lines) were injected raw into the PTY:
  the local shell's line editor split the file into several lines and
  lost the body (fragments "`cmdand heredoc> =`", `<<''EOF'>`,
  letter duplication observed on real sessions). Now
  `format_command_for_pty_with_session` routes commands containing a heredoc
  (`<<`) into `bash -c '…'` for **any local shell** (zsh/bash/dash, not
  only fish): the whole block becomes ONE single logical line for the interactive
  editor, and bash executes the script verbatim. Remote shells remain
  unchanged (their editor handles multiline, wrapping risked changing the
  semantics), and the "bare" bash syntax without heredoc remains native for bash.
  Tests `format_command_for_pty` extended (local zsh heredoc → wrap, remote
  heredoc → native, non-heredoc bash syntax → native).

- **Command proposals extracted from the model's `<think>` wrongly executed**
  (session audit 20260830) — the proposal extractor
  (`extract_all_command_proposals`, `src/app.rs`) scanned the code fences
  within the `<think>…</think>` reasoning block, which the tool-call parser
  already removed (`strip_think_blocks`, `src/agent/tools.rs`) but it did
  not. The reasoning often contains *example* fences that are not executable,
  which became ⚡ cards and were executed in place of the
  real command: real case "automount `/dev/sdb1`" where only the heredoc
  content line (`UUID=… /mnt/data …`) was injected (`code 127`) in
  place of `sudo mkdir … printf … | sudo tee -a /etc/fstab`, and the `.desktop`
  case executed as a command. `extract_all_command_proposals` now removes the
  `<think>`/`<thought>`/`<reasoning>` blocks before scanning (reuses
  `strip_think_blocks`), with a non-regression test on the real content.

## v0.5.6 — 2026-08-29

### Performance

- **Windowed rendering of the chat panel: no more 100% CPU lag on long
  sessions** — rendering concatenated **the entire** history into a single Paragraph
  and re-counted its wrap lines at each frame to compute the scroll: on
  a session of ~1000 messages / ~10,000 rows, the bottom-anchored view
  re-wrapped everything above the visible window at 11 fps, saturating a
  CPU core during LLM generation. Pass B now only materializes the
  messages overlapping the visible window (per-message geometry memoized in
  pass A) and scrolling is derived from the sum of per-message heights (both
  computed by ratatui: the old divergence came from the *simulated* counter
  since removed). Measurement on a 9820-row session: streaming CPU
  100% → ~20%.

### Fixed

- **Random reasoning expand on click** — after a successful toggle, the
  click zones ("💭 Reasoning · ") were emptied until the next render
  (~90 ms): a second quick click (double-click, rapid clicks) landed on
  an empty list and started a text selection instead of toggling.
  The zones are no longer emptied manually (rendering recomputes them anyway
  at each frame) and the target is widened to 2 rows when the reasoning
  is collapsed (the empty row under the toggle belongs to the target; in the
  expanded state, it remains selectable). Diagnostic instrumentation
  `SPIRITTY_UI_DEBUG=1` (`/tmp/spiritty_ui_debug.log`).

## v0.5.5 — 2026-08-29

### Changed

- **System prompt: a single command proposal per response** — §2
  literally taught the models to emit one bash block **per command**
  for "multi-step plans": the models stacked the successive steps
  into Alt+1/Alt+2/Alt+3 cards to be triggered blindly. Now: one
  proposal per response for sequential steps (the result returns to the
  model before the next step); several cards only for
  **alternatives** of the same action (pacman/apt/dnf → Alt+1/2/3); chaining
  `&&` for trivially atomic steps. Synchronized in the integrated prompt
  (`prompt.rs`) and the default template (`config/mod.rs`).

### Fixed

- **Tool calls emitted as HTML tags by Gemini ignored** — some models
  write `<tool:run_command>` (XML tag, closing omitted, isolated ```) instead of the
  taught fencing block ```` ```tool:run_command ````: the command was
  never executed, the model got stuck on its own format then displayed an
  *example* of syntax (💻 `command`) that the auto-approval policy
  executed as-is (`code 127`). The parser now recognizes the HTML-style
  tags (closed, with fence body, or truncated) and never extracts a
  tool call from `<think>` blocks anymore (reasoning is not an
  action). System prompt rule added: strict block syntax + prohibition
  of dummy/placeholder commands in the examples.
- **Gemini: HTTP 400 "Requests ending with a model turn are not supported"** —
  each request embedded the `Assistant("")` placeholder created by the UI as
  a streaming target: the history thus ended with a `model` turn, which
  the Gemini API refuses (OpenAI-compatible tolerates it, hence the unnoticed passage). The
  conversation preparation (`prepare_conversation`) now drops all
  empty messages, regardless of role; the Gemini provider additionally merges
  consecutive contents of the same role (System summaries mapped to `user`,
  user/user tool results) for a canonical request.
- **PTY capture: no more 45s timeout on local commands** — the
  incremental scan of the command end sentinel (`OSC 777`) only examined the
  last 20 characters of the buffer after each chunk. When the echo + the output +
  the sentinel coalesce into a single large PTY read (frequent on a fast local
  shell, depending on scheduling), the sentinel went unnoticed and the capture
  waited for the hard timeout to expire. The scan now uses a watermark
  (`sentinel_scan_upto`) that rescans only the necessary overlap:
  detection in ~4 ms instead of 45 s, amortized O(new bytes) cost preserved.

### Added

- **Capture instrumentation** (`SPIRITTY_CAPTURE_DEBUG=1`): event
  log of the capture cycle (arming, sentinel sighting, conclusion,
  buffer dump at 5 s) in `/tmp/spiritty_capture_debug.log` to diagnose
  captures that do not complete after the fact.

## v0.5.4 — 2026-08-28

### Changed

- **Release CI: removal of the `x86_64-apple-darwin` target** — the hosted Intel
  macOS runners (`macos-13`) are removed by GitHub; the job remained stuck in
  the queue (0 steps, no runner assigned) without ever producing a binary.
  The matrix now only builds Linux (`x86_64` + `aarch64`) and macOS Apple
  Silicon (`aarch64`).

### Fixed

- **Footer: `Ctx` display fixed** — `get_context_used_tokens` estimated
  the full session history (legacy of option C), hence an
  absurd "Ctx: 178k / 131k (100%)" (used > window, clamped to 100%) and almost
  identical to the total token counter. `Ctx` now estimates the context
  **actually compacted and sent** to the model (summary + 8 last verbatim turns),
  consistent with the request-time compaction; the "tok" counter remains the session
  total.

## v0.5.3 — 2026-08-28

### Changed

- **System prompt: prohibition to abbreviate commands with `...` or a placeholder** —
  addition of an explicit rule in `IMPORTANT RULES`: always paste the full
  content of a heredoc/script/file in the code block, never `...` /
  `BASE64` / `[content]` as a shortcut (the block is executed as-is — a
  placeholder is written verbatim to disk or fails; there is no
  truncation on the tool side). Fixes the case where GLM (`glm-5.3-flash`) reduced its
  long commands to `...`, then wrongly attributed the breakage to a
  "client truncation".

## v0.5.2 — 2026-08-28

### Changed

- 🟢 **Full history persisted, compaction reduced to the LLM context** (option C,
  "when scrolling back in the history we no longer see the whole of the exchanges"):
  `save_current_session` no longer compacts — the session JSON keeps **all**
  the exchanges, and reloading restores the whole conversation (no more
  the "1 summary + 8 turns = 9 messages" ceiling inherited from save-time
  compaction). Compaction moves **to request time**:
  `agent::send_prompt` now applies `compact_chat_messages` (extraction of
  the logic of `Session::compact` into a pure function in `session/mod.rs`) — the
  older turns roll into a System summary and the 8 most recent go
  verbatim to the provider, so the context budget remains bounded on
  long sessions, with an immediate benefit: the live context is compacted at
  each turn, not only on reload. The session JSONs already compacted
  by previous versions remain as they are (the history lost before
  this version is not reconstructible). Validated E2E in an isolated HOME:
  session of 15 messages → reload → re-save → 15 messages on
  disk, no summary persisted.

### Documentation

- README.md / README.fr.md refreshed for v0.5.x: compaction is documented
  honestly (full history persisted and restored — no more ceiling of 9
  messages in the session list —, and compaction limited to the LLM context
  alone: structured summary + 8 last verbatim turns), four-level classification
  badges (🟢 Safe / 🟡 Standard /
  🟣 Sudo / 🔴 Risky), SSH reconnect modal (`⏎ Reconnect`) and hint
  `· 🔗 SSH (resume)` added to the SSH section, `F10` (one-press
  authorization) and `F6` (focus) shortcuts in the table, and ASCII art fixed
  (`Ctrl+Space` for focus, `Alt+N` to execute — the old
  "Enter: Execute | Tab: Edit" did not correspond to any current binding).

## v0.5.1 — 2026-08-28

### Fixed

- 🔴 **The SSH reconnect modal did not appear on in-app reload**
  (report "when I load [the SSH session] I don't have the modal that offers
  to log me in via ssh"): `load_session` did set the `SshReconnect` offer, but the
  handler of the session list **closed it immediately** (`modal = None`
  after the `Load` action) — the offer only survived on the `-c` path
  (startup), where nothing closed it afterwards. The list now closes
  **except if** the reconnect offer has just been set. Validated end to end
  on a real SSH session reloaded in-app (modal "⏎ Reconnect /
  Esc Later" displayed, title `· 🔗 SSH (resume)` restored).

## v0.5.0 — 2026-08-28

### Added

- **Persistent SSH session indicator**: the terminal panel already displays
  `🌐 SSH: <target>` when a remote session is detected; it now also displays
  **`· 🔗 SSH <target> (resume)`** when resuming a session with `-c` that
  **was in SSH** while the PTY is still local (before reconnecting). The
  SSH target is persisted in the session JSON (`last_ssh_target`, sticky — a
  session that has been remote remains marked), and for sessions created by an
  earlier version a conservative heuristic infers it from the history (explicit
  `ssh …` command, classic remote prompts `user@host:~$`/`user@host$` or
  zsh bracket style `[user@host:/path] ±` — with anti git-remotes/e-mail guardrails).
  The title degrades cleanly according to the panel width (measured in cells,
  not bytes): `· 🔗 SSH <target> (resume)` → `· 🔗 <target> (resume)` →
  `· 🔗 SSH (resume)` → `· SSH (resume)` — the hint takes precedence over the cwd/branch when
  space is lacking. The restoration toast also mentions the SSH resume. Nothing is
  displayed for a 100% local session.
- **SSH reconnect modal**: when loading a `-c` session that was in SSH
  while the PTY is local, a small centered modal offers **`⏎ Reconnect`**
  to the recorded host (injection of `ssh <target>` into the terminal + automatic focus)
  or **`Esc Later`** (the title hint remains displayed). Never offered if already
  connected or during an ongoing PTY capture; i18n FR/EN.
  The target inferred from a prompt remainder (`user@hostname`, e.g. `xorne@prod`) is
  **resolved to a connectable address** via the host store (`prod` → `ducasse-seine.com`,
  most recent profile in case of collision, user preserved if it differs) — and the
  persisted hint is auto-corrected for future resumes.
- **Single-line thinking + reasoning timer + click to expand**:
  - The model's reasoning is now displayed **on a single line** (`▸ 💭 Think · …` /
    `▸ 💭 Reasoning · …`), showing the **end** of the reasoning (truncated with `…` if needed)
    instead of the multiline block that pushed the response off-screen.
  - **Line "💭 Deep thinking… / Deep reasoning…" kept BELOW the Think line**
    throughout the reasoning, with the **live `mm:ss` timer**
    (animated cyan shimmer); the timer disappears as soon as the response begins.
  - **Click on the line** `▸/▾` to **expand/collapse** the full reasoning. The
    hit-testing relies on ratatui's authoritative rows (no wrap drift),
    and the button toggles the state per message (cache properly invalidated, end always
    visible).
- **F10 = Allow the pending command**: the `ok`/`oui` + Enter validation becomes
  a simple **F10** key, operative from both panels (chat and terminal) — the
  ⚡ card now displays `[F10] Allow · [oui/ok + ↵] · [Esc] Deny`. Designed for
  long audit sessions where repeated approval becomes tedious.
- **Animated "Deep reasoning" wording**: during the silent reasoning phase of a
  model (no token received), the chat panel displays a dedicated line
  `⟳ 🧞 💭 Deep reasoning…` / `⟳ 🧞 💭 Deep thinking…` swept by a **cyan
  gradient shimmer** (DarkGray→LightCyan light wave sweeping the text at each tick,
  with a pause at the ends) instead of the silent ghost alone.
- **Full multiline prompt editing**:
  - `↑` / `↓` now navigate the visual rows of the prompt field (logical lines
    AND soft-wrapped newlines of a pasted paragraph), preserving the cursor
    column (clamped to the width of the target row). The prompt history only takes
    over at the ends (first/last row), as in a real editor.
  - `Ctrl+A` / `Ctrl+E` (readline memory): start/end of the **current logical
    line** (not only of the whole buffer).
  - The input area already followed the cursor vertically (scroll bounded to 8 lines) —
    now the cursor can finally reach it.
- New shared segmentation `prompt_visual_rows()`: single source of truth for
  cursor placement (`compute_prompt_cursor_and_lines` refactored onto it) and
  arrow navigation — no more risk of drift between displayed and edited.

### Fixed

- 🔴 **Duplicate command proposal "Alt+N" vs "F10"** (report with
  screenshot: "it proposed it in alt+f1 and via f10"): models with textual tools
  often write the command **in their text** (fence → interactive card
  "⚡ COMMAND #N / Alt+N Execute") **and** call it via the tool protocol
  (→ card "⚡ AUTHORIZATION REQUEST / F10 Allow") — the same command ended up
  with **two concurrent affordances**, and `Alt+N` could execute while
  bypassing the ongoing consent. Henceforth: the fence identical to the
  pending command is **demoted to an inert snippet** (the code remains visible, the
  authorization card becomes the sole action surface), and `Alt+N` is **blocked**
  (explicit toast) as long as an authorization is pending **or a PTY capture
  is in progress** — an injection during a capture would moreover overwrite the
  active capture.
- 🔴 **The timer after "💭 Deep thinking…" disappeared from the first tool**
  (report "the timer that is after Deep thinking has disappeared"): the `Instant` that
  feeds it is **consumed** (`take()`) by the tokens/s computation at the time of the
  tool call and was **never re-armed** for the continuation turns — from the
  first tool to the end of generation, the line remained without a chrono.
  Re-armed on each `AgentNewTurn` (continuation after an MCP/web/command tool) and
  in `record_command_result` (analysis after user command, path without an
  event), with reset of the segment counters (correct tokens/s per
  segment). Latent bug of the shimmer+timer wave, now visible since the
  tool loop runs again.
- 🔴 **Complete UI freeze during a tool execution** (report "the app
  froze completely, I killed the process but I still have outputs on my
  terminal") — three structural holes fixed together:
  1. **Blocking PTY writes on the UI loop**: each keystroke, paste and
     tool injection wrote *directly* into the PTY master from the events
     thread. A master write **blocks** as long as the downstream no longer consumes
     (stalled SSH pipe, remote tty input buffer full, remote shell frozen) — a
     single blocked write froze the whole application. Writes now go
     through a **dedicated writer thread** (FIFO queue, `write_all` never enqueues
     anymore, the \x15→command→sentinel order is preserved); responses to
     terminal queries (DA1/DSR/CPR/OSC) transit through the same queue.
  2. **`clean_pty_output` O(N) restarted at each tick** (~11×/s) on the capture
     buffer, which had **no size limit**: a chatty command made
     the per-frame cost quadratic (UI progressively frozen). The cleanup is no
     longer triggered only when the output has stabilized or at the hard-timeout, with
     a **cache indexed by the buffer length** (tick on a calm buffer = free),
     and the capture is **capped at 1 MiB**: beyond that, the bytes are counted without
     being buffered, a small rolling queue continues to detect the OSC 777
     sentinel (match on a real ESC byte — the echo `printf '\033]777…'` cannot
     false-positive) and the result carries an explicit notice "⚠️ Output truncated".
  3. **SIGTERM/SIGINT/SIGHUP without terminal restoration**: an external `kill`
     left the tty in raw mode, cursor hidden, alternate screen stuck (hence the
     "residual outputs" requiring a manual `reset`). A signal hook
     (signal-hook) restores raw-mode/screen/cursor/kitty protocol then exits with the
     conventional status `128+signal` — even when the main loop is frozen.
     `kill -9` remains impossible to intercept (`reset` remains the workaround).
- 🔴 **Raw tool-call markup displayed in the chat**: some models emit
  their tool call as inline XML (`<tool_calls><invoke name="exec_command"><parameter
  …>…`) in the visible stream instead of the tool protocol. The command was indeed
  executed (card + command block), but the XML leaked and was displayed in gray
  under the response. These blocks (`<tool_calls>`, `<DSML>`, `<invoke>`, `<parameter>`)
  are now **stripped**, including unclosed blocks (stream cut in the middle),
  while preserving the real response and the markdown code blocks. The
  hybrid marker is now **normalized before splitting** (`<｜｜DSML｜｜invoke …>`
  → `<invoke …>`, marker without a chevron → becomes the chevron): the old opener
  only matched at the 2nd full-width pipe and left a displayed `<｜` residue
  (report "display of `< |`"); a tag fragment truncated at the end of the message
  (`<`, `<|`, `</`) is also discarded, without touching legitimate `<` in plain text.
- 🔴 **Corrupted echo reported as command "output"** (report "the model
  cannot retrieve the result of the commands"): on an SSH session, the
  redraws of the remote line editor inject fragments into the raw echo and
  **double characters in the middle** (`logs/` → `llogs/`, `…8ecd…` →
  `…8ecdd…`). The echo cleaner required an exact match with the
  command sent → the corrupted echo survived the cleanup and was reported to the
  model as the real output. The model concluded that "the client systematically
  alters the commands" (hallucination documented in its reasoning)
  and worked around it with scripts/heredocs instead of reading the real results.
  Now the echo is also recognized by **subsequence** (a corruption by
  insertions preserves all the characters sent in order, comparison on an
  alphanumeric projection, length window ±50%): corrupted echo stripped,
  real output preserved, first legitimate line untouched. A `grep` with no
  result now reports an honest "⚠️ No output captured" instead of a
  false exploitable echo.
- 🔴 **Hybrid DSML tool calls never executed** (report "the model does
  nothing anymore"): the GLM/Z.ai model drifted to its native scaffolding
  `<｜｜DSML｜｜invoke name="exec_command">` (U+FF5C full-width pipes) emitted as
  text, a format the parser did not recognize — no tool detected, no command
  card, the turn ended in silence. The parser (`parse_tool_call`)
  now normalizes these markers (`｜`→`|`, removal of `|DSML|`) and extracts the
  HTML-style structure `<invoke name="…">` + `<parameter name="command">…</parameter>`
  (tolerates truncated streams), for `RunCommand` and `WebSearch`. The existing
  formats (` ```bash `, `<tool_call>`, `<|tool_calls|>`, JSON) remain priority
  and unchanged.
- 🔴 **"Empty-success" captures on slow remote commands**: the settle (3 s of
  silence, or 800 ms on a redisplayed prompt) could conclude the capture while
  **only the command's echo** had arrived — the real output (SSH latency, mysql
  handshake…) landed afterwards, and the result reported a lying
  "Command executed successfully in the terminal", on which the model reasoned
  (two consecutive empty commands on the VPS, user report). Now a cleaned
  capture that is **empty** never concludes on the settle: it waits for the real
  output until the hard-cap (45 s / 120 s), then reports an **explicit
  warning** ("⚠️ No output captured…") that pushes the model to check the terminal and
  retry, instead of inventing a success. The local sentinel path (exit code
  received) is not affected: there, "success without output" is truthful.
- 🔴 **The model's thinking was invisible** (OpenAI-compatible provider): the reasoning
  streamed by GLM / Z.ai / DeepSeek arrives in the **`reasoning_content`** field of the SSE
  chunks — separate from `content` — and Spiritty **silently discarded it**. The UI's
  `<think>` parser therefore never received anything: no "Think" line, only the
  ghost shimmer. The provider now captures `reasoning_content` and re-wraps it on
  the fly into a `<think>…</think>` block in the stream (`ReasoningBracket` state
  machine: opening on the first reasoning chunk, closing before the first response chunk,
  stray post-response reasoning ignored) — the existing UI displays it without change.
- ⚙️ **Reasoning timer on the same line**: no dedicated line — the
  `mm:ss` time since the last interaction is appended **after the wording**
  (`💭 Deep thinking… 8m 29s` / `💭 Deep reasoning… 8m 29s`), and on the
  collapsed think line after the end of the reasoning (`▸ 💭 Think · <end> · 8m 29s`).
- 🔴 **"The offset got worse, I can no longer see the end of the responses" (including when
  re-launching with `-c`)**: `max_scroll` of the chat panel was computed by summing, per
  message, a number of rows derived from a **home-made simulation** of word-wrap — which diverges
  (undercounts) from ratatui's real wrap on markdown tables, heredoc cards and wide
  glyphs. Each message accumulated its error: the bottom of the conversation became
  **unreachable by scrolling** (the offset saturated before the end), and the tail of the responses
  disappeared under the fold — reproduced and proven on the user's real session.
  `max_scroll` and the line badge now derive from **`Paragraph::line_count()`**
  (the same WordWrapper as the rendering, feature `unstable-rendered-line-info` enabled) — a
  single `Paragraph` serves for counting and rendering, zero additional clone per frame.
  End-to-end tests: end of the last response visible in the painted buffer, reloaded
  session included.
- 🔴 **PTY capture potentially blocked forever** (`on_tick`): when the output tail
  *resembled* a password prompt (keyword `password:`/`passphrase` — including a
  false positive, e.g. echo of a config file containing `password:`), the timeout switched to
  `u64::MAX` **and** the settle fallback was disabled — on a remote SSH session without an
  OSC sentinel, the capture **never** finished: the agent remained blocked waiting for the
  result, UI frozen, the only way out being to restart the app. The password wait remains generous but
  is now bounded (`PASSWORD_WAIT_HARD_CAP_SECS = 120 s`).
- ⚡ **Slowness of remote commands (SSH)**: each command paid the settle fallback
  (3 s of silence minimum) for lack of an OSC sentinel on the remote side. New
  **prompt-driven accelerated settle**: when the output tail ends with a line resembling the
  PS1 redisplayed by the remote shell (`user@host:path$`, `#`, `>`…) and has been silent for
  800 ms, the capture finishes immediately (~1 s instead of ~3.8 s per command) — it is
  half the time perceived in remote file editing loops. Commands without a final
  prompt (streams, `tail -f`) keep the 3 s fallback; hooked local shells
  continue to wait for their OSC 777 sentinel.
- 🔴 **```` ```bash ```` block displayed as "bash" + command but not executable** (recurrence of the
  historical bug, new variant): the anti-arrow heuristic (`->`) rejected any block
  whose command contained an arrow **in a quoted title** — real example:
  `echo "=== CLONE DB resa-v3 -> resa_pp ==="` (Stripe/resa prod session). The block was
  downgraded to a snippet box labeled `bash`, hence without an execution shortcut. Explicit
  shell tags (`bash`/`sh`/`zsh`…) now short-circuit the format heuristics
  (arrows, conversational phrasings): the tag is an author's intention;
  **untagged** fences remain protected by these same heuristics.
- 🔴 **Dead arrows in `vi`/`vim` in the terminal panel**: the shell switched its
  keyboard to application-cursor mode (DECCKM via `smkx`, `ESC[?1h`) and expected the
  arrows in SS3 (`ESC O A`), but Spiritty only sent CSI (`ESC [ A`) — arrows OK
  in the shell, dead in vi. The DECCKM state of the child is now tracked by scanning
  its output stream, and the key encoder switches CSI↔SS3 accordingly
  (arrows + Home/End).
- 🔴 **Regression "scrolling over empty lines during the stream"** (introduced by the render
  cache, detected on the first user test): the assistant tail being streamed
  was not stored in the cache whereas the cloning pass draws exclusively from it
  → its rows were missing from the widget but remained counted by the scroll (`max_scroll`
  oversized = scrolling into the void), until the "full redisplay" at the end of the
  stream. The tail is now stored unconditionally — the freshness predicate
  already recomposes it at each frame during `is_generating`, so zero staleness possible.
- Reasoning ghost invisible during the stream: same root cause (the loader line lived
  in the non-cloned tail).

### Performance

- **Per-message render cache for the chat panel** (the "M4 virtualization" effort, option A):
  each frame no longer re-parses the WHOLE history (markdown, ⚡ cards, reasoning blocks,
  wrap-simulation) but only the messages actually modified since the previous
  frame.
  - New `ChatRenderCache` on `App` (internal `RefCell`, UI thread only):
    artifacts rendered per message validated by a structured generation key
    `(panel width · language · debug mode · theme)` + role + content byte length;
    the streaming tail is recomposed on the fly without polluting the cache.
  - Verbatim extraction of the System/User/Assistant branches into pure composers
    (`compose_single_message`) + approval card outside the cache — zero visual divergence
    compared to the old inline loop.
  - Pass B kept as a full clone from the cache: the "visible window
    only" optimization was attempted then rejected for lack of being able to faithfully reproduce the paragraph
    wrap of a line truncated at its head (detailed comment in the code). The major gain remains
    the elimination of re-parse/re-wrap/reallocations per frame.
  - Massive invalidation at the right places: resize/theme-language-debug change (key),
    full session change and chat reset (`hard_reset()`).
  - Safety net: goldens of the wrap counter vs the real ratatui rendering (6 widths × 5
    ASCII/CJK/emoji/styled-spans fixtures) + documented discovery: `Line::from(String)`
    normalizes internal `\n` into separate spans as soon as it is constructed.
  - Suite raised to **92 green tests**, clippy `-D warnings` clean.

---

## v0.4.5 — 2026-08-27

### Added

- **Z.ai cloud provider (GLM / Zhipu AI)**:
  - New `ProviderType::Zai` type with the display name "Z.ai (GLM)", API key `ZAI_API_KEY`
    (+ fallback alias `ZHIPU_API_KEY`) and default base `https://api.z.ai/api/paas/v4`.
  - Automatic detection from the saved config via the aliases `zai`, `z.ai`, `z_ai`,
    `z-ai`, `zhipu`, `glm`.
  - Dynamic listing of the models handled for the base URL ending in `/v4`.
  - Context window auto-detected at 131,072 tokens for the `glm*` / `*zai*` models.
  - Popular models pre-listed (from `glm-5.3` to `glm-4-flash`); GLM pricing integrated
    into the local cache (`assets/pricing.json`).
  - Extended unit/integration tests (`config_test.rs`, `pricing_test.rs`).

- **`CHANGELOG.md`**: structured tracking of changes between each push (this convention).

### Fixed — security audits & user bugs (51d880d)

- 🔴 **Accidental approval via empty-Enter**: pressing `Enter` while a card
  ⚡ AUTHORIZATION REQUEST is displayed executed the pending command — even one classified
  Risky — because `""` was a natural approval phrase. The card now displays
  `[oui/ok + ↵] Allow`.
- 🟠 **Stray proposal `` `bash `` without execution** (reported screen): a ```bash block
  containing a literal `bash` line produced a multiline injection proposal that
  opened a nested shell instead of executing the command. New cleanup
  `sanitize_proposed_command()` applied to both extraction paths (markdown fences and
  `tool:run_command`): `bash/sh/zsh/shebang` lines removed, orphan `exit`/`logout` at the
  tail as well (they would have killed the user's shell).
- 🟠 **Output prose/token becoming "⚡ COMMAND #1"** (screens "Mail queue is empty",
  IP lists): rejection of untagged blocks, capitalized sentences without a meta-shell, prose threshold
  lowered from 7 to 5 words.
- **Divergent risk badge**: the ⚡ card and the `COMMAND #N` cards both
  use `safety::classify_command` as the single source (before: ad-hoc heuristics showing
  green "Safe" on `kill -9`/`chmod`).

### Security — four-level risk taxonomy (51d880d)

- New `CommandRisk::Sudo` class distinct from `Risky`: elevated but *non-destructive*
  commands (`sudo ls/grep/cat/certbot certificates/systemctl status …`).
- Auto-approval mapping revised: level `Sudo` ⇒ {Safe, Standard, Sudo} auto-approved;
  the destructive ones (`rm`, restarts, package installs) remain behind confirmation up to
  YOLO mode. Fixes the user report "I am in sudo approval and it asks me
  to approve a read-only cmd".
- Classifier order reversed: destructiveness is evaluated before elevation —
  `sudo rm/pacman -S/apt install/chmod` always remain `Risky`.
- `pacman/yay/paru`: detection tolerates a `sudo/doas` prefix (`sudo pacman -Syu` escaped
  the rule).
- API keys: `config.toml`, `hosts.json`, session JSON and pricing cache written in
  **0600** (before: 0664 depending on umask). The Ctrl+P modal never displays an existing
  key again (empty field = keep; typing masked as `••••`; `ENV:` refs visible).

### Performance

- **Incremental PTY capture**: end of the UTF-8 re-decoding of the whole buffer at each chunk
  (O(n²) during verbose commands). Decoding with multi-byte carry,
  sudo password detection on a bounded backward window (1 KB), OSC 777 sentinel
  search with a watermark. Validated by tests (emoji split into 4 chunks, invalid bytes).
- **UI thread unblocked** (AGENTS.md rule "NEVER block"):
  - Ctrl+V: asynchronous fire-and-forget clipboard read (anti-spam guard) instead of a
    blocking `recv_timeout(1500ms)`; modals bounded to 1 s.
  - `$SHELL -l -c` (ENV probe) memoized at process level: no more shell spawn on keydown
    during provider reloads.
  - Full `/proc` scan fallback of the watcher memoized 1.5 s per root PID (instead of ×2
    every ~360 ms on a transient kernel miss).

### Fixed — PTY lifecycle (3afca40)

- Typing `exit` in the terminal panel froze the TUI on a dead PTY and left the shell
  as a zombie. Now: dedicated reaper thread (`child.wait()`) + single-shot exit
  notification (reader EOF ∪ wait child, `AtomicBool` guard) converted into `AppEvent::PtyExit`
  → clean exit with session save.

### Infra & distribution

- `release.yml`: the LICENSE is now included in the published tarballs.
- `install.sh`: **sha256** verification of the downloaded archive (hard failure on mismatch,
  warn-and-skip if the checksum file is absent).
- Update of the `LATEST_TAG` fallback → `v0.4.5`.
- Repository fully normalized with `cargo fmt` (~530 hunks of debt erased);
  clippy `-D warnings` clean; test suite raised to **88 green tests**
  (+8: exact reproductions of user screens, Sudo matrix, split UTF-8 decoder,
  char-boundary helpers).

### Internal quality (not visible)

- MCP leak fixed: `pending` entry removed if the stdin write fails.
- 600 mode applied to the config save as well as to the hosts/sessions/pricing writers.

---

## v0.4.3 — 2026-08-26

### Fixed

- Truncated captures: drop of heredoc comments at the head of a proposal and rejection of
  tabular blocks as command proposals.
- Reliability of silent capture, clean shell integration and background jobs
  (v0.4.2); robustness & security hardening (v0.4.1).
