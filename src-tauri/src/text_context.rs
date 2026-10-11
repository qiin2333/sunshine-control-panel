//! User-session observer for Windows InputPane and UI Automation text focus.

use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc as std_mpsc,
};
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle as AsyncJoinHandle;
use tokio::sync::Notify;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, HWINEVENTHOOK, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
    IUIAutomationTextPattern2, IUIAutomationValuePattern, SetWinEventHook,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_EditControlTypeId,
    UIA_HasKeyboardFocusPropertyId, UIA_IsEnabledPropertyId, UIA_IsOffscreenPropertyId,
    UIA_IsPasswordPropertyId, UIA_TextPattern2Id, UIA_ValueIsReadOnlyPropertyId,
    UIA_ValuePatternId, UnhookWinEvent,
};
use windows::Win32::UI::Shell::IFrameworkInputPane;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND, GUITHREADINFO,
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, MSG, MWMO_INPUTAVAILABLE,
    MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
    WINEVENT_OUTOFCONTEXT,
};
use windows::core::{BOOL, GUID};

use crate::sunshine::{create_https_client, get_local_sunshine_url};

const FRAMEWORK_INPUT_PANE: GUID = GUID::from_u128(0xd5120aa3_46ba_44c5_822d_ca8092c1fc72);
// InputPane is sampled only while a confirmed editor is focused. It is not a
// desktop-wide heartbeat.
const PANE_POLL_INTERVAL: Duration = Duration::from_millis(250);
// Caret geometry is useful only for an active editor and remains scoped to
// that editor, so games and ordinary desktop focus do not pay this cost.
const EDITOR_CARET_POLL: Duration = Duration::from_millis(200);
// UIA focus/foreground events are the normal trigger. The watchdog is only a
// compatibility fallback while an editor is known to hold focus.
const FOCUS_WATCHDOG_POLL: Duration = Duration::from_secs(5);
// Focus-event bursts (menu traversal, Alt+Tab) collapse into one snapshot.
const RESCAN_DEBOUNCE: Duration = Duration::from_millis(40);
// The core expires GUI presence after 30 seconds. This task is deliberately
// much cheaper than the observer and remains alive so the first RTSP
// capability negotiation can see a supported GUI without starting UIA.
const PRESENCE_INTERVAL: Duration = Duration::from_secs(20);
const OBSERVATION_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const OBSERVER_STOP_TIMEOUT: Duration = Duration::from_secs(2);

fn agent_id() -> &'static str {
    static AGENT_ID: OnceLock<String> = OnceLock::new();
    AGENT_ID
        .get_or_init(|| uuid::Uuid::new_v4().to_string())
        .as_str()
}

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
    agent_id: &'static str,
    generation: u64,
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

struct CapabilityState {
    agent_present: AtomicBool,
    observer_active: AtomicBool,
    input_pane_available: AtomicBool,
    uia_available: AtomicBool,
    generation: AtomicU64,
    wake: Notify,
}

struct PresenceState {
    stop: Arc<AtomicBool>,
    capability: Arc<CapabilityState>,
    task: AsyncJoinHandle<()>,
}

struct ObserverState {
    generation: u64,
    stop: Arc<AtomicBool>,
    observer_thread: Option<JoinHandle<()>>,
    observer_done: std_mpsc::Receiver<()>,
    network_task: Option<AsyncJoinHandle<()>>,
}

static PRESENCE: Mutex<Option<PresenceState>> = Mutex::new(None);
static OBSERVER: Mutex<Option<ObserverState>> = Mutex::new(None);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static ENABLED: AtomicBool = AtomicBool::new(true);

fn owns_generation(capability: &CapabilityState, generation: u64) -> bool {
    capability.generation.load(Ordering::Acquire) == generation
}

fn clear_capability_if_owned(capability: &CapabilityState, generation: u64) {
    if !owns_generation(capability, generation) {
        return;
    }
    capability
        .input_pane_available
        .store(false, Ordering::Release);
    capability.uia_available.store(false, Ordering::Release);
    capability.observer_active.store(false, Ordering::Release);
    capability.wake.notify_one();
}

fn is_confirmed_editor(is_edit_control: bool, value_read_only: Option<bool>) -> bool {
    // TextEditPattern can exist on a read-only Chromium Document. Missing or
    // failed ValuePattern reads remain unknown, never evidence of editability.
    is_edit_control && value_read_only == Some(false)
}

fn inspect_focused_element(
    automation: &IUIAutomation,
    cache: &IUIAutomationCacheRequest,
    generation: u64,
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
    let value_read_only =
        unsafe { element.GetCachedPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
            .ok()
            .and_then(|pattern| unsafe { pattern.CachedIsReadOnly() }.ok())
            .map(|value| value.as_bool());
    let editable = is_confirmed_editor(control_type == UIA_EditControlTypeId.0, value_read_only);
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
        agent_id: agent_id(),
        generation,
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
    generation: u64,
    stop: Arc<AtomicBool>,
    capability: Arc<CapabilityState>,
    queue: Arc<ObservationQueue>,
) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        warn!("text context observer could not initialize COM");
        clear_capability_if_owned(&capability, generation);
        return;
    }

    let pane: Option<IFrameworkInputPane> =
        unsafe { CoCreateInstance(&FRAMEWORK_INPUT_PANE, None, CLSCTX_INPROC_SERVER).ok() };
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
    if stop.load(Ordering::Acquire) || !owns_generation(&capability, generation) {
        unsafe { CoUninitialize() };
        return;
    }
    capability
        .input_pane_available
        .store(pane.is_some(), Ordering::Release);
    capability
        .uia_available
        .store(automation.is_some(), Ordering::Release);
    capability.observer_active.store(true, Ordering::Release);
    capability.generation.store(generation, Ordering::Release);
    capability.wake.notify_one();
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
    let mut last_pane_check = Instant::now();

    while !stop.load(Ordering::Acquire) {
        pump_messages();
        let now = Instant::now();

        if editor_focused {
            if let Some(pane) = &pane {
                if now.duration_since(last_pane_check) >= PANE_POLL_INTERVAL {
                    last_pane_check = now;
                    if let Ok(rect) = unsafe { pane.Location() } {
                        let rect = WireRect::from(rect);
                        let visible = rect.visible();
                        if visible && !previous_pane_visible {
                            queue.publish(Observation {
                                version: 1,
                                agent_id: agent_id(),
                                generation,
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
        } else {
            // Do not carry a stale edge-triggered pane state into the next
            // editor focus, and avoid calling InputPane::Location() at all.
            previous_pane_visible = false;
            last_pane_check = now;
        }

        let rescan_due = RESCAN_AT.with(|slot| match slot.get() {
            Some(at) if now.duration_since(at) >= RESCAN_DEBOUNCE => {
                slot.set(None);
                true
            }
            _ => false,
        });
        let need_snapshot = rescan_due
            || (editor_focused && now.duration_since(last_snapshot) >= FOCUS_WATCHDOG_POLL)
            || (editor_focused && now.duration_since(last_snapshot) >= EDITOR_CARET_POLL);

        if need_snapshot {
            // Advance unconditionally: with UIA unavailable the watchdog must
            // not re-fire every iteration, or the loop degenerates to the
            // clamped 5ms minimum wait.
            last_snapshot = now;
            if let (Some(automation), Some(cache)) = (automation.as_ref(), cache.as_ref()) {
                match inspect_focused_element(automation, cache, generation) {
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
                            agent_id: agent_id(),
                            generation,
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
        if editor_focused {
            wait = wait.min(PANE_POLL_INTERVAL.saturating_sub(now.duration_since(last_pane_check)));
        }
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
    clear_capability_if_owned(&capability, generation);
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

async fn presence_loop(stop: Arc<AtomicBool>, capability: Arc<CapabilityState>) {
    let mut heartbeat = tokio::time::interval(PRESENCE_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                publish_capability(&capability).await;
            }
            _ = capability.wake.notified() => {
                publish_capability(&capability).await;
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
}

async fn publish_capability(capability: &CapabilityState) {
    let body = serde_json::json!({
        "version": 1,
        "agent": capability.agent_present.load(Ordering::Acquire),
        "agent_id": agent_id(),
        "supports_text_context": true,
        "observer_active": capability.observer_active.load(Ordering::Acquire),
        "input_pane": capability.input_pane_available.load(Ordering::Acquire),
        "uia": capability.uia_available.load(Ordering::Acquire),
        "generation": capability.generation.load(Ordering::Acquire),
    });
    if let Err(error) = post_json("/api/v1/text-context/capability", &body).await {
        debug!("text context capability update failed: {error}");
    }
}

pub fn register_presence() {
    let mut presence = PRESENCE.lock().unwrap();
    if presence.is_some() {
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let capability = Arc::new(CapabilityState {
        agent_present: AtomicBool::new(true),
        observer_active: AtomicBool::new(false),
        input_pane_available: AtomicBool::new(false),
        uia_available: AtomicBool::new(false),
        generation: AtomicU64::new(NEXT_GENERATION.fetch_add(1, Ordering::AcqRel)),
        wake: Notify::new(),
    });

    let task_stop = stop.clone();
    let task_capability = capability.clone();
    let task = tauri::async_runtime::spawn(async move {
        presence_loop(task_stop, task_capability).await;
    });
    *presence = Some(PresenceState {
        stop,
        capability,
        task,
    });
}

pub fn reconcile_streaming_state(active: bool) {
    if active && ENABLED.load(Ordering::Acquire) {
        start_observer();
    } else {
        stop_observer();
    }
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        stop_observer();
        let capability = PRESENCE.lock().unwrap().as_ref().map(|presence| {
            presence
                .capability
                .agent_present
                .store(false, Ordering::Release);
            presence.capability.clone()
        });
        if let Some(capability) = capability {
            tauri::async_runtime::spawn(async move {
                publish_capability(&capability).await;
            });
        }
        stop_presence();
        return;
    }

    register_presence();
    if crate::tray::is_text_context_streaming() {
        start_observer();
    }
}

fn start_observer() {
    register_presence();

    let capability = {
        let presence = PRESENCE.lock().unwrap();
        presence
            .as_ref()
            .expect("text context presence must be registered")
            .capability
            .clone()
    };

    let mut state = OBSERVER.lock().unwrap();
    if state.is_some() {
        return;
    }

    let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
    capability.generation.store(generation, Ordering::Release);
    capability.observer_active.store(false, Ordering::Release);
    capability
        .input_pane_available
        .store(false, Ordering::Release);
    capability.uia_available.store(false, Ordering::Release);
    capability.wake.notify_one();

    let stop = Arc::new(AtomicBool::new(false));
    let queue = Arc::new(ObservationQueue::default());
    let (observer_done_tx, observer_done) = std_mpsc::sync_channel(1);

    let observer_stop = stop.clone();
    let observer_capability = capability.clone();
    let observer_queue = queue.clone();
    let observer_thread = std::thread::Builder::new()
        .name("text-context-observer".into())
        .spawn(move || {
            observer_loop(
                generation,
                observer_stop,
                observer_capability,
                observer_queue,
            );
            let _ = observer_done_tx.send(());
        })
        .ok();

    if observer_thread.is_none() {
        warn!("failed to create text context observer thread (generation={generation})");
        capability.observer_active.store(false, Ordering::Release);
        capability.wake.notify_one();
        return;
    }

    let task_stop = stop.clone();
    let task_queue = queue;
    let network_task =
        tauri::async_runtime::spawn(async move { observation_loop(task_stop, task_queue).await });
    *state = Some(ObserverState {
        generation,
        stop,
        observer_thread,
        observer_done,
        network_task: Some(network_task),
    });
}

async fn observation_loop(stop: Arc<AtomicBool>, queue: Arc<ObservationQueue>) {
    loop {
        tokio::select! {
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

fn stop_observer() {
    let Some(mut state) = OBSERVER.lock().unwrap().take() else {
        return;
    };

    if let Some(presence) = PRESENCE.lock().unwrap().as_ref() {
        clear_capability_if_owned(&presence.capability, state.generation);
    }

    state.stop.store(true, Ordering::Release);
    if let Some(thread) = state.observer_thread.take() {
        let generation = state.generation;
        let observer_done = state.observer_done;
        let joiner = std::thread::Builder::new()
            .name("text-context-observer-joiner".into())
            .spawn(move || {
                if observer_done.recv_timeout(OBSERVER_STOP_TIMEOUT).is_ok() {
                    let _ = thread.join();
                } else {
                    warn!(
                        "text context observer did not stop within timeout; detaching thread (generation={generation})"
                    );
                }
            });
        if joiner.is_err() {
            warn!(
                "failed to create text context observer joiner; detaching thread (generation={generation})"
            );
        }
    }
    if let Some(task) = state.network_task.take() {
        task.abort();
    }
    info!(
        "text context observer stopped (generation={})",
        state.generation
    );
}

pub fn stop() {
    stop_observer();

    stop_presence();
}

fn stop_presence() {
    let Some(presence) = PRESENCE.lock().unwrap().take() else {
        return;
    };

    presence.stop.store(true, Ordering::Release);
    presence.capability.wake.notify_one();
    presence.task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicitly_writable_edit_is_confirmed() {
        for (is_edit, read_only, expected) in [
            (true, Some(false), true),
            (true, Some(true), false),
            (true, None, false),
            (false, Some(true), false), // Chrome read-only Document
            (false, Some(false), false),
            (false, None, false),
        ] {
            assert_eq!(is_confirmed_editor(is_edit, read_only), expected);
        }
    }

    fn observation(source: &'static str, active: bool) -> Observation {
        Observation {
            version: 1,
            agent_id: "test-agent",
            generation: 1,
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
