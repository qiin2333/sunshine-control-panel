//! Tauri command surface for the Device Hub USB/IP panel.

use super::{
    ElevatedRequest, OPERATION_LOCK, UsbipRemoteDevice, UsbipStatus, validate_bus_id,
    validate_remote, validate_tcp_port,
};
use super::elevated::run_elevated;
use super::exec::{parse_remote_devices, require_success, run_usbip};

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
    let mut status = super::status::usbip_get_status().await?;
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
    super::status::usbip_get_status().await
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
