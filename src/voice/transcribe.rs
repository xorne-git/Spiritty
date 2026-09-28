//! Local transcription via `whisper.cpp` (`whisper-cli`).
//!
//! The WAV produced by [`super::capture`] is handed to an external `whisper-cli` process,
//! which writes a plain-text transcript (`-otxt`). No network access is involved.

use crate::config::VoiceConfig;
use crate::i18n::Language;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Hard cap on a single `whisper-cli` run. A 30 s segment transcribes far under this;
/// the limit only exists so a stalled or OOM-killed process can never block a
/// transcription (and thus the whole voice feature) indefinitely.
const WHISPER_TIMEOUT: Duration = Duration::from_secs(120);

/// The `whisper.cpp` CLI binary to use (config override or the conventional `whisper-cli`).
/// A leading `~` is expanded so config paths like `~/.local/opt/...` work.
///
/// When the configured value is a bare binary name (the `"whisper-cli"` default) that is
/// not on `PATH`, fall back to the standard installer build location
/// (`~/.local/opt/whisper.cpp/build/bin/whisper-cli`). This makes the voice input work even
/// when the initial `[voice]` config patch was not persisted.
pub fn whisper_binary(cfg: &VoiceConfig) -> String {
    let bin = cfg.whisper_bin.trim();
    let candidate = if bin.is_empty() { "whisper-cli" } else { bin };

    // Explicit path (absolute or containing a separator): expand `~` and use it as-is.
    if candidate.contains('/') {
        return crate::config::expand_tilde(candidate)
            .to_string_lossy()
            .to_string();
    }

    if crate::voice::capture::find_in_path(candidate).is_some() {
        return candidate.to_string();
    }

    if candidate == "whisper-cli" {
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".local/opt/whisper.cpp/build/bin/whisper-cli");
            if p.is_file() {
                return p.to_string_lossy().to_string();
            }
        }
    }

    candidate.to_string()
}

/// Builds the `whisper-cli` command for a WAV file, writing `<out_base>.txt`.
pub fn build_whisper_command(
    cfg: &VoiceConfig,
    wav: &Path,
    lang: &str,
    out_base: &Path,
) -> Command {
    let mut cmd = Command::new(whisper_binary(cfg));
    cmd.arg("-m")
        .arg(cfg.resolved_model_path())
        .arg("-f")
        .arg(wav)
        .arg("-l")
        .arg(lang)
        .arg("-nt") // no timestamps
        .arg("-otxt")
        .arg("-of")
        .arg(out_base);
    cmd
}

/// Transcribes a WAV file locally and returns the cleaned transcript.
pub async fn transcribe(cfg: &VoiceConfig, ui: Language, wav: &Path) -> Result<String> {
    let model = cfg.resolved_model_path();
    if !model.is_file() {
        return Err(anyhow!(
            "Whisper model not found: {} (download a GGML model, e.g. ggml-small.bin)",
            model.display()
        ));
    }

    let lang = cfg.resolved_language(ui);
    let out_base = out_base_for(wav);
    let mut cmd = build_whisper_command(cfg, wav, &lang, &out_base);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // Ensure a dropped future (e.g. on timeout) also kills the child, so a hung
        // whisper-cli can never linger as an orphan process.
        .kill_on_drop(true);

    let output = match timeout(WHISPER_TIMEOUT, cmd.output()).await {
        Ok(res) => res.with_context(|| format!("Failed to run `{}`", whisper_binary(cfg)))?,
        Err(_) => {
            return Err(anyhow!(
                "whisper-cli timed out after {}s (model stalled or out of memory)",
                WHISPER_TIMEOUT.as_secs()
            ));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("whisper-cli failed: {}", stderr.trim()));
    }

    let txt_path = out_base.with_extension("txt");
    let raw = std::fs::read_to_string(&txt_path)
        .with_context(|| format!("whisper-cli produced no transcript at {:?}", txt_path))?;
    let _ = std::fs::remove_file(&txt_path);

    Ok(sanitize_transcript(&raw))
}

/// Strips the WAV extension from a path so it can be passed to `whisper-cli -of`.
pub fn out_base_for(wav: &Path) -> PathBuf {
    wav.with_extension("")
}

/// Cleans raw `whisper.cpp` output: strips a leading timestamp/bracket prefix, drops blank
/// lines and joins the rest into a single line.
pub fn sanitize_transcript(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let mut cleaned = line.trim();
        // Timestamps or bracketed markers (`[00:00:00.000 --> 00:00:02.000] Bonjour`,
        // `[BLANK_AUDIO]`) when they are not already suppressed by `-nt`.
        if let Some(rest) = cleaned.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                cleaned = rest[end + 1..].trim();
            }
        }
        if cleaned.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(cleaned);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VoiceConfig {
        VoiceConfig {
            whisper_bin: "/opt/whisper/whisper-cli".to_string(),
            model_path: "/models/ggml-small.bin".to_string(),
            language: "fr".to_string(),
            ..VoiceConfig::default()
        }
    }

    #[test]
    fn whisper_binary_keeps_explicit_paths() {
        let mut c = cfg();
        c.whisper_bin = "/opt/whisper/whisper-cli".to_string();
        assert_eq!(whisper_binary(&c), "/opt/whisper/whisper-cli");
        // A bare, unknown name is returned unchanged when nothing else is found.
        c.whisper_bin = "definitely-not-whisper-cli".to_string();
        assert_eq!(whisper_binary(&c), "definitely-not-whisper-cli");
    }

    #[test]
    fn command_contains_model_language_and_txt_output() {
        let cmd = build_whisper_command(
            &cfg(),
            Path::new("/tmp/voice.wav"),
            "fr",
            Path::new("/tmp/voice"),
        );
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "/opt/whisper/whisper-cli");
        let args: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"/models/ggml-small.bin".to_string()));
        assert!(args.contains(&"-l".to_string()));
        assert!(args.contains(&"fr".to_string()));
        assert!(args.contains(&"-otxt".to_string()));
        assert!(args.contains(&"/tmp/voice".to_string()));
        assert!(args.contains(&"-nt".to_string()));
    }

    #[test]
    fn out_base_strips_the_wav_extension() {
        assert_eq!(out_base_for(Path::new("/tmp/a/voice-1.wav")), PathBuf::from("/tmp/a/voice-1"));
    }

    #[test]
    fn sanitize_drops_blanks_and_timestamps_and_joins_lines() {
        let raw = "\n[00:00:00.000 --> 00:00:02.000]  Bonjour\n\n  le monde  \n";
        assert_eq!(sanitize_transcript(raw), "Bonjour le monde");
    }

    #[test]
    fn sanitize_empty_is_empty() {
        assert_eq!(sanitize_transcript("   \n\n"), "");
    }

    #[test]
    fn resolved_language_follows_ui_when_unset() {
        let mut c = cfg();
        c.language = String::new();
        assert_eq!(c.resolved_language(Language::Fr), "fr");
        assert_eq!(c.resolved_language(Language::En), "en");
        c.language = "de".to_string();
        assert_eq!(c.resolved_language(Language::Fr), "de");
    }
}
