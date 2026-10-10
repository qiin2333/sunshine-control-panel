//! Shared usbip-win2 installation policy and discovery.
//!
//! Device Hub passthrough and the DualSense sidecar use the same system-wide
//! VHCI driver. Keep the version, registry discovery, executable selection,
//! and replacement policy here so one feature cannot silently replace the
//! transport underneath the other.

pub(crate) const PINNED_VERSION: &str = "0.9.7.7";
pub(super) const MINIMUM_VERSION: &str = "0.9.7.7";
pub(crate) const INSTALLER_URL: &str =
    "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.7.7/USBip-0.9.7.7-x64.exe";
pub(crate) const INSTALLER_SHA256: &str =
    "51620fa5f9f8be5932bc9d786deee557ce06d5407a99cab490dcfac71f185fea";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallDisposition {
    Ready,
    Install,
}

/// Automatic downloads remain pinned, but a user's newer installation is reused.
/// Replacing an unsupported installation requires explicit shared cleanup.
pub(crate) fn install_disposition(
    installed_version: Option<&str>,
) -> Result<InstallDisposition, String> {
    match installed_version {
        Some(version) if supported_version_installed(Some(version)) => {
            Ok(InstallDisposition::Ready)
        }
        None => Ok(InstallDisposition::Install),
        Some(version) => Err(format!(
            "USBIP-SETUP-008: USB/IP {version} is installed; clean the shared transport before installing {PINNED_VERSION}"
        )),
    }
}

pub(crate) fn supported_version_installed(installed_version: Option<&str>) -> bool {
    installed_version
        .and_then(parse_version)
        .is_some_and(|version| {
            version >= parse_version(MINIMUM_VERSION).expect("valid minimum USB/IP version")
        })
}

fn parse_version(value: &str) -> Option<[u32; 4]> {
    let components: Vec<_> = value.split('.').collect();
    if components.len() != 4 {
        return None;
    }
    let mut version = [0; 4];
    for (index, component) in components.iter().enumerate() {
        if component.is_empty() || !component.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        version[index] = component.parse().ok()?;
    }
    Some(version)
}

// Inno Setup's English display name used to include "version". Newer
// usbip-win2 installers register the same product as "USBip 0.9.8.1".
// Keep discovery and the missing-DisplayVersion fallback consistent.
fn version_from_display_name(display_name: &str) -> Option<&str> {
    display_name
        .strip_prefix("USBip version ")
        .or_else(|| {
            display_name
                .strip_prefix("USBip ")
                .filter(|version| parse_version(version).is_some())
        })
}

fn consistent_version(
    versions: impl IntoIterator<Item = String>,
) -> Result<Option<String>, String> {
    let mut found: Option<String> = None;
    for value in versions {
        let parsed = parse_version(&value).ok_or_else(|| {
            "USBIP-SETUP-008: invalid USB/IP registration; clean the shared transport first"
                .to_string()
        })?;
        if found
            .as_deref()
            .and_then(parse_version)
            .is_some_and(|previous| previous != parsed)
        {
            return Err(
                "USBIP-SETUP-008: conflicting USB/IP versions; clean the shared transport first"
                    .to_string(),
            );
        }
        found.get_or_insert(value);
    }
    Ok(found)
}

#[cfg(target_os = "windows")]
pub(crate) struct UninstallEntry {
    pub(crate) key: winreg::RegKey,
    pub(crate) display_name: String,
}

#[cfg(target_os = "windows")]
pub(crate) fn uninstall_entries() -> Result<Vec<UninstallEntry>, String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut entries = Vec::new();
    for view in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        let root = match hklm.open_subkey_with_flags(uninstall, view) {
            Ok(root) => root,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "USBIP-SETUP-002: cannot inspect uninstall registry: {error}"
                ));
            }
        };
        for name in root.enum_keys() {
            let name = name.map_err(|error| {
                format!("USBIP-SETUP-002: cannot enumerate uninstall registry: {error}")
            })?;
            let key = match root.open_subkey_with_flags(&name, view) {
                Ok(key) => key,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!(
                        "USBIP-SETUP-002: cannot inspect uninstall entry: {error}"
                    ));
                }
            };
            let display_name = key
                .get_value::<String, _>("DisplayName")
                .unwrap_or_default();
            if version_from_display_name(&display_name).is_some() {
                entries.push(UninstallEntry { key, display_name });
            }
        }
    }
    Ok(entries)
}

#[cfg(target_os = "windows")]
pub(crate) fn installed_version() -> Result<Option<String>, String> {
    consistent_version(uninstall_entries()?.iter().map(entry_version))
}

#[cfg(target_os = "windows")]
fn entry_version(entry: &UninstallEntry) -> String {
    entry
        .key
        .get_value::<String, _>("DisplayVersion")
        .unwrap_or_else(|_| {
            version_from_display_name(&entry.display_name)
                .unwrap_or_default()
                .to_string()
        })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn installed_version() -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(super) struct Installation {
    pub(super) version: String,
    pub(super) executable: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
pub(super) fn find_installation() -> Result<Installation, String> {
    let entries = uninstall_entries()?;
    let version = consistent_version(entries.iter().map(entry_version))?
        .ok_or_else(|| "USBIP-SETUP-001: USB/IP transport is not installed".to_string())?;
    let mut installation = None;
    for entry in entries {
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
                version: version.clone(),
                executable,
            })
        })();
        // A valid registration must not hide another broken registration.
        let candidate = candidate?;
        installation.get_or_insert(candidate);
    }
    installation.ok_or_else(|| "USBIP-SETUP-001: USB/IP transport is not installed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_legacy_and_current_installer_display_names() {
        for (name, version) in [
            ("USBip version 0.9.7.7", "0.9.7.7"),
            ("USBip version 0.9.8.0", "0.9.8.0"),
            ("USBip 0.9.8.0", "0.9.8.0"),
            ("USBip version 0.9.8.1", "0.9.8.1"),
            ("USBip 0.9.8.1", "0.9.8.1"),
        ] {
            let discovered = version_from_display_name(name);
            assert_eq!(discovered, Some(version));
            assert_eq!(
                install_disposition(discovered).unwrap(),
                InstallDisposition::Ready
            );
        }
        // Preserve the existing fail-closed handling of malformed legacy
        // registrations, without treating other USB/IP products as usbip-win2.
        assert_eq!(version_from_display_name("USBip version bad"), Some("bad"));
        assert!(install_disposition(Some("bad")).is_err());
        for name in ["USBip Tools", "USBip Server", "Other USBip 0.9.8.1", "USBip"] {
            assert_eq!(version_from_display_name(name), None, "{name}");
        }
    }

    #[test]
    fn mixed_installer_display_names_still_detect_conflicting_versions() {
        for names in [
            ["USBip version 0.9.7.7", "USBip 0.9.8.1"],
            ["USBip 0.9.8.1", "USBip version 0.9.7.7"],
            ["USBip 0.9.8.0", "USBip 0.9.8.1"],
        ] {
            assert!(
                consistent_version(
                    names.map(|name| version_from_display_name(name).unwrap().to_string())
                )
                .is_err()
            );
        }
        assert_eq!(
            consistent_version(
                ["USBip version 0.9.8.1", "USBip 0.9.8.1"]
                    .map(|name| version_from_display_name(name).unwrap().to_string())
            )
            .unwrap(),
            Some("0.9.8.1".to_string())
        );
    }

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
        assert_eq!(
            install_disposition(Some("0.9.7.8")).unwrap(),
            InstallDisposition::Ready
        );
        assert!(install_disposition(Some("0.9.7.6")).is_err());
    }

    #[test]
    fn supported_versions_are_compared_numerically() {
        for version in [
            "0.9.7.7", "0.9.7.8", "0.9.7.10", "0.9.8.0", "0.9.8.1", "0.10.0.0", "1.0.0.0",
        ] {
            assert!(supported_version_installed(Some(version)), "{version}");
        }
        for version in [
            "0.9.7.6",
            "0.8.99.99",
            "",
            "0.9.7",
            "0.9.7.8-beta",
            "0.9.7.+8",
            "0.9.7.4294967296",
        ] {
            assert!(!supported_version_installed(Some(version)), "{version}");
        }
        assert!(!supported_version_installed(None));
    }

    #[test]
    fn all_registrations_must_agree_regardless_of_order() {
        for versions in [
            vec!["0.9.7.7", "0.9.7.8"],
            vec!["0.9.7.8", "0.9.7.7"],
            vec!["0.9.7.7", ""],
            vec!["bad", "0.9.7.8"],
        ] {
            assert!(consistent_version(versions.into_iter().map(str::to_string)).is_err());
        }
        assert_eq!(
            consistent_version(["0.9.7.8".into(), "0.9.7.8".into()]).unwrap(),
            Some("0.9.7.8".into())
        );
        assert_eq!(consistent_version(Vec::new()).unwrap(), None);
    }
}
