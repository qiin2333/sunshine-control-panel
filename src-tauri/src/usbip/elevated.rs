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
pub(super) async fn run_elevated(request: ElevatedRequest) -> Result<ElevatedResponse, String> {
    use tokio::io::AsyncReadExt;

    let timeout = request.timeout();
    let token = uuid::Uuid::new_v4();
    let mut server = crate::elevation::create_hardened_pipe(
        &crate::elevation::pipe_name("usbip", token),
        "USBIP-UAC-002",
    )?;
    let payload = serde_json::to_string(&request).map_err(|error| {
        format!("USBIP-UAC-002: unable to encode administrator request: {error}")
    })?;
    let token_arg = token.to_string();
    let process = crate::elevation::spawn_helper(
        vec![ELEVATED_ARG.to_string(), token_arg, payload],
        "USBIP-UAC-001",
    )
    .await?;

    // Races the connect against an early helper exit (UAC cancellation) and
    // verifies the pipe client is the process spawned above.
    crate::elevation::connect_verified(
        &mut server,
        &process,
        ELEVATED_CONNECT_TIMEOUT,
        "USBIP-UAC-002",
    )
    .await?;
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
    // The helper's own verdict stays authoritative; the exit code only gates
    // the success path as a belt-and-suspenders check.
    if !response.success {
        return Err(response.message);
    }
    let status = crate::elevation::wait_for_exit(&process, std::time::Duration::from_secs(5))
        .await
        .map_err(|error| format!("USBIP-UAC-002: administrator helper wait failed: {error}"))?
        .ok_or_else(|| "USBIP-UAC-002: administrator helper did not exit".to_string())?;
    if status == 0 {
        Ok(response)
    } else {
        Err(format!(
            "USBIP-UAC-002: administrator helper failed with exit code {status}"
        ))
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) async fn run_elevated(_request: ElevatedRequest) -> Result<ElevatedResponse, String> {
    Err("USBIP-SETUP-004: USB/IP passthrough is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
async fn run_elevated_helper(token: uuid::Uuid, request: ElevatedRequest) -> i32 {
    use tokio::io::AsyncWriteExt;

    let mut client = match crate::elevation::connect_helper_pipe(
        &crate::elevation::pipe_name("usbip", token),
        "USBIP-UAC-002",
    )
    .await
    {
        Ok(client) => client,
        Err(_) => return 3,
    };
    let result: Result<ElevatedResponse, String> = async {
        crate::elevation::bind_lifetime_job("USBIP-UAC-002")?;
        if !crate::elevation::is_elevated() {
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
