//! Transport status probing.

use super::{MINIMUM_VERSION, UsbipStatus};

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
        let devnodes = super::device::enumerate_vhci_devnodes()?;
        let installation = match super::manager::find_installation() {
            Ok(installation) => installation,
            Err(detail) => {
                let registered_version = super::installed_usbip_version();
                let partially_installed = !matches!(registered_version, Ok(None));
                return Ok(UsbipStatus {
                    supported: true,
                    installed: partially_installed,
                    ready: false,
                    version: registered_version.ok().flatten().unwrap_or_default(),
                    version_valid: false,
                    reboot_recommended: false,
                    // A registration without a usable executable is also a
                    // partial uninstall and must go through shared cleanup.
                    vhci_residual: !interfaces.is_empty()
                        || !devnodes.is_empty()
                        || partially_installed,
                    attached_devices: Vec::new(),
                    detail,
                });
            }
        };
        let version_valid = super::supported_usbip_installed(Some(&installation.version));
        let vhci_residual = super::device::has_vhci_residual(&interfaces, &devnodes);
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
                detail: format!("USB/IP {MINIMUM_VERSION} or newer is required"),
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
