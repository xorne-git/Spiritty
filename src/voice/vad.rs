//! Energy-based voice activity detection for continuous (hands-free) dictation.
//!
//! Frames of 16 kHz mono s16le audio are reduced to an RMS level; a segment is considered
//! finished after a configurable run of silence, which is what lets Spiritty transcribe and
//! send a sentence automatically when the speaker stops talking.
//!
//! The decision logic is pure (no I/O) so every timing branch is unit-testable.

use crate::config::VoiceConfig;

/// Frame duration used for silence accounting (30 ms ≈ 480 samples @ 16 kHz).
pub const FRAME_MS: u64 = 30;
/// Byte size of one frame of 16 kHz mono s16le audio (480 samples × 2 bytes).
pub const FRAME_BYTES: usize = (16_000 / 1000 * FRAME_MS as usize) * 2;

/// What the caller should do after feeding a frame to [`Vad::push_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadDecision {
    /// Keep recording.
    Continue,
    /// Close the segment; `usable` is false for pure-silence or too-short segments.
    Complete { usable: bool },
}

/// Pure energy VAD state machine. Feed it one RMS value per [`FRAME_MS`] frame.
#[derive(Debug)]
pub struct Vad {
    threshold: f64,
    silence_ms: u64,
    min_speech_ms: u64,
    max_segment_ms: u64,
    elapsed_ms: u64,
    speech_ms: u64,
    silence_run_ms: u64,
    has_speech: bool,
}

impl Vad {
    pub fn new(cfg: &VoiceConfig) -> Self {
        Self {
            threshold: cfg.vad_threshold.max(0.0),
            silence_ms: cfg.silence_ms,
            min_speech_ms: cfg.min_speech_ms,
            max_segment_ms: cfg.max_segment_ms.max(FRAME_MS),
            elapsed_ms: 0,
            speech_ms: 0,
            silence_run_ms: 0,
            has_speech: false,
        }
    }

    /// Clears all state so the instance can be reused for the next segment.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
        self.speech_ms = 0;
        self.silence_run_ms = 0;
        self.has_speech = false;
    }

    /// Whether the current (finished) segment contains enough speech to be transcribed.
    pub fn is_usable(&self) -> bool {
        self.has_speech && self.speech_ms >= self.min_speech_ms
    }

    /// Feeds one frame's RMS level (0.0–1.0) and reports what to do next.
    pub fn push_frame(&mut self, rms: f32) -> VadDecision {
        self.elapsed_ms += FRAME_MS;

        if rms as f64 >= self.threshold {
            self.has_speech = true;
            self.silence_run_ms = 0;
            self.speech_ms += FRAME_MS;
        } else if self.has_speech {
            self.silence_run_ms += FRAME_MS;
        }

        if self.elapsed_ms >= self.max_segment_ms {
            return VadDecision::Complete {
                usable: self.is_usable(),
            };
        }
        if self.has_speech && self.silence_run_ms >= self.silence_ms {
            return VadDecision::Complete {
                usable: self.is_usable(),
            };
        }
        VadDecision::Continue
    }
}

/// Root-mean-square level (0.0–1.0) of a block of 16-bit signed little-endian samples.
pub fn rms_s16le(bytes: &[u8]) -> f32 {
    let mut sum = 0f64;
    let mut count = 0u64;
    for chunk in bytes.as_chunks::<2>().0 {
        let sample = i16::from_le_bytes(*chunk) as f64;
        sum += sample * sample;
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    ((sum / count as f64).sqrt() / 32_768.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(silence_ms: u64, min_speech_ms: u64, max_segment_ms: u64) -> VoiceConfig {
        VoiceConfig {
            silence_ms,
            min_speech_ms,
            max_segment_ms,
            vad_threshold: 0.1,
            ..VoiceConfig::default()
        }
    }

    #[test]
    fn rms_of_silence_is_zero_and_full_scale_is_one() {
        assert_eq!(rms_s16le(&[0, 0, 0, 0]), 0.0);
        let full = i16::MAX.to_le_bytes();
        let bytes = [full[0], full[1], full[0], full[1]];
        assert!((rms_s16le(&bytes) - 1.0).abs() < 0.001);
        assert_eq!(rms_s16le(&[]), 0.0);
    }

    #[test]
    fn silence_only_segment_is_not_usable() {
        // Pure silence never triggers the speech→silence transition, so the max-segment cap
        // is the only way out — and the segment must be reported unusable.
        let mut vad = Vad::new(&cfg(90, 60, 150));
        let mut result = None;
        for _ in 0..10 {
            if let VadDecision::Complete { usable } = vad.push_frame(0.0) {
                result = Some(usable);
                break;
            }
        }
        assert_eq!(result, Some(false));
    }

    #[test]
    fn speech_then_silence_completes_as_usable() {
        let mut vad = Vad::new(&cfg(90, 60, 10_000));
        // 120 ms of speech (4 frames), then enough silence.
        for _ in 0..4 {
            assert_eq!(vad.push_frame(0.5), VadDecision::Continue);
        }
        let mut done = None;
        for _ in 0..5 {
            if let VadDecision::Complete { usable } = vad.push_frame(0.0) {
                done = Some(usable);
                break;
            }
        }
        assert_eq!(done, Some(true));
        assert!(vad.is_usable());
    }

    #[test]
    fn too_short_speech_is_not_usable() {
        let mut vad = Vad::new(&cfg(60, 300, 10_000));
        // One 30 ms speech frame (< 300 ms min), then silence.
        assert_eq!(vad.push_frame(0.5), VadDecision::Continue);
        let mut result = None;
        for _ in 0..5 {
            if let VadDecision::Complete { usable } = vad.push_frame(0.0) {
                result = Some(usable);
                break;
            }
        }
        assert_eq!(result, Some(false));
    }

    #[test]
    fn long_speech_is_capped_by_max_segment() {
        let mut vad = Vad::new(&cfg(100_000, 60, 150));
        // Never any silence: the only way out is the max-segment cap.
        let mut completed = false;
        for _ in 0..10 {
            if let VadDecision::Complete { usable } = vad.push_frame(0.5) {
                assert!(usable);
                completed = true;
                break;
            }
        }
        assert!(completed, "max_segment_ms must eventually close the segment");
    }

    #[test]
    fn reset_clears_state_for_the_next_segment() {
        let mut vad = Vad::new(&cfg(60, 60, 10_000));
        for _ in 0..4 {
            vad.push_frame(0.5);
        }
        vad.reset();
        assert!(!vad.is_usable());
        assert_eq!(vad.push_frame(0.0), VadDecision::Continue);
    }
}
