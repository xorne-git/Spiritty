//! 100% local voice input.
//!
//! The microphone is captured by an external recorder (`arecord` / `ffmpeg` / `sox`) and
//! transcribed locally by `whisper.cpp`; the resulting text is injected into the chat
//! prompt. Nothing ever leaves the machine.
//!
//! Two modes are supported:
//! - **Manual (`F8`)**: tap to record a segment, tap again to stop and transcribe.
//! - **Continuous (`F7`)**: the recorder keeps streaming; an energy VAD closes the segment
//!   after a short silence, which is then transcribed and (optionally) submitted — the
//!   hands-free huddle behaviour.
//!
//! Concurrency respects the project invariant "never block the event loop": capture and
//! transcription run inside dedicated `tokio` tasks owned by [`VoiceController`], and all
//! progress/results travel back to the UI exclusively through typed [`AppEvent`]s.

pub mod capture;
pub mod transcribe;
pub mod vad;

use crate::event::AppEvent;
use crate::i18n::Language;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;

/// Lifecycle of the local voice input, mirrored in the UI (footer badge / prompt hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceState {
    /// Feature disabled in the config.
    #[default]
    Off,
    /// Enabled and waiting for a segment.
    Idle,
    /// Capturing audio.
    Recording,
    /// Running local transcription.
    Transcribing,
}

impl VoiceState {
    /// Whether a segment is currently in flight (recording or transcribing).
    pub fn is_busy(self) -> bool {
        matches!(self, VoiceState::Recording | VoiceState::Transcribing)
    }
}

#[derive(Debug)]
enum VoiceCommand {
    StartSegment,
    StopSegment,
    StartContinuous,
    StopContinuous,
    Shutdown,
}

/// Public façade held by [`crate::app::App`]. Commands are fire-and-forget; the tasks report
/// progress and results back exclusively through [`AppEvent`].
pub struct VoiceController {
    cmd_tx: UnboundedSender<VoiceCommand>,
}

impl VoiceController {
    /// Spawns the voice task. `ui` is used to resolve the recognition language when the
    /// config leaves `voice.language` empty.
    pub fn spawn(
        config: crate::config::VoiceConfig,
        ui: Language,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_voice_task(config, ui, event_tx, cmd_rx));
        Self { cmd_tx }
    }

    /// Starts capturing a manual segment (`F8`).
    pub fn start_segment(&self) {
        let _ = self.cmd_tx.send(VoiceCommand::StartSegment);
    }

    /// Stops the current manual segment and launches local transcription.
    pub fn stop_segment(&self) {
        let _ = self.cmd_tx.send(VoiceCommand::StopSegment);
    }

    /// Starts continuous, silence-detected dictation (`F7`).
    pub fn start_continuous(&self) {
        let _ = self.cmd_tx.send(VoiceCommand::StartContinuous);
    }

    /// Stops continuous dictation.
    pub fn stop_continuous(&self) {
        let _ = self.cmd_tx.send(VoiceCommand::StopContinuous);
    }

    /// Best-effort shutdown (finalizes any in-flight recording).
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(VoiceCommand::Shutdown);
    }
}

async fn run_voice_task(
    config: crate::config::VoiceConfig,
    ui: Language,
    event_tx: UnboundedSender<AppEvent>,
    mut cmd_rx: UnboundedReceiver<VoiceCommand>,
) {
    let mut recorder: Option<capture::Recorder> = None;
    let mut continuous_stop: Option<Arc<Notify>> = None;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            VoiceCommand::StartSegment => {
                // Manual segments are ignored while continuous dictation owns the mic.
                if continuous_stop.is_some() || recorder.is_some() {
                    continue;
                }
                match capture::Recorder::spawn(&config) {
                    Ok(rec) => {
                        recorder = Some(rec);
                        let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Recording));
                    }
                    Err(e) => report_error(&event_tx, e.to_string()),
                }
            }
            VoiceCommand::StopSegment => {
                let Some(rec) = recorder.take() else { continue };
                let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Transcribing));
                match rec.stop_and_finalize().await {
                    Ok(wav) => {
                        transcribe_file(&config, ui, &event_tx, &wav).await;
                        let _ = std::fs::remove_file(&wav);
                    }
                    Err(e) => report_error(&event_tx, e.to_string()),
                }
                let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Idle));
            }
            VoiceCommand::StartContinuous => {
                if continuous_stop.is_some() {
                    continue;
                }
                let stop = Arc::new(Notify::new());
                continuous_stop = Some(stop.clone());
                tokio::spawn(run_continuous(config.clone(), ui, event_tx.clone(), stop));
            }
            VoiceCommand::StopContinuous => {
                if let Some(stop) = continuous_stop.take() {
                    stop.notify_one();
                }
            }
            VoiceCommand::Shutdown => {
                if let Some(rec) = recorder.take() {
                    let _ = rec.stop_and_finalize().await;
                }
                if let Some(stop) = continuous_stop.take() {
                    stop.notify_one();
                }
                break;
            }
        }
    }
}

/// Runs continuous dictation until `stop` is notified: records a segment, closes it on
/// silence (or the max-segment cap), transcribes it, then immediately starts the next one.
async fn run_continuous(
    config: crate::config::VoiceConfig,
    ui: Language,
    event_tx: UnboundedSender<AppEvent>,
    stop: Arc<Notify>,
) {
    loop {
        let mut rec = match capture::StreamRecorder::spawn(&config) {
            Ok(rec) => rec,
            Err(e) => {
                report_error(&event_tx, e.to_string());
                let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Idle));
                return;
            }
        };
        let Some(mut stdout) = rec.take_stdout() else {
            rec.stop().await;
            report_error(&event_tx, "recorder produced no audio stream".to_string());
            let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Idle));
            return;
        };

        let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Recording));

        let mut vad = vad::Vad::new(&config);
        let mut pcm: Vec<u8> = Vec::new();
        let mut carry: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];

        let end = loop {
            tokio::select! {
                _ = stop.notified() => break SegmentEnd::Stopped,
                read = stdout.read(&mut buf) => {
                    match read {
                        Ok(0) => break SegmentEnd::Ended,
                        Ok(n) => {
                            pcm.extend_from_slice(&buf[..n]);
                            carry.extend_from_slice(&buf[..n]);
                            let mut decision = None;
                            while carry.len() >= vad::FRAME_BYTES {
                                let frame: Vec<u8> = carry.drain(..vad::FRAME_BYTES).collect();
                                if let vad::VadDecision::Complete { usable } = vad.push_frame(vad::rms_s16le(&frame)) {
                                    decision = Some(usable);
                                    break;
                                }
                            }
                            if let Some(usable) = decision {
                                break SegmentEnd::Complete { usable };
                            }
                        }
                        Err(_) => break SegmentEnd::Ended,
                    }
                }
            }
        };

        rec.stop().await;

        match end {
            SegmentEnd::Stopped => {
                let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Idle));
                return;
            }
            SegmentEnd::Ended => {
                let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Idle));
                return;
            }
            SegmentEnd::Complete { usable } => {
                if usable && !pcm.is_empty() {
                    if let Err(e) = write_and_transcribe(&config, ui, &event_tx, &pcm).await {
                        report_error(&event_tx, e.to_string());
                    }
                }
                // Loop back to listen again; a pending `stop` notify is honoured at the top
                // of the next iteration (Notify stores a permit for `notify_one`).
            }
        }
    }
}

enum SegmentEnd {
    /// The user disabled continuous mode.
    Stopped,
    /// The VAD closed the segment (may be unusable, e.g. pure silence).
    Complete { usable: bool },
    /// The recorder stream ended unexpectedly.
    Ended,
}

async fn write_and_transcribe(
    config: &crate::config::VoiceConfig,
    ui: Language,
    event_tx: &UnboundedSender<AppEvent>,
    pcm: &[u8],
) -> anyhow::Result<()> {
    let wav = capture::new_temp_wav_path()?;
    capture::write_wav_bytes(pcm, &wav)?;
    let _ = event_tx.send(AppEvent::VoiceStateChanged(VoiceState::Transcribing));
    transcribe_file(config, ui, event_tx, &wav).await;
    let _ = std::fs::remove_file(&wav);
    Ok(())
}

async fn transcribe_file(
    config: &crate::config::VoiceConfig,
    ui: Language,
    event_tx: &UnboundedSender<AppEvent>,
    wav: &Path,
) {
    match transcribe::transcribe(config, ui, wav).await {
        Ok(text) => {
            if !text.trim().is_empty() {
                let _ = event_tx.send(AppEvent::VoiceTranscript {
                    text,
                    auto_submit: config.auto_submit,
                });
            }
        }
        Err(e) => report_error(event_tx, e.to_string()),
    }
}

fn report_error(event_tx: &UnboundedSender<AppEvent>, msg: String) {
    let _ = event_tx.send(AppEvent::VoiceError(msg));
}
