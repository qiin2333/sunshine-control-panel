use log::error;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};

#[derive(Default)]
struct State {
    ready: bool,
    pending: Vec<(String, serde_json::Value)>,
}

pub(super) struct Bridge {
    state: Mutex<State>,
}

impl Bridge {
    pub(super) const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                ready: false,
                pending: Vec::new(),
            }),
        }
    }

    pub(super) fn mark_loading(&self) {
        self.state.lock().unwrap().ready = false;
    }

    pub(super) fn emit_or_queue<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        event: &str,
        payload: serde_json::Value,
    ) {
        let emit_now = {
            let mut state = self.state.lock().unwrap();
            if state.ready {
                true
            } else {
                if let Some(pending) = state
                    .pending
                    .iter_mut()
                    .find(|(pending_event, _)| pending_event == event)
                {
                    pending.1 = payload.clone();
                } else {
                    state.pending.push((event.to_string(), payload.clone()));
                }
                false
            }
        };

        if emit_now
            && let Some(window) = app.get_webview_window("main")
            && let Err(e) = window.emit(event, payload)
        {
            error!("Failed to emit main panel event '{}': {}", event, e);
        }
    }

    pub(super) fn mark_ready<R: Runtime>(&self, app: &AppHandle<R>) {
        let pending = {
            let mut state = self.state.lock().unwrap();
            state.ready = true;
            std::mem::take(&mut state.pending)
        };

        if let Some(window) = app.get_webview_window("main") {
            log::debug!(
                "Main panel ready; flushing {} queued event(s)",
                pending.len()
            );
            for (event, payload) in pending {
                if let Err(e) = window.emit(&event, payload) {
                    error!("Failed to flush main panel event '{}': {}", event, e);
                }
            }
        }
    }
}
