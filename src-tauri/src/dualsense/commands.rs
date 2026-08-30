//! Tauri commands for the DualSense component lifecycle.

use log::info;
use std::path::PathBuf;

use super::config::{
    CONFIG_APPLY_TIMEOUT, clamp_tuning, get_core_ds5_settings, require_entity_tag,
    save_core_ds5_settings, update_config_fields, update_tuning_fields,
};
#[cfg(target_os = "windows")]
use super::elevated::{ElevatedOperation, run_elevated_operation};
#[cfg(not(target_os = "windows"))]
use super::emit_progress;
#[cfg(not(target_os = "windows"))]
use super::install::{dualsense_install_impl, dualsense_self_test_impl, dualsense_uninstall_impl};
#[cfg(not(target_os = "windows"))]
use super::packages::classify_local_component_packages;
use super::packages::sidecar_path;
use super::probe::{
    dualsense_get_status_with_config, ensure_no_active_session,
    ensure_no_active_session_for_uninstall, installed_usbip_version, pinned_usbip_installed,
    run_probe, validate_requested_profile,
};
use super::{
    COMPONENT_OPERATION, DualSenseStatus, DualSenseTuningResult, MAX_LOCAL_COMPONENT_PACKAGES,
    observe_config_revision,
};

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
    observe_config_revision(applied.revision);
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
    dualsense_get_status_with_config(None).await
}

#[tauri::command]
pub fn dualsense_log_panel_opened() {
    info!("DualSense settings panel opened");
}

#[tauri::command]
pub async fn dualsense_install(
    app: tauri::AppHandle,
    package_paths: Vec<String>,
) -> Result<DualSenseStatus, String> {
    let _operation = COMPONENT_OPERATION.try_lock().map_err(|_| {
        "DS5-RUN-002: another DualSense component operation is still running".to_string()
    })?;
    ensure_no_active_session().await?;
    let selected_packages = package_paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if selected_packages.len() > MAX_LOCAL_COMPONENT_PACKAGES {
        return Err("DS5-PKG-002: select no more than three local component packages".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let operation = if selected_packages.is_empty() {
            ElevatedOperation::Install
        } else {
            ElevatedOperation::InstallLocal
        };
        let data = run_elevated_operation(Some(&app), operation, &selected_packages).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let progress = |stage: &str, value: u32| emit_progress(&app, stage, value);
        let local_packages = tokio::task::spawn_blocking(move || {
            classify_local_component_packages(selected_packages)
        })
        .await
        .map_err(|error| {
            format!("DS5-PKG-002: local package classification task failed: {error}")
        })??;
        dualsense_install_impl(&progress, local_packages).await
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
    update_config_fields(&mut settings, enabled, audio_haptics, genshin_compatibility);
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
        return run_elevated_operation(None, operation, &[]).await;
    }
    #[cfg(not(target_os = "windows"))]
    {
        dualsense_self_test_impl(profile).await
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
        let data = run_elevated_operation(None, ElevatedOperation::Uninstall, &[]).await?;
        return serde_json::from_value(data)
            .map_err(|error| format!("DS5-PKG-003: invalid administrator result: {error}"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        dualsense_uninstall_impl().await
    }
}
