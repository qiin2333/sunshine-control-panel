//! Locates the pinned transport and runs `usbip.exe`, parsing its output.

use super::{
    COMMAND_TIMEOUT, DEFAULT_TCP_PORT, MAX_OUTPUT_BYTES, REQUIRED_VERSION, UsbipAttachedDevice,
    UsbipRemoteDevice, validate_bus_id,
};
/// Verifies the shared tools and driver, then executes a bounded CLI command.
#[cfg(target_os = "windows")]
pub(super) async fn run_usbip(arguments: Vec<String>) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    let installation = super::manager::find_installation()?;
    if !super::supported_usbip_installed(Some(&installation.version)) {
        return Err(format!(
            "USBIP-SETUP-003: USB/IP {} is installed; the tested version {} is required",
            installation.version, REQUIRED_VERSION
        ));
    }
    super::manager::verify_driver_compatibility()?;
    let mut command = tokio::process::Command::new(installation.executable);
    command
        .args(arguments)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .as_std_mut()
        .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    let mut child = command
        .spawn()
        .map_err(|error| format!("USBIP-EXEC-001: unable to start usbip.exe: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "USBIP-EXEC-001: unable to capture USB/IP output".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "USBIP-EXEC-001: unable to capture USB/IP errors".to_string())?;
    let operation = async {
        let (stdout, stderr, status) = tokio::try_join!(
            read_bounded_output(stdout, MAX_OUTPUT_BYTES / 2),
            read_bounded_output(stderr, MAX_OUTPUT_BYTES / 2),
            async {
                child.wait().await.map_err(|error| {
                    format!("USBIP-EXEC-001: unable to wait for usbip.exe: {error}")
                })
            }
        )?;
        Ok::<_, String>(std::process::Output {
            status,
            stdout,
            stderr,
        })
    };
    let result = tokio::time::timeout(COMMAND_TIMEOUT, operation).await;
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(error)
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err("USBIP-EXEC-002: USB/IP operation timed out".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) async fn run_usbip(_arguments: Vec<String>) -> Result<std::process::Output, String> {
    Err("USBIP-SETUP-004: USB/IP passthrough is only supported on Windows".to_string())
}

async fn read_bounded_output<R>(mut reader: R, limit: usize) -> Result<Vec<u8>, String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut output = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|error| format!("USBIP-EXEC-001: unable to read USB/IP output: {error}"))?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > limit {
            return Err("USBIP-EXEC-003: USB/IP output exceeded the safety limit".to_string());
        }
        output.extend_from_slice(&chunk[..read]);
    }
}

fn command_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[derive(Clone, Copy)]
pub(super) enum CommandScope {
    Remote,
    Local,
}

/// Preserves both diagnostic streams and distinguishes ABI, local and exporter failures.
pub(super) fn require_success(
    output: std::process::Output,
    action: &str,
    scope: CommandScope,
) -> Result<String, String> {
    if output.status.success() {
        return Ok(command_text(&output.stdout));
    }
    // Retain both streams: the driver diagnosis may appear on stdout while
    // stderr contains only a generic command failure (or the reverse).
    let detail = [command_text(&output.stderr), command_text(&output.stdout)]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let code = if detail.to_ascii_lowercase().contains("abi mismatch") {
        "USBIP-EXEC-006"
    } else {
        match scope {
            CommandScope::Remote => "USBIP-EXEC-004",
            CommandScope::Local => "USBIP-EXEC-007",
        }
    };
    Err(format!(
        "{code}: {action} failed{}",
        if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        }
    ))
}

pub(super) fn parse_remote_devices(output: &str) -> Vec<UsbipRemoteDevice> {
    let mut devices = Vec::<UsbipRemoteDevice>::new();
    for line in output
        .lines()
        .skip_while(|line| !line.contains("===="))
        .skip(1)
    {
        let Some((prefix, value)) = line.split_once(':') else {
            continue;
        };
        let prefix = prefix.trim();
        let value = value.trim();
        if !prefix.is_empty() {
            if let Ok(bus_id) = validate_bus_id(prefix) {
                devices.push(UsbipRemoteDevice {
                    bus_id,
                    description: value.to_string(),
                    details: Vec::new(),
                });
            }
        } else if !value.is_empty()
            && let Some(device) = devices.last_mut()
        {
            device.details.push(value.to_string());
        }
    }
    devices
}

fn parse_attached_devices(output: &str) -> Vec<UsbipAttachedDevice> {
    let port_pattern = regex::Regex::new(r"^Port\s+(\d+):\s+device in use at\s+(.+)$").unwrap();
    let location_pattern = regex::Regex::new(r"^->\s+usbip://(.+):(\d+)/(.+)$").unwrap();
    let mut devices = Vec::new();
    let mut current: Option<UsbipAttachedDevice> = None;
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if let Some(captures) = port_pattern.captures(line) {
            if let Some(device) = current.take()
                && !device.remote_host.is_empty()
            {
                devices.push(device);
            }
            current = captures[1]
                .parse::<u8>()
                .ok()
                .map(|port| UsbipAttachedDevice {
                    port,
                    speed: captures[2].trim().to_string(),
                    description: String::new(),
                    remote_host: String::new(),
                    remote_port: DEFAULT_TCP_PORT,
                    remote_bus_id: String::new(),
                    serial: String::new(),
                    mode: String::new(),
                });
            continue;
        }
        let Some(device) = current.as_mut() else {
            continue;
        };
        if let Some(captures) = location_pattern.captures(line) {
            device.remote_host = captures[1].to_string();
            device.remote_port = captures[2].parse().unwrap_or(DEFAULT_TCP_PORT);
            device.remote_bus_id = captures[3].to_string();
        } else if let Some(value) = line.strip_prefix("-> serial:") {
            device.serial = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("-> mode:") {
            device.mode = value.trim().to_string();
        } else if !line.is_empty() && !line.starts_with("->") && device.description.is_empty() {
            device.description = line.to_string();
        }
    }
    if let Some(device) = current
        && !device.remote_host.is_empty()
    {
        devices.push(device);
    }
    devices
}

/// Enumerates local imports; local failures must not be presented as exporter errors.
pub(super) async fn list_attached() -> Result<Vec<UsbipAttachedDevice>, String> {
    let output = run_usbip(vec!["port".to_string()]).await?;
    let text = require_success(output, "listing attached devices", CommandScope::Local)?;
    Ok(parse_attached_devices(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_output(stdout: &str, stderr: &str) -> std::process::Output {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;

        std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn reports_driver_abi_mismatch_for_local_and_remote_commands() {
        for scope in [CommandScope::Local, CommandScope::Remote] {
            let error = require_success(
                failed_output(
                    "",
                    "error: ABI mismatch, unexpected size of the input structure",
                ),
                "listing attached devices",
                scope,
            )
            .unwrap_err();
            assert!(error.starts_with("USBIP-EXEC-006:"), "{error}");
            assert!(error.contains("unexpected size of the input structure"));
        }
    }

    #[test]
    fn preserves_driver_diagnosis_on_either_output_stream() {
        let error = require_success(
            failed_output("ABI mismatch", "command failed"),
            "listing attached devices",
            CommandScope::Local,
        )
        .unwrap_err();
        assert!(error.starts_with("USBIP-EXEC-006:"), "{error}");
        assert!(error.contains("command failed\nABI mismatch"));
    }

    #[test]
    fn local_enumeration_failure_is_not_reported_as_an_exporter_failure() {
        for (scope, code) in [
            (CommandScope::Local, "USBIP-EXEC-007:"),
            (CommandScope::Remote, "USBIP-EXEC-004:"),
        ] {
            let error =
                require_success(failed_output("", "access denied"), "probing", scope).unwrap_err();
            assert!(error.starts_with(code), "{error}");
            assert!(error.contains("access denied"));
        }
    }

    #[test]
    fn parses_exportable_devices() {
        let output = r#"Exportable USB devices
======================
    3-3.2  : Wireless Controller
           : /sys/devices/pci0000:00/usb3/3-3/3-3.2
           : Miscellaneous Device / ? / Interface Association
           :  0 - Audio / Control Device / unknown protocol

        4-1: USB Flash Disk
           : /sys/devices/pci0000:00/usb4/4-1
"#;
        let devices = parse_remote_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].bus_id, "3-3.2");
        assert_eq!(devices[0].description, "Wireless Controller");
        assert_eq!(devices[0].details.len(), 3);
        assert_eq!(devices[1].bus_id, "4-1");
    }

    #[test]
    fn parses_imported_devices_including_ipv6_hosts() {
        let output = r#"Imported USB devices
====================
Port 01: device in use at High Speed
         Wireless Controller
           -> usbip://usb-host.local:3240/3-3.2
           -> remote bus/dev: 003/004
           -> serial: ABC123
           -> mode: tcp
Port 12: device in use at SuperSpeed
         USB Flash Disk
           -> usbip://fe80::1:4324/4-1
           -> remote bus/dev: 004/002
           -> serial:
           -> mode: tcp
"#;
        let devices = parse_attached_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].port, 1);
        assert_eq!(devices[0].remote_host, "usb-host.local");
        assert_eq!(devices[0].remote_bus_id, "3-3.2");
        assert_eq!(devices[1].remote_host, "fe80::1");
        assert_eq!(devices[1].remote_port, 4324);
    }

    #[tokio::test]
    async fn rejects_process_output_while_it_crosses_the_limit() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(32);
        writer.write_all(b"12345").await.unwrap();
        writer.shutdown().await.unwrap();
        let error = read_bounded_output(reader, 4).await.unwrap_err();
        assert!(error.starts_with("USBIP-EXEC-003:"));
    }
}
