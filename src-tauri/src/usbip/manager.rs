//! Shared usbip-win2 installation policy and discovery.
//!
//! Device Hub passthrough and the DualSense sidecar use the same system-wide
//! VHCI driver. Keep the version, registry discovery, executable selection,
//! and replacement policy here so one feature cannot silently replace the
//! transport underneath the other.

// This tested combination is shared with the Sidecar build. The runtime may
// use this driver, but must never install or replace it while creating devices.
pub(crate) const PINNED_VERSION: &str = env!("SUNSHINE_USBIP_VERSION");
pub(super) const REQUIRED_VERSION: &str = PINNED_VERSION;
pub(crate) const INSTALLER_URL: &str = env!("SUNSHINE_USBIP_URL");
pub(crate) const INSTALLER_SHA256: &str = env!("SUNSHINE_USBIP_SHA256");
pub(crate) const TRANSPORT_POLICY: &str = env!("SUNSHINE_USBIP_POLICY");

/// Rejects missing or replaced transport binaries using the contract's SHA-256.
#[cfg(target_os = "windows")]
fn verify_transport_file(path: &std::path::Path, expected: &str) -> Result<(), String> {
    let actual = crate::native_components::install::sha256_file(path, u64::MAX)
        .map_err(|error| format!("USBIP-EXEC-006: cannot verify the shared transport: {error}"))?;
    if actual != expected {
        return Err(
            "USBIP-EXEC-006: the shared transport does not match the tested component combination"
                .to_string(),
        );
    }
    Ok(())
}

/// Checks the registered driver path and bytes without changing driver state.
#[cfg(target_os = "windows")]
pub(super) fn verify_driver_compatibility() -> Result<(), String> {
    use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};
    let service = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\usbip2_ude")
        .map_err(|error| {
            format!("USBIP-EXEC-006: shared driver registration is unavailable: {error}")
        })?;
    let image: String = service
        .get_value("ImagePath")
        .map_err(|error| format!("USBIP-EXEC-006: shared driver path is unavailable: {error}"))?;
    let windows = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").ok_or("USBIP-EXEC-006: Windows directory is unavailable")?,
    );
    let path = if image.to_ascii_lowercase().starts_with(r"\systemroot\") {
        windows.join(&image[12..])
    } else if let Some(path) = image.strip_prefix(r"\??\") {
        std::path::PathBuf::from(path)
    } else if image.to_ascii_lowercase().starts_with(r"system32\") {
        windows.join(image)
    } else {
        std::path::PathBuf::from(image)
    };
    let path = path
        .canonicalize()
        .map_err(|error| format!("USBIP-EXEC-006: shared driver is unavailable: {error}"))?;
    let windows = windows
        .canonicalize()
        .map_err(|error| format!("USBIP-EXEC-006: cannot resolve Windows directory: {error}"))?;
    if !path.starts_with(windows)
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.eq_ignore_ascii_case("usbip2_ude.sys"))
    {
        return Err(
            "USBIP-EXEC-006: shared driver path is outside the Windows driver store".to_string(),
        );
    }
    verify_transport_file(&path, env!("SUNSHINE_USBIP_DRIVER_SHA256"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallDisposition {
    Ready,
    Install,
}

/// Only the explicitly tested combination is reused; newer is not an ABI guarantee.
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

/// Accepts only the tested four-part version, including neither older nor newer ABIs.
pub(crate) fn supported_version_installed(installed_version: Option<&str>) -> bool {
    installed_version
        .and_then(parse_version)
        .is_some_and(|version| {
            version == parse_version(PINNED_VERSION).expect("valid pinned USB/IP version")
        })
}

/// Requires four unsigned numeric components; suffixes, signs and overflow are invalid.
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
/// Recognizes both usbip-win2 installer names without matching other USB/IP products.
fn version_from_display_name(display_name: &str) -> Option<&str> {
    display_name.strip_prefix("USBip version ").or_else(|| {
        display_name
            .strip_prefix("USBip ")
            .filter(|version| parse_version(version).is_some())
    })
}

/// Rejects malformed or conflicting registrations instead of selecting whichever entry appears first.
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

/// Resolves the registered CLI and verifies its pinned bytes before execution.
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
            if supported_version_installed(Some(&version)) {
                verify_transport_file(&executable, env!("SUNSHINE_USBIP_TOOL_SHA256"))?;
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

    #[cfg(target_os = "windows")]
    #[test]
    fn shared_binary_verification_rejects_replaced_or_missing_files() {
        let file = std::env::temp_dir().join(format!("usbip-verify-{}", uuid::Uuid::new_v4()));
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        std::fs::write(&file, b"hello").unwrap();
        assert!(verify_transport_file(&file, expected).is_ok());
        std::fs::write(&file, b"replaced").unwrap();
        assert!(
            verify_transport_file(&file, expected)
                .unwrap_err()
                .starts_with("USBIP-EXEC-006:")
        );
        std::fs::remove_file(&file).unwrap();
        assert!(verify_transport_file(&file, expected).is_err());
    }

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
        }
        // Keep malformed legacy registrations visible to version validation,
        // without treating other USB/IP products as usbip-win2.
        assert_eq!(version_from_display_name("USBip version bad"), Some("bad"));
        for name in [
            "USBip Tools",
            "USBip Server",
            "Other USBip 0.9.8.1",
            "USBip",
        ] {
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
                ["USBip version 0.9.8.1", "USBip 0.9.8.1"].map(|name| version_from_display_name(
                    name
                )
                .unwrap()
                .to_string())
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
        assert!(install_disposition(Some("0.9.8.2")).is_err());
        assert!(install_disposition(Some("0.9.7.6")).is_err());
        assert!(install_disposition(Some("0.9.7.7")).is_err());
    }

    #[test]
    fn only_the_tested_transport_version_is_accepted() {
        assert!(supported_version_installed(Some(PINNED_VERSION)));
        for version in [
            "0.9.8.2",
            "0.9.8.10",
            "0.10.0.0",
            "1.0.0.0",
            "0.9.7.6",
            "0.9.7.7",
            "0.9.7.10",
            "0.9.8.0",
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
