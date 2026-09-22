mod gamepad;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex,
    },
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    nr_shortcut: String,
    overlay_shortcut: String,
    nr_gamepad: String,
    overlay_gamepad: String,
    opacity: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            nr_shortcut: "Ctrl+Alt+KeyN".into(),
            overlay_shortcut: String::new(),
            nr_gamepad: String::new(),
            overlay_gamepad: String::new(),
            opacity: 62,
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    nr_shortcut: Option<String>,
    overlay_shortcut: Option<String>,
    nr_gamepad: Option<String>,
    overlay_gamepad: Option<String>,
    opacity: Option<u32>,
}
#[derive(Default)]
struct OverlayState {
    settings: Settings,
    owned: HashMap<u32, Shortcut>,
    target: Option<u64>,
    capture_generation: u64,
    capture: Option<(String, std::time::Instant)>,
}
static STATE: LazyLock<Mutex<OverlayState>> = LazyLock::new(|| Mutex::new(OverlayState::default()));
static VISIBLE: AtomicBool = AtomicBool::new(false);
static TOGGLING: AtomicBool = AtomicBool::new(false);
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsStatus {
    settings: Settings,
    nr_registered: bool,
    overlay_registered: bool,
    gamepad_supported: bool,
    gamepad_connected: usize,
    capture_active: bool,
    target: Option<u64>,
}
fn snapshot(state: &OverlayState) -> SettingsStatus {
    let registered = |s: &str| {
        s.parse::<Shortcut>()
            .ok()
            .is_some_and(|key| state.owned.contains_key(&key.id()))
    };
    SettingsStatus {
        settings: state.settings.clone(),
        nr_registered: registered(&state.settings.nr_shortcut),
        overlay_registered: registered(&state.settings.overlay_shortcut),
        gamepad_supported: cfg!(target_os = "windows"),
        gamepad_connected: crate::controller_input::connected_count(),
        capture_active: state.capture.is_some(),
        target: state.target,
    }
}
fn path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| p.join("Sunshine GUI").join("enhancement-controls.json"))
        .ok_or("nr_settings_path".into())
}
fn save(settings: &Settings) -> Result<(), String> {
    let path = path()?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    let result = (|| {
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::{
                core::PCWSTR,
                Win32::Storage::FileSystem::{
                    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
                },
            };
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            }
            .map_err(|e| e.to_string())?;
        }
        #[cfg(not(target_os = "windows"))]
        fs::rename(&temp, &path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn normalized_key(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Ok(String::new());
    }
    if value.len() > 100 {
        return Err("nr_shortcut_invalid".into());
    }
    let key: Shortcut = value.parse().map_err(|_| "nr_shortcut_invalid")?;
    if !key
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT)
        || key.mods.contains(Modifiers::SUPER)
    {
        return Err("nr_shortcut_invalid".into());
    }
    if key.id()
        == crate::app::TOOLBAR_SHORTCUT
            .parse::<Shortcut>()
            .unwrap()
            .id()
    {
        return Err("nr_shortcut_conflict".into());
    }
    Ok(key.to_string())
}
fn bindings(settings: &Settings) -> Result<Vec<Shortcut>, String> {
    gamepad::validate(&settings.nr_gamepad, &settings.overlay_gamepad)?;
    let keys: Vec<Shortcut> = [&settings.nr_shortcut, &settings.overlay_shortcut]
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().map_err(|_| "nr_shortcut_invalid".to_string()))
        .collect::<Result<_, _>>()?;
    if keys.len() == 2 && keys[0].id() == keys[1].id() {
        return Err("nr_shortcut_duplicate".into());
    }
    Ok(keys)
}
trait Registrar {
    fn register(&self, key: Shortcut) -> Result<(), String>;
    fn unregister(&self, key: Shortcut) -> Result<(), String>;
}
struct GlobalRegistrar<'a, R: Runtime>(&'a AppHandle<R>);
impl<R: Runtime> Registrar for GlobalRegistrar<'_, R> {
    fn register(&self, key: Shortcut) -> Result<(), String> {
        if self.0.global_shortcut().is_registered(key) {
            return Err("nr_shortcut_conflict".into());
        }
        self.0
            .global_shortcut()
            .on_shortcut(key, |app, shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app.clone();
                let id = shortcut.id();
                // Never wait for the settings mutex on the UI thread: registration can
                // itself dispatch to that thread while a settings command holds it.
                tauri::async_runtime::spawn(async move {
                    let action = {
                        let state = STATE.lock().unwrap();
                        if state.capture.is_some() {
                            return;
                        }
                        if state
                            .settings
                            .nr_shortcut
                            .parse::<Shortcut>()
                            .ok()
                            .is_some_and(|k| k.id() == id)
                        {
                            1
                        } else if state
                            .settings
                            .overlay_shortcut
                            .parse::<Shortcut>()
                            .ok()
                            .is_some_and(|k| k.id() == id)
                        {
                            2
                        } else {
                            0
                        }
                    };
                    run_action(&app, action).await;
                });
            })
            .map_err(|_| "nr_shortcut_conflict".into())
    }
    fn unregister(&self, key: Shortcut) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(key)
            .map_err(|_| "nr_shortcut_unregister".into())
    }
}
async fn run_action<R: Runtime>(app: &AppHandle<R>, action: u8) {
    let result = match action {
        1 => toggle_nr().await,
        2 => {
            let visible = app
                .get_webview_window("nr_overlay")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            set_visible(app, !visible)
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        log::warn!("NR shortcut: {error}");
        let _ = app.emit("nr-action-error", error);
    }
}
// Track each successful OS operation so even a failed rollback is reported honestly.
fn replace_bindings(
    registrar: &impl Registrar,
    owned: &mut HashMap<u32, Shortcut>,
    desired: &[Shortcut],
) -> Result<(), String> {
    for key in desired {
        if !owned.contains_key(&key.id()) {
            registrar.register(*key)?;
            owned.insert(key.id(), *key);
        }
    }
    let removed: Vec<_> = owned
        .values()
        .filter(|key| !desired.iter().any(|new| new.id() == key.id()))
        .copied()
        .collect();
    for key in removed {
        registrar.unregister(key)?;
        owned.remove(&key.id());
    }
    Ok(())
}
fn apply_settings(
    state: &mut OverlayState,
    next: Settings,
    registrar: &impl Registrar,
    persist: impl FnOnce(&Settings) -> Result<(), String>,
) -> Result<(), String> {
    let mut desired = bindings(&next)?;
    if state.capture.is_some() {
        desired.clear();
    }
    let previous: Vec<_> = state.owned.values().copied().collect();
    if let Err(error) =
        replace_bindings(registrar, &mut state.owned, &desired).and_then(|_| persist(&next))
    {
        if replace_bindings(registrar, &mut state.owned, &previous).is_err() {
            return Err("nr_shortcut_restore_failed".into());
        }
        return Err(error);
    }
    state.settings = next;
    gamepad::wake();
    Ok(())
}
pub fn initialize<R: Runtime>(app: &AppHandle<R>) {
    let mut state = STATE.lock().unwrap();
    if let Ok(settings) = path()
        .and_then(|p| fs::read(p).map_err(|e| e.to_string()))
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).map_err(|e| e.to_string()))
    {
        if let (Ok(nr), Ok(overlay)) = (
            normalized_key(&settings.nr_shortcut),
            normalized_key(&settings.overlay_shortcut),
        ) {
            let settings = Settings {
                nr_shortcut: nr,
                overlay_shortcut: overlay,
                nr_gamepad: gamepad::normalize(&settings.nr_gamepad).unwrap_or_default(),
                overlay_gamepad: gamepad::normalize(&settings.overlay_gamepad).unwrap_or_default(),
                opacity: settings.opacity.clamp(35, 95),
            };
            if bindings(&settings).is_ok() {
                state.settings = settings;
            }
        }
    }
    for key in bindings(&state.settings).unwrap_or_default() {
        match GlobalRegistrar(app).register(key) {
            Ok(()) => {
                state.owned.insert(key.id(), key);
            }
            Err(error) => log::warn!("NR shortcut unavailable at startup: {error}"),
        }
    }
    drop(state);
    gamepad::start(app);
}
// Restore from current settings, including edits made by another manager window.
fn finish_capture(state: &mut OverlayState, registrar: &impl Registrar) -> Result<(), String> {
    state.capture = None;
    gamepad::wake();
    replace_bindings(registrar, &mut state.owned, &bindings(&state.settings)?)
}
#[tauri::command]
pub async fn nr_overlay_capture_shortcut(
    window: tauri::WebviewWindow,
    active: bool,
) -> Result<u64, String> {
    let app = window.app_handle().clone();
    let mut state = STATE.lock().unwrap();
    if active {
        let previous: Vec<_> = state.owned.values().copied().collect();
        if let Err(error) = replace_bindings(&GlobalRegistrar(&app), &mut state.owned, &[]) {
            replace_bindings(&GlobalRegistrar(&app), &mut state.owned, &previous)
                .map_err(|_| "nr_shortcut_restore_failed")?;
            return Err(error);
        }
        let lease = (
            window.label().to_string(),
            std::time::Instant::now() + std::time::Duration::from_secs(30),
        );
        state.capture_generation += 1;
        state.capture = Some(lease.clone());
        gamepad::wake();
        let _ = app.emit("nr-settings-changed", snapshot(&state));
        // Backend expiry also restores bindings when the recording window disappears.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let result = {
                let mut state = STATE.lock().unwrap();
                if state.capture.as_ref() != Some(&lease) {
                    return;
                }
                let result = finish_capture(&mut state, &GlobalRegistrar(&app));
                (result, snapshot(&state))
            };
            let _ = app.emit("nr-settings-changed", result.1);
            if let Err(error) = result.0 {
                let _ = app.emit("nr-action-error", error);
            }
        });
    } else if state
        .capture
        .as_ref()
        .is_some_and(|(label, _)| label == window.label())
    {
        let result = finish_capture(&mut state, &GlobalRegistrar(&app));
        let status = snapshot(&state);
        let generation = state.capture_generation;
        drop(state);
        let _ = app.emit("nr-settings-changed", status);
        result?;
        return Ok(generation);
    }
    Ok(state.capture_generation)
}
#[tauri::command]
pub async fn nr_overlay_settings() -> SettingsStatus {
    snapshot(&STATE.lock().unwrap())
}
#[tauri::command]
pub async fn nr_overlay_save_settings(
    app: AppHandle,
    patch: SettingsPatch,
) -> Result<SettingsStatus, String> {
    let result = {
        let mut state = STATE.lock().unwrap();
        let mut next = state.settings.clone();
        let update_shortcuts = patch.nr_shortcut.is_some()
            || patch.overlay_shortcut.is_some()
            || patch.nr_gamepad.is_some()
            || patch.overlay_gamepad.is_some();
        if let Some(key) = patch.nr_shortcut {
            next.nr_shortcut = normalized_key(&key)?;
        }
        if let Some(key) = patch.overlay_shortcut {
            next.overlay_shortcut = normalized_key(&key)?;
        }
        if let Some(key) = patch.nr_gamepad {
            next.nr_gamepad = gamepad::normalize(&key)?;
        }
        if let Some(key) = patch.overlay_gamepad {
            next.overlay_gamepad = gamepad::normalize(&key)?;
        }
        if let Some(opacity) = patch.opacity {
            if !(35..=95).contains(&opacity) {
                return Err("nr_opacity_invalid".into());
            }
            next.opacity = opacity;
        }
        let result = if update_shortcuts {
            apply_settings(&mut state, next, &GlobalRegistrar(&app), save)
        } else {
            save(&next).map(|_| state.settings = next)
        };
        let status = snapshot(&state);
        (result, status)
    };
    let _ = app.emit("nr-settings-changed", &result.1);
    result.0.map(|_| result.1)
}
#[tauri::command]
pub async fn nr_overlay_select_session(app: AppHandle, id: u64) {
    let mut state = STATE.lock().unwrap();
    state.target = Some(id);
    let status = snapshot(&state);
    drop(state);
    let _ = app.emit("nr-settings-changed", status);
}
pub fn visibility_requested() -> bool {
    VISIBLE.load(Ordering::Acquire)
}
pub fn request_visibility<R: Runtime>(app: &AppHandle<R>, visible: bool) {
    VISIBLE.store(visible, Ordering::Release);
    let _ = app.emit("nr-overlay-visibility", visible);
}
pub fn set_visible<R: Runtime>(app: &AppHandle<R>, visible: bool) -> Result<(), String> {
    request_visibility(app, visible);
    let result = if visible {
        crate::toolbar::create_tool_window_internal(app, "nr")
    } else if let Some(window) = app.get_webview_window("nr_overlay") {
        window.hide().map_err(|e| e.to_string())
    } else {
        Ok(())
    };
    if result.is_err() {
        let actual = app
            .get_webview_window("nr_overlay")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
        request_visibility(app, actual);
    }
    result
}
#[tauri::command]
pub async fn nr_overlay_set_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    set_visible(&app, visible)
}
#[tauri::command]
pub fn nr_overlay_is_visible(app: AppHandle) -> bool {
    app.get_webview_window("nr_overlay")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}
fn toggle_request(body: &serde_json::Value, target: Option<u64>) -> Result<(u64, bool), String> {
    let pipelines = body["pipelines"].as_array().ok_or("nr_status_invalid")?;
    let selected = if let Some(id) = target {
        pipelines
            .iter()
            .find(|p| p["id"].as_u64() == Some(id))
            .ok_or("nr_session_ended")?
    } else if pipelines.len() == 1 {
        &pipelines[0]
    } else {
        return Err(if pipelines.is_empty() {
            "nr_no_session"
        } else {
            "nr_select_session"
        }
        .into());
    };
    if selected["nr_toggle_supported"] != true {
        return Err("nr_toggle_unsupported".into());
    }
    let enabled = selected["nr_requested_enabled"]
        .as_bool()
        .ok_or("nr_status_invalid")?;
    let state = selected["nr_state"].as_str().ok_or("nr_status_invalid")?;
    let pending = matches!(state, "warming_up" | "stopping")
        || (enabled && !matches!(state, "active" | "degraded"))
        || (!enabled && state == "active")
        || (enabled
            && state == "active"
            && [
                "scale_percent",
                "intensity",
                "style",
                "motion_quality",
                "ui_correction",
                "skin_structure_strength",
                "auto_mask",
            ]
            .iter()
            .any(|key| selected[format!("nr_requested_{key}")] != selected[format!("nr_{key}")]));
    if pending {
        return Err("nr_settings_pending".into());
    }
    Ok((
        selected["id"]
            .as_u64()
            .filter(|id| *id > 0)
            .ok_or("nr_status_invalid")?,
        !enabled,
    ))
}
async fn toggle_nr() -> Result<(), String> {
    if TOGGLING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TOGGLING.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    let target = STATE.lock().unwrap().target;
    let body = crate::hdr_enhanced::nr_live_status().await?;
    let (id, enabled) = toggle_request(&body, target)?;
    // Recheck selection after the network read; never act on a newly selected stream.
    if STATE.lock().unwrap().target != target {
        return Err("nr_select_session".into());
    }
    crate::hdr_enhanced::nr_live_set_enabled(id, enabled, None, None, None, None, None, None, None)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake {
        keys: RefCell<Vec<u32>>,
        conflict: Option<u32>,
    }
    impl Registrar for Fake {
        fn register(&self, key: Shortcut) -> Result<(), String> {
            if self.conflict == Some(key.id()) {
                return Err("nr_shortcut_conflict".into());
            }
            self.keys.borrow_mut().push(key.id());
            Ok(())
        }
        fn unregister(&self, key: Shortcut) -> Result<(), String> {
            self.keys.borrow_mut().retain(|id| *id != key.id());
            Ok(())
        }
    }
    #[test]
    fn conflict_and_persistence_failure_keep_previous_bindings() {
        let mut state = OverlayState::default();
        let old = bindings(&state.settings).unwrap()[0];
        state.owned.insert(old.id(), old);
        let new: Shortcut = "Ctrl+Alt+KeyK".parse().unwrap();
        let mut fake = Fake {
            keys: RefCell::new(vec![old.id()]),
            conflict: Some(new.id()),
        };
        let next = Settings {
            nr_shortcut: new.to_string(),
            ..Settings::default()
        };
        assert!(apply_settings(&mut state, next.clone(), &fake, |_| Ok(())).is_err());
        assert_eq!(state.settings, Settings::default());
        assert_eq!(*fake.keys.borrow(), vec![old.id()]);
        fake.conflict = None;
        assert!(apply_settings(&mut state, next, &fake, |_| Err("disk failed".into())).is_err());
        assert_eq!(state.settings, Settings::default());
        assert_eq!(*fake.keys.borrow(), vec![old.id()]);
    }
    #[test]
    fn canonical_duplicates_and_unmodified_keys_are_rejected() {
        assert!(normalized_key("KeyN").is_err());
        assert!(normalized_key("Ctrl+Alt+Shift+KeyT").is_err());
        let key = normalized_key("Ctrl+Alt+N").unwrap();
        assert!(bindings(&Settings {
            nr_shortcut: key.clone(),
            overlay_shortcut: key,
            ..Settings::default()
        })
        .is_err());
        assert_eq!(normalized_key("").unwrap(), "");
    }
    #[test]
    fn capture_restores_latest_bindings() {
        let mut state = OverlayState::default();
        let fake = Fake {
            keys: RefCell::new(vec![]),
            conflict: None,
        };
        let old = bindings(&state.settings).unwrap();
        replace_bindings(&fake, &mut state.owned, &old).unwrap();
        replace_bindings(&fake, &mut state.owned, &[]).unwrap();
        state.capture = Some(("manager".into(), std::time::Instant::now()));
        let next = Settings {
            nr_shortcut: "Ctrl+Alt+KeyK".into(),
            ..Settings::default()
        };
        apply_settings(&mut state, next, &fake, |_| Ok(())).unwrap();
        assert!(fake.keys.borrow().is_empty());
        finish_capture(&mut state, &fake).unwrap();
        assert!(state.capture.is_none());
        assert_eq!(
            *fake.keys.borrow(),
            vec![bindings(&state.settings).unwrap()[0].id()]
        );
    }
    #[test]
    fn gamepad_settings_preserve_keyboard_and_old_settings_load() {
        let old: Settings =
            serde_json::from_str(r#"{"nrShortcut":"Ctrl+Alt+KeyN","opacity":62}"#).unwrap();
        assert_eq!(old, Settings::default());
        let mut state = OverlayState::default();
        let fake = Fake {
            keys: RefCell::new(vec![]),
            conflict: None,
        };
        let keyboard = bindings(&state.settings).unwrap();
        replace_bindings(&fake, &mut state.owned, &keyboard).unwrap();
        let next = Settings {
            nr_gamepad: "LB+A".into(),
            overlay_gamepad: "Back+Y".into(),
            ..old
        };
        assert!(apply_settings(&mut state, next.clone(), &fake, |_| Err("disk".into())).is_err());
        assert!(state.settings.nr_gamepad.is_empty());
        apply_settings(&mut state, next.clone(), &fake, |_| Ok(())).unwrap();
        assert_eq!(state.settings, next);
        assert_eq!(*fake.keys.borrow(), vec![keyboard[0].id()]);
        let duplicate = Settings {
            overlay_gamepad: "A+LB".into(),
            ..next
        };
        assert!(apply_settings(&mut state, duplicate, &fake, |_| Ok(())).is_err());
        assert_eq!(state.settings.overlay_gamepad, "Back+Y");
    }
    #[test]
    fn hidden_shortcut_never_redirects_an_expired_target() {
        let p = serde_json::json!({"id":2,"nr_toggle_supported":true,"nr_requested_enabled":false,"nr_state":"disabled"});
        assert_eq!(
            toggle_request(&serde_json::json!({"pipelines":[p.clone()]}), None).unwrap(),
            (2, true)
        );
        assert!(toggle_request(&serde_json::json!({"pipelines":[p.clone()]}), Some(1)).is_err());
        assert!(toggle_request(&serde_json::json!({"pipelines":[p.clone(),p]}), None).is_err());
        assert!(toggle_request(&serde_json::json!({"pipelines":[]}), None).is_err());
    }
}
