//! Status probing, sidecar process helpers, and active-session guards.

use log::warn;
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use super::config::{CoreDualSenseResponse, get_core_ds5_settings, resolve_core_config};
use super::packages::{
    active_dir, component_matches_current_runtime, component_update_available,
    installed_component_manifest, sidecar_path, validate_installed_component_integrity,
};
use super::{
    COMPONENT_VERSION, DualSenseStatus, PROTOCOL_VERSION, USBIP_VERSION, observe_config_revision,
};

#[cfg(target_os = "windows")]
pub(crate) fn installed_usbip_version() -> Option<String> {
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
pub(crate) fn installed_usbip_version() -> Option<String> {
    None
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ProbeResult {
    pub(crate) protocol: u32,
    pub(crate) runtime_version: String,
    pub(crate) standard: bool,
    pub(crate) composite: bool,
    pub(crate) genshin_compatibility_identity: bool,
    pub(crate) audio_policy_violation: bool,
    pub(crate) driver_installed: bool,
    pub(crate) usbip_available: bool,
}

#[cfg(target_os = "windows")]
fn bind_child_process_tree(
    child: &std::process::Child,
) -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::core::PCWSTR;

    unsafe {
        let raw_job = CreateJobObjectW(None, PCWSTR::null()).map_err(|error| {
            format!("DS5-PKG-003: unable to create sidecar process job: {error}")
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
            format!("DS5-PKG-003: unable to configure sidecar process job: {error}")
        })?;
        AssignProcessToJobObject(job_handle, HANDLE(child.as_raw_handle())).map_err(|error| {
            format!("DS5-PKG-003: unable to bind sidecar process tree: {error}")
        })?;
        Ok(job)
    }
}

pub(crate) fn run_with_timeout(
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

    fn drain_pipe<R: Read + Send + 'static>(mut pipe: R) -> std::sync::mpsc::Receiver<Vec<u8>> {
        let (sender, receiver) = std::sync::mpsc::channel();
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
            let _ = sender.send(captured);
        });
        receiver
    }

    fn receive_pipe(
        receiver: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
        deadline: std::time::Instant,
    ) -> Result<Vec<u8>, ()> {
        let Some(receiver) = receiver else {
            return Ok(Vec::new());
        };
        match receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
            Ok(output) => Ok(output),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(Vec::new()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(()),
        }
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
    #[cfg(target_os = "windows")]
    let mut child_job = if capture_output {
        match bind_child_process_tree(&child) {
            Ok(job) => Some(job),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    } else {
        None
    };
    let stdout_reader = child.stdout.take().map(drain_pipe);
    let stderr_reader = child.stderr.take().map(drain_pipe);
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("DS5-PKG-003: unable to wait for sidecar: {error}"))?
        {
            #[cfg(target_os = "windows")]
            drop(child_job.take());
            let stdout =
                receive_pipe(stdout_reader, deadline).map_err(|_| timeout_error.to_string())?;
            let stderr =
                receive_pipe(stderr_reader, deadline).map_err(|_| timeout_error.to_string())?;
            return Ok(std::process::Output {
                status,
                stdout,
                stderr,
            });
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            #[cfg(target_os = "windows")]
            drop(child_job.take());
            return Err(timeout_error.to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub(crate) fn run_probe(executable: &Path) -> Result<ProbeResult, String> {
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

pub(crate) fn component_test_failure(output: &std::process::Output, result_path: &Path) -> String {
    if let Ok(contents) = fs::read_to_string(result_path)
        && let Ok(result) = serde_json::from_str::<serde_json::Value>(&contents)
        && let Some(error) = result.get("error").and_then(serde_json::Value::as_str)
        && let Some(summary) = error.lines().find(|line| !line.trim().is_empty())
    {
        return format!("DS5-PKG-003: component test failed: {}", summary.trim());
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

pub(crate) async fn has_active_session() -> bool {
    crate::sunshine::get_active_sessions()
        .await
        .map(|sessions| !sessions.is_empty())
        .unwrap_or(false)
}

pub(crate) async fn ensure_no_active_session() -> Result<(), String> {
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

pub(crate) fn component_state(
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
    } else if update_available {
        "update_available"
    } else if !verified {
        "repair_required"
    } else if !transport_available {
        "transport_missing"
    } else {
        "ready"
    }
}

pub(crate) async fn ensure_no_active_session_for_uninstall() -> Result<(), String> {
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

pub(crate) fn local_uninstalled_status() -> DualSenseStatus {
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

pub(crate) fn validate_requested_profile(
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

pub(crate) fn pinned_usbip_installed(installed_version: Option<&str>) -> bool {
    installed_version == Some(USBIP_VERSION)
}

pub(crate) async fn dualsense_get_status_with_config(
    confirmed: Option<CoreDualSenseResponse>,
) -> Result<DualSenseStatus, String> {
    let (settings, config_revision, config_error) =
        resolve_core_config(confirmed, get_core_ds5_settings).await;
    let config_readable = config_error.is_empty();
    if config_readable {
        observe_config_revision(config_revision);
    }
    let executable = sidecar_path();
    let installed = executable.is_file();
    let in_use = has_active_session().await;
    let manifest = installed.then(installed_component_manifest).flatten();
    let integrity_error = if installed {
        manifest
            .as_ref()
            .ok_or_else(|| "DS5-PKG-001: installed component manifest is missing".to_string())
            .and_then(validate_installed_component_integrity)
            .err()
    } else {
        None
    };
    let probe = if installed {
        if let Some(error) = integrity_error {
            Some(Err(error))
        } else {
            let probe_executable = executable.clone();
            Some(
                tokio::task::spawn_blocking(move || run_probe(&probe_executable))
                    .await
                    .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))
                    .and_then(|result| result),
            )
        }
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
    let update_available = installed && component_update_available(manifest.as_ref());
    let matches_current_runtime = component_matches_current_runtime(
        manifest.as_ref(),
        result.genshin_compatibility_identity,
        result.audio_policy_violation,
    );
    let verified = probe_succeeded && matches_current_runtime;
    if probe_succeeded && !matches_current_runtime {
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
