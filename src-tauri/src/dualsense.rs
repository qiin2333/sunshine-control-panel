//! Optional DualSense component lifecycle.
//!
//! HIDMaestro is downloaded from its pinned upstream release and verified before
//! extraction. The Sunshine-owned sidecar is acquired from a release asset
//! pinned by the installed Sunshine manifest, from a matching user-selected
//! package, or from an explicit development override. This keeps the optional
//! self-contained .NET runtime out of the main Sunshine package.

use futures_util::StreamExt;
use log::warn;
use once_cell::sync::{Lazy, OnceCell};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;

const COMPONENT_VERSION: &str = "1.2.0";
const PROTOCOL_VERSION: u32 = 1;
const HIDMAESTRO_VERSION: &str = "v1.6.2";
const HIDMAESTRO_URL: &str =
    "https://github.com/hifihedgehog/HIDMaestro/releases/download/v1.6.2/HIDMaestro-v1.6.2.zip";
const HIDMAESTRO_SHA256: &str = "6ae8df0cf317baf7e65777e2929f618916a67831b5ff1162205310f2c08b80ff";
const USBIP_VERSION: &str = "0.9.7.7";
const USBIP_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
const USBIP_SHA256: &str = "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";
const MAX_ARCHIVE_BYTES: u64 = 160 * 1024 * 1024;
const MAX_USBIP_INSTALLER_BYTES: u64 = 48 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 200 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 256;
const SIDECAR_EXE: &str = "Sunshine.Ds5Sidecar.exe";
const SIDECAR_PACKAGE_MANIFEST: &str = "ds5-sidecar-package.json";
const SIDECAR_PACKAGE_ASSET: &str = "Sunshine.Ds5Sidecar.x64.zip";
const SIDECAR_PACKAGE_TARGET: &str = "win-x64-self-contained";
const SIDECAR_PACKAGE_LICENSE: &str = "GPL-3.0-only";
const MAX_SIDECAR_PACKAGE_BYTES: u64 = 160 * 1024 * 1024;
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ElevatedOperation {
    Install,
    InstallLocal,
    TestStandard,
    TestComposite,
    Uninstall,
}

#[cfg(target_os = "windows")]
impl ElevatedOperation {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::InstallLocal => "install-local",
            Self::TestStandard => "test-standard",
            Self::TestComposite => "test-composite",
            Self::Uninstall => "uninstall",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "install" => Some(Self::Install),
            "install-local" => Some(Self::InstallLocal),
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
            Self::Install | Self::InstallLocal => std::time::Duration::from_secs(35 * 60),
            Self::TestStandard | Self::TestComposite => std::time::Duration::from_secs(90),
            Self::Uninstall => std::time::Duration::from_secs(120),
        }
    }
}

#[cfg(target_os = "windows")]
struct TemporaryPackageFile(PathBuf);

#[cfg(target_os = "windows")]
impl TemporaryPackageFile {
    fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(target_os = "windows")]
impl Drop for TemporaryPackageFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
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
    pub genshin_compatibility: bool,
    pub genshin_compatibility_available: bool,
    pub legacy_strength: f64,
    pub legacy_curve: f64,
    pub legacy_noise_gate: f64,
    pub config_revision: u64,
    pub config_readable: bool,
    pub component_version: String,
    pub available_component_version: String,
    pub update_available: bool,
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
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

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
    genshin_compatibility_identity: bool,
    audio_policy_violation: bool,
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
    ds5_genshin_compatibility: bool,
}

impl Default for CoreDualSenseSettings {
    fn default() -> Self {
        Self {
            ds5_enabled: false,
            ds5_audio_haptics: true,
            ds5_legacy_haptics_strength: 1.0,
            ds5_legacy_haptics_curve: 0.5,
            ds5_legacy_haptics_noise_gate: 0.020,
            ds5_genshin_compatibility: false,
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

#[derive(Debug)]
struct CoreDualSenseSnapshot {
    response: CoreDualSenseResponse,
    entity_tag: Option<HeaderValue>,
}

#[derive(Debug, Serialize)]
pub struct DualSenseTuningResult {
    legacy_strength: f64,
    legacy_curve: f64,
    legacy_noise_gate: f64,
    revision: u64,
    changed: bool,
}

fn validate_strong_entity_tag(value: HeaderValue) -> Result<HeaderValue, String> {
    const MAX_ENTITY_TAG_SIZE: usize = 512;
    let bytes = value.as_bytes();
    let valid = bytes.len() >= 2
        && bytes.len() <= MAX_ENTITY_TAG_SIZE
        && bytes.first() == Some(&b'"')
        && bytes.last() == Some(&b'"')
        && bytes[1..bytes.len() - 1]
            .iter()
            .copied()
            .all(|byte| byte == 0x21 || (0x23..=0x7e).contains(&byte) || byte >= 0x80);
    valid.then_some(value).ok_or_else(|| {
        "DS5-CFG-007: Sunshine returned an invalid DualSense configuration ETag".to_string()
    })
}

fn require_entity_tag(entity_tag: Option<HeaderValue>) -> Result<HeaderValue, String> {
    entity_tag.ok_or_else(|| {
        "DS5-CFG-007: Sunshine does not expose conditional DualSense configuration updates"
            .to_string()
    })
}

fn core_ds5_http_error(error_code: Option<&str>, status: reqwest::StatusCode) -> String {
    match error_code {
        Some("ds5_config_invalid") => {
            "DS5-CFG-005: the independent DualSense configuration is invalid".to_string()
        }
        Some("ds5_precondition_failed") => {
            "DS5-CFG-006: DualSense configuration changed during this save; refresh and try again"
                .to_string()
        }
        Some("ds5_precondition_required") | Some("ds5_if_match_invalid") => {
            "DS5-CFG-007: Sunshine rejected the conditional DualSense configuration protocol"
                .to_string()
        }
        _ if status == reqwest::StatusCode::PRECONDITION_FAILED => {
            "DS5-CFG-006: DualSense configuration changed during this save; refresh and try again"
                .to_string()
        }
        _ if status.as_u16() == 428 => {
            "DS5-CFG-007: Sunshine rejected the conditional DualSense configuration protocol"
                .to_string()
        }
        _ => format!(
            "DS5-CFG-001: Sunshine rejected the DualSense configuration request (HTTP {})",
            status.as_u16()
        ),
    }
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
    if values.ds5_genshin_compatibility
        && (!values.ds5_enabled || !values.ds5_audio_haptics)
    {
        return Err("DS5-CFG-001: Sunshine returned invalid DualSense values".to_string());
    }
    Ok(result)
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InstalledComponentManifest {
    component_version: String,
    hidmaestro_version: String,
    sha256: String,
    protocol: u32,
    sidecar_file: String,
    sidecar_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SidecarPackageManifest {
    schema: u32,
    component_version: String,
    protocol: u32,
    target: String,
    license: String,
    asset_name: String,
    download_url: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct SidecarRuntimeMetadata {
    component_version: String,
    protocol: u32,
    target: String,
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

fn sidecar_package_manifest_path() -> PathBuf {
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("tools")
        .join(SIDECAR_PACKAGE_MANIFEST)
}

fn manually_placed_sidecar_package() -> Option<PathBuf> {
    let sunshine_root = PathBuf::from(crate::sunshine::get_sunshine_install_path());
    [
        sunshine_root.join(SIDECAR_PACKAGE_ASSET),
        sunshine_root.join("tools").join(SIDECAR_PACKAGE_ASSET),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn validate_sidecar_package_manifest(
    manifest: SidecarPackageManifest,
) -> Result<SidecarPackageManifest, String> {
    let valid_digest =
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
    if manifest.schema != 1
        || manifest.component_version != COMPONENT_VERSION
        || manifest.protocol != PROTOCOL_VERSION
        || manifest.target != SIDECAR_PACKAGE_TARGET
        || manifest.license != SIDECAR_PACKAGE_LICENSE
        || manifest.asset_name != SIDECAR_PACKAGE_ASSET
        || !valid_digest
        || manifest.size == 0
        || manifest.size > MAX_SIDECAR_PACKAGE_BYTES
    {
        return Err("DS5-PKG-002: the DualSense package manifest is invalid".to_string());
    }
    if !manifest.download_url.is_empty() {
        let url = url::Url::parse(&manifest.download_url)
            .map_err(|_| "DS5-PKG-002: the DualSense package URL is invalid".to_string())?;
        let valid_url = url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url
                .path()
                .starts_with("/AlkaidLab/foundation-sunshine/releases/download/")
            && url
                .path_segments()
                .and_then(Iterator::last)
                .is_some_and(|name| name == SIDECAR_PACKAGE_ASSET);
        if !valid_url {
            return Err("DS5-PKG-002: the DualSense package URL is not trusted".to_string());
        }
    }
    Ok(manifest)
}

fn sidecar_package_manifest() -> Result<SidecarPackageManifest, String> {
    let path = sidecar_package_manifest_path();
    let contents = fs::read(&path).map_err(|error| {
        format!(
            "DS5-PKG-002: the DualSense package manifest is missing ({}): {error}",
            path.display()
        )
    })?;
    let manifest = serde_json::from_slice(&contents)
        .map_err(|error| format!("DS5-PKG-002: invalid DualSense package manifest: {error}"))?;
    validate_sidecar_package_manifest(manifest)
}

#[cfg(target_os = "windows")]
fn local_sidecar_package_handoff_path(token: uuid::Uuid) -> PathBuf {
    component_root().join(format!("handoff-{token}.partial.zip"))
}

fn purge_stale_handoff_packages(root: &Path, current_package: Option<&Path>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if current_package.is_some_and(|current| current == path) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with("handoff-") && name.ends_with(".partial.zip") {
            let _ = fs::remove_file(path);
        }
    }
}

fn installed_component_manifest() -> Option<InstalledComponentManifest> {
    let contents = fs::read(active_dir().join("component.json")).ok()?;
    serde_json::from_slice(&contents).ok()
}

fn component_update_available(manifest: Option<&InstalledComponentManifest>) -> bool {
    manifest.is_some_and(|manifest| manifest.component_version != COMPONENT_VERSION)
}

fn component_matches_current_runtime(
    manifest: Option<&InstalledComponentManifest>,
    genshin_compatibility_available: bool,
    audio_policy_violation_available: bool,
) -> bool {
    let Some(manifest) = manifest else {
        return false;
    };
    manifest.component_version == COMPONENT_VERSION
        && manifest.hidmaestro_version == HIDMAESTRO_VERSION
        && manifest.sha256.eq_ignore_ascii_case(HIDMAESTRO_SHA256)
        && manifest.protocol == PROTOCOL_VERSION
        && manifest.sidecar_file == SIDECAR_EXE
        && genshin_compatibility_available
        && audio_policy_violation_available
}

async fn read_core_ds5_response(
    mut response: reqwest::Response,
) -> Result<CoreDualSenseSnapshot, String> {
    const MAX_RESPONSE_BYTES: usize = 64 * 1024;
    let status = response.status();
    let entity_tag = response.headers().get(reqwest::header::ETAG).cloned();
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
        return Err(core_ds5_http_error(error_code.as_deref(), status));
    }
    let result: CoreDualSenseResponse = serde_json::from_slice(&bytes).map_err(|error| {
        format!("DS5-CFG-001: invalid DualSense configuration response: {error}")
    })?;
    let result = validate_core_ds5_response(result)?;
    let entity_tag = entity_tag.map(validate_strong_entity_tag).transpose()?;
    Ok(CoreDualSenseSnapshot {
        response: result,
        entity_tag,
    })
}

async fn get_core_ds5_settings() -> Result<CoreDualSenseSnapshot, String> {
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
    entity_tag: HeaderValue,
) -> Result<CoreDualSenseSnapshot, String> {
    let base_url = crate::sunshine::get_sunshine_url().await?;
    let response = crate::sunshine::create_https_client()?
        .post(format!(
            "{}/api/dualsense/config",
            base_url.trim_end_matches('/')
        ))
        .header(reqwest::header::IF_MATCH, entity_tag)
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
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

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
    if !result.genshin_compatibility_identity {
        return Err(
            "DS5-PROTO-001: the sidecar runtime does not support Genshin compatibility mode"
                .to_string(),
        );
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
    update_available: bool,
) -> &'static str {
    if in_use && installed {
        "in_use"
    } else if !installed {
        "not_installed"
    } else if !verified {
        "repair_required"
    } else if update_available {
        "update_available"
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

fn update_config_fields(
    settings: &mut CoreDualSenseSettings,
    enabled: bool,
    audio_haptics: bool,
    genshin_compatibility: bool,
) {
    settings.ds5_enabled = enabled;
    settings.ds5_audio_haptics = audio_haptics;
    settings.ds5_genshin_compatibility = genshin_compatibility;
}

fn update_tuning_fields(
    settings: &mut CoreDualSenseSettings,
    strength: f64,
    curve: f64,
    noise_gate: f64,
) {
    settings.ds5_legacy_haptics_strength = strength;
    settings.ds5_legacy_haptics_curve = curve;
    settings.ds5_legacy_haptics_noise_gate = noise_gate;
}

fn local_uninstalled_status() -> DualSenseStatus {
    let usbip_version = installed_usbip_version().unwrap_or_default();
    DualSenseStatus {
        state: "not_installed".to_string(),
        installed: false,
        verified: false,
        enabled: false,
        audio_haptics: true,
        genshin_compatibility: false,
        genshin_compatibility_available: false,
        legacy_strength: 1.0,
        legacy_curve: 0.5,
        legacy_noise_gate: 0.020,
        config_revision: 0,
        config_readable: false,
        component_version: String::new(),
        available_component_version: COMPONENT_VERSION.to_string(),
        update_available: false,
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
    genshin_compatibility: bool,
    usbip_available: bool,
) -> Result<(), String> {
    if genshin_compatibility && (!enabled || !audio_haptics) {
        Err(
            "DS5-RUN-004: Genshin compatibility mode requires enabled four-channel haptics"
                .to_string(),
        )
    } else if enabled && audio_haptics && !usbip_available {
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
    let snapshot = get_core_ds5_settings().await?;
    let mut settings = snapshot.response.settings;
    update_tuning_fields(&mut settings, strength, curve, noise_gate);
    let entity_tag = require_entity_tag(snapshot.entity_tag)?;
    let applied = save_core_ds5_settings(settings, entity_tag).await?.response;
    Ok(DualSenseTuningResult {
        legacy_strength: applied.settings.ds5_legacy_haptics_strength,
        legacy_curve: applied.settings.ds5_legacy_haptics_curve,
        legacy_noise_gate: applied.settings.ds5_legacy_haptics_noise_gate,
        revision: applied.revision,
        changed: applied.changed.unwrap_or(false),
    })
}

async fn resolve_core_config<F, Fut>(
    confirmed: Option<CoreDualSenseResponse>,
    fetch: F,
) -> (CoreDualSenseSettings, u64, String)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<CoreDualSenseSnapshot, String>>,
{
    if let Some(response) = confirmed {
        return (response.settings, response.revision, String::new());
    }

    match fetch().await {
        Ok(snapshot) => (
            snapshot.response.settings,
            snapshot.response.revision,
            String::new(),
        ),
        Err(error) => {
            warn!("DualSense status could not read the configuration: {error}");
            (CoreDualSenseSettings::default(), 0, error)
        }
    }
}

async fn dualsense_get_status_with_config(
    confirmed: Option<CoreDualSenseResponse>,
) -> Result<DualSenseStatus, String> {
    let (settings, config_revision, config_error) =
        resolve_core_config(confirmed, get_core_ds5_settings).await;
    let config_readable = config_error.is_empty();
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
    let (probe_succeeded, result, mut error_code, mut detail) = match probe {
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
    let manifest = installed.then(installed_component_manifest).flatten();
    let update_available = installed && component_update_available(manifest.as_ref());
    let matches_current_runtime = component_matches_current_runtime(
        manifest.as_ref(),
        result.genshin_compatibility_identity,
        result.audio_policy_violation,
    );
    let verified = probe_succeeded && (update_available || matches_current_runtime);
    if probe_succeeded && !update_available && !matches_current_runtime {
        error_code = "DS5-PROTO-001".to_string();
        detail = "DS5-PROTO-001: the installed component metadata or capabilities do not match this Control Panel build".to_string();
    }
    let state = component_state(
        installed,
        verified,
        usbip_available,
        in_use,
        update_available,
    );

    Ok(DualSenseStatus {
        state: state.to_string(),
        installed,
        verified,
        enabled: settings.ds5_enabled,
        audio_haptics: settings.ds5_audio_haptics,
        genshin_compatibility: settings.ds5_genshin_compatibility,
        genshin_compatibility_available: result.genshin_compatibility_identity,
        legacy_strength: settings.ds5_legacy_haptics_strength,
        legacy_curve: settings.ds5_legacy_haptics_curve,
        legacy_noise_gate: settings.ds5_legacy_haptics_noise_gate,
        config_revision,
        config_readable,
        component_version: manifest
            .as_ref()
            .map(|manifest| manifest.component_version.clone())
            .unwrap_or_default(),
        available_component_version: COMPONENT_VERSION.to_string(),
        update_available,
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

#[tauri::command]
pub async fn dualsense_get_status() -> Result<DualSenseStatus, String> {
    dualsense_get_status_with_config(None).await
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
    validate_sidecar_runtime_metadata(source)?;
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

fn validate_sidecar_runtime_metadata(directory: &Path) -> Result<(), String> {
    let runtime_metadata: SidecarRuntimeMetadata =
        serde_json::from_slice(&fs::read(directory.join("runtime.json")).map_err(|error| {
            format!("DS5-PKG-002: sidecar runtime metadata is missing: {error}")
        })?)
        .map_err(|error| format!("DS5-PKG-002: invalid sidecar runtime metadata: {error}"))?;
    if runtime_metadata.component_version != COMPONENT_VERSION
        || runtime_metadata.protocol != PROTOCOL_VERSION
        || runtime_metadata.target != SIDECAR_PACKAGE_TARGET
    {
        return Err("DS5-PKG-002: sidecar runtime metadata does not match Sunshine".to_string());
    }
    Ok(())
}

fn validate_component_backup(directory: &Path) -> Result<(), String> {
    let manifest: InstalledComponentManifest = serde_json::from_slice(
        &fs::read(directory.join("component.json"))
            .map_err(|error| format!("backup component manifest is missing: {error}"))?,
    )
    .map_err(|error| format!("backup component manifest is invalid: {error}"))?;
    if manifest.protocol != PROTOCOL_VERSION || manifest.sidecar_file != SIDECAR_EXE {
        return Err("backup component protocol or sidecar does not match Sunshine".to_string());
    }

    let runtime_metadata: SidecarRuntimeMetadata = serde_json::from_slice(
        &fs::read(directory.join("runtime.json"))
            .map_err(|error| format!("backup runtime metadata is missing: {error}"))?,
    )
    .map_err(|error| format!("backup runtime metadata is invalid: {error}"))?;
    if runtime_metadata.component_version != manifest.component_version
        || runtime_metadata.protocol != PROTOCOL_VERSION
        || runtime_metadata.target != SIDECAR_PACKAGE_TARGET
    {
        return Err("backup runtime metadata does not match its component manifest".to_string());
    }

    let sidecar_hash = sha256_file(&directory.join(SIDECAR_EXE))?;
    if manifest.sidecar_sha256.len() != 64
        || !sidecar_hash.eq_ignore_ascii_case(&manifest.sidecar_sha256)
    {
        return Err("backup sidecar integrity check failed".to_string());
    }
    let hidmaestro_hash = sha256_file(&directory.join("HIDMaestro.Core.dll"))?;
    if manifest.sha256.len() != 64 || !hidmaestro_hash.eq_ignore_ascii_case(&manifest.sha256) {
        return Err("backup HIDMaestro integrity check failed".to_string());
    }
    Ok(())
}

fn extract_sidecar_package(
    archive_path: &Path,
    staging: &Path,
    manifest: &SidecarPackageManifest,
) -> Result<(), String> {
    let metadata = fs::metadata(archive_path)
        .map_err(|error| format!("DS5-PKG-002: unable to inspect sidecar package: {error}"))?;
    if metadata.len() != manifest.size {
        return Err(format!(
            "DS5-PKG-001: expected sidecar package size {}, got {}",
            manifest.size,
            metadata.len()
        ));
    }
    let actual = sha256_file(archive_path)?;
    if !actual.eq_ignore_ascii_case(&manifest.sha256) {
        return Err(format!(
            "DS5-PKG-001: expected sidecar SHA-256 {}, got {actual}",
            manifest.sha256
        ));
    }

    let mut archive =
        zip::ZipArchive::new(File::open(archive_path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("DS5-PKG-002: invalid sidecar package: {error}"))?;
    if archive.len() > MAX_ARCHIVE_FILES {
        return Err("DS5-PKG-002: sidecar package contains too many files".to_string());
    }
    let mut extracted_bytes = 0u64;
    let mut names = HashSet::new();
    for index in 0..archive.len() {
        let mut item = archive.by_index(index).map_err(|error| error.to_string())?;
        if item.is_dir() {
            continue;
        }
        extracted_bytes = extracted_bytes.saturating_add(item.size());
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err("DS5-PKG-002: sidecar package exceeds the extraction limit".to_string());
        }
        let Some(relative) = item.enclosed_name() else {
            return Err("DS5-PKG-002: sidecar package contains an unsafe path".to_string());
        };
        if relative.components().count() != 1 {
            return Err("DS5-PKG-002: sidecar package must contain only root files".to_string());
        }
        let Some(name) = relative.file_name().and_then(|name| name.to_str()) else {
            return Err("DS5-PKG-002: sidecar package contains an invalid file name".to_string());
        };
        if name.eq_ignore_ascii_case("HIDMaestro.Core.dll") {
            return Err(
                "DS5-PKG-002: sidecar package must not bundle HIDMaestro.Core.dll".to_string(),
            );
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err("DS5-PKG-002: sidecar package contains duplicate files".to_string());
        }
        let mut output = File::create(staging.join(name)).map_err(|error| error.to_string())?;
        std::io::copy(&mut item, &mut output).map_err(|error| error.to_string())?;
    }

    if !staging.join(SIDECAR_EXE).is_file() {
        return Err("DS5-PKG-002: sidecar executable is missing from the package".to_string());
    }
    validate_sidecar_runtime_metadata(staging)?;
    Ok(())
}

async fn acquire_sidecar_package(
    client: &reqwest::Client,
    manifest: &SidecarPackageManifest,
    local_package: Option<&Path>,
    destination: &Path,
    progress: &ProgressReporter<'_>,
) -> Result<(), String> {
    if let Some(source) = local_package {
        let metadata = fs::metadata(source).map_err(|error| {
            format!("DS5-PKG-002: unable to open the selected sidecar package: {error}")
        })?;
        if !metadata.is_file() || metadata.len() != manifest.size {
            return Err(
                "DS5-PKG-005: the selected component package does not match this Sunshine build"
                    .to_string(),
            );
        }
        report_progress(progress, "sidecar_verifying", 18);
        tokio::fs::copy(source, destination)
            .await
            .map_err(|error| format!("DS5-PKG-002: unable to stage sidecar package: {error}"))?;
        let actual = sha256_file(destination)?;
        if !actual.eq_ignore_ascii_case(&manifest.sha256) {
            return Err(format!(
                "DS5-PKG-005: selected package SHA-256 does not match (got {actual})"
            ));
        }
        return Ok(());
    }

    if manifest.download_url.is_empty() {
        return Err(
            "DS5-PKG-001: this development build has no Sidecar download URL; select the matching local component package"
                .to_string(),
        );
    }
    report_progress(progress, "sidecar_downloading", 12);
    let response = client
        .get(&manifest.download_url)
        .send()
        .await
        .map_err(|error| format!("DS5-PKG-001: sidecar download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("DS5-PKG-001: sidecar download failed: {error}"))?;
    let total_size = response.content_length();
    if total_size.is_some_and(|size| size != manifest.size) {
        return Err("DS5-PKG-001: sidecar download size does not match the manifest".to_string());
    }
    let mut output = File::create(destination).map_err(|error| error.to_string())?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| format!("DS5-PKG-001: sidecar download failed: {error}"))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > manifest.size || downloaded > MAX_SIDECAR_PACKAGE_BYTES {
            return Err("DS5-PKG-001: sidecar download exceeds the manifest size".to_string());
        }
        output
            .write_all(&chunk)
            .map_err(|error| error.to_string())?;
        let value = 12 + (downloaded.saturating_mul(24) / manifest.size).min(24) as u32;
        report_progress(progress, "sidecar_downloading", value);
    }
    drop(output);
    if downloaded != manifest.size {
        return Err("DS5-PKG-001: sidecar download ended before the expected size".to_string());
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

fn rollback_activated_component(
    active: &Path,
    backup: &Path,
    had_previous: bool,
) -> Result<(), String> {
    if active.exists() {
        fs::remove_dir_all(active)
            .map_err(|error| format!("unable to remove failed active component: {error}"))?;
    }
    if had_previous {
        if !backup.exists() {
            return Err("previous component backup is missing".to_string());
        }
        validate_component_backup(backup)?;
        fs::rename(backup, active)
            .map_err(|error| format!("unable to restore previous component: {error}"))?;
    }
    Ok(())
}

fn install_error_after_rollback(
    error: String,
    active: &Path,
    backup: &Path,
    had_previous: bool,
) -> String {
    match rollback_activated_component(active, backup, had_previous) {
        Ok(()) => format!("{error}; activated component was rolled back"),
        Err(rollback_error) => {
            format!("{error}; component rollback also failed: {rollback_error}")
        }
    }
}

async fn dualsense_install_impl(
    progress: &ProgressReporter<'_>,
    local_sidecar_package: Option<&Path>,
) -> Result<DualSenseStatus, String> {
    report_progress(progress, "preparing", 1);
    let discovered_package = local_sidecar_package
        .is_none()
        .then(manually_placed_sidecar_package)
        .flatten();
    let using_discovered_package = discovered_package.is_some();
    let using_selected_package = local_sidecar_package.is_some();
    let local_sidecar_package = local_sidecar_package.or(discovered_package.as_deref());
    let manifest_available = sidecar_package_manifest_path()
        .try_exists()
        .map_err(|error| {
            format!("DS5-PKG-002: unable to inspect the DualSense package manifest: {error}")
        })?;
    let package_manifest = if local_sidecar_package.is_some() || manifest_available {
        Some(sidecar_package_manifest()?)
    } else {
        None
    };
    let bundled_source = if package_manifest.is_none() {
        Some(sidecar_source_dir()?)
    } else {
        None
    };
    let root = component_root();
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let operation = format!("staging-{}", std::process::id());
    let staging = root.join(&operation);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    purge_stale_handoff_packages(&root, local_sidecar_package);
    let archive_path = root.join(format!("{operation}-hidmaestro.partial"));
    let sidecar_archive_path = root.join(format!("{operation}-sidecar.partial.zip"));
    let active = active_dir();
    let backup = root.join("previous");
    let had_previous = active.exists();
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("foundation-sunshine-dualsense-component")
        .build()
        .map_err(|error| error.to_string())?;

    let install_result: Result<UsbipInstallResult, String> = async {
        let usbip_install_result = ensure_pinned_usbip(progress, &client, &root).await?;
        if let Some(manifest) = package_manifest.as_ref() {
            if using_discovered_package {
                report_progress(progress, "sidecar_local", 12);
            }
            acquire_sidecar_package(
                &client,
                manifest,
                local_sidecar_package,
                &sidecar_archive_path,
                progress,
            )
            .await?;
            report_progress(progress, "sidecar_verifying", 36);
            let archive = sidecar_archive_path.clone();
            let destination = staging.clone();
            let manifest = manifest.clone();
            tokio::task::spawn_blocking(move || {
                extract_sidecar_package(&archive, &destination, &manifest)
            })
            .await
            .map_err(|error| format!("DS5-PKG-002: sidecar extraction task failed: {error}"))??;
        } else if let Some(source) = bundled_source.as_ref() {
            copy_runtime_files(source, &staging)?;
        } else {
            return Err("DS5-PKG-002: no DualSense sidecar source is available".to_string());
        }

        let response = client
            .get(HIDMAESTRO_URL)
            .send()
            .await
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("DS5-PKG-001: download failed: {error}"))?;
        report_progress(progress, "downloading", 38);
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
                let download_progress = (downloaded * 34 / total).min(34) as u32;
                report_progress(progress, "downloading", 38 + download_progress);
            }
        }
        drop(output);

        report_progress(progress, "verifying", 74);
        let archive = archive_path.clone();
        let destination = staging.clone();
        tokio::task::spawn_blocking(move || extract_verified_package(&archive, &destination))
            .await
            .map_err(|error| error.to_string())??;
        report_progress(progress, "probing", 86);
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
                "sidecar_source": if using_discovered_package {
                    "discovered-local"
                } else if using_selected_package {
                    "selected-local"
                } else {
                    package_manifest.as_ref().map(|manifest| manifest.download_url.as_str()).unwrap_or("bundled")
                },
                "sidecar_package_sha256": package_manifest.as_ref().map(|manifest| manifest.sha256.as_str()).unwrap_or_default(),
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
    let _ = fs::remove_file(&sidecar_archive_path);
    if install_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let reboot_recommended = matches!(install_result?, UsbipInstallResult::RebootRecommended);
    report_progress(progress, "complete", 100);
    let mut status = match dualsense_get_status().await {
        Ok(status) => status,
        Err(status_error) => {
            return Err(install_error_after_rollback(
                status_error,
                &active,
                &backup,
                had_previous,
            ));
        }
    };
    if !status.verified {
        let verification_error = if status.detail.is_empty() {
            "DS5-PKG-003: the installed component did not pass the final capability check"
                .to_string()
        } else {
            status.detail
        };
        return Err(install_error_after_rollback(
            verification_error,
            &active,
            &backup,
            had_previous,
        ));
    }
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
async fn receive_local_sidecar_package<R>(
    reader: &mut R,
    token: uuid::Uuid,
) -> Result<TemporaryPackageFile, String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut encoded_size = [0u8; std::mem::size_of::<u64>()];
    reader
        .read_exact(&mut encoded_size)
        .await
        .map_err(|error| {
            format!("DS5-PKG-002: unable to receive the selected component package size: {error}")
        })?;
    let size = u64::from_le_bytes(encoded_size);
    if size == 0 || size > MAX_SIDECAR_PACKAGE_BYTES {
        return Err("DS5-PKG-002: the selected component package is invalid".to_string());
    }

    let root = component_root();
    fs::create_dir_all(&root).map_err(|error| {
        format!("DS5-PKG-002: unable to prepare the component directory: {error}")
    })?;
    let package = TemporaryPackageFile(local_sidecar_package_handoff_path(token));
    let mut output = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(package.path())
        .await
        .map_err(|error| {
            format!("DS5-PKG-002: unable to create the component handoff file: {error}")
        })?;
    let mut limited = reader.take(size);
    let copied = tokio::io::copy(&mut limited, &mut output)
        .await
        .map_err(|error| {
            format!("DS5-PKG-002: unable to receive the selected component package: {error}")
        })?;
    if copied != size {
        return Err("DS5-PKG-002: the selected component package transfer ended early".to_string());
    }
    output.flush().await.map_err(|error| {
        format!("DS5-PKG-002: unable to finish the component package transfer: {error}")
    })?;
    Ok(package)
}

#[cfg(target_os = "windows")]
async fn run_elevated_helper(operation: ElevatedOperation, token: uuid::Uuid) -> i32 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let Ok(pipe) = connect_elevated_pipe(token).await else {
        return 3;
    };
    let (mut pipe_reader, mut pipe_writer) = tokio::io::split(pipe);
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
    let local_package = if operation == ElevatedOperation::InstallLocal {
        receive_local_sidecar_package(&mut pipe_reader, token)
            .await
            .map(Some)
    } else {
        Ok(None)
    };
    // After the optional package handoff, EOF means that the parent timed out
    // or failed. Long-running child processes are job-bound and terminate when
    // Windows closes this process's handles.
    let disconnect_watcher = tokio::spawn(async move {
        let mut control = [0u8; 1];
        let _ = pipe_reader.read(&mut control).await;
        std::process::exit(5);
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
            ElevatedOperation::Install | ElevatedOperation::InstallLocal => {
                let local_package = local_package?;
                serde_json::to_value(
                    dualsense_install_impl(
                        &progress,
                        local_package.as_ref().map(TemporaryPackageFile::path),
                    )
                    .await?,
                )
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
    if succeeded {
        0
    } else {
        1
    }
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
    selected_package: Option<&Path>,
) -> Result<serde_json::Value, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ServerOptions;

    let token = uuid::Uuid::new_v4();
    let mut local_package = if operation == ElevatedOperation::InstallLocal {
        let source = selected_package.ok_or_else(|| {
            "DS5-PKG-002: no local DualSense component package was selected".to_string()
        })?;
        let file = tokio::fs::File::open(source).await.map_err(|error| {
            format!("DS5-PKG-002: unable to open the selected component package: {error}")
        })?;
        let metadata = file.metadata().await.map_err(|error| {
            format!("DS5-PKG-002: unable to inspect the selected component package: {error}")
        })?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_SIDECAR_PACKAGE_BYTES
        {
            return Err("DS5-PKG-002: the selected component package is invalid".to_string());
        }
        Some((file, metadata.len()))
    } else {
        if selected_package.is_some() {
            return Err("DS5-PKG-002: a local package is not valid for this operation".to_string());
        }
        None
    };
    let pipe_name = elevated_pipe_name(token);
    let mut server = ServerOptions::new()
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
    if let Some((file, size)) = local_package.take() {
        server
            .write_all(&size.to_le_bytes())
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to transfer the selected component package: {error}")
            })?;
        let mut limited = file.take(size);
        let copied = tokio::io::copy(&mut limited, &mut server)
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to transfer the selected component package: {error}")
            })?;
        if copied != size {
            return Err(
                "DS5-PKG-002: the selected component package changed while it was being transferred"
                    .to_string(),
            );
        }
        server.flush().await.map_err(|error| {
            format!("DS5-PKG-002: unable to finish the component package transfer: {error}")
        })?;
    }

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
pub async fn dualsense_install(
    app: tauri::AppHandle,
    package_path: Option<String>,
) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    let selected_package = package_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from);
    #[cfg(target_os = "windows")]
    {
        let operation = if selected_package.is_some() {
            ElevatedOperation::InstallLocal
        } else {
            ElevatedOperation::Install
        };
        let data =
            run_elevated_operation(Some(&app), operation, selected_package.as_deref()).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let progress = |stage: &str, value: u32| emit_progress(&app, stage, value);
        dualsense_install_impl(&progress, selected_package.as_deref()).await
    }
}

#[tauri::command]
pub async fn dualsense_set_config(
    enabled: bool,
    audio_haptics: bool,
    genshin_compatibility: bool,
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
        if genshin_compatibility && !probe.genshin_compatibility_identity {
            return Err(
                "DS5-PROTO-001: the installed sidecar does not support Genshin compatibility mode"
                    .to_string(),
            );
        }
        validate_requested_profile(
            enabled,
            audio_haptics,
            genshin_compatibility,
            usbip_available,
        )?;
    } else {
        validate_requested_profile(enabled, audio_haptics, genshin_compatibility, false)?;
    }
    let snapshot = get_core_ds5_settings().await?;
    let mut settings = snapshot.response.settings;
    update_config_fields(
        &mut settings,
        enabled,
        audio_haptics,
        genshin_compatibility,
    );
    let entity_tag = require_entity_tag(snapshot.entity_tag)?;
    let applied = tokio::time::timeout(
        CONFIG_APPLY_TIMEOUT,
        save_core_ds5_settings(settings, entity_tag),
    )
    .await
    .map_err(|_| {
        "DS5-CFG-004: timed out while applying DualSense configuration; the resulting state is unknown"
            .to_string()
    })??;
    dualsense_get_status_with_config(Some(applied.response)).await
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
        return run_elevated_operation(None, operation, None).await;
    }
    #[cfg(not(target_os = "windows"))]
    {
        dualsense_self_test_impl(profile).await
    }
}

async fn dualsense_uninstall_impl() -> Result<DualSenseStatus, String> {
    let reset_result: Result<(), String> = async {
        let snapshot = get_core_ds5_settings().await?;
        let mut settings = snapshot.response.settings;
        update_config_fields(&mut settings, false, true, false);
        let entity_tag = require_entity_tag(snapshot.entity_tag)?;
        save_core_ds5_settings(settings, entity_tag).await?;
        Ok(())
    }
    .await;
    if let Err(error) = reset_result {
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
        let data = run_elevated_operation(None, ElevatedOperation::Uninstall, None).await?;
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
        clamp_tuning, classify_usbip_installer_exit_code, component_state, component_test_failure,
        component_matches_current_runtime, component_update_available, core_ds5_http_error,
        copy_runtime_files, extract_sidecar_package, local_uninstalled_status,
        pinned_usbip_installed, require_entity_tag, resolve_core_config,
        rollback_activated_component, sha256_file, update_config_fields, update_tuning_fields,
        validate_core_ds5_response, validate_requested_profile, validate_strong_entity_tag,
        validate_sidecar_package_manifest, CoreDualSenseResponse, CoreDualSenseSettings,
        InstalledComponentManifest, SidecarPackageManifest, UsbipInstallResult,
    };
    #[cfg(target_os = "windows")]
    use super::{
        elevated_pipe_name, read_limited_elevated_line, wait_for_elevated_pipe_connection,
        ElevatedMessage, ElevatedOperation, MAX_ELEVATED_MESSAGE_BYTES,
    };
    use reqwest::header::HeaderValue;
    use std::io::Write as _;
    use std::process::Command;

    fn write_valid_component(directory: &std::path::Path, marker: &[u8]) {
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join(super::SIDECAR_EXE), marker).unwrap();
        std::fs::write(directory.join("HIDMaestro.Core.dll"), marker).unwrap();
        std::fs::write(
            directory.join("runtime.json"),
            serde_json::json!({
                "component_version": "1.0.0",
                "protocol": super::PROTOCOL_VERSION,
                "target": super::SIDECAR_PACKAGE_TARGET,
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            directory.join("component.json"),
            serde_json::json!({
                "component_version": "1.0.0",
                "hidmaestro_version": "1.6.1",
                "sha256": sha256_file(&directory.join("HIDMaestro.Core.dll")).unwrap(),
                "protocol": super::PROTOCOL_VERSION,
                "sidecar_file": super::SIDECAR_EXE,
                "sidecar_sha256": sha256_file(&directory.join(super::SIDECAR_EXE)).unwrap(),
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn composite_profile_requires_usbip() {
        let error = validate_requested_profile(true, true, false, false).unwrap_err();
        assert!(error.starts_with("DS5-RUN-003:"));
    }

    #[test]
    fn hid_only_profile_remains_available_without_usbip() {
        assert!(validate_requested_profile(true, false, false, false).is_ok());
        assert!(validate_requested_profile(false, true, false, false).is_ok());
    }

    #[test]
    fn genshin_compatibility_requires_enabled_composite_profile() {
        assert!(validate_requested_profile(true, true, true, true).is_ok());
        let error = validate_requested_profile(true, false, true, true).unwrap_err();
        assert!(error.starts_with("DS5-RUN-004:"));
        let error = validate_requested_profile(false, true, true, true).unwrap_err();
        assert!(error.starts_with("DS5-RUN-004:"));
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
        assert_eq!(component_state(true, true, true, true, true), "in_use");
        assert_eq!(
            component_state(false, false, false, true, false),
            "not_installed"
        );
        assert_eq!(
            component_state(true, false, true, false, true),
            "repair_required"
        );
        assert_eq!(
            component_state(true, true, true, false, true),
            "update_available"
        );
        assert_eq!(
            component_state(true, true, false, false, false),
            "transport_missing"
        );
        assert_eq!(component_state(true, true, true, false, false), "ready");
    }

    #[test]
    fn component_status_separates_updates_from_same_version_repairs() {
        let current = InstalledComponentManifest {
            component_version: super::COMPONENT_VERSION.to_string(),
            hidmaestro_version: super::HIDMAESTRO_VERSION.to_string(),
            sha256: super::HIDMAESTRO_SHA256.to_uppercase(),
            protocol: super::PROTOCOL_VERSION,
            sidecar_file: super::SIDECAR_EXE.to_string(),
            sidecar_sha256: "a".repeat(64),
        };
        assert!(!component_update_available(Some(&current)));
        assert!(component_matches_current_runtime(Some(&current), true, true));
        assert!(!component_matches_current_runtime(Some(&current), false, true));
        assert!(!component_matches_current_runtime(Some(&current), true, false));
        assert!(!component_update_available(None));
        assert!(!component_matches_current_runtime(None, true, true));

        let outdated = InstalledComponentManifest {
            component_version: "1.0.0".to_string(),
            ..current
        };
        assert!(component_update_available(Some(&outdated)));
        assert!(!component_matches_current_runtime(Some(&outdated), true, true));
    }

    #[test]
    fn sidecar_package_manifest_requires_pinned_release_asset() {
        let manifest = SidecarPackageManifest {
            schema: 1,
            component_version: super::COMPONENT_VERSION.to_string(),
            protocol: super::PROTOCOL_VERSION,
            target: super::SIDECAR_PACKAGE_TARGET.to_string(),
            license: super::SIDECAR_PACKAGE_LICENSE.to_string(),
            asset_name: super::SIDECAR_PACKAGE_ASSET.to_string(),
            download_url: format!(
                "https://github.com/AlkaidLab/foundation-sunshine/releases/download/v1/{}",
                super::SIDECAR_PACKAGE_ASSET
            ),
            sha256: "a".repeat(64),
            size: 1024,
        };
        assert!(validate_sidecar_package_manifest(manifest.clone()).is_ok());

        let untrusted = SidecarPackageManifest {
            download_url: format!("https://example.com/{}", super::SIDECAR_PACKAGE_ASSET),
            ..manifest.clone()
        };
        assert!(validate_sidecar_package_manifest(untrusted).is_err());

        let wrong_license = SidecarPackageManifest {
            license: "MIT".to_string(),
            ..manifest
        };
        assert!(validate_sidecar_package_manifest(wrong_license).is_err());
    }

    #[test]
    fn sidecar_package_extracts_only_matching_runtime() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let archive_path = root.join("sidecar.zip");
        let staging = root.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file(super::SIDECAR_EXE, options).unwrap();
        archive.write_all(b"MZ-test").unwrap();
        archive.start_file("runtime.json", options).unwrap();
        archive
            .write_all(
                serde_json::json!({
                    "component_version": super::COMPONENT_VERSION,
                    "protocol": super::PROTOCOL_VERSION,
                    "target": super::SIDECAR_PACKAGE_TARGET,
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();
        archive.finish().unwrap();

        let manifest = SidecarPackageManifest {
            schema: 1,
            component_version: super::COMPONENT_VERSION.to_string(),
            protocol: super::PROTOCOL_VERSION,
            target: super::SIDECAR_PACKAGE_TARGET.to_string(),
            license: super::SIDECAR_PACKAGE_LICENSE.to_string(),
            asset_name: super::SIDECAR_PACKAGE_ASSET.to_string(),
            download_url: String::new(),
            sha256: sha256_file(&archive_path).unwrap(),
            size: std::fs::metadata(&archive_path).unwrap().len(),
        };
        extract_sidecar_package(&archive_path, &staging, &manifest).unwrap();
        assert!(staging.join(super::SIDECAR_EXE).is_file());
        assert!(staging.join("runtime.json").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundled_runtime_rejects_stale_component_metadata() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let source = root.join("source");
        let destination = root.join("destination");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(source.join(super::SIDECAR_EXE), b"MZ-test").unwrap();
        std::fs::write(
            source.join("runtime.json"),
            serde_json::json!({
                "component_version": "1.0.0-build-runtime",
                "protocol": super::PROTOCOL_VERSION,
                "target": super::SIDECAR_PACKAGE_TARGET,
            })
            .to_string(),
        )
        .unwrap();

        let error = copy_runtime_files(&source, &destination).unwrap_err();

        assert!(error.contains("runtime metadata does not match Sunshine"));
        assert!(!destination.join(super::SIDECAR_EXE).exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn final_verification_rollback_restores_the_previous_component() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let active = root.join("active");
        let backup = root.join("previous");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::write(active.join("runtime.txt"), b"failed").unwrap();
        write_valid_component(&backup, b"previous");

        rollback_activated_component(&active, &backup, true).unwrap();

        assert_eq!(
            std::fs::read(active.join(super::SIDECAR_EXE)).unwrap(),
            b"previous"
        );
        assert!(!backup.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn final_verification_rollback_removes_a_failed_first_install() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let active = root.join("active");
        let backup = root.join("previous");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::write(active.join("runtime.txt"), b"failed").unwrap();

        rollback_activated_component(&active, &backup, false).unwrap();

        assert!(!active.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn final_verification_rollback_rejects_a_corrupt_backup() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let active = root.join("active");
        let backup = root.join("previous");
        std::fs::create_dir_all(&active).unwrap();
        write_valid_component(&backup, b"previous");
        std::fs::write(backup.join(super::SIDECAR_EXE), b"tampered").unwrap();

        let error = rollback_activated_component(&active, &backup, true).unwrap_err();

        assert!(error.contains("backup sidecar integrity check failed"));
        assert!(!active.exists());
        assert!(backup.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_handoff_cleanup_preserves_current_and_unrelated_entries() {
        let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
        let stale = root.join("handoff-stale.partial.zip");
        let current = root.join("handoff-current.partial.zip");
        let unrelated = root.join("other.partial.zip");
        let matching_directory = root.join("handoff-directory.partial.zip");
        std::fs::create_dir_all(&matching_directory).unwrap();
        std::fs::write(&stale, b"stale").unwrap();
        std::fs::write(&current, b"current").unwrap();
        std::fs::write(&unrelated, b"unrelated").unwrap();

        super::purge_stale_handoff_packages(&root, Some(&current));

        assert!(!stale.exists());
        assert!(current.is_file());
        assert!(unrelated.is_file());
        assert!(matching_directory.is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn core_settings_payload_contains_only_independent_ds5_fields() {
        let payload = serde_json::to_value(CoreDualSenseSettings {
            ds5_enabled: true,
            ds5_audio_haptics: false,
            ds5_legacy_haptics_strength: 1.5,
            ds5_legacy_haptics_curve: 0.5,
            ds5_legacy_haptics_noise_gate: 0.006,
            ds5_genshin_compatibility: false,
        })
        .unwrap();
        let object = payload.as_object().unwrap();
        assert_eq!(object.len(), 6);
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
    fn conditional_updates_require_a_short_strong_entity_tag() {
        let strong = HeaderValue::from_static("\"ds5-v1-2\"");
        assert_eq!(validate_strong_entity_tag(strong.clone()).unwrap(), strong);
        for invalid in [
            HeaderValue::from_static("W/\"ds5-v1-2\""),
            HeaderValue::from_static("ds5-v1-2"),
            HeaderValue::from_static("\"first\", \"second\""),
        ] {
            assert!(validate_strong_entity_tag(invalid).is_err());
        }
        let oversized_text = format!("\"{}\"", "x".repeat(512));
        let oversized = HeaderValue::from_bytes(oversized_text.as_bytes()).unwrap();
        assert!(validate_strong_entity_tag(oversized).is_err());
        assert!(require_entity_tag(None)
            .unwrap_err()
            .starts_with("DS5-CFG-007:"));
    }

    #[test]
    fn conditional_update_errors_keep_stable_user_facing_codes() {
        assert!(core_ds5_http_error(
            Some("ds5_precondition_failed"),
            reqwest::StatusCode::PRECONDITION_FAILED,
        )
        .starts_with("DS5-CFG-006:"));
        assert!(core_ds5_http_error(
            Some("ds5_precondition_required"),
            reqwest::StatusCode::from_u16(428).unwrap(),
        )
        .starts_with("DS5-CFG-007:"));
        assert!(core_ds5_http_error(
            Some("ds5_if_match_invalid"),
            reqwest::StatusCode::BAD_REQUEST,
        )
        .starts_with("DS5-CFG-007:"));
    }

    #[tokio::test]
    async fn confirmed_save_result_skips_a_failing_config_refresh() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fetch_called = AtomicBool::new(false);
        let confirmed = CoreDualSenseResponse {
            status: true,
            applied: true,
            revision: 12,
            changed: Some(true),
            settings: CoreDualSenseSettings {
                ds5_enabled: true,
                ds5_audio_haptics: false,
                ds5_legacy_haptics_strength: 1.4,
                ds5_legacy_haptics_curve: 0.7,
                ds5_legacy_haptics_noise_gate: 0.008,
                ds5_genshin_compatibility: false,
            },
        };

        let (settings, revision, error) = resolve_core_config(Some(confirmed), || async {
            fetch_called.store(true, Ordering::Relaxed);
            Err("DS5-CFG-001: simulated refresh failure".to_string())
        })
        .await;

        assert!(!fetch_called.load(Ordering::Relaxed));
        assert!(settings.ds5_enabled);
        assert!(!settings.ds5_audio_haptics);
        assert_eq!(settings.ds5_legacy_haptics_strength, 1.4);
        assert_eq!(settings.ds5_legacy_haptics_curve, 0.7);
        assert_eq!(settings.ds5_legacy_haptics_noise_gate, 0.008);
        assert_eq!(revision, 12);
        assert!(error.is_empty());
    }

    #[test]
    fn field_updates_preserve_unrelated_settings() {
        let mut settings = CoreDualSenseSettings::default();
        update_tuning_fields(&mut settings, 1.25, 0.7, 0.006);
        update_config_fields(&mut settings, true, true, true);
        assert!(settings.ds5_enabled);
        assert!(settings.ds5_audio_haptics);
        assert!(settings.ds5_genshin_compatibility);
        assert_eq!(settings.ds5_legacy_haptics_strength, 1.25);
        assert_eq!(settings.ds5_legacy_haptics_curve, 0.7);
        assert_eq!(settings.ds5_legacy_haptics_noise_gate, 0.006);

        update_tuning_fields(&mut settings, 1.5, 0.9, 0.010);
        assert!(settings.ds5_enabled);
        assert!(settings.ds5_audio_haptics);
        assert!(settings.ds5_genshin_compatibility);
        assert_eq!(settings.ds5_legacy_haptics_strength, 1.5);
        assert_eq!(settings.ds5_legacy_haptics_curve, 0.9);
        assert_eq!(settings.ds5_legacy_haptics_noise_gate, 0.010);
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

        let mut invalid_profile = valid();
        invalid_profile.settings.ds5_genshin_compatibility = true;
        assert!(validate_core_ds5_response(invalid_profile).is_err());
    }

    #[test]
    fn offline_uninstall_status_is_locally_complete() {
        let status = local_uninstalled_status();
        assert_eq!(status.state, "not_installed");
        assert!(!status.installed);
        assert!(!status.enabled);
        assert!(status.audio_haptics);
        assert!(!status.config_readable);
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
            ElevatedOperation::parse("install-local"),
            Some(ElevatedOperation::InstallLocal)
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
