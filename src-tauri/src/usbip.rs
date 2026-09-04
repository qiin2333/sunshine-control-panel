//! Narrow Windows USB/IP client management for the Device Hub.
//!
//! The renderer can discover devices from a standard USB/IP exporter, then
//! explicitly attach or detach them. Privileged operations are allowlisted and
//! revalidated in a short-lived elevated copy of this executable.

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

#[cfg(target_os = "windows")]
fn find_installation() -> Result<UsbipInstallation, String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut fallback = None;
    let mut last_error = None;
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
            if !display_name.starts_with("USBip version ") {
                continue;
            }
            let version = key
                .get_value::<String, _>("DisplayVersion")
                .unwrap_or_else(|_| {
                    display_name
                        .trim_start_matches("USBip version ")
                        .to_string()
                });
            let candidate = (|| -> Result<UsbipInstallation, String> {
                let install_location =
                    key.get_value::<String, _>("InstallLocation").map_err(|_| {
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
                        "USBIP-SETUP-002: the installed USB/IP executable path is unsafe"
                            .to_string(),
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
const VHCI_INTERFACE_GUID: windows::core::GUID =
    windows::core::GUID::from_u128(0xB4030C06_DC5F_4FCC_87EB_E5515A0935C0);

/// usbip.exe fails with "Multiple instances of VHCI device interface found"
/// when more than one present device interface answers this GUID. Enumerating
/// the same list here detects the leftover-devnode state without running it.
#[cfg(target_os = "windows")]
fn enumerate_vhci_interfaces() -> Result<Vec<String>, String> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_ListW,
        CM_Get_Device_Interface_List_SizeW, CR_SUCCESS,
    };

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
    if result != CR_SUCCESS {
        return Err(format!(
            "USBIP-CLEAN-002: unable to list the VHCI device interfaces (config error {})",
            result.0
        ));
    }
    Ok(split_multi_sz(&buffer))
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
/// the executable and the remaining arguments.
fn split_executable_command(command: &str) -> Option<(String, String)> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if let Some(rest) = command.strip_prefix('"') {
        let (executable, arguments) = rest.split_once('"')?;
        Some((executable.to_string(), arguments.trim().to_string()))
    } else {
        let (executable, arguments) = command.split_once(' ')?;
        Some((executable.to_string(), arguments.trim().to_string()))
    }
}

#[cfg(target_os = "windows")]
fn find_usbip_uninstall_string() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    for view in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        let Ok(root) = hklm.open_subkey_with_flags(uninstall, view) else {
            continue;
        };
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, view) else {
                continue;
            };
            if !key
                .get_value::<String, _>("DisplayName")
                .unwrap_or_default()
                .starts_with("USBip version ")
            {
                continue;
            }
            if let Ok(uninstall_string) = key.get_value::<String, _>("UninstallString") {
                let uninstall_string = uninstall_string.trim().to_string();
                if !uninstall_string.is_empty() {
                    return Some(uninstall_string);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
async fn run_usbip(arguments: Vec<String>) -> Result<std::process::Output, String> {
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
async fn run_usbip(_arguments: Vec<String>) -> Result<std::process::Output, String> {
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

fn require_success(output: std::process::Output, action: &str) -> Result<String, String> {
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

fn parse_remote_devices(output: &str) -> Vec<UsbipRemoteDevice> {
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

async fn list_attached() -> Result<Vec<UsbipAttachedDevice>, String> {
    let output = run_usbip(vec!["port".to_string()]).await?;
    let text = require_success(output, "listing attached devices")?;
    Ok(parse_attached_devices(&text))
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
        attached_devices: Vec::new(),
        detail: "USB/IP passthrough is only supported on Windows".to_string(),
    });

    #[cfg(target_os = "windows")]
    {
        let vhci_residual = match enumerate_vhci_interfaces() {
            Ok(interfaces) => interfaces.len() > 1,
            Err(_) => false,
        };
        let installation = match find_installation() {
            Ok(installation) => installation,
            Err(detail) => {
                return Ok(UsbipStatus {
                    supported: true,
                    installed: false,
                    ready: false,
                    version: String::new(),
                    version_valid: false,
                    reboot_recommended: false,
                    vhci_residual,
                    attached_devices: Vec::new(),
                    detail,
                });
            }
        };
        let version_valid = installation.version == PINNED_VERSION;
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
        match list_attached().await {
            Ok(attached_devices) => Ok(UsbipStatus {
                supported: true,
                installed: true,
                ready: true,
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

#[tauri::command]
pub async fn usbip_list_remote(
    remote: String,
    tcp_port: Option<u16>,
) -> Result<Vec<UsbipRemoteDevice>, String> {
    let remote = validate_remote(&remote)?;
    let tcp_port = validate_tcp_port(tcp_port)?;
    let output = run_usbip(vec![
        "--tcp-port".to_string(),
        tcp_port.to_string(),
        "list".to_string(),
        "--remote".to_string(),
        remote,
    ])
    .await?;
    let text = require_success(output, "discovering remote devices")?;
    Ok(parse_remote_devices(&text))
}

#[tauri::command]
pub async fn usbip_install_transport() -> Result<UsbipStatus, String> {
    let _guard = OPERATION_LOCK.lock().await;
    let response = run_elevated(ElevatedRequest::Install).await?;
    let reboot_recommended = response.message == "reboot_recommended";
    let mut status = usbip_get_status().await?;
    status.reboot_recommended = reboot_recommended;
    Ok(status)
}

/// Removes a broken USB/IP installation plus every leftover VHCI device node
/// so the pinned installer can run from a clean state. The elevated helper
/// uninstalls the registered transport (when present), then sweeps devnodes
/// that the vendor uninstaller failed to remove.
#[tauri::command]
pub async fn usbip_cleanup_transport() -> Result<UsbipStatus, String> {
    let _guard = OPERATION_LOCK.lock().await;
    run_elevated(ElevatedRequest::Cleanup).await?;
    usbip_get_status().await
}

#[tauri::command]
pub async fn usbip_attach(
    remote: String,
    tcp_port: Option<u16>,
    bus_id: String,
) -> Result<u8, String> {
    let request = ElevatedRequest::Attach {
        remote: validate_remote(&remote)?,
        tcp_port: validate_tcp_port(tcp_port)?,
        bus_id: validate_bus_id(&bus_id)?,
    };
    let _guard = OPERATION_LOCK.lock().await;
    let response = run_elevated(request).await?;
    response
        .port
        .ok_or_else(|| "USBIP-EXEC-005: attach succeeded without returning a port".to_string())
}

#[tauri::command]
pub async fn usbip_detach(port: u8) -> Result<(), String> {
    if port == 0 {
        return Err("USBIP-INPUT-004: the local USB/IP port is invalid".to_string());
    }
    let _guard = OPERATION_LOCK.lock().await;
    run_elevated(ElevatedRequest::Detach { port }).await?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn pipe_name(token: uuid::Uuid) -> String {
    format!(r"\\.\pipe\sunshine-usbip-{token}")
}

#[cfg(target_os = "windows")]
async fn run_elevated(request: ElevatedRequest) -> Result<ElevatedResponse, String> {
    use std::os::windows::io::AsRawHandle;
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;

    let timeout = request.timeout();
    let token = uuid::Uuid::new_v4();
    let pipe = pipe_name(token);
    let server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(true)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe)
        .map_err(|error| format!("USBIP-UAC-002: unable to create administrator IPC: {error}"))?;
    let payload = serde_json::to_string(&request).map_err(|error| {
        format!("USBIP-UAC-002: unable to encode administrator request: {error}")
    })?;
    let token_arg = token.to_string();
    let process = tokio::task::spawn_blocking(move || {
        crate::utils::launch_current_executable_elevated(
            &[ELEVATED_ARG, &token_arg, &payload],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("USBIP-UAC-001: administrator launch task failed: {error}"))?
    .map_err(|error| format!("USBIP-UAC-001: administrator authorization was canceled: {error}"))?;

    tokio::time::timeout(ELEVATED_CONNECT_TIMEOUT, server.connect())
        .await
        .map_err(|_| "USBIP-UAC-002: administrator helper connection timed out".to_string())?
        .map_err(|error| format!("USBIP-UAC-002: administrator IPC connection failed: {error}"))?;
    let expected_process_id = process.process_id();
    let mut client_process_id = 0;
    unsafe {
        GetNamedPipeClientProcessId(HANDLE(server.as_raw_handle()), &mut client_process_id)
            .map_err(|error| {
                format!("USBIP-UAC-002: unable to authenticate administrator IPC client: {error}")
            })?;
    }
    if client_process_id == 0 || client_process_id != expected_process_id {
        return Err(
            "USBIP-UAC-002: administrator IPC client is not the process authorized for this session"
                .to_string(),
        );
    }
    let mut encoded = Vec::with_capacity(MAX_ELEVATED_RESPONSE_BYTES);
    let mut reader = server.take((MAX_ELEVATED_RESPONSE_BYTES + 1) as u64);
    tokio::time::timeout(timeout, reader.read_to_end(&mut encoded))
        .await
        .map_err(|_| "USBIP-UAC-003: administrator operation timed out".to_string())?
        .map_err(|error| format!("USBIP-UAC-002: administrator IPC read failed: {error}"))?;
    if encoded.len() > MAX_ELEVATED_RESPONSE_BYTES {
        return Err("USBIP-UAC-002: administrator response exceeded the safety limit".to_string());
    }
    let response: ElevatedResponse = serde_json::from_slice(&encoded)
        .map_err(|error| format!("USBIP-UAC-002: invalid administrator response: {error}"))?;
    drop(process);
    if response.success {
        Ok(response)
    } else {
        Err(response.message)
    }
}

#[cfg(not(target_os = "windows"))]
async fn run_elevated(_request: ElevatedRequest) -> Result<ElevatedResponse, String> {
    Err("USBIP-SETUP-004: USB/IP passthrough is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
const CLEANUP_POLL_ATTEMPTS: usize = 30;
#[cfg(target_os = "windows")]
const INNO_UNINSTALL_FLAGS: &str = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART";

/// Runs inside the elevated helper. Order matters: the vendor uninstaller is
/// the only supported way to retire the driver packages, service registration,
/// and scheduled task; the devnode sweep afterwards only has to catch the
/// nodes the vendor uninstaller silently failed to remove.
#[cfg(target_os = "windows")]
async fn cleanup_broken_transport() -> Result<(), String> {
    crate::dualsense::ensure_no_active_session().await.map_err(|_| {
        "USBIP-CLEAN-001: finish the active Sunshine stream before cleaning up the transport"
            .to_string()
    })?;
    if crate::dualsense::installed_usbip_version().is_some() {
        run_inno_uninstaller().await?;
    }
    remove_vhci_devnodes().await
}

#[cfg(target_os = "windows")]
async fn run_inno_uninstaller() -> Result<(), String> {
    use std::process::Stdio;

    let uninstall_string = find_usbip_uninstall_string().ok_or_else(|| {
        "USBIP-CLEAN-001: the USB/IP uninstaller registration is missing".to_string()
    })?;
    let (executable, arguments) = split_executable_command(&uninstall_string)
        .ok_or_else(|| "USBIP-CLEAN-001: the USB/IP uninstaller registration is invalid".to_string())?;
    let arguments = if arguments.is_empty() {
        INNO_UNINSTALL_FLAGS.to_string()
    } else {
        format!("{arguments} {INNO_UNINSTALL_FLAGS}")
    };
    let mut command = tokio::process::Command::new(&executable);
    command
        .args(arguments.split_whitespace())
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
        Ok(Ok(_)) => {}
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
    for _ in 0..3 {
        let interfaces = enumerate_vhci_interfaces()?;
        if interfaces.is_empty() {
            return Ok(());
        }
        for path in interfaces {
            if let Some(instance_id) = instance_id_from_interface_path(&path)
                && let Err(error) = uninstall_vhci_devnode(&instance_id)
            {
                return Err(error);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if enumerate_vhci_interfaces()?.is_empty() {
        Ok(())
    } else {
        Err(
            "USBIP-CLEAN-002: residual USB/IP host controller devices could not be removed"
                .to_string(),
        )
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
        return Err(format!(
            "USBIP-CLEAN-002: residual device {instance_id} could not be located (config error {})",
            located.0
        ));
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

#[cfg(target_os = "windows")]
async fn run_elevated_helper(token: uuid::Uuid, request: ElevatedRequest) -> i32 {
    use tokio::io::AsyncWriteExt;
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut client = match ClientOptions::new().write(true).open(pipe_name(token)) {
        Ok(client) => client,
        Err(_) => return 3,
    };
    let result: Result<ElevatedResponse, String> = async {
        if !crate::bat_runner::is_elevated() {
            return Err("USBIP-UAC-001: administrator authorization was not granted".to_string());
        }
        match request {
            ElevatedRequest::Install => {
                crate::dualsense::ensure_no_active_session().await.map_err(|_| {
                    "USBIP-SETUP-006: finish the active Sunshine stream before installing the transport"
                        .to_string()
                })?;
                let root = crate::dualsense::component_root();
                tokio::fs::create_dir_all(&root).await.map_err(|error| {
                    format!("USBIP-SETUP-005: unable to prepare component storage: {error}")
                })?;
                let progress = |_stage: &str, _value: u32| {};
                let result = crate::dualsense::ensure_pinned_usbip(&progress, &root, None)
                    .await
                    .map_err(|error| format!("USBIP-SETUP-007: transport installation failed: {error}"))?;
                let reboot_recommended = matches!(
                    result,
                    crate::dualsense::UsbipInstallResult::RebootRecommended
                );
                Ok(ElevatedResponse {
                    success: true,
                    port: None,
                    message: if reboot_recommended {
                        "reboot_recommended".to_string()
                    } else {
                        String::new()
                    },
                })
            }
            ElevatedRequest::Cleanup => {
                cleanup_broken_transport().await?;
                Ok(ElevatedResponse {
                    success: true,
                    port: None,
                    message: String::new(),
                })
            }
            ElevatedRequest::Attach {
                remote,
                tcp_port,
                bus_id,
            } => {
                let remote = validate_remote(&remote)?;
                let tcp_port = validate_tcp_port(Some(tcp_port))?;
                let bus_id = validate_bus_id(&bus_id)?;
                let output = run_usbip(vec![
                    "--tcp-port".to_string(),
                    tcp_port.to_string(),
                    "attach".to_string(),
                    "--remote".to_string(),
                    remote,
                    "--bus-id".to_string(),
                    bus_id,
                    "--terse".to_string(),
                    "--once".to_string(),
                ])
                .await?;
                let text = require_success(output, "attaching the remote device")?;
                let port = text.parse::<u8>().map_err(|_| {
                    "USBIP-EXEC-005: attach returned an invalid local port".to_string()
                })?;
                Ok(ElevatedResponse {
                    success: true,
                    port: Some(port),
                    message: String::new(),
                })
            }
            ElevatedRequest::Detach { port } => {
                if port == 0 {
                    return Err("USBIP-INPUT-004: the local USB/IP port is invalid".to_string());
                }
                let output = run_usbip(vec![
                    "detach".to_string(),
                    "--port".to_string(),
                    port.to_string(),
                ])
                .await?;
                require_success(output, "detaching the remote device")?;
                Ok(ElevatedResponse {
                    success: true,
                    port: None,
                    message: String::new(),
                })
            }
        }
    }
    .await;
    let succeeded = result.is_ok();
    let response = result.unwrap_or_else(|message| ElevatedResponse {
        success: false,
        port: None,
        message,
    });
    let Ok(encoded) = serde_json::to_string(&response) else {
        return 4;
    };
    if client
        .write_all(format!("{encoded}\n").as_bytes())
        .await
        .is_err()
    {
        return 4;
    }
    if succeeded { 0 } else { 1 }
}

#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next()?.as_str() != ELEVATED_ARG {
        return None;
    }
    let token = match arguments
        .next()
        .and_then(|value| uuid::Uuid::parse_str(&value).ok())
    {
        Some(token) => token,
        None => return Some(2),
    };
    let request = match arguments
        .next()
        .and_then(|value| serde_json::from_str(&value).ok())
    {
        Some(request) if arguments.next().is_none() => request,
        _ => return Some(2),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return Some(3),
    };
    Some(runtime.block_on(run_elevated_helper(token, request)))
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
            Some(("C:\\Program Files\\USBip\\unins000.exe".to_string(), String::new()))
        );
        assert_eq!(
            split_executable_command(r#""C:\Program Files\USBip\unins000.exe" /log=x"#),
            Some((
                "C:\\Program Files\\USBip\\unins000.exe".to_string(),
                "/log=x".to_string()
            ))
        );
        assert_eq!(
            split_executable_command(r"C:\USBip\unins000.exe /silent"),
            Some(("C:\\USBip\\unins000.exe".to_string(), "/silent".to_string()))
        );
        assert_eq!(split_executable_command("  "), None);
        assert_eq!(split_executable_command(r#""unclosed"#), None);
    }

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
