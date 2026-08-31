//! Optional DualSense component lifecycle.
//!
//! HIDMaestro is downloaded from its pinned upstream release and verified before
//! extraction. The Sunshine-owned sidecar is acquired from a release asset
//! pinned by the installed Sunshine manifest, from a matching user-selected
//! package, or from an explicit development override. This keeps the optional
//! self-contained .NET runtime out of the main Sunshine package.

mod commands;
mod config;
#[cfg(target_os = "windows")]
mod elevated;
mod install;
mod packages;
mod probe;
#[cfg(test)]
mod tests;

pub use commands::*;
#[cfg(target_os = "windows")]
pub(crate) use elevated::try_handle_elevated_command;

#[cfg(test)]
pub(crate) use {
    config::{
        CoreDualSenseResponse, CoreDualSenseSettings, clamp_tuning, core_ds5_http_error,
        require_entity_tag, resolve_core_config, update_config_fields, update_tuning_fields,
        validate_core_ds5_response, validate_strong_entity_tag,
    },
    install::{
        classify_usbip_installer_exit_code, component_fs_error, copy_runtime_files,
        extract_sidecar_package, recover_interrupted_activation, retry_component_fs_operation,
        rollback_activated_component,
    },
    packages::{
        InstalledComponentManifest, LocalComponentKind, SidecarPackageManifest, UsbipInstallResult,
        classify_local_component_packages, component_matches_current_runtime,
        component_update_available, local_component_kind, purge_stale_handoff_packages,
        read_sidecar_package_manifest, sha256_file, validate_sidecar_package_manifest,
    },
    probe::{
        component_is_verified, component_state, component_test_failure, local_uninstalled_status,
        pinned_usbip_installed, run_with_timeout, validate_requested_profile,
    },
};

#[cfg(test)]
#[cfg(target_os = "windows")]
pub(crate) use elevated::{
    ElevatedMessage, ElevatedOperation, MAX_ELEVATED_MESSAGE_BYTES, elevated_pipe_name,
    read_limited_elevated_line, receive_local_component_packages_into,
    wait_for_elevated_pipe_connection,
};

use log::info;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Emitter;

pub(crate) const COMPONENT_VERSION: &str = "1.2.0";
pub(crate) const PROTOCOL_VERSION: u32 = 1;
pub(crate) const HIDMAESTRO_VERSION: &str = "v1.6.2";
pub(crate) const HIDMAESTRO_URL: &str =
    "https://github.com/hifihedgehog/HIDMaestro/releases/download/v1.6.2/HIDMaestro-v1.6.2.zip";
pub(crate) const HIDMAESTRO_SHA256: &str =
    "6ae8df0cf317baf7e65777e2929f618916a67831b5ff1162205310f2c08b80ff";
pub(crate) const USBIP_VERSION: &str = "0.9.7.7";
pub(crate) const USBIP_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
pub(crate) const USBIP_SHA256: &str =
    "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";
pub(crate) const MAX_ARCHIVE_BYTES: u64 = 160 * 1024 * 1024;
pub(crate) const MAX_USBIP_INSTALLER_BYTES: u64 = 48 * 1024 * 1024;
pub(crate) const MAX_EXTRACTED_BYTES: u64 = 200 * 1024 * 1024;
pub(crate) const MAX_ARCHIVE_FILES: usize = 256;
pub(crate) const SIDECAR_EXE: &str = "Sunshine.Ds5Sidecar.exe";
pub(crate) const SIDECAR_PACKAGE_MANIFEST: &str = "ds5-sidecar-package.json";
pub(crate) const SIDECAR_PACKAGE_ASSET: &str = "Sunshine.Ds5Sidecar.x64.zip";
pub(crate) const SIDECAR_PACKAGE_TARGET: &str = "win-x64-self-contained";
pub(crate) const SIDECAR_PACKAGE_LICENSE: &str = "GPL-3.0-only";
pub(crate) const MAX_SIDECAR_PACKAGE_MANIFEST_BYTES: u64 = 64 * 1024;
pub(crate) const MAX_SIDECAR_PACKAGE_BYTES: u64 = 160 * 1024 * 1024;
pub(crate) const MAX_LOCAL_COMPONENT_PACKAGES: usize = 3;
pub(crate) const MAX_LOCAL_COMPONENT_PACKAGE_BYTES: u64 = MAX_ARCHIVE_BYTES;
pub(crate) const MAX_LOCAL_COMPONENT_TOTAL_BYTES: u64 =
    MAX_SIDECAR_PACKAGE_BYTES + MAX_ARCHIVE_BYTES + MAX_USBIP_INSTALLER_BYTES;
pub(crate) const COMPONENT_DOWNLOAD_OVERALL_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);
pub(crate) const COMPONENT_FS_RETRY_ATTEMPTS: usize = 20;
pub(crate) const COMPONENT_FS_RETRY_DELAY: std::time::Duration =
    std::time::Duration::from_millis(200);

pub(crate) static COMPONENT_OPERATION: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));
pub(crate) static LAST_OBSERVED_CONFIG_REVISION: AtomicU64 = AtomicU64::new(0);

pub(crate) fn observe_config_revision(revision: u64) {
    if revision == 0 {
        return;
    }

    let previous = LAST_OBSERVED_CONFIG_REVISION.swap(revision, Ordering::Relaxed);
    if previous != 0 && previous != revision {
        info!("DualSense configuration revision changed: {previous} -> {revision}");
    }
}

pub(crate) type ProgressReporter<'a> = dyn Fn(&str, u32) + Send + Sync + 'a;

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

#[derive(Debug, Serialize)]
pub struct DualSenseTuningResult {
    legacy_strength: f64,
    legacy_curve: f64,
    legacy_noise_gate: f64,
    revision: u64,
    changed: bool,
}

pub(crate) fn emit_progress(app: &tauri::AppHandle, stage: &str, progress: u32) {
    let _ = app.emit(
        "dualsense-operation-progress",
        serde_json::json!({ "stage": stage, "progress": progress.min(100) }),
    );
}

pub(crate) fn report_progress(progress: &ProgressReporter<'_>, stage: &str, value: u32) {
    progress(stage, value.min(100));
}
