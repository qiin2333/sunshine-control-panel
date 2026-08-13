//! Optional DualSense component lifecycle.
//!
//! HIDMaestro is downloaded from its pinned upstream release and verified before
//! extraction. The Sunshine-owned sidecar is only copied from the installed
//! Sunshine package (or an explicit development override), keeping third-party
//! runtime and first-party process ownership separate.

use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;

const COMPONENT_VERSION: &str = "1.0.0";
const PROTOCOL_VERSION: u32 = 1;
const HIDMAESTRO_VERSION: &str = "v1.6.1";
const HIDMAESTRO_URL: &str =
    "https://github.com/hifihedgehog/HIDMaestro/releases/download/v1.6.1/HIDMaestro-v1.6.1.zip";
const HIDMAESTRO_SHA256: &str = "00145c23d9838be6089389ce58b3fd2b6766fa9bc0f1f3c60a3c885361b53c34";
const MAX_ARCHIVE_BYTES: u64 = 160 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 200 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 256;
const SIDECAR_EXE: &str = "Sunshine.Ds5Sidecar.exe";
static COMPONENT_OPERATION: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Serialize, Clone)]
pub struct DualSenseStatus {
    pub state: String,
    pub installed: bool,
    pub verified: bool,
    pub enabled: bool,
    pub audio_haptics: bool,
    pub component_version: String,
    pub runtime_version: String,
    pub install_path: String,
    pub sidecar_path: String,
    pub driver_installed: bool,
    pub usbip_available: bool,
    pub standard_profile: bool,
    pub composite_profile: bool,
    pub in_use: bool,
    pub error_code: String,
    pub detail: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ProbeResult {
    protocol: u32,
    runtime_version: String,
    standard: bool,
    composite: bool,
    driver_installed: bool,
    usbip_available: bool,
}

fn emit_progress(app: &tauri::AppHandle, stage: &str, progress: u32) {
    let _ = app.emit(
        "dualsense-operation-progress",
        serde_json::json!({ "stage": stage, "progress": progress.min(100) }),
    );
}

fn component_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Sunshine")
        .join("components")
        .join("dualsense")
}

fn active_dir() -> PathBuf {
    component_root().join("active")
}

fn sidecar_path() -> PathBuf {
    active_dir().join(SIDECAR_EXE)
}

fn config_path() -> PathBuf {
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("config")
        .join("sunshine.conf")
}

fn read_config_value(key: &str) -> Option<String> {
    let content = fs::read_to_string(config_path()).ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            return None;
        }
        let (candidate, value) = line.split_once('=')?;
        (candidate.trim() == key).then(|| value.trim().to_string())
    })
}

fn config_bool(key: &str, default_value: bool) -> bool {
    read_config_value(key)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "yes" | "1" | "enabled"
            )
        })
        .unwrap_or(default_value)
}

fn run_probe(executable: &Path) -> Result<ProbeResult, String> {
    if !executable.is_file() {
        return Err("DS5-PKG-003: Sunshine DualSense sidecar is missing".to_string());
    }
    let output = Command::new(executable)
        .arg("--probe")
        .current_dir(executable.parent().unwrap_or_else(|| Path::new(".")))
        .output()
        .map_err(|error| format!("DS5-PKG-003: unable to start sidecar probe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "DS5-PKG-003: sidecar probe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let result: ProbeResult = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("DS5-PKG-003: invalid sidecar probe response: {error}"))?;
    if result.protocol != PROTOCOL_VERSION {
        return Err(format!(
            "DS5-PROTO-001: expected protocol {PROTOCOL_VERSION}, got {}",
            result.protocol
        ));
    }
    if !result.standard || !result.composite {
        return Err("DS5-PKG-003: required DualSense profiles are unavailable".to_string());
    }
    Ok(result)
}

async fn has_active_session() -> bool {
    crate::sunshine::get_active_sessions()
        .await
        .map(|sessions| !sessions.is_empty())
        .unwrap_or(false)
}

async fn ensure_no_active_session() -> Result<(), String> {
    let sessions = crate::sunshine::get_active_sessions()
        .await
        .map_err(|error| format!("DS5-RUN-002: unable to verify stream ownership: {error}"))?;
    if sessions.is_empty() {
        Ok(())
    } else {
        Err(
            "DS5-RUN-002: finish the active Sunshine stream before changing the component"
                .to_string(),
        )
    }
}

fn component_state(
    installed: bool,
    verified: bool,
    driver_installed: bool,
    in_use: bool,
) -> &'static str {
    if in_use && installed {
        "in_use"
    } else if !installed {
        "not_installed"
    } else if !verified {
        "repair_required"
    } else if !driver_installed {
        "transport_missing"
    } else {
        "ready"
    }
}

#[tauri::command]
pub async fn dualsense_get_status() -> Result<DualSenseStatus, String> {
    let executable = sidecar_path();
    let installed = executable.is_file();
    let in_use = has_active_session().await;
    let enabled = config_bool("ds5_enabled", false);
    let audio_haptics = config_bool("ds5_audio_haptics", true);
    let probe = if installed {
        let probe_executable = executable.clone();
        Some(
            tokio::task::spawn_blocking(move || run_probe(&probe_executable))
                .await
                .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))
                .and_then(|result| result),
        )
    } else {
        None
    };
    let (verified, result, error_code, detail) = match probe {
        Some(Ok(result)) => (true, result, String::new(), String::new()),
        Some(Err(detail)) => {
            let code = detail
                .split(':')
                .next()
                .unwrap_or("DS5-PKG-003")
                .to_string();
            (false, ProbeResult::default(), code, detail)
        }
        None => (false, ProbeResult::default(), String::new(), String::new()),
    };
    let state = component_state(installed, verified, result.driver_installed, in_use);

    Ok(DualSenseStatus {
        state: state.to_string(),
        installed,
        verified,
        enabled,
        audio_haptics,
        component_version: installed
            .then_some(COMPONENT_VERSION)
            .unwrap_or_default()
            .to_string(),
        runtime_version: result.runtime_version,
        install_path: active_dir().to_string_lossy().to_string(),
        sidecar_path: executable.to_string_lossy().to_string(),
        driver_installed: result.driver_installed,
        usbip_available: result.usbip_available,
        standard_profile: result.standard,
        composite_profile: result.composite,
        in_use,
        error_code,
        detail,
    })
}

fn sidecar_source_dir() -> Result<PathBuf, String> {
    if let Some(override_dir) = std::env::var_os("SUNSHINE_DS5_SIDECAR_DIR") {
        let path = PathBuf::from(override_dir);
        if path.join(SIDECAR_EXE).is_file() {
            return Ok(path);
        }
    }

    let sunshine_root = PathBuf::from(crate::sunshine::get_sunshine_install_path());
    let candidates = [
        sunshine_root.join("tools").join("sunshine-ds5-sidecar"),
        sunshine_root
            .join("tools")
            .join("sunshine-ds5-sidecar")
            .join("bin")
            .join("Release")
            .join("net10.0-windows10.0.26100.0"),
    ];
    candidates
        .into_iter()
        .find(|path| path.join(SIDECAR_EXE).is_file())
        .ok_or_else(|| {
            "DS5-PKG-002: this Sunshine build does not contain the DualSense sidecar runtime"
                .to_string()
        })
}

fn copy_runtime_files(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("HIDMaestro.Core.dll")
            {
                continue;
            }
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .map_err(|error| format!("DS5-PKG-002: unable to copy sidecar runtime: {error}"))?;
        }
    }
    Ok(())
}

fn extract_verified_package(archive_path: &Path, staging: &Path) -> Result<(), String> {
    let mut archive_file = File::open(archive_path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = archive_file
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual = format!("{:x}", digest.finalize());
    if actual != HIDMAESTRO_SHA256 {
        return Err(format!(
            "DS5-PKG-001: expected {HIDMAESTRO_SHA256}, got {actual}"
        ));
    }

    let mut archive =
        zip::ZipArchive::new(File::open(archive_path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("DS5-PKG-002: invalid release archive: {error}"))?;
    if archive.len() > MAX_ARCHIVE_FILES {
        return Err("DS5-PKG-002: release archive contains too many files".to_string());
    }
    let mut extracted_bytes = 0u64;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index).map_err(|error| error.to_string())?;
        extracted_bytes = extracted_bytes.saturating_add(item.size());
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err("DS5-PKG-002: release archive exceeds the extraction limit".to_string());
        }
        let Some(relative) = item.enclosed_name() else {
            return Err("DS5-PKG-002: release archive contains an unsafe path".to_string());
        };
        // Runtime integration needs only the root assembly and compliance files.
        if relative.components().count() != 1 || item.is_dir() {
            continue;
        }
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !matches!(
            name,
            "HIDMaestro.Core.dll" | "LICENSE" | "README.md" | "THIRD-PARTY-NOTICES.txt"
        ) {
            continue;
        }
        let mut output = File::create(staging.join(name)).map_err(|error| error.to_string())?;
        std::io::copy(&mut item, &mut output).map_err(|error| error.to_string())?;
    }
    if !staging.join("HIDMaestro.Core.dll").is_file() || !staging.join("LICENSE").is_file() {
        return Err("DS5-PKG-002: required runtime or license file is missing".to_string());
    }
    Ok(())
}

async fn apply_config(
    enabled: bool,
    audio_haptics: bool,
    executable: Option<&Path>,
) -> Result<(), String> {
    let mut config = crate::vdd::read_full_sunshine_config()
        .await
        .unwrap_or_default();
    config.insert("ds5_enabled".to_string(), serde_json::json!(enabled));
    config.insert(
        "ds5_audio_haptics".to_string(),
        serde_json::json!(audio_haptics),
    );
    config.insert(
        "ds5_sidecar_path".to_string(),
        serde_json::json!(
            executable
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default()
        ),
    );
    crate::sunshine::post_sunshine_config(&config).await
}

#[tauri::command]
pub async fn dualsense_install(app: tauri::AppHandle) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    let previous_enabled = config_bool("ds5_enabled", false);
    let previous_audio_haptics = config_bool("ds5_audio_haptics", true);
    emit_progress(&app, "preparing", 1);
    let source = sidecar_source_dir()?;
    let root = component_root();
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let operation = format!("staging-{}", std::process::id());
    let staging = root.join(&operation);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let archive_path = root.join(format!("{operation}.partial"));

    let install_result: Result<(), String> = async {
        let response = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(600))
            .user_agent("foundation-sunshine-dualsense-component")
            .build()
            .map_err(|error| error.to_string())?
            .get(HIDMAESTRO_URL)
            .send()
            .await
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?;
        emit_progress(&app, "downloading", 3);
        let total_size = response.content_length();
        if total_size.is_some_and(|size| size > MAX_ARCHIVE_BYTES) {
            return Err("DS5-PKG-002: release archive exceeds the download limit".to_string());
        }
        let mut output = File::create(&archive_path).map_err(|error| error.to_string())?;
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > MAX_ARCHIVE_BYTES {
                return Err("DS5-PKG-002: release archive exceeds the download limit".to_string());
            }
            output
                .write_all(&chunk)
                .map_err(|error| error.to_string())?;
            if let Some(total) = total_size.filter(|total| *total != 0) {
                let download_progress = (downloaded * 70 / total).min(70) as u32;
                emit_progress(&app, "downloading", 3 + download_progress);
            }
        }
        drop(output);

        emit_progress(&app, "verifying", 76);
        let archive = archive_path.clone();
        let destination = staging.clone();
        tokio::task::spawn_blocking(move || extract_verified_package(&archive, &destination))
            .await
            .map_err(|error| error.to_string())??;
        copy_runtime_files(&source, &staging)?;
        emit_progress(&app, "probing", 88);
        run_probe(&staging.join(SIDECAR_EXE))?;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "component_version": COMPONENT_VERSION,
                "hidmaestro_version": HIDMAESTRO_VERSION,
                "source": HIDMAESTRO_URL,
                "sha256": HIDMAESTRO_SHA256,
                "protocol": PROTOCOL_VERSION
            }))
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        let active = active_dir();
        let backup = root.join("previous");
        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|error| error.to_string())?;
        }
        if active.exists() {
            fs::rename(&active, &backup).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&staging, &active) {
            if backup.exists() {
                let _ = fs::rename(&backup, &active);
            }
            return Err(format!(
                "DS5-PKG-002: unable to activate component: {error}"
            ));
        }
        emit_progress(&app, "activating", 96);
        Ok(())
    }
    .await;
    let _ = fs::remove_file(&archive_path);
    if install_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    install_result?;
    apply_config(
        previous_enabled,
        previous_audio_haptics,
        Some(&sidecar_path()),
    )
    .await?;
    emit_progress(&app, "complete", 100);
    dualsense_get_status().await
}

#[tauri::command]
pub async fn dualsense_set_config(
    enabled: bool,
    audio_haptics: bool,
) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    if enabled {
        run_probe(&sidecar_path())?;
    }
    apply_config(enabled, audio_haptics, Some(&sidecar_path())).await?;
    dualsense_get_status().await
}

#[tauri::command]
pub async fn dualsense_self_test(profile: String) -> Result<serde_json::Value, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    if profile != "standard" && profile != "composite" {
        return Err("DS5-PKG-003: invalid self-test profile".to_string());
    }
    let result_path = component_root().join(format!("self-test-{}.json", std::process::id()));
    #[cfg(target_os = "windows")]
    let status = {
        let wrapper_path = component_root().join(format!("self-test-{}.bat", std::process::id()));
        let executable = sidecar_path();
        fs::write(
            &wrapper_path,
            format!(
                "@echo off\r\n\"{}\" --self-test \"{}\" --result \"{}\"\r\nexit /b %ERRORLEVEL%\r\n",
                executable.display(),
                profile,
                result_path.display()
            ),
        )
        .map_err(|error| format!("DS5-PKG-003: unable to prepare component test: {error}"))?;
        let outcome = crate::bat_runner::run_elevated(&wrapper_path, "ds5-self-test", &[]);
        let _ = fs::remove_file(&wrapper_path);
        outcome?;
        true
    };
    #[cfg(not(target_os = "windows"))]
    let status = false;
    let result = fs::read_to_string(&result_path).unwrap_or_default();
    let _ = fs::remove_file(&result_path);
    let json =
        serde_json::from_str(&result).unwrap_or_else(|_| serde_json::json!({ "detail": result }));
    if !status {
        return Err(format!("DS5-PKG-003: component test failed: {json}"));
    }
    Ok(json)
}

#[tauri::command]
pub async fn dualsense_uninstall() -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    apply_config(false, true, None).await?;
    let root = component_root();
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("DS5-PKG-002: unable to remove component: {error}"))?;
    }
    dualsense_get_status().await
}

#[cfg(test)]
mod tests {
    use super::component_state;

    #[test]
    fn component_state_prioritizes_stream_ownership_and_recovery() {
        assert_eq!(component_state(true, true, true, true), "in_use");
        assert_eq!(component_state(false, false, false, true), "not_installed");
        assert_eq!(component_state(true, false, true, false), "repair_required");
        assert_eq!(
            component_state(true, true, false, false),
            "transport_missing"
        );
        assert_eq!(component_state(true, true, true, false), "ready");
    }
}
