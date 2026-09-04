use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const BACKEND_FILE: &str = "foundation_truehdr_backend.dll";
const RUNTIME_FILE: &str = "nvngx_truehdr.dll";
const MAX_BACKEND_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RUNTIME_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PE_OFFSET: u64 = 16 * 1024 * 1024;
const MAX_PE_SECTIONS: u64 = 96;
static COMPONENT_OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[cfg(target_os = "windows")]
const ELEVATED_INSTALL_ARG: &str = "--elevated-rtx-hdr-install";
#[cfg(target_os = "windows")]
const ELEVATED_REMOVE_ARG: &str = "--elevated-rtx-hdr-remove";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComponentManifest {
    schema: u32,
    component_id: String,
    backend_sha256: String,
    runtime_sha256: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RtxHdrComponentStatus {
    pub state: String,
    pub installed: bool,
    pub ready: bool,
    pub in_use: bool,
    pub backend_present: bool,
    pub runtime_present: bool,
    pub configured: bool,
    pub restart_required: bool,
    pub managed_path: String,
    pub backend_sha256: String,
    pub runtime_sha256: String,
    pub detail: String,
}

fn component_root() -> PathBuf {
    crate::sunshine::install_dir().join("tools").join("rtx_hdr")
}

fn versions_dir() -> PathBuf {
    component_root().join("versions")
}

fn validate_named_dll(path: &Path, expected_name: &str, max_size: u64) -> Result<(), String> {
    let actual_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "RTXHDR-PKG-001: selected DLL has no valid file name".to_string())?;
    if !actual_name.eq_ignore_ascii_case(expected_name) {
        return Err(format!(
            "RTXHDR-PKG-001: expected {expected_name}, selected {actual_name}"
        ));
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("RTXHDR-PKG-002: unable to read {expected_name}: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_size {
        return Err(format!(
            "RTXHDR-PKG-002: {expected_name} has an invalid size"
        ));
    }
    validate_pe_x64(path)
        .map_err(|error| format!("RTXHDR-PKG-003: {expected_name} is not a valid x64 DLL: {error}"))
}

fn validate_pe_x64(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let file_size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|error| error.to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("missing MZ header".to_string());
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().unwrap()) as u64;
    if !(64..=MAX_PE_OFFSET).contains(&pe_offset) || pe_offset + 24 > file_size {
        return Err("invalid PE header offset".to_string());
    }

    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|error| error.to_string())?;
    let mut coff = [0u8; 24];
    file.read_exact(&mut coff)
        .map_err(|error| error.to_string())?;
    if &coff[..4] != b"PE\0\0" {
        return Err("missing PE signature".to_string());
    }
    if u16::from_le_bytes([coff[4], coff[5]]) != 0x8664 {
        return Err("machine is not AMD64".to_string());
    }
    let section_count = u16::from_le_bytes([coff[6], coff[7]]) as u64;
    if section_count == 0 || section_count > MAX_PE_SECTIONS {
        return Err("invalid PE section count".to_string());
    }
    let optional_size = u16::from_le_bytes([coff[20], coff[21]]) as u64;
    if optional_size < 0x70 {
        return Err("missing PE optional header".to_string());
    }
    if u16::from_le_bytes([coff[22], coff[23]]) & 0x2000 == 0 {
        return Err("image is not marked as a DLL".to_string());
    }
    let section_table_end = pe_offset
        .checked_add(24)
        .and_then(|value| value.checked_add(optional_size))
        .and_then(|value| value.checked_add(section_count * 40))
        .ok_or_else(|| "PE layout overflow".to_string())?;
    if section_table_end > file_size {
        return Err("PE headers extend beyond the file".to_string());
    }

    let mut optional = vec![0u8; optional_size as usize];
    file.read_exact(&mut optional)
        .map_err(|error| error.to_string())?;
    if u16::from_le_bytes([optional[0], optional[1]]) != 0x20b {
        return Err("optional header is not PE32+".to_string());
    }
    let size_of_image = u32::from_le_bytes(optional[56..60].try_into().unwrap()) as u64;
    let size_of_headers = u32::from_le_bytes(optional[60..64].try_into().unwrap()) as u64;
    if size_of_headers == 0 || size_of_headers > file_size || size_of_image < size_of_headers {
        return Err("invalid PE image or header size".to_string());
    }

    let mut has_nonempty_section = false;
    for _ in 0..section_count {
        let mut section = [0u8; 40];
        file.read_exact(&mut section)
            .map_err(|error| error.to_string())?;
        let virtual_size = u32::from_le_bytes(section[8..12].try_into().unwrap()) as u64;
        let virtual_address = u32::from_le_bytes(section[12..16].try_into().unwrap()) as u64;
        let raw_size = u32::from_le_bytes(section[16..20].try_into().unwrap()) as u64;
        let raw_offset = u32::from_le_bytes(section[20..24].try_into().unwrap()) as u64;
        if virtual_size == 0 && raw_size == 0 {
            continue;
        }
        has_nonempty_section = true;
        if raw_size > 0
            && raw_offset
                .checked_add(raw_size)
                .is_none_or(|end| raw_offset < size_of_headers || end > file_size)
        {
            return Err("PE section raw data is outside the file".to_string());
        }
        if virtual_address
            .checked_add(virtual_size.max(raw_size))
            .is_none_or(|end| end > size_of_image)
        {
            return Err("PE section virtual range is outside the image".to_string());
        }
    }
    if !has_nonempty_section {
        return Err("PE image has no nonempty sections".to_string());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn component_id(backend_hash: &str, runtime_hash: &str) -> String {
    format!("{}-{}", &backend_hash[..16], &runtime_hash[..16])
}

fn is_managed_backend_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(BACKEND_FILE))
        && path.starts_with(versions_dir())
        && path
            .parent()
            .is_some_and(|parent| parent.parent() == Some(versions_dir().as_path()))
}

fn read_manifest(directory: &Path) -> Option<ComponentManifest> {
    fs::read(directory.join("component.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn validate_version(directory: &Path, expected: &ComponentManifest) -> bool {
    let backend = directory.join(BACKEND_FILE);
    let runtime = directory.join(RUNTIME_FILE);
    let Some(stored) = read_manifest(directory) else {
        return false;
    };
    stored.schema == 1
        && stored.component_id == expected.component_id
        && stored.backend_sha256 == expected.backend_sha256
        && stored.runtime_sha256 == expected.runtime_sha256
        && validate_named_dll(&backend, BACKEND_FILE, MAX_BACKEND_BYTES).is_ok()
        && validate_named_dll(&runtime, RUNTIME_FILE, MAX_RUNTIME_BYTES).is_ok()
        && sha256_file(&backend).ok().as_deref() == Some(expected.backend_sha256.as_str())
        && sha256_file(&runtime).ok().as_deref() == Some(expected.runtime_sha256.as_str())
}

fn build_status(
    config: &crate::sunshine::RtxHdrBackendConfigState,
    in_use: bool,
    cleanup_detail: String,
) -> RtxHdrComponentStatus {
    let configured_path = &config.persisted_path;
    let backend = PathBuf::from(configured_path);
    let configured = !configured_path.is_empty() && is_managed_backend_path(&backend);
    let active_matches = config.active_path.eq_ignore_ascii_case(configured_path);
    let directory = configured.then(|| backend.parent().unwrap().to_path_buf());
    let runtime = directory.as_ref().map(|path| path.join(RUNTIME_FILE));
    let manifest = directory.as_deref().and_then(read_manifest);
    let backend_present = configured && backend.is_file();
    let runtime_present = runtime.as_ref().is_some_and(|path| path.is_file());
    let integrity_valid = manifest.as_ref().is_some_and(|manifest| {
        directory
            .as_deref()
            .is_some_and(|directory| validate_version(directory, manifest))
    });
    let ready = configured && integrity_valid && active_matches && !config.restart_required;
    let installed = configured;
    let state = if in_use {
        "in_use"
    } else if ready {
        "ready"
    } else if !installed {
        "not_installed"
    } else {
        "repair_required"
    };
    let detail = if !cleanup_detail.is_empty() {
        cleanup_detail
    } else if installed && !integrity_valid {
        "RTXHDR-PKG-006: managed files or component manifest failed integrity validation"
            .to_string()
    } else if !configured_path.is_empty() && !configured {
        "RTXHDR-CFG-003: Sunshine uses a backend path outside the GUI-managed component store"
            .to_string()
    } else if configured && !active_matches {
        "RTXHDR-CFG-004: Sunshine restart is required to activate the managed backend".to_string()
    } else {
        String::new()
    };
    RtxHdrComponentStatus {
        state: state.to_string(),
        installed,
        ready,
        in_use,
        backend_present,
        runtime_present,
        configured,
        restart_required: config.restart_required || (configured && !active_matches),
        managed_path: if configured {
            configured_path.to_string()
        } else {
            String::new()
        },
        backend_sha256: manifest
            .as_ref()
            .map(|value| value.backend_sha256.clone())
            .unwrap_or_default(),
        runtime_sha256: manifest
            .as_ref()
            .map(|value| value.runtime_sha256.clone())
            .unwrap_or_default(),
        detail,
    }
}

async fn ensure_idle() -> Result<(), String> {
    let state = crate::sunshine::get_tray_state()
        .await
        .map_err(|error| format!("RTXHDR-SESSION-001: unable to verify stream state: {error}"))?;
    if !state.sessions.is_empty() {
        return Err(
            "RTXHDR-SESSION-002: finish all active streams before changing the RTX HDR component"
                .to_string(),
        );
    }
    Ok(())
}

fn permission_error(action: &str, error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        format!("RTXHDR-PKG-005: {action} requires administrator permission")
    } else {
        format!("RTXHDR-PKG-004: {action} failed: {error}")
    }
}

fn prepare_immutable_version(
    backend_source: &Path,
    runtime_source: &Path,
    manifest: &ComponentManifest,
) -> Result<PathBuf, String> {
    validate_named_dll(backend_source, BACKEND_FILE, MAX_BACKEND_BYTES)?;
    validate_named_dll(runtime_source, RUNTIME_FILE, MAX_RUNTIME_BYTES)?;
    let versions = versions_dir();
    fs::create_dir_all(&versions)
        .map_err(|error| permission_error("create component directory", error))?;
    let version_dir = versions.join(&manifest.component_id);
    if version_dir.exists() {
        if validate_version(&version_dir, manifest) {
            return Ok(version_dir);
        }
        return Err(
            "RTXHDR-PKG-006: existing immutable component version failed validation".to_string(),
        );
    }

    let staging = component_root().join(format!("staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging)
        .map_err(|error| permission_error("create staging directory", error))?;
    let prepared = (|| -> Result<(), String> {
        fs::copy(backend_source, staging.join(BACKEND_FILE))
            .map_err(|error| permission_error("copy backend DLL", error))?;
        fs::copy(runtime_source, staging.join(RUNTIME_FILE))
            .map_err(|error| permission_error("copy NVIDIA runtime", error))?;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(manifest).unwrap(),
        )
        .map_err(|error| permission_error("write component manifest", error))?;
        if !validate_version(&staging, manifest) {
            return Err("RTXHDR-PKG-006: staged component failed integrity validation".to_string());
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, &version_dir)
        .map_err(|error| permission_error("activate immutable component version", error))?;
    Ok(version_dir)
}

fn manifest_for_sources(
    backend_source: &Path,
    runtime_source: &Path,
) -> Result<ComponentManifest, String> {
    validate_named_dll(backend_source, BACKEND_FILE, MAX_BACKEND_BYTES)?;
    validate_named_dll(runtime_source, RUNTIME_FILE, MAX_RUNTIME_BYTES)?;
    let backend_hash = sha256_file(backend_source)
        .map_err(|error| format!("RTXHDR-PKG-004: hash backend failed: {error}"))?;
    let runtime_hash = sha256_file(runtime_source)
        .map_err(|error| format!("RTXHDR-PKG-004: hash runtime failed: {error}"))?;
    Ok(ComponentManifest {
        schema: 1,
        component_id: component_id(&backend_hash, &runtime_hash),
        backend_sha256: backend_hash,
        runtime_sha256: runtime_hash,
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    let mut arguments = std::env::args_os().skip(1);
    match arguments.next()?.to_str()? {
        ELEVATED_INSTALL_ARG => {
            let backend = PathBuf::from(arguments.next()?);
            let runtime = PathBuf::from(arguments.next()?);
            if arguments.next().is_some() {
                return Some(2);
            }
            let result = manifest_for_sources(&backend, &runtime)
                .and_then(|manifest| prepare_immutable_version(&backend, &runtime, &manifest));
            Some(if result.is_ok() { 0 } else { 1 })
        }
        ELEVATED_REMOVE_ARG => {
            if arguments.next().is_some() {
                return Some(2);
            }
            let result = fs::remove_dir_all(component_root());
            Some(
                if result.is_ok()
                    || result.is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                {
                    0
                } else {
                    1
                },
            )
        }
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    None
}

#[cfg(target_os = "windows")]
async fn prepare_version_with_elevation(
    backend_source: &Path,
    runtime_source: &Path,
    manifest: &ComponentManifest,
) -> Result<PathBuf, String> {
    if crate::utils::is_running_as_admin()? {
        return prepare_immutable_version(backend_source, runtime_source, manifest);
    }
    let backend = backend_source.to_string_lossy().into_owned();
    let runtime = runtime_source.to_string_lossy().into_owned();
    let process = tokio::task::spawn_blocking(move || {
        crate::utils::launch_current_executable_elevated(
            &[ELEVATED_INSTALL_ARG, &backend, &runtime],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("RTXHDR-UAC-002: elevated helper task failed: {error}"))?
    .map_err(|error| format!("RTXHDR-UAC-001: {error}"))?;
    let exit_code = tokio::task::spawn_blocking(move || process.wait_for_exit())
        .await
        .map_err(|error| format!("RTXHDR-UAC-002: elevated helper wait failed: {error}"))??;
    if exit_code != 0 {
        return Err(format!(
            "RTXHDR-UAC-003: elevated helper exited with {exit_code}"
        ));
    }
    let version_dir = versions_dir().join(&manifest.component_id);
    if !validate_version(&version_dir, manifest) {
        return Err(
            "RTXHDR-PKG-006: elevated installation did not produce a valid component".to_string(),
        );
    }
    Ok(version_dir)
}

#[cfg(target_os = "windows")]
async fn remove_component_with_elevation() -> Result<(), String> {
    if crate::utils::is_running_as_admin()? {
        return fs::remove_dir_all(component_root())
            .or_else(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                }
            })
            .map_err(|error| permission_error("remove managed component", error));
    }
    let process = tokio::task::spawn_blocking(|| {
        crate::utils::launch_current_executable_elevated(
            &[ELEVATED_REMOVE_ARG],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("RTXHDR-UAC-002: elevated cleanup task failed: {error}"))?
    .map_err(|error| format!("RTXHDR-UAC-001: {error}"))?;
    let exit_code = tokio::task::spawn_blocking(move || process.wait_for_exit())
        .await
        .map_err(|error| format!("RTXHDR-UAC-002: elevated cleanup wait failed: {error}"))??;
    if exit_code == 0 {
        Ok(())
    } else {
        Err(format!(
            "RTXHDR-UAC-003: elevated cleanup exited with {exit_code}"
        ))
    }
}

#[cfg(not(target_os = "windows"))]
async fn remove_component_with_elevation() -> Result<(), String> {
    fs::remove_dir_all(component_root())
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error)
            }
        })
        .map_err(|error| permission_error("remove managed component", error))
}

#[cfg(not(target_os = "windows"))]
async fn prepare_version_with_elevation(
    backend_source: &Path,
    runtime_source: &Path,
    manifest: &ComponentManifest,
) -> Result<PathBuf, String> {
    prepare_immutable_version(backend_source, runtime_source, manifest)
}

#[tauri::command]
pub async fn rtx_hdr_get_status() -> Result<RtxHdrComponentStatus, String> {
    let config = crate::sunshine::get_local_rtx_hdr_backend_path()
        .await
        .map_err(|error| format!("RTXHDR-CFG-001: {error}"))?;
    let in_use = crate::sunshine::get_tray_state()
        .await
        .map(|state| !state.sessions.is_empty())
        .unwrap_or(false);
    tokio::task::spawn_blocking(move || build_status(&config, in_use, String::new()))
        .await
        .map_err(|error| format!("RTXHDR-PKG-004: status task failed: {error}"))
}

#[tauri::command]
pub async fn rtx_hdr_install(
    backend_path: String,
    runtime_path: String,
) -> Result<RtxHdrComponentStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "RTXHDR-BUSY-001: another RTX HDR component operation is running".to_string()
    })?;
    ensure_idle().await?;

    let backend_source = PathBuf::from(backend_path);
    let runtime_source = PathBuf::from(runtime_path);
    let manifest = manifest_for_sources(&backend_source, &runtime_source)?;
    let version_dir =
        prepare_version_with_elevation(&backend_source, &runtime_source, &manifest).await?;

    // Recheck immediately before the only commit point. Component directories
    // are immutable, so even a stream racing this check can only observe one
    // complete version and no loaded DLL is ever overwritten.
    ensure_idle().await?;
    let managed_backend = version_dir.join(BACKEND_FILE);
    let managed_backend_text = managed_backend.to_string_lossy().into_owned();
    let mut config = crate::sunshine::set_local_rtx_hdr_backend_path(&managed_backend_text)
        .await
        .map_err(|error| format!("RTXHDR-CFG-002: {error}"))?;
    if config.restart_required {
        config = crate::sunshine::restart_and_wait_for_rtx_hdr_backend(&managed_backend_text)
            .await
            .map_err(|error| format!("RTXHDR-CFG-004: {error}"))?;
    }
    Ok(build_status(&config, false, String::new()))
}

#[tauri::command]
pub async fn rtx_hdr_uninstall() -> Result<RtxHdrComponentStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "RTXHDR-BUSY-001: another RTX HDR component operation is running".to_string()
    })?;
    ensure_idle().await?;
    let current = crate::sunshine::get_local_rtx_hdr_backend_path()
        .await
        .map_err(|error| format!("RTXHDR-CFG-001: {error}"))?;
    let mut config = current.clone();
    if is_managed_backend_path(Path::new(&current.persisted_path)) {
        config = crate::sunshine::set_local_rtx_hdr_backend_path("")
            .await
            .map_err(|error| format!("RTXHDR-CFG-002: {error}"))?;
        if config.restart_required {
            config = crate::sunshine::restart_and_wait_for_rtx_hdr_backend("")
                .await
                .map_err(|error| format!("RTXHDR-CFG-004: {error}"))?;
        }
    }

    // Clearing the Core path is the uninstall commit. Cleanup is best effort:
    // a racing process or antivirus scanner may still hold a file temporarily,
    // but that must not restore or corrupt configuration that was safely cleared.
    let cleanup_detail = match remove_component_with_elevation().await {
        Ok(()) => String::new(),
        Err(error) => format!("RTXHDR-PKG-007: component disabled; deferred file cleanup: {error}"),
    };
    Ok(build_status(&config, false, cleanup_detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_pe(path: &Path, machine: u16, dll: bool, magic: u16, sections: u16) {
        const PE_OFFSET: usize = 0x80;
        const OPTIONAL_SIZE: usize = 0xf0;
        const RAW_OFFSET: usize = 0x400;
        const RAW_SIZE: usize = 0x200;
        let size = (PE_OFFSET + 24 + OPTIONAL_SIZE + usize::from(sections) * 40)
            .max(RAW_OFFSET + RAW_SIZE);
        let mut bytes = vec![0u8; size];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
        bytes[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
        bytes[PE_OFFSET + 4..PE_OFFSET + 6].copy_from_slice(&machine.to_le_bytes());
        bytes[PE_OFFSET + 6..PE_OFFSET + 8].copy_from_slice(&sections.to_le_bytes());
        bytes[PE_OFFSET + 20..PE_OFFSET + 22]
            .copy_from_slice(&(OPTIONAL_SIZE as u16).to_le_bytes());
        let characteristics: u16 = if dll { 0x2000 } else { 0 };
        bytes[PE_OFFSET + 22..PE_OFFSET + 24].copy_from_slice(&characteristics.to_le_bytes());
        bytes[PE_OFFSET + 24..PE_OFFSET + 26].copy_from_slice(&magic.to_le_bytes());
        bytes[PE_OFFSET + 24 + 56..PE_OFFSET + 24 + 60].copy_from_slice(&(0x2000u32).to_le_bytes());
        bytes[PE_OFFSET + 24 + 60..PE_OFFSET + 24 + 64]
            .copy_from_slice(&(RAW_OFFSET as u32).to_le_bytes());
        if sections > 0 {
            let section = PE_OFFSET + 24 + OPTIONAL_SIZE;
            bytes[section + 8..section + 12].copy_from_slice(&(RAW_SIZE as u32).to_le_bytes());
            bytes[section + 12..section + 16].copy_from_slice(&(0x1000u32).to_le_bytes());
            bytes[section + 16..section + 20].copy_from_slice(&(RAW_SIZE as u32).to_le_bytes());
            bytes[section + 20..section + 24].copy_from_slice(&(RAW_OFFSET as u32).to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn accepts_x64_pe32_plus_dll() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-pe-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join(BACKEND_FILE);
        write_test_pe(&path, 0x8664, true, 0x20b, 1);
        assert!(validate_named_dll(&path, BACKEND_FILE, MAX_BACKEND_BYTES).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_wrong_machine_kind_or_truncated_headers() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-bad-pe-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        for (name, machine, dll, magic, sections) in [
            ("x86", 0x014c, true, 0x10b, 1),
            ("exe", 0x8664, false, 0x20b, 1),
            ("pe32", 0x8664, true, 0x10b, 1),
            ("nosections", 0x8664, true, 0x20b, 0),
        ] {
            let path = root.join(format!("{name}.dll"));
            write_test_pe(&path, machine, dll, magic, sections);
            assert!(validate_pe_x64(&path).is_err(), "{name} should be rejected");
        }
        let truncated = root.join("truncated.dll");
        fs::write(&truncated, b"MZ").unwrap();
        assert!(validate_pe_x64(&truncated).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_misnamed_component_files() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-name-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("unknown.dll");
        write_test_pe(&path, 0x8664, true, 0x20b, 1);
        assert!(validate_named_dll(&path, BACKEND_FILE, MAX_BACKEND_BYTES).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accepts_only_one_level_hash_version_paths() {
        let version = versions_dir().join("abc-def").join(BACKEND_FILE);
        assert!(is_managed_backend_path(&version));
        assert!(!is_managed_backend_path(
            &versions_dir()
                .join("abc-def")
                .join("nested")
                .join(BACKEND_FILE)
        ));
        assert!(!is_managed_backend_path(
            &component_root().join(BACKEND_FILE)
        ));
    }
}
