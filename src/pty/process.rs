use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    env,
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread,
};
use tokio::sync::mpsc::UnboundedSender;

use super::vt::VtScreen;

pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    #[allow(dead_code)]
    child: Box<dyn Child + Send + Sync>,
    screen: VtScreen,
    current_size: PtySize,
    shell: String,
}

impl PtyProcess {
    pub fn spawn(
        rows: u16,
        cols: u16,
        output_tx: UnboundedSender<Vec<u8>>,
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

        // Automatically configure shell hooks for silent OSC completion notification
        if shell.contains("fish") {
            cmd.args(["-C", "function __spiritty_post --on-event fish_postexec; printf '\\e]777;spiritty_done;%s\\a' $status; end"]);
        } else if shell.contains("bash") {
            cmd.env("PROMPT_COMMAND", "printf '\\e]777;spiritty_done;%s\\a' $?; ${PROMPT_COMMAND:-}");
        }

        // Inherit current working directory
        if let Ok(cwd) = env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process on PTY slave")?;

        // Writer for injecting keystrokes into master PTY
        let writer = pair
            .master
            .take_writer()
            .context("Failed to get master PTY writer")?;
        let writer = Arc::new(Mutex::new(writer));

        // Reader for reading shell output from master PTY
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone master PTY reader")?;

        let screen = VtScreen::new(rows.max(1), cols.max(1));
        let screen_clone = screen.clone();
        let writer_clone = Arc::clone(&writer);

        // Spawn background thread for continuous PTY output reading
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();

                        // Respond immediately to terminal capability inquiries (DA1, DA2, DSR, CPR, OSC)
                        respond_to_terminal_queries(&data, &writer_clone, &screen_clone);

                        screen_clone.process(&data);
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
        });

        Ok(Self {
            master: pair.master,
            writer,
            child,
            screen,
            current_size: size,
            shell,
        })
    }

    pub fn shell(&self) -> &str {
        &self.shell
    }

    pub fn shell_name(&self) -> &str {
        self.shell.rsplit('/').next().unwrap_or(&self.shell)
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

    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        if let Ok(mut writer) = self.writer.lock() {
            writer.write_all(data)?;
            writer.flush()?;
        }
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
fn respond_to_terminal_queries(
    data: &[u8],
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    screen: &VtScreen,
) {
    // 1. Primary Device Attributes (DA1): ESC [ c or ESC [ 0 c
    if data.windows(3).any(|w| w == b"\x1b[c") || data.windows(4).any(|w| w == b"\x1b[0c") {
        if let Ok(mut w) = writer.lock() {
            // Reply: VT220 with 132 columns, printer, etc. (\x1b[?62;c)
            let _ = w.write_all(b"\x1b[?62;1;2;6;7;8;9c");
            let _ = w.flush();
        }
    }

    // 2. Secondary Device Attributes (DA2): ESC [ > c or ESC [ > 0 c
    if data.windows(4).any(|w| w == b"\x1b[>c") || data.windows(5).any(|w| w == b"\x1b[>0c") {
        if let Ok(mut w) = writer.lock() {
            // Reply: VT220, version 10, ROM 0
            let _ = w.write_all(b"\x1b[>0;10;0c");
            let _ = w.flush();
        }
    }

    // 3. Device Status Report (DSR): ESC [ 5 n
    if data.windows(4).any(|w| w == b"\x1b[5n") {
        if let Ok(mut w) = writer.lock() {
            // Reply: Terminal OK
            let _ = w.write_all(b"\x1b[0n");
            let _ = w.flush();
        }
    }

    // 4. Cursor Position Report (CPR): ESC [ 6 n
    if data.windows(4).any(|w| w == b"\x1b[6n") {
        let (col, row, _) = screen.cursor_position();
        let resp = format!("\x1b[{};{}R", row + 1, col + 1);
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(resp.as_bytes());
            let _ = w.flush();
        }
    }

    // 5. OSC 11 background color query (ESC ] 11 ; ? ...)
    if data.windows(6).any(|w| w == b"\x1b]11;?") {
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(b"\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\");
            let _ = w.flush();
        }
    }

    // 6. OSC 10 foreground color query (ESC ] 10 ; ? ...)
    if data.windows(6).any(|w| w == b"\x1b]10;?") {
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\");
            let _ = w.flush();
        }
    }
}
