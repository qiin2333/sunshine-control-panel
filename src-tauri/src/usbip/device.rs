//! Residual VHCI device detection and cleanup.
//!
//! The pinned USB/IP transport leaves ghost VHCI devnodes behind when the
//! vendor uninstaller is interrupted or the upgrade path skips cleanup. This
//! module enumerates those devices the same way `usbip.exe` does and removes
//! the leftovers the installer tolerates but usbip.exe refuses to.

#[cfg(target_os = "windows")]
const VHCI_INTERFACE_GUID: windows::core::GUID =
    windows::core::GUID::from_u128(0xB4030C06_DC5F_4FCC_87EB_E5515A0935C0);

#[cfg(target_os = "windows")]
const CLEANUP_POLL_ATTEMPTS: usize = 30;
#[cfg(target_os = "windows")]
const INNO_UNINSTALL_FLAGS: &str = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART";

/// usbip.exe fails with "Multiple instances of VHCI device interface found"
/// when more than one present device interface answers this GUID. Enumerating
/// the same list here detects the leftover-devnode state without running it.
#[cfg(target_os = "windows")]
pub(super) fn enumerate_vhci_interfaces() -> Result<Vec<String>, String> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_ListW,
        CM_Get_Device_Interface_List_SizeW, CR_BUFFER_SMALL, CR_SUCCESS,
    };

    // PRESENT matches what usbip.exe itself enumerates, so this reports exactly
    // the state that breaks it. Non-present interfaces are deliberately not
    // counted: they do not fail usbip.exe, and CM_Locate_DevNodeW cannot remove
    // them, so counting them would both disable a working transport and make
    // the sweep's final emptiness check unsatisfiable.
    // The size query and the list query are not atomic: interfaces arriving in
    // between make the list call report CR_BUFFER_SMALL. Device removal during
    // cleanup is exactly when this races, so retry instead of failing.
    const MAX_ENUM_RETRIES: usize = 5;
    for _ in 0..MAX_ENUM_RETRIES {
        let mut length = 0u32;
        let result = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut length,
                &VHCI_INTERFACE_GUID,
                None,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if result != CR_SUCCESS {
            return Err(format!(
                "USBIP-CLEAN-002: unable to measure the VHCI device interface list (config error {})",
                result.0
            ));
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let mut buffer = vec![0u16; length as usize];
        let result = unsafe {
            CM_Get_Device_Interface_ListW(
                &VHCI_INTERFACE_GUID,
                None,
                &mut buffer,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if result == CR_BUFFER_SMALL {
            continue;
        }
        if result != CR_SUCCESS {
            return Err(format!(
                "USBIP-CLEAN-002: unable to list the VHCI device interfaces (config error {})",
                result.0
            ));
        }
        return Ok(split_multi_sz(&buffer));
    }
    Err(
        "USBIP-CLEAN-002: the VHCI device interface list kept changing while enumerating"
            .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn split_multi_sz(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|&unit| unit == 0)
        .filter(|unit| !unit.is_empty())
        .map(|unit| String::from_utf16_lossy(unit))
        .collect()
}

/// Device interface symbolic links are laid out as
/// `\\?\ROOT#USB#0001#{interface-guid}`; the portion before the GUID brace is
/// the device instance ID with `#` separators.
fn instance_id_from_interface_path(path: &str) -> Option<String> {
    let body = path.strip_prefix(r"\\?\")?;
    let head = body.split_once('{')?.0;
    let head = head.strip_suffix('#')?;
    if head.is_empty() {
        return None;
    }
    Some(head.replace('#', "\\"))
}

/// Splits a registry command line such as `"C:\dir\unins000.exe" /flag` into
/// the executable and its arguments, applying the quoting rules of
/// CommandLineToArgvW: double quotes group whitespace, a quote toggles
/// quoting (an unclosed quote quotes to the end), and backslashes immediately
/// before a quote are half-escaped. Bare paths without arguments are valid
/// and yield an empty argument list.
fn split_executable_command(command: &str) -> Option<(String, Vec<String>)> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mut slashes = 1;
            while chars.peek() == Some(&'\\') {
                chars.next();
                slashes += 1;
            }
            if chars.peek() == Some(&'"') {
                // Runs before a quote are half-escaped: 2n backslashes keep n
                // and leave the quote as a toggle; 2n+1 keep n plus a literal
                // quote.
                for _ in 0..(slashes / 2) {
                    current.push('\\');
                }
                if slashes % 2 == 1 {
                    chars.next();
                    current.push('"');
                }
            } else {
                for _ in 0..slashes {
                    current.push('\\');
                }
            }
            continue;
        }
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    arguments.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    if arguments.is_empty() {
        return None;
    }
    let mut arguments = arguments;
    let executable = arguments.remove(0);
    Some((executable, arguments))
}

#[cfg(target_os = "windows")]
fn find_usbip_uninstall_string() -> Option<String> {
    crate::dualsense::usbip_uninstall_entries().into_iter().find_map(|entry| {
        entry
            .key
            .get_value::<String, _>("UninstallString")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// Runs inside the elevated helper. Order matters: the vendor uninstaller is
/// the only supported way to retire the driver packages, service registration,
/// and scheduled task; the devnode sweep afterwards only has to catch the
/// nodes the vendor uninstaller silently failed to remove. An uninstaller
/// failure never skips the sweep — leftover devnodes are the hard blocker for
/// the next install — but it is still reported so the user can retry.
#[cfg(target_os = "windows")]
pub(super) async fn cleanup_broken_transport() -> Result<(), String> {
    crate::dualsense::ensure_no_active_session().await.map_err(|_| {
        "USBIP-CLEAN-001: finish the active Sunshine stream before cleaning up the transport"
            .to_string()
    })?;
    let mut uninstall_error = None;
    if crate::dualsense::installed_usbip_version().is_some() {
        if let Err(error) = run_inno_uninstaller().await {
            uninstall_error = Some(error);
        }
    }
    remove_vhci_devnodes().await?;
    uninstall_error.map_or(Ok(()), Err)
}

#[cfg(target_os = "windows")]
async fn run_inno_uninstaller() -> Result<(), String> {
    use std::process::Stdio;

    let uninstall_string = find_usbip_uninstall_string().ok_or_else(|| {
        "USBIP-CLEAN-001: the USB/IP uninstaller registration is missing".to_string()
    })?;
    let (executable, arguments) = split_executable_command(&uninstall_string)
        .ok_or_else(|| "USBIP-CLEAN-001: the USB/IP uninstaller registration is invalid".to_string())?;
    let mut command = tokio::process::Command::new(&executable);
    command
        .args(arguments)
        .args(INNO_UNINSTALL_FLAGS.split_whitespace())
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    let child = command.spawn().map_err(|error| {
        format!("USBIP-CLEAN-001: unable to start the USB/IP uninstaller: {error}")
    })?;
    let finished = tokio::time::timeout(
        std::time::Duration::from_secs(10 * 60),
        async {
            let mut child = child;
            child.wait().await
        },
    )
    .await;
    match finished {
        // Inno uninstallers exit 0 on success; any non-zero code means the
        // user cancelled or a fatal error occurred.
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => {
            return Err(format!(
                "USBIP-CLEAN-001: the USB/IP uninstaller exited with {status}"
            ));
        }
        Ok(Err(error)) => {
            return Err(format!("USBIP-CLEAN-001: the USB/IP uninstaller failed: {error}"));
        }
        Err(_) => {
            return Err("USBIP-CLEAN-003: the USB/IP uninstaller timed out".to_string());
        }
    }
    wait_for_uninstall_registration_gone().await
}

#[cfg(target_os = "windows")]
async fn wait_for_uninstall_registration_gone() -> Result<(), String> {
    for _ in 0..CLEANUP_POLL_ATTEMPTS {
        if crate::dualsense::installed_usbip_version().is_none() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err("USBIP-CLEAN-001: the USB/IP uninstaller did not remove its registration".to_string())
}

#[cfg(target_os = "windows")]
async fn remove_vhci_devnodes() -> Result<(), String> {
    // PnP teardown is asynchronous; reuse the full cleanup polling budget so a
    // slow but successful device removal is not reported as a failure.
    let mut last_error = None;
    for _ in 0..CLEANUP_POLL_ATTEMPTS {
        let interfaces = enumerate_vhci_interfaces()?;
        if interfaces.is_empty() {
            return Ok(());
        }
        for path in interfaces {
            if let Some(instance_id) = instance_id_from_interface_path(&path) {
                // Removal can be transiently vetoed while PnP is busy; keep
                // sweeping and let the closing emptiness check decide the
                // outcome instead of abandoning the remaining nodes and retries.
                if let Err(error) = uninstall_vhci_devnode(&instance_id) {
                    last_error = Some(error);
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if enumerate_vhci_interfaces()?.is_empty() {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| {
            "USBIP-CLEAN-002: residual USB/IP host controller devices could not be removed"
                .to_string()
        }))
    }
}

#[cfg(target_os = "windows")]
fn uninstall_vhci_devnode(instance_id: &str) -> Result<(), String> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CM_Uninstall_DevNode, CR_SUCCESS,
    };
    use windows::core::PCWSTR;

    let encoded: Vec<u16> = instance_id.encode_utf16().chain(Some(0)).collect();
    let mut devinst = 0u32;
    let located =
        unsafe { CM_Locate_DevNodeW(&mut devinst, PCWSTR(encoded.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) };
    if located != CR_SUCCESS {
        // The devnode can disappear between enumeration and this lookup while
        // asynchronous PnP teardown completes. Treat it as already removed so
        // the sweep continues; the sweep's final emptiness check still fails
        // the cleanup if real leftovers remain.
        return Ok(());
    }
    let uninstalled = unsafe { CM_Uninstall_DevNode(devinst, 0) };
    if uninstalled != CR_SUCCESS {
        return Err(format!(
            "USBIP-CLEAN-002: residual device {instance_id} could not be removed (config error {})",
            uninstalled.0
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_instance_ids_from_interface_paths() {
        assert_eq!(
            instance_id_from_interface_path(
                r"\\?\ROOT#USB#0001#{b4030c06-dc5f-4fcc-87eb-e5515a0935c0}"
            ),
            Some("ROOT\\USB\\0001".to_string())
        );
        assert_eq!(instance_id_from_interface_path(r"\\?\ROOT#USB#0001"), None);
        assert_eq!(instance_id_from_interface_path(r"\\?\#{guid}"), None);
        assert_eq!(instance_id_from_interface_path("C:\\plain\\path"), None);
    }

    #[test]
    fn splits_quoted_and_bare_uninstaller_commands() {
        assert_eq!(
            split_executable_command(r#""C:\Program Files\USBip\unins000.exe""#),
            Some(("C:\\Program Files\\USBip\\unins000.exe".to_string(), Vec::new()))
        );
        assert_eq!(
            split_executable_command(r#""C:\Program Files\USBip\unins000.exe" /log=x"#),
            Some((
                "C:\\Program Files\\USBip\\unins000.exe".to_string(),
                vec!["/log=x".to_string()]
            ))
        );
        assert_eq!(
            split_executable_command(r"C:\USBip\unins000.exe /silent"),
            Some((
                "C:\\USBip\\unins000.exe".to_string(),
                vec!["/silent".to_string()]
            ))
        );
        assert_eq!(
            split_executable_command(r"C:\USBip\unins000.exe"),
            Some(("C:\\USBip\\unins000.exe".to_string(), Vec::new()))
        );
        assert_eq!(split_executable_command("  "), None);
        // An unclosed quote quotes to the end, as CommandLineToArgvW does.
        assert_eq!(
            split_executable_command(r#""C:\USBip\unins000.exe"#),
            Some(("C:\\USBip\\unins000.exe".to_string(), Vec::new()))
        );
        // Spaces inside a quoted argument are preserved.
        assert_eq!(
            split_executable_command(r#""C:\USBip\unins000.exe" /log="C:\logs dir\x"#),
            Some((
                "C:\\USBip\\unins000.exe".to_string(),
                vec!["/log=C:\\logs dir\\x".to_string()]
            ))
        );
        // An escaped quote stays inside the argument.
        assert_eq!(
            split_executable_command(r#""C:\USBip\unins000.exe" /D="a\"b""#),
            Some((
                "C:\\USBip\\unins000.exe".to_string(),
                vec!["/D=a\"b".to_string()]
            ))
        );
        // Backslashes before a non-quote are literal.
        assert_eq!(
            split_executable_command(r#""C:\a\\b\unins000.exe""#),
            Some(("C:\\a\\\\b\\unins000.exe".to_string(), Vec::new()))
        );
    }
}