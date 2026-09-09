//! Serialized native component writes and bounded helper completion.

#[derive(Debug, Clone)]
pub struct Maintenance {
    pub id: String,
    pub journal: std::path::PathBuf,
}

/// Hold the cross-process write lock and verify the Core-issued operation identity.
pub(crate) fn with_maintenance<T>(
    operation: &Maintenance,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    use std::io::Read;
    #[cfg(target_os = "windows")]
    use std::os::windows::fs::OpenOptionsExt;
    let mut lock_path = operation.journal.as_os_str().to_os_string();
    lock_path.push(".lock");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(target_os = "windows")]
    options.share_mode(0);
    let _lock = options
        .open(std::path::PathBuf::from(lock_path))
        .map_err(|_| "HDR-OP-001: component write lock is unavailable".to_string())?;
    let mut bytes = Vec::new();
    std::fs::File::open(&operation.journal)
        .and_then(|file| file.take(4097).read_to_end(&mut bytes))
        .map_err(|_| "HDR-OP-002: component operation is no longer authorized".to_string())?;
    if bytes.len() > 4096 {
        return Err("HDR-OP-002: invalid component operation".to_string());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "HDR-OP-002: invalid component operation".to_string())?;
    if value
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        != Some(operation.id.as_str())
    {
        return Err("HDR-OP-002: component operation was superseded".to_string());
    }
    action()
}

pub(crate) static COMPONENT_OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[cfg(target_os = "windows")]
static PENDING_HELPER: tokio::sync::Mutex<Option<crate::utils::ElevatedProcess>> =
    tokio::sync::Mutex::const_new(None);

#[cfg(target_os = "windows")]
pub(crate) async fn ensure_helper_finished() -> Result<(), String> {
    let mut pending = PENDING_HELPER.lock().await;
    if let Some(process) = pending.as_ref() {
        match process.exit_code() {
            Ok(Some(_)) => {
                *pending = None;
            }
            // 查询失败也不能推断助手已经停止。
            _ => {
                return Err(
                    "RTXHDR-BUSY-001: the previous component helper has not exited".to_string(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) async fn wait_for_component_helper(
    process: crate::utils::ElevatedProcess,
) -> Result<i32, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        match process.exit_code() {
            Ok(Some(code)) => return Ok(code),
            Ok(None) if tokio::time::Instant::now() < deadline => {}
            _ => {
                // 超时只结束本次等待，不意味着进程已退出；保留句柄阻止重叠操作。
                *PENDING_HELPER.lock().await = Some(process);
                return Err("RTXHDR-UAC-004: component helper did not finish in time".to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
