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
const USBIP_VERSION: &str = "0.9.7.7";
const USBIP_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
const USBIP_SHA256: &str = "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";
const MAX_ARCHIVE_BYTES: u64 = 160 * 1024 * 1024;
const MAX_USBIP_INSTALLER_BYTES: u64 = 48 * 1024 * 1024;
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
    pub usbip_version: String,
    pub standard_profile: bool,
    pub composite_profile: bool,
    pub in_use: bool,
    pub error_code: String,
    pub detail: String,
}

#[cfg(target_os = "windows")]
fn installed_usbip_version() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    for view in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        let Ok(root) = hklm.open_subkey_with_flags(uninstall, view) else {
            continue;
        };
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, view) else {
                continue;
            };
            let display_name = key
                .get_value::<String, _>("DisplayName")
                .unwrap_or_default();
            if display_name.starts_with("USBip version ") {
                if let Ok(version) = key.get_value::<String, _>("DisplayVersion") {
                    return Some(version);
                }
                return display_name
                    .strip_prefix("USBip version ")
                    .map(str::to_string);
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn installed_usbip_version() -> Option<String> {
    None
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
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("tools")
        .join("sunshine-ds5-component")
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

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut input = File::open(path)
        .map_err(|error| format!("DS5-PKG-002: unable to open component file: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("DS5-PKG-002: unable to hash component file: {error}"))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn run_with_timeout(
    command: &mut Command,
    timeout: std::time::Duration,
    timeout_error: &str,
) -> Result<std::process::Output, String> {
    const MAX_CAPTURED_OUTPUT: usize = 1024 * 1024;

    fn drain_pipe<R: Read + Send + 'static>(mut pipe: R) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut captured = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let Ok(count) = pipe.read(&mut buffer) else {
                    break;
                };
                if count == 0 {
                    break;
                }
                let remaining = MAX_CAPTURED_OUTPUT.saturating_sub(captured.len());
                captured.extend_from_slice(&buffer[..count.min(remaining)]);
            }
            captured
        })
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("DS5-PKG-003: unable to start sidecar: {error}"))?;
    let stdout_reader = child
        .stdout
        .take()
        .map(drain_pipe)
        .ok_or_else(|| "DS5-PKG-003: unable to capture sidecar stdout".to_string())?;
    let stderr_reader = child
        .stderr
        .take()
        .map(drain_pipe)
        .ok_or_else(|| "DS5-PKG-003: unable to capture sidecar stderr".to_string())?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("DS5-PKG-003: unable to wait for sidecar: {error}"))?
        {
            return Ok(std::process::Output {
                status,
                stdout: stdout_reader.join().unwrap_or_default(),
                stderr: stderr_reader.join().unwrap_or_default(),
            });
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(timeout_error.to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn apply_gamepad_selection(config: &mut serde_json::Map<String, serde_json::Value>, enabled: bool) {
    if enabled {
        config.insert("gamepad".to_string(), serde_json::json!("ds5"));
    } else if config
        .get("gamepad")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|gamepad| gamepad.eq_ignore_ascii_case("ds5"))
    {
        config.insert("gamepad".to_string(), serde_json::json!("auto"));
    }
}

fn run_probe(executable: &Path) -> Result<ProbeResult, String> {
    if !executable.is_file() {
        return Err("DS5-PKG-003: Sunshine DualSense sidecar is missing".to_string());
    }
    let mut command = Command::new(executable);
    command
        .arg("--probe")
        .current_dir(executable.parent().unwrap_or_else(|| Path::new(".")));
    let output = run_with_timeout(
        &mut command,
        std::time::Duration::from_secs(15),
        "DS5-PKG-003: sidecar probe timed out",
    )?;
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

fn component_test_failure(output: &std::process::Output, result_path: &Path) -> String {
    if let Ok(contents) = fs::read_to_string(result_path) {
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let Some(error) = result.get("error").and_then(serde_json::Value::as_str) {
                if let Some(summary) = error.lines().find(|line| !line.trim().is_empty()) {
                    return format!("DS5-PKG-003: component test failed: {}", summary.trim());
                }
            }
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        return format!("DS5-PKG-003: component test failed: {}", stderr.trim());
    }

    format!(
        "DS5-PKG-003: component test process failed with exit code {}",
        output.status.code().unwrap_or(-1)
    )
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
    transport_available: bool,
    in_use: bool,
) -> &'static str {
    if in_use && installed {
        "in_use"
    } else if !installed {
        "not_installed"
    } else if !verified {
        "repair_required"
    } else if !transport_available {
        "transport_missing"
    } else {
        "ready"
    }
}

fn validate_requested_profile(
    enabled: bool,
    audio_haptics: bool,
    usbip_available: bool,
) -> Result<(), String> {
    if enabled && audio_haptics && !usbip_available {
        Err(
            "DS5-RUN-003: four-channel haptics requires the USB/IP transport; disable audio haptics or repair the transport"
                .to_string(),
        )
    } else {
        Ok(())
    }
}

fn pinned_usbip_installed(installed_version: Option<&str>) -> bool {
    installed_version == Some(USBIP_VERSION)
}

#[tauri::command]
pub async fn dualsense_get_status() -> Result<DualSenseStatus, String> {
    let executable = sidecar_path();
    let installed = executable.is_file();
    let in_use = has_active_session().await;
    let enabled = config_bool("ds5_enabled", false)
        && read_config_value("gamepad").is_some_and(|gamepad| gamepad.eq_ignore_ascii_case("ds5"));
    let audio_haptics = config_bool("ds5_audio_haptics", false);
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
    let usbip_version = installed_usbip_version().unwrap_or_default();
    let usbip_available =
        result.usbip_available && pinned_usbip_installed(Some(usbip_version.as_str()));
    let state = component_state(installed, verified, usbip_available, in_use);

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
        usbip_available,
        usbip_version,
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

#[cfg(target_os = "windows")]
async fn ensure_pinned_usbip(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    component_root: &Path,
) -> Result<(), String> {
    if installed_usbip_version().as_deref() == Some(USBIP_VERSION) {
        return Ok(());
    }

    emit_progress(app, "transport_downloading", 3);
    let installer_path = component_root.join(format!("USBip-{USBIP_VERSION}-x64.partial.exe"));
    let download_result: Result<(), String> = async {
        let response = client
            .get(USBIP_URL)
            .send()
            .await
            .map_err(|error| format!("DS5-DRV-001: USB/IP download failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("DS5-DRV-001: USB/IP download failed: {error}"))?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_USBIP_INSTALLER_BYTES)
        {
            return Err("DS5-DRV-001: USB/IP installer exceeds the download limit".to_string());
        }

        let mut output = File::create(&installer_path).map_err(|error| error.to_string())?;
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|error| format!("DS5-DRV-001: USB/IP download failed: {error}"))?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > MAX_USBIP_INSTALLER_BYTES {
                return Err(
                    "DS5-DRV-001: USB/IP installer exceeds the download limit".to_string(),
                );
            }
            output
                .write_all(&chunk)
                .map_err(|error| error.to_string())?;
        }
        drop(output);

        let actual = sha256_file(&installer_path)?;
        if actual != USBIP_SHA256 {
            return Err(format!(
                "DS5-DRV-001: expected USB/IP SHA-256 {USBIP_SHA256}, got {actual}"
            ));
        }

        emit_progress(app, "transport_installing", 10);
        let executable = installer_path.clone();
        let output = tokio::task::spawn_blocking(move || {
            let mut command = Command::new(executable);
            command.args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/RESTARTEXITCODE=3010",
                "/SP-",
            ]);
            run_with_timeout(
                &mut command,
                std::time::Duration::from_secs(600),
                "DS5-DRV-001: USB/IP installer timed out",
            )
        })
        .await
        .map_err(|error| format!("DS5-DRV-001: USB/IP installer task failed: {error}"))?
        .map_err(|error| {
            if error.starts_with("DS5-DRV-") {
                error
            } else {
                format!("DS5-DRV-001: USB/IP installer failed: {error}")
            }
        })?;
        match output.status.code() {
            Some(0) => {}
            Some(3010) => {
                return Err(
                    "DS5-DRV-003: USB/IP 0.9.7.7 was installed and Windows must restart"
                        .to_string(),
                );
            }
            code => {
                return Err(format!(
                    "DS5-DRV-001: USB/IP installer failed with exit code {}",
                    code.unwrap_or(-1)
                ));
            }
        }
        if installed_usbip_version().as_deref() != Some(USBIP_VERSION) {
            return Err(format!(
                "DS5-DRV-001: USB/IP installer completed but version {USBIP_VERSION} is not registered"
            ));
        }
        Ok(())
    }
    .await;
    let _ = fs::remove_file(&installer_path);
    download_result
}

#[cfg(not(target_os = "windows"))]
async fn ensure_pinned_usbip(
    _app: &tauri::AppHandle,
    _client: &reqwest::Client,
    _component_root: &Path,
) -> Result<(), String> {
    Ok(())
}

async fn apply_config(
    enabled: bool,
    audio_haptics: bool,
    executable: Option<&Path>,
    sync_gamepad_selection: bool,
) -> Result<(), String> {
    if !config_path().is_file() {
        return Err(
            "DS5-CFG-001: Sunshine configuration file is missing; refusing to create a partial replacement"
                .to_string(),
        );
    }
    let mut config = crate::vdd::read_full_sunshine_config()
        .await
        .map_err(|error| {
            format!("DS5-CFG-001: unable to read the complete Sunshine configuration: {error}")
        })?;
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
    if sync_gamepad_selection {
        apply_gamepad_selection(&mut config, enabled);
    }
    crate::sunshine::post_sunshine_config(&config).await?;
    let restart = crate::sunshine::post_tray_restart_action()
        .await
        .map_err(|error| {
            format!(
                "DS5-CFG-002: configuration was saved, but Sunshine could not be restarted: {error}"
            )
        })?;
    if let Some(response) = restart.filter(|response| !response.status) {
        let error = if !response.error.trim().is_empty() {
            response.error
        } else if !response.message.trim().is_empty() {
            response.message
        } else {
            "Sunshine rejected the restart request".to_string()
        };
        return Err(format!(
            "DS5-CFG-002: configuration was saved, but Sunshine could not be restarted: {error}"
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn dualsense_install(app: tauri::AppHandle) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    if !crate::bat_runner::is_elevated() {
        return Err(
            "DS5-PKG-004: restart Sunshine Control Panel as administrator to install the protected component"
                .to_string(),
        );
    }
    let previous_enabled = config_bool("ds5_enabled", false);
    let previous_audio_haptics = config_bool("ds5_audio_haptics", false);
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
    let active = active_dir();
    let backup = root.join("previous");
    let had_previous = active.exists();
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("foundation-sunshine-dualsense-component")
        .build()
        .map_err(|error| error.to_string())?;

    let install_result: Result<(), String> = async {
        ensure_pinned_usbip(&app, &client, &root).await?;
        let response = client
            .get(HIDMAESTRO_URL)
            .send()
            .await
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?;
        emit_progress(&app, "downloading", 12);
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
                let download_progress = (downloaded * 60 / total).min(60) as u32;
                emit_progress(&app, "downloading", 12 + download_progress);
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
        let probe_executable = staging.join(SIDECAR_EXE);
        let probe_path = probe_executable.clone();
        tokio::task::spawn_blocking(move || run_probe(&probe_path))
            .await
            .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))??;
        let sidecar_sha256 = sha256_file(&probe_executable)?;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "component_version": COMPONENT_VERSION,
                "hidmaestro_version": HIDMAESTRO_VERSION,
                "source": HIDMAESTRO_URL,
                "sha256": HIDMAESTRO_SHA256,
                "protocol": PROTOCOL_VERSION,
                "sidecar_file": SIDECAR_EXE,
                "sidecar_sha256": sidecar_sha256
            }))
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

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
    if let Err(config_error) = apply_config(
        previous_enabled,
        previous_audio_haptics,
        Some(&sidecar_path()),
        false,
    )
    .await
    {
        // The component and configuration are already valid when only the
        // runtime restart failed. Keep them so a manual restart can finish the
        // activation instead of rolling back to a missing or older component.
        if config_error.starts_with("DS5-CFG-002:") {
            return Err(config_error);
        }
        let rollback_result = (|| -> Result<(), String> {
            if active.exists() {
                fs::remove_dir_all(&active).map_err(|error| {
                    format!("unable to remove failed active component: {error}")
                })?;
            }
            if had_previous {
                if !backup.exists() {
                    return Err("previous component backup is missing".to_string());
                }
                fs::rename(&backup, &active)
                    .map_err(|error| format!("unable to restore previous component: {error}"))?;
            }
            Ok(())
        })();
        return match rollback_result {
            Ok(()) => Err(format!(
                "{config_error}; activated component was rolled back"
            )),
            Err(rollback_error) => Err(format!(
                "{config_error}; component rollback also failed: {rollback_error}"
            )),
        };
    }
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
        let executable = sidecar_path();
        let probe = tokio::task::spawn_blocking(move || run_probe(&executable))
            .await
            .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))??;
        let usbip_version = installed_usbip_version();
        let usbip_available =
            probe.usbip_available && pinned_usbip_installed(usbip_version.as_deref());
        validate_requested_profile(enabled, audio_haptics, usbip_available)?;
    }
    apply_config(enabled, audio_haptics, Some(&sidecar_path()), true).await?;
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
    if !crate::bat_runner::is_elevated() {
        return Err(
            "DS5-PKG-004: restart Sunshine Control Panel as administrator to test the protected component"
                .to_string(),
        );
    }
    let result_path = component_root().join(format!("self-test-{}.json", std::process::id()));
    #[cfg(target_os = "windows")]
    {
        let executable = sidecar_path();
        let test_profile = profile.clone();
        let test_result = result_path.clone();
        tokio::task::spawn_blocking(move || {
            let _ = fs::remove_file(&test_result);
            let mut command = Command::new(&executable);
            command
                .args(["--self-test", &test_profile, "--result"])
                .arg(&test_result)
                .current_dir(executable.parent().unwrap_or_else(|| Path::new(".")));
            let outcome = run_with_timeout(
                &mut command,
                std::time::Duration::from_secs(60),
                "DS5-PKG-003: component self-test timed out",
            )
            .and_then(|output| {
                output
                    .status
                    .success()
                    .then_some(())
                    .ok_or_else(|| component_test_failure(&output, &test_result))
            });
            if outcome.is_err() {
                let _ = fs::remove_file(&test_result);
            }
            outcome
        })
        .await
        .map_err(|error| format!("DS5-PKG-003: component test task failed: {error}"))??;
    };
    #[cfg(not(target_os = "windows"))]
    return Err("DS5-PKG-003: component tests are only supported on Windows".to_string());
    let result = fs::read_to_string(&result_path);
    let _ = fs::remove_file(&result_path);
    let result = result
        .map_err(|error| format!("DS5-PKG-003: component test produced no result: {error}"))?;
    let json: serde_json::Value = serde_json::from_str(&result)
        .map_err(|error| format!("DS5-PKG-003: invalid component test result: {error}"))?;
    Ok(json)
}

#[tauri::command]
pub async fn dualsense_uninstall() -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    if !crate::bat_runner::is_elevated() {
        return Err(
            "DS5-PKG-004: restart Sunshine Control Panel as administrator to uninstall the protected component"
                .to_string(),
        );
    }
    apply_config(false, true, None, true).await?;
    let root = component_root();
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("DS5-PKG-002: unable to remove component: {error}"))?;
    }
    dualsense_get_status().await
}

#[cfg(test)]
mod tests {
    use super::{
        apply_gamepad_selection, component_state, component_test_failure, pinned_usbip_installed,
        validate_requested_profile,
    };
    use std::process::Command;

    #[test]
    fn composite_profile_requires_usbip() {
        let error = validate_requested_profile(true, true, false).unwrap_err();
        assert!(error.starts_with("DS5-RUN-003:"));
    }

    #[test]
    fn hid_only_profile_remains_available_without_usbip() {
        assert!(validate_requested_profile(true, false, false).is_ok());
        assert!(validate_requested_profile(false, true, false).is_ok());
    }

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

    #[test]
    fn enabling_dualsense_selects_the_ds5_gamepad_backend() {
        let mut config = serde_json::Map::new();
        config.insert("gamepad".to_string(), serde_json::json!("auto"));

        apply_gamepad_selection(&mut config, true);

        assert_eq!(config.get("gamepad"), Some(&serde_json::json!("ds5")));
    }

    #[test]
    fn disabling_dualsense_restores_auto_only_when_ds5_is_selected() {
        let mut ds5_config = serde_json::Map::new();
        ds5_config.insert("gamepad".to_string(), serde_json::json!("ds5"));
        apply_gamepad_selection(&mut ds5_config, false);
        assert_eq!(ds5_config.get("gamepad"), Some(&serde_json::json!("auto")));

        let mut explicit_config = serde_json::Map::new();
        explicit_config.insert("gamepad".to_string(), serde_json::json!("x360"));
        apply_gamepad_selection(&mut explicit_config, false);
        assert_eq!(
            explicit_config.get("gamepad"),
            Some(&serde_json::json!("x360"))
        );
    }

    #[test]
    fn usbip_requires_the_exact_pinned_version() {
        assert!(pinned_usbip_installed(Some("0.9.7.7")));
        assert!(!pinned_usbip_installed(Some("0.9.7.3")));
        assert!(!pinned_usbip_installed(None));
    }

    #[test]
    fn component_test_failure_preserves_sidecar_diagnostic() {
        let result_path = std::env::temp_dir().join(format!(
            "sunshine-ds5-test-failure-{}.json",
            std::process::id()
        ));
        std::fs::write(&result_path, r#"{"error":"usbip-win2 attach failed"}"#).unwrap();
        let output = if cfg!(windows) {
            Command::new("cmd")
                .args(["/c", "exit", "1"])
                .output()
                .unwrap()
        } else {
            Command::new("sh").args(["-c", "exit 1"]).output().unwrap()
        };

        let error = component_test_failure(&output, &result_path);
        let _ = std::fs::remove_file(result_path);

        assert_eq!(
            error,
            "DS5-PKG-003: component test failed: usbip-win2 attach failed"
        );
    }
}
