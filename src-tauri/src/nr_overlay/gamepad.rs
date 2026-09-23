use crate::controller_input::{parse, read, Chord};
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
                        state.capture.is_some(),
                    )
                };
                if previous.as_ref() != Some(&context) {
                    pads = std::array::from_fn(|_| Chord::default());
                    previous = Some(context.clone());
                }
                let (nr, overlay, capturing) = &context;
                let active = !nr.is_empty() || !overlay.is_empty();
                let mut connected = 0;
                let mut fired = [false; 2];
                for (index, pad) in pads.iter_mut().enumerate() {
                    let sample = read(index as u32);
                    connected += usize::from(sample.is_some());
                    if active {
                        if let Some(mask) = pad.update(sample, Instant::now()) {
                            fired[0] |= parse(nr).ok() == Some(mask);
                            fired[1] |= parse(overlay).ok() == Some(mask);
                        }
                    }
                }
                if !*capturing {
                    for (index, triggered) in fired.into_iter().enumerate() {
                        if !triggered {
                            continue;
                        }
                        // Do not delay polling during a network request. Recheck
                        // settings in each task before dispatching the action.
                        let app = app.clone();
                        let expected = context.clone();
                        tauri::async_runtime::spawn(async move {
                            {
                                let state = super::STATE.lock().unwrap();
                                if state.capture.is_some()
                                    || state.settings.nr_gamepad != expected.0
                                    || state.settings.overlay_gamepad != expected.1
                                {
                                    return;
                                }
                            }
                            super::run_action(&app, index as u8 + 1).await;
                        });
                    }
                }
                tokio::select! {
                    _ = CHANGED.notified() => {},
                    _ = tokio::time::sleep(Duration::from_millis(if active && connected > 0 { 20 } else { 500 })) => {},
                }
            }
        });
    }
}
