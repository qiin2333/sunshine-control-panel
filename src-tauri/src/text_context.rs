//! User-session observer for Windows InputPane and UI Automation text focus.

use std::cell::Cell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc as std_mpsc, Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle as AsyncJoinHandle;
use tokio::sync::Notify;
use windows::core::{BOOL, GUID};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
    IUIAutomationTextEditPattern, IUIAutomationTextPattern2, IUIAutomationValuePattern,
    SetWinEventHook, UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId,
    UIA_EditControlTypeId, UIA_HasKeyboardFocusPropertyId, UIA_IsEnabledPropertyId,
    UIA_IsOffscreenPropertyId, UIA_IsPasswordPropertyId, UIA_TextEditPatternId, UIA_TextPattern2Id,
    UIA_ValueIsReadOnlyPropertyId, UIA_ValuePatternId, UnhookWinEvent, HWINEVENTHOOK,
};
use windows::Win32::UI::Shell::IFrameworkInputPane;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    MsgWaitForMultipleObjectsEx, PeekMessageW, TranslateMessage, EVENT_OBJECT_FOCUS,
    EVENT_SYSTEM_FOREGROUND, GUITHREADINFO, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT,
    WINEVENT_OUTOFCONTEXT,
};

use crate::sunshine::{create_https_client, get_local_sunshine_url};

const FRAMEWORK_INPUT_PANE: GUID = GUID::from_u128(0xd5120aa3_46ba_44c5_822d_ca8092c1fc72);
// InputPane state is read in-process, so its poll can stay slow.
const PANE_POLL_INTERVAL: Duration = Duration::from_millis(500);
// While an editable element holds focus we refresh the caret geometry at the
// same cadence v1 used for everything, so IME avoidance stays responsive.
const EDITOR_CARET_POLL: Duration = Duration::from_millis(100);
// Apps with broken UIA event support never raise a focus event; a slow
// watchdog snapshot keeps the cached state from going stale. This mirrors the
// documented Narrator failure mode ("if no event is being raised, Narrator is
// not being made aware of the change in keyboard focus").
const FOCUS_WATCHDOG_POLL: Duration = Duration::from_secs(2);
// Focus-event bursts (menu traversal, Alt+Tab) collapse into one snapshot.
const RESCAN_DEBOUNCE: Duration = Duration::from_millis(40);
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
        unsafe { element.GetCachedPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) }.ok();
    if let Some(pattern) = pattern {
        let mut active = BOOL::default();
        if let Ok(range) = unsafe { pattern.GetCaretRange(&mut active) } {
            if !active.as_bool() {
                return None;
            }
            if let Ok(bounds) = unsafe { range.GetBoundingRectangles() } {
                if let Some(rect) = unsafe { take_last_bounding_rect(bounds) } {
                    return Some(rect);
                }
            }
        }
    }
    caret_from_win32()
}

/// Win32 caret fallback: editors with self-drawn UI that skip UIA text
/// patterns often still maintain the ::SetCaretPos caret, which real IMEs read
/// the same way when richer sources degenerate. The caret rect is in the
/// client coordinates of hwndCaret.
fn caret_from_win32() -> Option<WireRect> {
    unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        if thread == 0 {
            return None;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(thread, &mut info).is_err() || info.hwndCaret.is_invalid() {
            return None;
        }
        let mut top_left = POINT {
            x: info.rcCaret.left,
            y: info.rcCaret.top,
        };
        let mut bottom_right = POINT {
            x: info.rcCaret.right,
            y: info.rcCaret.bottom,
        };
        if !ClientToScreen(info.hwndCaret, &mut top_left).as_bool()
            || !ClientToScreen(info.hwndCaret, &mut bottom_right).as_bool()
        {
            return None;
        }
        let rect = WireRect {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x.max(top_left.x + 1),
            bottom: bottom_right.y.max(top_left.y + 1),
        };
        rect.visible().then_some(rect)
    }
}

struct State {
    stop: Arc<AtomicBool>,
    observer_thread: Option<JoinHandle<()>>,
    observer_done: std_mpsc::Receiver<()>,
    network_task: Option<AsyncJoinHandle<()>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn inspect_focused_element(
    automation: &IUIAutomation,
    cache: &IUIAutomationCacheRequest,
) -> Option<(FocusSignature, Observation)> {
    // BuildCache fetches every property and pattern in one cross-process
    // round trip instead of one per Current* call.
    let element = unsafe { automation.GetFocusedElementBuildCache(cache) }.ok()?;
    let has_focus = unsafe { element.CachedHasKeyboardFocus() }.ok()?.as_bool();
    let enabled = unsafe { element.CachedIsEnabled() }.ok()?.as_bool();
    let offscreen = unsafe { element.CachedIsOffscreen() }.ok()?.as_bool();
    if !has_focus || !enabled || offscreen {
        return None;
    }
    let rect = WireRect::from(unsafe { element.CachedBoundingRectangle() }.ok()?);
    if !rect.visible() {
        return None;
    }
    let control_type = unsafe { element.CachedControlType() }.ok()?.0;
    let password = unsafe { element.CachedIsPassword() }
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let has_text_edit = unsafe {
        element.GetCachedPatternAs::<IUIAutomationTextEditPattern>(UIA_TextEditPatternId)
    }
    .is_ok();
    let writable_value =
        unsafe { element.GetCachedPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
            .ok()
            .and_then(|pattern| unsafe { pattern.CachedIsReadOnly() }.ok())
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

thread_local! {
    // Set from the WinEvent callback (which fires on this thread while
    // messages are pumped); the observer loop coalesces bursts into at most
    // one snapshot per debounce window.
    static RESCAN_AT: Cell<Option<Instant>> = Cell::new(None);
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // EVENT_OBJECT_FOCUS arrives with varying idObject values (OBJID_CLIENT
    // for child controls, OBJID_WINDOW for top-level), so do not filter on
    // it — the debounce in the observer loop absorbs the extra triggers.
    if hwnd.0.is_null() {
        return;
    }
    RESCAN_AT.with(|slot| {
        if slot.get().is_none() {
            slot.set(Some(Instant::now()));
        }
    });
}

fn pump_messages() {
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
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

    let cache = automation.as_ref().and_then(|automation| {
        let request = unsafe { automation.CreateCacheRequest() }.ok()?;
        unsafe {
            let _ = request.AddProperty(UIA_HasKeyboardFocusPropertyId);
            let _ = request.AddProperty(UIA_IsEnabledPropertyId);
            let _ = request.AddProperty(UIA_IsOffscreenPropertyId);
            let _ = request.AddProperty(UIA_BoundingRectanglePropertyId);
            let _ = request.AddProperty(UIA_ControlTypePropertyId);
            let _ = request.AddProperty(UIA_IsPasswordPropertyId);
            let _ = request.AddProperty(UIA_ValueIsReadOnlyPropertyId);
            let _ = request.AddPattern(UIA_TextEditPatternId);
            let _ = request.AddPattern(UIA_ValuePatternId);
            let _ = request.AddPattern(UIA_TextPattern2Id);
        }
        Some(request)
    });

    // Focus changes push us; the remaining polling is only a watchdog for
    // apps that raise no events, plus the caret cadence while an editor is
    // focused. Idle desktops and full-screen games cost zero cross-process
    // UIA traffic.
    let hook_focus = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_FOCUS,
            EVENT_OBJECT_FOCUS,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    let hook_foreground = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    let mut previous_pane_visible = false;
    let mut previous_focus: Option<FocusSignature> = None;
    let mut editor_focused = false;
    // Prime both deadlines so the first iteration establishes a baseline.
    let mut last_snapshot = Instant::now() - FOCUS_WATCHDOG_POLL;
    let mut last_pane_check = Instant::now() - PANE_POLL_INTERVAL;

    while !stop.load(Ordering::Acquire) {
        pump_messages();
        let now = Instant::now();

        if let Some(pane) = &pane {
            if now.duration_since(last_pane_check) >= PANE_POLL_INTERVAL {
                last_pane_check = now;
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
        }

        let rescan_due = RESCAN_AT.with(|slot| match slot.get() {
            Some(at) if now.duration_since(at) >= RESCAN_DEBOUNCE => {
                slot.set(None);
                true
            }
            _ => false,
        });
        let need_snapshot = rescan_due
            || now.duration_since(last_snapshot) >= FOCUS_WATCHDOG_POLL
            || (editor_focused && now.duration_since(last_snapshot) >= EDITOR_CARET_POLL);

        if need_snapshot {
            // Advance unconditionally: with UIA unavailable the watchdog must
            // not re-fire every iteration, or the loop degenerates to the
            // clamped 5ms minimum wait.
            last_snapshot = now;
            if let (Some(automation), Some(cache)) = (automation.as_ref(), cache.as_ref()) {
                match inspect_focused_element(automation, cache) {
                    Some((signature, observation)) => {
                        editor_focused = signature.editable;
                        if previous_focus != Some(signature) {
                            // Publish non-editable focus too. The core caches UIA as
                            // level-triggered state, so explicit negative transitions
                            // prevent a previously focused editor from going stale.
                            queue.publish(observation);
                            previous_focus = Some(signature);
                        }
                    }
                    None if previous_focus.take().is_some() => {
                        editor_focused = false;
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
                    None => editor_focused = false,
                }
            }
        }

        // Block until the nearest deadline while still receiving WinEvents.
        // The pending debounce deadline must be included, or a freshly
        // scheduled rescan sleeps behind the 500ms clamp and delays the
        // focus response by that much.
        let mut wait = FOCUS_WATCHDOG_POLL.saturating_sub(now.duration_since(last_snapshot));
        if editor_focused {
            wait = wait.min(EDITOR_CARET_POLL.saturating_sub(now.duration_since(last_snapshot)));
        }
        wait = wait.min(PANE_POLL_INTERVAL.saturating_sub(now.duration_since(last_pane_check)));
        if let Some(at) = RESCAN_AT.with(|slot| slot.get()) {
            wait = wait.min(RESCAN_DEBOUNCE.saturating_sub(now.duration_since(at)));
        }
        let wait = wait.clamp(Duration::from_millis(5), Duration::from_millis(500));
        unsafe {
            MsgWaitForMultipleObjectsEx(
                None,
                wait.as_millis() as u32,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
        }
    }

    unsafe {
        if !hook_focus.is_invalid() {
            let _ = UnhookWinEvent(hook_focus);
        }
        if !hook_foreground.is_invalid() {
            let _ = UnhookWinEvent(hook_foreground);
        }
    }
    input_pane_available.store(false, Ordering::Release);
    uia_available.store(false, Ordering::Release);
    drop(cache);
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
