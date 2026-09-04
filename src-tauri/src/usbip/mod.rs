//! Narrow Windows USB/IP client management for the Device Hub.
//!
//! The renderer can discover devices from a standard USB/IP exporter, then
//! explicitly attach or detach them. Privileged operations are allowlisted and
//! revalidated in a short-lived elevated copy of this executable.

mod commands;
mod device;
mod elevated;
mod exec;
mod status;

use serde::{Deserialize, Serialize};

const PINNED_VERSION: &str = "0.9.7.7";
const DEFAULT_TCP_PORT: u16 = 3240;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_ELEVATED_RESPONSE_BYTES: usize = 16 * 1024;
const ELEVATED_ARG: &str = "--elevated-usbip";
const ELEVATED_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

static OPERATION_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UsbipRemoteDevice {
    pub bus_id: String,
    pub description: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UsbipAttachedDevice {
    pub port: u8,
    pub speed: String,
    pub description: String,
    pub remote_host: String,
    pub remote_port: u16,
    pub remote_bus_id: String,
    pub serial: String,
    pub mode: String,
}

#[derive(Debug, Serialize)]
pub struct UsbipStatus {
    pub supported: bool,
    pub installed: bool,
    pub ready: bool,
    pub version: String,
    pub version_valid: bool,
    pub reboot_recommended: bool,
    pub vhci_residual: bool,
    pub attached_devices: Vec<UsbipAttachedDevice>,
    pub detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum ElevatedRequest {
    Install,
    Cleanup,
    Attach {
        remote: String,
        tcp_port: u16,
        bus_id: String,
    },
    Detach {
        port: u8,
    },
}

impl ElevatedRequest {
    fn timeout(&self) -> std::time::Duration {
        match self {
            // The pinned download has a 30-minute overall budget and the signed
            // installer has a separate 10-minute budget. Keep the IPC alive
            // through both, with a small hand-off margin.
            Self::Install => std::time::Duration::from_secs(45 * 60),
            // The silent uninstaller plus the devnode sweep stay well below a
            // single Inno install, but leave headroom for slow driver teardown.
            Self::Cleanup => std::time::Duration::from_secs(15 * 60),
            Self::Attach { .. } | Self::Detach { .. } => {
                COMMAND_TIMEOUT + std::time::Duration::from_secs(5)
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ElevatedResponse {
    success: bool,
    port: Option<u8>,
    message: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct UsbipInstallation {
    version: String,
    executable: std::path::PathBuf,
}

fn validate_remote(remote: &str) -> Result<String, String> {
    let value = remote.trim();
    if value.is_empty() || value.len() > 253 {
        return Err("USBIP-INPUT-001: enter a valid exporter hostname or IP address".to_string());
    }
    let unwrapped = value
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(value);
    if unwrapped.parse::<std::net::IpAddr>().is_ok() {
        return Ok(unwrapped.to_string());
    }
    let valid_hostname = !unwrapped.contains(':')
        && unwrapped.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
                && label
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphanumeric())
                && label
                    .chars()
                    .last()
                    .is_some_and(|character| character.is_ascii_alphanumeric())
        });
    if !valid_hostname {
        return Err("USBIP-INPUT-001: exporter hostname or IP address is invalid".to_string());
    }
    Ok(unwrapped.to_string())
}

fn validate_bus_id(bus_id: &str) -> Result<String, String> {
    let value = bus_id.trim();
    if value.is_empty()
        || value.len() > 64
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | ':' | '_'))
        })
    {
        return Err("USBIP-INPUT-002: the remote USB bus ID is invalid".to_string());
    }
    Ok(value.to_string())
}

fn validate_tcp_port(tcp_port: Option<u16>) -> Result<u16, String> {
    let value = tcp_port.unwrap_or(DEFAULT_TCP_PORT);
    if value < 1024 {
        return Err("USBIP-INPUT-003: the TCP port must be between 1024 and 65535".to_string());
    }
    Ok(value)
}

// Re-export the tauri commands and the elevated-helper entry point so callers
// keep spelling `usbip::` with the same paths they used against the single-file
// module. Glob re-exports carry the `#[tauri::command]`-generated hidden items
// that generate_handler! resolves (same pattern as dualsense).
pub use commands::*;
pub use status::*;
#[cfg(target_os = "windows")]
pub(crate) use elevated::try_handle_elevated_command;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_only_host_and_bus_id_characters() {
        assert_eq!(validate_remote(" 192.168.1.20 ").unwrap(), "192.168.1.20");
        assert_eq!(validate_remote("[fe80::1]").unwrap(), "fe80::1");
        assert_eq!(validate_remote("usb-host.local").unwrap(), "usb-host.local");
        assert!(validate_remote("host name").is_err());
        assert!(validate_remote("host&calc").is_err());
        assert!(validate_remote("-host.local").is_err());
        assert_eq!(validate_bus_id("3-3.2").unwrap(), "3-3.2");
        assert!(validate_bus_id("3-3 /tmp").is_err());
    }

    #[test]
    fn validates_usbip_win2_tcp_port_range() {
        assert_eq!(validate_tcp_port(None).unwrap(), DEFAULT_TCP_PORT);
        assert!(validate_tcp_port(Some(1023)).is_err());
        assert_eq!(validate_tcp_port(Some(1024)).unwrap(), 1024);
        assert_eq!(validate_tcp_port(Some(u16::MAX)).unwrap(), u16::MAX);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "requires the pinned USB/IP transport on the local Windows host"]
    async fn installed_transport_reports_ready() {
        let status = usbip_get_status().await.unwrap();
        assert!(status.installed);
        assert!(status.version_valid);
        assert!(status.ready, "{}", status.detail);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "requires the pinned USB/IP transport on the local Windows host"]
    async fn unreachable_exporter_fails_cleanly() {
        let error = usbip_list_remote("127.0.0.1".to_string(), Some(43241))
            .await
            .unwrap_err();
        assert!(error.starts_with("USBIP-EXEC-004:"), "{error}");
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "requires the pinned USB/IP transport on the local Windows host"]
    async fn discovers_a_device_from_a_standard_usbip_exporter() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let tcp_port = listener.local_addr().unwrap().port();
        let exporter = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 8];
            socket.read_exact(&mut request).unwrap();
            assert_eq!(&request[..4], &[0x01, 0x11, 0x80, 0x05]);

            let mut reply = Vec::new();
            reply.extend_from_slice(&0x0111u16.to_be_bytes());
            reply.extend_from_slice(&0x0005u16.to_be_bytes());
            reply.extend_from_slice(&0u32.to_be_bytes());
            reply.extend_from_slice(&1u32.to_be_bytes());
            let mut path = [0u8; 256];
            path[..18].copy_from_slice(b"/sys/devices/mock1");
            reply.extend_from_slice(&path);
            let mut bus_id = [0u8; 32];
            bus_id[..3].copy_from_slice(b"1-1");
            reply.extend_from_slice(&bus_id);
            reply.extend_from_slice(&1u32.to_be_bytes());
            reply.extend_from_slice(&2u32.to_be_bytes());
            reply.extend_from_slice(&3u32.to_be_bytes());
            reply.extend_from_slice(&0x054cu16.to_be_bytes());
            reply.extend_from_slice(&0x0ce6u16.to_be_bytes());
            reply.extend_from_slice(&0x0100u16.to_be_bytes());
            reply.extend_from_slice(&[0, 0, 0, 1, 1, 1]);
            reply.extend_from_slice(&[3, 0, 0, 0]);
            socket.write_all(&reply).unwrap();
        });

        let devices = usbip_list_remote("127.0.0.1".to_string(), Some(tcp_port))
            .await
            .unwrap();
        exporter.join().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].bus_id, "1-1");
        assert!(!devices[0].description.is_empty());
    }
}
