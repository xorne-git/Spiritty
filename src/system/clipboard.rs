use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

use crate::app::PendingImage;

static CLIPBOARD_CHANNEL: OnceLock<Sender<String>> = OnceLock::new();
/// Guards against stacking multiple concurrent paste reads when the clipboard manager
/// hangs and the user hammers Ctrl+V: a second request while one is in flight is dropped.
static PASTE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
/// Same guard for image pastes (Ctrl+Shift+V).
static IMAGE_PASTE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

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

/// Fire-and-forget clipboard read: the (potentially blocking) read runs on a dedicated
/// background thread and the result is delivered through `on_result`, so a hung
/// wl-paste/xclip can never stall keydown handling on the UI thread. At most one read
/// is in flight at a time; extra requests while busy are silently dropped.
pub fn spawn_paste_request<F>(on_result: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    if PASTE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("spiritty-clipboard-read".to_string())
        .spawn(move || {
            let text = read_clipboard_text_blocking();
            PASTE_IN_FLIGHT.store(false, Ordering::Release);
            on_result(text);
        });
}

/// What a "smart" Ctrl+V read resolved to: an image (to attach) or plain text.
#[derive(Debug, Clone)]
pub enum ClipboardPayload {
    Text(String),
    Image(PendingImage),
}

/// True when a pasted TEXT payload looks like a file path / URI to an image. Used to route
/// a terminal-emulator Ctrl+Shift+V paste (which Ghostty converts to the image's text URI)
/// back to the image-attach flow instead of spilling the path into the shell / chat.
pub fn looks_like_image_path(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    // file:// URI (X/portal style) or a plain absolute/relative path with an image suffix.
    let path = t
        .strip_prefix("file://")
        .or_else(|| t.strip_prefix("file:"))
        .unwrap_or(t)
        .trim();
    let lower = path.trim_end_matches(['\r', '\n']).to_lowercase();
    const IMAGE_EXTS: &[&str] = &[
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".tif", ".tiff", ".svg", ".avif",
    ];
    IMAGE_EXTS.iter().any(|ext| lower.ends_with(ext))
}

/// Smart clipboard read: if the clipboards holds an image, attach it; otherwise paste the
/// text. This makes a single `Ctrl+V` work with the model's OWN clipboard read (arboard)
/// rather than relying on a terminal-emulator paste binding, which Ghostty/others capture
/// for Ctrl+Shift+V and void our per-app shortcut.
pub fn spawn_smart_paste_request<F>(on_result: F)
where
    F: FnOnce(Option<ClipboardPayload>) + Send + 'static,
{
    if PASTE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("spiritty-clipboard-read".to_string())
        .spawn(move || {
            let payload = read_clipboard_payload_blocking();
            PASTE_IN_FLIGHT.store(false, Ordering::Release);
            on_result(payload);
        });
}

/// Reads the clipboard once, preferring an image when present, then text. The image branch
/// runs first because Ghostty (and other emulators) may copy a screenshot — if arboard can
/// decode it as pixels we attach it; otherwise we treat the clipboard as text.
fn read_clipboard_payload_blocking() -> Option<ClipboardPayload> {
    if let Some(image) = read_clipboard_image_blocking() {
        return Some(ClipboardPayload::Image(image));
    }
    read_clipboard_text_blocking().map(ClipboardPayload::Text)
}

/// Bounded-timeout clipboard read kept for modal paste fields, where the result must
/// mutate modal state synchronously (no event channel available). The read still runs
/// on a background thread; the *caller's* thread waits at most `timeout`, so a hung
/// clipboard manager degrades to a missed paste instead of a frozen UI.
pub fn get_clipboard_text_timeout(timeout: std::time::Duration) -> Option<String> {
    let (tx, rx) = channel::<Option<String>>();
    let _ = std::thread::Builder::new()
        .name("spiritty-clipboard-read".to_string())
        .spawn(move || {
            let _ = tx.send(read_clipboard_text_blocking());
        });
    rx.recv_timeout(timeout).ok().flatten()
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

/// Reads an image from the clipboard as PNG (via arboard → RGBA → PNG encode → base64).
/// Returns `None` when the clipboard holds no image or the encoding fails. Runs on the
/// caller thread — always wrap it in a background thread (see `spawn_image_paste_request`).
fn read_clipboard_image_blocking() -> Option<PendingImage> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let image = cb.get_image().ok()?;
    if image.width == 0 || image.height == 0 || image.bytes.is_empty() {
        return None;
    }

    let rgba_len = image.width * image.height * 4;
    if image.bytes.len() < rgba_len {
        return None;
    }
    let rgba = image.bytes[..rgba_len].to_vec();
    PendingImage::from_rgba(rgba, image.width, image.height)
}

/// Encodes raw RGBA (row-major, 4 bytes/pixel) into a PNG byte buffer.
pub fn encode_rgba_to_png(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    // The PNG encoder asserts on a mismatched buffer length; reject up front so a
    // malformed clipboard payload degrades to a missed paste instead of a panic.
    let expected = width.checked_mul(height)?.checked_mul(4)?;
    if rgba.len() != expected || width == 0 || height == 0 {
        return None;
    }
    let mut buf = Vec::new();
    {
        use image::ImageEncoder as _;
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        let color = image::ExtendedColorType::Rgba8;
        if encoder
            .write_image(rgba, width as u32, height as u32, color)
            .is_err()
        {
            return None;
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

/// Fire-and-forget clipboard image read. The (potentially blocking) read runs on a
/// background thread and the result is delivered through `on_result`, so a hung
/// clipboard manager can never stall the UI event loop. At most one image read is in
/// flight at a time; extra requests while busy are silently dropped.
pub fn spawn_image_paste_request<F>(on_result: F)
where
    F: FnOnce(Option<PendingImage>) + Send + 'static,
{
    if IMAGE_PASTE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("spiritty-clipboard-image".to_string())
        .spawn(move || {
            let image = read_clipboard_image_blocking();
            IMAGE_PASTE_IN_FLIGHT.store(false, Ordering::Release);
            on_result(image);
        });
}

#[cfg(test)]
mod clipboard_test {
    use super::{base64_encode, encode_rgba_to_png};

    #[test]
    fn base64_encode_pads_and_maps_correctly() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encodes_small_rgba_to_valid_png() {
        // 2x2 opaque red RGBA pixels.
        let rgba: Vec<u8> = vec![
            255, 0, 0, 255, 255, 0, 0, 255, //
            255, 0, 0, 255, 255, 0, 0, 255,
        ];
        let png = encode_rgba_to_png(&rgba, 2, 2).expect("PNG encode must succeed");
        // PNG magic: 8-byte signature.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // base64 of the PNG is non-empty and valid base64 (starts with the PNG magic
        // bytes base64-encoded: iVBO).
        let b64 = base64_encode(&png);
        assert!(b64.starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn rejects_empty_or_misaligned_encoding() {
        assert!(encode_rgba_to_png(&[], 0, 0).is_none());
        // Wrong length vs declared dimensions.
        assert!(encode_rgba_to_png(&[1, 2, 3], 2, 2).is_none());
    }

    #[test]
    fn image_path_detection() {
        use super::looks_like_image_path;
        // Ghostty converts a Ctrl+Shift+V image paste into the image's URI/path as text.
        assert!(looks_like_image_path("file:///run/user/1000/doc/x.png\r\n"));
        assert!(looks_like_image_path("/home/x/capture.png"));
        assert!(looks_like_image_path("~/screenshot.JPEG"));
        assert!(looks_like_image_path("/tmp/pic.webp"));
        // Plain text / non-image paths must NOT be treated as an image paste.
        assert!(!looks_like_image_path("hello world"));
        assert!(!looks_like_image_path("/etc/hosts"));
        assert!(!looks_like_image_path(""));
        assert!(!looks_like_image_path("un fichier.config"));
    }
}
