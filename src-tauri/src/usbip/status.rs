//! Transport status probing.

use super::{REQUIRED_VERSION, UsbipStatus};

/// Extracts the structured "USBIP-XXX-NNN" code from a detail string.
/// Returns an empty string when the prefix is not a well-shaped code, so
/// consumers can branch on `error_code` instead of parsing detail text.
pub(super) fn detail_error_code(detail: &str) -> String {
    let code = detail.split(':').next().unwrap_or_default().trim();
    let mut segments = code.split('-');
    let shaped = matches!(
        (segments.next(), segments.next(), segments.next(), segments.next()),
        (Some(area), Some(kind), Some(number), None)
            if area == "USBIP"
                && (3..=8).contains(&kind.len())
                && kind.bytes().all(|b| b.is_ascii_uppercase())
                && number.len() == 3
                && number.bytes().all(|b| b.is_ascii_digit())
    );
    if shaped {
        code.to_string()
    } else {
        String::new()
    }
}

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
        error_code: String::new(),
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
                    error_code: detail_error_code(&detail),
                    attached_devices: Vec::new(),
                    detail,
                });
            }
        };
        let version_valid = super::supported_usbip_installed(Some(&installation.version));
        let vhci_residual = super::device::has_vhci_residual(&interfaces, &devnodes);
        if !version_valid {
            let detail = format!(
                "USBIP-SETUP-003: the tested USB/IP {REQUIRED_VERSION} tools and driver are required"
            );
            return Ok(UsbipStatus {
                supported: true,
                installed: true,
                ready: false,
                version: installation.version,
                version_valid,
                reboot_recommended: false,
                vhci_residual,
                error_code: detail_error_code(&detail),
                attached_devices: Vec::new(),
                detail,
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
                error_code: String::new(),
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
                error_code: detail_error_code(&detail),
                attached_devices: Vec::new(),
                detail,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::detail_error_code;

    #[test]
    fn extracts_only_well_shaped_codes() {
        assert_eq!(detail_error_code("USBIP-EXEC-006: the shared transport"), "USBIP-EXEC-006");
        assert_eq!(detail_error_code("USBIP-SETUP-001: not installed"), "USBIP-SETUP-001");
        assert_eq!(detail_error_code("USBIP-EXEC-006"), "USBIP-EXEC-006");
        // 非码前缀的 detail 必须得到空串，前端不得据此推断语义
        assert_eq!(detail_error_code("USB/IP passthrough is only supported"), "");
        assert_eq!(detail_error_code("usbip-exec-006: lowercase"), "");
        assert_eq!(detail_error_code("USBIP-EXEC-6: short number"), "");
        assert_eq!(detail_error_code(""), "");
    }
}
