//! Core-owned HDR configuration and component maintenance protocol.

use reqwest::header::{ETAG, HeaderValue, IF_MATCH};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendSettings {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub selected_backend: Option<String>,
    pub backends: BTreeMap<String, BackendSettings>,
}

#[derive(Debug, Clone)]
pub struct ConfigState {
    pub settings: Settings,
    pub etag: HeaderValue,
}

pub use crate::native_components::operation::Maintenance;

async fn request(
    route: &str,
    body: Option<Value>,
    etag: Option<&HeaderValue>,
    operation_id: Option<&str>,
) -> Result<(Value, Option<HeaderValue>), String> {
    let base = crate::sunshine::get_local_sunshine_url()
        .await
        .map_err(|_| "HDR-CFG-001: Sunshine is unavailable".to_string())?;
    let client = crate::sunshine::create_https_client()?;
    let url = format!("{}/api/hdr-enhanced/{route}", base.trim_end_matches('/'));
    let mut builder = if let Some(body) = body {
        client.post(url).json(&body)
    } else {
        client.get(url)
    };
    if let Some(etag) = etag {
        builder = builder.header(IF_MATCH, etag);
    }
    if let Some(operation_id) = operation_id {
        builder = builder.header("X-HDR-Operation", operation_id);
    }
    let mut response = builder
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| "HDR-CFG-001: unable to contact Sunshine".to_string())?;
    let status = response.status();
    let etag = response.headers().get(ETAG).cloned();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "HDR-CFG-001: incomplete response".to_string())?
    {
        if bytes.len().saturating_add(chunk.len()) > 64 * 1024 {
            return Err("HDR-CFG-002: response exceeds the configuration limit".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body: Value =
        serde_json::from_slice(&bytes).map_err(|_| "HDR-CFG-002: invalid response".to_string())?;
    if !status.is_success() || body.get("status") != Some(&Value::Bool(true)) {
        let code = match (
            status.as_u16(),
            body.get("error_code").and_then(Value::as_str),
        ) {
            (_, Some("hdr_component_untrusted")) => {
                "HDR-PKG-009: the component version is missing or untrusted"
            }
            (_, Some("hdr_config_invalid")) => {
                "HDR-CFG-003: the HDR configuration file could not be read"
            }
            (_, Some("hdr_helper_running")) => {
                "HDR-OP-003: the component installer is still running"
            }
            (412, _) => "HDR-CFG-006: settings changed in another window; refresh before saving",
            (428, _) => "HDR-CFG-007: conditional update is required",
            (409, _) => "HDR-OP-001: component is in use or awaiting maintenance completion",
            _ => "HDR-CFG-002: Sunshine rejected the component operation",
        };
        log::warn!(
            "HDR operation rejected: HTTP {} / {:?}",
            status.as_u16(),
            body.get("error_code")
        );
        return Err(code.to_string());
    }
    Ok((body, etag))
}

fn decode_config(body: Value, etag: Option<HeaderValue>) -> Result<ConfigState, String> {
    let etag = etag
        .filter(|value| {
            value.to_str().is_ok_and(|text| {
                text.len() == 73
                    && text.starts_with("\"hdr-v1-")
                    && text.ends_with('"')
                    && text.as_bytes()[8..72]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            })
        })
        .ok_or_else(|| "HDR-CFG-007: missing strong configuration validator".to_string())?;
    let settings: Settings =
        serde_json::from_value(body.get("config").cloned().unwrap_or(Value::Null))
            .map_err(|_| "HDR-CFG-002: invalid configuration document".to_string())?;
    if settings.schema_version != 1 {
        return Err("HDR-CFG-007: unsupported configuration schema".to_string());
    }
    Ok(ConfigState { settings, etag })
}

pub async fn get_config() -> Result<ConfigState, String> {
    let (body, etag) = request("config", None, None, None).await?;
    decode_config(body, etag)
}

pub async fn save_config(
    state: ConfigState,
    operation_id: Option<&str>,
) -> Result<ConfigState, String> {
    let body = serde_json::to_value(&state.settings)
        .map_err(|_| "HDR-CFG-002: invalid settings".to_string())?;
    let (body, etag) = request("config", Some(body), Some(&state.etag), operation_id).await?;
    decode_config(body, etag)
}

pub async fn get_status() -> Result<Value, String> {
    let (body, _) = request("status", None, None, None).await?;
    Ok(body.get("runtime").cloned().unwrap_or(Value::Null))
}

pub async fn begin_maintenance() -> Result<Maintenance, String> {
    maintenance_identity("begin", None).await
}

pub async fn inspect_maintenance() -> Result<Maintenance, String> {
    maintenance_identity("inspect", None).await
}

pub async fn verify_maintenance(operation_id: &str) -> Result<Maintenance, String> {
    maintenance_identity("verify", Some(operation_id)).await
}

async fn maintenance_identity(
    action: &str,
    operation_id: Option<&str>,
) -> Result<Maintenance, String> {
    let (body, _) = request(
        "components/alkaidlab.nvidia_rtx_video/maintenance",
        Some(json!({"action":action, "operation_id":operation_id})),
        None,
        None,
    )
    .await?;
    let id = body
        .get("operation_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "HDR-OP-002: missing maintenance identity".to_string())?;
    let journal = body
        .get("journal_path")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .filter(|path| {
            path.is_absolute()
                && path
                    .file_name()
                    .is_some_and(|name| name == "hdr_enhanced.maintenance.json")
        })
        .ok_or_else(|| "HDR-OP-002: invalid maintenance location".to_string())?;
    if operation_id.is_some_and(|expected| expected != id) {
        return Err("HDR-OP-002: maintenance identity does not match Sunshine".to_string());
    }
    Ok(Maintenance {
        id: id.to_string(),
        journal,
    })
}

pub async fn finish_maintenance(operation_id: &str) -> Result<(), String> {
    request(
        "components/alkaidlab.nvidia_rtx_video/maintenance",
        Some(json!({"action":"commit", "operation_id":operation_id})),
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn recover_maintenance() -> Result<(), String> {
    request(
        "components/alkaidlab.nvidia_rtx_video/maintenance",
        Some(json!({"action":"recover"})),
        None,
        None,
    )
    .await?;
    Ok(())
}
