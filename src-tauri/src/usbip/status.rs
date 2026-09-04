//! Transport status probing.

use super::{PINNED_VERSION, UsbipStatus};

/// Spawns a `usbip.exe port` subprocess on every call to enumerate attached
/// devices. Intended for on-demand refreshes — do not poll at high frequency.
#[tauri::command]
pub async fn usbip_get_status() -> Result<UsbipStatus, String> {
    #[cfg(not(target_os = "windows"))]
    return Ok(UsbipStatus {
        supported: false,
        installed: false,
        ready: false,
        version: String::new(),
        version_valid: false,
        reboot_recommended: false,
        vhci_residual: false,
        attached_devices: Vec::new(),
        detail: "USB/IP passthrough is only supported on Windows".to_string(),
    });

    #[cfg(target_os = "windows")]
    {
        // Fail closed: if the residual state cannot be determined, do not
        // report a clean transport. Propagating the error surfaces the probe
        // as unavailable, hiding install and attach actions.
        let interfaces = super::device::enumerate_vhci_interfaces()?;
        let installation = match super::exec::find_installation() {
            Ok(installation) => installation,
            Err(detail) => {
                return Ok(UsbipStatus {
                    supported: true,
                    installed: false,
                    ready: false,
                    version: String::new(),
                    version_valid: false,
                    reboot_recommended: false,
                    // With no usable installation every present VHCI interface
                    // is a ghost that would corrupt the next install.
                    vhci_residual: !interfaces.is_empty(),
                    attached_devices: Vec::new(),
                    detail,
                });
            }
        };
        let version_valid = installation.version == PINNED_VERSION;
        let vhci_residual = interfaces.len() > 1;
        if !version_valid {
            return Ok(UsbipStatus {
                supported: true,
                installed: true,
                ready: false,
                version: installation.version,
                version_valid,
                reboot_recommended: false,
                vhci_residual,
                attached_devices: Vec::new(),
                detail: format!("USB/IP {PINNED_VERSION} is required"),
            });
        }
        match super::exec::list_attached().await {
            Ok(attached_devices) => Ok(UsbipStatus {
                supported: true,
                installed: true,
                // Residual instances break every usbip.exe operation; reflect
                // that even if a race let this probe succeed.
                ready: !vhci_residual,
                version: installation.version,
                version_valid,
                reboot_recommended: false,
                vhci_residual,
                attached_devices,
                detail: String::new(),
            }),
            Err(detail) => Ok(UsbipStatus {
                supported: true,
                installed: true,
                ready: false,
                version: installation.version,
                version_valid,
                reboot_recommended: false,
                vhci_residual,
                attached_devices: Vec::new(),
                detail,
            }),
        }
    }
}