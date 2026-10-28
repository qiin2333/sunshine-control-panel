//! Install, self-test, and uninstall orchestration.

use log::{info, warn};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config::{
    get_core_ds5_settings, require_entity_tag, save_core_ds5_settings, update_config_fields,
};
use super::packages::{
    ComponentDownloadSpec, InstalledComponentManifest, LocalComponentPackages,
    SidecarPackageManifest, SidecarRuntimeMetadata, UsbipInstallResult, active_dir, component_root,
    download_component_asset, download_component_asset_from_candidates,
    manually_placed_sidecar_package, purge_stale_handoff_packages, sha256_file,
    sidecar_package_manifest, sidecar_package_manifest_path, validate_core_integrity,
};
use super::probe::{component_test_failure, local_uninstalled_status, run_probe, run_with_timeout};
use super::{
    COMPONENT_FS_RETRY_ATTEMPTS, COMPONENT_FS_RETRY_DELAY, COMPONENT_VERSION, DualSenseStatus,
    HIDMAESTRO_VERSION, MAX_ARCHIVE_FILES, MAX_EXTRACTED_BYTES, MAX_SIDECAR_PACKAGE_BYTES,
    MAX_USBIP_INSTALLER_BYTES, PROTOCOL_VERSION, ProgressReporter, SIDECAR_EXE,
    SIDECAR_PACKAGE_TARGET, dualsense_get_status, report_progress,
};
use crate::usbip::{
    InstallDisposition, USBIP_SHA256, USBIP_URL, USBIP_VERSION, install_disposition,
    installed_usbip_version,
};

pub(crate) fn classify_usbip_installer_exit_code(
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

pub(crate) fn sidecar_source_dir() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(override_dir) = std::env::var_os("SUNSHINE_DS5_SIDECAR_DIR") {
        let path = PathBuf::from(override_dir);
        if path.join(SIDECAR_EXE).is_file() {
            return Ok(path);
        }
    }

    let sunshine_root = crate::sunshine::install_dir();
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

pub(crate) fn copy_runtime_files(source: &Path, destination: &Path) -> Result<(), String> {
    validate_sidecar_runtime_metadata(source)?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .map_err(|error| format!("DS5-PKG-002: unable to copy sidecar runtime: {error}"))?;
        }
    }
    validate_complete_runtime(destination)?;
    Ok(())
}

/// Requires the runtime payload, matching compatibility contract and package metadata.
pub(crate) fn validate_complete_runtime(directory: &Path) -> Result<(), String> {
    for name in [
        SIDECAR_EXE,
        "HIDMaestro.Core.dll",
        "LICENSE.HIDMaestro",
        "THIRD-PARTY-NOTICES.txt",
    ] {
        if !directory.join(name).is_file() {
            return Err(format!("DS5-PKG-002: component package is missing {name}"));
        }
    }
    validate_compatibility_file(directory)?;
    validate_sidecar_runtime_metadata(directory)
}

/// Compares contract values while allowing harmless JSON formatting differences.
pub(crate) fn validate_compatibility_file(directory: &Path) -> Result<(), String> {
    let actual = fs::read(directory.join("component-compatibility.json")).map_err(|error| {
        format!("DS5-PROTO-001: component compatibility contract is missing: {error}")
    })?;
    let actual: serde_json::Value = serde_json::from_slice(&actual).map_err(|error| {
        format!("DS5-PROTO-001: invalid component compatibility contract: {error}")
    })?;
    let expected: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../component-compatibility.json"))
            .expect("build-validated component compatibility contract");
    if actual != expected {
        return Err(
            "DS5-PROTO-001: component compatibility contract does not match this Control Panel"
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) fn validate_sidecar_runtime_metadata(directory: &Path) -> Result<(), String> {
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

pub(crate) fn validate_component_backup(directory: &Path) -> Result<(), String> {
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
    validate_core_integrity(&directory.join("HIDMaestro.Core.dll"), &manifest)?;
    Ok(())
}

pub(crate) fn extract_sidecar_package(
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
        if !names.insert(name.to_ascii_lowercase()) {
            return Err("DS5-PKG-002: sidecar package contains duplicate files".to_string());
        }
        let mut output = File::create(staging.join(name)).map_err(|error| error.to_string())?;
        std::io::copy(&mut item, &mut output).map_err(|error| error.to_string())?;
    }

    if !staging.join(SIDECAR_EXE).is_file() {
        return Err("DS5-PKG-002: sidecar executable is missing from the package".to_string());
    }
    validate_complete_runtime(staging)?;
    Ok(())
}

pub(crate) async fn acquire_sidecar_package(
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
    let candidates = crate::update::resolve_component_download_candidates(
        "dualsense-sidecar-zip",
        &manifest.asset_name,
        &manifest.sha256,
        &manifest.download_url,
    )
    .await
    .map_err(|error| format!("DS5-PKG-001: unable to resolve Sidecar download sources: {error}"))?;
    report_progress(progress, "sidecar_downloading", 12);
    download_component_asset_from_candidates(
        ComponentDownloadSpec {
            url: &manifest.download_url,
            destination,
            expected_size: Some(manifest.size),
            expected_sha256: Some(&manifest.sha256),
            max_size: MAX_SIDECAR_PACKAGE_BYTES,
            stage: "sidecar_downloading",
            progress_start: 12,
            progress_span: 24,
            error_code: "DS5-PKG-001",
        },
        &candidates,
        progress,
    )
    .await
}

/// Auto-discovered archives are an optional download shortcut, never a sticky source choice.
/// Explicitly selected archives must still fail if they do not match the pinned release.
/// Returns whether the auto-discovered archive was used, for the installed source metadata.
pub(crate) async fn acquire_sidecar_package_from_sources(
    manifest: &SidecarPackageManifest,
    selected_package: Option<&Path>,
    discovered_package: Option<&Path>,
    destination: &Path,
    progress: &ProgressReporter<'_>,
) -> Result<bool, String> {
    acquire_sidecar_package_with_fallback(
        manifest,
        selected_package,
        discovered_package,
        destination,
        progress,
        acquire_sidecar_package(manifest, selected_package, destination, progress),
    )
    .await
}

// Keep final acquisition lazy so tests can supply a local download fixture.
pub(super) async fn acquire_sidecar_package_with_fallback(
    manifest: &SidecarPackageManifest,
    selected_package: Option<&Path>,
    discovered_package: Option<&Path>,
    destination: &Path,
    progress: &ProgressReporter<'_>,
    fallback: impl std::future::Future<Output = Result<(), String>>,
) -> Result<bool, String> {
    if selected_package.is_none()
        && let Some(source) = discovered_package
    {
        report_progress(progress, "sidecar_local", 12);
        match acquire_sidecar_package(manifest, Some(source), destination, progress).await {
            Ok(()) => return Ok(true),
            Err(error) => warn!(
                "Ignoring auto-discovered DualSense package {}: {error}; falling back to download",
                source.display()
            ),
        }
    }
    fallback.await?;
    Ok(false)
}

/// Reuses a healthy pinned transport or stages a verified installer during explicit maintenance.
/// Existing incompatible installations require shared cleanup before replacement.
#[cfg(target_os = "windows")]
pub(crate) async fn ensure_pinned_usbip(
    progress: &ProgressReporter<'_>,
    component_root: &Path,
    local_installer: Option<&Path>,
) -> Result<UsbipInstallResult, String> {
    match install_disposition(installed_usbip_version()?.as_deref())? {
        InstallDisposition::Ready => {
            let status = crate::usbip::usbip_get_status().await?;
            if !status.ready {
                return Err(format!(
                    "USBIP-SETUP-008: existing USB/IP transport needs repair before reuse: {}",
                    status.detail
                ));
            }
            return Ok(UsbipInstallResult::Ready);
        }
        InstallDisposition::Install => {}
    }

    let installer_file = tempfile::Builder::new()
        .prefix("usbip-installer-")
        .suffix(".exe")
        .tempfile_in(component_root)
        .map_err(|error| format!("DS5-DRV-001: unable to stage the USB/IP installer: {error}"))?
        .into_temp_path();
    let installer_path = installer_file.to_path_buf();
    let download_result: Result<UsbipInstallResult, String> = async {
        if let Some(source) = local_installer {
            report_progress(progress, "transport_local", 3);
            tokio::fs::copy(source, &installer_path).await.map_err(|error| {
                format!("DS5-DRV-001: unable to stage the local USB/IP installer: {error}")
            })?;
        } else {
            download_component_asset(
                ComponentDownloadSpec {
                    url: USBIP_URL,
                    destination: &installer_path,
                    expected_size: None,
                    expected_sha256: None,
                    max_size: MAX_USBIP_INSTALLER_BYTES,
                    stage: "transport_downloading",
                    progress_start: 3,
                    progress_span: 6,
                    error_code: "DS5-DRV-001",
                },
                progress,
            )
            .await?;
        }

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
        if installed_usbip_version()?.as_deref() != Some(USBIP_VERSION) {
            return Err(format!(
                "DS5-DRV-001: USB/IP installer completed but version {USBIP_VERSION} is not registered"
            ));
        }
        Ok(install_result)
    }
    .await;
    download_result
}

#[cfg(not(target_os = "windows"))]
pub(crate) async fn ensure_pinned_usbip(
    _progress: &ProgressReporter<'_>,
    _component_root: &Path,
    _local_installer: Option<&Path>,
) -> Result<UsbipInstallResult, String> {
    Ok(UsbipInstallResult::Ready)
}

pub(crate) fn rollback_activated_component(
    active: &Path,
    backup: &Path,
    had_previous: bool,
) -> Result<(), String> {
    if active.exists() {
        retry_component_fs_operation(|| fs::remove_dir_all(active)).map_err(|error| {
            component_fs_error("unable to remove failed active component", &error)
        })?;
    }
    if had_previous {
        if !backup.exists() {
            return Err("previous component backup is missing".to_string());
        }
        validate_component_backup(backup)?;
        retry_component_fs_operation(|| fs::rename(backup, active))
            .map_err(|error| component_fs_error("unable to restore previous component", &error))?;
    }
    Ok(())
}

pub(crate) fn retry_component_fs_operation<T>(
    mut operation: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    for attempt in 0..COMPONENT_FS_RETRY_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error)
                if attempt + 1 < COMPONENT_FS_RETRY_ATTEMPTS
                    && is_transient_component_fs_error(&error) =>
            {
                std::thread::sleep(COMPONENT_FS_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("component filesystem retry loop always returns")
}

pub(crate) fn is_transient_component_fs_error(error: &std::io::Error) -> bool {
    #[cfg(target_os = "windows")]
    {
        // ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, and ERROR_LOCK_VIOLATION.
        matches!(error.raw_os_error(), Some(5 | 32 | 33))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = error;
        false
    }
}

pub(crate) fn component_fs_error(action: &str, error: &std::io::Error) -> String {
    match error.raw_os_error() {
        Some(code) => format!("{action} (OS error {code}): {error}"),
        None => format!("{action}: {error}"),
    }
}

pub(crate) fn activate_staged_component(
    staging: &Path,
    active: &Path,
    backup: &Path,
) -> Result<(), String> {
    if backup.exists() {
        retry_component_fs_operation(|| fs::remove_dir_all(backup)).map_err(|error| {
            format!(
                "DS5-PKG-002: {}",
                component_fs_error("unable to remove previous component backup", &error)
            )
        })?;
    }
    if active.exists() {
        retry_component_fs_operation(|| fs::rename(active, backup)).map_err(|error| {
            format!(
                "DS5-PKG-002: {}",
                component_fs_error("unable to back up the active component", &error)
            )
        })?;
    }
    if let Err(error) = retry_component_fs_operation(|| fs::rename(staging, active)) {
        if backup.exists()
            && let Err(rollback_error) = retry_component_fs_operation(|| fs::rename(backup, active))
        {
            warn!(
                "DualSense activation rollback failed: {}",
                component_fs_error("unable to restore previous component", &rollback_error)
            );
        }
        return Err(format!(
            "DS5-PKG-002: {}",
            component_fs_error("unable to activate component", &error)
        ));
    }
    Ok(())
}

/// Restores the last valid component when a previous process stopped between
/// moving `active` aside and promoting the staged directory.
pub(crate) fn recover_interrupted_activation(active: &Path, backup: &Path) -> Result<bool, String> {
    if active.exists() || !backup.exists() {
        return Ok(false);
    }
    validate_component_backup(backup).map_err(|error| {
        format!("DS5-PKG-002: interrupted activation backup is invalid: {error}")
    })?;
    retry_component_fs_operation(|| fs::rename(backup, active)).map_err(|error| {
        format!(
            "DS5-PKG-002: {}",
            component_fs_error("unable to recover interrupted component activation", &error)
        )
    })?;
    Ok(true)
}

pub(crate) async fn install_error_after_rollback(
    error: String,
    active: &Path,
    backup: &Path,
    had_previous: bool,
) -> String {
    let active = active.to_path_buf();
    let backup = backup.to_path_buf();
    match tokio::task::spawn_blocking(move || {
        rollback_activated_component(&active, &backup, had_previous)
    })
    .await
    {
        Ok(Ok(())) => format!("{error}; activated component was rolled back"),
        Ok(Err(rollback_error)) => {
            format!("{error}; component rollback also failed: {rollback_error}")
        }
        Err(task_error) => format!("{error}; component rollback task failed: {task_error}"),
    }
}

/// Requires two completed rounds, released devices and the expected transport policy.
pub(crate) fn validate_transport_acceptance(value: &serde_json::Value) -> Result<(), String> {
    if value["rounds"] != 2
        || value["released"] != true
        || value["usbip_policy"] != crate::usbip::USBIP_POLICY
        || value["usbip_version"] != USBIP_VERSION
    {
        return Err("DS5-PKG-003: shared transport acceptance is incomplete".to_string());
    }
    Ok(())
}

/// Exercises the staged runtime before activation with no active stream or imports.
/// Rechecks release after test failure without detaching another application's devices.
pub(crate) async fn verify_staged_transport(executable: &Path) -> Result<(), String> {
    super::probe::ensure_no_active_session().await?;
    let before = crate::usbip::usbip_get_status().await?;
    if !before.ready || !before.attached_devices.is_empty() {
        return Err("DS5-PKG-003: shared USB/IP must be ready and all imports released before component acceptance; reboot or repair the transport if required".to_string());
    }
    let executable = executable.to_path_buf();
    let outcome = tokio::task::spawn_blocking(move || {
        let result = tempfile::Builder::new()
            .prefix("transport-acceptance-")
            .suffix(".json")
            .tempfile_in(executable.parent().unwrap())
            .map_err(|error| {
                format!("DS5-PKG-003: unable to prepare the acceptance result: {error}")
            })?
            .into_temp_path();
        let mut command = Command::new(&executable);
        command
            .args(["--self-test", "transport", "--result"])
            .arg(&result)
            .current_dir(executable.parent().unwrap());
        let output = run_with_timeout(
            &mut command,
            std::time::Duration::from_secs(120),
            "DS5-PKG-003: shared transport acceptance timed out",
            true,
        )?;
        if !output.status.success() {
            return Err(component_test_failure(&output, &result));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&result).map_err(|error| {
                format!("DS5-PKG-003: missing transport acceptance result: {error}")
            })?)
            .map_err(|error| {
                format!("DS5-PKG-003: invalid transport acceptance result: {error}")
            })?;
        validate_transport_acceptance(&value)
    })
    .await
    .map_err(|error| format!("DS5-PKG-003: transport acceptance task failed: {error}"))?;
    // Check release even on failure; never sweep another application's imports.
    let after = crate::usbip::usbip_get_status().await?;
    if !after.ready || !after.attached_devices.is_empty() {
        return Err("DS5-PKG-003: shared transport did not recover after acceptance; component was not activated".to_string());
    }
    outcome
}

/// Stages and verifies a complete component before activation, preserving the old copy.
/// Driver installation is restricted to explicit maintenance through `install_transport`.
pub(crate) async fn dualsense_install_impl(
    progress: &ProgressReporter<'_>,
    local_packages: LocalComponentPackages,
    install_transport: bool,
) -> Result<DualSenseStatus, String> {
    report_progress(progress, "preparing", 1);
    let selected_sidecar_package = local_packages.sidecar.as_deref();
    let discovered_package = selected_sidecar_package
        .is_none()
        .then(manually_placed_sidecar_package)
        .flatten();
    let mut using_discovered_package = false;
    let using_selected_package = selected_sidecar_package.is_some();
    let local_sidecar_package = selected_sidecar_package.or(discovered_package.as_deref());
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
    let active = active_dir();
    let backup = root.join("previous");
    let recovery_active = active.clone();
    let recovery_backup = backup.clone();
    if tokio::task::spawn_blocking(move || {
        recover_interrupted_activation(&recovery_active, &recovery_backup)
    })
    .await
    .map_err(|error| format!("DS5-PKG-002: component recovery task failed: {error}"))??
    {
        info!("Recovered the previous DualSense component after an interrupted activation");
    }
    // Stay on the installation volume so activation can keep using atomic renames.
    let staging_directory = tempfile::Builder::new()
        .prefix("staging-")
        .tempdir_in(&root)
        .map_err(|error| format!("DS5-PKG-002: unable to prepare component staging: {error}"))?;
    let staging = staging_directory.path().to_path_buf();
    let current_handoff_packages = [local_sidecar_package, local_packages.usbip.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    purge_stale_handoff_packages(&root, &current_handoff_packages);
    let sidecar_archive = tempfile::Builder::new()
        .prefix("sidecar-")
        .suffix(".partial.zip")
        .tempfile_in(&root)
        .map_err(|error| format!("DS5-PKG-002: unable to stage the sidecar archive: {error}"))?
        .into_temp_path();
    let sidecar_archive_path = sidecar_archive.to_path_buf();
    let had_previous = active.exists();
    let install_result: Result<UsbipInstallResult, String> = async {
        let usbip_install_result = if install_transport {
            ensure_pinned_usbip(progress, &root, local_packages.usbip.as_deref()).await?
        } else {
            UsbipInstallResult::Ready
        };
        if let Some(manifest) = package_manifest.as_ref() {
            using_discovered_package = acquire_sidecar_package_from_sources(
                manifest,
                selected_sidecar_package,
                discovered_package.as_deref(),
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

        report_progress(progress, "probing", 86);
        let probe_executable = staging.join(SIDECAR_EXE);
        let probe_path = probe_executable.clone();
        tokio::task::spawn_blocking(move || run_probe(&probe_path))
            .await
            .map_err(|error| format!("DS5-PKG-003: sidecar probe task failed: {error}"))??;
        let sidecar_sha256 = sha256_file(&probe_executable)?;
        let core_sha256 = sha256_file(&staging.join("HIDMaestro.Core.dll"))?;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "component_version": COMPONENT_VERSION,
                "hidmaestro_version": HIDMAESTRO_VERSION,
                "sha256": core_sha256,
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
                "sidecar_sha256": sidecar_sha256,
                "usbip_policy": crate::usbip::USBIP_POLICY,
                "usbip_version": crate::usbip::USBIP_VERSION,
                "compatibility_sha256": sha256_file(&staging.join("component-compatibility.json"))?
            }))
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        report_progress(progress, "verifying_transport", 90);
        verify_staged_transport(&probe_executable).await?;

        // A stream may have started while the package was downloading.
        super::probe::ensure_no_active_session().await?;
        let activation_staging = staging.clone();
        let activation_active = active.clone();
        let activation_backup = backup.clone();
        tokio::task::spawn_blocking(move || {
            activate_staged_component(
                &activation_staging,
                &activation_active,
                &activation_backup,
            )
        })
        .await
        .map_err(|error| format!("DS5-PKG-002: component activation task failed: {error}"))??;
        report_progress(progress, "activating", 96);
        Ok(usbip_install_result)
    }
    .await;
    if install_result.is_err() {
        let failed_staging = staging.clone();
        match tokio::task::spawn_blocking(move || {
            retry_component_fs_operation(|| fs::remove_dir_all(&failed_staging))
        })
        .await
        {
            Ok(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => warn!(
                "Unable to clean the failed DualSense staging component: {}",
                component_fs_error("cleanup failed", &error)
            ),
            Err(error) => warn!("DualSense staging cleanup task failed: {error}"),
            _ => {}
        }
    }
    let reboot_recommended = matches!(install_result?, UsbipInstallResult::RebootRecommended);
    report_progress(progress, "complete", 100);
    let mut status = match dualsense_get_status().await {
        Ok(status) => status,
        Err(status_error) => {
            return Err(
                install_error_after_rollback(status_error, &active, &backup, had_previous).await,
            );
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
        )
        .await);
    }
    status.reboot_recommended = reboot_recommended;
    Ok(status)
}

pub(crate) async fn dualsense_self_test_impl(profile: String) -> Result<serde_json::Value, String> {
    if profile != "standard" && profile != "composite" {
        return Err("DS5-PKG-003: invalid self-test profile".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        tokio::task::spawn_blocking(move || {
            let executable = super::packages::verified_sidecar_path()?;
            let result = tempfile::Builder::new()
                .prefix("self-test-")
                .suffix(".json")
                .tempfile_in(component_root())
                .map_err(|error| {
                    format!("DS5-PKG-003: unable to prepare the self-test result: {error}")
                })?
                .into_temp_path();
            let mut command = Command::new(&executable);
            command
                .args(["--self-test", &profile, "--result"])
                .arg(&result)
                .current_dir(executable.parent().unwrap_or_else(|| Path::new(".")));
            let output = run_with_timeout(
                &mut command,
                std::time::Duration::from_secs(60),
                "DS5-PKG-003: component self-test timed out",
                true,
            )?;
            if !output.status.success() {
                return Err(component_test_failure(&output, &result));
            }
            let contents = fs::read_to_string(&result).map_err(|error| {
                format!("DS5-PKG-003: component test produced no result: {error}")
            })?;
            serde_json::from_str(&contents)
                .map_err(|error| format!("DS5-PKG-003: invalid component test result: {error}"))
        })
        .await
        .map_err(|error| format!("DS5-PKG-003: component test task failed: {error}"))?
    }
    #[cfg(not(target_os = "windows"))]
    Err("DS5-PKG-003: component tests are only supported on Windows".to_string())
}

pub(crate) async fn dualsense_uninstall_impl() -> Result<DualSenseStatus, String> {
    let reset_result: Result<(), String> = async {
        let snapshot = get_core_ds5_settings().await?;
        let mut settings = snapshot.response.settings;
        update_config_fields(&mut settings, true, false);
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
        tokio::task::spawn_blocking(move || {
            retry_component_fs_operation(|| fs::remove_dir_all(&root))
        })
        .await
        .map_err(|error| format!("DS5-PKG-002: component removal task failed: {error}"))?
        .map_err(|error| {
            format!(
                "DS5-PKG-002: {}",
                component_fs_error("unable to remove component", &error)
            )
        })?;
    }
    match dualsense_get_status().await {
        Ok(status) => Ok(status),
        Err(error) => {
            warn!("DualSense uninstall could not refresh Core status: {error}");
            Ok(local_uninstalled_status())
        }
    }
}
