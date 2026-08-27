use std::io::{self, Write};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

static CLIPBOARD_CHANNEL: OnceLock<Sender<String>> = OnceLock::new();

fn get_clipboard_sender() -> &'static Sender<String> {
    CLIPBOARD_CHANNEL.get_or_init(|| {
        let (tx, rx) = channel::<String>();
        let _ = std::thread::Builder::new()
            .name("spiritty-clipboard".to_string())
            .spawn(move || {
                let mut clipboard = match arboard::Clipboard::new() {
                    Ok(cb) => Some(cb),
                    Err(e) => {
                        tracing::warn!("Failed to initialize persistent clipboard: {}", e);
                        None
                    }
                };

                while let Ok(text) = rx.recv() {
                    if let Some(ref mut cb) = clipboard {
                        let _ = cb.set_text(text);
                    } else if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(text);
                        clipboard = Some(cb);
                    }
                }
            });
        tx
    })
}

pub fn base64_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Universal clipboard copy: uses persistent arboard for direct Wayland/X11 clipboard communication,
/// plus ANSI OSC 52 escape sequences.
pub fn copy_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }

    // 1. Send text to persistent background clipboard worker (keeps selection alive across the entire app session)
    let sender = get_clipboard_sender();
    let _ = sender.send(text.to_string());

    // 2. Universal OSC 52 sequence to stdout for terminal emulators that support it (Ghostty, Kitty, WezTerm)
    let encoded = base64_encode(text.as_bytes());
    let osc52 = format!("\x1b]52;c;{}\x1b\\", encoded);
    let _ = io::stdout().write_all(osc52.as_bytes());
    let _ = io::stdout().flush();
}

/// Universal clipboard paste reader: uses arboard and falls back to system CLI (wl-paste / xclip / pbpaste).
/// Runs the (potentially blocking) read on a background thread with a bounded timeout so a hung
/// clipboard manager cannot freeze the TUI indefinitely.
pub fn get_clipboard_text() -> Option<String> {
    let (tx, rx) = channel::<Option<String>>();
    let _ = std::thread::Builder::new()
        .name("spiritty-clipboard-read".to_string())
        .spawn(move || {
            let _ = tx.send(read_clipboard_text_blocking());
        });
    rx.recv_timeout(std::time::Duration::from_millis(1500))
        .ok()
        .flatten()
}

fn read_clipboard_text_blocking() -> Option<String> {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            if let Ok(output) = std::process::Command::new("wl-paste").arg("-n").output() {
                if output.status.success() {
                    if let Ok(text) = String::from_utf8(output.stdout) {
                        if !text.is_empty() {
                            return Some(text);
                        }
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("pbpaste").output() {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }

    None
}
