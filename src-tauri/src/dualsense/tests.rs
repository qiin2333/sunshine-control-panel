use super::{
    CoreDualSenseResponse, CoreDualSenseSettings, InstalledComponentManifest,
    SidecarPackageManifest, UsbipInstallResult, clamp_tuning, classify_usbip_installer_exit_code,
    component_is_verified, component_matches_current_runtime, component_state,
    component_test_failure, component_update_available, copy_runtime_files, core_ds5_http_error,
    extract_sidecar_package, local_uninstalled_status, pinned_usbip_installed,
    read_sidecar_package_manifest, recover_interrupted_activation, require_entity_tag,
    resolve_core_config, rollback_activated_component, run_with_timeout, sha256_file,
    update_config_fields, update_tuning_fields, validate_core_ds5_response,
    validate_requested_profile, validate_sidecar_package_manifest, validate_strong_entity_tag,
};
#[cfg(target_os = "windows")]
use super::{
    ElevatedMessage, ElevatedOperation, MAX_ELEVATED_MESSAGE_BYTES, elevated_pipe_name,
    read_limited_elevated_line, wait_for_elevated_pipe_connection,
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
fn interrupted_activation_restores_a_valid_backup() {
    let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
    let active = root.join("active");
    let backup = root.join("previous");
    write_valid_component(&backup, b"previous");

    assert!(recover_interrupted_activation(&active, &backup).unwrap());
    assert!(active.exists());
    assert!(!backup.exists());
    assert!(!recover_interrupted_activation(&active, &backup).unwrap());
    std::fs::remove_dir_all(root).unwrap();
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
    assert!(component_matches_current_runtime(
        Some(&current),
        true,
        true
    ));
    assert!(!component_matches_current_runtime(
        Some(&current),
        false,
        true
    ));
    assert!(!component_matches_current_runtime(
        Some(&current),
        true,
        false
    ));
    assert!(!component_update_available(None));
    assert!(!component_matches_current_runtime(None, true, true));

    let outdated = InstalledComponentManifest {
        component_version: "1.0.0".to_string(),
        ..current
    };
    assert!(component_update_available(Some(&outdated)));
    assert!(!component_matches_current_runtime(
        Some(&outdated),
        true,
        true
    ));
    assert!(component_is_verified(true, true, false));
    assert!(!component_is_verified(false, true, false));
    assert!(component_is_verified(true, false, true));
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
fn sidecar_package_manifest_read_is_bounded() {
    let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1,
            "component_version": super::COMPONENT_VERSION,
            "protocol": super::PROTOCOL_VERSION,
            "target": super::SIDECAR_PACKAGE_TARGET,
            "license": super::SIDECAR_PACKAGE_LICENSE,
            "asset_name": super::SIDECAR_PACKAGE_ASSET,
            "download_url": "",
            "sha256": "a".repeat(64),
            "size": 1024,
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(read_sidecar_package_manifest(&manifest_path).is_ok());

    std::fs::write(
        &manifest_path,
        vec![b'x'; super::MAX_SIDECAR_PACKAGE_MANIFEST_BYTES as usize + 1],
    )
    .unwrap();
    let error = read_sidecar_package_manifest(&manifest_path).unwrap_err();
    assert!(error.contains("manifest is too large"));
    std::fs::remove_dir_all(root).unwrap();
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

#[cfg(target_os = "windows")]
#[test]
fn component_filesystem_operation_retries_windows_sharing_violations() {
    let mut attempts = 0;

    let result = super::retry_component_fs_operation(|| {
        attempts += 1;
        if attempts == 1 {
            Err(std::io::Error::from_raw_os_error(32))
        } else {
            Ok("renamed")
        }
    })
    .unwrap();

    assert_eq!(result, "renamed");
    assert_eq!(attempts, 2);
}

#[test]
fn component_filesystem_operation_does_not_retry_structural_errors() {
    let mut attempts = 0;

    let error = super::retry_component_fs_operation(|| {
        attempts += 1;
        Err::<(), _>(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid destination",
        ))
    })
    .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(attempts, 1);
}

#[test]
fn component_filesystem_error_includes_the_operating_system_code() {
    let error = std::io::Error::from_raw_os_error(5);

    let message = super::component_fs_error("unable to activate component", &error);

    assert!(message.starts_with("unable to activate component (OS error 5):"));
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
    let stale = root.join("handoff-stale.partial");
    let legacy_stale = root.join("handoff-legacy.partial.zip");
    let current = root.join("handoff-current.partial");
    let unrelated = root.join("other.partial.zip");
    let matching_directory = root.join("handoff-directory.partial.zip");
    std::fs::create_dir_all(&matching_directory).unwrap();
    std::fs::write(&stale, b"stale").unwrap();
    std::fs::write(&legacy_stale, b"legacy stale").unwrap();
    std::fs::write(&current, b"current").unwrap();
    std::fs::write(&unrelated, b"unrelated").unwrap();

    super::purge_stale_handoff_packages(&root, &[current.as_path()]);

    assert!(!stale.exists());
    assert!(!legacy_stale.exists());
    assert!(current.is_file());
    assert!(unrelated.is_file());
    assert!(matching_directory.is_dir());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_component_hashes_are_identified_without_trusting_file_names() {
    assert_eq!(
        super::local_component_kind(super::HIDMAESTRO_SHA256, None),
        Some(super::LocalComponentKind::Hidmaestro)
    );
    assert_eq!(
        super::local_component_kind(super::USBIP_SHA256, None),
        Some(super::LocalComponentKind::Usbip)
    );
    assert_eq!(
        super::local_component_kind("aabbcc", Some("AABBCC")),
        Some(super::LocalComponentKind::Sidecar)
    );
    assert_eq!(
        super::local_component_kind("unknown", Some("sidecar")),
        None
    );
}

#[test]
fn online_install_does_not_require_a_local_package_manifest_for_classification() {
    let packages =
        super::classify_local_component_packages(std::iter::empty::<std::path::PathBuf>()).unwrap();

    assert!(packages.sidecar.is_none());
    assert!(packages.hidmaestro.is_none());
    assert!(packages.usbip.is_none());
}

#[cfg(target_os = "windows")]
#[test]
fn component_download_and_install_timeouts_cover_slow_release_assets() {
    assert_eq!(
        super::COMPONENT_DOWNLOAD_OVERALL_TIMEOUT,
        std::time::Duration::from_secs(30 * 60)
    );
    assert_eq!(
        super::ElevatedOperation::Install.timeout(),
        std::time::Duration::from_secs(110 * 60)
    );
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
    assert!(
        require_entity_tag(None)
            .unwrap_err()
            .starts_with("DS5-CFG-007:")
    );
}

#[test]
fn conditional_update_errors_keep_stable_user_facing_codes() {
    assert!(
        core_ds5_http_error(
            Some("ds5_precondition_failed"),
            reqwest::StatusCode::PRECONDITION_FAILED,
        )
        .starts_with("DS5-CFG-006:")
    );
    assert!(
        core_ds5_http_error(
            Some("ds5_precondition_required"),
            reqwest::StatusCode::from_u16(428).unwrap(),
        )
        .starts_with("DS5-CFG-007:")
    );
    assert!(
        core_ds5_http_error(
            Some("ds5_if_match_invalid"),
            reqwest::StatusCode::BAD_REQUEST,
        )
        .starts_with("DS5-CFG-007:")
    );
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
async fn elevated_ipc_receives_multiple_local_packages_with_bounded_framing() {
    let root = std::env::temp_dir().join(format!("sunshine-ds5-test-{}", uuid::Uuid::new_v4()));
    let token = uuid::Uuid::new_v4();
    let contents = [
        b"sidecar".as_slice(),
        b"hidmaestro".as_slice(),
        b"usbip".as_slice(),
    ];
    let mut encoded = vec![contents.len() as u8];
    for content in contents {
        encoded.extend_from_slice(&(content.len() as u64).to_le_bytes());
        encoded.extend_from_slice(content);
    }
    let mut reader = encoded.as_slice();

    let packages = super::receive_local_component_packages_into(&mut reader, token, &root)
        .await
        .unwrap();

    assert_eq!(packages.len(), contents.len());
    for (package, expected) in packages.iter().zip(contents) {
        assert_eq!(std::fs::read(package.path()).unwrap(), expected);
    }
    let paths = packages
        .iter()
        .map(|package| package.path().to_path_buf())
        .collect::<Vec<_>>();
    drop(packages);
    assert!(paths.iter().all(|path| !path.exists()));
    std::fs::remove_dir_all(root).unwrap();
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

#[cfg(target_os = "windows")]
#[test]
#[allow(clippy::zombie_processes)]
fn process_pipe_descendant_helper() {
    match std::env::var("SUNSHINE_DS5_PIPE_HELPER").as_deref() {
        Ok("parent") => {
            std::thread::sleep(std::time::Duration::from_millis(250));
            // Intentionally exit without waiting so the descendant keeps the
            // inherited output handles open for the process-tree regression.
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "dualsense::tests::process_pipe_descendant_helper",
                    "--nocapture",
                ])
                .env("SUNSHINE_DS5_PIPE_HELPER", "child")
                .spawn()
                .unwrap();
        }
        Ok("child") => std::thread::sleep(std::time::Duration::from_secs(5)),
        _ => {}
    }
}

#[cfg(target_os = "windows")]
#[test]
fn process_timeout_does_not_wait_for_descendant_pipe_handles() {
    let timeout = std::time::Duration::from_secs(2);
    let started = std::time::Instant::now();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "dualsense::tests::process_pipe_descendant_helper",
            "--nocapture",
        ])
        .env("SUNSHINE_DS5_PIPE_HELPER", "parent");

    let output = run_with_timeout(&mut command, timeout, "test process timed out", true).unwrap();

    assert!(output.status.success());
    assert!(started.elapsed() < timeout + std::time::Duration::from_secs(1));
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
