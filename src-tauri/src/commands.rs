use crate::windows;
use base64::Engine as _;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

/// 共享的 CDN HTTP 客户端（连接池复用，避免频繁 TLS 握手被 CDN 拒绝）
pub fn cdn_client() -> &'static reqwest::Client {
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
            build_request()
                .send()
                .await
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
        "logs" | "log_console" => {
            windows::open_log_console(&app);
        }
        _ => return Err(format!("未知的工具名称: {}", tool_name)),
    }
    Ok(())
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn value_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "enabled" => Some(true),
            "false" | "0" | "no" | "disabled" | "" => Some(false),
            _ => None,
        },
        Value::Number(value) => Some(value.as_i64().unwrap_or(0) != 0),
        _ => None,
    }
}

#[tauri::command]
pub async fn launch_app(
    cmd: Option<String>,
    working_dir: Option<String>,
    elevated: Option<bool>,
    app: Option<Value>,
) -> Result<(), String> {
    let cmd = cmd
        .or_else(|| app.as_ref().and_then(|app| value_string(app, "cmd")))
        .unwrap_or_default();
    let has_detached = app
        .as_ref()
        .and_then(|app| app.get("detached"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());

    if cmd.trim().is_empty() && !has_detached {
        return Err("启动命令不能为空".to_string());
    }
    info!("🚀 启动应用: {}", cmd);

    let working_dir = working_dir.or_else(|| {
        app.as_ref()
            .and_then(|app| value_string(app, "working-dir"))
    });
    let is_elevated = elevated
        .or_else(|| app.as_ref().and_then(|app| value_bool(app.get("elevated"))))
        .unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        use ::windows::Win32::Foundation::HWND;
        use ::windows::Win32::UI::Shell::ShellExecuteW;
        use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use ::windows::core::PCWSTR;

        fn to_wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0u16)).collect()
        }

        fn shell_exec(
            operation: &[u16],
            file: &str,
            params: &str,
            dir_wide: Option<&Vec<u16>>,
        ) -> isize {
            let file_wide = to_wide(file);
            let params_wide = to_wide(params);
            let result = unsafe {
                ShellExecuteW(
                    Some(HWND(std::ptr::null_mut())),
                    PCWSTR(operation.as_ptr()),
                    PCWSTR(file_wide.as_ptr()),
                    PCWSTR(params_wide.as_ptr()),
                    dir_wide.map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
                    SW_SHOWNORMAL,
                )
            };
            result.0 as isize
        }

        fn error_desc(code: isize) -> &'static str {
            match code {
                0 | 8 => "内存不足",
                2 => "找不到指定文件",
                3 => "找不到指定路径",
                5 => "拒绝访问",
                11 => "格式无效",
                26 => "共享冲突",
                27 => "文件关联不完整",
                28 => "DDE 请求超时",
                29 => "DDE 事务失败",
                30 => "DDE 繁忙",
                31 => "没有关联的应用程序",
                32 => "DLL 未找到",
                _ => "未知错误",
            }
        }

        fn run_command(
            command: &str,
            elevated: bool,
            dir_wide: Option<&Vec<u16>>,
            label: &str,
        ) -> Result<(), String> {
            let operation = to_wide(if elevated { "runas" } else { "open" });
            let trimmed = command.trim();
            if trimmed.is_empty() {
                return Ok(());
            }

            if trimmed.starts_with('"') {
                if let Some(end) = trimmed[1..].find('"') {
                    let file = &trimmed[1..=end];
                    let params = if end + 2 < trimmed.len() {
                        trimmed[end + 2..].trim()
                    } else {
                        ""
                    };
                    info!("  [{} quoted] file: {}, params: {}", label, file, params);
                    let rc = shell_exec(&operation, file, params, dir_wide);
                    return if rc > 32 {
                        Ok(())
                    } else {
                        Err(format!(
                            "{} failed: {} (code {})",
                            label,
                            error_desc(rc),
                            rc
                        ))
                    };
                }
            }

            info!("  [{} try1] file: {}", label, trimmed);
            let rc1 = shell_exec(&operation, trimmed, "", dir_wide);
            if rc1 > 32 {
                return Ok(());
            }

            if let Some(space_pos) = trimmed.find(' ') {
                let file = &trimmed[..space_pos];
                let params = trimmed[space_pos + 1..].trim();
                info!("  [{} try2] file: {}, params: {}", label, file, params);
                let rc2 = shell_exec(&operation, file, params, dir_wide);
                if rc2 > 32 {
                    return Ok(());
                }
                let best_rc = if rc1 == 2 || rc1 == 3 { rc2 } else { rc1 };
                Err(format!(
                    "{} failed: {} (code {})",
                    label,
                    error_desc(best_rc),
                    best_rc
                ))
            } else {
                Err(format!(
                    "{} failed: {} (code {})",
                    label,
                    error_desc(rc1),
                    rc1
                ))
            }
        }

        let operation = to_wide(if is_elevated { "runas" } else { "open" });
        let trimmed = cmd.trim();
        let dir_wide: Option<Vec<u16>> = working_dir
            .as_ref()
            .filter(|d| !d.is_empty())
            .map(|d| to_wide(d));

        if let Some(app) = app.as_ref() {
            if let Some(prep_cmds) = app.get("prep-cmd").and_then(Value::as_array) {
                for prep in prep_cmds {
                    let Some(do_cmd) = value_string(prep, "do") else {
                        continue;
                    };
                    let elevated = value_bool(prep.get("elevated")).unwrap_or(false);
                    run_command(&do_cmd, elevated, dir_wide.as_ref(), "prep-cmd")?;
                }
            }

            if let Some(detached_cmds) = app.get("detached").and_then(Value::as_array) {
                for detached in detached_cmds {
                    let Some(detached_cmd) = detached.as_str() else {
                        continue;
                    };
                    run_command(detached_cmd, is_elevated, dir_wide.as_ref(), "detached")?;
                }
            }
        }

        if trimmed.is_empty() {
            return Ok(());
        }

        // 解析引号包裹的路径
        if trimmed.starts_with('"') {
            if let Some(end) = trimmed[1..].find('"') {
                let file = &trimmed[1..=end];
                let params = if end + 2 < trimmed.len() {
                    trimmed[end + 2..].trim()
                } else {
                    ""
                };
                info!("  📂 [quoted] file: {}, params: {}", file, params);
                let rc = shell_exec(&operation, file, params, dir_wide.as_ref());
                return if rc > 32 {
                    info!("✅ 应用启动成功: {}", file);
                    Ok(())
                } else {
                    Err(format!("启动失败: {} (错误码: {})", error_desc(rc), rc))
                };
            }
        }

        // 策略：先把整个字符串作为 file 尝试，失败后再分割重试
        // 这样 "J:\DESSERT Soft\WannabeCN.exe" 和 "steam://run/123" 都能直接成功
        info!("  📂 [尝试1] 整个命令作为 file: {}", trimmed);
        let rc1 = shell_exec(&operation, trimmed, "", dir_wide.as_ref());
        if rc1 > 32 {
            info!("✅ 应用启动成功: {}", trimmed);
            return Ok(());
        }

        // 第一次失败 → 尝试在第一个空格处分割 (file + params)
        if let Some(space_pos) = trimmed.find(' ') {
            let file = &trimmed[..space_pos];
            let params = trimmed[space_pos + 1..].trim();
            info!(
                "  📂 [尝试2] file: {}, params: {} (尝试1 错误码: {})",
                file, params, rc1
            );
            let rc2 = shell_exec(&operation, file, params, dir_wide.as_ref());
            if rc2 > 32 {
                info!("✅ 应用启动成功: {}", file);
                return Ok(());
            }
            // 两次都失败，返回更有意义的那个错误
            let (best_rc, _context) = if rc1 == 2 || rc1 == 3 {
                (rc2, file)
            } else {
                (rc1, trimmed)
            };
            Err(format!(
                "启动失败: {} (错误码: {})",
                error_desc(best_rc),
                best_rc
            ))
        } else {
            Err(format!("启动失败: {} (错误码: {})", error_desc(rc1), rc1))
        }
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

/// 话术响应（包含 ETag 用于条件请求）
#[derive(Serialize)]
pub struct SpeechPhrasesResponse {
    pub phrases: Vec<String>,
    pub etag: Option<String>,
}

#[tauri::command]
pub async fn fetch_speech_phrases(
    if_none_match: Option<String>,
) -> Result<Option<SpeechPhrasesResponse>, String> {
    let url = "https://assets.alkaidlab.com/speech-phrases.json";

    let result = cdn_get(url, if_none_match.as_deref()).await?;
    let (response, etag) = match result {
        CdnResult::NotModified => return Ok(None),
        CdnResult::Fresh(resp, etag) => (resp, etag),
    };

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("读取响应体失败: {}", e))?;

    // 去除 UTF-8 BOM（CDN 返回的 JSON 可能带 BOM，serde_json 不接受）
    let body_text = body_text.trim_start_matches('\u{FEFF}');

    let phrases: Vec<String> = serde_json::from_str(body_text).map_err(|e| {
        let preview = if body_text.len() > 200 {
            &body_text[..200]
        } else {
            body_text
        };
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
pub async fn fetch_remote_bytes(
    url: String,
    if_none_match: Option<String>,
) -> Result<Option<RemoteBytesResponse>, String> {
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

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取响应体失败: {}", e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);

    info!("✅ 代理下载成功 ({} bytes)", bytes.len());
    Ok(Some(RemoteBytesResponse { data_url, etag }))
}

// ===== AI API 代理（绕过 CORS） =====

/// AI API 代理专用 HTTP 客户端
fn ai_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_max_idle_per_host(3)
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create AI HTTP client")
    })
}

#[derive(Debug, Deserialize)]
pub struct AiProxyRequest {
    pub url: String,
    pub method: String, // "GET" or "POST"
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
}

/// 通用 AI API 代理：转发 HTTP 请求到外部 AI 服务，绕过 WebView CORS 限制
#[tauri::command]
pub async fn ai_api_proxy(request: AiProxyRequest) -> Result<String, String> {
    let client = ai_client();

    let mut req_builder = match request.method.to_uppercase().as_str() {
        "GET" => client.get(&request.url),
        "POST" => client.post(&request.url),
        _ => return Err(format!("不支持的 HTTP 方法: {}", request.method)),
    };

    // 设置请求头
    for (key, value) in &request.headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    // 设置请求体
    if let Some(body) = &request.body {
        req_builder = req_builder.header("Content-Type", "application/json");
        req_builder = req_builder.body(body.clone());
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("AI API 请求失败: {}", e))?;

    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 AI API 响应失败: {}", e))?;

    if status >= 400 {
        return Err(format!("{} - {}", status, &body[..body.len().min(500)]));
    }

    Ok(body)
}

// ===== 桌面截屏（用于 AI 桌宠视觉识别） =====

/// 截取主显示器画面，返回 base64 编码的 JPEG（质量 60，尺寸缩放到最大 1024px 宽）
#[tauri::command]
pub async fn capture_screenshot() -> Result<String, String> {
    use std::io::Cursor;
    use xcap::Monitor;

    // 在阻塞线程中执行截屏（xcap 内部使用同步 API）
    tokio::task::spawn_blocking(|| {
        let monitors = Monitor::all().map_err(|e| format!("枚举显示器失败: {}", e))?;
        let monitor = monitors.into_iter().next().ok_or("没有找到显示器")?;

        let image = monitor
            .capture_image()
            .map_err(|e| format!("截屏失败: {}", e))?;

        // 缩放到最大 1024px 宽以减少 token 消耗
        let (w, h) = (image.width(), image.height());
        let max_width = 1024u32;
        let resized = if w > max_width {
            let new_h = (h as f64 * max_width as f64 / w as f64) as u32;
            image::imageops::resize(
                &image,
                max_width,
                new_h,
                image::imageops::FilterType::Triangle,
            )
        } else {
            image::imageops::resize(&image, w, h, image::imageops::FilterType::Triangle)
        };

        // RGBA → RGB (JPEG 不支持 alpha 通道)
        let rgb_image = image::DynamicImage::ImageRgba8(resized).to_rgb8();

        // 编码为 JPEG（质量 60）
        let mut buf = Cursor::new(Vec::new());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 60);
        rgb_image
            .write_with_encoder(encoder)
            .map_err(|e| format!("JPEG 编码失败: {}", e))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
        Ok(format!("data:image/jpeg;base64,{}", b64))
    })
    .await
    .map_err(|e| format!("截屏任务失败: {}", e))?
}
