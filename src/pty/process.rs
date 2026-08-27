use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::{
    env,
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use tokio::sync::mpsc::UnboundedSender;

use super::vt::VtScreen;

pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    /// Sender to the dedicated writer thread: PTY input (keystrokes, tool injections,
    /// terminal-query replies) is enqueued here and flushed off-thread. See `spawn`.
    input_tx: std::sync::mpsc::Sender<Vec<u8>>,
    /// PID of the spawned shell, kept for `/proc/<pid>` inspection (SSH detection, CWD…).
    /// The `Child` handle itself lives in a dedicated reaper thread that emits the exit
    /// notification once `wait()` completes (see `spawn`), so no zombie is ever left.
    child_pid: Option<u32>,
    screen: VtScreen,
    current_size: PtySize,
    shell: String,
    /// DECCKM (application cursor keys) state advertised by the child: full-screen
    /// apps like `vim` enable it via `smkx` (`ESC [ ? 1 h`) and then expect arrow
    /// keys as SS3 (`ESC O A`), not CSI (`ESC [ A`). Tracked on the OUTPUT stream
    /// by [`update_app_cursor_mode`], consulted by the key encoder.
    app_cursor_mode: Arc<AtomicBool>,
}

impl PtyProcess {
    pub fn spawn(
        rows: u16,
        cols: u16,
        output_tx: UnboundedSender<Vec<u8>>,
        exit_tx: UnboundedSender<()>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .context("Failed to open pseudo-terminal pair")?;

        // Determine default user shell
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("SYSTEMD_PAGER", "cat");
        cmd.env("PAGER", "cat");

        let temp_dir = std::env::temp_dir().join("spiritty");
        let _ = std::fs::create_dir_all(&temp_dir);

        // Automatically configure shell hooks for silent OSC completion notification
        if shell.contains("bash") {
            let rc_path = temp_dir.join("bash_init.sh");
            let rc_content = r#"
[ -f /etc/bash.bashrc ] && . /etc/bash.bashrc
[ -f ~/.bashrc ] && . ~/.bashrc
__spiritty_done() {
    local code=$?
    printf '\033]777;spiritty_done;%s\007' "${code:-0}"
}
PROMPT_COMMAND="__spiritty_done${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
"#;
            let _ = std::fs::write(&rc_path, rc_content);
            cmd.args([
                "--rcfile",
                rc_path.to_str().unwrap_or("/tmp/spiritty/bash_init.sh"),
            ]);
        } else if shell.contains("zsh") {
            let zsh_dir = temp_dir.join("zsh");
            let _ = std::fs::create_dir_all(&zsh_dir);
            let zsh_rc = zsh_dir.join(".zshrc");
            let zsh_content = r#"
[ -f ~/.zshrc ] && . ~/.zshrc
__spiritty_done() {
    local code=$?
    printf '\033]777;spiritty_done;%s\007' "${code:-0}"
}
precmd_functions+=(__spiritty_done)
"#;
            let _ = std::fs::write(&zsh_rc, zsh_content);
            cmd.env("ZDOTDIR", zsh_dir.to_str().unwrap_or("/tmp/spiritty/zsh"));
        } else if shell.contains("fish") {
            cmd.args(["-C", "function __spiritty_post --on-event fish_postexec; printf '\\033]777;spiritty_done;%s\\007' $status; end"]);
        }

        // Inherit current working directory
        if let Ok(cwd) = env::current_dir() {
            cmd.cwd(cwd);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process on PTY slave")?;

        // Writer for injecting keystrokes into master PTY
        let writer = pair
            .master
            .take_writer()
            .context("Failed to get master PTY writer")?;

        // Dedicated writer thread: a master write BLOCKS while the downstream stops
        // consuming (stalled SSH pipe, full remote tty input buffer, wedged remote
        // shell) — and every keystroke or agent tool injection used to be written from
        // the UI event loop, so one stalled write froze the entire application. Input
        // is now enqueued (never blocks the caller) and flushed by this thread; FIFO
        // order is preserved because there is a single consumer.
        let (input_tx, input_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut writer = writer;
            while let Ok(data) = input_rx.recv() {
                if writer.write_all(&data).is_err() || writer.flush().is_err() {
                    // Master closed: the reader thread owns exit notification; stop
                    // consuming and let the channel disconnect.
                    break;
                }
            }
        });

        // Reader for reading shell output from master PTY
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone master PTY reader")?;

        let screen = VtScreen::new(rows.max(1), cols.max(1));
        let screen_clone = screen.clone();
        let input_tx_reader = input_tx.clone();

        // Single-shot exit signaling shared between the reader (EOF side) and the reaper
        // (wait side): whichever observes the shell termination first notifies the app,
        // exactly once.
        let exited = Arc::new(AtomicBool::new(false));

        // DECCKM tracking shared between the reader thread (writer side of the flag)
        // and the key encoder on the UI thread (reader side).
        let app_cursor_mode = Arc::new(AtomicBool::new(false));

        // Dedicated reaper thread: reaps the child as soon as it terminates (no zombie)
        // and emits the PTY-exit notification. Without this, `exit` typed inside the
        // panel left Spiritty running on a dead PTY with an unreaped process behind it.
        let child_pid = child.process_id();
        {
            let exited_reaper = Arc::clone(&exited);
            let exit_tx_reaper = exit_tx.clone();
            thread::spawn(move || {
                let _status = child.wait();
                if !exited_reaper.swap(true, Ordering::SeqCst) {
                    let _ = exit_tx_reaper.send(());
                }
            });
        }

        // Spawn background thread for continuous PTY output reading
        {
            let exited_reader = Arc::clone(&exited);
            let exit_tx_reader = exit_tx;
            let app_cursor_reader = Arc::clone(&app_cursor_mode);
            thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            // EOF: slave side closed — the shell has terminated.
                            break;
                        }
                        Ok(n) => {
                            let data = buf[..n].to_vec();

                            // Respond immediately to terminal capability inquiries (DA1, DA2, DSR, CPR, OSC)
                            respond_to_terminal_queries(&data, &input_tx_reader, &screen_clone);

                            screen_clone.process(&data);
                            update_app_cursor_mode(&data, &app_cursor_reader);
                            if output_tx.send(data).is_err() {
                                // Receiver dropped, stop thread
                                break;
                            }
                        }
                        Err(_) => {
                            // Read error or PTY closed
                            break;
                        }
                    }
                }
                if !exited_reader.swap(true, Ordering::SeqCst) {
                    let _ = exit_tx_reader.send(());
                }
            });
        }

        Ok(Self {
            master: pair.master,
            input_tx,
            child_pid,
            screen,
            current_size: size,
            shell,
            app_cursor_mode,
        })
    }

    pub fn shell(&self) -> &str {
        &self.shell
    }

    /// Whether the child currently expects SS3 (application) cursor keys —
    /// see [`PtyProcess::app_cursor_mode`] field docs.
    pub fn app_cursor_mode(&self) -> bool {
        self.app_cursor_mode.load(Ordering::SeqCst)
    }

    pub fn shell_name(&self) -> &str {
        self.shell.rsplit('/').next().unwrap_or(&self.shell)
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    pub fn screen(&self) -> &VtScreen {
        &self.screen
    }

    pub fn scroll_up(&self, lines: usize) {
        self.screen.scroll_up(lines);
    }

    pub fn scroll_down(&self, lines: usize) {
        self.screen.scroll_down(lines);
    }

    pub fn reset_scroll(&self) {
        self.screen.reset_scroll();
    }

    pub fn scroll_offset(&self) -> usize {
        self.screen.scroll_offset()
    }

    pub fn scroll_info(&self) -> (usize, usize) {
        self.screen.scroll_info()
    }

    /// Enqueues bytes for injection into the PTY master and returns immediately. The
    /// actual write happens on the dedicated writer thread (see `spawn`): a master write
    /// blocks while the downstream stops consuming, and it must never block the UI
    /// thread. A `send` only fails if the writer thread is gone (PTY dead) — the reader
    /// thread then delivers `PtyExit` and the caller already treats the PTY as lost.
    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let _ = self.input_tx.send(data.to_vec());
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.max(1);
        let cols = cols.max(1);

        if self.current_size.rows == rows && self.current_size.cols == cols {
            return Ok(());
        }

        self.current_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        self.screen.resize(rows, cols);
        self.master
            .resize(self.current_size)
            .context("Failed to resize master PTY")?;

        Ok(())
    }
}

/// Automatically replies to ANSI/VT terminal capability inquiries from shells
/// like fish, zsh, starship, neovim, etc.
/// Scans a chunk of child OUTPUT for DECSET/DECRST of DECCKM — `ESC [ ? 1 h`
/// enables application cursor keys (what `vim` does via `smkx` on startup),
/// `ESC [ ? 1 l` disables it (`rmkx`). While enabled, arrows must be sent as
/// SS3 (`ESC O A`) for the application to recognize them.
///
/// Note: a sequence split exactly across two read chunks is missed until the
/// next one; acceptable in practice since `smkx` is emitted in a single burst
/// at startup, before any keystroke can matter.
fn update_app_cursor_mode(data: &[u8], state: &AtomicBool) {
    let mut i = 0usize;
    while i < data.len() {
        let Some(pos) = data[i..].iter().position(|&b| b == 0x1B) else {
            break;
        };
        let abs = i + pos;
        let rest = &data[abs..];
        if rest.len() >= 4 && rest[1] == b'[' && rest[2] == b'?' {
            let mut j = 3;
            while j < rest.len() && (rest[j].is_ascii_digit() || rest[j] == b';') {
                j += 1;
            }
            if j < rest.len() && matches!(rest[j], b'h' | b'l') && j > 3 {
                let params = std::str::from_utf8(&rest[3..j]).unwrap_or("");
                if params.split(';').any(|p| p == "1") {
                    state.store(rest[j] == b'h', Ordering::SeqCst);
                }
                i = abs + j + 1;
                continue;
            }
        }
        i = abs + 1;
    }
}

fn respond_to_terminal_queries(
    data: &[u8],
    input_tx: &std::sync::mpsc::Sender<Vec<u8>>,
    screen: &VtScreen,
) {
    // 1. Primary Device Attributes (DA1): ESC [ c or ESC [ 0 c
    if data.windows(3).any(|w| w == b"\x1b[c") || data.windows(4).any(|w| w == b"\x1b[0c") {
        // Reply: VT220 with 132 columns, printer, etc. (\x1b[?62;c)
        let _ = input_tx.send(b"\x1b[?62;1;2;6;7;8;9c".to_vec());
    }

    // 2. Secondary Device Attributes (DA2): ESC [ > c or ESC [ > 0 c
    if data.windows(4).any(|w| w == b"\x1b[>c") || data.windows(5).any(|w| w == b"\x1b[>0c") {
        // Reply: VT220, version 10, ROM 0
        let _ = input_tx.send(b"\x1b[>0;10;0c".to_vec());
    }

    // 3. Device Status Report (DSR): ESC [ 5 n
    if data.windows(4).any(|w| w == b"\x1b[5n") {
        // Reply: Terminal OK
        let _ = input_tx.send(b"\x1b[0n".to_vec());
    }

    // 4. Cursor Position Report (CPR): ESC [ 6 n
    if data.windows(4).any(|w| w == b"\x1b[6n") {
        let (col, row, _) = screen.cursor_position();
        let resp = format!("\x1b[{};{}R", row + 1, col + 1);
        let _ = input_tx.send(resp.into_bytes());
    }

    // 5. OSC 11 background color query (ESC ] 11 ; ? ...)
    if data.windows(6).any(|w| w == b"\x1b]11;?") {
        let _ = input_tx.send(b"\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\".to_vec());
    }

    // 6. OSC 10 foreground color query (ESC ] 10 ; ? ...)
    if data.windows(6).any(|w| w == b"\x1b]10;?") {
        let _ = input_tx.send(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\".to_vec());
    }
}

#[cfg(test)]
mod decckm_tests {
    use super::update_app_cursor_mode;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn state() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn smkx_enables_application_cursor() {
        let s = state();
        update_app_cursor_mode(b"\x1b[?1h\x1b=", &s);
        assert!(s.load(Ordering::SeqCst));
        update_app_cursor_mode(b"\x1b[?1l", &s);
        assert!(!s.load(Ordering::SeqCst));
    }

    #[test]
    fn combined_params_with_decckm_are_detected() {
        let s = state();
        update_app_cursor_mode(b"\x1b[?1000;1;1006h", &s);
        assert!(s.load(Ordering::SeqCst));
        update_app_cursor_mode(b"\x1b[?1006;1l", &s);
        assert!(!s.load(Ordering::SeqCst));
    }

    #[test]
    fn unrelated_escapes_do_not_toggle() {
        let s = state();
        update_app_cursor_mode(b"\x1b[?25l\x1b[2J\x1b[?1049h", &s);
        assert!(!s.load(Ordering::SeqCst));
        // Bare ESC / CSI without '?' must not panic or change state.
        update_app_cursor_mode(b"\x1b[A\x1b", &s);
        assert!(!s.load(Ordering::SeqCst));
    }
}
