//! Install, self-test, and uninstall orchestration.

use log::warn;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config::{
    get_core_ds5_settings, require_entity_tag, save_core_ds5_settings, update_config_fields,
};
use super::packages::{
    ComponentDownloadSpec, InstalledComponentManifest, LocalComponentPackages,
    SidecarPackageManifest, SidecarRuntimeMetadata, UsbipInstallResult, active_dir, component_root,
    download_component_asset, manually_placed_sidecar_package, purge_stale_handoff_packages,
    sha256_file, sidecar_package_manifest, sidecar_package_manifest_path, sidecar_path,
};
use super::probe::{
    component_test_failure, installed_usbip_version, local_uninstalled_status, run_probe,
    run_with_timeout,
};
use super::{
    COMPONENT_FS_RETRY_ATTEMPTS, COMPONENT_FS_RETRY_DELAY, COMPONENT_VERSION, DualSenseStatus,
    HIDMAESTRO_SHA256, HIDMAESTRO_URL, HIDMAESTRO_VERSION, MAX_ARCHIVE_BYTES, MAX_ARCHIVE_FILES,
    MAX_EXTRACTED_BYTES, MAX_SIDECAR_PACKAGE_BYTES, MAX_USBIP_INSTALLER_BYTES, PROTOCOL_VERSION,
    ProgressReporter, SIDECAR_EXE, SIDECAR_PACKAGE_TARGET, USBIP_SHA256, USBIP_URL, USBIP_VERSION,
    dualsense_get_status, report_progress,
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

pub(crate) fn copy_runtime_files(source: &Path, destination: &Path) -> Result<(), String> {
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
    let hidmaestro_hash = sha256_file(&directory.join("HIDMaestro.Core.dll"))?;
    if manifest.sha256.len() != 64 || !hidmaestro_hash.eq_ignore_ascii_case(&manifest.sha256) {
        return Err("backup HIDMaestro integrity check failed".to_string());
    }
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
    report_progress(progress, "sidecar_downloading", 12);
    download_component_asset(
        ComponentDownloadSpec {
            url: &manifest.download_url,
            destination,
            expected_size: Some(manifest.size),
            max_size: MAX_SIDECAR_PACKAGE_BYTES,
            stage: "sidecar_downloading",
            progress_start: 12,
            progress_span: 24,
            error_code: "DS5-PKG-001",
        },
        progress,
    )
    .await
}

pub(crate) fn extract_verified_package(archive_path: &Path, staging: &Path) -> Result<(), String> {
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
pub(crate) async fn ensure_pinned_usbip(
    progress: &ProgressReporter<'_>,
    component_root: &Path,
    local_installer: Option<&Path>,
) -> Result<UsbipInstallResult, String> {
    if installed_usbip_version().as_deref() == Some(USBIP_VERSION) {
        return Ok(UsbipInstallResult::Ready);
    }

    let installer_path = component_root.join(format!("USBip-{USBIP_VERSION}-x64.partial.exe"));
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

pub(crate) async fn dualsense_install_impl(
    progress: &ProgressReporter<'_>,
    local_packages: LocalComponentPackages,
) -> Result<DualSenseStatus, String> {
    report_progress(progress, "preparing", 1);
    let selected_sidecar_package = local_packages.sidecar.as_deref();
    let discovered_package = selected_sidecar_package
        .is_none()
        .then(manually_placed_sidecar_package)
        .flatten();
    let using_discovered_package = discovered_package.is_some();
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
    let operation = format!("staging-{}", std::process::id());
    let staging = root.join(&operation);
    if staging.exists() {
        let stale_staging = staging.clone();
        tokio::task::spawn_blocking(move || {
            retry_component_fs_operation(|| fs::remove_dir_all(&stale_staging)).map_err(|error| {
                format!(
                    "DS5-PKG-002: {}",
                    component_fs_error("unable to remove stale staging component", &error)
                )
            })
        })
        .await
        .map_err(|error| format!("DS5-PKG-002: staging cleanup task failed: {error}"))??;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let current_handoff_packages = [
        local_sidecar_package,
        local_packages.hidmaestro.as_deref(),
        local_packages.usbip.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    purge_stale_handoff_packages(&root, &current_handoff_packages);
    let archive_path = root.join(format!("{operation}-hidmaestro.partial"));
    let sidecar_archive_path = root.join(format!("{operation}-sidecar.partial.zip"));
    let active = active_dir();
    let backup = root.join("previous");
    let had_previous = active.exists();
    let install_result: Result<UsbipInstallResult, String> = async {
        let usbip_install_result =
            ensure_pinned_usbip(progress, &root, local_packages.usbip.as_deref()).await?;
        if let Some(manifest) = package_manifest.as_ref() {
            if using_discovered_package {
                report_progress(progress, "sidecar_local", 12);
            }
            acquire_sidecar_package(
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

        if let Some(source) = local_packages.hidmaestro.as_deref() {
            report_progress(progress, "runtime_local", 38);
            tokio::fs::copy(source, &archive_path).await.map_err(|error| {
                format!("DS5-PKG-002: unable to stage the local HIDMaestro package: {error}")
            })?;
        } else {
            download_component_asset(
                ComponentDownloadSpec {
                    url: HIDMAESTRO_URL,
                    destination: &archive_path,
                    expected_size: None,
                    max_size: MAX_ARCHIVE_BYTES,
                    stage: "downloading",
                    progress_start: 38,
                    progress_span: 34,
                    error_code: "DS5-PKG-001",
                },
                progress,
            )
            .await?;
        }

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
                "source": if local_packages.hidmaestro.is_some() { "selected-local" } else { HIDMAESTRO_URL },
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
    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_file(&sidecar_archive_path);
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

pub(crate) async fn dualsense_uninstall_impl() -> Result<DualSenseStatus, String> {
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
