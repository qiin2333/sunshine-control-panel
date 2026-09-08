//! One-transaction Windows clipboard write carrying text AND image flavors.
//!
//! clipboard-rs's `set_text`/`set_image` each open + EmptyClipboard before
//! writing (and its batched `set(Vec<ClipboardContent>)` delegates the image
//! branch back to `set_image`, clearing the batch), so a compound clipboard
//! cannot be written through it. This module opens the clipboard once,
//! empties it once, and sets all flavors with clipboard-win's no-clear raw
//! setter.

use clipboard_win::{formats, raw, Clipboard};
use std::sync::OnceLock;

const CF_DIB: u32 = 8;

/// Registered "PNG" format id (0 = registration failed and PNG bytes are
/// skipped; CF_DIB still carries the image).
fn png_format_id() -> u32 {
    static PNG: OnceLock<u32> = OnceLock::new();
    *PNG.get_or_init(|| unsafe {
        let name: Vec<u16> = "PNG\0".encode_utf16().collect();
        raw::register_raw_format(&name)
            .map(|f| f.get())
            .unwrap_or(0)
    })
}

/// Build a 32bpp bottom-up BGRA CF_DIB (BITMAPINFOHEADER + pixels).
fn build_dib(rgba: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = rgba.dimensions();
    let row_bytes = w as usize * 4;
    let mut out = Vec::with_capacity(40 + row_bytes * h as usize);

    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(w as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&(h as i32).to_le_bytes()); // biHeight (positive = bottom-up)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    out.extend_from_slice(&(row_bytes as u32 * h).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // biXPelPerMeter..biClrImportant

    for y in (0..h).rev() {
        for x in 0..w {
            let p = rgba.get_pixel(x, y).0;
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]); // RGBA -> BGRA
        }
    }
    out
}

/// Write CF_UNICODETEXT + CF_DIB + registered "PNG" in one clipboard
/// transaction. On partial failure the flavors already written stay on the
/// clipboard and the error is returned for the caller's fallback.
pub fn write_compound(text: &str, rgba: &image::RgbaImage, png: &[u8]) -> Result<(), String> {
    if text.is_empty() || png.is_empty() {
        return Err("empty compound payload".to_string());
    }

    let _clip = Clipboard::new_attempts(10).map_err(|e| format!("open clipboard: {e}"))?;
    raw::empty().map_err(|e| format!("empty clipboard: {e}"))?;

    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0);
    let mut text_bytes = Vec::with_capacity(utf16.len() * 2);
    for unit in &utf16 {
        text_bytes.extend_from_slice(&unit.to_le_bytes());
    }
    raw::set_without_clear(formats::CF_UNICODETEXT, &text_bytes)
        .map_err(|e| format!("set CF_UNICODETEXT: {e}"))?;

    let dib = build_dib(rgba);
    raw::set_without_clear(CF_DIB, &dib).map_err(|e| format!("set CF_DIB: {e}"))?;

    let png_id = png_format_id();
    if png_id != 0 {
        // Browsers and modern editors prefer the verbatim PNG bytes over
        // the DIB; mirrors clipboard-rs's set_image flavor pair.
        raw::set_without_clear(png_id, png).map_err(|e| format!("set PNG format: {e}"))?;
    }

    Ok(())
}
