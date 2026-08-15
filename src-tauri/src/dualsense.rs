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
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;
use url::Url;

const COMPONENT_VERSION: &str = "1.0.0";
const PROTOCOL_VERSION: u32 = 1;
const HIDMAESTRO_VERSION: &str = "v1.6.1";
const HIDMAESTRO_URL: &str =
    "https://github.com/hifihedgehog/HIDMaestro/releases/download/v1.6.1/HIDMaestro-v1.6.1.zip";
const HIDMAESTRO_SHA256: &str = "00145c23d9838be6089389ce58b3fd2b6766fa9bc0f1f3c60a3c885361b53c34";
const HIDMAESTRO_DOWNLOAD_BYTES: u64 = 118_879_222;
const USBIP_VERSION: &str = "0.9.7.7";
const USBIP_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
const USBIP_SHA256: &str = "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";
const MAX_SIDECAR_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_USBIP_INSTALLER_BYTES: u64 = 48 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 200 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 256;
const MAX_SIDECAR_PAYLOAD_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_COMPONENT_REDIRECTS: usize = 5;
const SIDECAR_EXE: &str = "Sunshine.Ds5Sidecar.exe";
const SIDECAR_PAYLOAD_MANIFEST: &str = "payload-manifest.json";
const COMPONENT_MANIFEST_SCHEMA: u32 = 1;
const COMPONENT_MANIFEST_NAME: &str = "sunshine-dualsense";
const COMPONENT_MANIFEST_PATH: &str = "components/dualsense.json";
#[cfg(target_os = "windows")]
const ELEVATED_DS5_ARG: &str = "--elevated-dualsense";
#[cfg(target_os = "windows")]
const MAX_ELEVATED_MESSAGE_BYTES: usize = 64 * 1024;
#[cfg(target_os = "windows")]
const ELEVATION_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
static COMPONENT_OPERATION: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));

type ProgressReporter<'a> = dyn Fn(&str, u32) + Send + Sync + 'a;

#[derive(Debug, Deserialize, Clone)]
struct ComponentManifest {
    schema: u32,
    component: String,
    component_version: String,
    architecture: String,
    sidecar_protocol: u32,
    sunshine_version: String,
    sidecar: SidecarRelease,
    hidmaestro: HidmaestroRelease,
}

#[derive(Debug, Deserialize, Clone)]
struct SidecarRelease {
    url: String,
    sha256: String,
    download_size: u64,
    expanded_size: u64,
    max_files: usize,
    entrypoint: String,
}

#[derive(Debug, Deserialize, Clone)]
struct HidmaestroRelease {
    version: String,
    url: String,
    sha256: String,
    download_size: u64,
    allow_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SidecarPayloadManifest {
    schema: u32,
    component_version: String,
    protocol: u32,
    rid: String,
    self_contained: bool,
    entrypoint: String,
    files: Vec<SidecarPayloadFile>,
}

#[derive(Debug, Deserialize)]
struct SidecarPayloadFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug)]
enum SidecarSource {
    Development(PathBuf),
    Bundled(PathBuf),
    Release(ComponentManifest),
}

impl SidecarSource {
    fn label(&self) -> &'static str {
        match self {
            Self::Development(_) => "development-override",
            Self::Bundled(_) => "bundled-legacy",
            Self::Release(_) => "release-manifest",
        }
    }

    fn component_version(&self) -> &str {
        match self {
            Self::Release(manifest) => &manifest.component_version,
            Self::Development(_) | Self::Bundled(_) => COMPONENT_VERSION,
        }
    }

    fn download_size(&self) -> u64 {
        match self {
            Self::Release(manifest) => manifest.sidecar.download_size,
            Self::Development(_) | Self::Bundled(_) => 0,
        }
    }
}

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
    pub component_version: String,
    pub runtime_version: String,
    pub install_path: String,
    pub sidecar_path: String,
    pub driver_installed: bool,
    pub usbip_available: bool,
    pub usbip_version: String,
    pub usbip_version_valid: bool,
    pub standard_profile: bool,
    pub composite_profile: bool,
    pub in_use: bool,
    pub manifest_available: bool,
    pub manifest_version: String,
    pub download_required: bool,
    pub download_bytes: u64,
    pub source: String,
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

fn component_manifest_path() -> PathBuf {
    crate::sunshine::assets_dir().join(COMPONENT_MANIFEST_PATH)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value == value.to_ascii_lowercase()
}

fn valid_release_tag(value: &str) -> bool {
    !value.is_empty()
        && value != "latest"
        && value != "."
        && value != ".."
        && !value.chars().any(|character| {
            character.is_control() || matches!(character, '/' | '\\' | '?' | '#' | '%')
        })
}

fn github_release_download_prefix(repository: &str, release_tag: &str) -> Option<String> {
    let mut url = Url::parse("https://github.com/").ok()?;
    {
        let mut segments = url.path_segments_mut().ok()?;
        segments.extend(repository.split('/'));
        segments.push("releases");
        segments.push("download");
        segments.push(release_tag);
    }
    Some(format!("{}/", url.path()))
}

fn is_pinned_github_release_url(value: &str, repository: &str, release_tag: &str) -> bool {
    if !valid_release_tag(release_tag) {
        return false;
    }
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let Some(expected_release_path) = github_release_download_prefix(repository, release_tag)
    else {
        return false;
    };
    let Some(asset) = url.path().strip_prefix(&expected_release_path) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.query().is_none()
        && url.fragment().is_none()
        && !asset.contains('/')
        && asset.starts_with("Sunshine.Ds5Sidecar.")
        && asset.ends_with(".win-x64.zip")
}

fn validate_hidmaestro_release(release: &HidmaestroRelease) -> Result<(), String> {
    let expected_files = BTreeSet::from([
        "HIDMaestro.Core.dll".to_string(),
        "LICENSE".to_string(),
        "README.md".to_string(),
        "THIRD-PARTY-NOTICES.txt".to_string(),
    ]);
    let actual_files = release.allow_files.iter().cloned().collect::<BTreeSet<_>>();
    if release.version != HIDMAESTRO_VERSION
        || release.url != HIDMAESTRO_URL
        || release.sha256 != HIDMAESTRO_SHA256
        || release.download_size != HIDMAESTRO_DOWNLOAD_BYTES
        || actual_files != expected_files
        || actual_files.len() != release.allow_files.len()
    {
        return Err(
            "DS5-MANIFEST-002: HIDMaestro release does not match the pinned component contract"
                .to_string(),
        );
    }
    Ok(())
}

fn pinned_hidmaestro_release() -> HidmaestroRelease {
    HidmaestroRelease {
        version: HIDMAESTRO_VERSION.to_string(),
        url: HIDMAESTRO_URL.to_string(),
        sha256: HIDMAESTRO_SHA256.to_string(),
        download_size: HIDMAESTRO_DOWNLOAD_BYTES,
        allow_files: vec![
            "HIDMaestro.Core.dll".to_string(),
            "LICENSE".to_string(),
            "README.md".to_string(),
            "THIRD-PARTY-NOTICES.txt".to_string(),
        ],
    }
}

fn validate_component_manifest(manifest: ComponentManifest) -> Result<ComponentManifest, String> {
    if manifest.schema != COMPONENT_MANIFEST_SCHEMA
        || manifest.component != COMPONENT_MANIFEST_NAME
        || manifest.architecture != "x86_64"
        || manifest.sidecar_protocol != PROTOCOL_VERSION
        || manifest.component_version.trim().is_empty()
        || manifest.sunshine_version.trim().is_empty()
    {
        return Err(
            "DS5-MANIFEST-002: component manifest is incompatible with this Control Panel"
                .to_string(),
        );
    }

    let sidecar = &manifest.sidecar;
    if !is_pinned_github_release_url(
        &sidecar.url,
        "AlkaidLab/foundation-sunshine",
        &manifest.sunshine_version,
    ) || !valid_sha256(&sidecar.sha256)
        || sidecar.download_size == 0
        || sidecar.download_size > MAX_SIDECAR_ARCHIVE_BYTES
        || sidecar.expanded_size == 0
        || sidecar.expanded_size > MAX_EXTRACTED_BYTES
        || sidecar.max_files == 0
        || sidecar.max_files > MAX_ARCHIVE_FILES
        || sidecar.entrypoint != SIDECAR_EXE
    {
        return Err("DS5-MANIFEST-002: sidecar release metadata is invalid".to_string());
    }
    validate_hidmaestro_release(&manifest.hidmaestro)?;
    Ok(manifest)
}

fn load_component_manifest_from_path(path: &Path) -> Result<Option<ComponentManifest>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let size = fs::metadata(path)
        .map_err(|error| format!("DS5-MANIFEST-001: unable to read component manifest: {error}"))?
        .len();
    if size > MAX_SIDECAR_PAYLOAD_MANIFEST_BYTES {
        return Err("DS5-MANIFEST-001: component manifest exceeds the size limit".to_string());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("DS5-MANIFEST-001: unable to read component manifest: {error}"))?;
    let manifest = serde_json::from_str::<ComponentManifest>(&text)
        .map_err(|error| format!("DS5-MANIFEST-001: component manifest is invalid: {error}"))?;
    validate_component_manifest(manifest).map(Some)
}

fn load_component_manifest() -> Result<Option<ComponentManifest>, String> {
    load_component_manifest_from_path(&component_manifest_path())
}

fn active_component_receipt() -> Option<serde_json::Value> {
    let receipt = active_dir().join("component.json");
    fs::read_to_string(receipt)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
}

fn active_component_source() -> String {
    active_component_receipt()
        .and_then(|value| value.get("source_kind")?.as_str().map(str::to_string))
        .unwrap_or_else(|| "bundled-legacy".to_string())
}

fn active_component_version() -> String {
    active_component_receipt()
        .and_then(|value| value.get("component_version")?.as_str().map(str::to_string))
        .unwrap_or_else(|| COMPONENT_VERSION.to_string())
}

#[cfg(debug_assertions)]
fn development_sidecar_override() -> Option<PathBuf> {
    std::env::var_os("SUNSHINE_DS5_SIDECAR_DIR").map(PathBuf::from)
}

#[cfg(not(debug_assertions))]
fn development_sidecar_override() -> Option<PathBuf> {
    None
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
    let manifest = load_component_manifest();
    let (manifest_available, manifest_version, download_bytes, manifest_error) = match manifest {
        Ok(Some(manifest)) => (
            true,
            manifest.component_version.clone(),
            manifest.sidecar.download_size,
            None,
        ),
        Ok(None) => (false, String::new(), 0, None),
        Err(error) => (false, String::new(), 0, Some(error)),
    };
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
    if !installed && let Some(error) = manifest_error {
        error_code = error
            .split(':')
            .next()
            .unwrap_or("DS5-MANIFEST-001")
            .to_string();
        detail = error;
    }
    let usbip_version = installed_usbip_version().unwrap_or_default();
    let usbip_version_valid = pinned_usbip_installed(Some(usbip_version.as_str()));
    let usbip_available = result.usbip_available && usbip_version_valid;
    let state = component_state(installed, verified, usbip_available, in_use);

    Ok(DualSenseStatus {
        state: state.to_string(),
        installed,
        verified,
        enabled,
        audio_haptics,
        component_version: installed.then(active_component_version).unwrap_or_default(),
        runtime_version: result.runtime_version,
        install_path: active_dir().to_string_lossy().to_string(),
        sidecar_path: executable.to_string_lossy().to_string(),
        driver_installed: result.driver_installed,
        usbip_available,
        usbip_version,
        usbip_version_valid,
        standard_profile: result.standard,
        composite_profile: result.composite,
        in_use,
        manifest_available,
        manifest_version,
        download_required: !installed && manifest_available,
        download_bytes,
        source: if installed {
            active_component_source()
        } else if development_sidecar_override().is_some() {
            "development-override".to_string()
        } else if manifest_available {
            "release-manifest".to_string()
        } else {
            "bundled-legacy".to_string()
        },
        error_code,
        detail,
    })
}

fn sidecar_source() -> Result<SidecarSource, String> {
    if let Some(path) = development_sidecar_override() {
        if path.join(SIDECAR_EXE).is_file() {
            return Ok(SidecarSource::Development(path));
        }
        return Err(
            "DS5-PKG-002: the development Sidecar override does not contain its entrypoint"
                .to_string(),
        );
    }

    if let Some(manifest) = load_component_manifest()? {
        return Ok(SidecarSource::Release(manifest));
    }

    bundled_sidecar_source_dir().map(SidecarSource::Bundled)
}

fn bundled_sidecar_source_dir() -> Result<PathBuf, String> {
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
            "DS5-MANIFEST-001: this Sunshine build has no component manifest or bundled DualSense sidecar runtime"
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

fn github_redirect_allowed(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some(
            "github.com"
                | "objects.githubusercontent.com"
                | "release-assets.githubusercontent.com"
                | "github-releases.githubusercontent.com"
        )
    ) && url.scheme() == "https"
}

fn validate_component_redirect(url: &Url, previous_count: usize) -> Result<(), &'static str> {
    if previous_count > MAX_COMPONENT_REDIRECTS {
        return Err("component download exceeded the redirect limit");
    }
    if !github_redirect_allowed(url) {
        return Err("component download redirected to an untrusted host");
    }
    Ok(())
}

fn component_download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::custom(
            |attempt| match validate_component_redirect(attempt.url(), attempt.previous().len()) {
                Ok(()) => attempt.follow(),
                Err(message) => attempt.error(std::io::Error::other(message)),
            },
        ))
        .user_agent("foundation-sunshine-dualsense-component")
        .build()
        .map_err(|error| format!("DS5-DL-001: unable to create component downloader: {error}"))
}

async fn download_verified_archive(
    client: &reqwest::Client,
    url: &str,
    expected_size: u64,
    expected_sha256: &str,
    destination: &Path,
    progress: &ProgressReporter<'_>,
    progress_stage: &str,
    progress_start: u32,
    progress_span: u32,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("DS5-DL-001: component download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("DS5-DL-001: component download failed: {error}"))?;
    if response
        .content_length()
        .is_some_and(|size| size > expected_size)
    {
        return Err("DS5-DL-002: component download exceeds the manifest size".to_string());
    }

    let mut output = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("DS5-DL-001: unable to create component download: {error}"))?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| format!("DS5-DL-001: component download failed: {error}"))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > expected_size {
            return Err("DS5-DL-002: component download exceeds the manifest size".to_string());
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|error| format!("DS5-DL-001: unable to save component download: {error}"))?;
        let progress_value = progress_start.saturating_add(
            (downloaded.saturating_mul(progress_span as u64) / expected_size) as u32,
        );
        report_progress(progress, progress_stage, progress_value);
    }
    output
        .flush()
        .await
        .map_err(|error| format!("DS5-DL-001: unable to save component download: {error}"))?;
    drop(output);

    if downloaded != expected_size {
        return Err("DS5-DL-002: component download size does not match the manifest".to_string());
    }
    let path = destination.to_owned();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|error| format!("DS5-PKG-002: component hash task failed: {error}"))??;
    if actual != expected_sha256 {
        return Err(
            "DS5-PKG-001: component download digest does not match the manifest".to_string(),
        );
    }
    Ok(())
}

fn canonical_payload_file_name(value: &str) -> String {
    value.to_lowercase()
}

fn flat_payload_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(':')
        && path.components().count() == 1
        && path.file_name().and_then(|name| name.to_str()) == Some(value)
        && value != "."
        && value != ".."
}

fn extract_verified_sidecar_package(
    archive_path: &Path,
    staging: &Path,
    release: &ComponentManifest,
) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(File::open(archive_path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("DS5-PKG-002: invalid Sidecar archive: {error}"))?;
    if archive.len() > release.sidecar.max_files {
        return Err("DS5-PKG-002: Sidecar archive contains too many files".to_string());
    }

    let payload = {
        let mut file = archive
            .by_name(SIDECAR_PAYLOAD_MANIFEST)
            .map_err(|_| "DS5-PKG-002: Sidecar payload manifest is missing".to_string())?;
        if file.size() > MAX_SIDECAR_PAYLOAD_MANIFEST_BYTES {
            return Err("DS5-PKG-002: Sidecar payload manifest exceeds the size limit".to_string());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| {
            format!("DS5-PKG-002: unable to read Sidecar payload manifest: {error}")
        })?;
        serde_json::from_slice::<SidecarPayloadManifest>(&bytes)
            .map_err(|error| format!("DS5-PKG-002: Sidecar payload manifest is invalid: {error}"))?
    };
    if payload.schema != COMPONENT_MANIFEST_SCHEMA
        || payload.component_version != release.component_version
        || payload.protocol != PROTOCOL_VERSION
        || payload.rid != "win-x64"
        || !payload.self_contained
        || payload.entrypoint != release.sidecar.entrypoint
    {
        return Err("DS5-PKG-002: Sidecar payload manifest is incompatible".to_string());
    }

    let mut expected_files = BTreeMap::new();
    let mut declared_bytes = 0u64;
    for file in payload.files {
        let file_size = file.size;
        let canonical_name = canonical_payload_file_name(&file.path);
        if !flat_payload_path(&file.path)
            || file.path == SIDECAR_PAYLOAD_MANIFEST
            || file.path.eq_ignore_ascii_case("HIDMaestro.Core.dll")
            || !valid_sha256(&file.sha256)
            || file.size == 0
            || expected_files.insert(canonical_name, file).is_some()
        {
            return Err("DS5-PKG-002: Sidecar payload file list is invalid".to_string());
        }
        declared_bytes = declared_bytes.saturating_add(file_size);
    }
    if expected_files.is_empty() || declared_bytes > release.sidecar.expanded_size {
        return Err("DS5-PKG-002: Sidecar payload exceeds the manifest limit".to_string());
    }

    let mut seen_files = BTreeSet::new();
    let mut extracted_bytes = 0u64;
    let mut has_payload_manifest = false;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = item.enclosed_name() else {
            return Err("DS5-PKG-002: Sidecar archive contains an unsafe path".to_string());
        };
        if item.is_dir() || relative.components().count() != 1 {
            return Err("DS5-PKG-002: Sidecar archive must contain only root files".to_string());
        }
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| flat_payload_path(name))
            .ok_or_else(|| {
                "DS5-PKG-002: Sidecar archive contains an unsafe file name".to_string()
            })?;
        let canonical_name = canonical_payload_file_name(name);
        let is_payload_manifest = name.eq_ignore_ascii_case(SIDECAR_PAYLOAD_MANIFEST);
        if is_payload_manifest {
            if has_payload_manifest {
                return Err(
                    "DS5-PKG-002: Sidecar archive contains duplicate payload manifests".to_string(),
                );
            }
            has_payload_manifest = true;
        } else {
            let expected = expected_files.get(&canonical_name).ok_or_else(|| {
                "DS5-PKG-002: Sidecar archive contains an unexpected file".to_string()
            })?;
            if item.size() != expected.size || !seen_files.insert(canonical_name.clone()) {
                return Err(
                    "DS5-PKG-002: Sidecar archive file list does not match the payload manifest"
                        .to_string(),
                );
            }
        }
        extracted_bytes = extracted_bytes.saturating_add(item.size());
        if extracted_bytes > release.sidecar.expanded_size {
            return Err("DS5-PKG-002: Sidecar archive exceeds the extraction limit".to_string());
        }
        let output_path = staging.join(name);
        let mut output = File::create(&output_path)
            .map_err(|error| format!("DS5-PKG-002: unable to extract Sidecar file: {error}"))?;
        std::io::copy(&mut item, &mut output)
            .map_err(|error| format!("DS5-PKG-002: unable to extract Sidecar file: {error}"))?;
        drop(output);
        if !is_payload_manifest {
            let expected = expected_files
                .get(&canonical_name)
                .ok_or_else(|| "DS5-PKG-002: Sidecar archive file list is invalid".to_string())?;
            if sha256_file(&output_path)? != expected.sha256 {
                return Err("DS5-PKG-002: extracted Sidecar file digest mismatch".to_string());
            }
        }
    }
    if !has_payload_manifest
        || seen_files.len() != expected_files.len()
        || !staging.join(SIDECAR_EXE).is_file()
    {
        return Err("DS5-PKG-002: Sidecar archive is missing required files".to_string());
    }
    Ok(())
}

fn extract_verified_hidmaestro_package(
    archive_path: &Path,
    staging: &Path,
    release: &HidmaestroRelease,
) -> Result<(), String> {
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
    if actual != release.sha256 {
        return Err(format!(
            "DS5-PKG-001: expected HIDMaestro digest {}, got {actual}",
            release.sha256
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
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    if installed_usbip_version().as_deref() == Some(USBIP_VERSION) {
        return Ok(());
    }

    report_progress(progress, "transport_downloading", 3);
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

        let mut output = tokio::fs::File::create(&installer_path)
            .await
            .map_err(|error| error.to_string())?;
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
                .await
                .map_err(|error| error.to_string())?;
        }
        output.flush().await.map_err(|error| error.to_string())?;
        drop(output);

        let hash_path = installer_path.clone();
        let actual = tokio::task::spawn_blocking(move || sha256_file(&hash_path))
            .await
            .map_err(|error| format!("DS5-DRV-001: USB/IP hash task failed: {error}"))?
            .map_err(|error| format!("DS5-DRV-001: unable to hash USB/IP installer: {error}"))?;
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
    _progress: &ProgressReporter<'_>,
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

async fn dualsense_install_impl(
    progress: &ProgressReporter<'_>,
) -> Result<DualSenseStatus, String> {
    let previous_enabled = config_bool("ds5_enabled", false);
    let previous_audio_haptics = config_bool("ds5_audio_haptics", false);
    report_progress(progress, "preparing", 1);
    let source = sidecar_source()?;
    let root = component_root();
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let operation = format!("staging-{}", uuid::Uuid::new_v4());
    let staging = root.join(&operation);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let hidmaestro_archive = root.join(format!("{operation}.hidmaestro.partial"));
    let sidecar_archive = root.join(format!("{operation}.sidecar.partial"));
    let active = active_dir();
    let backup = root.join("previous");
    let had_previous = active.exists();
    let client = component_download_client()?;
    let hidmaestro = match &source {
        SidecarSource::Release(manifest) => manifest.hidmaestro.clone(),
        SidecarSource::Development(_) | SidecarSource::Bundled(_) => pinned_hidmaestro_release(),
    };

    let install_result: Result<(), String> = async {
        ensure_pinned_usbip(progress, &client, &root).await?;
        match &source {
            SidecarSource::Development(path) | SidecarSource::Bundled(path) => {
                copy_runtime_files(path, &staging)?;
            }
            SidecarSource::Release(manifest) => {
                report_progress(progress, "downloading_sidecar", 12);
                download_verified_archive(
                    &client,
                    &manifest.sidecar.url,
                    manifest.sidecar.download_size,
                    &manifest.sidecar.sha256,
                    &sidecar_archive,
                    progress,
                    "downloading_sidecar",
                    12,
                    31,
                )
                .await?;
                report_progress(progress, "verifying", 44);
                let archive = sidecar_archive.clone();
                let destination = staging.clone();
                let manifest = manifest.clone();
                tokio::task::spawn_blocking(move || {
                    extract_verified_sidecar_package(&archive, &destination, &manifest)
                })
                .await
                .map_err(|error| error.to_string())??;
            }
        }

        report_progress(progress, "downloading_hidmaestro", 45);
        download_verified_archive(
            &client,
            &hidmaestro.url,
            hidmaestro.download_size,
            &hidmaestro.sha256,
            &hidmaestro_archive,
            progress,
            "downloading_hidmaestro",
            45,
            31,
        )
        .await?;
        report_progress(progress, "verifying", 77);
        let archive = hidmaestro_archive.clone();
        let destination = staging.clone();
        let hidmaestro_for_extract = hidmaestro.clone();
        tokio::task::spawn_blocking(move || {
            extract_verified_hidmaestro_package(&archive, &destination, &hidmaestro_for_extract)
        })
        .await
        .map_err(|error| error.to_string())??;
        report_progress(progress, "probing", 88);
        let probe_executable = staging.join(SIDECAR_EXE);
        let probe_path = probe_executable.clone();
        tokio::task::spawn_blocking(move || run_probe(&probe_path))
            .await
            .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))??;
        let sidecar_for_hash = probe_executable.clone();
        let sidecar_sha256 = tokio::task::spawn_blocking(move || sha256_file(&sidecar_for_hash))
            .await
            .map_err(|error| format!("DS5-PKG-002: component hash task failed: {error}"))??;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "component_version": source.component_version(),
                "source_kind": source.label(),
                "sidecar_source": match &source {
                    SidecarSource::Release(manifest) => manifest.sidecar.url.as_str(),
                    SidecarSource::Development(_) | SidecarSource::Bundled(_) => "bundled",
                },
                "hidmaestro_version": hidmaestro.version,
                "hidmaestro_source": hidmaestro.url,
                "hidmaestro_sha256": hidmaestro.sha256,
                "protocol": PROTOCOL_VERSION,
                "sidecar_file": SIDECAR_EXE,
                "sidecar_sha256": sidecar_sha256,
                "sidecar_archive_sha256": match &source {
                    SidecarSource::Release(manifest) => manifest.sidecar.sha256.as_str(),
                    SidecarSource::Development(_) | SidecarSource::Bundled(_) => "",
                },
                "sidecar_download_size": source.download_size()
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
        Ok(())
    }
    .await;
    let _ = fs::remove_file(&hidmaestro_archive);
    let _ = fs::remove_file(&sidecar_archive);
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
    report_progress(progress, "complete", 100);
    dualsense_get_status().await
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
    // failed, so terminate this elevated helper instead of leaving an orphaned
    // operation running at high integrity.
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
    let writer_result = writer.await;
    disconnect_watcher.abort();
    let _ = disconnect_watcher.await;
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
async fn run_elevated_operation(
    app: Option<&tauri::AppHandle>,
    operation: ElevatedOperation,
) -> Result<serde_json::Value, String> {
    use tokio::io::BufReader;
    use tokio::net::windows::named_pipe::ServerOptions;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let token = uuid::Uuid::new_v4();
    let pipe_name = elevated_pipe_name(token);
    let server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(true)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe_name)
        .map_err(|error| format!("DS5-PKG-004: unable to create administrator IPC: {error}"))?;
    let executable = std::env::current_exe()
        .map_err(|error| format!("DS5-PKG-004: unable to locate Control Panel: {error}"))?;
    let executable = executable.to_string_lossy().replace('\'', "''");
    let ps_script = format!(
        "$ErrorActionPreference = 'Stop'; try {{ $p = Start-Process -FilePath '{executable}' -ArgumentList '{ELEVATED_DS5_ARG}','{}','{token}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1223 }}",
        operation.as_arg()
    );
    let mut launcher = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| {
            format!("DS5-PKG-004: unable to request administrator authorization: {error}")
        })?;

    tokio::select! {
        connected = server.connect() => connected.map_err(|error| {
            format!("DS5-PKG-004: administrator IPC connection failed: {error}")
        })?,
        status = launcher.wait() => {
            let status = status.map_err(|error| error.to_string())?;
            return Err(format!(
                "DS5-PKG-004: administrator authorization was canceled or the helper exited early ({})",
                status.code().unwrap_or(-1)
            ));
        },
        _ = tokio::time::sleep(ELEVATION_CONNECT_TIMEOUT) => {
            let _ = launcher.start_kill();
            let _ = launcher.wait().await;
            return Err("DS5-PKG-004: administrator authorization timed out".to_string());
        },
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
            if tokio::time::timeout(std::time::Duration::from_secs(5), launcher.wait())
                .await
                .is_err()
            {
                let _ = launcher.start_kill();
                let _ = launcher.wait().await;
            }
            return Err(error);
        }
        Err(_) => {
            // Dropping the timed-out receive future closes the pipe and causes
            // the helper to terminate itself even though this process cannot
            // directly kill a high-integrity child.
            if tokio::time::timeout(std::time::Duration::from_secs(5), launcher.wait())
                .await
                .is_err()
            {
                let _ = launcher.start_kill();
                let _ = launcher.wait().await;
            }
            return Err("DS5-PKG-004: administrator operation timed out".to_string());
        }
    };
    let status = launcher.wait().await.map_err(|error| error.to_string())?;
    match final_result {
        Some(Ok(data)) if status.success() => Ok(data),
        Some(Err(error)) => Err(error),
        _ => Err(format!(
            "DS5-PKG-004: administrator helper failed with exit code {}",
            status.code().unwrap_or(-1)
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
    if !crate::bat_runner::is_elevated() {
        let data = run_elevated_operation(Some(&app), ElevatedOperation::Install).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    let progress = |stage: &str, value: u32| emit_progress(&app, stage, value);
    dualsense_install_impl(&progress).await
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
    if !crate::bat_runner::is_elevated() {
        let operation = if profile == "composite" {
            ElevatedOperation::TestComposite
        } else {
            ElevatedOperation::TestStandard
        };
        return run_elevated_operation(None, operation).await;
    }
    dualsense_self_test_impl(profile).await
}

async fn dualsense_uninstall_impl() -> Result<DualSenseStatus, String> {
    apply_config(false, true, None, true).await?;
    let root = component_root();
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("DS5-PKG-002: unable to remove component: {error}"))?;
    }
    dualsense_get_status().await
}

#[tauri::command]
pub async fn dualsense_uninstall() -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    #[cfg(target_os = "windows")]
    if !crate::bat_runner::is_elevated() {
        let data = run_elevated_operation(None, ElevatedOperation::Uninstall).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    dualsense_uninstall_impl().await
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentManifest, SidecarRelease, apply_gamepad_selection, component_state,
        component_test_failure, extract_verified_sidecar_package, flat_payload_path,
        load_component_manifest_from_path, pinned_hidmaestro_release, pinned_usbip_installed,
        valid_release_tag, validate_component_manifest, validate_component_redirect,
        validate_requested_profile,
    };
    #[cfg(target_os = "windows")]
    use super::{
        ElevatedMessage, ElevatedOperation, MAX_ELEVATED_MESSAGE_BYTES, elevated_pipe_name,
        read_limited_elevated_line,
    };
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        process::Command,
    };
    use url::Url;
    use zip::{ZipWriter, write::SimpleFileOptions};

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

    fn valid_component_manifest() -> ComponentManifest {
        ComponentManifest {
            schema: 1,
            component: "sunshine-dualsense".to_string(),
            component_version: "1.0.0+test".to_string(),
            architecture: "x86_64".to_string(),
            sidecar_protocol: 1,
            sunshine_version: "v2026.0815.0".to_string(),
            sidecar: SidecarRelease {
                url: "https://github.com/AlkaidLab/foundation-sunshine/releases/download/v2026.0815.0/Sunshine.Ds5Sidecar.1.0.0%2Btest.win-x64.zip".to_string(),
                sha256: "a".repeat(64),
                download_size: 1,
                expanded_size: 1,
                max_files: 1,
                entrypoint: "Sunshine.Ds5Sidecar.exe".to_string(),
            },
            hidmaestro: pinned_hidmaestro_release(),
        }
    }

    fn temporary_test_directory(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("sunshine-ds5-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let mut digest = Sha256::new();
        digest.update(bytes);
        format!("{:x}", digest.finalize())
    }

    fn write_test_sidecar_archive(
        path: &Path,
        declared_files: &[(&str, &[u8])],
        archive_files: &[(&str, &[u8])],
    ) -> ComponentManifest {
        let payload = serde_json::json!({
            "schema": 1,
            "component_version": "1.0.0+test",
            "protocol": 1,
            "rid": "win-x64",
            "self_contained": true,
            "entrypoint": "Sunshine.Ds5Sidecar.exe",
            "files": declared_files.iter().map(|(path, bytes)| serde_json::json!({
                "path": path,
                "sha256": sha256_bytes(bytes),
                "size": bytes.len(),
            })).collect::<Vec<_>>(),
        });
        let mut writer = ZipWriter::new(File::create(path).unwrap());
        let options = SimpleFileOptions::default();
        writer.start_file("payload-manifest.json", options).unwrap();
        writer.write_all(payload.to_string().as_bytes()).unwrap();
        for (name, bytes) in archive_files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();

        let mut manifest = valid_component_manifest();
        manifest.sidecar.max_files = archive_files.len() + 1;
        manifest.sidecar.expanded_size = 1024 * 1024;
        manifest
    }

    fn extract_test_sidecar_archive(
        label: &str,
        declared_files: &[(&str, &[u8])],
        archive_files: &[(&str, &[u8])],
    ) -> Result<(), String> {
        let root = temporary_test_directory(label);
        let archive = root.join("sidecar.zip");
        let staging = root.join("staging");
        fs::create_dir(&staging).unwrap();
        let manifest = write_test_sidecar_archive(&archive, declared_files, archive_files);
        let result = extract_verified_sidecar_package(&archive, &staging, &manifest);
        let _ = fs::remove_dir_all(root);
        result
    }

    #[test]
    fn component_manifest_requires_the_local_release_tag_and_asset_pattern() {
        assert!(validate_component_manifest(valid_component_manifest()).is_ok());

        let mut manifest = valid_component_manifest();
        manifest.sidecar.url = "https://github.com/AlkaidLab/foundation-sunshine/releases/download/latest/Sunshine.Ds5Sidecar.1.0.0.win-x64.zip".to_string();
        assert!(validate_component_manifest(manifest).is_err());

        let mut manifest = valid_component_manifest();
        manifest.sidecar.url = "https://github.com/AlkaidLab/foundation-sunshine/releases/download/v2026.0815.0/Sunshine.exe".to_string();
        assert!(validate_component_manifest(manifest).is_err());

        let mut manifest = valid_component_manifest();
        manifest.sunshine_version = "../../other/repo/releases/download/v1".to_string();
        assert!(validate_component_manifest(manifest).is_err());

        let mut manifest = valid_component_manifest();
        manifest.sunshine_version = "v2026.0815.0.杂鱼".to_string();
        manifest.sidecar.url = "https://github.com/AlkaidLab/foundation-sunshine/releases/download/v2026.0815.0.%E6%9D%82%E9%B1%BC/Sunshine.Ds5Sidecar.v2026.0815.0.win-x64.zip".to_string();
        assert!(validate_component_manifest(manifest).is_ok());

        assert!(!valid_release_tag("%2f"));
        assert!(!valid_release_tag(".."));
    }

    #[test]
    fn component_redirects_are_bounded_and_restricted_to_github_delivery_hosts() {
        let github = Url::parse("https://github.com/AlkaidLab/foundation-sunshine").unwrap();
        let untrusted = Url::parse("https://example.invalid/component.zip").unwrap();
        assert!(validate_component_redirect(&github, 5).is_ok());
        assert!(validate_component_redirect(&github, 6).is_err());
        assert!(validate_component_redirect(&untrusted, 0).is_err());
    }

    #[test]
    fn component_manifest_read_has_a_size_limit() {
        let root = temporary_test_directory("manifest-limit");
        let path = root.join("dualsense.json");
        fs::write(
            &path,
            vec![b' '; super::MAX_SIDECAR_PAYLOAD_MANIFEST_BYTES as usize + 1],
        )
        .unwrap();
        let error = load_component_manifest_from_path(&path).unwrap_err();
        let _ = fs::remove_dir_all(root);
        assert!(error.contains("exceeds the size limit"));
    }

    #[test]
    fn sidecar_archive_extracts_a_complete_verified_payload() {
        let root = temporary_test_directory("valid-archive");
        let archive = root.join("sidecar.zip");
        let staging = root.join("staging");
        fs::create_dir(&staging).unwrap();
        let files = [
            ("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice()),
            ("hostfxr.dll", b"runtime".as_slice()),
        ];
        let manifest = write_test_sidecar_archive(&archive, &files, &files);
        extract_verified_sidecar_package(&archive, &staging, &manifest).unwrap();
        assert_eq!(
            fs::read(staging.join("Sunshine.Ds5Sidecar.exe")).unwrap(),
            b"sidecar"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sidecar_archive_rejects_missing_and_unexpected_files() {
        let declared = [
            ("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice()),
            ("hostfxr.dll", b"runtime".as_slice()),
        ];
        let missing = [("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice())];
        assert!(
            extract_test_sidecar_archive("missing-file", &declared, &missing)
                .unwrap_err()
                .contains("missing required files")
        );

        let declared = [("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice())];
        let extra = [
            ("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice()),
            ("unexpected.dll", b"extra".as_slice()),
        ];
        assert!(
            extract_test_sidecar_archive("unexpected-file", &declared, &extra)
                .unwrap_err()
                .contains("unexpected file")
        );
    }

    #[test]
    fn sidecar_archive_rejects_size_digest_and_case_collisions() {
        let declared = [("Sunshine.Ds5Sidecar.exe", b"abcdef".as_slice())];
        let size_mismatch = [("Sunshine.Ds5Sidecar.exe", b"short".as_slice())];
        assert!(
            extract_test_sidecar_archive("size-mismatch", &declared, &size_mismatch)
                .unwrap_err()
                .contains("file list does not match")
        );

        let digest_mismatch = [("Sunshine.Ds5Sidecar.exe", b"ghijkl".as_slice())];
        assert!(
            extract_test_sidecar_archive("digest-mismatch", &declared, &digest_mismatch)
                .unwrap_err()
                .contains("digest mismatch")
        );

        let declared = [("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice())];
        let collision = [
            ("Sunshine.Ds5Sidecar.exe", b"sidecar".as_slice()),
            ("sunshine.ds5sidecar.exe", b"runtime".as_slice()),
        ];
        assert!(
            extract_test_sidecar_archive("case-collision", &declared, &collision)
                .unwrap_err()
                .contains("file list does not match")
        );
    }

    #[test]
    fn component_payload_paths_must_be_single_file_names() {
        assert!(flat_payload_path("Sunshine.Ds5Sidecar.exe"));
        assert!(!flat_payload_path("runtime/hostfxr.dll"));
        assert!(!flat_payload_path("..\\Sunshine.Ds5Sidecar.exe"));
        assert!(!flat_payload_path("C:\\component.dll"));
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
