use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const SHORTCUT: &str = "Ctrl+Alt+N";
static REGISTERED: AtomicBool = AtomicBool::new(false);

pub fn register<R: Runtime>(app: &AppHandle<R>) {
    if REGISTERED.load(Ordering::Acquire) || app.global_shortcut().is_registered(SHORTCUT) {
        return;
    }
    let result = app.global_shortcut().on_shortcut(SHORTCUT, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window("nr_overlay") {
                let _ = window.emit("nr-toggle", ());
            }
        }
    });
    match result {
        Ok(_) => REGISTERED.store(true, Ordering::Release),
        Err(error) => log::warn!("NR overlay shortcut unavailable: {error}"),
    }
}

pub fn unregister<R: Runtime>(app: &AppHandle<R>) {
    if REGISTERED.swap(false, Ordering::AcqRel) {
        let _ = app.global_shortcut().unregister(SHORTCUT);
    }
}

#[tauri::command]
pub fn nr_overlay_shortcut_status() -> bool {
    REGISTERED.load(Ordering::Acquire)
}
