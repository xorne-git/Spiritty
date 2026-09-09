use std::time::{Duration, Instant};

/// Maximum bytes buffered in memory for one single PTY tool capture (1 MiB).
/// Beyond this cap, bytes are counted and scanned for sentinel in a rolling tail,
/// with an explicit truncation notice added to the final summary.
pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

/// Rolling tail window inspected for interactive password or confirmation prompts.
pub const PASSWORD_WINDOW_BYTES: usize = 512;

/// Extended timeout cap when an interactive prompt is detected (120 seconds).
pub const PASSWORD_WAIT_HARD_CAP_SECS: u64 = 120;

/// Default hard timeout for command capture when no sentinel arrives (45 seconds).
pub const DEFAULT_CAPTURE_TIMEOUT_SECS: u64 = 45;

/// Appends a debug message to `/tmp/spiritty_capture.log` if enabled.
pub fn capture_debug(msg: &str) {
    if std::env::var("SPIRITTY_CAPTURE_DEBUG").is_ok() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/spiritty_capture.log")
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let _ = writeln!(f, "[{} ms] {}", now, msg);
        }
    }
}

/// Category of human interaction detected at the live terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Password,
    Confirmation,
    Pager,
}

/// Outcome of ingesting newly arrived PTY output bytes into a capture session.
#[derive(Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    /// Capture is still ongoing.
    Continuing,
    /// An interactive prompt was detected; human intervention requested.
    InteractionRequired(InteractionKind),
    /// Command finished definitively (e.g. via completion sentinel or overflow sentinel).
    Concluded {
        command: String,
        summary: String,
        auto_prompt: bool,
    },
}

/// Outcome of a periodic tick inspection on a capture session.
#[derive(Debug, PartialEq, Eq)]
pub enum TickOutcome {
    /// Capture is still within bounded wait windows.
    Continuing,
    /// Capture reached conclusion (settled by prompt/silence or reached hard timeout).
    Concluded {
        command: String,
        summary: String,
        auto_prompt: bool,
    },
}

/// Deep module managing the full lifecycle of an autonomous tool execution in a live PTY.
///
/// Encapsulates:
/// - Incremental UTF-8 decoding with multi-byte carry buffers across read chunks.
/// - Incremental sentinel scanning (`\x1b]777;spiritty_done;` and `__SPIRITTY_DONE__:`) with high-watermark.
/// - 1 MiB overflow protection with rolling-tail completion detection.
/// - Universal interactive prompt detection (`InteractionKind`) and extended wait timeout.
/// - Silence and prompt-aware settle heuristics for unhooked shells (remote SSH, sh, dash).
/// - Safe cancellation with SIGINT injection and guaranteed channel delivery on Drop.
pub struct ToolCaptureSession {
    pub command: String,
    pub auto_prompt: bool,
    pub result_tx: Option<tokio::sync::oneshot::Sender<String>>,
    pub output_bytes: Vec<u8>,
    pub decoded_text: String,
    pub pending_utf8: Vec<u8>,
    pub sentinel_pending: Option<(usize, usize, bool)>,
    pub sentinel_scan_upto: usize,
    pub start_time: Instant,
    pub last_output_time: Instant,
    pub truncated: bool,
    pub overflow_bytes: u64,
    pub overflow_tail: Vec<u8>,
    pub clean_cache: Option<(usize, String)>,
}

impl ToolCaptureSession {
    /// Creates a new active tool capture session.
    pub fn new(
        command: String,
        result_tx: Option<tokio::sync::oneshot::Sender<String>>,
        auto_prompt: bool,
    ) -> Self {
        let now = Instant::now();
        Self {
            command,
            auto_prompt,
            result_tx,
            output_bytes: Vec::new(),
            decoded_text: String::new(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: now,
            last_output_time: now,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        }
    }

    /// Ingests a slice of newly arrived raw bytes from the master PTY reader.
    pub fn ingest(&mut self, bytes: &[u8]) -> IngestOutcome {
        self.last_output_time = Instant::now();

        // 1. Handle 1 MiB overflow mode
        if self.output_bytes.len() >= MAX_CAPTURE_BYTES {
            self.truncated = true;
            self.overflow_bytes += bytes.len() as u64;
            self.overflow_tail.extend_from_slice(bytes);
            let stale = self.overflow_tail.len().saturating_sub(256);
            if stale > 0 {
                self.overflow_tail.drain(..stale);
            }
            if let Some(exit_code) = scan_completed_sentinel(&self.overflow_tail) {
                let clean_output = clean_pty_output(&self.decoded_text, &self.command);
                let empty_summary = if exit_code == 0 {
                    "(Commande exécutée avec succès dans le terminal)".to_string()
                } else {
                    format!("(Commande terminée avec le code {})", exit_code)
                };
                let final_summary = build_capture_summary(
                    exit_code,
                    clean_output,
                    self.truncated,
                    self.overflow_bytes,
                    self.decoded_text.len(),
                    empty_summary,
                );
                return self.conclude(final_summary);
            }
            return IngestOutcome::Continuing;
        }

        // 2. Normal buffering and incremental UTF-8 decode
        self.output_bytes.extend_from_slice(bytes);
        self.pending_utf8.extend_from_slice(bytes);
        utf8_decode_incremental(&mut self.decoded_text, &mut self.pending_utf8);

        // 3. Scan for interactive prompts at the tail
        let mut interaction_detected = None;
        if let Some(kind) = is_waiting_for_user_interaction(char_safe_tail(
            &self.decoded_text,
            PASSWORD_WINDOW_BYTES,
        )) {
            interaction_detected = Some(kind);
        }

        // 4. Locate OSC 777 / plain sentinel incrementally
        const OSC_PREFIX_LEN: usize = "\x1b]777;spiritty_done;".len();
        const PLAIN_PREFIX_LEN: usize = "__SPIRITTY_DONE__:".len();

        if self.sentinel_pending.is_none() {
            let max_prefix = OSC_PREFIX_LEN.max(PLAIN_PREFIX_LEN);
            let from = char_safe_floor(
                &self.decoded_text,
                self.sentinel_scan_upto.saturating_sub(max_prefix - 1),
            );
            let hay = &self.decoded_text[from..];
            if let Some(rel) = hay.rfind("\x1b]777;spiritty_done;") {
                capture_debug(&format!(
                    "SENTINEL OSC SEEN at +{}ms (len={})",
                    self.start_time.elapsed().as_millis(),
                    self.decoded_text.len()
                ));
                self.sentinel_pending = Some((from + rel, OSC_PREFIX_LEN, true));
                self.sentinel_scan_upto = from + rel + 1;
            } else if let Some(rel) = hay.rfind("__SPIRITTY_DONE__:") {
                capture_debug("SENTINEL PLAIN SEEN");
                self.sentinel_pending = Some((from + rel, PLAIN_PREFIX_LEN, false));
                self.sentinel_scan_upto = from + rel + 1;
            } else {
                self.sentinel_scan_upto = self.decoded_text.len();
            }
        }

        if let Some((pos, prefix_len, is_osc)) = self.sentinel_pending {
            let text = &self.decoded_text;
            if pos + prefix_len <= text.len() {
                let after = &text[pos + prefix_len..];
                let found_terminator = if is_osc {
                    after
                        .find('\x1b')
                        .or(after.find('\x07'))
                        .or(after.find('\n'))
                        .or(after.find('\r'))
                        .or(after.find('\\'))
                        .or(after.find(';'))
                } else {
                    after.find('\n').or(after.find('\r'))
                };

                if let Some(end_idx) = found_terminator {
                    let code_str = after[..end_idx].trim_matches(|c: char| !c.is_ascii_digit());
                    let exit_code: i32 = code_str.parse().unwrap_or(0);
                    let raw_output = &text[..pos];
                    let clean_output = clean_pty_output(raw_output, &self.command);
                    capture_debug(&format!(
                        "CONCLUDED via=SENTINEL code={} elapsed={}ms raw_len={} clean_len={}",
                        exit_code,
                        self.start_time.elapsed().as_millis(),
                        raw_output.len(),
                        clean_output.len()
                    ));
                    let empty_summary = if exit_code == 0 {
                        "(Commande exécutée avec succès dans le terminal)".to_string()
                    } else {
                        format!("(Commande terminée avec le code {})", exit_code)
                    };
                    let final_summary = build_capture_summary(
                        exit_code,
                        clean_output,
                        self.truncated,
                        self.overflow_bytes,
                        self.decoded_text.len(),
                        empty_summary,
                    );
                    return self.conclude(final_summary);
                }
            }
        }

        if let Some(kind) = interaction_detected {
            IngestOutcome::InteractionRequired(kind)
        } else {
            IngestOutcome::Continuing
        }
    }

    /// Inspects timeouts, silence settle, and prompt remnant heuristics on clock ticks.
    pub fn tick(&mut self, shell_has_hooks: bool) -> TickOutcome {
        let elapsed_since_start = self.start_time.elapsed();
        let elapsed_since_last_output = self.last_output_time.elapsed();
        let interaction_opt = is_waiting_for_user_interaction(char_safe_tail(
            &self.decoded_text,
            PASSWORD_WINDOW_BYTES,
        ));
        let is_waiting_interaction = matches!(
            interaction_opt,
            Some(InteractionKind::Password) | Some(InteractionKind::Confirmation)
        );

        let mut timeout_secs = DEFAULT_CAPTURE_TIMEOUT_SECS;
        if is_waiting_interaction {
            timeout_secs = PASSWORD_WAIT_HARD_CAP_SECS;
        }

        let last_output_line =
            strip_ansi_sequences(char_safe_tail(&self.decoded_text, PASSWORD_WINDOW_BYTES))
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_default();
        let remote_prompt_quiet = !last_output_line.is_empty()
            && !shell_has_hooks
            && !is_waiting_interaction
            && elapsed_since_last_output >= Duration::from_millis(800)
            && is_prompt_remnant(&last_output_line);

        let has_output_settled = !shell_has_hooks
            && !is_waiting_interaction
            && !self.decoded_text.is_empty()
            && elapsed_since_start >= Duration::from_millis(800)
            && (elapsed_since_last_output >= Duration::from_millis(3000) || remote_prompt_quiet);

        let timeout_reached = elapsed_since_start >= Duration::from_secs(timeout_secs);

        let mut clean_is_empty = false;
        if has_output_settled && !timeout_reached {
            let current_len = self.decoded_text.len();
            match &self.clean_cache {
                Some((cached_len, cached_str)) if *cached_len == current_len => {
                    clean_is_empty = cached_str.is_empty();
                }
                _ => {
                    let cleaned = clean_pty_output(&self.decoded_text, &self.command);
                    self.clean_cache = Some((current_len, cleaned));
                    clean_is_empty = self
                        .clean_cache
                        .as_ref()
                        .map(|(_, s)| s.is_empty())
                        .unwrap_or(true);
                }
            }
        }
        let may_conclude = (!clean_is_empty && has_output_settled) || timeout_reached;

        if may_conclude {
            capture_debug(&format!(
                "CONCLUDED via={} elapsed={}ms len={} clean_is_empty={} truncated={} waiting_interaction={}",
                if timeout_reached { "TIMEOUT" } else { "SETTLE" },
                elapsed_since_start.as_millis(),
                self.decoded_text.len(),
                clean_is_empty,
                self.truncated,
                is_waiting_interaction
            ));
            let clean_output = match &self.clean_cache {
                Some((l, s)) if *l == self.decoded_text.len() => s.clone(),
                _ => clean_pty_output(&self.decoded_text, &self.command),
            };
            let final_summary = build_capture_summary(
                0,
                clean_output,
                self.truncated,
                self.overflow_bytes,
                self.decoded_text.len(),
                "⚠️ Aucune sortie capturée : la commande a été injectée mais le terminal n'a rien renvoyé d'exploitable. Le shell distant n'était peut-être pas prêt, la session a pu être interrompue, ou la commande n'a produit ni sortie ni erreur. Vérifie l'état du terminal (invite visible ?) et relance une commande minimale (ex. `pwd`) si besoin.".to_string(),
            );

            let command = self.command.clone();
            let auto_prompt = self.auto_prompt;
            if let Some(tx) = self.result_tx.take() {
                let _ = tx.send(final_summary.clone());
            }

            TickOutcome::Concluded {
                command,
                summary: final_summary,
                auto_prompt,
            }
        } else {
            TickOutcome::Continuing
        }
    }

    /// Cancels the capture session and delivers an explicit diagnostic message to the oneshot
    /// sender, preventing RecvError. Returns the `SIGINT` bytes (`b"\x03"`) to be written to the PTY.
    pub fn cancel(&mut self) -> &'static [u8] {
        if let Some(tx) = self.result_tx.take() {
            let _ = tx.send("(Génération interrompue par l'utilisateur)".to_string());
        }
        b"\x03"
    }

    fn conclude(&mut self, final_summary: String) -> IngestOutcome {
        let command = self.command.clone();
        let auto_prompt = self.auto_prompt;
        if let Some(tx) = self.result_tx.take() {
            let _ = tx.send(final_summary.clone());
        }
        IngestOutcome::Concluded {
            command,
            summary: final_summary,
            auto_prompt,
        }
    }
}

impl Drop for ToolCaptureSession {
    fn drop(&mut self) {
        if let Some(tx) = self.result_tx.take() {
            let _ = tx.send("(Session de capture interrompue)".to_string());
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Pure Parsing & Formatting Functions
// ------------------------------------------------------------------------------------------------

/// Scans raw bytes for a completed OSC 777 or plain sentinel pattern.
pub fn scan_completed_sentinel(bytes: &[u8]) -> Option<i32> {
    const OSC_PREFIX: &[u8] = b"\x1b]777;spiritty_done;";
    const PLAIN_PREFIX: &[u8] = b"__SPIRITTY_DONE__:";

    let (prefix, is_osc) = if let Some(idx) = bytes.windows(OSC_PREFIX.len()).rposition(|w| w == OSC_PREFIX) {
        (&bytes[idx + OSC_PREFIX.len()..], true)
    } else {
        let idx = bytes.windows(PLAIN_PREFIX.len()).rposition(|w| w == PLAIN_PREFIX)?;
        (&bytes[idx + PLAIN_PREFIX.len()..], false)
    };

    let found_terminator = if is_osc {
        prefix.iter().position(|&b| {
            b == 0x1b || b == 0x07 || b == b'\n' || b == b'\r' || b == b'\\' || b == b';'
        })
    } else {
        prefix.iter().position(|&b| b == b'\n' || b == b'\r')
    };

    found_terminator.map(|end_idx| {
        let code_bytes = &prefix[..end_idx];
        let code_str = std::str::from_utf8(code_bytes).unwrap_or("0");
        let cleaned: String = code_str.chars().filter(|c| c.is_ascii_digit()).collect();
        cleaned.parse::<i32>().unwrap_or(0)
    })
}

/// Checks if the raw terminal output indicates that a process is waiting for user interaction
/// (sudo/doas/su password, ssh passphrase, [y/n] confirmation, press enter, etc.)
pub fn is_waiting_for_user_interaction(raw_text: &str) -> Option<InteractionKind> {
    let clean = strip_ansi_sequences(raw_text);
    let trimmed = clean.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    let last_line = lower.lines().next_back().unwrap_or(&lower).trim();

    // 1. Password / Passphrase / Sudo / Authentication
    if last_line.contains("password for")
        || last_line.contains("mot de passe de")
        || last_line.contains("mot de passe pour")
        || last_line.contains("mot de passe :")
        || last_line.contains("mot de passe:")
        || last_line.contains("password:")
        || last_line.contains("password :")
        || last_line.contains("passphrase")
        || last_line.contains("authentication required")
        || last_line.contains("authenticating")
        || last_line.contains("doas password")
        || last_line.starts_with("[sudo]")
    {
        return Some(InteractionKind::Password);
    }

    // 2. Interactive confirmation prompts ([y/n], [o/n], continue connecting, etc.)
    if last_line.contains("[y/n]")
        || last_line.contains("[o/n]")
        || last_line.contains("(y/n)")
        || last_line.contains("(o/n)")
        || last_line.contains("[yes/no]")
        || last_line.contains("(yes/no")
        || last_line.contains("continue connecting (yes/no")
        || last_line.contains("voulez-vous continuer")
        || last_line.contains("souhaitez-vous continuer")
        || last_line.contains("do you want to continue")
        || last_line.contains("proceed with installation")
        || last_line.contains("press enter")
        || last_line.contains("press [enter]")
        || last_line.contains("appuyez sur entrée")
        || last_line.contains("appuyez sur entree")
        || last_line.contains("press any key")
    {
        return Some(InteractionKind::Confirmation);
    }

    // 3. Interactive pagers (less, more, git/systemd pager waiting for 'q')
    if (last_line.starts_with("lines ") && last_line.contains("(end)"))
        || last_line.ends_with("(end)")
        || last_line.ends_with("(end)>")
        || last_line.ends_with("(end)>%")
        || last_line.starts_with("--more--")
        || last_line.starts_with("--plus--")
        || last_line.contains("press 'q' to quit")
        || last_line.contains("press q to quit")
        || last_line.contains("(q to quit)")
        || last_line.ends_with("press return to continue")
    {
        return Some(InteractionKind::Pager);
    }

    None
}

/// Backward compatibility helper for sudo password check.
pub fn is_waiting_for_password(raw_text: &str) -> bool {
    matches!(is_waiting_for_user_interaction(raw_text), Some(InteractionKind::Password))
}

/// True if a line is (trailing) shell-prompt noise that should not be reported as command output.
/// Handles decorative one-symbol prompts (`∙`, `❯`, `➜`, `λ`, …) and standard `user@host:path$` /
/// `host:~#` style prompts.
pub fn is_prompt_remnant(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    // Single decorative prompt symbol (e.g. `∙`, `❯`, `➜`, `λ`, `›`, `±`)
    if t.chars().count() <= 2 && t.chars().all(|c| "∙•·❯➜λ›±◆✗✔".contains(c)) {
        return true;
    }
    // Standard shell prompt: user@host:path…$ / host:~# / host:/path> (short, ends with a marker).
    // Also matches git-aware prompts like `[user@host:path] branch(+0/-7) ±`.
    let ends_marker = t.ends_with('$')
        || t.ends_with('#')
        || t.ends_with('>')
        || t.ends_with('%')
        || t.ends_with('❯')
        || t.ends_with('➜')
        || t.ends_with('λ')
        || t.ends_with('±');
    if ends_marker && t.len() <= 96 && (t.contains(':') || t.contains('~') || t.contains('@')) {
        return true;
    }
    false
}

/// Decodes incoming bytes into UTF-8 incrementally, maintaining split bytes in `pending`.
pub fn utf8_decode_incremental(text: &mut String, pending: &mut Vec<u8>) {
    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                text.push_str(valid);
                pending.clear();
                break;
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                if valid_up_to > 0 {
                    if let Ok(chunk) = std::str::from_utf8(&pending[..valid_up_to]) {
                        text.push_str(chunk);
                    }
                    pending.drain(..valid_up_to);
                }
                match err.error_len() {
                    Some(bad_len) => {
                        text.push('\u{FFFD}');
                        pending.drain(..bad_len.max(1));
                    }
                    None => break,
                }
            }
        }
    }
}

/// Strips all ANSI escape sequences, CSI controls, and OSC strings from raw PTY text.
pub fn strip_ansi_sequences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    // Consume until terminating char (@ through ~)
                    for c2 in chars.by_ref() {
                        if ('@'..='~').contains(&c2) {
                            break;
                        }
                    }
                    continue;
                } else if next == ']' {
                    chars.next();
                    // OSC sequence: consume until BEL (\x07) or ST (\x1B\\)
                    let mut prev = '\0';
                    for c2 in chars.by_ref() {
                        if c2 == '\x07' || (prev == '\x1B' && c2 == '\\') {
                            break;
                        }
                        prev = c2;
                    }
                    continue;
                } else if next == '(' || next == ')' {
                    chars.next();
                    chars.next(); // Charset designator
                    continue;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Safely floors an index in a UTF-8 slice to avoid slicing mid-codepoint.
pub fn char_safe_floor(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Safely extracts the trailing N bytes of a string without slicing mid-codepoint.
pub fn char_safe_tail(s: &str, tail_bytes: usize) -> &str {
    let start = s.len().saturating_sub(tail_bytes);
    let safe_start = char_safe_floor(s, start);
    &s[safe_start..]
}

/// Builds the model-facing summary of a concluded PTY capture, unifying the sentinel,
/// settle and overflow conclusion paths.
pub fn build_capture_summary(
    exit_code: i32,
    clean_output: String,
    truncated: bool,
    overflow_bytes: u64,
    captured_len: usize,
    empty_output_summary: String,
) -> String {
    let mut body = clean_output;
    if truncated {
        body.push_str(&format!(
            "\n⚠️ [Sortie tronquée : seuls les premiers {} Ko ont été capturés ; {:.1} Mo supplémentaires ont été reçus puis ignorés.]",
            captured_len / 1024,
            overflow_bytes as f64 / (1024.0 * 1024.0)
        ));
    }
    if body.is_empty() {
        empty_output_summary
    } else if exit_code == 0 {
        format!("Sortie dans le terminal:\n{}", body)
    } else {
        format!("Sortie dans le terminal (code {}):\n{}", exit_code, body)
    }
}

/// Cleans captured PTY output by stripping ANSI colors, CRs, and prompt echoes.
pub fn clean_pty_output(raw: &str, command: &str) -> String {
    let no_ansi = strip_ansi_sequences(raw);
    let no_cr = no_ansi.replace('\r', "");
    let no_ctrl: String = no_cr
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    let mut lines: Vec<&str> = no_ctrl.lines().collect();

    lines.retain(|l| {
        let t = l.trim();
        let lower = t.to_lowercase();
        !t.is_empty()
            && !t.contains("spiritty_done")
            && !t.contains("__spiritty")
            && !t.contains("__SPIRITTY")
            && !t.contains("printf '\\e]777")
            && !t.contains("printf '\\033]777")
            && !lower.contains("password for")
            && !lower.contains("mot de passe de")
            && !lower.contains("mot de passe pour")
            && !(lower.starts_with("[sudo]") && lower.contains("password"))
            && !(lower.starts_with("lines ") && lower.contains("(end)"))
            && lower != "(end)"
            && !lower.starts_with("--more--")
    });

    if let Some(first) = lines.first() {
        let first_t = first.trim();
        let cmd_t = command.trim();
        if first_t == cmd_t
            || first_t.starts_with(cmd_t)
            || (first_t.contains(cmd_t)
                && (first_t.contains("printf '\\033]777") || first_t.contains("printf '\\e]777")))
            || (first_t.ends_with(cmd_t) && first_t.len() <= cmd_t.len() + 10)
        {
            lines.remove(0);
        }
    }

    let cmd_t = command.trim();
    if !cmd_t.is_empty() {
        if let Some(first) = lines.first() {
            let first_t = first.trim();
            let fold =
                |s: &str| -> Vec<char> { s.chars().filter(|c| c.is_alphanumeric()).collect() };
            let cmd_fold = fold(cmd_t);
            let line_fold = fold(first_t);
            let plausible_len =
                line_fold.len() >= cmd_fold.len() && line_fold.len() <= cmd_fold.len() * 3 / 2 + 16;
            if plausible_len {
                let mut cursor = 0usize;
                let mut is_subseq = true;
                for &c in &cmd_fold {
                    while cursor < line_fold.len() && line_fold[cursor] != c {
                        cursor += 1;
                    }
                    if cursor == line_fold.len() {
                        is_subseq = false;
                        break;
                    }
                    cursor += 1;
                }
                if is_subseq {
                    lines.remove(0);
                }
            }
        }
    }

    while let Some(last) = lines.last() {
        if is_prompt_remnant(last) {
            lines.pop();
        } else {
            break;
        }
    }

    lines.join("\n").trim().to_string()
}

// ------------------------------------------------------------------------------------------------
// Unit Tests
// ------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentinel_scan_osc() {
        let raw = b"some output\r\n\x1b]777;spiritty_done;0\x07";
        assert_eq!(scan_completed_sentinel(raw), Some(0));

        let non_zero = b"error occurred\r\n\x1b]777;spiritty_done;127\x07";
        assert_eq!(scan_completed_sentinel(non_zero), Some(127));
    }

    #[test]
    fn test_sentinel_scan_plain() {
        let plain = b"data stream\n__SPIRITTY_DONE__:42\n";
        assert_eq!(scan_completed_sentinel(plain), Some(42));
    }

    #[test]
    fn test_interaction_detection_password_and_prompts() {
        assert_eq!(
            is_waiting_for_user_interaction("[sudo] password for user: "),
            Some(InteractionKind::Password)
        );
        assert_eq!(
            is_waiting_for_user_interaction("Enter passphrase for key: "),
            Some(InteractionKind::Password)
        );
        assert_eq!(
            is_waiting_for_user_interaction("Do you want to continue? [Y/n] "),
            Some(InteractionKind::Confirmation)
        );
        assert_eq!(
            is_waiting_for_user_interaction("Voulez-vous continuer ? [O/n] "),
            Some(InteractionKind::Confirmation)
        );
        assert_eq!(
            is_waiting_for_user_interaction("Press [Enter] to continue..."),
            Some(InteractionKind::Confirmation)
        );
        assert_eq!(
            is_waiting_for_user_interaction("lines 1-25/25 (END)>%"),
            Some(InteractionKind::Pager)
        );
        assert_eq!(
            is_waiting_for_user_interaction("--More--(73%)"),
            Some(InteractionKind::Pager)
        );
        assert_eq!(is_waiting_for_user_interaction("normal output line\n"), None);
    }

    #[test]
    fn test_strip_ansi_and_clean_pty_output() {
        let raw = "\x1b[32mhello\x1b[0m world\r\n\x1b]777;spiritty_done;0\x07";
        assert_eq!(clean_pty_output(raw, "echo hello"), "hello world");
    }

    #[test]
    fn test_session_lifecycle_ingest_sentinel() {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let mut session = ToolCaptureSession::new("echo test".to_string(), Some(tx), false);

        let out = session.ingest(b"test\r\n\x1b]777;spiritty_done;0\x07");
        match out {
            IngestOutcome::Concluded { command, summary, auto_prompt } => {
                assert_eq!(command, "echo test");
                assert_eq!(summary, "Sortie dans le terminal:\ntest");
                assert!(!auto_prompt);
            }
            _ => panic!("Expected Concluded outcome"),
        }

        let sent = rx.blocking_recv().unwrap();
        assert_eq!(sent, "Sortie dans le terminal:\ntest");
    }

    #[test]
    fn test_session_cancel_sends_sigint_and_notifies_receiver() {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let mut session = ToolCaptureSession::new("sleep 10".to_string(), Some(tx), false);

        let sigint = session.cancel();
        assert_eq!(sigint, b"\x03");

        let msg = rx.blocking_recv().unwrap();
        assert!(msg.contains("interrompue"));
    }
}
