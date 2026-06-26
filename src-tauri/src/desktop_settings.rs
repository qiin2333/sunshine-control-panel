use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const SETTINGS_FILE: &str = "desktop-settings.json";
const RUN_VALUE_NAME: &str = "Sunshine GUI Desktop";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub auto_start: bool,
    pub start_minimized: bool,
    pub auto_start_sunshine: bool,
    pub notifications: bool,
    pub connection_notify: bool,
    pub update_notify: bool,
    pub dev_mode: bool,
    pub log_level: String,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            start_minimized: false,
            auto_start_sunshine: true,
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

#[cfg(target_os = "windows")]
fn startup_command(start_minimized: bool) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = format!("\"{}\" --desktop", exe.display());
    if start_minimized {
        command.push_str(" --minimized");
    }
    Ok(command)
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
fn is_auto_start_registered() -> bool {
    use winreg::RegKey;
    use winreg::enums::*;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE_NAME))
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_auto_start_registered() -> bool {
    false
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
    if settings.auto_start_sunshine {
        ensure_sunshine_started_async();
    }
}

fn apply_dev_mode(app: &AppHandle, enabled: bool) {
    if !enabled {
        return;
    }

    #[cfg(debug_assertions)]
    {
        for label in ["main", "desktop", "log_console"] {
            if let Some(window) = app.get_webview_window(label) {
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
