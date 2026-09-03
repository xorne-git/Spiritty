pub mod capture;
pub mod process;
pub mod vt;

pub use capture::{
    build_capture_summary, capture_debug, char_safe_floor, char_safe_tail, clean_pty_output,
    is_prompt_remnant, is_waiting_for_password, is_waiting_for_user_interaction,
    scan_completed_sentinel, strip_ansi_sequences, utf8_decode_incremental, IngestOutcome,
    InteractionKind, TickOutcome, ToolCaptureSession, DEFAULT_CAPTURE_TIMEOUT_SECS,
    MAX_CAPTURE_BYTES, PASSWORD_WAIT_HARD_CAP_SECS, PASSWORD_WINDOW_BYTES,
};
pub use process::PtyProcess;
#[allow(unused_imports)]
pub use vt::VtScreen;
