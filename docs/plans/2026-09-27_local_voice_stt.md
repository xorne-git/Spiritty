# Implementation Plan — 100% Local Voice Input (Speech-to-Text → Prompt)

**Date:** September 27, 2026
**Status:** Phases 1 & 2 implemented (`F8` manual segment, `F7` continuous VAD); Phase 3 (native backend, model downloader) pending
**Related:** `ARCHITECTURE.md` (§1 Event Loop), `AGENTS.md` (invariants 1, 2, 3, 4, 5)

---

## 🎯 Context & Objective

Today Spiritty has no voice input. The OpenCode + PIPA huddle gives a very fluid
experience: you speak, a short silence ends the turn, the speech is transcribed and the
text lands directly in the agent prompt. The user wants the **same behaviour inside
Spiritty, but 100% local** (no cloud STT, no browser Web Speech API — that API is only
reachable from a web page and is cloud-backed anyway).

Target UX:

- A mode where the microphone stays open, an activity detector spots the end of a
  sentence (≈1–2 s of silence) and Whisper transcribes it locally.
- The transcript is injected into the **chat prompt** (left panel), ready to send —
  optionally auto-submitted.
- A visible state indicator (idle / listening / recording / transcribing) so the mic is
  never silently on.
- A manual fallback (tap to record a segment) for noisy environments.

---

## 🧭 Design

### Module layout (new `src/voice/`)

Mirrors the existing subsystem style (a façade owning a background task + typed events).

```
src/voice/
├── mod.rs          # VoiceController: public façade, owns the task handle + command channel
├── state.rs        # VoiceState: Off | Idle | Listening | Recording | Transcribing | Error
├── capture.rs      # audio capture → 16 kHz mono PCM WAV (backend trait)
├── vad.rs          # voice-activity detection (pure, unit-testable decision fn)
└── transcribe.rs   # Transcriber trait + backends (whisper-cli default, whisper-rs optional)
```

`pub mod voice;` added to `src/lib.rs`.

### Concurrency & events (invariant 1)

- The voice subsystem runs in its own spawned task; the raw audio/VAD loop runs on a
  dedicated OS thread (or a child process) so the `crossterm` event loop is never blocked.
- Results come back **only** through new `AppEvent` variants (central bus, `src/event.rs`):

  ```rust
  VoiceStateChanged(VoiceState),
  VoiceTranscript { text: String, auto_submit: bool },
  VoiceError(String),
  ```

- `App` holds a `VoiceController` and talks to the task over a small command channel
  (`StartContinuous`, `Stop`, `StartSegment`, `StopSegment`, `Shutdown`).

### Audio capture backends (`capture.rs`)

Prefer **no build-time native dependency** so the `cross`/GitHub release matrix stays
untouched:

1. **Default — external recorder** (detected at runtime, clear i18n error if missing):
   - Linux: `arecord -f S16_LE -r 16000 -c 1 -t raw` (or `ffmpeg`/`sox` when present).
   - macOS: `ffmpeg`/`sox`.
   - Emits raw s16le to a bounded in-memory ring buffer.
2. **Optional — `cpal`** behind a `voice-native` feature (self-contained binary; requires
   ALSA dev libs on Linux → must be added to the cross image). Phase 3.

Recording always ends as a 16 kHz mono WAV (`hound` writer or a hand-rolled header — the
header writer is a unit-test target).

### Voice activity detection (`vad.rs`)

Pure logic first, no mic required to test:

- Energy/RMS gate on framed samples with a configurable threshold.
- **Pre-roll ring buffer** (~300 ms) so the start of the sentence is not clipped.
- Segment ends after `silence_ms` (default ~1400 ms) below threshold, or at
  `max_segment_ms` (default 30 s).
- A segment is kept only if it contains at least `min_speech_ms` (default ~300 ms).
- Exposed as a pure `fn push_frame(&mut self, rms: f32, ts: ...) -> VadDecision` so all
  timing branches are unit-tested with synthetic frames.
- Phase 3 upgrade: swap in `webrtc-vad` behind the same trait for better noise immunity.

### Transcription backends (`transcribe.rs`)

`trait Transcriber { async fn transcribe(&self, wav: &Path) -> Result<String>; }`

1. **Default — `whisper-cli` (whisper.cpp)**, installed by the user:
   - Invocation: `whisper-cli -m <model> -f <wav> -l <lang> -nt -otxt -of <tmpbase>`
     then read `<tmpbase>.txt` (or parse stdout).
   - `language` = active UI language by default (fr/en).
   - `initial_prompt` seeded with French sysadmin vocabulary (command names, `sudo`,
     `systemctl`, `nginx`, `ssh`…) to bias recognition.
   - Non-zero exit / missing binary → `VoiceError` (never a panic; invariant 5).
2. **Optional — in-process `whisper-rs`** behind `voice-native` (Phase 3), same trait.

Model files live in `~/.config/spiritty/models/ggml-<size>.bin`. Recommend `small` for FR
(accuracy/latency sweet spot) with `base` as the fast option.

### Configuration (`src/config/mod.rs`)

New `[voice]` table, all fields `#[serde(default)]` so existing configs load unchanged:

```toml
[voice]
enabled         = false          # opt-in (privacy)
mode            = "vad"          # "vad" | "manual"
capture_backend = "auto"         # "auto" | "arecord" | "ffmpeg" | "sox" | "cpal"
recorder_bin    = ""             # optional explicit path
engine          = "whisper-cli"  # "whisper-cli" | "whisper-rs"
whisper_bin     = "whisper-cli"
model_path      = "~/.config/spiritty/models/ggml-small.bin"
language        = ""             # empty = follow UI language
input_device    = ""             # empty = system default
silence_ms      = 1400
min_speech_ms   = 300
max_segment_ms  = 30000
auto_submit     = false          # false = inject only (human-in-the-loop, invariant 2)
```

Getters (`get_voice_*`) + defaults + a `config_test` round-trip test.

### Keybindings

`F1`…`F5` and `F6`/`Ctrl+Space`/`Shift+Tab` are already taken (help, config, approval,
layout, swap, focus). Use free function keys:

- **`F7`** — toggle continuous voice mode (VAD auto-transcribe on/off).
- **`F8`** — manual segment: first press starts recording, second press stops and
  transcribes (works even when continuous mode is off; also the noisy-room fallback).

Tap-to-toggle (not hold-to-talk) avoids having to forward `KeyEventKind::Release`, which
`event.rs` currently drops and which some terminals do not deliver.

### App integration (`src/app.rs`)

- `App.voice: VoiceController`, initialized disabled from config.
- `F7` / `F8` handlers route to the controller; state changes update a badge.
- On `AppEvent::VoiceTranscript { text, auto_submit }`:
  - insert at `self.cursor_pos` in `self.chat_input` (add a separating space when the
    buffer is non-empty and does not already end in whitespace);
  - if `auto_submit`, call the shared submit path (see refactor below) and move focus to
    `Focus::Chat` so the user sees the turn.
- **Refactor:** extract the body of the `KeyCode::Enter` submit branch (~`app.rs:2879`)
  into `App::submit_chat_input(&mut self)` so manual Enter and voice auto-submit cannot
  diverge.

### i18n (invariant 3)

New `I18nKey` variants added to `mod.rs` **and** `fr.rs` **and** `en.rs`:

- `HelpKeyVoiceToggle` / `HelpDescVoiceToggle`
- `HelpKeyVoiceSegment` / `HelpDescVoiceSegment`
- `VoiceStateIdle` / `VoiceStateListening` / `VoiceStateRecording` / `VoiceStateTranscribing`
- `VoiceDisabledHint`, `VoiceError`, `VoiceNoModel`
- `ToastVoiceEnabled`, `ToastVoiceDisabled`
- `FooterVoiceLabel`

Default fallback stays **French**.

### UI (`src/ui/mod.rs`, help modal)

- **Footer:** a voice pill next to `F3` (`F7 Voix` / `F7 Voice`) carrying a state dot
  (idle/listening/recording/transcribing), registered in `right_shortcut_spans` with the
  index arrays + tests updated.
- **Help modal:** add the `F7` / `F8` rows.
- **Chat prompt:** optional small “● listening…” hint near the input while recording.

---

## ✅ Phases

### Phase 1 — Skeleton + manual segment (MVP) ✅

- [x] `src/voice/` module skeleton, `VoiceState`, command channel, task + `AppEvent` bridge.
- [x] `[voice]` config section + getters + tests.
- [x] `capture.rs` external-recorder backend (arecord/ffmpeg/sox) → 16 kHz mono WAV.
- [x] `transcribe.rs` `whisper-cli` backend (command builder + `sanitize_transcript`, unit-tested).
- [x] `F8` manual segment + transcript injection into the prompt.
- [x] i18n keys (mod/fr/en) + help rows.
- [x] Unit tests (WAV header, transcript insertion, config, i18n catalog completeness).

> Implementation note: Phase 1 ships the `F8` manual segment (tap to start, tap to stop)
> with the footer/help labelled `F8`. `F7` continuous VAD mode lands in Phase 2.

### Phase 2 — Continuous VAD auto-transcribe (target UX) ✅

- [x] `vad.rs` energy VAD (pre-roll, min-speech, silence timeout, max segment) + tests.
- [x] `F7` continuous mode: state badge, auto-inject on segment end.
- [x] Optional `auto_submit` (default off, human-in-the-loop).
- [x] Footer indicator + state toasts.
- [x] Errors surfaced as toasts (`VoiceError`), never a crash.

> Implementation note: continuous mode streams `arecord`/`ffmpeg`/`sox` stdout and frames it
> in Rust (no pre-roll buffer yet — Whisper trims leading silence). `F7` toggles it; `F8`
> remains the manual tap-to-record segment. A `stop` notification interrupts the stream
> cleanly between selections.

### Phase 3 — Polish & self-contained option

- [ ] `voice-native` feature: `cpal` capture + in-process `whisper-rs`.
- [ ] Model downloader (`spiritty --install-voice-model small`, or first-run prompt) with
      checksum + progress.
- [ ] Input-device picker; `webrtc-vad`; French sysadmin `initial_prompt` tuning.
- [ ] README / README.fr / ROADMAP / CHANGELOG updates.

---

## 🧪 Testing strategy

Pure-logic only (no mic, no model) — consistent with the repo’s headless test suite:

- VAD decisions over synthetic RMS frame sequences (start, hangover, silence cut, max
  segment, below min-speech dropped).
- WAV header bytes / sample rate / mono channel count.
- Transcript insertion: empty buffer, non-empty (space handling), mid-buffer cursor,
  multiline, auto-submit routing.
- Config TOML round-trip with/without `[voice]`.
- i18n: every new key resolves in both languages.
- Footer contains `F7`; `help_modal` contains the `F7`/`F8` rows.
- Transcription mocked via `FakeTranscriber`.

Integration/E2E (manual): real mic + real `whisper-cli` + model, like the existing manual
TUI validation.

---

## ⚠️ Risks & decisions

- **CI / release matrix:** keep the default build free of native audio deps by shelling out
  to `arecord`/`ffmpeg` and `whisper-cli`. Only the optional `voice-native` feature pulls
  `cpal`/`whisper-rs` (needs ALSA dev + cmake; must be added to the cross image if built in
  CI).
- **Runtime dependency:** default backends require the user to install a recorder and
  whisper.cpp + a model; detect and explain clearly at first use.
- **Latency:** `small` FR ≈ a few hundred ms for short utterances on CPU; `base` is faster
  but less accurate.
- **Privacy:** audio never leaves the machine; the mic state is always visible and off by
  default.
- **Key conflicts:** verified `F7`/`F8` are unused (F1–F6 and Ctrl+Space are bound).
- **Accuracy:** bias with `initial_prompt` (sysadmin vocabulary); keep `auto_submit`
  off by default so the user validates the transcript before it reaches the model.

---

## 🚫 Out of scope

- Cloud STT providers (explicitly rejected — user wants 100% local).
- Text-to-speech / voice replies.
- Full wake-word detection.

---

## ❓ Open questions

- Default model size: `small` (recommended) vs `base`?
- Bundle a model downloader in Phase 3, or document a manual `whisper.cpp` install only?
- Should `auto_submit` ever default to on for a hands-free “huddle-like” flow?
