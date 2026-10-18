//! Upgrade installed components through the existing installer and rollback path.

use super::COMPONENT_VERSION;
use super::packages::InstalledComponentManifest;

fn version(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('.');
    let result = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(result)
}

fn needs_update(manifest: Option<&InstalledComponentManifest>) -> bool {
    manifest.is_some_and(|manifest| {
        manifest.protocol == super::PROTOCOL_VERSION
            && manifest.sidecar_file == super::SIDECAR_EXE
            && version(&manifest.component_version)
                .zip(version(COMPONENT_VERSION))
                .is_some_and(|(installed, available)| installed < available)
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn installed_update_needed() -> bool {
    use super::packages::{active_dir, component_root};
    // A previous directory is also an installed component when an interrupted
    // activation left active absent. The installer already knows how to recover it.
    let active = active_dir();
    let directory = if active.exists() {
        active
    } else {
        component_root().join("previous")
    };
    if !directory.join(super::SIDECAR_EXE).is_file() {
        return false;
    }
    let manifest = std::fs::read(directory.join("component.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    needs_update(manifest.as_ref())
}

#[cfg(target_os = "windows")]
pub(crate) fn start_installed_component_update(app: tauri::AppHandle) {
    use super::COMPONENT_OPERATION;
    use super::elevated::{ElevatedOperation, run_elevated_operation};
    use super::packages::{manually_placed_sidecar_package, sidecar_package_manifest};
    use super::probe::ensure_no_active_session;
    use log::{info, warn};

    tauri::async_runtime::spawn(async move {
        loop {
            let Ok(operation) = COMPONENT_OPERATION.try_lock() else {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            };
            if !installed_update_needed() {
                return;
            }
            let Ok(package) = sidecar_package_manifest() else {
                // The host package, not the optional component installer,
                // supplies this pin. A newer standalone GUI must wait for a
                // matching Sunshine update; never reuse an older URL/digest.
                return;
            };
            if package.download_url.is_empty() && manually_placed_sidecar_package().is_none() {
                return;
            }
            if ensure_no_active_session().await.is_err() {
                drop(operation);
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
            info!("Updating the installed DualSense component to {COMPONENT_VERSION}");
            // Reuse the same authorization, verified download, staging and
            // rollback as manual updates. This mode never installs USB/IP.
            match run_elevated_operation(Some(&app), ElevatedOperation::Update, &[]).await {
                Ok(_) => info!("DualSense component update completed"),
                Err(error) => warn!(
                    "DualSense automatic update failed; manual update remains available: {error}"
                ),
            }
            // Never repeatedly prompt for elevation or retry a failed download
            // in the background. A later launch or manual update can retry.
            return;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_installed_older_components_are_automatically_updated() {
        assert!(!needs_update(None));
        let mut manifest = InstalledComponentManifest {
            protocol: super::super::PROTOCOL_VERSION,
            sidecar_file: super::super::SIDECAR_EXE.into(),
            ..Default::default()
        };
        for installed in ["1.0.0", "1.1.0", "1.2.0", "1.3.0", "1.4.0"] {
            manifest.component_version = installed.into();
            assert!(needs_update(Some(&manifest)));
        }
        for installed in [
            COMPONENT_VERSION,
            "1.4.2",
            "1.5.0",
            "2.0.0",
            "invalid",
            "1.2.0-dev",
            "1.2",
        ] {
            manifest.component_version = installed.into();
            assert!(!needs_update(Some(&manifest)));
        }
        manifest.component_version = "1.2.0".into();
        manifest.protocol += 1;
        assert!(!needs_update(Some(&manifest)));
    }
}
