//! Shared usbip-win2 installation policy and discovery.
//!
//! Device Hub passthrough and the DualSense sidecar use the same system-wide
//! VHCI driver. Keep the version, registry discovery, executable selection,
//! and replacement policy here so one feature cannot silently replace the
//! transport underneath the other.

pub(crate) const PINNED_VERSION: &str = "0.9.7.7";
pub(crate) const INSTALLER_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
pub(crate) const INSTALLER_SHA256: &str =
    "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallDisposition {
    Ready,
    Install,
}

/// Installing a different release invokes usbip-win2's system-wide upgrade or
/// downgrade path. That can tear down devices owned by either feature and has
/// left duplicate VHCI nodes in practice, so replacement is always explicit:
/// clean the shared transport first, then install the pinned release.
pub(crate) fn install_disposition(
    installed_version: Option<&str>,
) -> Result<InstallDisposition, String> {
    match installed_version {
        Some(PINNED_VERSION) => Ok(InstallDisposition::Ready),
        None => Ok(InstallDisposition::Install),
        Some(version) => Err(format!(
            "USBIP-SETUP-008: USB/IP {version} is installed; clean the shared transport before installing {PINNED_VERSION}"
        )),
    }
}

pub(crate) fn pinned_version_installed(installed_version: Option<&str>) -> bool {
    installed_version == Some(PINNED_VERSION)
}

#[cfg(target_os = "windows")]
pub(crate) struct UninstallEntry {
    pub(crate) key: winreg::RegKey,
    pub(crate) display_name: String,
}

#[cfg(target_os = "windows")]
pub(crate) fn uninstall_entries() -> Vec<UninstallEntry> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut entries = Vec::new();
    for view in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        let Ok(root) = hklm.open_subkey_with_flags(uninstall, view) else {
            continue;
        };
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, view) else {
                continue;
            };
            let display_name = key
                .get_value::<String, _>("DisplayName")
                .unwrap_or_default();
            if display_name.starts_with("USBip version ") {
                entries.push(UninstallEntry { key, display_name });
            }
        }
    }
    entries
}

#[cfg(target_os = "windows")]
pub(crate) fn installed_version() -> Option<String> {
    for entry in uninstall_entries() {
        if let Ok(version) = entry.key.get_value::<String, _>("DisplayVersion") {
            return Some(version);
        }
        return entry
            .display_name
            .strip_prefix("USBip version ")
            .map(str::to_string);
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn installed_version() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(super) struct Installation {
    pub(super) version: String,
    pub(super) executable: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
pub(super) fn find_installation() -> Result<Installation, String> {
    let mut fallback = None;
    let mut last_error = None;
    for entry in uninstall_entries() {
        let version = entry
            .key
            .get_value::<String, _>("DisplayVersion")
            .unwrap_or_else(|_| {
                entry
                    .display_name
                    .trim_start_matches("USBip version ")
                    .to_string()
            });
        let candidate = (|| -> Result<Installation, String> {
            let install_location = entry
                .key
                .get_value::<String, _>("InstallLocation")
                .map_err(|_| "USBIP-SETUP-002: USB/IP install location is missing".to_string())?;
            let root = std::path::PathBuf::from(install_location)
                .canonicalize()
                .map_err(|error| {
                    format!("USBIP-SETUP-002: USB/IP install location is invalid: {error}")
                })?;
            let executable = root.join("usbip.exe").canonicalize().map_err(|error| {
                format!(
                    "USBIP-SETUP-002: usbip.exe is missing from the installed transport: {error}"
                )
            })?;
            if !executable.starts_with(&root)
                || !executable
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("usbip.exe"))
            {
                return Err(
                    "USBIP-SETUP-002: the installed USB/IP executable path is unsafe".to_string(),
                );
            }
            Ok(Installation {
                version,
                executable,
            })
        })();
        match candidate {
            Ok(installation) if installation.version == PINNED_VERSION => {
                return Ok(installation);
            }
            Ok(installation) => {
                fallback.get_or_insert(installation);
            }
            Err(error) => last_error = Some(error),
        }
    }
    fallback.map_or_else(
        || {
            Err(last_error.unwrap_or_else(|| {
                "USBIP-SETUP-001: USB/IP transport is not installed".to_string()
            }))
        },
        Ok,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_transport_never_silently_replaces_another_version() {
        assert_eq!(
            install_disposition(Some(PINNED_VERSION)).unwrap(),
            InstallDisposition::Ready
        );
        assert_eq!(
            install_disposition(None).unwrap(),
            InstallDisposition::Install
        );
        assert!(install_disposition(Some("0.9.7.8")).is_err());
    }
}
