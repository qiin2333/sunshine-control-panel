use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

mod runtime_transaction;

#[cfg(test)]
use crate::native_components::install::validate_pe_x64;
use crate::native_components::install::{permission_error, sha256_file, validate_named_dll};
use crate::native_components::operation::COMPONENT_OPERATION;
#[cfg(target_os = "windows")]
use crate::native_components::operation::{ensure_helper_finished, wait_for_component_helper};
use crate::native_components::{
    NVIDIA_RTX_VIDEO_ID, RTX_HDR_RUNTIME as RUNTIME_FILE, RTX_VIDEO_BRIDGE as BRIDGE_FILE,
};
const MAX_BRIDGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RUNTIME_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(target_os = "windows")]
const ELEVATED_INSTALL_ARG: &str = "--elevated-rtx-hdr-install";
#[cfg(target_os = "windows")]
const ELEVATED_REMOVE_ARG: &str = "--elevated-rtx-hdr-remove";
#[cfg(target_os = "windows")]
const ELEVATED_RECOVER_ARG: &str = "--elevated-rtx-hdr-recover";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ComponentManifest {
    schema: u32,
    component_id: String,
    bridge_sha256: String,
    runtime_sha256: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RtxHdrComponentStatus {
    pub component_id: String,
    pub enabled: bool,
    pub download_available: bool,
    pub state: String,
    pub installed: bool,
    pub ready: bool,
    pub in_use: bool,
    pub maintenance: bool,
    pub bridge_present: bool,
    pub runtime_present: bool,
    pub configured: bool,
    pub managed_path: String,
    pub bridge_sha256: String,
    pub runtime_sha256: String,
}

fn component_root() -> PathBuf {
    crate::native_components::core_component_root(NVIDIA_RTX_VIDEO_ID)
        .expect("RTX HDR must be registered as a Core component")
}

fn component_id(bridge_hash: &str, runtime_hash: &str) -> String {
    format!("{}-{}", &bridge_hash[..16], &runtime_hash[..16])
}

fn read_manifest(directory: &Path) -> Option<ComponentManifest> {
    let mut bytes = Vec::new();
    File::open(directory.join("component.json"))
        .ok()?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 64 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn trusted_manifest(component_id: &str) -> Result<ComponentManifest, String> {
    let path = crate::sunshine::install_dir()
        .join("assets")
        .join("hdr-components.json");
    let file = File::open(path)
        .map_err(|_| "HDR-PKG-009: trusted component versions are not configured".to_string())?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "HDR-PKG-009: unable to read trusted component versions".to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("HDR-PKG-009: invalid trusted component versions".to_string());
    }
    let catalog: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "HDR-PKG-009: invalid trusted component versions".to_string())?;
    if catalog
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err("HDR-PKG-009: unsupported trusted component schema".to_string());
    }
    let trusted = catalog
        .get("components")
        .and_then(|components| components.get(NVIDIA_RTX_VIDEO_ID))
        .and_then(|versions| versions.get(component_id))
        .ok_or_else(|| {
            "HDR-PKG-009: this component version is not trusted by this installation".to_string()
        })?;
    let bridge_sha256 = trusted
        .get(BRIDGE_FILE)
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "HDR-PKG-009: invalid trusted component versions".to_string())?;
    let runtime_sha256 = trusted
        .get(RUNTIME_FILE)
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "HDR-PKG-009: invalid trusted component versions".to_string())?;
    Ok(ComponentManifest {
        schema: 1,
        component_id: component_id.to_string(),
        bridge_sha256: bridge_sha256.to_ascii_lowercase(),
        runtime_sha256: runtime_sha256.to_ascii_lowercase(),
    })
}

fn validate_version(directory: &Path, expected: &ComponentManifest) -> bool {
    let bridge = directory.join(BRIDGE_FILE);
    let runtime = directory.join(RUNTIME_FILE);
    let Some(stored) = read_manifest(directory) else {
        return false;
    };
    let Ok(canonical_directory) = directory.canonicalize() else {
        return false;
    };
    if [bridge.as_path(), runtime.as_path()].iter().any(|path| {
        path.canonicalize().ok().as_deref().and_then(Path::parent)
            != Some(canonical_directory.as_path())
    }) {
        return false;
    }
    stored.schema == 1
        && stored.component_id == expected.component_id
        && stored.bridge_sha256 == expected.bridge_sha256
        && stored.runtime_sha256 == expected.runtime_sha256
        && validate_named_dll(&bridge, BRIDGE_FILE, MAX_BRIDGE_BYTES).is_ok()
        && validate_named_dll(&runtime, RUNTIME_FILE, MAX_RUNTIME_BYTES).is_ok()
        && sha256_file(&bridge, MAX_BRIDGE_BYTES).ok().as_deref()
            == Some(expected.bridge_sha256.as_str())
        && sha256_file(&runtime, MAX_RUNTIME_BYTES).ok().as_deref()
            == Some(expected.runtime_sha256.as_str())
}

fn build_status(
    config: &crate::hdr_enhanced::ConfigState,
    runtime_status: &serde_json::Value,
) -> RtxHdrComponentStatus {
    let version = config
        .settings
        .backends
        .get(NVIDIA_RTX_VIDEO_ID)
        .map(|value| value.version.as_str());
    let directory = component_root();
    let manifest = version.and_then(|version| trusted_manifest(version).ok());
    let bridge = directory.join(BRIDGE_FILE);
    let bridge_present = bridge.is_file();
    let runtime_present = directory.join(RUNTIME_FILE).is_file();
    let integrity_valid = manifest
        .as_ref()
        .is_some_and(|manifest| validate_version(&directory, manifest));
    let enabled = config.settings.selected_backend.as_deref() == Some(NVIDIA_RTX_VIDEO_ID);
    let in_use = runtime_status
        .get("in_use")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let pipelines = runtime_status
        .get("pipelines")
        .and_then(serde_json::Value::as_array);
    let has_state = |state: &str| {
        pipelines.is_some_and(|pipelines| {
            pipelines.iter().any(|pipeline| {
                pipeline.get("backend").and_then(serde_json::Value::as_str)
                    == Some(NVIDIA_RTX_VIDEO_ID)
                    && pipeline.get("state").and_then(serde_json::Value::as_str) == Some(state)
            })
        })
    };
    // 任一处理链降级时不能被另一个正常会话的 active 状态掩盖。
    let degraded = has_state("degraded");
    let ready = has_state("active") && !degraded;
    let selection_verified = runtime_status
        .get("selection_verified")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    let state = if degraded {
        "degraded"
    } else if ready {
        "active"
    } else if version.is_none() {
        "not_installed"
    } else if !integrity_valid || (enabled && !selection_verified) {
        "repair_required"
    } else if enabled {
        "selected"
    } else {
        "configured"
    };
    RtxHdrComponentStatus {
        component_id: NVIDIA_RTX_VIDEO_ID.to_string(),
        enabled,
        download_available: false,
        state: state.to_string(),
        installed: version.is_some(),
        ready,
        in_use,
        maintenance: runtime_status
            .get("maintenance")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        bridge_present,
        runtime_present,
        configured: version.is_some(),
        managed_path: bridge.to_string_lossy().into_owned(),
        bridge_sha256: manifest
            .as_ref()
            .map(|value| value.bridge_sha256.clone())
            .unwrap_or_default(),
        runtime_sha256: manifest
            .as_ref()
            .map(|value| value.runtime_sha256.clone())
            .unwrap_or_default(),
    }
}

async fn ensure_idle() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    ensure_helper_finished().await?;
    let state = crate::sunshine::get_tray_state()
        .await
        .map_err(|_| "HDR-SESSION-001: unable to verify stream state".to_string())?;
    if !state.sessions.is_empty() {
        return Err(
            "RTXHDR-SESSION-002: finish all active streams before changing the RTX HDR component"
                .to_string(),
        );
    }
    Ok(())
}

async fn status_for_config(
    config: crate::hdr_enhanced::ConfigState,
) -> Result<RtxHdrComponentStatus, String> {
    let runtime = crate::hdr_enhanced::get_status().await?;
    tokio::task::spawn_blocking(move || build_status(&config, &runtime))
        .await
        .map_err(|_| "HDR-PKG-004: component status check failed".to_string())
}

fn verify_source_trust(manifest: &ComponentManifest) -> Result<(), String> {
    if trusted_manifest(&manifest.component_id)? != *manifest {
        return Err(
            "HDR-PKG-009: this component version is not trusted by this installation".to_string(),
        );
    }
    Ok(())
}

fn validate_component_root() -> Result<PathBuf, String> {
    let expected = crate::sunshine::install_dir()
        .canonicalize()
        .map_err(|_| "HDR-PKG-004: invalid installation directory".to_string())?
        .join("tools")
        .join("hdr_enhanced")
        .join("nvidia_rtx_video");
    let actual = component_root()
        .canonicalize()
        .map_err(|_| "HDR-PKG-004: invalid component directory".to_string())?;
    if actual != expected {
        return Err("HDR-PKG-004: component directory must not redirect elsewhere".to_string());
    }
    Ok(actual)
}

fn install_runtime(
    runtime_source: &Path,
    manifest: &ComponentManifest,
    commit_config: impl FnOnce() -> Result<(), String>,
) -> Result<PathBuf, String> {
    verify_source_trust(manifest)?;
    validate_named_dll(runtime_source, RUNTIME_FILE, MAX_RUNTIME_BYTES)?;
    let root = validate_component_root()?;
    let bridge = root.join(BRIDGE_FILE);
    validate_named_dll(&bridge, BRIDGE_FILE, MAX_BRIDGE_BYTES)?;
    if sha256_file(&bridge, MAX_BRIDGE_BYTES).ok().as_deref()
        != Some(manifest.bridge_sha256.as_str())
    {
        return Err("HDR-PKG-009: installed bridge does not match trusted metadata".to_string());
    }
    if validate_version(&root, manifest) {
        commit_config()?;
        return Ok(root);
    }

    let staging = root.join(format!("staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging)
        .map_err(|error| permission_error("create staging directory", error))?;
    let prepared = (|| -> Result<(), String> {
        runtime_transaction::copy_bounded(
            runtime_source,
            &staging.join(RUNTIME_FILE),
            MAX_RUNTIME_BYTES,
        )?;
        fs::write(
            staging.join("component.json"),
            serde_json::to_vec_pretty(manifest)
                .map_err(|_| "HDR-PKG-004: invalid component manifest".to_string())?,
        )
        .map_err(|error| permission_error("write component manifest", error))?;
        if sha256_file(&staging.join(RUNTIME_FILE), MAX_RUNTIME_BYTES)
            .ok()
            .as_deref()
            != Some(manifest.runtime_sha256.as_str())
        {
            return Err("HDR-PKG-006: staged component failed integrity validation".to_string());
        }
        runtime_transaction::prepare(&root, manifest)
    })();
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let activated = (|| -> Result<(), String> {
        for name in [RUNTIME_FILE, "component.json"] {
            let target = root.join(name);
            if target.symlink_metadata().is_ok() {
                fs::remove_file(&target)
                    .map_err(|error| permission_error("replace runtime file", error))?;
            }
            fs::rename(staging.join(name), target)
                .map_err(|error| permission_error("activate runtime file", error))?;
        }
        if !validate_version(&root, manifest) {
            return Err("HDR-PKG-006: installed component failed integrity validation".to_string());
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    if let Err(error) = activated {
        runtime_transaction::settle(&root, None)?;
        return Err(error);
    }
    // 响应丢失不能假定保存失败；保留备份，由恢复流程读取 Core 的实际版本。
    commit_config()?;
    runtime_transaction::settle(&root, Some(&manifest.component_id))?;
    Ok(root)
}

async fn publish_installed_version(
    manifest: &ComponentManifest,
    operation_id: &str,
) -> Result<(), String> {
    let mut current = crate::hdr_enhanced::get_config().await?;
    current.settings.backends.insert(
        NVIDIA_RTX_VIDEO_ID.to_string(),
        crate::hdr_enhanced::BackendSettings {
            version: manifest.component_id.clone(),
        },
    );
    crate::hdr_enhanced::save_config(current, Some(operation_id)).await?;
    Ok(())
}

async fn recover_runtime_files() -> Result<(), String> {
    let current = crate::hdr_enhanced::get_config().await?;
    let version = current
        .settings
        .backends
        .get(NVIDIA_RTX_VIDEO_ID)
        .map(|value| value.version.clone());
    tokio::task::spawn_blocking(move || {
        let root = validate_component_root()?;
        runtime_transaction::settle(&root, version.as_deref())
    })
    .await
    .map_err(|_| "HDR-PKG-004: component recovery failed".to_string())?
}

fn manifest_for_sources(
    bridge_source: &Path,
    runtime_source: &Path,
) -> Result<ComponentManifest, String> {
    validate_named_dll(bridge_source, BRIDGE_FILE, MAX_BRIDGE_BYTES)?;
    validate_named_dll(runtime_source, RUNTIME_FILE, MAX_RUNTIME_BYTES)?;
    let bridge_hash = sha256_file(bridge_source, MAX_BRIDGE_BYTES)
        .map_err(|error| format!("RTXHDR-PKG-004: hash bridge failed: {error}"))?;
    let runtime_hash = sha256_file(runtime_source, MAX_RUNTIME_BYTES)
        .map_err(|error| format!("RTXHDR-PKG-004: hash runtime failed: {error}"))?;
    Ok(ComponentManifest {
        schema: 1,
        component_id: component_id(&bridge_hash, &runtime_hash),
        bridge_sha256: bridge_hash,
        runtime_sha256: runtime_hash,
    })
}

fn manifest_for_runtime(runtime_source: &Path) -> Result<ComponentManifest, String> {
    manifest_for_sources(&component_root().join(BRIDGE_FILE), runtime_source)
}

fn remove_managed_runtime() -> Result<(), String> {
    let root = component_root();
    if !root.exists() {
        return Ok(());
    }
    let root = validate_component_root()?;
    for path in [root.join(RUNTIME_FILE), root.join("component.json")] {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| permission_error("remove NVIDIA runtime", error))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next()?;
    let command = command.to_str()?;
    if command != ELEVATED_INSTALL_ARG
        && command != ELEVATED_REMOVE_ARG
        && command != ELEVATED_RECOVER_ARG
    {
        return None;
    }
    let mut run = || -> Result<(), String> {
        let runtime = if command == ELEVATED_INSTALL_ARG {
            Some(PathBuf::from(args.next().ok_or("missing runtime")?))
        } else {
            None
        };
        let id = args
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("missing operation")?;
        if args.next().is_some() || id.is_empty() {
            return Err("invalid operation".to_string());
        }
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "HDR-OP-002: unable to initialize component helper".to_string())?;
        // 维护路径来自运行中的 Core，不能由命令行提供另一份自制凭据。
        let operation = executor.block_on(crate::hdr_enhanced::verify_maintenance(&id))?;
        crate::native_components::operation::with_maintenance(&operation, || {
            executor.block_on(recover_runtime_files())?;
            if let Some(runtime) = runtime {
                let manifest = manifest_for_runtime(&runtime)?;
                install_runtime(&runtime, &manifest, || {
                    executor.block_on(publish_installed_version(&manifest, &id))
                })?;
                Ok(())
            } else if command == ELEVATED_REMOVE_ARG {
                remove_managed_runtime()
            } else {
                Ok(())
            }
        })
    };
    Some(if run().is_ok() { 0 } else { 1 })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    None
}

async fn install_runtime_with_elevation(
    runtime_source: &Path,
    manifest: &ComponentManifest,
    operation: &crate::hdr_enhanced::Maintenance,
) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let runtime = runtime_source.to_owned();
        let manifest = manifest.clone();
        let operation = crate::hdr_enhanced::verify_maintenance(&operation.id).await?;
        if crate::utils::is_running_as_admin()? {
            let handle = tokio::runtime::Handle::current();
            return tokio::task::spawn_blocking(move || {
                crate::native_components::operation::with_maintenance(&operation, || {
                    handle.block_on(recover_runtime_files())?;
                    install_runtime(&runtime, &manifest, || {
                        handle.block_on(publish_installed_version(&manifest, &operation.id))
                    })
                })
            })
            .await
            .map_err(|_| "HDR-PKG-004: installation task failed".to_string())?;
        }
        let runtime = runtime.to_string_lossy().into_owned();
        let id = operation.id;
        let process = tokio::task::spawn_blocking(move || {
            crate::utils::launch_current_executable_elevated(
                &[ELEVATED_INSTALL_ARG, &runtime, &id],
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
            )
        })
        .await
        .map_err(|_| "HDR-OP-002: unable to start installation".to_string())??;
        let code = wait_for_component_helper(process).await?;
        if code != 0 {
            return Err("HDR-PKG-004: component helper rejected installation".to_string());
        }
        tokio::task::spawn_blocking(move || {
            let root = component_root();
            if !validate_version(&root, &manifest) {
                return Err("HDR-PKG-006: installed version failed validation".to_string());
            }
            Ok(root)
        })
        .await
        .map_err(|_| "HDR-PKG-004: installation verification failed".to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (runtime_source, manifest, operation);
        Err("HDR-PLATFORM: Windows is required".to_string())
    }
}

async fn remove_component_with_elevation(
    operation: &crate::hdr_enhanced::Maintenance,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let operation = crate::hdr_enhanced::verify_maintenance(&operation.id).await?;
        if crate::utils::is_running_as_admin()? {
            return tokio::task::spawn_blocking(move || {
                crate::native_components::operation::with_maintenance(
                    &operation,
                    remove_managed_runtime,
                )
            })
            .await
            .map_err(|_| "HDR-PKG-004: removal task failed".to_string())?;
        }
        let id = operation.id;
        let process = tokio::task::spawn_blocking(move || {
            crate::utils::launch_current_executable_elevated(
                &[ELEVATED_REMOVE_ARG, &id],
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
            )
        })
        .await
        .map_err(|_| "HDR-OP-002: unable to start removal".to_string())??;
        let code = wait_for_component_helper(process).await?;
        if code != 0 {
            return Err("HDR-PKG-004: component helper rejected removal".to_string());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = operation;
        Err("HDR-PLATFORM: Windows is required".to_string())
    }
}

pub async fn rtx_hdr_get_status() -> Result<RtxHdrComponentStatus, String> {
    status_for_config(crate::hdr_enhanced::get_config().await?).await
}

async fn finish_operation(operation_id: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    ensure_helper_finished().await?;
    crate::hdr_enhanced::finish_maintenance(operation_id).await
}

pub async fn rtx_hdr_install(runtime_path: String) -> Result<RtxHdrComponentStatus, String> {
    let _operation = COMPONENT_OPERATION
        .try_lock()
        .map_err(|_| "HDR-OP-001: another component operation is running".to_string())?;
    ensure_idle().await?;
    let runtime_source = PathBuf::from(runtime_path);
    let runtime = runtime_source.clone();
    let manifest = tokio::task::spawn_blocking(move || {
        let manifest = manifest_for_runtime(&runtime)?;
        verify_source_trust(&manifest)?;
        Ok::<_, String>(manifest)
    })
    .await
    .map_err(|_| "HDR-PKG-004: component validation failed".to_string())??;
    let operation_id = crate::hdr_enhanced::begin_maintenance().await?;
    let installed = install_runtime_with_elevation(&runtime_source, &manifest, &operation_id).await;
    if let Err(error) = installed {
        // UAC 取消等尚未替换文件的失败可直接结束；助手存活或有恢复资料时仍保留维护。
        if !runtime_transaction::pending(&component_root()) {
            let _ = finish_operation(&operation_id.id).await;
        }
        return Err(error);
    }
    finish_operation(&operation_id.id).await?;
    rtx_hdr_get_status().await
}

pub async fn rtx_hdr_recover() -> Result<RtxHdrComponentStatus, String> {
    #[cfg(target_os = "windows")]
    {
        ensure_helper_finished().await?;
        // 只有待恢复文件确实存在时才再次请求管理员权限。
        if runtime_transaction::pending(&component_root()) {
            let operation = crate::hdr_enhanced::inspect_maintenance().await?;
            if crate::utils::is_running_as_admin()? {
                let handle = tokio::runtime::Handle::current();
                tokio::task::spawn_blocking(move || {
                    crate::native_components::operation::with_maintenance(&operation, || {
                        handle.block_on(recover_runtime_files())
                    })
                })
                .await
                .map_err(|_| "HDR-PKG-004: component recovery failed".to_string())??;
            } else {
                let process = tokio::task::spawn_blocking(move || {
                    crate::utils::launch_current_executable_elevated(
                        &[ELEVATED_RECOVER_ARG, &operation.id],
                        windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
                    )
                })
                .await
                .map_err(|_| "HDR-OP-002: unable to start recovery".to_string())??;
                if wait_for_component_helper(process).await? != 0 {
                    return Err("HDR-PKG-004: component recovery failed".to_string());
                }
            }
        }
    }
    crate::hdr_enhanced::recover_maintenance().await?;
    rtx_hdr_get_status().await
}

pub async fn rtx_hdr_set_enabled(enabled: bool) -> Result<RtxHdrComponentStatus, String> {
    let _operation = COMPONENT_OPERATION
        .try_lock()
        .map_err(|_| "HDR-OP-001: another component operation is running".to_string())?;
    let mut current = crate::hdr_enhanced::get_config().await?;
    if enabled && !current.settings.backends.contains_key(NVIDIA_RTX_VIDEO_ID) {
        return Err("HDR-PKG-006: import the component before selecting it".to_string());
    }
    current.settings.selected_backend = enabled.then(|| NVIDIA_RTX_VIDEO_ID.to_string());
    let saved = crate::hdr_enhanced::save_config(current, None).await?;
    status_for_config(saved).await
}

pub async fn rtx_hdr_uninstall() -> Result<RtxHdrComponentStatus, String> {
    let _operation = COMPONENT_OPERATION
        .try_lock()
        .map_err(|_| "HDR-OP-001: another component operation is running".to_string())?;
    ensure_idle().await?;
    let mut current = crate::hdr_enhanced::get_config().await?;
    let operation_id = crate::hdr_enhanced::begin_maintenance().await?;
    if current.settings.selected_backend.as_deref() == Some(NVIDIA_RTX_VIDEO_ID) {
        current.settings.selected_backend = None;
    }
    // 先停止新会话使用；文件删除成功后再去掉版本记录，失败时仍保留卸载/修复入口。
    let saved = crate::hdr_enhanced::save_config(current, Some(&operation_id.id)).await;
    let saved = match saved {
        Ok(saved) => saved,
        Err(error) => {
            let _ = finish_operation(&operation_id.id).await;
            return Err(error);
        }
    };
    let removed = remove_component_with_elevation(&operation_id).await;
    if let Err(error) = removed {
        let _ = finish_operation(&operation_id.id).await;
        return Err(error);
    }
    let mut cleared = saved;
    cleared.settings.backends.remove(NVIDIA_RTX_VIDEO_ID);
    let saved = crate::hdr_enhanced::save_config(cleared, Some(&operation_id.id)).await;
    let finished = finish_operation(&operation_id.id).await;
    let saved = saved?;
    finished?;
    status_for_config(saved).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_is_not_runtime_verification() {
        let config = crate::hdr_enhanced::ConfigState {
            settings: crate::hdr_enhanced::Settings {
                schema_version: 1,
                selected_backend: None,
                backends: Default::default(),
            },
            etag: reqwest::header::HeaderValue::from_static("\"hdr-v1-test\""),
        };
        let status = build_status(&config, &serde_json::json!({"in_use":false}));
        assert!(!status.ready);
        assert!(!status.download_available);
        assert_eq!(status.component_id, NVIDIA_RTX_VIDEO_ID);
        assert_eq!(status.state, "not_installed");
    }

    #[test]
    fn degraded_pipeline_is_not_hidden_by_an_active_pipeline() {
        let config = crate::hdr_enhanced::ConfigState {
            settings: crate::hdr_enhanced::Settings {
                schema_version: 1,
                selected_backend: None,
                backends: Default::default(),
            },
            etag: reqwest::header::HeaderValue::from_static("\"unused\""),
        };
        let runtime = serde_json::json!({"pipelines":[
            {"backend":NVIDIA_RTX_VIDEO_ID,"state":"active"},
            {"backend":NVIDIA_RTX_VIDEO_ID,"state":"degraded"}
        ]});
        let status = build_status(&config, &runtime);
        assert_eq!(status.state, "degraded");
        assert!(!status.ready);
    }

    #[test]
    fn component_hashing_is_bounded() {
        let path = std::env::temp_dir().join(format!("hdr-hash-{}", uuid::Uuid::new_v4()));
        fs::write(&path, b"abc").unwrap();
        assert!(sha256_file(&path, 2).is_err());
        assert_eq!(
            sha256_file(&path, 3).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn static_integrity_check_does_not_execute_imported_dlls() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-status-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let bridge = root.join(BRIDGE_FILE);
        let runtime = root.join(RUNTIME_FILE);
        write_test_pe(&bridge, 0x8664, true, 0x20b, 1);
        write_test_pe(&runtime, 0x8664, true, 0x20b, 1);
        let manifest = manifest_for_sources(&bridge, &runtime).unwrap();
        fs::write(
            root.join("component.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(validate_version(&root, &manifest));
        // 文件校验只确认已安装的数据，不执行 DLL，也不证明后端导出或显卡支持。
        assert!(
            crate::native_components::descriptor(NVIDIA_RTX_VIDEO_ID)
                .unwrap()
                .distribution
                .local_import
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn saved_version_survives_a_lost_install_response() {
        let root = std::env::temp_dir().join(format!("hdr-committed-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let bridge = root.join(BRIDGE_FILE);
        let runtime = root.join(RUNTIME_FILE);
        write_test_pe(&bridge, 0x8664, true, 0x20b, 1);
        write_test_pe(&runtime, 0x8664, true, 0x20b, 1);
        let manifest = manifest_for_sources(&bridge, &runtime).unwrap();
        fs::remove_file(&runtime).unwrap();
        runtime_transaction::prepare(&root, &manifest).unwrap();
        write_test_pe(&runtime, 0x8664, true, 0x20b, 1);
        fs::write(
            root.join("component.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        // Core 已经提交版本，只是 Panel 没有收到响应；恢复必须保留新文件。
        runtime_transaction::settle(&root, Some(&manifest.component_id)).unwrap();
        assert!(validate_version(&root, &manifest));
        assert!(!runtime_transaction::pending(&root));
        fs::remove_dir_all(root).unwrap();
    }

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
        let path = root.join(BRIDGE_FILE);
        write_test_pe(&path, 0x8664, true, 0x20b, 1);
        assert!(validate_named_dll(&path, BRIDGE_FILE, MAX_BRIDGE_BYTES).is_ok());
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
        assert!(validate_named_dll(&path, BRIDGE_FILE, MAX_BRIDGE_BYTES).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
