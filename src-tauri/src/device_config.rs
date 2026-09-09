//! Shared Device Hub configuration I/O. Never include client tunnel credentials.
use serde::de::DeserializeOwned;

const MAX_DEVICE_API_RESPONSE_BYTES: usize = 64 * 1024;
pub(crate) static DEVICE_CONFIG_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) async fn read_device_api_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
    operation: &str,
) -> Result<T, String> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_DEVICE_API_RESPONSE_BYTES as u64)
    {
        return Err(format!("{operation} response exceeds 64 KiB"));
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_DEVICE_API_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Unable to read {operation} response: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_DEVICE_API_RESPONSE_BYTES {
            return Err(format!("{operation} response exceeds 64 KiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let detail = String::from_utf8_lossy(&bytes);
        return Err(format!("{operation} failed ({status}): {detail}"));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("Invalid {operation} response: {error}"))
}
