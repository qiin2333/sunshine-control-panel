use log::{debug, warn};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tokio::sync::Notify;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{ICoreWebView2_3, ICoreWebView2Controller},
    TrySuspendCompletedHandler,
};
use wv2_windows_core::Interface;

const DEBOUNCE: Duration = Duration::from_millis(200);
const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(300);
const DISPLAY_SETTLE: Duration = Duration::from_secs(1);
static STREAMING: AtomicBool = AtomicBool::new(false);
static STATES: Lazy<Mutex<HashMap<String, Arc<Mutex<State>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

struct State {
    wake: Arc<Notify>,
    revision: u64,
    closed: bool,
    scheduled: bool,
    due: Instant,
    controller_hidden: bool,
    refresh_pending: bool,
    refresh_revision: u64,
    refresh_after: Instant,
    settle_until: Instant,
}

impl State {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            wake: Arc::new(Notify::new()),
            revision: 0,
            closed: false,
            scheduled: false,
            due: now,
            controller_hidden: false,
            refresh_pending: false,
            refresh_revision: 0,
            refresh_after: now,
            settle_until: now,
        }
    }

    fn current(&self, revision: u64) -> bool {
        !self.closed && self.revision == revision
    }

    fn close(&mut self) {
        self.closed = true;
        self.wake.notify_one();
    }

    fn should_suspend(&self, visible: bool, streaming: bool, now: Instant) -> bool {
        !visible && !streaming && now >= self.settle_until
    }

    fn finish_refresh(&mut self, revision: u64, success: bool) {
        if success && self.refresh_revision == revision {
            self.refresh_pending = false;
        }
    }

    fn request(&mut self, now: Instant, delay: Duration, refresh: bool, display: bool) -> bool {
        if self.closed {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        self.refresh_pending |= refresh;
        if refresh {
            self.refresh_revision = self.refresh_revision.wrapping_add(1);
        }
        if display {
            self.refresh_after = now + DISPLAY_DEBOUNCE;
            self.settle_until = now + DISPLAY_SETTLE;
        }
        self.due = if self.refresh_pending {
            (now + delay).max(self.refresh_after)
        } else {
            now + delay
        };
        let start = !self.scheduled;
        self.scheduled = true;
        self.wake.notify_one();
        start
    }
}

fn state_for(label: &str) -> Option<Arc<Mutex<State>>> {
    STATES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(label)
        .cloned()
}

pub(super) fn register<R: Runtime>(window: &WebviewWindow<R>, initially_hidden: bool) {
    let state = Arc::new(Mutex::new(State::new()));
    if let Some(old) = STATES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(window.label().to_owned(), state.clone())
    {
        old.lock().unwrap_or_else(|e| e.into_inner()).close();
    }
    let native_window = window.clone();
    let native_state = state.clone();
    if let Err(error) =
        window.run_on_main_thread(move || install_display_hook(native_window, native_state))
    {
        warn!("Failed to install WebView display hook: {error}");
    }
    // A window awaiting its first page load is also natively hidden; only
    // suspend here when the caller actually requested a hidden startup.
    if initially_hidden {
        request(window, false);
    }
}

pub(super) fn forget(label: &str) {
    if let Some(state) = STATES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(label)
    {
        state.lock().unwrap_or_else(|e| e.into_inner()).close();
    }
}

pub(super) fn is_suspended(label: &str) -> bool {
    state_for(label).is_some_and(|state| {
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .controller_hidden
    })
}

pub(super) fn needs_reconcile(label: &str) -> bool {
    state_for(label).is_some_and(|state| {
        let state = state.lock().unwrap_or_else(|e| e.into_inner());
        state.controller_hidden || state.refresh_pending
    })
}

pub(super) fn request<R: Runtime>(window: &WebviewWindow<R>, refresh: bool) {
    if let Some(state) = state_for(window.label()) {
        schedule(window.clone(), state, DEBOUNCE, refresh, false);
    }
}

pub(super) fn set_streaming<R: Runtime>(app: &AppHandle<R>, active: bool) {
    if STREAMING.swap(active, Ordering::AcqRel) == active {
        return;
    }
    for label in [super::MAIN_WINDOW_ID, super::DESKTOP_WINDOW_ID] {
        if let Some(window) = app.get_webview_window(label) {
            request(&window, false);
        }
    }
}

fn schedule<R: Runtime>(
    window: WebviewWindow<R>,
    state: Arc<Mutex<State>>,
    delay: Duration,
    refresh: bool,
    display: bool,
) {
    let wake = {
        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
        if !state.request(Instant::now(), delay, refresh, display) {
            return;
        }
        state.wake.clone()
    };

    // Going through the worker also defers callers already on the GUI thread.
    tauri::async_runtime::spawn(async move {
        loop {
            let due = {
                let state = state.lock().unwrap_or_else(|e| e.into_inner());
                if state.closed {
                    return;
                }
                state.due
            };
            tokio::select! {
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(due)) => {},
                _ = wake.notified() => continue,
            }
            let revision = {
                let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                if state.closed {
                    return;
                }
                if Instant::now() < state.due {
                    continue;
                }
                state.scheduled = false;
                state.revision
            };
            let app = window.app_handle().clone();
            if let Err(error) = app.run_on_main_thread(move || reconcile(window, state, revision)) {
                warn!("Failed to schedule WebView lifecycle update: {error}");
            }
            return;
        }
    });
}

fn reconcile<R: Runtime>(window: WebviewWindow<R>, state: Arc<Mutex<State>>, revision: u64) {
    if !state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .current(revision)
    {
        return;
    }
    // Check on the UI thread, after the native visibility/size event has finished.
    let (Ok(visible), Ok(minimized)) = (window.is_visible(), window.is_minimized()) else {
        return;
    };
    let visible = visible && !minimized;
    super::set_webview_window_visibility(&window, visible);
    let now = Instant::now();
    let (suspend, refresh, refresh_revision, retry_at) = {
        let current = state.lock().unwrap_or_else(|e| e.into_inner());
        if !current.current(revision) {
            return;
        }
        let streaming = STREAMING.load(Ordering::Acquire);
        let suspend = current.should_suspend(visible, streaming, now);
        let refresh = visible && current.refresh_pending;
        let retry_at =
            (!visible && !streaming && now < current.settle_until).then_some(current.settle_until);
        (suspend, refresh, current.refresh_revision, retry_at)
    };
    let label = window.label().to_owned();
    let controller_state = state.clone();
    let dispatched = window.with_webview(move |webview| {
        if !controller_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current(revision)
        {
            return;
        }
        let controller = webview.controller();
        if suspend && !STREAMING.load(Ordering::Acquire) {
            suspend_controller(&controller, &controller_state, revision, &label);
        } else {
            resume_controller(&controller, &controller_state, &label);
        }
        let ready = {
            let state = controller_state.lock().unwrap_or_else(|e| e.into_inner());
            !state.closed && !state.controller_hidden
        };
        if refresh && ready {
            let success = refresh_controller(&controller, &label);
            controller_state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .finish_refresh(refresh_revision, success);
        }
    });
    if let Err(error) = dispatched {
        warn!(
            "Failed to update WebView lifecycle [{}]: {error}",
            window.label()
        );
    }
    let current = state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .current(revision);
    if let Some(retry_at) = retry_at.filter(|_| current) {
        schedule(
            window,
            state,
            retry_at.saturating_duration_since(Instant::now()),
            false,
            false,
        );
    }
}

fn resume_controller(controller: &ICoreWebView2Controller, state: &Arc<Mutex<State>>, label: &str) {
    if !state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .controller_hidden
    {
        return;
    }
    unsafe {
        if let Ok(core) = controller.CoreWebView2()
            && let Ok(webview) = core.cast::<ICoreWebView2_3>()
            && let Err(error) = webview.Resume()
        {
            warn!("Failed to resume WebView [{label}]: {error}");
        }
        match controller.SetIsVisible(true) {
            Ok(()) => {
                let alive = {
                    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                    state.controller_hidden = false;
                    !state.closed
                };
                if alive {
                    super::webview_resumed(label);
                }
            }
            Err(error) => warn!("Failed to restore WebView visibility [{label}]: {error}"),
        }
    }
}

fn suspend_controller(
    controller: &ICoreWebView2Controller,
    state: &Arc<Mutex<State>>,
    revision: u64,
    label: &str,
) {
    if state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .controller_hidden
    {
        return;
    }
    unsafe {
        if let Err(error) = controller.SetIsVisible(false) {
            warn!("Failed to hide WebView [{label}]: {error}");
            return;
        }
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .controller_hidden = true;
        if !state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current(revision)
        {
            return;
        }
        let webview = controller
            .CoreWebView2()
            .and_then(|core| core.cast::<ICoreWebView2_3>());
        if let Ok(webview) = webview {
            let callback_state = state.clone();
            let callback_controller = controller.clone();
            let callback_label = label.to_owned();
            let handler = TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
                if !callback_state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .current(revision)
                {
                    return Ok(());
                }
                if result.is_ok() && suspended {
                    debug!("WebView suspended [{callback_label}]");
                } else {
                    debug!(
                        "WebView suspension did not complete [{callback_label}]: {:?}",
                        result.err()
                    );
                    resume_controller(&callback_controller, &callback_state, &callback_label);
                }
                Ok(())
            }));
            if webview.TrySuspend(&handler).is_ok() {
                return;
            }
        }
        resume_controller(controller, state, label);
    }
}

fn refresh_controller(controller: &ICoreWebView2Controller, label: &str) -> bool {
    unsafe {
        let mut bounds = Default::default();
        let result = controller.Bounds(&mut bounds).and_then(|()| {
            let mut nudged = bounds;
            nudged.right = nudged.right.saturating_add(1);
            // Always attempt to restore the bounds, even when the first call fails.
            let nudged_result = controller.SetBounds(nudged);
            let restored = controller.SetBounds(bounds);
            nudged_result.and(restored)
        });
        if let Err(error) = result.and_then(|()| controller.NotifyParentWindowPositionChanged()) {
            warn!("Failed to refresh WebView after display change [{label}]: {error}");
            return false;
        }
    }
    true
}

struct DisplayHook<R: Runtime> {
    window: WebviewWindow<R>,
    state: Arc<Mutex<State>>,
}

fn install_display_hook<R: Runtime>(window: WebviewWindow<R>, state: Arc<Mutex<State>>) {
    use windows::Win32::UI::Shell::SetWindowSubclass;
    if state.lock().unwrap_or_else(|e| e.into_inner()).closed {
        return;
    }
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let context = Box::into_raw(Box::new(DisplayHook { window, state }));
    // The HWND owns this context until WM_NCDESTROY; failed installation owns nothing.
    if !unsafe {
        SetWindowSubclass(
            windows::Win32::Foundation::HWND(hwnd.0),
            Some(display_hook::<R>),
            SUBCLASS_ID,
            context as usize,
        )
    }
    .as_bool()
    {
        unsafe {
            drop(Box::from_raw(context));
        }
        warn!("Failed to install WebView display subclass");
    }
}

const SUBCLASS_ID: usize = 0x53535756;

unsafe extern "system" fn display_hook<R: Runtime>(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::{WM_DISPLAYCHANGE, WM_DPICHANGED, WM_NCDESTROY, WM_SHOWWINDOW},
    };
    let context = unsafe { &*(data as *const DisplayHook<R>) };
    if message == WM_NCDESTROY {
        context
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .close();
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(display_hook::<R>), SUBCLASS_ID);
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            drop(Box::from_raw(data as *mut DisplayHook<R>));
            return result;
        }
    }
    if matches!(message, WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SHOWWINDOW) {
        let display = message != WM_SHOWWINDOW;
        schedule(
            context.window.clone(),
            context.state.clone(),
            DEBOUNCE,
            display,
            display,
        );
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_invalidates_queued_suspend_and_its_completion() {
        let mut state = State::new();
        let now = Instant::now();
        assert!(state.request(now, DEBOUNCE, false, false));
        let suspend_revision = state.revision;
        assert!(!state.request(now, Duration::ZERO, false, false));
        assert!(!state.current(suspend_revision));
        assert!(state.current(state.revision));
    }

    #[test]
    fn closed_window_cannot_accept_work_from_a_reused_label() {
        let mut old = State::new();
        old.request(Instant::now(), DEBOUNCE, false, false);
        let revision = old.revision;
        old.close();
        assert!(!old.current(revision));
        assert!(!old.request(Instant::now(), DEBOUNCE, true, false));
        let mut replacement = State::new();
        assert!(replacement.request(Instant::now(), DEBOUNCE, false, false));
        assert!(!old.current(replacement.revision));
    }

    #[test]
    fn visibility_events_keep_pending_display_refresh_and_stabilization() {
        let mut state = State::new();
        let now = Instant::now();
        assert!(state.request(now, DEBOUNCE, true, true));
        assert!(!state.request(now, Duration::ZERO, false, false));
        assert!(state.refresh_pending);
        assert_eq!(state.due, now + DISPLAY_DEBOUNCE);
        assert_eq!(state.settle_until, now + DISPLAY_SETTLE);
    }

    #[test]
    fn streaming_and_display_transitions_prevent_deep_suspend() {
        let mut state = State::new();
        let now = Instant::now();
        state.request(now, DEBOUNCE, true, true);
        assert!(!state.should_suspend(false, false, now));
        assert!(!state.should_suspend(false, true, now + DISPLAY_SETTLE));
        assert!(!state.should_suspend(true, false, now + DISPLAY_SETTLE));
        assert!(state.should_suspend(false, false, now + DISPLAY_SETTLE));
    }

    #[test]
    fn failed_or_stale_refresh_cannot_consume_a_new_display_change() {
        let mut state = State::new();
        let now = Instant::now();
        state.request(now, DEBOUNCE, true, true);
        let first = state.refresh_revision;
        state.finish_refresh(first, false);
        assert!(state.refresh_pending);
        state.request(now, DEBOUNCE, true, true);
        state.finish_refresh(first, true);
        assert!(state.refresh_pending);
        state.finish_refresh(state.refresh_revision, true);
        assert!(!state.refresh_pending);
    }
}
