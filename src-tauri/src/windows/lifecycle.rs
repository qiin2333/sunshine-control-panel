//! Owns the lifetime of main/desktop WebViews. Native visibility is the input;
//! focus only requests reconciliation. No session service depends on this module.
//!
//! Loading -> Active -> Suspending -> Background, regardless of suspend success.
//! A visible restore returns to Active and restarts its heartbeat grace period.
//! Close is terminal. Each Panel belongs to one native window lifetime; its
//! revision cancels pending render operations, never operations on a new window
//! that happens to use the same label. State locks never cover COM calls.
//! Streaming services run natively and do not require a hidden panel to stay awake.

use std::sync::LazyLock;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow, WindowEvent};

#[cfg(target_os = "windows")]
mod display;
const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(300);

const STARTUP_GRACE: Duration = Duration::from_secs(90);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RECOVERIES: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Loading,
    Active,
    Suspending,
    Background,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Suspend(u64),
    Resume(u64),
}

struct State {
    phase: Phase,
    revision: u64,
    show_on_load: bool,
    show_revision: u64,
    grace_started: Instant,
    last_beat: Option<Instant>,
    recoveries: u8,
    display_revision: u64,
    refresh_after: Option<Instant>,
}

impl State {
    fn new(show_on_load: bool, now: Instant) -> Self {
        Self {
            phase: Phase::Loading,
            revision: 0,
            show_on_load,
            show_revision: 0,
            grace_started: now,
            last_beat: None,
            recoveries: 0,
            display_revision: 0,
            refresh_after: None,
        }
    }

    fn loaded(&mut self) -> Option<u64> {
        if self.phase == Phase::Closed {
            return None;
        }
        if self.phase == Phase::Loading {
            self.phase = Phase::Active;
        }
        std::mem::take(&mut self.show_on_load).then_some(self.show_revision)
    }

    fn reconcile(&mut self, active: bool, now: Instant) -> Option<Action> {
        match (self.phase, active) {
            (Phase::Closed, _) | (Phase::Loading, false) | (Phase::Active, true) => None,
            (Phase::Loading, true) => {
                self.phase = Phase::Active;
                None
            }
            (_, true) => {
                self.phase = Phase::Active;
                self.revision += 1;
                self.grace_started = now;
                self.last_beat = None;
                Some(Action::Resume(self.revision))
            }
            (Phase::Active, false) => {
                self.phase = Phase::Suspending;
                self.revision += 1;
                Some(Action::Suspend(self.revision))
            }
            _ => None,
        }
    }

    fn display_changed(&mut self, now: Instant) -> Option<u64> {
        if self.phase == Phase::Closed {
            return None;
        }
        self.display_revision += 1;
        self.refresh_after = Some(now + DISPLAY_DEBOUNCE);
        Some(self.display_revision)
    }

    fn pending_refresh(&self, now: Instant) -> Option<u64> {
        (self.phase == Phase::Active && self.refresh_after.is_some_and(|at| now >= at))
            .then_some(self.display_revision)
    }

    fn refreshed(&mut self, revision: u64, succeeded: bool) {
        if self.phase != Phase::Closed && self.display_revision == revision && succeeded {
            self.refresh_after = None;
        }
    }

    fn current(&self, revision: u64) -> bool {
        self.phase != Phase::Closed && self.revision == revision
    }

    fn resume_failed(&mut self, revision: u64) {
        if self.current(revision) && self.phase == Phase::Active {
            self.phase = Phase::Background;
        }
    }

    fn suspend_completed(&mut self, revision: u64) -> bool {
        if !self.current(revision) || self.phase != Phase::Suspending {
            return false;
        }
        self.phase = Phase::Background;
        true
    }

    fn heartbeat(&mut self, now: Instant) {
        if self.phase != Phase::Closed {
            self.last_beat = Some(now);
            self.recoveries = 0;
        }
    }

    fn recover(&mut self, now: Instant) -> bool {
        if !matches!(self.phase, Phase::Loading | Phase::Active)
            || self.recoveries >= MAX_RECOVERIES
            || now.duration_since(self.grace_started) < STARTUP_GRACE
            || self
                .last_beat
                .is_some_and(|last| now.duration_since(last) <= HEARTBEAT_TIMEOUT)
        {
            return false;
        }
        self.recoveries += 1;
        self.grace_started = now;
        self.last_beat = None;
        true
    }

    fn presented(&mut self, now: Instant) {
        self.show_on_load = false;
        self.show_revision += 1;
        self.refresh_after.get_or_insert(now);
    }
}

#[derive(Clone)]
pub(super) struct Panel {
    generation: u64,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct Registry {
    next_generation: u64,
    panels: HashMap<String, Panel>,
}

impl Registry {
    fn register(&mut self, label: &str, show_on_load: bool) -> Panel {
        if let Some(previous) = self.panels.remove(label) {
            previous.close();
        }
        self.next_generation += 1;
        let panel = Panel {
            generation: self.next_generation,
            state: Arc::new(Mutex::new(State::new(show_on_load, Instant::now()))),
        };
        self.panels.insert(label.to_owned(), panel.clone());
        panel
    }

    fn remove(&mut self, label: &str, generation: u64) {
        if self
            .panels
            .get(label)
            .is_some_and(|panel| panel.generation == generation)
        {
            self.panels.remove(label);
        }
    }
}

static PANELS: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

pub(super) fn register(label: &str, show_on_load: bool) -> Panel {
    PANELS.lock().unwrap().register(label, show_on_load)
}

pub(super) fn unregister(label: &str, panel: &Panel) {
    panel.close();
    PANELS.lock().unwrap().remove(label, panel.generation);
}

fn get(label: &str) -> Option<Panel> {
    PANELS.lock().unwrap().panels.get(label).cloned()
}

impl Panel {
    pub(super) fn initialization_script(&self) -> String {
        include_str!("lifecycle.js")
            .replace("__LIFECYCLE_GENERATION__", &self.generation.to_string())
    }

    pub(super) fn close(&self) {
        self.state.lock().unwrap().phase = Phase::Closed;
    }

    pub(super) fn attach<R: Runtime>(&self, window: &WebviewWindow<R>) {
        #[cfg(target_os = "windows")]
        display::install(window, self);
        let panel = self.clone();
        let handle = window.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Focused(focused) => {
                if *focused {
                    crate::proxy_server::reset_fast_fail();
                }
                reconcile(&handle, &panel);
            }
            WindowEvent::Resized(_) => reconcile(&handle, &panel),
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                panel.close();
                PANELS
                    .lock()
                    .unwrap()
                    .remove(handle.label(), panel.generation);
            }
            _ => {}
        });
    }

    pub(super) fn page_loaded<R: Runtime>(&self, window: WebviewWindow<R>) {
        let show_revision = {
            let mut state = self.state.lock().unwrap();
            if state.phase == Phase::Closed {
                return;
            }
            state.loaded()
        };
        if let Some(revision) = show_revision {
            let panel = self.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(120)).await;
                let handle = window.clone();
                let _ = window.run_on_main_thread(move || {
                    let show = {
                        let state = panel.state.lock().unwrap();
                        state.phase != Phase::Closed && state.show_revision == revision
                    };
                    if show && !handle.is_minimized().unwrap_or(true) {
                        super::show_and_activate_window(&handle);
                    }
                });
            });
        } else {
            // A reloaded page has a fresh JS realm even if native visibility
            // hasn't changed. Reapply its state, without requesting focus.
            reconcile(&window, self);
        }
    }
}

pub(super) fn heartbeat(label: &str, generation: u64) {
    if let Some(panel) = get(label).filter(|panel| panel.generation == generation) {
        panel.state.lock().unwrap().heartbeat(Instant::now());
    }
}

pub(super) fn presented<R: Runtime>(window: &WebviewWindow<R>) {
    if let Some(panel) = get(window.label()) {
        let handle = window.clone();
        let _ = window.run_on_main_thread(move || {
            // Async commands can present a panel from a worker thread. Keep
            // this mutation serialized with synchronous native refreshes.
            panel.state.lock().unwrap().presented(Instant::now());
            reconcile(&handle, &panel);
        });
    }
}

fn is_active<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    // An unavailable native query is not permission to hide a renderer.
    window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false)
}

fn notify_visibility<R: Runtime>(window: &WebviewWindow<R>, active: bool) {
    let _ = window.eval(format!(
        "if(window.__setWebviewVisibility)window.__setWebviewVisibility({active})"
    ));
}

fn reconcile<R: Runtime>(window: &WebviewWindow<R>, panel: &Panel) {
    let handle = window.clone();
    let panel = panel.clone();
    let _ = window.run_on_main_thread(move || {
        let active = is_active(&handle);
        let minimized = handle.is_minimized().unwrap_or(false);
        let action = {
            let mut state = panel.state.lock().unwrap();
            let now = Instant::now();
            if minimized {
                state.presented(now);
            }
            state.reconcile(active, now)
        };
        notify_visibility(&handle, active);
        match action {
            Some(Action::Resume(revision)) => {
                log::debug!(
                    "Panel {} generation {} -> active",
                    handle.label(),
                    panel.generation
                );
                apply_render(&handle, panel.clone(), revision, false);
            }
            Some(Action::Suspend(revision)) => {
                log::debug!(
                    "Panel {} generation {} -> background",
                    handle.label(),
                    panel.generation
                );
                let (handle, panel) = (handle.clone(), panel.clone());
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let queued = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        if !panel.state.lock().unwrap().current(revision) {
                            return;
                        }
                        if is_active(&queued) {
                            reconcile(&queued, &panel);
                        } else {
                            apply_render(&queued, panel, revision, true);
                        }
                    });
                });
            }
            None => {}
        }
        #[cfg(target_os = "windows")]
        if active {
            display::refresh(&handle, &panel);
        }
    });
}

#[cfg(target_os = "windows")]
fn apply_render<R: Runtime>(window: &WebviewWindow<R>, panel: Panel, revision: u64, suspend: bool) {
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::ICoreWebView2_3, TrySuspendCompletedHandler,
    };
    use wv2_windows_core::Interface;
    let failed = panel.clone();
    let result = window.with_webview(move |webview| unsafe {
        if !panel.state.lock().unwrap().current(revision) {
            return;
        }
        let controller = webview.controller();
        if !suspend {
            // WebView2 resumes automatically when its controller becomes visible.
            if let Err(error) = controller.SetIsVisible(true) {
                panel.state.lock().unwrap().resume_failed(revision);
                log::warn!("WebView resume failed: {error}");
            }
            return;
        }
        let core = controller
            .CoreWebView2()
            .and_then(|core| core.cast::<ICoreWebView2_3>());
        let callback_panel = panel.clone();
        let callback_controller = controller.clone();
        let completed = TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
            let succeeded = result.is_ok() && suspended;
            let current = callback_panel
                .state
                .lock()
                .unwrap()
                .suspend_completed(revision);
            if current && !succeeded {
                let _ = callback_controller.SetIsVisible(true);
            }
            Ok(())
        }));
        let result = controller
            .SetIsVisible(false)
            .and_then(|_| core)
            .and_then(|core| core.TrySuspend(&completed));
        if let Err(error) = result {
            if panel.state.lock().unwrap().suspend_completed(revision) {
                let _ = controller.SetIsVisible(true);
            }
            log::warn!("WebView suspension failed: {error}");
        }
    });
    if let Err(error) = result {
        if suspend {
            failed.state.lock().unwrap().suspend_completed(revision);
        } else {
            failed.state.lock().unwrap().resume_failed(revision);
        }
        log::warn!("WebView lifecycle dispatch failed: {error}");
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_render<R: Runtime>(window: &WebviewWindow<R>, panel: Panel, revision: u64, suspend: bool) {
    if suspend {
        panel.state.lock().unwrap().suspend_completed(revision);
    }
}

pub(crate) fn start_monitor(app: AppHandle) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let panels: Vec<_> = PANELS
                .lock()
                .unwrap()
                .panels
                .iter()
                .map(|(label, panel)| (label.clone(), panel.clone()))
                .collect();
            for (label, panel) in panels {
                let Some(window) = app.get_webview_window(&label) else {
                    continue;
                };
                let handle = window.clone();
                let _ = window.run_on_main_thread(move || {
                    reconcile(&handle, &panel);
                    let active = is_active(&handle);
                    let recover = {
                        let mut state = panel.state.lock().unwrap();
                        (active || state.show_on_load) && state.recover(Instant::now())
                    };
                    #[cfg(target_os = "windows")]
                    if recover {
                        log::warn!(
                            "WebView heartbeat stalled [{}], generation {}; reloading",
                            handle.label(),
                            panel.generation
                        );
                        let _ = handle.with_webview(move |webview| unsafe {
                            if panel.state.lock().unwrap().phase == Phase::Closed {
                                return;
                            }
                            if let Ok(core) = webview.controller().CoreWebView2() {
                                if let Err(error) = core.Reload() {
                                    log::warn!("WebView recovery failed: {error}");
                                }
                            }
                        });
                    }
                    #[cfg(not(target_os = "windows"))]
                    let _ = recover;
                });
            }
        }
    })
}

pub(crate) fn shutdown() {
    let mut registry = PANELS.lock().unwrap();
    for panel in registry.panels.values() {
        panel.close();
    }
    registry.panels.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_not_focus_drives_idempotent_transitions() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        assert_eq!(state.reconcile(false, now), None); // hidden page still loading
        state.loaded();
        let Some(Action::Suspend(old)) = state.reconcile(false, now) else {
            panic!()
        };
        assert_eq!(state.reconcile(false, now), None);
        let Some(Action::Resume(new)) = state.reconcile(true, now) else {
            panic!()
        };
        assert!(!state.suspend_completed(old));
        assert!(state.current(new));
        assert_eq!(state.reconcile(true, now), None); // focus changes while visible
        let Some(Action::Suspend(latest)) = state.reconcile(false, now) else {
            panic!()
        };
        assert!(!state.suspend_completed(old)); // first hide's late callback
        assert_eq!(state.phase, Phase::Suspending);
        assert!(state.current(latest));
        assert!(!state.current(new));
        assert!(state.suspend_completed(latest));
        assert_eq!(state.phase, Phase::Background);
    }

    #[test]
    fn recovery_covers_missing_first_beat_and_stops_after_three_attempts() {
        let now = Instant::now();
        let mut state = State::new(true, now);
        assert!(!state.recover(now + Duration::from_secs(89)));
        for attempt in 1..=3 {
            assert!(state.recover(now + STARTUP_GRACE * attempt));
        }
        assert!(!state.recover(now + Duration::from_secs(900)));
        state.heartbeat(now + Duration::from_secs(900));
        assert!(!state.recover(now + Duration::from_secs(910)));
        assert!(state.recover(now + Duration::from_secs(940)));
    }

    #[test]
    fn sleeping_page_is_not_recovered_and_resume_gets_a_fresh_grace_period() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        state.loaded();
        let Some(Action::Suspend(revision)) = state.reconcile(false, now) else {
            panic!()
        };
        state.suspend_completed(revision);
        assert_eq!(state.phase, Phase::Background);
        assert!(!state.suspend_completed(revision)); // duplicate completion is stale
        let later = now + Duration::from_secs(600);
        assert!(!state.recover(later));
        state.reconcile(true, later);
        assert!(!state.recover(later));
        assert!(state.recover(later + STARTUP_GRACE));
    }

    #[test]
    fn close_recreate_and_late_callbacks_are_isolated() {
        let mut registry = Registry::default();
        let old = registry.register("main", true);
        let new = registry.register("main", false);
        assert_eq!(old.state.lock().unwrap().phase, Phase::Closed);
        assert_eq!(old.state.lock().unwrap().loaded(), None);
        {
            let mut state = old.state.lock().unwrap();
            let later = Instant::now() + STARTUP_GRACE;
            state.heartbeat(later);
            assert!(state.last_beat.is_none());
            assert_eq!(state.reconcile(true, later), None);
            assert!(!state.suspend_completed(0));
            assert!(!state.recover(later));
        }
        registry.remove("main", old.generation);
        assert_eq!(registry.panels["main"].generation, new.generation);
    }

    #[test]
    fn display_refresh_survives_visibility_changes_and_retries_without_reverting_newer_events() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        state.loaded();
        let first = state.display_changed(now).unwrap();
        assert!(state.pending_refresh(now).is_none());
        // Focus/visibility callbacks must not consume a pending display refresh.
        state.reconcile(false, now);
        state.reconcile(true, now);
        assert_eq!(state.pending_refresh(now + DISPLAY_DEBOUNCE), Some(first));
        state.refreshed(first, false);
        let later = now + DISPLAY_DEBOUNCE * 2;
        assert_eq!(state.pending_refresh(later), Some(first));
        let newer = state.display_changed(later).unwrap();
        state.refreshed(first, true);
        assert!(state.pending_refresh(later).is_none());
        assert_eq!(state.pending_refresh(later + DISPLAY_DEBOUNCE), Some(newer));
        state.refreshed(newer, true);
        assert!(state.pending_refresh(later + DISPLAY_DEBOUNCE).is_none());
        state.phase = Phase::Closed;
        assert!(state.display_changed(now).is_none());
        assert!(state.pending_refresh(later).is_none());
    }

    #[test]
    fn presenting_during_display_debounce_keeps_the_refresh_timer_current() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        state.loaded();
        let timer_revision = state.display_changed(now).unwrap();
        for elapsed in [50, 250] {
            state.presented(now + Duration::from_millis(elapsed));
            assert!(
                state
                    .pending_refresh(now + Duration::from_millis(elapsed))
                    .is_none()
            );
        }
        assert_eq!(state.display_revision, timer_revision);
        assert_eq!(
            state.pending_refresh(now + DISPLAY_DEBOUNCE),
            Some(timer_revision)
        );
        state.refreshed(timer_revision, true);
        assert!(state.pending_refresh(now + DISPLAY_DEBOUNCE).is_none());
    }

    #[test]
    fn display_changes_leave_hidden_panels_asleep_and_refresh_only_after_restore() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        state.loaded();
        let Some(Action::Suspend(old)) = state.reconcile(false, now) else {
            panic!()
        };
        let refresh = state.display_changed(now).unwrap();
        assert_eq!(state.reconcile(false, now), None);
        assert!(state.suspend_completed(old));
        let later = now + DISPLAY_DEBOUNCE;
        assert_eq!(state.reconcile(false, later), None);
        assert!(state.pending_refresh(later).is_none());
        assert!(matches!(
            state.reconcile(true, later),
            Some(Action::Resume(_))
        ));
        assert_eq!(state.pending_refresh(later), Some(refresh));
    }

    #[test]
    fn failed_resume_retries_but_a_stale_failure_cannot_hide_the_current_panel() {
        let now = Instant::now();
        let mut state = State::new(false, now);
        state.loaded();
        state.reconcile(false, now);
        let Some(Action::Resume(first)) = state.reconcile(true, now) else {
            panic!()
        };
        state.resume_failed(first);
        let Some(Action::Resume(retry)) = state.reconcile(true, now) else {
            panic!()
        };
        state.resume_failed(first);
        assert_eq!(state.phase, Phase::Active);
        assert!(state.current(retry));
        state.phase = Phase::Closed;
        state.resume_failed(retry);
        assert_eq!(state.phase, Phase::Closed);
    }

    #[test]
    fn reload_never_repeats_initial_show_and_explicit_presentation_cancels_it() {
        let now = Instant::now();
        let mut state = State::new(true, now);
        let revision = state.loaded().unwrap();
        assert_eq!(state.loaded(), None);
        state.presented(now);
        assert_ne!(state.show_revision, revision);
        assert_eq!(state.pending_refresh(now), Some(0));
        state.refreshed(0, true);
        assert!(state.pending_refresh(now).is_none());
        state.presented(now);
        assert_eq!(state.pending_refresh(now), Some(0));
    }
}
