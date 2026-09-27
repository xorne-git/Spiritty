//! Audio capture backend: records a raw 16 kHz / mono / s16le stream to a temp file via an
//! external recorder, then wraps it in a minimal WAV header for `whisper.cpp`.
//!
//! Using an external recorder keeps Spiritty's build free of native audio dependencies, so
//! the release/CI matrix is untouched. `arecord` (alsa-utils) is the primary Linux backend,
//! with `ffmpeg` and `sox` as portable fallbacks.

use crate::config::VoiceConfig;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};

/// Sample rate expected by Whisper.
pub const SAMPLE_RATE: u32 = 16_000;
/// Mono capture.
pub const CHANNELS: u16 = 1;
/// 16-bit signed little-endian samples.
pub const BITS: u16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    Arecord,
    Ffmpeg,
    Sox,
}

impl CaptureBackend {
    pub fn binary(self) -> &'static str {
        match self {
            CaptureBackend::Arecord => "arecord",
            CaptureBackend::Ffmpeg => "ffmpeg",
            CaptureBackend::Sox => "sox",
        }
    }

    /// Explicit config value that selects this backend (`capture_backend = "..."`).
    pub fn key(self) -> &'static str {
        match self {
            CaptureBackend::Arecord => "arecord",
            CaptureBackend::Ffmpeg => "ffmpeg",
            CaptureBackend::Sox => "sox",
        }
    }
}

/// Searches `PATH` (or checks an explicit path) for an executable. Returns `None` when the
/// binary cannot be found.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if name.contains('/') {
        let candidate = PathBuf::from(name);
        return candidate.is_file().then_some(candidate);
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Resolves which recorder to use: the explicit `capture_backend` when set, otherwise the
/// first available among `arecord`, `ffmpeg`, `sox`.
pub fn resolve_backend(cfg: &VoiceConfig) -> Result<CaptureBackend> {
    let explicit = cfg.capture_backend.trim().to_lowercase();
    let bin_override = cfg.recorder_bin.trim();

    if !bin_override.is_empty() {
        let backend = match explicit.as_str() {
            "ffmpeg" => CaptureBackend::Ffmpeg,
            "sox" => CaptureBackend::Sox,
            _ => CaptureBackend::Arecord,
        };
        return if find_in_path(bin_override).is_some() {
            Ok(backend)
        } else {
            Err(anyhow!("configured recorder `{}` not found", bin_override))
        };
    }

    let candidates: &[CaptureBackend] = match explicit.as_str() {
        "arecord" => &[CaptureBackend::Arecord],
        "ffmpeg" => &[CaptureBackend::Ffmpeg],
        "sox" => &[CaptureBackend::Sox],
        _ => &[
            CaptureBackend::Arecord,
            CaptureBackend::Ffmpeg,
            CaptureBackend::Sox,
        ],
    };

    candidates
        .iter()
        .copied()
        .find(|b| find_in_path(b.binary()).is_some())
        .ok_or_else(|| {
            anyhow!(
                "no audio recorder found (looked for {}); install alsa-utils (arecord), ffmpeg or sox",
                candidates
                    .iter()
                    .map(|b| b.binary())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// Returns the `(input format, default device)` pair for `ffmpeg` on the current platform.
fn ffmpeg_input() -> (&'static str, &'static str) {
    #[cfg(target_os = "macos")]
    {
        ("avfoundation", ":0")
    }
    #[cfg(not(target_os = "macos"))]
    {
        ("pulse", "default")
    }
}

/// Builds the recorder command that writes raw s16le PCM to `out_raw`.
pub fn build_command(backend: CaptureBackend, cfg: &VoiceConfig, out_raw: &Path) -> Command {
    let bin = if cfg.recorder_bin.trim().is_empty() {
        backend.binary().to_string()
    } else {
        cfg.recorder_bin.trim().to_string()
    };
    let device = cfg.input_device.trim();
    let mut cmd = Command::new(bin);

    match backend {
        CaptureBackend::Arecord => {
            cmd.arg("-q")
                .arg("-f")
                .arg("S16_LE")
                .arg("-r")
                .arg(SAMPLE_RATE.to_string())
                .arg("-c")
                .arg(CHANNELS.to_string())
                .arg("-t")
                .arg("raw");
            if !device.is_empty() {
                cmd.arg("-D").arg(device);
            }
            cmd.arg(out_raw);
        }
        CaptureBackend::Ffmpeg => {
            let (format, default_device) = ffmpeg_input();
            cmd.arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-y")
                .arg("-f")
                .arg(format)
                .arg("-i")
                .arg(if device.is_empty() { default_device } else { device })
                .arg("-ar")
                .arg(SAMPLE_RATE.to_string())
                .arg("-ac")
                .arg(CHANNELS.to_string())
                .arg("-f")
                .arg("s16le")
                .arg(out_raw);
        }
        CaptureBackend::Sox => {
            if !device.is_empty() {
                cmd.env("AUDIODEV", device);
            }
            cmd.arg("-q")
                .arg("-d")
                .arg("-r")
                .arg(SAMPLE_RATE.to_string())
                .arg("-c")
                .arg(CHANNELS.to_string())
                .arg("-t")
                .arg("raw")
                .arg(out_raw);
        }
    }

    cmd
}

/// A minimal canonical PCM WAV header (44 bytes) for `data_len` bytes of audio.
pub fn wav_header(data_len: u32, sample_rate: u32, channels: u16, bits: u16) -> [u8; 44] {
    let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
    let block_align = channels * (bits / 8);
    let mut header = [0u8; 44];

    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_len.to_le_bytes());

    header
}

/// Wraps a raw s16le stream into a WAV file at `wav`.
pub fn finalize_wav(raw: &Path, wav: &Path) -> Result<()> {
    let data =
        std::fs::read(raw).with_context(|| format!("Failed to read raw audio {:?}", raw))?;
    write_wav_bytes(&data, wav)
}

/// Writes a WAV file containing `pcm` (raw s16le samples at 16 kHz mono).
pub fn write_wav_bytes(pcm: &[u8], wav: &Path) -> Result<()> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(&wav_header(pcm.len() as u32, SAMPLE_RATE, CHANNELS, BITS));
    out.extend_from_slice(pcm);
    std::fs::write(wav, out).with_context(|| format!("Failed to write WAV {:?}", wav))?;
    Ok(())
}

/// Builds a recorder command that streams raw s16le PCM to **stdout** (continuous mode).
pub fn build_stream_command(backend: CaptureBackend, cfg: &VoiceConfig) -> Command {
    let bin = if cfg.recorder_bin.trim().is_empty() {
        backend.binary().to_string()
    } else {
        cfg.recorder_bin.trim().to_string()
    };
    let device = cfg.input_device.trim();
    let mut cmd = Command::new(bin);

    match backend {
        CaptureBackend::Arecord => {
            cmd.arg("-q")
                .arg("-f")
                .arg("S16_LE")
                .arg("-r")
                .arg(SAMPLE_RATE.to_string())
                .arg("-c")
                .arg(CHANNELS.to_string())
                .arg("-t")
                .arg("raw");
            if !device.is_empty() {
                cmd.arg("-D").arg(device);
            }
        }
        CaptureBackend::Ffmpeg => {
            let (format, default_device) = ffmpeg_input();
            cmd.arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-f")
                .arg(format)
                .arg("-i")
                .arg(if device.is_empty() { default_device } else { device })
                .arg("-ar")
                .arg(SAMPLE_RATE.to_string())
                .arg("-ac")
                .arg(CHANNELS.to_string())
                .arg("-f")
                .arg("s16le")
                .arg("pipe:1");
        }
        CaptureBackend::Sox => {
            if !device.is_empty() {
                cmd.env("AUDIODEV", device);
            }
            cmd.arg("-q")
                .arg("-d")
                .arg("-r")
                .arg(SAMPLE_RATE.to_string())
                .arg("-c")
                .arg(CHANNELS.to_string())
                .arg("-t")
                .arg("raw")
                .arg("-");
        }
    }

    cmd
}

/// An in-flight recording: the recorder child process plus the raw PCM path it writes to.
pub struct Recorder {
    child: Child,
    raw_path: PathBuf,
}

impl Recorder {
    /// Starts a recorder writing raw PCM to a fresh temp file.
    pub fn spawn(cfg: &VoiceConfig) -> Result<Self> {
        let backend = resolve_backend(cfg)?;
        let raw_path = new_temp_path("raw")?;
        let mut cmd = build_command(backend, cfg, &raw_path);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start `{}`", backend.binary()))?;
        Ok(Self { child, raw_path })
    }

    /// Stops the recorder and returns the finalized WAV path.
    pub async fn stop_and_finalize(mut self) -> Result<PathBuf> {
        stop_child(&mut self.child).await;
        let wav = self.raw_path.with_extension("wav");
        finalize_wav(&self.raw_path, &wav)?;
        let _ = std::fs::remove_file(&self.raw_path);
        Ok(wav)
    }
}

/// A continuous recorder streaming raw s16le PCM on its stdout.
pub struct StreamRecorder {
    child: Child,
    stdout: Option<tokio::process::ChildStdout>,
}

impl StreamRecorder {
    /// Spawns a recorder whose stdout carries the raw PCM stream.
    pub fn spawn(cfg: &VoiceConfig) -> Result<Self> {
        let backend = resolve_backend(cfg)?;
        let mut cmd = build_stream_command(backend, cfg);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start `{}`", backend.binary()))?;
        let stdout = child.stdout.take();
        Ok(Self { child, stdout })
    }

    /// Takes the PCM stdout handle (can be called once).
    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.stdout.take()
    }

    /// Stops the recorder.
    pub async fn stop(&mut self) {
        stop_child(&mut self.child).await;
    }
}

/// Best-effort graceful stop (SIGINT so the recorder flushes), then a hard kill fallback.
async fn stop_child(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let _ = Command::new("kill")
            .arg("-INT")
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }

    match tokio::time::timeout(std::time::Duration::from_millis(600), child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

/// Creates a fresh temp path for a WAV file with a unique name.
pub fn new_temp_wav_path() -> Result<PathBuf> {
    new_temp_path("wav")
}

fn new_temp_path(ext: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("spiritty");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create temp directory {:?}", dir))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(dir.join(format!("voice-{}-{}.{}", std::process::id(), nanos, ext)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_a_valid_canonical_pcm_header() {
        let data_len = 32_000u32; // 1 s of 16 kHz mono s16le
        let header = wav_header(data_len, SAMPLE_RATE, CHANNELS, BITS);

        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 36 + data_len);
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(header[16..20].try_into().unwrap()), 16);
        assert_eq!(u16::from_le_bytes(header[20..22].try_into().unwrap()), 1); // PCM
        assert_eq!(u16::from_le_bytes(header[22..24].try_into().unwrap()), 1); // mono
        assert_eq!(u32::from_le_bytes(header[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u32::from_le_bytes(header[28..32].try_into().unwrap()), 32_000); // byte rate
        assert_eq!(u16::from_le_bytes(header[32..34].try_into().unwrap()), 2); // block align
        assert_eq!(u16::from_le_bytes(header[34..36].try_into().unwrap()), 16);
        assert_eq!(&header[36..40], b"data");
        assert_eq!(u32::from_le_bytes(header[40..44].try_into().unwrap()), data_len);
        assert_eq!(header.len(), 44);
    }

    #[test]
    fn arecord_command_uses_raw_16k_mono() {
        let cfg = VoiceConfig {
            capture_backend: "arecord".to_string(),
            ..VoiceConfig::default()
        };
        let cmd = build_command(CaptureBackend::Arecord, &cfg, Path::new("/tmp/out.raw"));
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "arecord");
        let args: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"S16_LE".to_string()));
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"raw".to_string()));
        assert_eq!(args.last().unwrap(), "/tmp/out.raw");
    }

    #[test]
    fn ffmpeg_command_targets_raw_s16le() {
        let cfg = VoiceConfig::default();
        let cmd = build_command(CaptureBackend::Ffmpeg, &cfg, Path::new("/tmp/out.raw"));
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "ffmpeg");
        let args: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"s16le".to_string()));
        assert!(args.contains(&"-i".to_string()));
    }

    #[test]
    fn explicit_recorder_bin_is_used() {
        let cfg = VoiceConfig {
            recorder_bin: "/bin/sh".to_string(),
            capture_backend: "sox".to_string(),
            ..VoiceConfig::default()
        };
        assert_eq!(resolve_backend(&cfg).unwrap(), CaptureBackend::Sox);
        let cmd = build_command(CaptureBackend::Sox, &cfg, Path::new("/tmp/out.raw"));
        assert_eq!(cmd.as_std().get_program(), "/bin/sh");
    }

    #[test]
    fn explicit_backend_is_honored_when_available() {
        // `arecord` may not be installed in CI; only assert the explicit-path branch.
        let cfg = VoiceConfig {
            capture_backend: "arecord".to_string(),
            recorder_bin: "/bin/sh".to_string(),
            ..VoiceConfig::default()
        };
        assert_eq!(resolve_backend(&cfg).unwrap(), CaptureBackend::Arecord);
    }

    #[test]
    fn stream_command_does_not_write_a_file() {
        let cfg = VoiceConfig::default();
        let cmd = build_stream_command(CaptureBackend::Arecord, &cfg);
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "arecord");
        let args: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        // No trailing output path: arecord writes raw PCM to stdout.
        assert_eq!(args.last().unwrap(), "raw");
        assert!(args.contains(&"16000".to_string()));
    }

    #[test]
    fn write_wav_bytes_prefixes_a_44_byte_header() {
        let pcm = vec![0u8; 3200]; // 100 ms of s16le mono
        let dir = std::env::temp_dir().join("spiritty");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test-{}.wav", std::process::id()));
        write_wav_bytes(&pcm, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 44 + pcm.len());
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[36..40], b"data");
        let _ = std::fs::remove_file(&path);
    }
}
