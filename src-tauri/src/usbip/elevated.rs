//! Elevated IPC for privileged USB/IP operations.
//!
//! Privileged operations run in a short-lived elevated copy of this
//! executable, authenticated over a per-invocation named pipe.

use super::{
    ELEVATED_ARG, ELEVATED_CONNECT_TIMEOUT, MAX_ELEVATED_RESPONSE_BYTES, ElevatedRequest,
    ElevatedResponse,
};
#[cfg(target_os = "windows")]
use super::{validate_bus_id, validate_remote, validate_tcp_port};

#[cfg(target_os = "windows")]
fn pipe_name(token: uuid::Uuid) -> String {
    format!(r"\\.\pipe\sunshine-usbip-{token}")
}

#[cfg(target_os = "windows")]
pub(super) async fn run_elevated(request: ElevatedRequest) -> Result<ElevatedResponse, String> {
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
pub(super) async fn run_elevated(_request: ElevatedRequest) -> Result<ElevatedResponse, String> {
    Err("USBIP-SETUP-004: USB/IP passthrough is only supported on Windows".to_string())
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
                super::device::cleanup_broken_transport().await?;
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
                let output = super::exec::run_usbip(vec![
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
                let text = super::exec::require_success(output, "attaching the remote device")?;
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
                let output = super::exec::run_usbip(vec![
                    "detach".to_string(),
                    "--port".to_string(),
                    port.to_string(),
                ])
                .await?;
                super::exec::require_success(output, "detaching the remote device")?;
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
