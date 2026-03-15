use tauri::{AppHandle, Manager, Emitter};
use log::{info, warn};
use crate::windows;
use base64::Engine as _;
use std::sync::OnceLock;
use serde::Serialize;

/// 共享的 CDN HTTP 客户端（连接池复用，避免频繁 TLS 握手被 CDN 拒绝）
fn cdn_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_max_idle_per_host(5)
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create CDN HTTP client")
    })
}

/// CDN 响应（区分"未修改"和"有新数据"两种状态）
enum CdnResult {
    /// 304 Not Modified，资源未变化
    NotModified,
    /// 200 OK，返回响应和 ETag
    Fresh(reqwest::Response, Option<String>),
}

/// 带一次重试的 CDN GET 请求，支持 ETag 条件请求
async fn cdn_get(url: &str, if_none_match: Option<&str>) -> Result<CdnResult, String> {
    let client = cdn_client();
    
    let build_request = || {
        let mut req = client.get(url);
        if let Some(etag) = if_none_match {
            req = req.header("If-None-Match", etag);
        }
        req
    };
    
    let result = build_request().send().await;
    let response = match result {
        Ok(resp) => resp,
        Err(_first_err) => {
            warn!("⚠️  CDN 请求失败，1s 后重试");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            build_request().send().await
                .map_err(|e| format!("请求失败（重试后仍失败）: {}", e))?
        }
    };
    
    // 304 Not Modified
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(CdnResult::NotModified);
    }
    
    if !response.status().is_success() {
        return Err(format!("HTTP 错误: {}", response.status()));
    }
    
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    
    Ok(CdnResult::Fresh(response, etag))
}

#[tauri::command]
pub async fn toggle_dark_mode(_window: tauri::Window) -> Result<bool, String> {
    // Tauri 通过前端控制主题，这里只是示例
    Ok(true)
}

#[tauri::command]
pub async fn open_tool_window(app: AppHandle, tool_name: String) -> Result<(), String> {
    info!("🔧 打开工具窗口: {}", tool_name);
    
    match tool_name.as_str() {
        "main" => {
            if let Some(window) = app.get_webview_window("main") {
                windows::show_and_activate_window(&window);
            }
        }
        "vdd" => {
            if let Some(window) = app.get_webview_window("main") {
                windows::show_and_activate_window(&window);
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "about" => {
            windows::open_about_window(&app)?;
        }
        _ => return Err(format!("未知的工具名称: {}", tool_name)),
    }
    Ok(())
}

#[tauri::command]
pub async fn launch_app(cmd: String, working_dir: Option<String>, elevated: Option<bool>) -> Result<(), String> {
    if cmd.trim().is_empty() {
        return Err("启动命令不能为空".to_string());
    }
    info!("🚀 启动应用: {}", cmd);

    let is_elevated = elevated.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        use ::windows::core::PCWSTR;
        use ::windows::Win32::Foundation::HWND;
        use ::windows::Win32::UI::Shell::ShellExecuteW;
        use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        fn to_wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0u16)).collect()
        }

        let operation = to_wide(if is_elevated { "runas" } else { "open" });

        // 分离命令和参数
        let trimmed = cmd.trim();
        let (file, params) = if trimmed.starts_with('"') {
            if let Some(end) = trimmed[1..].find('"') {
                let f = &trimmed[1..=end];
                let p = trimmed[end + 2..].trim();
                (f.to_string(), p.to_string())
            } else {
                (trimmed.to_string(), String::new())
            }
        } else if let Some(space_pos) = trimmed.find(' ') {
            (trimmed[..space_pos].to_string(), trimmed[space_pos + 1..].to_string())
        } else {
            (trimmed.to_string(), String::new())
        };

        let file_wide = to_wide(&file);
        let params_wide = to_wide(&params);
        let dir_wide: Option<Vec<u16>> = working_dir.as_ref().filter(|d| !d.is_empty()).map(|d| to_wide(d));

        let result = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(operation.as_ptr()),
                PCWSTR(file_wide.as_ptr()),
                PCWSTR(params_wide.as_ptr()),
                dir_wide.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
                SW_SHOWNORMAL,
            )
        };

        if (result.0 as isize) <= 32 {
            Err(format!("启动失败，错误码: {}", result.0 as isize))
        } else {
            info!("✅ 应用启动成功: {}", file);
            Ok(())
        }
    }).await.map_err(|e| format!("任务执行失败: {}", e))?
}

/// 话术响应（包含 ETag 用于条件请求）
#[derive(Serialize)]
pub struct SpeechPhrasesResponse {
    pub phrases: Vec<String>,
    pub etag: Option<String>,
}

#[tauri::command]
pub async fn fetch_speech_phrases(if_none_match: Option<String>) -> Result<Option<SpeechPhrasesResponse>, String> {
    let url = "https://assets.alkaidlab.com/speech-phrases.json";
    
    let result = cdn_get(url, if_none_match.as_deref()).await?;
    let (response, etag) = match result {
        CdnResult::NotModified => return Ok(None),
        CdnResult::Fresh(resp, etag) => (resp, etag),
    };
    
    let body_text = response.text().await
        .map_err(|e| format!("读取响应体失败: {}", e))?;
    
    // 去除 UTF-8 BOM（CDN 返回的 JSON 可能带 BOM，serde_json 不接受）
    let body_text = body_text.trim_start_matches('\u{FEFF}');
    
    let phrases: Vec<String> = serde_json::from_str(body_text)
        .map_err(|e| {
            let preview = if body_text.len() > 200 { &body_text[..200] } else { body_text };
            format!("JSON 解析失败: {}，响应前200字符: {}", e, preview)
        })?;
    
    info!("✅ 话术加载成功，共 {} 条", phrases.len());
    Ok(Some(SpeechPhrasesResponse { phrases, etag }))
}

/// 远程资源响应（包含 ETag 用于条件请求）
#[derive(Serialize)]
pub struct RemoteBytesResponse {
    pub data_url: String,
    pub etag: Option<String>,
}

/// 通过 Rust 后端代理下载远程资源（绕过 WebView 的 CORS 限制）
/// 支持 ETag 条件请求：传入 if_none_match 则只在资源变化时返回数据
/// 返回 None 表示 304 未修改
#[tauri::command]
pub async fn fetch_remote_bytes(url: String, if_none_match: Option<String>) -> Result<Option<RemoteBytesResponse>, String> {
    // 安全检查：仅允许特定域名
    if !url.starts_with("https://assets.alkaidlab.com/") {
        return Err(format!("不允许代理此 URL: {}", url));
    }
    
    let result = cdn_get(&url, if_none_match.as_deref()).await?;
    let (response, etag) = match result {
        CdnResult::NotModified => return Ok(None),
        CdnResult::Fresh(resp, etag) => (resp, etag),
    };
    
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    
    let bytes = response.bytes().await
        .map_err(|e| format!("读取响应体失败: {}", e))?;
    
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);
    
    info!("✅ 代理下载成功 ({} bytes)", bytes.len());
    Ok(Some(RemoteBytesResponse { data_url, etag }))
}
