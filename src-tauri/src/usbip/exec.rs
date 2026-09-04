//! Locates the pinned transport and runs `usbip.exe`, parsing its output.

use super::{
    COMMAND_TIMEOUT, DEFAULT_TCP_PORT, MAX_OUTPUT_BYTES, PINNED_VERSION, UsbipAttachedDevice,
    UsbipRemoteDevice, validate_bus_id,
};
#[cfg(target_os = "windows")]
use super::UsbipInstallation;

#[cfg(target_os = "windows")]
pub(super) fn find_installation() -> Result<UsbipInstallation, String> {
    let mut fallback = None;
    let mut last_error = None;
    for entry in crate::dualsense::usbip_uninstall_entries() {
        let version = entry
            .key
            .get_value::<String, _>("DisplayVersion")
            .unwrap_or_else(|_| {
                entry
                    .display_name
                    .trim_start_matches("USBip version ")
                    .to_string()
            });
        let candidate = (|| -> Result<UsbipInstallation, String> {
            let install_location =
                entry
                    .key
                    .get_value::<String, _>("InstallLocation")
                    .map_err(|_| {
                        "USBIP-SETUP-002: USB/IP install location is missing".to_string()
                    })?;
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
            Ok(UsbipInstallation {
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
            Err(error) => {
                last_error = Some(error);
            }
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

#[cfg(target_os = "windows")]
pub(super) async fn run_usbip(arguments: Vec<String>) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    let installation = find_installation()?;
    if installation.version != PINNED_VERSION {
        return Err(format!(
            "USBIP-SETUP-003: USB/IP {} is installed; version {} is required",
            installation.version, PINNED_VERSION
        ));
    }
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

pub(super) fn require_success(output: std::process::Output, action: &str) -> Result<String, String> {
    if output.status.success() {
        return Ok(command_text(&output.stdout));
    }
    let detail = command_text(&output.stderr);
    let detail = if detail.is_empty() {
        command_text(&output.stdout)
    } else {
        detail
    };
    Err(format!(
        "USBIP-EXEC-004: {action} failed{}",
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

pub(super) async fn list_attached() -> Result<Vec<UsbipAttachedDevice>, String> {
    let output = run_usbip(vec!["port".to_string()]).await?;
    let text = require_success(output, "listing attached devices")?;
    Ok(parse_attached_devices(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

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