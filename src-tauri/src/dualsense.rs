//! Optional DualSense component lifecycle.
//!
//! HIDMaestro is downloaded from its pinned upstream release and verified before
//! extraction. The Sunshine-owned sidecar is only copied from the installed
//! Sunshine package (or an explicit development override), keeping third-party
//! runtime and first-party process ownership separate.

use futures_util::StreamExt;
use log::warn;
use once_cell::sync::{Lazy, OnceCell};
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
const CONFIG_APPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(target_os = "windows")]
const ELEVATED_DS5_ARG: &str = "--elevated-dualsense";
#[cfg(target_os = "windows")]
const MAX_ELEVATED_MESSAGE_BYTES: usize = 64 * 1024;
#[cfg(target_os = "windows")]
const ELEVATION_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
static COMPONENT_OPERATION: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));
#[cfg(target_os = "windows")]
static ELEVATED_HELPER_JOB: OnceCell<std::os::windows::io::OwnedHandle> = OnceCell::new();

type ProgressReporter<'a> = dyn Fn(&str, u32) + Send + Sync + 'a;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum ElevatedOperation {
    Install,
    TestStandard,
    TestComposite,
    Uninstall,
}

#[cfg(target_os = "windows")]
impl ElevatedOperation {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::TestStandard => "test-standard",
            Self::TestComposite => "test-composite",
            Self::Uninstall => "uninstall",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "install" => Some(Self::Install),
            "test-standard" => Some(Self::TestStandard),
            "test-composite" => Some(Self::TestComposite),
            "uninstall" => Some(Self::Uninstall),
            _ => None,
        }
    }

    fn timeout(self) -> std::time::Duration {
        match self {
            // Installation can include two downloads and a driver installer,
            // each with its own ten-minute timeout.
            Self::Install => std::time::Duration::from_secs(35 * 60),
            Self::TestStandard | Self::TestComposite => std::time::Duration::from_secs(90),
            Self::Uninstall => std::time::Duration::from_secs(120),
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ElevatedMessage {
    Progress { stage: String, progress: u32 },
    Complete { data: serde_json::Value },
    Error { message: String },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DualSenseStatus {
    pub state: String,
    pub installed: bool,
    pub verified: bool,
    pub enabled: bool,
    pub audio_haptics: bool,
    pub legacy_strength: f64,
    pub legacy_curve: f64,
    pub legacy_noise_gate: f64,
    pub config_revision: u64,
    pub component_version: String,
    pub runtime_version: String,
    pub install_path: String,
    pub sidecar_path: String,
    pub driver_installed: bool,
    pub usbip_available: bool,
    pub usbip_version: String,
    pub usbip_version_valid: bool,
    pub reboot_recommended: bool,
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct CoreDualSenseSettings {
    ds5_enabled: bool,
    ds5_audio_haptics: bool,
    ds5_legacy_haptics_strength: f64,
    ds5_legacy_haptics_curve: f64,
    ds5_legacy_haptics_noise_gate: f64,
}

impl Default for CoreDualSenseSettings {
    fn default() -> Self {
        Self {
            ds5_enabled: false,
            ds5_audio_haptics: true,
            ds5_legacy_haptics_strength: 1.0,
            ds5_legacy_haptics_curve: 1.0,
            ds5_legacy_haptics_noise_gate: 0.020,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CoreDualSenseResponse {
    status: bool,
    applied: bool,
    revision: u64,
    #[serde(default)]
    changed: Option<bool>,
    #[serde(flatten)]
    settings: CoreDualSenseSettings,
}

#[derive(Debug, Serialize)]
pub struct DualSenseTuningResult {
    legacy_strength: f64,
    legacy_curve: f64,
    legacy_noise_gate: f64,
    revision: u64,
    changed: bool,
}

fn validate_core_ds5_response(
    result: CoreDualSenseResponse,
) -> Result<CoreDualSenseResponse, String> {
    if !result.status || !result.applied || result.revision == 0 {
        return Err(
            "DS5-CFG-001: Sunshine rejected the DualSense configuration request".to_string(),
        );
    }
    let values = result.settings;
    if clamp_tuning(
        values.ds5_legacy_haptics_strength,
        values.ds5_legacy_haptics_curve,
        values.ds5_legacy_haptics_noise_gate,
    ) != Some((
        values.ds5_legacy_haptics_strength,
        values.ds5_legacy_haptics_curve,
        values.ds5_legacy_haptics_noise_gate,
    )) {
        return Err("DS5-CFG-001: Sunshine returned invalid DualSense values".to_string());
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UsbipInstallResult {
    Ready,
    RebootRecommended,
}

fn emit_progress(app: &tauri::AppHandle, stage: &str, progress: u32) {
    let _ = app.emit(
        "dualsense-operation-progress",
        serde_json::json!({ "stage": stage, "progress": progress.min(100) }),
    );
}

fn report_progress(progress: &ProgressReporter<'_>, stage: &str, value: u32) {
    progress(stage, value.min(100));
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

async fn read_core_ds5_response(
    mut response: reqwest::Response,
) -> Result<CoreDualSenseResponse, String> {
    const MAX_RESPONSE_BYTES: usize = 64 * 1024;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err("DS5-CFG-001: DualSense configuration response is too large".to_string());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("DS5-CFG-001: unable to read DualSense configuration: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("DS5-CFG-001: DualSense configuration response is too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let error_code = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|body| body.get("error_code")?.as_str().map(str::to_owned));
        if error_code.as_deref() == Some("ds5_config_invalid") {
            return Err(
                "DS5-CFG-005: the independent DualSense configuration is invalid".to_string(),
            );
        }
        return Err(format!(
            "DS5-CFG-001: Sunshine rejected the DualSense configuration request (HTTP {})",
            status.as_u16()
        ));
    }
    let result: CoreDualSenseResponse = serde_json::from_slice(&bytes).map_err(|error| {
        format!("DS5-CFG-001: invalid DualSense configuration response: {error}")
    })?;
    validate_core_ds5_response(result)
}

async fn get_core_ds5_settings() -> Result<CoreDualSenseResponse, String> {
    let base_url = crate::sunshine::get_sunshine_url().await?;
    let response = crate::sunshine::create_https_client()?
        .get(format!(
            "{}/api/dualsense/config",
            base_url.trim_end_matches('/')
        ))
        .send()
        .await
        .map_err(|error| format!("DS5-CFG-001: unable to read DualSense configuration: {error}"))?;
    read_core_ds5_response(response).await
}

async fn save_core_ds5_settings(
    settings: CoreDualSenseSettings,
) -> Result<CoreDualSenseResponse, String> {
    let base_url = crate::sunshine::get_sunshine_url().await?;
    let response = crate::sunshine::create_https_client()?
        .post(format!(
            "{}/api/dualsense/config",
            base_url.trim_end_matches('/')
        ))
        .json(&settings)
        .send()
        .await
        .map_err(|error| format!("DS5-CFG-003: unable to save DualSense configuration: {error}"))?;
    read_core_ds5_response(response)
        .await
        .map_err(|error| error.replacen("DS5-CFG-001:", "DS5-CFG-003:", 1))
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
    capture_output: bool,
) -> Result<std::process::Output, String> {
    const MAX_CAPTURED_OUTPUT: usize = 1024 * 1024;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

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

    if capture_output {
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
    } else {
        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("DS5-PKG-003: unable to start sidecar: {error}"))?;
    let stdout_reader = child.stdout.take().map(drain_pipe);
    let stderr_reader = child.stderr.take().map(drain_pipe);
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("DS5-PKG-003: unable to wait for sidecar: {error}"))?
        {
            return Ok(std::process::Output {
                status,
                stdout: stdout_reader
                    .map(|reader| reader.join().unwrap_or_default())
                    .unwrap_or_default(),
                stderr: stderr_reader
                    .map(|reader| reader.join().unwrap_or_default())
                    .unwrap_or_default(),
            });
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            #[cfg(target_os = "windows")]
            if ELEVATED_HELPER_JOB.get().is_some() {
                // A descendant may still hold an inherited pipe handle. The
                // helper reports the timeout and exits, which closes its Job
                // handle and terminates the complete process tree.
                drop(stdout_reader);
                drop(stderr_reader);
                return Err(timeout_error.to_string());
            }
            if let Some(reader) = stdout_reader {
                let _ = reader.join();
            }
            if let Some(reader) = stderr_reader {
                let _ = reader.join();
            }
            return Err(timeout_error.to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(target_os = "windows")]
/// Bind the elevated helper and all children it launches to one lifetime job.
fn bind_elevated_helper_lifetime() -> Result<(), String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::core::PCWSTR;

    ELEVATED_HELPER_JOB
        .get_or_try_init(|| unsafe {
            let raw_job = CreateJobObjectW(None, PCWSTR::null()).map_err(|error| {
                format!("DS5-PKG-003: unable to create elevated helper job: {error}")
            })?;
            let job = std::os::windows::io::OwnedHandle::from_raw_handle(raw_job.0);
            let job_handle = HANDLE(job.as_raw_handle());
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job_handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
            .map_err(|error| {
                format!("DS5-PKG-003: unable to configure elevated helper job: {error}")
            })?;
            AssignProcessToJobObject(job_handle, GetCurrentProcess()).map_err(|error| {
                format!("DS5-PKG-003: unable to bind elevated helper lifetime: {error}")
            })?;
            Ok(job)
        })
        .map(|_| ())
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
        true,
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

async fn ensure_no_active_session_for_uninstall() -> Result<(), String> {
    match crate::sunshine::get_active_sessions().await {
        Ok(sessions) if sessions.is_empty() => Ok(()),
        Ok(_) => Err(
            "DS5-RUN-002: finish the active Sunshine stream before changing the component"
                .to_string(),
        ),
        Err(error) => {
            // An offline Core cannot own an active Sidecar session. If the API
            // alone is degraded, Windows file ownership still makes removal fail safely.
            warn!("DualSense uninstall could not verify active sessions: {error}");
            Ok(())
        }
    }
}

fn clamp_tuning(strength: f64, curve: f64, noise_gate: f64) -> Option<(f64, f64, f64)> {
    (strength.is_finite() && curve.is_finite() && noise_gate.is_finite()).then_some((
        strength.clamp(0.1, 4.0),
        curve.clamp(0.3, 2.0),
        noise_gate.clamp(0.002, 0.060),
    ))
}

fn local_uninstalled_status() -> DualSenseStatus {
    let usbip_version = installed_usbip_version().unwrap_or_default();
    DualSenseStatus {
        state: "not_installed".to_string(),
        installed: false,
        verified: false,
        enabled: false,
        audio_haptics: true,
        legacy_strength: 1.0,
        legacy_curve: 1.0,
        legacy_noise_gate: 0.020,
        config_revision: 0,
        component_version: String::new(),
        runtime_version: String::new(),
        install_path: active_dir().to_string_lossy().to_string(),
        sidecar_path: sidecar_path().to_string_lossy().to_string(),
        driver_installed: false,
        usbip_available: false,
        usbip_version_valid: pinned_usbip_installed(Some(usbip_version.as_str())),
        usbip_version,
        reboot_recommended: false,
        standard_profile: false,
        composite_profile: false,
        in_use: false,
        error_code: String::new(),
        detail: String::new(),
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

fn classify_usbip_installer_exit_code(
    exit_code: Option<i32>,
) -> Result<UsbipInstallResult, String> {
    match exit_code {
        Some(0) => Ok(UsbipInstallResult::Ready),
        Some(3010) => Ok(UsbipInstallResult::RebootRecommended),
        code => Err(format!(
            "DS5-DRV-001: USB/IP installer failed with exit code {}",
            code.unwrap_or(-1)
        )),
    }
}

#[tauri::command]
pub async fn dualsense_set_haptics_tuning(
    strength: f64,
    curve: f64,
    noise_gate: f64,
) -> Result<DualSenseTuningResult, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    let (strength, curve, noise_gate) = clamp_tuning(strength, curve, noise_gate)
        .ok_or_else(|| "DS5-CFG-001: DualSense tuning values must be finite".to_string())?;
    let mut settings = get_core_ds5_settings().await?.settings;
    settings.ds5_legacy_haptics_strength = strength;
    settings.ds5_legacy_haptics_curve = curve;
    settings.ds5_legacy_haptics_noise_gate = noise_gate;
    let applied = save_core_ds5_settings(settings).await?;
    Ok(DualSenseTuningResult {
        legacy_strength: applied.settings.ds5_legacy_haptics_strength,
        legacy_curve: applied.settings.ds5_legacy_haptics_curve,
        legacy_noise_gate: applied.settings.ds5_legacy_haptics_noise_gate,
        revision: applied.revision,
        changed: applied.changed.unwrap_or(false),
    })
}

#[tauri::command]
pub async fn dualsense_get_status() -> Result<DualSenseStatus, String> {
    let (settings, config_revision, config_error) = match get_core_ds5_settings().await {
        Ok(snapshot) => (snapshot.settings, snapshot.revision, String::new()),
        Err(error) => {
            warn!("DualSense status could not read the configuration: {error}");
            (CoreDualSenseSettings::default(), 0, error)
        }
    };
    let executable = sidecar_path();
    let installed = executable.is_file();
    let in_use = has_active_session().await;
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
    let (verified, result, mut error_code, mut detail) = match probe {
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
    if error_code.is_empty() && !config_error.is_empty() {
        error_code = config_error
            .split(':')
            .next()
            .unwrap_or("DS5-CFG-001")
            .to_string();
        detail = config_error;
    }
    let usbip_version = installed_usbip_version().unwrap_or_default();
    let usbip_version_valid = pinned_usbip_installed(Some(usbip_version.as_str()));
    let usbip_available = result.usbip_available && usbip_version_valid;
    let state = component_state(installed, verified, usbip_available, in_use);

    Ok(DualSenseStatus {
        state: state.to_string(),
        installed,
        verified,
        enabled: settings.ds5_enabled,
        audio_haptics: settings.ds5_audio_haptics,
        legacy_strength: settings.ds5_legacy_haptics_strength,
        legacy_curve: settings.ds5_legacy_haptics_curve,
        legacy_noise_gate: settings.ds5_legacy_haptics_noise_gate,
        config_revision,
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
        usbip_version_valid,
        reboot_recommended: false,
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
    progress: &ProgressReporter<'_>,
    client: &reqwest::Client,
    component_root: &Path,
) -> Result<UsbipInstallResult, String> {
    if installed_usbip_version().as_deref() == Some(USBIP_VERSION) {
        return Ok(UsbipInstallResult::Ready);
    }

    report_progress(progress, "transport_downloading", 3);
    let installer_path = component_root.join(format!("USBip-{USBIP_VERSION}-x64.partial.exe"));
    let download_result: Result<UsbipInstallResult, String> = async {
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

        report_progress(progress, "transport_installing", 10);
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
                false,
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
        let install_result = classify_usbip_installer_exit_code(output.status.code())?;
        if installed_usbip_version().as_deref() != Some(USBIP_VERSION) {
            return Err(format!(
                "DS5-DRV-001: USB/IP installer completed but version {USBIP_VERSION} is not registered"
            ));
        }
        Ok(install_result)
    }
    .await;
    let _ = fs::remove_file(&installer_path);
    download_result
}

#[cfg(not(target_os = "windows"))]
async fn ensure_pinned_usbip(
    _progress: &ProgressReporter<'_>,
    _client: &reqwest::Client,
    _component_root: &Path,
) -> Result<UsbipInstallResult, String> {
    Ok(UsbipInstallResult::Ready)
}

async fn dualsense_install_impl(
    progress: &ProgressReporter<'_>,
) -> Result<DualSenseStatus, String> {
    report_progress(progress, "preparing", 1);
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
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("foundation-sunshine-dualsense-component")
        .build()
        .map_err(|error| error.to_string())?;

    let install_result: Result<UsbipInstallResult, String> = async {
        let usbip_install_result = ensure_pinned_usbip(progress, &client, &root).await?;
        let response = client
            .get(HIDMAESTRO_URL)
            .send()
            .await
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?;
        report_progress(progress, "downloading", 12);
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
                report_progress(progress, "downloading", 12 + download_progress);
            }
        }
        drop(output);

        report_progress(progress, "verifying", 76);
        let archive = archive_path.clone();
        let destination = staging.clone();
        tokio::task::spawn_blocking(move || extract_verified_package(&archive, &destination))
            .await
            .map_err(|error| error.to_string())??;
        copy_runtime_files(&source, &staging)?;
        report_progress(progress, "probing", 88);
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
        report_progress(progress, "activating", 96);
        Ok(usbip_install_result)
    }
    .await;
    let _ = fs::remove_file(&archive_path);
    if install_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let reboot_recommended = matches!(install_result?, UsbipInstallResult::RebootRecommended);
    report_progress(progress, "complete", 100);
    let mut status = dualsense_get_status().await?;
    status.reboot_recommended = reboot_recommended;
    Ok(status)
}

#[cfg(target_os = "windows")]
fn elevated_pipe_name(token: uuid::Uuid) -> String {
    format!(r"\\.\pipe\sunshine-dualsense-{token}")
}

#[cfg(target_os = "windows")]
async fn connect_elevated_pipe(
    token: uuid::Uuid,
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, String> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let pipe_name = elevated_pipe_name(token);
    for _ in 0..100 {
        match ClientOptions::new().read(true).write(true).open(&pipe_name) {
            Ok(client) => return Ok(client),
            Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(error) => {
                return Err(format!(
                    "DS5-PKG-004: unable to connect to the administrator operation: {error}"
                ));
            }
        }
    }
    Err("DS5-PKG-004: administrator operation pipe was unavailable".to_string())
}

#[cfg(target_os = "windows")]
async fn run_elevated_helper(operation: ElevatedOperation, token: uuid::Uuid) -> i32 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let Ok(pipe) = connect_elevated_pipe(token).await else {
        return 3;
    };
    let (mut pipe_reader, mut pipe_writer) = tokio::io::split(pipe);
    // The parent never sends application data. EOF means that it timed out or
    // failed, so terminate this elevated helper. Long-running child processes
    // are job-bound and terminate when Windows closes this process's handles.
    let disconnect_watcher = tokio::spawn(async move {
        let mut control = [0u8; 1];
        let _ = pipe_reader.read(&mut control).await;
        std::process::exit(5);
    });
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<ElevatedMessage>();
    let writer = tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            let mut encoded = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
            encoded.push(b'\n');
            pipe_writer
                .write_all(&encoded)
                .await
                .map_err(|error| error.to_string())?;
        }
        pipe_writer
            .shutdown()
            .await
            .map_err(|error| error.to_string())
    });

    let progress_sender = sender.clone();
    let progress = move |stage: &str, value: u32| {
        let _ = progress_sender.send(ElevatedMessage::Progress {
            stage: stage.to_string(),
            progress: value.min(100),
        });
    };
    let outcome: Result<serde_json::Value, String> = async {
        bind_elevated_helper_lifetime()?;
        if !crate::bat_runner::is_elevated() {
            return Err("DS5-PKG-004: administrator authorization was not granted".to_string());
        }
        ensure_no_active_session().await?;
        match operation {
            ElevatedOperation::Install => {
                serde_json::to_value(dualsense_install_impl(&progress).await?)
                    .map_err(|error| error.to_string())
            }
            ElevatedOperation::TestStandard => {
                dualsense_self_test_impl("standard".to_string()).await
            }
            ElevatedOperation::TestComposite => {
                dualsense_self_test_impl("composite".to_string()).await
            }
            ElevatedOperation::Uninstall => serde_json::to_value(dualsense_uninstall_impl().await?)
                .map_err(|error| error.to_string()),
        }
    }
    .await;

    let succeeded = outcome.is_ok();
    let final_message = match outcome {
        Ok(data) => ElevatedMessage::Complete { data },
        Err(message) => ElevatedMessage::Error { message },
    };
    let _ = sender.send(final_message);
    drop(progress);
    drop(sender);
    // The parent closes its pipe end after receiving the final message. Stop
    // treating that expected EOF as cancellation before allowing the writer
    // to finish and close its half of the connection.
    disconnect_watcher.abort();
    let _ = disconnect_watcher.await;
    let writer_result = writer.await;
    match writer_result {
        Ok(Ok(())) => {}
        Ok(Err(_)) | Err(_) => return 4,
    }
    if succeeded { 0 } else { 1 }
}

/// Handle a narrowly allowlisted elevated DualSense operation before Tauri or
/// WebView startup. No caller-provided paths or commands are accepted.
#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next()?.as_str() != ELEVATED_DS5_ARG {
        return None;
    }
    let operation = match args.next().as_deref().and_then(ElevatedOperation::parse) {
        Some(operation) => operation,
        None => return Some(2),
    };
    let token = match args
        .next()
        .and_then(|value| uuid::Uuid::parse_str(&value).ok())
    {
        Some(token) if args.next().is_none() => token,
        _ => return Some(2),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return Some(3),
    };
    Some(runtime.block_on(run_elevated_helper(operation, token)))
}

#[cfg(target_os = "windows")]
async fn read_limited_elevated_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<String>, String> {
    use tokio::io::AsyncBufReadExt;

    let mut bytes = Vec::with_capacity(1024);
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|error| format!("DS5-PKG-004: administrator IPC read failed: {error}"))?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index);
        if bytes.len().saturating_add(take) > MAX_ELEVATED_MESSAGE_BYTES {
            return Err(format!(
                "DS5-PKG-004: administrator IPC message exceeds {MAX_ELEVATED_MESSAGE_BYTES} bytes"
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("DS5-PKG-004: administrator IPC was not UTF-8: {error}"))
}

#[cfg(target_os = "windows")]
async fn wait_for_elevated_process_exit(
    process: &crate::utils::ElevatedProcess,
    timeout: std::time::Duration,
) -> Result<Option<i32>, String> {
    let wait = async {
        loop {
            if let Some(exit_code) = process.exit_code()? {
                return Ok::<_, String>(exit_code);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    };

    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result.map(Some),
        Err(_) => Ok(None),
    }
}

#[cfg(target_os = "windows")]
async fn wait_for_elevated_pipe_connection<Connect, HelperExit>(
    connect: Connect,
    helper_exit: HelperExit,
) -> Result<(), String>
where
    Connect: std::future::Future<Output = std::io::Result<()>>,
    HelperExit: std::future::Future<Output = Result<Option<i32>, String>>,
{
    tokio::select! {
        biased;
        connected = connect => connected.map_err(|error| {
            format!("DS5-PKG-004: administrator IPC connection failed: {error}")
        }),
        status = helper_exit => {
            let status = status
                .map_err(|error| format!("DS5-PKG-004: administrator helper wait failed: {error}"))?;
            let Some(status) = status else {
                return Err("DS5-PKG-004: administrator authorization timed out".to_string());
            };
            Err(format!(
                "DS5-PKG-004: administrator authorization was canceled or the helper exited early ({status})"
            ))
        },
    }
}

#[cfg(target_os = "windows")]
async fn run_elevated_operation(
    app: Option<&tauri::AppHandle>,
    operation: ElevatedOperation,
) -> Result<serde_json::Value, String> {
    use tokio::io::BufReader;
    use tokio::net::windows::named_pipe::ServerOptions;

    let token = uuid::Uuid::new_v4();
    let pipe_name = elevated_pipe_name(token);
    let server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(true)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe_name)
        .map_err(|error| format!("DS5-PKG-004: unable to create administrator IPC: {error}"))?;
    let operation_arg = operation.as_arg();
    let token_arg = token.to_string();
    let elevated_process = tokio::task::spawn_blocking(move || {
        crate::utils::launch_current_executable_elevated(
            &[ELEVATED_DS5_ARG, operation_arg, &token_arg],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("DS5-PKG-004: administrator launch task failed: {error}"))?
    .map_err(|error| {
        format!("DS5-PKG-004: unable to request administrator authorization: {error}")
    })?;
    let mut helper_exit = Box::pin(wait_for_elevated_process_exit(
        &elevated_process,
        ELEVATION_CONNECT_TIMEOUT,
    ));

    wait_for_elevated_pipe_connection(server.connect(), &mut helper_exit).await?;
    drop(helper_exit);

    let receive = async move {
        let mut reader = BufReader::new(server);
        let mut final_result = None;
        while let Some(line) = read_limited_elevated_line(&mut reader).await? {
            let message: ElevatedMessage = serde_json::from_str(&line).map_err(|error| {
                format!("DS5-PKG-004: invalid administrator IPC response: {error}")
            })?;
            match message {
                ElevatedMessage::Progress { stage, progress } => {
                    if let Some(app) = app {
                        emit_progress(app, &stage, progress);
                    }
                }
                ElevatedMessage::Complete { data } => final_result = Some(Ok(data)),
                ElevatedMessage::Error { message } => final_result = Some(Err(message)),
            }
        }
        Ok::<_, String>(final_result)
    };
    let final_result = match tokio::time::timeout(operation.timeout(), receive).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            // Dropping the pipe wakes the elevated helper's disconnect watcher.
            let _ = wait_for_elevated_process_exit(
                &elevated_process,
                std::time::Duration::from_secs(5),
            )
            .await;
            return Err(error);
        }
        Err(_) => {
            // Dropping the timed-out receive future closes the pipe and causes
            // the helper to terminate itself even though this process cannot
            // directly kill a high-integrity child.
            let _ = wait_for_elevated_process_exit(
                &elevated_process,
                std::time::Duration::from_secs(5),
            )
            .await;
            return Err("DS5-PKG-004: administrator operation timed out".to_string());
        }
    };
    let status =
        wait_for_elevated_process_exit(&elevated_process, std::time::Duration::from_secs(5))
            .await
            .map_err(|error| format!("DS5-PKG-004: administrator helper wait failed: {error}"))?
            .ok_or_else(|| "DS5-PKG-004: administrator helper did not exit".to_string())?;
    match final_result {
        Some(Ok(data)) if status == 0 => Ok(data),
        Some(Err(error)) => Err(error),
        _ => Err(format!(
            "DS5-PKG-004: administrator helper failed with exit code {}",
            status
        )),
    }
}

#[tauri::command]
pub async fn dualsense_install(app: tauri::AppHandle) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    #[cfg(target_os = "windows")]
    {
        let data = run_elevated_operation(Some(&app), ElevatedOperation::Install).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let progress = |stage: &str, value: u32| emit_progress(&app, stage, value);
        dualsense_install_impl(&progress).await
    }
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
    let mut settings = get_core_ds5_settings().await?.settings;
    settings.ds5_enabled = enabled;
    settings.ds5_audio_haptics = audio_haptics;
    tokio::time::timeout(CONFIG_APPLY_TIMEOUT, save_core_ds5_settings(settings))
    .await
    .map_err(|_| {
        "DS5-CFG-004: timed out while applying DualSense configuration; the resulting state is unknown"
            .to_string()
    })??;
    dualsense_get_status().await
}

async fn dualsense_self_test_impl(profile: String) -> Result<serde_json::Value, String> {
    if profile != "standard" && profile != "composite" {
        return Err("DS5-PKG-003: invalid self-test profile".to_string());
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
                true,
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
pub async fn dualsense_self_test(profile: String) -> Result<serde_json::Value, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    if profile != "standard" && profile != "composite" {
        return Err("DS5-PKG-003: invalid self-test profile".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let operation = if profile == "composite" {
            ElevatedOperation::TestComposite
        } else {
            ElevatedOperation::TestStandard
        };
        return run_elevated_operation(None, operation).await;
    }
    #[cfg(not(target_os = "windows"))]
    {
        dualsense_self_test_impl(profile).await
    }
}

async fn dualsense_uninstall_impl() -> Result<DualSenseStatus, String> {
    let mut settings = match get_core_ds5_settings().await {
        Ok(snapshot) => snapshot.settings,
        Err(error) => {
            warn!("DualSense uninstall could not read the current configuration: {error}");
            CoreDualSenseSettings::default()
        }
    };
    settings.ds5_enabled = false;
    settings.ds5_audio_haptics = true;
    if let Err(error) = save_core_ds5_settings(settings).await {
        // Removing the optional component must remain possible while Sunshine
        // is stopped. A later Core start safely falls back when files are absent.
        warn!("DualSense uninstall could not reset the configuration: {error}");
    }
    let root = component_root();
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("DS5-PKG-002: unable to remove component: {error}"))?;
    }
    match dualsense_get_status().await {
        Ok(status) => Ok(status),
        Err(error) => {
            warn!("DualSense uninstall could not refresh Core status: {error}");
            Ok(local_uninstalled_status())
        }
    }
}

#[tauri::command]
pub async fn dualsense_uninstall() -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session_for_uninstall().await?;
    #[cfg(target_os = "windows")]
    {
        let data = run_elevated_operation(None, ElevatedOperation::Uninstall).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        dualsense_uninstall_impl().await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoreDualSenseResponse, CoreDualSenseSettings, UsbipInstallResult, clamp_tuning,
        classify_usbip_installer_exit_code, component_state, component_test_failure,
        local_uninstalled_status, pinned_usbip_installed, validate_core_ds5_response,
        validate_requested_profile,
    };
    #[cfg(target_os = "windows")]
    use super::{
        ElevatedMessage, ElevatedOperation, MAX_ELEVATED_MESSAGE_BYTES, elevated_pipe_name,
        read_limited_elevated_line, wait_for_elevated_pipe_connection,
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
    fn usbip_reboot_exit_code_keeps_component_installation_running() {
        assert_eq!(
            classify_usbip_installer_exit_code(Some(0)).unwrap(),
            UsbipInstallResult::Ready
        );
        assert_eq!(
            classify_usbip_installer_exit_code(Some(3010)).unwrap(),
            UsbipInstallResult::RebootRecommended
        );
        assert!(classify_usbip_installer_exit_code(Some(1)).is_err());
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
    fn core_settings_payload_contains_only_independent_ds5_fields() {
        let payload = serde_json::to_value(CoreDualSenseSettings {
            ds5_enabled: true,
            ds5_audio_haptics: false,
            ds5_legacy_haptics_strength: 1.5,
            ds5_legacy_haptics_curve: 0.5,
            ds5_legacy_haptics_noise_gate: 0.006,
        })
        .unwrap();
        let object = payload.as_object().unwrap();
        assert_eq!(object.len(), 5);
        assert!(!object.contains_key("gamepad"));
        assert!(!object.contains_key("ds5_sidecar_path"));
    }

    #[test]
    fn tuning_values_are_clamped_to_the_supported_ranges() {
        assert_eq!(clamp_tuning(9.0, 9.0, 9.0), Some((4.0, 2.0, 0.060)));
        assert_eq!(clamp_tuning(-1.0, -1.0, 0.0), Some((0.1, 0.3, 0.002)));
        assert!(clamp_tuning(f64::NAN, 1.0, 0.020).is_none());
        assert!(clamp_tuning(1.0, f64::INFINITY, 0.020).is_none());
    }

    #[test]
    fn core_response_requires_applied_revision_and_valid_values() {
        let valid = || CoreDualSenseResponse {
            status: true,
            applied: true,
            revision: 2,
            changed: Some(true),
            settings: CoreDualSenseSettings::default(),
        };
        assert!(validate_core_ds5_response(valid()).is_ok());

        let mut rejected = valid();
        rejected.applied = false;
        assert!(validate_core_ds5_response(rejected).is_err());

        let mut missing_revision = valid();
        missing_revision.revision = 0;
        assert!(validate_core_ds5_response(missing_revision).is_err());

        let mut invalid_value = valid();
        invalid_value.settings.ds5_legacy_haptics_curve = 3.0;
        assert!(validate_core_ds5_response(invalid_value).is_err());
    }

    #[test]
    fn offline_uninstall_status_is_locally_complete() {
        let status = local_uninstalled_status();
        assert_eq!(status.state, "not_installed");
        assert!(!status.installed);
        assert!(!status.enabled);
        assert!(status.audio_haptics);
        assert!(!status.in_use);
    }

    #[test]
    fn usbip_requires_the_exact_pinned_version() {
        assert!(pinned_usbip_installed(Some("0.9.7.7")));
        assert!(!pinned_usbip_installed(Some("0.9.7.3")));
        assert!(!pinned_usbip_installed(None));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn elevated_operations_are_strictly_allowlisted() {
        assert!(matches!(
            ElevatedOperation::parse("install"),
            Some(ElevatedOperation::Install)
        ));
        assert!(matches!(
            ElevatedOperation::parse("test-standard"),
            Some(ElevatedOperation::TestStandard)
        ));
        assert!(ElevatedOperation::parse("test-custom").is_none());
        assert!(ElevatedOperation::parse("install C:\\Windows").is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn elevated_ipc_uses_a_random_local_pipe_and_typed_messages() {
        let token = uuid::Uuid::parse_str("55d1fc2d-b474-4a86-a867-c6c514c077ef").unwrap();
        assert_eq!(
            elevated_pipe_name(token),
            r"\\.\pipe\sunshine-dualsense-55d1fc2d-b474-4a86-a867-c6c514c077ef"
        );
        let encoded = serde_json::to_string(&ElevatedMessage::Progress {
            stage: "probing".to_string(),
            progress: 88,
        })
        .unwrap();
        let decoded: ElevatedMessage = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded,
            ElevatedMessage::Progress {
                ref stage,
                progress: 88
            } if stage == "probing"
        ));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn elevated_ipc_rejects_oversized_messages_without_waiting_for_eof() {
        use tokio::io::{AsyncWriteExt, BufReader};

        let (reader, mut writer) = tokio::io::duplex(MAX_ELEVATED_MESSAGE_BYTES + 2);
        let write = tokio::spawn(async move {
            writer
                .write_all(&vec![b'x'; MAX_ELEVATED_MESSAGE_BYTES + 1])
                .await
                .unwrap();
        });
        let error = read_limited_elevated_line(&mut BufReader::new(reader))
            .await
            .unwrap_err();
        write.await.unwrap();
        assert!(error.contains("exceeds 65536 bytes"));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn elevated_ipc_prefers_a_ready_pipe_over_a_ready_helper_exit() {
        let result = wait_for_elevated_pipe_connection(
            std::future::ready(Ok(())),
            std::future::ready(Ok(Some(0))),
        )
        .await;

        assert!(result.is_ok());
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
