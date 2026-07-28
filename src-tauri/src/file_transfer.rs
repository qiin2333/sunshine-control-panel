use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::clipboard;
use crate::sunshine::{create_https_client, get_active_sessions, get_sunshine_url};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FileOffer {
    id: String,
    name: String,
    size: u64,
    mime: String,
    download_url: String,
    expires_in: u64,
    #[serde(rename = "type")]
    offer_type: String,
}

pub fn parse_send_to_client_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut take_rest = false;

    for arg in args {
        if take_rest {
            if !arg.trim().is_empty() {
                out.push(arg.clone());
            }
            continue;
        }

        if arg == "--send-to-client" {
            take_rest = true;
            continue;
        }

        if let Some(path) = arg.strip_prefix("--send-to-client=") {
            if !path.trim().is_empty() {
                out.push(path.to_string());
            }
        }
    }

    out
}

pub fn dispatch_cli_send(paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        match send_paths_to_client(paths).await {
            Ok(msg) => info!("file transfer: {msg}"),
            Err(e) => warn!("file transfer failed: {e}"),
        }
    });
}

#[tauri::command]
pub async fn send_file_to_client(path: String) -> Result<String, String> {
    send_paths_to_client(vec![path]).await
}

async fn send_paths_to_client(paths: Vec<String>) -> Result<String, String> {
    let first = paths.first().ok_or_else(|| "没有选择文件".to_string())?;

    if paths.len() > 1 {
        return Err("当前版本仅支持一次发送一个文件".to_string());
    }

    let active_sessions = get_active_sessions()
        .await
        .map_err(|e| format!("无法查询客户端连接状态: {e}"))?;

    let running_sessions = active_sessions
        .iter()
        .filter(|s| s.state.eq_ignore_ascii_case("RUNNING"))
        .count();

    if running_sessions == 0 {
        return Err("没有正在串流的客户端连接".to_string());
    }

    let path = canonicalize_file(first)?;
    let offer = register_offer(&path).await?;
    let payload = build_client_offer_payload(&offer)?;
    clipboard::post_file_offer_payload(payload)
        .await
        .map_err(|e| format!("发送文件 offer 失败: {e}"))?;

    Ok(format!(
        "已发送文件 offer: {} ({} bytes, {} 个客户端)",
        offer.name, offer.size, running_sessions
    ))
}

fn canonicalize_file(path: &str) -> Result<PathBuf, String> {
    let raw = PathBuf::from(path);
    let canonical = std::fs::canonicalize(&raw)
        .map_err(|e| format!("文件不存在或无法访问: {} ({e})", raw.display()))?;
    if !canonical.is_file() {
        return Err("当前版本仅支持发送单个文件".to_string());
    }
    Ok(canonical)
}

async fn register_offer(path: &PathBuf) -> Result<FileOffer, String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client().await?;
    let endpoint = format!("{}/api/v1/file-transfer/offers", url.trim_end_matches('/'));

    let body = serde_json::json!({
        "path": path.to_string_lossy(),
    });

    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("注册文件 offer 失败: HTTP {status} {text}"));
    }

    serde_json::from_str::<FileOffer>(&text)
        .map_err(|e| format!("解析文件 offer 响应失败: {e}; body={text}"))
}

fn build_client_offer_payload(offer: &FileOffer) -> Result<Vec<u8>, String> {
    serde_json::to_vec(offer).map_err(|e| format!("编码文件 offer 失败: {e}"))
}
