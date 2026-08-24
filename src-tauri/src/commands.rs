use crate::{toolbar, windows};
use base64::Engine as _;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

/// Shared CDN HTTP client with connection pooling.
pub fn cdn_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // Redirect targets intentionally bypass the initial asset allowlist.
        // assets.alkaidlab.com and its redirect configuration are controlled by
        // the domain owner, so redirects are part of the trusted delivery chain.
        reqwest::Client::builder()
            .pool_max_idle_per_host(5)
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create CDN HTTP client")
    })
}

/// CDN response state.
enum CdnResult {
    /// 304 Not Modified.
    NotModified,
    /// 200 OK response and optional ETag.
    Fresh(reqwest::Response, Option<String>),
}

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn is_allowed_asset_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("assets.alkaidlab.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
    })
}

/// CDN GET with one retry and optional ETag conditional request.
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
            warn!("CDN request failed; retrying in 1s");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            build_request()
                .send()
                .await
                .map_err(|e| format!("Request failed after retry: {}", e))?
        }
    };

    // 304 Not Modified
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(CdnResult::NotModified);
    }

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
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
    // Theme state is controlled by the frontend.
    Ok(true)
}

#[tauri::command]
pub async fn open_tool_window(app: AppHandle, tool_name: String) -> Result<(), String> {
    info!("Opening tool window: {}", tool_name);

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
        "performance" | "host_performance" => {
            toolbar::create_tool_window_internal(&app, "performance");
        }
        _ => return Err(format!("Unknown tool name: {}", tool_name)),
    }
    Ok(())
}

/// Speech phrase response with optional ETag.
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
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Strip a UTF-8 BOM because serde_json does not accept it.
    let body_text = body_text.trim_start_matches('\u{FEFF}');

    let phrases: Vec<String> = serde_json::from_str(body_text).map_err(|e| {
        let preview = utf8_prefix(body_text, 200);
        format!("Failed to parse JSON: {}; first 200 chars: {}", e, preview)
    })?;

    info!("Loaded {} speech phrases", phrases.len());
    Ok(Some(SpeechPhrasesResponse { phrases, etag }))
}

/// Remote asset response with optional ETag.
#[derive(Serialize)]
pub struct RemoteBytesResponse {
    pub data_url: String,
    pub etag: Option<String>,
}

/// Fetch a remote asset through Rust to avoid WebView CORS limits.
/// Returns None when the remote responds 304 Not Modified.
#[tauri::command]
pub async fn fetch_remote_bytes(
    url: String,
    if_none_match: Option<String>,
) -> Result<Option<RemoteBytesResponse>, String> {
    // Allowlist the asset CDN only.
    if !is_allowed_asset_url(&url) {
        return Err(format!("Proxying this URL is not allowed: {}", url));
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
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);

    info!("Proxied remote asset successfully ({} bytes)", bytes.len());
    Ok(Some(RemoteBytesResponse { data_url, etag }))
}

// ===== AI API proxy (CORS bypass) =====

/// HTTP client for AI API proxy requests.
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

/// Forward HTTP requests to external AI services.
#[tauri::command]
pub async fn ai_api_proxy(request: AiProxyRequest) -> Result<String, String> {
    let client = ai_client();

    let mut req_builder = match request.method.to_uppercase().as_str() {
        "GET" => client.get(&request.url),
        "POST" => client.post(&request.url),
        _ => return Err(format!("Unsupported HTTP method: {}", request.method)),
    };

    // Forward request headers.
    for (key, value) in &request.headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    // Forward request body.
    if let Some(body) = &request.body {
        req_builder = req_builder.header("Content-Type", "application/json");
        req_builder = req_builder.body(body.clone());
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("AI API request failed: {}", e))?;

    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read AI API response: {}", e))?;

    if status >= 400 {
        return Err(format!("{} - {}", status, utf8_prefix(&body, 500)));
    }

    Ok(body)
}

// ===== Desktop screenshot for pet vision =====

static SCREENSHOT_CAPTURE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct ScreenshotCaptureGuard;

impl ScreenshotCaptureGuard {
    fn try_acquire() -> Result<Self, String> {
        SCREENSHOT_CAPTURE_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "A screenshot capture is already in progress".to_string())
    }
}

impl Drop for ScreenshotCaptureGuard {
    fn drop(&mut self) {
        SCREENSHOT_CAPTURE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

fn scaled_image_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let max_dimension = max_dimension.max(1);
    let longest = width.max(height);
    if longest <= max_dimension || longest == 0 {
        return (width, height);
    }

    let scale = max_dimension as f64 / longest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

/// Capture the primary display as a base64 JPEG.
#[tauri::command]
pub async fn capture_screenshot(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    use std::io::Cursor;
    use xcap::Monitor;

    if window.label() != "toolbar" {
        return Err("Screenshot capture is only available to the desktop pet toolbar".to_string());
    }

    // xcap uses a synchronous API, so capture on a blocking thread.
    tokio::task::spawn_blocking(|| {
        let _capture_guard = ScreenshotCaptureGuard::try_acquire()?;

        let mut monitors =
            Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
        if monitors.is_empty() {
            return Ok(None);
        }
        let mut primary_index = None;
        for (index, monitor) in monitors.iter().enumerate() {
            if monitor
                .is_primary()
                .map_err(|e| format!("Failed to identify primary monitor: {}", e))?
            {
                primary_index = Some(index);
                break;
            }
        }
        let primary_index = primary_index.ok_or_else(|| "No primary monitor found".to_string())?;
        let monitor = monitors.swap_remove(primary_index);

        let image = monitor
            .capture_image()
            .map_err(|e| format!("Screenshot failed: {}", e))?;

        // Limit the longest edge to reduce token usage for both landscape and
        // portrait displays.
        let (w, h) = (image.width(), image.height());
        let (target_w, target_h) = scaled_image_dimensions(w, h, 1024);
        let resized = if (target_w, target_h) != (w, h) {
            image::imageops::resize(
                &image,
                target_w,
                target_h,
                image::imageops::FilterType::Triangle,
            )
        } else {
            image
        };

        // JPEG does not support alpha.
        let rgb_image = image::DynamicImage::ImageRgba8(resized).to_rgb8();

        // Encode as JPEG at quality 60.
        let mut buf = Cursor::new(Vec::new());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 60);
        rgb_image
            .write_with_encoder(encoder)
            .map_err(|e| format!("JPEG encoding failed: {}", e))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
        Ok(Some(format!("data:image/jpeg;base64,{}", b64)))
    })
    .await
    .map_err(|e| format!("Screenshot task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::{is_allowed_asset_url, scaled_image_dimensions, utf8_prefix};

    #[test]
    fn utf8_prefix_stops_at_a_character_boundary() {
        let value = "abc桌面观察";
        assert_eq!(utf8_prefix(value, 4), "abc");
        assert_eq!(utf8_prefix(value, 6), "abc桌");
        assert_eq!(utf8_prefix(value, value.len()), value);
    }

    #[test]
    fn screenshot_scaling_limits_the_longest_edge() {
        assert_eq!(scaled_image_dimensions(1920, 1080, 1024), (1024, 576));
        assert_eq!(scaled_image_dimensions(1080, 1920, 1024), (576, 1024));
        assert_eq!(scaled_image_dimensions(800, 600, 1024), (800, 600));
    }

    #[test]
    fn remote_asset_url_requires_the_exact_https_cdn_host() {
        assert!(is_allowed_asset_url(
            "https://assets.alkaidlab.com/toolbar-spritesheet.webp?t=1"
        ));
        assert!(is_allowed_asset_url(
            "https://assets.alkaidlab.com:443/speech-phrases.json"
        ));
        assert!(!is_allowed_asset_url(
            "https://assets.alkaidlab.com.example.com/toolbar-spritesheet.webp"
        ));
        assert!(!is_allowed_asset_url(
            "http://assets.alkaidlab.com/toolbar-spritesheet.webp"
        ));
        assert!(!is_allowed_asset_url(
            "https://assets.alkaidlab.com:444/toolbar-spritesheet.webp"
        ));
        assert!(!is_allowed_asset_url(
            "https://user:secret@assets.alkaidlab.com/toolbar-spritesheet.webp"
        ));
    }
}
