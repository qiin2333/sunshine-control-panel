use crate::controller_input::{label, parse, read, Chord};
use std::time::{Duration, Instant};
fn binding_error(error: String) -> String {
    if error == "controller_shortcut_duplicate" {
        "nr_shortcut_duplicate".into()
    } else {
        "nr_gamepad_invalid".into()
    }
}
pub(super) fn normalize(value: &str) -> Result<String, String> {
    crate::controller_input::normalize(value).map_err(binding_error)
}
pub(super) fn validate(nr: &str, overlay: &str) -> Result<(), String> {
    crate::controller_input::validate(nr, overlay).map_err(binding_error)
}
static CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();
pub(super) fn wake() {
    CHANGED.notify_one();
}

pub(super) fn start<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(not(target_os = "windows"))]
    let _ = app;
    #[cfg(target_os = "windows")]
    {
        use tauri::{Emitter, Manager};
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut pads: [Chord; 4] = std::array::from_fn(|_| Chord::default());
            let mut previous = None;
            loop {
                let context = {
                    let state = super::STATE.lock().unwrap();
                    (
                        state.settings.nr_gamepad.clone(),
                        state.settings.overlay_gamepad.clone(),
                        state.capture.clone(),
                        state.capture_generation,
                    )
                };
                if previous.as_ref() != Some(&context) {
                    pads = std::array::from_fn(|_| Chord::default());
                    previous = Some(context.clone());
                }
                let (nr, overlay, capture, generation) = &context;
                let active = capture.is_some() || !nr.is_empty() || !overlay.is_empty();
                let mut connected = false;
                let mut fired = None;
                if active {
                    for (index, pad) in pads.iter_mut().enumerate() {
                        let sample = read(index as u32);
                        connected |= sample.is_some();
                        let chord = pad.update(sample, Instant::now());
                        if fired.is_none() {
                            fired = chord;
                        }
                    }
                }
                if let Some(mask) = fired {
                    if let Some((owner, _)) = capture {
                        // Never send recordings to another manager window.
                        if let Some(window) = app.get_webview_window(owner) {
                            if window.is_focused().unwrap_or(false) {
                                let _ = window.emit("nr-gamepad-captured", serde_json::json!({"shortcut": label(mask), "generation": generation}));
                            }
                        }
                    } else {
                        let action = if parse(nr).ok() == Some(mask) {
                            1
                        } else if parse(overlay).ok() == Some(mask) {
                            2
                        } else {
                            0
                        };
                        // Do not delay polling during a network request. Recheck the
                        // lease/settings in the task before dispatching the action.
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            {
                                let state = super::STATE.lock().unwrap();
                                if state.capture.is_some()
                                    || state.settings.nr_gamepad != context.0
                                    || state.settings.overlay_gamepad != context.1
                                {
                                    return;
                                }
                            }
                            super::run_action(&app, action).await;
                        });
                    }
                }
                tokio::select! {
                    _ = CHANGED.notified() => {},
                    _ = tokio::time::sleep(Duration::from_millis(if active && connected { 20 } else { 500 })) => {},
                }
            }
        });
    }
}
