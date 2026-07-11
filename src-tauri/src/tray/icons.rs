use log::debug;
use std::{io::Cursor, path::PathBuf};
use tauri::{AppHandle, Runtime, image::Image};

use super::{CURRENT_CORE_ICON, TRAY_ID};
use crate::sunshine;

pub(super) fn load_initial_tray_icon<R: Runtime>(app: &AppHandle<R>) -> Image<'_> {
    load_tray_icon(&core_tray_icon_path("sunshine.ico"), "sunshine.ico").unwrap_or_else(|e| {
        debug!(
            "Failed to load initial C++ tray icon, using app icon: {}",
            e
        );
        app.default_window_icon().unwrap().clone()
    })
}

pub(super) fn apply_tray_icon<R: Runtime>(app: &AppHandle<R>, icon: &str) {
    let normalized_icon = if icon.is_empty() { "default" } else { icon };

    {
        let current_icon = CURRENT_CORE_ICON.lock().unwrap();
        if current_icon.as_deref() == Some(normalized_icon) {
            return;
        }
    }

    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let file_name = tray_icon_file_name(normalized_icon);
    let icon_path = core_tray_icon_path(file_name);

    match load_tray_icon(&icon_path, file_name) {
        Ok(image) => {
            if let Err(e) = tray.set_icon(Some(image)) {
                debug!("Failed to apply tray icon '{}': {}", file_name, e);
                return;
            }
            *CURRENT_CORE_ICON.lock().unwrap() = Some(normalized_icon.to_string());
        }
        Err(e) => debug!("Failed to load tray icon '{}': {}", file_name, e),
    }
}

fn tray_icon_file_name(icon: &str) -> &'static str {
    match icon {
        "playing" => "sunshine-playing.ico",
        "pausing" => "sunshine-pausing.ico",
        "locked" => "sunshine-locked.ico",
        _ => "sunshine.ico",
    }
}

fn core_tray_icon_path(file_name: &str) -> PathBuf {
    sunshine::assets_dir()
        .join("web")
        .join("images")
        .join(file_name)
}

fn bundled_tray_icon_bytes(file_name: &str) -> &'static [u8] {
    match file_name {
        "sunshine-playing.ico" => include_bytes!("../../icons/tray/sunshine-playing.ico"),
        "sunshine-pausing.ico" => include_bytes!("../../icons/tray/sunshine-pausing.ico"),
        "sunshine-locked.ico" => include_bytes!("../../icons/tray/sunshine-locked.ico"),
        _ => include_bytes!("../../icons/tray/sunshine.ico"),
    }
}

fn load_tray_icon(path: &PathBuf, file_name: &str) -> Result<Image<'static>, String> {
    if path.exists() {
        match load_tray_icon_from_path(path) {
            Ok(image) => return Ok(image),
            Err(e) => debug!("Failed to load core tray icon '{}': {}", path.display(), e),
        }
    }
    load_tray_icon_from_bytes(bundled_tray_icon_bytes(file_name))
}

fn load_tray_icon_from_path(path: &PathBuf) -> Result<Image<'static>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    load_tray_icon_from_ico_bytes(&bytes).or_else(|e| {
        debug!(
            "Failed to select small ICO frame from '{}': {}",
            path.display(),
            e
        );
        let image = image::ImageReader::open(path)
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok(rgba_to_tauri_image(image))
    })
}

fn load_tray_icon_from_bytes(bytes: &'static [u8]) -> Result<Image<'static>, String> {
    load_tray_icon_from_ico_bytes(bytes).or_else(|e| {
        debug!("Failed to select small bundled ICO frame: {}", e);
        let image = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok(rgba_to_tauri_image(image))
    })
}

fn load_tray_icon_from_ico_bytes(bytes: &[u8]) -> Result<Image<'static>, String> {
    let selected_ico = select_small_ico_frame(bytes)?;
    let image = image::ImageReader::with_format(Cursor::new(selected_ico), image::ImageFormat::Ico)
        .decode()
        .map_err(|e| e.to_string())?
        .to_rgba8();
    Ok(rgba_to_tauri_image(image))
}

fn select_small_ico_frame(bytes: &[u8]) -> Result<Vec<u8>, String> {
    const ICO_HEADER_LEN: usize = 6;
    const ICO_ENTRY_LEN: usize = 16;
    const PREFERRED_SIZES: [u16; 4] = [32, 24, 16, 48];

    if bytes.len() < ICO_HEADER_LEN {
        return Err("ICO header is too short".to_string());
    }
    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let icon_type = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if reserved != 0 || icon_type != 1 || count == 0 {
        return Err("not an ICO image".to_string());
    }
    let directory_len = ICO_HEADER_LEN + count * ICO_ENTRY_LEN;
    if bytes.len() < directory_len {
        return Err("ICO directory is truncated".to_string());
    }

    let mut selected: Option<(usize, usize)> = None;
    for index in 0..count {
        let entry_offset = ICO_HEADER_LEN + index * ICO_ENTRY_LEN;
        let width = ico_dimension(bytes[entry_offset]);
        let height = ico_dimension(bytes[entry_offset + 1]);
        let entry_size = u32::from_le_bytes([
            bytes[entry_offset + 8],
            bytes[entry_offset + 9],
            bytes[entry_offset + 10],
            bytes[entry_offset + 11],
        ]) as usize;
        let image_offset = u32::from_le_bytes([
            bytes[entry_offset + 12],
            bytes[entry_offset + 13],
            bytes[entry_offset + 14],
            bytes[entry_offset + 15],
        ]) as usize;
        if image_offset
            .checked_add(entry_size)
            .is_none_or(|end| end > bytes.len())
        {
            continue;
        }
        let Some(rank) = PREFERRED_SIZES
            .iter()
            .position(|preferred| width == *preferred && height == *preferred)
        else {
            continue;
        };
        if selected
            .map(|(_, old_rank)| rank < old_rank)
            .unwrap_or(true)
        {
            selected = Some((index, rank));
        }
    }

    let (index, _) = selected.ok_or_else(|| "no small tray ICO frame found".to_string())?;
    let entry_offset = ICO_HEADER_LEN + index * ICO_ENTRY_LEN;
    let entry_size = u32::from_le_bytes([
        bytes[entry_offset + 8],
        bytes[entry_offset + 9],
        bytes[entry_offset + 10],
        bytes[entry_offset + 11],
    ]) as usize;
    let image_offset = u32::from_le_bytes([
        bytes[entry_offset + 12],
        bytes[entry_offset + 13],
        bytes[entry_offset + 14],
        bytes[entry_offset + 15],
    ]) as usize;

    let mut selected_ico = Vec::with_capacity(ICO_HEADER_LEN + ICO_ENTRY_LEN + entry_size);
    selected_ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    selected_ico.extend_from_slice(&bytes[entry_offset..entry_offset + ICO_ENTRY_LEN]);
    selected_ico[ICO_HEADER_LEN + 12..ICO_HEADER_LEN + 16]
        .copy_from_slice(&((ICO_HEADER_LEN + ICO_ENTRY_LEN) as u32).to_le_bytes());
    selected_ico.extend_from_slice(&bytes[image_offset..image_offset + entry_size]);
    Ok(selected_ico)
}

fn ico_dimension(value: u8) -> u16 {
    if value == 0 { 256 } else { value as u16 }
}

fn rgba_to_tauri_image(image: image::RgbaImage) -> Image<'static> {
    let (width, height) = image.dimensions();
    Image::new_owned(image.into_raw(), width, height)
}
