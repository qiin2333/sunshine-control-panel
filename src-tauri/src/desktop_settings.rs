use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(debug_assertions)]
use tauri::Manager;

const SETTINGS_FILE: &str = "desktop-settings.json";
const RUN_VALUE_NAME: &str = "Sunshine GUI Desktop";
const REMOVE_AUTO_START_ARG: &str = "--remove-autostart";

pub fn try_remove_auto_start_from_args() -> bool {
    if !std::env::args().any(|arg| arg == REMOVE_AUTO_START_ARG) {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        use winreg::RegKey;
        use winreg::enums::*;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        ) {
            let _ = run_key.delete_value(RUN_VALUE_NAME);
        }
    }
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub auto_start: bool,
    pub start_minimized: bool,
    pub auto_start_sunshine: bool,
    pub file_mapping_menu_enabled: bool,
    pub notifications: bool,
    pub connection_notify: bool,
    pub update_notify: bool,
    pub dev_mode: bool,
    pub log_level: String,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            auto_start: true,
            start_minimized: true,
            auto_start_sunshine: true,
            file_mapping_menu_enabled: true,
            notifications: true,
            connection_notify: true,
            update_notify: true,
            dev_mode: false,
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsStatus {
    pub auto_start_registered: bool,
    pub settings_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsResponse {
    pub settings: DesktopSettings,
    pub status: DesktopSettingsStatus,
}

fn settings_dir() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|dir| dir.join("Sunshine GUI"))
        .ok_or_else(|| "Cannot resolve user config directory".to_string())
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(settings_dir()?.join(SETTINGS_FILE))
}

fn normalize(settings: &mut DesktopSettings) {
    let level = settings.log_level.trim().to_ascii_lowercase();
    settings.log_level = match level.as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => level,
        _ => "info".to_string(),
    };
}

#[cfg(target_os = "windows")]
fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn load_desktop_settings_from_disk() -> DesktopSettings {
    let Ok(path) = settings_path() else {
        return DesktopSettings::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return DesktopSettings::default();
    };
    let mut settings = serde_json::from_str::<DesktopSettings>(&text).unwrap_or_default();
    normalize(&mut settings);
    settings
}

fn save_desktop_settings_to_disk(settings: &DesktopSettings) -> Result<(), String> {
    let dir = settings_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(dir.join(SETTINGS_FILE), text).map_err(|e| e.to_string())
}

pub fn set_file_mapping_menu_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = load_desktop_settings_from_disk();
    settings.file_mapping_menu_enabled = enabled;
    save_desktop_settings_to_disk(&settings)
}

#[cfg(target_os = "windows")]
fn startup_command(start_minimized: bool) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mode = if start_minimized {
        "--hidden"
    } else {
        "--desktop"
    };
    Ok(format!("\"{}\" {}", exe.display(), mode))
}

#[cfg(target_os = "windows")]
fn apply_auto_start(settings: &DesktopSettings) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::*;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;
    if settings.auto_start {
        let command = startup_command(settings.start_minimized)?;
        run_key
            .set_value(RUN_VALUE_NAME, &command)
            .map_err(|e| e.to_string())?;
    } else {
        let _ = run_key.delete_value(RUN_VALUE_NAME);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn apply_auto_start(_settings: &DesktopSettings) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_auto_start_registered() -> bool {
    use winreg::RegKey;
    use winreg::enums::*;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE_NAME))
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn is_auto_start_registered() -> bool {
    false
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServiceStartMode {
    Auto,
    DelayedAuto,
    Demand,
    Disabled,
}

#[cfg(target_os = "windows")]
fn sunshine_service_start_mode() -> Result<Option<ServiceStartMode>, String> {
    use winreg::RegKey;
    use winreg::enums::*;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let service = match hklm.open_subkey(r"SYSTEM\CurrentControlSet\Services\SunshineService") {
        Ok(service) => service,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let start = service
        .get_value::<u32, _>("Start")
        .map_err(|error| error.to_string())?;
    match start {
        2 if service.get_value::<u32, _>("DelayedAutoStart").unwrap_or(0) != 0 => {
            Ok(Some(ServiceStartMode::DelayedAuto))
        }
        2 => Ok(Some(ServiceStartMode::Auto)),
        3 => Ok(Some(ServiceStartMode::Demand)),
        4 => Ok(Some(ServiceStartMode::Disabled)),
        value => Err(format!("Unsupported Sunshine service start type: {value}")),
    }
}

#[cfg(target_os = "windows")]
fn core_auto_start_ready(mode: Option<ServiceStartMode>) -> bool {
    matches!(
        mode,
        None | Some(ServiceStartMode::Auto | ServiceStartMode::DelayedAuto)
    )
}

#[cfg(not(target_os = "windows"))]
fn core_auto_start_ready(_mode: Option<()>) -> bool {
    true
}

pub fn is_combined_auto_start_enabled() -> bool {
    #[cfg(target_os = "windows")]
    let core_ready = sunshine_service_start_mode()
        .map(core_auto_start_ready)
        .unwrap_or(false);
    #[cfg(not(target_os = "windows"))]
    let core_ready = core_auto_start_ready(None);

    is_auto_start_registered() && core_ready
}

#[cfg(target_os = "windows")]
fn configure_sunshine_service_start(mode: ServiceStartMode) -> Result<(), String> {
    let start_mode = match mode {
        ServiceStartMode::Auto => "auto",
        ServiceStartMode::DelayedAuto => "delayed-auto",
        ServiceStartMode::Demand => "demand",
        ServiceStartMode::Disabled => "disabled",
    };
    let script_path = std::env::temp_dir().join("sunshine-set-service-start.bat");
    let script = format!(
        "@echo off\r\nsc.exe config SunshineService start= {start_mode}\r\nexit /b %ERRORLEVEL%\r\n"
    );
    fs::write(&script_path, script).map_err(|e| e.to_string())?;
    let result = crate::bat_runner::run_elevated(&script_path, "service-start", &[]);
    let _ = fs::remove_file(&script_path);
    result
}

pub fn set_combined_auto_start_enabled(enabled: bool) -> Result<(), String> {
    let previous = load_desktop_settings_from_disk();
    let mut settings = previous.clone();
    settings.auto_start = enabled;
    settings.auto_start_sunshine = enabled;

    #[cfg(target_os = "windows")]
    let previous_service_mode = sunshine_service_start_mode()?;
    #[cfg(target_os = "windows")]
    let mut service_changed = false;
    #[cfg(target_os = "windows")]
    if let Some(mode) = previous_service_mode {
        let target_mode = if enabled {
            ServiceStartMode::Auto
        } else {
            ServiceStartMode::Demand
        };
        if mode != target_mode {
            configure_sunshine_service_start(target_mode)?;
            service_changed = true;
        }
    }

    if let Err(error) = save_desktop_settings_to_disk(&settings) {
        #[cfg(target_os = "windows")]
        if service_changed && let Some(mode) = previous_service_mode {
            let _ = configure_sunshine_service_start(mode);
        }
        return Err(error);
    }

    if let Err(error) = apply_auto_start(&settings) {
        let _ = save_desktop_settings_to_disk(&previous);
        #[cfg(target_os = "windows")]
        if service_changed && let Some(mode) = previous_service_mode {
            let _ = configure_sunshine_service_start(mode);
        }
        return Err(error);
    }

    Ok(())
}

fn status() -> DesktopSettingsStatus {
    DesktopSettingsStatus {
        auto_start_registered: is_auto_start_registered(),
        settings_path: settings_path()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

#[tauri::command]
pub fn get_desktop_settings() -> DesktopSettingsResponse {
    DesktopSettingsResponse {
        settings: load_desktop_settings_from_disk(),
        status: status(),
    }
}

#[tauri::command]
pub async fn save_desktop_settings(
    app: AppHandle,
    mut settings: DesktopSettings,
) -> Result<DesktopSettingsResponse, String> {
    normalize(&mut settings);
    save_desktop_settings_to_disk(&settings)?;
    apply_auto_start(&settings)?;
    crate::tray::refresh_menu(&app);
    crate::logger::set_log_level(&settings.log_level);
    apply_dev_mode(&app, settings.dev_mode);

    if settings.auto_start_sunshine {
        ensure_sunshine_started_async();
    }

    Ok(DesktopSettingsResponse {
        settings,
        status: status(),
    })
}

pub fn apply_startup_settings(app: &AppHandle, settings: &DesktopSettings) {
    crate::logger::set_log_level(&settings.log_level);
    apply_dev_mode(app, settings.dev_mode);
    if let Err(e) = apply_auto_start(settings) {
        log::warn!("Failed to reconcile GUI auto-start registration: {}", e);
    }
    if settings.auto_start_sunshine {
        ensure_sunshine_started_async();
    }
}

fn apply_dev_mode(_app: &AppHandle, enabled: bool) {
    if !enabled {
        return;
    }

    #[cfg(debug_assertions)]
    {
        for label in ["main", "desktop", "log_console"] {
            if let Some(window) = _app.get_webview_window(label) {
                window.open_devtools();
            }
        }
    }
}

pub fn ensure_sunshine_started_async() {
    tauri::async_runtime::spawn(async {
        if let Err(e) = ensure_sunshine_started().await {
            log::warn!("Failed to auto-start Sunshine: {}", e);
        }
    });
}

async fn ensure_sunshine_started() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let already_running = tokio::task::spawn_blocking(|| {
            std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "if (Get-Process sunshine -ErrorAction SilentlyContinue) { 'true' } else { 'false' }",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("true"))?;

        if already_running {
            return Ok(());
        }

        let install_dir = crate::sunshine::install_dir();
        let sunshine_exe = install_dir.join("sunshine.exe");
        if !sunshine_exe.exists() {
            return Err(format!(
                "sunshine.exe not found: {}",
                sunshine_exe.display()
            ));
        }

        let command = format!(
            "$svc = Get-Service -Name 'SunshineService' -ErrorAction SilentlyContinue; \
             if ($svc) {{ Start-Service -Name 'SunshineService' }} \
             else {{ Start-Process -FilePath '{}' -WorkingDirectory '{}' -WindowStyle Hidden }}",
            powershell_single_quote(&sunshine_exe.to_string_lossy()),
            powershell_single_quote(&install_dir.to_string_lossy())
        );

        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &command])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn combined_startup_accepts_automatic_or_service_free_core() {
        assert!(core_auto_start_ready(None));
        assert!(core_auto_start_ready(Some(ServiceStartMode::Auto)));
        assert!(core_auto_start_ready(Some(ServiceStartMode::DelayedAuto)));
        assert!(!core_auto_start_ready(Some(ServiceStartMode::Demand)));
        assert!(!core_auto_start_ready(Some(ServiceStartMode::Disabled)));
    }
}
