//! User-session observer for Windows InputPane and UI Automation text focus.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc as std_mpsc, Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;

use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle as AsyncJoinHandle;
use tokio::sync::Notify;
use windows::core::{BOOL, GUID};
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextEditPattern,
    IUIAutomationTextPattern2, IUIAutomationValuePattern, UIA_EditControlTypeId,
    UIA_TextEditPatternId, UIA_TextPattern2Id, UIA_ValuePatternId,
};
use windows::Win32::UI::Shell::IFrameworkInputPane;

use crate::sunshine::{create_https_client, get_local_sunshine_url};

const FRAMEWORK_INPUT_PANE: GUID = GUID::from_u128(0xd5120aa3_46ba_44c5_822d_ca8092c1fc72);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const OBSERVATION_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const OBSERVER_STOP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct WireRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl From<RECT> for WireRect {
    fn from(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

impl WireRect {
    fn visible(self) -> bool {
        self.right > self.left && self.bottom > self.top
    }
}

#[derive(Clone, Debug, Serialize)]
struct Observation {
    version: u8,
    source: &'static str,
    active: bool,
    editable: bool,
    password: bool,
    multiline: bool,
    pane_visible: bool,
    auto_show: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    element_rect: Option<WireRect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    caret_rect: Option<WireRect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FocusSignature {
    control_type: i32,
    rect: WireRect,
    password: bool,
    editable: bool,
    caret_rect: Option<WireRect>,
}

#[derive(Default)]
struct PendingObservations {
    input_pane: Option<Observation>,
    uia: Option<Observation>,
}

#[derive(Default)]
struct ObservationQueue {
    pending: Mutex<PendingObservations>,
    notify: Notify,
}

impl ObservationQueue {
    fn publish(&self, observation: Observation) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match observation.source {
            "input_pane" => pending.input_pane = Some(observation),
            _ => pending.uia = Some(observation),
        }
        drop(pending);
        self.notify.notify_one();
    }

    fn take(&self) -> Vec<Observation> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        [pending.input_pane.take(), pending.uia.take()]
            .into_iter()
            .flatten()
            .collect()
    }

    fn requeue_if_empty(&self, observation: Observation) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let slot = if observation.source == "input_pane" {
            &mut pending.input_pane
        } else {
            &mut pending.uia
        };
        if slot.is_none() {
            *slot = Some(observation);
        }
        drop(pending);
        self.notify.notify_one();
    }
}

unsafe fn take_last_bounding_rect(array: *mut SAFEARRAY) -> Option<WireRect> {
    if array.is_null() {
        return None;
    }
    let result = (|| {
        let lower = unsafe { SafeArrayGetLBound(array, 1) }.ok()?;
        let upper = unsafe { SafeArrayGetUBound(array, 1) }.ok()?;
        let count = upper.checked_sub(lower)?.checked_add(1)? as usize;
        if count < 4 || count % 4 != 0 {
            return None;
        }

        let mut raw = std::ptr::null_mut();
        unsafe { SafeArrayAccessData(array, &mut raw) }.ok()?;
        let values = unsafe { std::slice::from_raw_parts(raw.cast::<f64>(), count) };
        let rect = values.chunks_exact(4).last().and_then(|values| {
            let (left, top, width, height) = (values[0], values[1], values[2], values[3]);
            if !left.is_finite() || !top.is_finite() || !width.is_finite() || !height.is_finite() {
                return None;
            }
            Some(WireRect {
                left: left.floor() as i32,
                top: top.floor() as i32,
                right: (left + width.max(1.0)).ceil() as i32,
                bottom: (top + height.max(1.0)).ceil() as i32,
            })
        });
        let _ = unsafe { SafeArrayUnaccessData(array) };
        rect.filter(|rect| rect.visible())
    })();
    let _ = unsafe { SafeArrayDestroy(array) };
    result
}

fn inspect_caret(element: &IUIAutomationElement) -> Option<WireRect> {
    let pattern =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) }
            .ok()?;
    let mut active = BOOL::default();
    let range = unsafe { pattern.GetCaretRange(&mut active) }.ok()?;
    if !active.as_bool() {
        return None;
    }
    let bounds = unsafe { range.GetBoundingRectangles() }.ok()?;
    unsafe { take_last_bounding_rect(bounds) }
}

struct State {
    stop: Arc<AtomicBool>,
    observer_thread: Option<JoinHandle<()>>,
    observer_done: std_mpsc::Receiver<()>,
    network_task: Option<AsyncJoinHandle<()>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn inspect_focused_element(automation: &IUIAutomation) -> Option<(FocusSignature, Observation)> {
    let element = unsafe { automation.GetFocusedElement() }.ok()?;
    let has_focus = unsafe { element.CurrentHasKeyboardFocus() }.ok()?.as_bool();
    let enabled = unsafe { element.CurrentIsEnabled() }.ok()?.as_bool();
    let offscreen = unsafe { element.CurrentIsOffscreen() }.ok()?.as_bool();
    if !has_focus || !enabled || offscreen {
        return None;
    }
    let rect = WireRect::from(unsafe { element.CurrentBoundingRectangle() }.ok()?);
    if !rect.visible() {
        return None;
    }
    let control_type = unsafe { element.CurrentControlType() }.ok()?.0;
    let password = unsafe { element.CurrentIsPassword() }
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let has_text_edit = unsafe {
        element.GetCurrentPatternAs::<IUIAutomationTextEditPattern>(UIA_TextEditPatternId)
    }
    .is_ok();
    let writable_value =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
            .ok()
            .and_then(|pattern| unsafe { pattern.CurrentIsReadOnly() }.ok())
            .map(|value| !value.as_bool())
            .unwrap_or(false);
    let editable = has_text_edit || (control_type == UIA_EditControlTypeId.0 && writable_value);
    let caret_rect = editable.then(|| inspect_caret(&element)).flatten();
    let signature = FocusSignature {
        control_type,
        rect,
        password,
        editable,
        caret_rect,
    };
    let observation = Observation {
        version: 1,
        source: "uia",
        active: true,
        editable,
        password,
        multiline: false,
        pane_visible: false,
        auto_show: false,
        element_rect: Some(rect),
        caret_rect,
    };
    Some((signature, observation))
}

fn observer_loop(
    stop: Arc<AtomicBool>,
    input_pane_available: Arc<AtomicBool>,
    uia_available: Arc<AtomicBool>,
    ready: Arc<Notify>,
    queue: Arc<ObservationQueue>,
) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        warn!("text context observer could not initialize COM");
        return;
    }

    let pane: Option<IFrameworkInputPane> =
        unsafe { CoCreateInstance(&FRAMEWORK_INPUT_PANE, None, CLSCTX_INPROC_SERVER).ok() };
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
    input_pane_available.store(pane.is_some(), Ordering::Release);
    uia_available.store(automation.is_some(), Ordering::Release);
    ready.notify_one();
    info!(
        "text context observer started (input_pane={}, uia={})",
        pane.is_some(),
        automation.is_some()
    );

    let mut previous_pane_visible = false;
    let mut previous_focus: Option<FocusSignature> = None;
    while !stop.load(Ordering::Acquire) {
        if let Some(pane) = &pane {
            if let Ok(rect) = unsafe { pane.Location() } {
                let rect = WireRect::from(rect);
                let visible = rect.visible();
                if visible && !previous_pane_visible {
                    queue.publish(Observation {
                        version: 1,
                        source: "input_pane",
                        active: true,
                        editable: true,
                        password: false,
                        multiline: false,
                        pane_visible: true,
                        auto_show: true,
                        element_rect: None,
                        caret_rect: None,
                    });
                }
                previous_pane_visible = visible;
            }
        }

        if let Some(automation) = &automation {
            let focused = inspect_focused_element(automation);
            match focused {
                Some((signature, observation)) if previous_focus != Some(signature) => {
                    // Publish non-editable focus too. The core caches UIA as
                    // level-triggered state, so explicit negative transitions
                    // prevent a previously focused editor from going stale.
                    queue.publish(observation);
                    previous_focus = Some(signature);
                }
                Some((signature, _)) => previous_focus = Some(signature),
                None if previous_focus.take().is_some() => {
                    queue.publish(Observation {
                        version: 1,
                        source: "uia",
                        active: false,
                        editable: false,
                        password: false,
                        multiline: false,
                        pane_visible: false,
                        auto_show: false,
                        element_rect: None,
                        caret_rect: None,
                    });
                }
                None => {}
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    input_pane_available.store(false, Ordering::Release);
    uia_available.store(false, Ordering::Release);
    drop(pane);
    drop(automation);
    unsafe { CoUninitialize() };
    info!("text context observer stopped");
}

async fn post_json(path: &str, body: &impl Serialize) -> Result<(), String> {
    let base = get_local_sunshine_url().await?;
    let client = create_https_client()?;
    let response = client
        .post(format!("{}{}", base.trim_end_matches('/'), path))
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", response.status()))
    }
}

async fn network_loop(
    stop: Arc<AtomicBool>,
    input_pane_available: Arc<AtomicBool>,
    uia_available: Arc<AtomicBool>,
    ready: Arc<Notify>,
    queue: Arc<ObservationQueue>,
) {
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let body = serde_json::json!({
                    "version": 1,
                    "input_pane": input_pane_available.load(Ordering::Acquire),
                    "uia": uia_available.load(Ordering::Acquire),
                });
                if let Err(error) = post_json("/api/v1/text-context/capability", &body).await {
                    debug!("text context heartbeat failed: {error}");
                }
            }
            _ = ready.notified() => {
                let body = serde_json::json!({
                    "version": 1,
                    "input_pane": input_pane_available.load(Ordering::Acquire),
                    "uia": uia_available.load(Ordering::Acquire),
                });
                if let Err(error) = post_json("/api/v1/text-context/capability", &body).await {
                    debug!("text context ready notification failed: {error}");
                }
            }
            _ = queue.notify.notified() => {
                for observation in queue.take() {
                    if let Err(error) = post_json("/api/v1/text-context/observation", &observation).await {
                        debug!("text context observation failed: {error}");
                        queue.requeue_if_empty(observation);
                        tokio::time::sleep(OBSERVATION_RETRY_INTERVAL).await;
                    }
                }
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
}

pub fn auto_start() {
    let mut state = STATE.lock().unwrap();
    if state.is_some() {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let input_pane_available = Arc::new(AtomicBool::new(false));
    let uia_available = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(Notify::new());
    let queue = Arc::new(ObservationQueue::default());
    let (observer_done_tx, observer_done) = std_mpsc::sync_channel(1);

    let observer_stop = stop.clone();
    let observer_pane = input_pane_available.clone();
    let observer_uia = uia_available.clone();
    let observer_ready = ready.clone();
    let observer_queue = queue.clone();
    let observer_thread = std::thread::Builder::new()
        .name("text-context-observer".into())
        .spawn(move || {
            observer_loop(
                observer_stop,
                observer_pane,
                observer_uia,
                observer_ready,
                observer_queue,
            );
            let _ = observer_done_tx.send(());
        })
        .ok();

    let task_stop = stop.clone();
    let task_pane = input_pane_available.clone();
    let task_uia = uia_available.clone();
    let task_ready = ready;
    let task_queue = queue;
    let network_task = tauri::async_runtime::spawn(async move {
        network_loop(task_stop, task_pane, task_uia, task_ready, task_queue).await
    });
    *state = Some(State {
        stop,
        observer_thread,
        observer_done,
        network_task: Some(network_task),
    });
}

pub fn stop() {
    let Some(mut state) = STATE.lock().unwrap().take() else {
        return;
    };
    state.stop.store(true, Ordering::Release);
    if let Some(thread) = state.observer_thread.take() {
        if state
            .observer_done
            .recv_timeout(OBSERVER_STOP_TIMEOUT)
            .is_ok()
        {
            let _ = thread.join();
        } else {
            warn!("text context observer did not stop within timeout; detaching thread");
        }
    }
    if let Some(task) = state.network_task.take() {
        task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(source: &'static str, active: bool) -> Observation {
        Observation {
            version: 1,
            source,
            active,
            editable: active,
            password: false,
            multiline: false,
            pane_visible: false,
            auto_show: false,
            element_rect: None,
            caret_rect: None,
        }
    }

    #[test]
    fn observation_queue_keeps_latest_state_per_source() {
        let queue = ObservationQueue::default();
        queue.publish(observation("uia", true));
        queue.publish(observation("uia", false));

        let pending = queue.take();
        assert_eq!(pending.len(), 1);
        assert!(!pending[0].active);
    }

    #[test]
    fn failed_send_does_not_replace_newer_state() {
        let queue = ObservationQueue::default();
        let failed = observation("uia", true);
        queue.publish(failed.clone());
        let _ = queue.take();
        queue.publish(observation("uia", false));
        queue.requeue_if_empty(failed);

        let pending = queue.take();
        assert_eq!(pending.len(), 1);
        assert!(!pending[0].active);
    }

    #[test]
    fn observation_queue_preserves_independent_sources() {
        let queue = ObservationQueue::default();
        queue.publish(observation("input_pane", true));
        queue.publish(observation("uia", false));

        let pending = queue.take();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].source, "input_pane");
        assert_eq!(pending[1].source, "uia");
    }
}
