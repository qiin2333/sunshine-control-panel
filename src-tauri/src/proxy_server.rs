use axum::{
    Router,
    body::Body,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tower_http::cors::CorsLayer;

/// 全局 Sunshine 目标 URL（动态配置）
static SUNSHINE_TARGET: Lazy<Arc<RwLock<String>>> =
    Lazy::new(|| Arc::new(RwLock::new(String::from("https://localhost:47990"))));

#[cfg(test)]
static TEST_REFRESH_TARGET: Lazy<std::sync::Mutex<Option<String>>> =
    Lazy::new(|| std::sync::Mutex::new(None));

/// 快速失败机制：记录 Sunshine 是否可用
static SUNSHINE_AVAILABLE: AtomicBool = AtomicBool::new(true);
static LAST_CHECK_TIME: AtomicU64 = AtomicU64::new(0);

/// 代理服务器实际使用的端口
static PROXY_PORT: AtomicU16 = AtomicU16::new(48081);

/// The page must not navigate to the local proxy until the listener is bound.
/// On a cold GUI start the main window and the async proxy task otherwise race.
static PROXY_READY: AtomicBool = AtomicBool::new(false);
static PROXY_START_REQUESTED: AtomicBool = AtomicBool::new(false);
static PROXY_READY_NOTIFY: Lazy<Notify> = Lazy::new(Notify::new);
static PROXY_REQUEST_PERMITS: Lazy<Arc<Semaphore>> =
    Lazy::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PROXY_REQUESTS)));
static PROXY_PENDING_PERMITS: Lazy<Arc<Semaphore>> =
    Lazy::new(|| Arc::new(Semaphore::new(MAX_PENDING_PROXY_REQUESTS)));

/// 快速失败冷却时间（秒）- 在此时间内不重试，超过后会重新尝试连接
const FAST_FAIL_COOLDOWN_SECS: u64 = 3;

/// 默认代理超时：用于 Sunshine 普通页面/API，保持界面快速失败。
const DEFAULT_PROXY_TIMEOUT_SECS: u64 = 5;

/// AI 请求会携带日志、应用列表等上下文，模型首 token 可能明显慢于普通 API。
const AI_PROXY_TIMEOUT_SECS: u64 = 120;

/// 本机 Sunshine 连接应快速建立；连接超时不跟随 AI 响应超时放大。
const PROXY_CONNECT_TIMEOUT_MS: u64 = 500;

/// Bound memory and active work exposed by the unauthenticated loopback proxy.
const MAX_PROXY_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROXY_RESPONSE_BODY_BYTES: usize = 64 * 1024 * 1024;
const MAX_INJECTABLE_HTML_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONCURRENT_PROXY_REQUESTS: usize = 16;
const MAX_PENDING_PROXY_REQUESTS: usize = 16;
const PROXY_ADMISSION_WAIT_SECS: u64 = 3;
const PROXY_REQUEST_BODY_TIMEOUT_SECS: u64 = 15;
const PROXY_STREAM_IDLE_TIMEOUT_SECS: u64 = 60;
const PROXY_STREAM_CHUNK_BYTES: usize = 64 * 1024;

struct UnboundedStreamRule {
    method: &'static str,
    path: &'static str,
    bounded_when_header_present: Option<&'static str>,
}

/// Explicit allowlist for responses that are valid unbounded streams.
///
/// Rules must use an exact method and path. Optional headers can select a
/// bounded mode on the same endpoint, as X-Log-Offset does for live log tails.
const UNBOUNDED_STREAM_RULES: &[UnboundedStreamRule] = &[UnboundedStreamRule {
    method: "GET",
    path: "/api/logs",
    bounded_when_header_present: Some("x-log-offset"),
}];

/// 代理服务器端口范围
const PROXY_PORT_START: u16 = 48081;
const PROXY_PORT_END: u16 = 48090;

/// 获取代理服务器实际使用的端口
pub fn get_proxy_port() -> u16 {
    PROXY_PORT.load(Ordering::Relaxed)
}

/// 获取代理服务器的完整 URL
pub fn get_proxy_url() -> String {
    format!("http://127.0.0.1:{}", get_proxy_port())
}

/// Tauri command: 获取代理服务器 URL
#[tauri::command]
pub fn get_proxy_url_command() -> String {
    get_proxy_url()
}

/// Wait until the local proxy listener is ready before loading the main frame.
#[tauri::command]
pub async fn wait_for_proxy_ready() -> Result<String, String> {
    ensure_started();
    if PROXY_READY.load(Ordering::Acquire) {
        return Ok(get_proxy_url());
    }

    let notified = PROXY_READY_NOTIFY.notified();
    if PROXY_READY.load(Ordering::Acquire) {
        return Ok(get_proxy_url());
    }

    tokio::time::timeout(std::time::Duration::from_secs(15), notified)
        .await
        .map_err(|_| "Timed out waiting for the local Sunshine proxy".to_string())?;

    if PROXY_READY.load(Ordering::Acquire) {
        Ok(get_proxy_url())
    } else {
        Err("The local Sunshine proxy stopped during startup".to_string())
    }
}

/// Start the local Web UI proxy on first use. Hidden tray-only startup does
/// not need the router, listener, or proxy HTTP clients resident in memory.
pub fn ensure_started() {
    if PROXY_READY.load(Ordering::Acquire) || PROXY_START_REQUESTED.swap(true, Ordering::AcqRel) {
        return;
    }

    tauri::async_runtime::spawn(async {
        if let Ok(dev_target) = std::env::var("WEBUI_DEV_TARGET") {
            info!("🛠️ [开发模式] 检测到 WEBUI_DEV_TARGET 环境变量");
            info!("🎯 代理目标: {}", dev_target);
            set_sunshine_target(dev_target);
        } else {
            match crate::sunshine::get_sunshine_url().await {
                Ok(url) => {
                    info!("🎯 Sunshine URL: {}", url);
                    set_sunshine_target(url.trim_end_matches('/').to_string());
                }
                Err(e) => warn!("⚠️ 无法获取 Sunshine URL，使用默认: {}", e),
            }
        }

        if let Err(e) = start_proxy_server().await {
            error!("❌ 代理服务器启动失败: {}", e);
        }
        PROXY_START_REQUESTED.store(false, Ordering::Release);
    });
}

/// 设置 Sunshine 目标 URL
pub fn set_sunshine_target(url: String) {
    if let Ok(mut target) = SUNSHINE_TARGET.write() {
        info!("🎯 代理目标已更新: {}", url);
        *target = url;
    }
}

/// Dynamic Sunshine target helpers.
fn get_sunshine_target() -> String {
    SUNSHINE_TARGET
        .read()
        .map(|url| url.clone())
        .unwrap_or_else(|_| "https://localhost:47990".to_string())
}

async fn refresh_sunshine_target_internal() -> Result<String, String> {
    #[cfg(test)]
    if let Some(base_url) = TEST_REFRESH_TARGET.lock().unwrap().clone() {
        set_sunshine_target(base_url.clone());
        return Ok(base_url);
    }

    let url = crate::sunshine::get_sunshine_url().await?;
    let base_url = url.trim_end_matches('/').to_string();
    set_sunshine_target(base_url.clone());
    Ok(base_url)
}

/// Refresh the proxy target from the current Sunshine config.
#[tauri::command]
pub async fn refresh_sunshine_target() -> Result<String, String> {
    let base_url = refresh_sunshine_target_internal().await?;
    reset_fast_fail();
    Ok(base_url)
}

/// 注入到 Sunshine 页面的 CSS 样式（编译时从文件读取）
const INJECT_STYLES: &str = include_str!("../inject-styles.css");

/// 注入的 JavaScript 脚本（编译时从文件读取）
const INJECT_SCRIPT: &str = include_str!("../inject-script.js");

/// 调皮的404页面（当Sunshine未启动时显示，编译时从文件读取）
const ERROR_404_PAGE: &str = include_str!("../error-404.html");

/// Private Network Access (PNA) Middleware
/// 根据 Microsoft Edge 143+ 的要求添加 PNA 支持头部
async fn pna_middleware(req: Request, next: Next) -> Response {
    // 预定义常用的 header 值
    const PNA_HEADER: &str = "Access-Control-Allow-Private-Network";
    const PNA_VALUE: axum::http::HeaderValue = axum::http::HeaderValue::from_static("true");

    // 检查是否是 OPTIONS 预检请求（CORS）
    if req.method() == axum::http::Method::OPTIONS {
        // 处理 CORS 预检请求，添加 PNA 支持
        return Response::builder()
            .status(axum::http::StatusCode::NO_CONTENT)
            .header("Access-Control-Allow-Origin", "*")
            .header(
                "Access-Control-Allow-Methods",
                "GET, POST, PUT, DELETE, OPTIONS, PATCH, HEAD",
            )
            .header("Access-Control-Allow-Headers", "*")
            .header(PNA_HEADER, "true")
            .header("Access-Control-Max-Age", "86400")
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // 对于非 OPTIONS 请求，执行原有的处理器，然后在响应中添加 PNA 头部
    let mut response = next.run(req).await;
    response.headers_mut().insert(PNA_HEADER, PNA_VALUE);
    response
}

fn hold_proxy_permit(response: Response, permit: OwnedSemaphorePermit) -> Response {
    hold_proxy_permit_with_timeout(
        response,
        permit,
        std::time::Duration::from_secs(PROXY_STREAM_IDLE_TIMEOUT_SECS),
    )
}

fn hold_proxy_permit_with_timeout(
    response: Response,
    permit: OwnedSemaphorePermit,
    idle_timeout: std::time::Duration,
) -> Response {
    let (parts, body) = response.into_parts();
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(2);
    tokio::spawn(async move {
        let _permit = permit;
        let mut source = body.into_data_stream();

        loop {
            let item = tokio::select! {
                _ = sender.closed() => break,
                result = tokio::time::timeout(idle_timeout, source.next()) => {
                    match result {
                        Ok(item) => item,
                        Err(_) => break,
                    }
                },
            };
            let Some(item) = item else {
                break;
            };
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(error) => {
                    let _ = tokio::select! {
                        _ = sender.closed() => false,
                        result = tokio::time::timeout(
                            idle_timeout,
                            sender.send(Err(std::io::Error::other(error))),
                        ) => result.is_ok_and(|sent| sent.is_ok()),
                    };
                    break;
                }
            };

            let mut offset = 0;
            while offset < chunk.len() {
                let end = offset
                    .saturating_add(PROXY_STREAM_CHUNK_BYTES)
                    .min(chunk.len());
                let part = chunk.slice(offset..end);
                let sent = tokio::select! {
                    _ = sender.closed() => break,
                    result = tokio::time::timeout(idle_timeout, sender.send(Ok(part))) => {
                        result.is_ok_and(|sent| sent.is_ok())
                    },
                };
                if !sent {
                    return;
                }
                offset = end;
            }
        }
    });
    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|item| (item, receiver))
    });
    Response::from_parts(parts, Body::from_stream(stream))
}

async fn proxy_concurrency_middleware(req: Request, next: Next) -> Response {
    let permit = acquire_proxy_permit_with_wait(
        PROXY_REQUEST_PERMITS.clone(),
        PROXY_PENDING_PERMITS.clone(),
        std::time::Duration::from_secs(PROXY_ADMISSION_WAIT_SECS),
    )
    .await;
    let Some(permit) = permit else {
        return proxy_limit_response(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Too many concurrent proxy requests",
        );
    };
    hold_proxy_permit(next.run(req).await, permit)
}

async fn acquire_proxy_permit_with_wait(
    active: Arc<Semaphore>,
    pending: Arc<Semaphore>,
    wait_timeout: std::time::Duration,
) -> Option<OwnedSemaphorePermit> {
    if let Ok(permit) = active.clone().try_acquire_owned() {
        return Some(permit);
    }

    let _pending_permit = pending.try_acquire_owned().ok()?;
    tokio::time::timeout(wait_timeout, active.acquire_owned())
        .await
        .ok()?
        .ok()
}

/// 启动本地代理服务器
pub async fn start_proxy_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    PROXY_READY.store(false, Ordering::Release);

    let app = Router::new()
        .fallback(proxy_handler)
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(pna_middleware))
        .layer(axum::middleware::from_fn(proxy_concurrency_middleware));

    // 尝试在端口范围内找到可用端口
    let mut listener = None;
    let mut bound_port = PROXY_PORT_START;

    for port in PROXY_PORT_START..=PROXY_PORT_END {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => {
                info!("✅ 代理服务器成功绑定到 http://{}", addr);
                bound_port = port;
                listener = Some(l);
                break;
            }
            Err(e) => {
                if port == PROXY_PORT_START {
                    warn!("⚠️  端口 {} 被占用，尝试其他端口...", port);
                }
                debug!("   端口 {} 不可用: {}", port, e);
            }
        }
    }

    let listener = match listener {
        Some(l) => l,
        None => {
            error!(
                "❌ 代理服务器绑定端口失败: 端口 {}-{} 均被占用",
                PROXY_PORT_START, PROXY_PORT_END
            );
            return Err(format!("无法绑定端口 {}-{}", PROXY_PORT_START, PROXY_PORT_END).into());
        }
    };

    // 保存实际使用的端口
    PROXY_PORT.store(bound_port, Ordering::Relaxed);
    PROXY_READY.store(true, Ordering::Release);
    PROXY_READY_NOTIFY.notify_waiters();
    info!(
        "🚀 Sunshine 代理服务器已启动: http://127.0.0.1:{}",
        bound_port
    );
    info!("   开始监听请求...");

    let result = axum::serve(listener, app).await.map_err(|e| {
        error!("❌ 代理服务器运行失败: {}", e);
        e.into()
    });
    PROXY_READY.store(false, Ordering::Release);
    result
}

/// 获取当前时间戳（秒）
#[inline]
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 检查是否应该快速失败（在冷却时间内跳过请求）
/// 返回 true 表示应该快速失败，false 表示应该尝试请求
#[inline]
fn should_fast_fail() -> bool {
    // 如果 Sunshine 标记为可用，不需要快速失败
    if SUNSHINE_AVAILABLE.load(Ordering::Relaxed) {
        return false;
    }

    // Sunshine 标记为不可用，检查是否已过冷却时间
    let last_check = LAST_CHECK_TIME.load(Ordering::Relaxed);
    let elapsed = current_timestamp().saturating_sub(last_check);

    if elapsed >= FAST_FAIL_COOLDOWN_SECS {
        // 冷却时间已过，允许重试（重置状态为可用，让请求尝试连接）
        debug!("⏰ 快速失败冷却时间已过 ({}秒)，允许重试", elapsed);
        mark_available();
        false
    } else {
        // 仍在冷却时间内，快速失败
        true
    }
}

/// 标记 Sunshine 为不可用
#[inline]
fn mark_unavailable() {
    SUNSHINE_AVAILABLE.store(false, Ordering::Relaxed);
    LAST_CHECK_TIME.store(current_timestamp(), Ordering::Relaxed);
}

/// 标记 Sunshine 为可用
#[inline]
fn mark_available() {
    SUNSHINE_AVAILABLE.store(true, Ordering::Relaxed);
}

/// 重置代理快速失败状态（窗口恢复时调用，确保首次请求不被拦截）
pub fn reset_fast_fail() {
    mark_available();
}

/// 检查是否是连接错误
fn is_connection_error(error: &str) -> bool {
    const CONNECTION_ERROR_PATTERNS: &[&str] = &[
        "connection",
        "refused",
        "timed out",
        "timeout",
        "unreachable",
        "error sending request",
        "network",
        "dns",
    ];
    let error_lower = error.to_lowercase();
    CONNECTION_ERROR_PATTERNS
        .iter()
        .any(|p| error_lower.contains(p))
}

fn is_timeout_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    error_lower.contains("timed out") || error_lower.contains("timeout")
}

fn proxy_error_kind(error: &str) -> &'static str {
    if error.contains("Connection refused") || error.contains("connection refused") {
        "连接被拒绝（后端未启动？）"
    } else if is_timeout_error(error) {
        "连接超时"
    } else if error.contains("certificate") || error.contains("ssl") || error.contains("tls") {
        "TLS/证书错误"
    } else if is_connection_error(error) {
        "连接失败"
    } else {
        "请求失败"
    }
}

/// 检查是否是 API 请求
#[inline]
fn is_api_request(path: &str) -> bool {
    path.starts_with("/api/")
}

/// 检查是否是 AI API 请求。
///
/// AI 配置和对话通常会连续触发多个请求；如果前一个普通 API 请求把
/// Sunshine 标记为不可用，不能让共享 AI 入口在冷却窗口内被直接短路。
#[inline]
fn is_ai_api_request(path: &str) -> bool {
    path.starts_with("/api/ai/")
}

/// 检查是否是外部代理请求
#[inline]
fn is_external_proxy_request(path: &str) -> bool {
    path.starts_with("/_proxy/")
}

/// 检查是否是 Steam API 请求
#[inline]
fn is_steam_api_request(path: &str) -> bool {
    path.starts_with("/steam-store/") || path.starts_with("/steamgriddb/")
}

/// 解析外部代理 URL
fn parse_external_proxy_url(path: &str, query: &str) -> Option<String> {
    use url::form_urlencoded;

    // 路径格式: /_proxy/{encoded_url}
    // 或者: /_proxy/?url={encoded_url}
    if let Some(encoded_url) = path.strip_prefix("/_proxy/") {
        if !encoded_url.is_empty() {
            // URL 编码在路径中，使用 percent_decode 解码
            return percent_decode_str(encoded_url);
        }
    }

    // 检查查询参数
    if !query.is_empty() {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == "url" {
                return Some(value.into_owned());
            }
        }
    }

    None
}

/// 解码 URL 编码的字符串
fn percent_decode_str(s: &str) -> Option<String> {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(result).ok()
}

/// 创建服务不可用响应（根据请求类型返回不同格式）
fn service_unavailable_response(is_api: bool) -> Response {
    if is_api {
        // API 请求返回 JSON 格式错误
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            r#"{"success":false,"error":"Sunshine service is unavailable"}"#,
        )
            .into_response()
    } else {
        // 页面请求返回 HTML 错误页面
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            ERROR_404_PAGE,
        )
            .into_response()
    }
}

fn proxy_error_response(
    is_api: bool,
    status: axum::http::StatusCode,
    message: impl Into<String>,
    detail: Option<&str>,
) -> Response {
    let message = message.into();
    if is_api {
        let body = match detail {
            Some(detail) => serde_json::json!({
                "success": false,
                "error": message,
                "detail": detail,
            }),
            None => serde_json::json!({
                "success": false,
                "error": message,
            }),
        };
        (
            status,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            body.to_string(),
        )
            .into_response()
    } else {
        (status, message).into_response()
    }
}

fn ai_proxy_error_response(error_kind: &str, error: &str) -> Response {
    let is_timeout = is_timeout_error(error);
    let status = if is_timeout {
        axum::http::StatusCode::GATEWAY_TIMEOUT
    } else {
        axum::http::StatusCode::BAD_GATEWAY
    };
    let message = if is_timeout {
        "AI 请求超时，请稍后重试或减少日志上下文".to_string()
    } else {
        format!("AI 请求代理失败：{}", error_kind)
    };

    proxy_error_response(true, status, message, Some(error))
}

fn connection_failure_response(
    is_ai_api: bool,
    is_api: bool,
    error_kind: &str,
    error: &str,
) -> Response {
    if is_ai_api {
        ai_proxy_error_response(error_kind, error)
    } else {
        mark_unavailable();
        service_unavailable_response(is_api)
    }
}

fn proxy_limit_response(status: axum::http::StatusCode, message: &'static str) -> Response {
    let mut response = (status, message).into_response();
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("access-control-allow-private-network"),
        axum::http::HeaderValue::from_static("true"),
    );
    response
}

async fn read_proxy_request_body(req: Request) -> Result<Bytes, Response> {
    read_proxy_request_body_with_timeout(
        req,
        std::time::Duration::from_secs(PROXY_REQUEST_BODY_TIMEOUT_SECS),
    )
    .await
}

async fn read_proxy_request_body_with_timeout(
    req: Request,
    timeout: std::time::Duration,
) -> Result<Bytes, Response> {
    match tokio::time::timeout(
        timeout,
        axum::body::to_bytes(req.into_body(), MAX_PROXY_REQUEST_BODY_BYTES),
    )
    .await
    {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Err(proxy_limit_response(
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            "Proxy request body exceeds 16 MiB",
        )),
        Err(_) => Err(proxy_limit_response(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            "Proxy request body timed out",
        )),
    }
}

fn limited_response_body(response: reqwest::Response, max_bytes: usize) -> Result<Body, String> {
    if response
        .content_length()
        .is_some_and(|size| size > max_bytes as u64)
    {
        return Err(format!("Proxy response body exceeds {max_bytes} bytes"));
    }

    let stream = response
        .bytes_stream()
        .scan((0usize, false), move |(received, stopped), item| {
            let next = if *stopped {
                None
            } else {
                match item {
                    Ok(chunk) => {
                        if (*received).saturating_add(chunk.len()) > max_bytes {
                            *stopped = true;
                            Some(Err(std::io::Error::other(format!(
                                "Proxy response body exceeds {max_bytes} bytes"
                            ))))
                        } else {
                            *received += chunk.len();
                            Some(Ok(chunk))
                        }
                    }
                    Err(error) => {
                        *stopped = true;
                        Some(Err(std::io::Error::other(error)))
                    }
                }
            };
            futures_util::future::ready(next)
        });
    Ok(Body::from_stream(stream))
}

fn streamed_response_body(
    response: reqwest::Response,
    max_bytes: Option<usize>,
) -> Result<Body, String> {
    match max_bytes {
        Some(max_bytes) => limited_response_body(response, max_bytes),
        None => Ok(Body::from_stream(
            response
                .bytes_stream()
                .map(|item| item.map_err(std::io::Error::other)),
        )),
    }
}

fn proxy_response_body_limit(
    url: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
) -> Option<usize> {
    let path = url::Url::parse(url).ok().map(|url| url.path().to_string());
    let is_allowlisted_stream = path.as_deref().is_some_and(|path| {
        UNBOUNDED_STREAM_RULES.iter().any(|rule| {
            method.as_str() == rule.method
                && path == rule.path
                && !rule
                    .bounded_when_header_present
                    .is_some_and(|header| headers.contains_key(header))
        })
    });
    (!is_allowlisted_stream).then_some(MAX_PROXY_RESPONSE_BODY_BYTES)
}

async fn read_limited_response_bytes(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|size| size > max_bytes as u64)
    {
        return Err(format!("Proxy response body exceeds {max_bytes} bytes"));
    }

    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(max_bytes as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Failed to read proxy response: {error}"))?
    {
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("Proxy response body exceeds {max_bytes} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// 代理处理器
async fn proxy_handler(req: Request) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    let query = uri.query().unwrap_or("").to_string();
    let headers = req.headers().clone();

    // 检查是否是外部代理请求（用于绕过 CORS）
    if is_external_proxy_request(&path) {
        return handle_external_proxy(&path, &query, &method, &headers, req).await;
    }

    // 检查是否是 Steam API 请求（需要特殊处理）
    if is_steam_api_request(&path) {
        return handle_steam_api(&path, &query, &method, &headers, req).await;
    }

    // 判断是否是 API 请求
    let is_api = is_api_request(&path);
    let is_ai_api = is_ai_api_request(&path);

    // 获取请求体
    let body = match read_proxy_request_body(req).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    // 构建目标 URL
    let sunshine_base = get_sunshine_target();

    let target_url = if query.is_empty() {
        format!("{}{}", sunshine_base, path)
    } else {
        format!("{}{}?{}", sunshine_base, path, query)
    };

    #[cfg(debug_assertions)]
    if path == "/" || path.ends_with(".html") || path.starts_with("/api/") {
        debug!("📡 代理请求: {} {}", method, path);
    }

    // 快速失败检查：在冷却时间内直接返回错误，避免大量无效请求。
    // AI 入口除外：它是用户显式触发的共享代理能力，需要真实尝试一次，
    // 否则会因为旧的全局不可用状态误报 "Sunshine service is unavailable"。
    if !is_ai_api && should_fast_fail() {
        return service_unavailable_response(is_api);
    }

    // 请求 Sunshine
    match fetch_and_proxy(&target_url, &method, &headers, &body, is_ai_api).await {
        Ok(response) => {
            mark_available();
            if method == axum::http::Method::POST
                && path == "/api/restart"
                && response.status().is_success()
            {
                tauri::async_runtime::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    if let Err(e) = refresh_sunshine_target_internal().await {
                        warn!(
                            "Failed to refresh Sunshine target after restart request: {}",
                            e
                        );
                    }
                });
            }
            let status = response.status();
            if status.is_client_error() || status.is_server_error() {
                warn!("⚠️ 代理响应异常 [{}]: HTTP {}", path, status.as_u16());
            }
            response
        }
        Err(e) => {
            let error_str = e.to_string();
            let error_kind = proxy_error_kind(&error_str);
            error!("❌ 代理错误 [{}] ({}): {}", path, error_kind, error_str);

            if is_connection_error(&error_str) {
                match refresh_sunshine_target_internal().await {
                    Ok(refreshed_base) if refreshed_base != sunshine_base => {
                        let refreshed_url = if query.is_empty() {
                            format!("{}{}", refreshed_base, path)
                        } else {
                            format!("{}{}?{}", refreshed_base, path, query)
                        };

                        match fetch_and_proxy(&refreshed_url, &method, &headers, &body, is_ai_api)
                            .await
                        {
                            Ok(response) => {
                                mark_available();
                                response
                            }
                            Err(retry_err) => {
                                let retry_error = retry_err.to_string();
                                let retry_kind = proxy_error_kind(&retry_error);
                                error!(
                                    "Proxy retry failed [{}] ({}): {}",
                                    path, retry_kind, retry_error
                                );
                                connection_failure_response(
                                    is_ai_api,
                                    is_api,
                                    retry_kind,
                                    &retry_error,
                                )
                            }
                        }
                    }
                    Ok(_) => connection_failure_response(is_ai_api, is_api, error_kind, &error_str),
                    Err(refresh_err) => {
                        warn!(
                            "Failed to refresh Sunshine target after proxy error: {}",
                            refresh_err
                        );
                        connection_failure_response(is_ai_api, is_api, error_kind, &error_str)
                    }
                }
            } else {
                proxy_error_response(
                    is_api,
                    axum::http::StatusCode::BAD_GATEWAY,
                    format!("代理错误：{}", error_kind),
                    Some(&error_str),
                )
            }
        }
    }
}

/// 处理 Steam API 请求（直接转发到 Steam API）
async fn handle_steam_api(
    path: &str,
    query: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    req: Request,
) -> Response {
    // 获取请求体
    let body = match read_proxy_request_body(req).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    // 构建目标 URL
    let target_url = if path.starts_with("/steam-store/") {
        let api_path = path.strip_prefix("/steam-store").unwrap_or(path);
        let params = if query.is_empty() {
            "l=schinese&cc=CN"
        } else {
            query
        };
        format!("https://store.steampowered.com{}?{}", api_path, params)
    } else if path.starts_with("/steamgriddb/") {
        let api_path = path.strip_prefix("/steamgriddb").unwrap_or(path);
        format!("https://www.steamgriddb.com/api/v2{}?{}", api_path, query)
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            r#"{"success":false,"error":"Unknown Steam API path"}"#,
        )
            .into_response();
    };

    debug!("🎮 Steam API 代理请求: {} -> {}", path, target_url);

    // 发送请求并构建响应
    let client = get_http_client();
    match send_request(client, &target_url, method, headers, &body).await {
        Ok(response) => build_cors_response(response).await,
        Err(e) => {
            error!("❌ Steam API 请求失败: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                format!(
                    r#"{{"success":false,"error":"Steam API request failed: {}"}}"#,
                    e
                ),
            )
                .into_response()
        }
    }
}

/// 构建带 CORS 头的响应
async fn build_cors_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let resp_headers = response.headers().clone();
    let body = match limited_response_body(response, MAX_PROXY_RESPONSE_BODY_BYTES) {
        Ok(body) => body,
        Err(error) => {
            warn!("拒绝过大的代理响应: {}", error);
            return proxy_limit_response(
                axum::http::StatusCode::BAD_GATEWAY,
                "Proxy response body exceeds 64 MiB",
            );
        }
    };
    let mut builder = axum::http::Response::builder().status(status.as_u16());

    // 复制响应头（排除 CORS、长度与传输编码）
    for (key, value) in resp_headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if !key_str.starts_with("access-control-") && key_str != "transfer-encoding" {
            builder = builder.header(key.as_str(), value);
        }
    }

    builder
        .header("Access-Control-Allow-Origin", "*")
        .header(
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        )
        .header("Access-Control-Allow-Headers", "*")
        .body(body)
        .unwrap_or_else(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "构建响应失败",
            )
                .into_response()
        })
}

/// 处理外部代理请求（绕过 CORS 限制）
async fn handle_external_proxy(
    path: &str,
    query: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    req: Request,
) -> Response {
    // 解析目标 URL
    let target_url = match parse_external_proxy_url(path, query) {
        Some(url) => url,
        None => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                r#"{"success":false,"error":"Missing or invalid URL parameter"}"#,
            )
                .into_response();
        }
    };

    // 安全检查：只允许 HTTPS 请求到白名单域名
    let allowed_domains = [
        "github.io",
        "raw.githubusercontent.com",
        "github.com",
        "api.github.com",
    ];

    let is_allowed = url::Url::parse(&target_url)
        .ok()
        .map(|u| {
            u.scheme() == "https"
                && u.host_str()
                    .map(|host| {
                        allowed_domains
                            .iter()
                            .any(|d| host == *d || host.ends_with(&format!(".{}", d)))
                    })
                    .unwrap_or(false)
        })
        .unwrap_or(false);

    if !is_allowed {
        warn!("⚠️ 外部代理请求被拒绝（域名不在白名单）: {}", target_url);
        return (
            axum::http::StatusCode::FORBIDDEN,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            r#"{"success":false,"error":"Domain not allowed"}"#,
        )
            .into_response();
    }

    debug!("🌐 外部代理请求: {}", target_url);

    // 获取请求体
    let body = match read_proxy_request_body(req).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    // 发送请求
    let client = get_http_client();
    match send_request(client, &target_url, method, headers, &body).await {
        Ok(response) => build_cors_response(response).await,
        Err(e) => {
            error!("❌ 外部代理请求失败: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                format!(
                    r#"{{"success":false,"error":"External request failed: {}"}}"#,
                    e
                ),
            )
                .into_response()
        }
    }
}

/// 创建共享的 HTTP 客户端（连接复用，性能优化）
fn get_http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_http_client(DEFAULT_PROXY_TIMEOUT_SECS))
}

/// AI 共享入口使用更长响应超时，避免日志诊断/对话被普通代理的 5 秒超时截断。
fn get_ai_http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_http_client(AI_PROXY_TIMEOUT_SECS))
}

fn get_streaming_http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .pool_max_idle_per_host(3)
            .connect_timeout(std::time::Duration::from_millis(PROXY_CONNECT_TIMEOUT_MS))
            .read_timeout(std::time::Duration::from_secs(
                PROXY_STREAM_IDLE_TIMEOUT_SECS,
            ))
            .build()
            .expect("Failed to create streaming HTTP client")
    })
}

fn build_http_client(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .pool_max_idle_per_host(20)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .connect_timeout(std::time::Duration::from_millis(PROXY_CONNECT_TIMEOUT_MS))
        .build()
        .expect("Failed to create HTTP client")
}

/// 发送 HTTP 请求的辅助函数
async fn send_request(
    client: &reqwest::Client,
    url: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    body: &Bytes,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut req_builder = match method.as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => client.get(url),
    };

    // 复制请求头（排除特殊头部）
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        if !matches!(
            key_str,
            "host" | "connection" | "content-length" | "transfer-encoding"
        ) {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(key_str, value_str);
            }
        }
    }

    if !body.is_empty() {
        req_builder = req_builder.body(body.clone());
    }

    req_builder.send().await
}

/// 获取并代理内容
async fn fetch_and_proxy(
    url: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    body: &Bytes,
    is_ai_api: bool,
) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    let response_body_limit = proxy_response_body_limit(url, method, headers);
    let client = if response_body_limit.is_none() {
        get_streaming_http_client()
    } else if is_ai_api {
        get_ai_http_client()
    } else {
        get_http_client()
    };

    // 尝试请求，HTTPS 失败时降级到 HTTP（仅限非连接错误）
    let response = match send_request(client, url, method, headers, body).await {
        Ok(resp) => resp,
        Err(e) if url.starts_with("https://") && !is_connection_error(&e.to_string()) => {
            let http_url = url.replace("https://", "http://");
            warn!("⚠️  HTTPS 连接失败，尝试 HTTP: {}", http_url);
            send_request(client, &http_url, method, headers, body).await?
        }
        Err(e) => return Err(e.into()),
    };

    let status = response.status();
    let resp_headers = response.headers().clone();
    let content_type = resp_headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html");

    // 判断是否需要注入脚本
    let needs_injection = should_inject_script(url, content_type);
    let final_body = if needs_injection {
        let body_bytes = read_limited_response_bytes(response, MAX_INJECTABLE_HTML_BYTES)
            .await
            .map_err(std::io::Error::other)?;
        Body::from(inject_if_needed(body_bytes))
    } else {
        streamed_response_body(response, response_body_limit).map_err(std::io::Error::other)?
    };

    // 构建响应
    let mut res = axum::http::Response::builder().status(status.as_u16());

    for (key, value) in resp_headers.iter() {
        let key_str = key.as_str().to_lowercase();
        // 排除内容长度、传输编码、内容编码，以及需要注入时排除缓存相关头部
        if matches!(
            key_str.as_str(),
            "content-length" | "transfer-encoding" | "content-encoding"
        ) {
            continue;
        }
        if needs_injection
            && matches!(
                key_str.as_str(),
                "cache-control" | "etag" | "last-modified" | "expires"
            )
        {
            continue;
        }
        res = res.header(key, value);
    }

    // 需要注入脚本的页面添加无缓存头部
    if needs_injection {
        res = res.header("Cache-Control", "no-cache, no-store, must-revalidate");
        res = res.header("Pragma", "no-cache");
        res = res.header("Expires", "0");
    }

    Ok(res.body(final_body)?)
}

/// 判断是否应该注入脚本
fn should_inject_script(url: &str, content_type: &str) -> bool {
    if !content_type.contains("text/html") {
        return false;
    }

    let path = url.rsplit('/').next().unwrap_or("");
    matches!(
        path,
        "" | "apps" | "config" | "password" | "pin" | "troubleshooting" | "welcome"
    ) || url.ends_with(".html")
        || url.ends_with(".htm")
}

/// 如果需要则注入脚本
fn inject_if_needed(body: Vec<u8>) -> Vec<u8> {
    match String::from_utf8(body) {
        Ok(html)
            if !html.contains("主题同步脚本已加载")
                && (html.contains("<html") || html.contains("<!DOCTYPE")) =>
        {
            inject_theme_script(html).into_bytes()
        }
        Ok(html) => html.into_bytes(),
        Err(e) => e.into_bytes(),
    }
}

/// 注入主题同步脚本到 HTML
fn inject_theme_script(html: String) -> String {
    let Some(pos) = html.find("</head>") else {
        return html;
    };

    // 根据编译配置决定是否是生产环境
    let is_production = cfg!(not(debug_assertions));
    let production_flag = if is_production {
        "window.TAURI_PRODUCTION = true;"
    } else {
        "window.TAURI_PRODUCTION = false;"
    };

    let inject_size = INJECT_STYLES.len() + INJECT_SCRIPT.len() + production_flag.len() + 150;
    let mut result = String::with_capacity(html.len() + inject_size);

    result.push_str(&html[..pos]);
    result.push_str("\n<!-- Tauri 样式优化 -->\n<style id=\"tauri-scrollbar-theme\">\n");
    result.push_str(INJECT_STYLES);
    result.push_str("\n</style>\n<!-- Tauri 功能脚本 -->\n<script>\n");
    result.push_str(production_flag);
    result.push_str("\n");
    result.push_str(INJECT_SCRIPT);
    result.push_str("\n</script>\n");
    result.push_str(&html[pos..]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    static TEST_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

    async fn unused_local_port() -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    }

    async fn spawn_one_shot_http_server(body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request_buf = [0_u8; 1024];
            let _ = stream.read(&mut request_buf).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{}", addr)
    }

    async fn spawn_one_shot_unknown_length_server(body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request_buf = [0_u8; 1024];
            let _ = stream.read(&mut request_buf).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        });

        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn proxy_rejects_oversized_request_body() {
        let request = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/config")
            .body(Body::from(vec![0_u8; MAX_PROXY_REQUEST_BODY_BYTES + 1]))
            .unwrap();

        let response = read_proxy_request_body(request).await.unwrap_err();
        assert_eq!(response.status(), axum::http::StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.headers()["Access-Control-Allow-Origin"], "*");
        assert_eq!(
            response.headers()["Access-Control-Allow-Private-Network"],
            "true"
        );
    }

    #[tokio::test]
    async fn proxy_permit_lives_until_response_body_is_dropped() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let body =
            Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
        let response = hold_proxy_permit_with_timeout(
            Response::new(body),
            permit,
            std::time::Duration::from_secs(1),
        );

        assert_eq!(semaphore.available_permits(), 0);
        drop(response);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while semaphore.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[tokio::test]
    async fn proxy_admission_waits_for_a_short_normal_burst() {
        let active = Arc::new(Semaphore::new(1));
        let pending = Arc::new(Semaphore::new(1));
        let held = active.clone().acquire_owned().await.unwrap();
        let release = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            drop(held);
        });

        let permit = acquire_proxy_permit_with_wait(
            active.clone(),
            pending.clone(),
            std::time::Duration::from_secs(1),
        )
        .await
        .expect("request should enter after the active slot is released");
        release.await.unwrap();

        assert_eq!(pending.available_permits(), 1);
        drop(permit);
        assert_eq!(active.available_permits(), 1);
    }

    #[tokio::test]
    async fn proxy_admission_rejects_when_the_wait_queue_is_full() {
        let active = Arc::new(Semaphore::new(1));
        let pending = Arc::new(Semaphore::new(1));
        let _active_permit = active.clone().acquire_owned().await.unwrap();
        let _pending_permit = pending.clone().acquire_owned().await.unwrap();

        let permit =
            acquire_proxy_permit_with_wait(active, pending, std::time::Duration::from_secs(1))
                .await;
        assert!(permit.is_none());
    }

    #[tokio::test]
    async fn proxy_admission_timeout_releases_the_wait_slot() {
        let active = Arc::new(Semaphore::new(1));
        let pending = Arc::new(Semaphore::new(1));
        let _active_permit = active.clone().acquire_owned().await.unwrap();

        let permit = acquire_proxy_permit_with_wait(
            active,
            pending.clone(),
            std::time::Duration::from_millis(10),
        )
        .await;
        assert!(permit.is_none());
        assert_eq!(pending.available_permits(), 1);
    }

    #[tokio::test]
    async fn proxy_releases_permit_after_response_idle_timeout() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let body =
            Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
        let response = hold_proxy_permit_with_timeout(
            Response::new(body),
            permit,
            std::time::Duration::from_millis(10),
        );

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while semaphore.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(semaphore.available_permits(), 1);
        drop(response);
    }

    #[tokio::test]
    async fn proxy_releases_permit_when_source_errors_with_full_queue() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let source = futures_util::stream::iter(vec![
            Ok(Bytes::from(vec![0_u8; PROXY_STREAM_CHUNK_BYTES * 2])),
            Err(std::io::Error::other("upstream failed")),
        ]);
        let response = hold_proxy_permit_with_timeout(
            Response::new(Body::from_stream(source)),
            permit,
            std::time::Duration::from_millis(10),
        );

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while semaphore.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(semaphore.available_permits(), 1);
        drop(response);
    }

    #[tokio::test]
    async fn proxy_times_out_a_stalled_request_body() {
        let body =
            Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
        let request = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/config")
            .body(body)
            .unwrap();

        let response =
            read_proxy_request_body_with_timeout(request, std::time::Duration::from_millis(10))
                .await
                .unwrap_err();
        assert_eq!(response.status(), axum::http::StatusCode::REQUEST_TIMEOUT);
    }

    #[tokio::test]
    async fn proxy_rejects_oversized_buffered_response() {
        let url = spawn_one_shot_unknown_length_server("response-too-large").await;
        let response = reqwest::get(url).await.unwrap();

        let error = read_limited_response_bytes(response, 8).await.unwrap_err();
        assert!(error.contains("exceeds 8 bytes"));
    }

    #[tokio::test]
    async fn proxy_stops_oversized_streamed_response() {
        let url = spawn_one_shot_unknown_length_server("response-too-large").await;
        let response = reqwest::get(url).await.unwrap();
        let body = limited_response_body(response, 8).unwrap();

        assert!(axum::body::to_bytes(body, usize::MAX).await.is_err());
    }

    #[tokio::test]
    async fn full_log_download_streams_without_a_total_size_limit() {
        let url = spawn_one_shot_unknown_length_server("response-larger-than-test-limit").await;
        let response = reqwest::get(url).await.unwrap();
        let body = streamed_response_body(response, None).unwrap();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        assert_eq!(bytes.as_ref(), b"response-larger-than-test-limit");
    }

    #[test]
    fn only_full_log_download_bypasses_the_response_limit() {
        let empty_headers = axum::http::HeaderMap::new();
        assert_eq!(
            proxy_response_body_limit(
                "https://127.0.0.1:47990/api/logs",
                &axum::http::Method::GET,
                &empty_headers,
            ),
            None
        );

        let mut incremental_headers = axum::http::HeaderMap::new();
        incremental_headers.insert("X-Log-Offset", axum::http::HeaderValue::from_static("0"));
        assert_eq!(
            proxy_response_body_limit(
                "https://127.0.0.1:47990/api/logs",
                &axum::http::Method::GET,
                &incremental_headers,
            ),
            Some(MAX_PROXY_RESPONSE_BODY_BYTES)
        );
        assert_eq!(
            proxy_response_body_limit(
                "https://127.0.0.1:47990/api/config",
                &axum::http::Method::GET,
                &empty_headers,
            ),
            Some(MAX_PROXY_RESPONSE_BODY_BYTES)
        );
        for (method, url) in [
            (axum::http::Method::POST, "https://127.0.0.1:47990/api/logs"),
            (axum::http::Method::HEAD, "https://127.0.0.1:47990/api/logs"),
            (axum::http::Method::GET, "https://127.0.0.1:47990/api/logs/"),
        ] {
            assert_eq!(
                proxy_response_body_limit(url, &method, &empty_headers),
                Some(MAX_PROXY_RESPONSE_BODY_BYTES)
            );
        }
    }

    #[tokio::test]
    async fn proxy_retries_with_refreshed_target_when_port_changes() {
        let _guard = TEST_LOCK.lock().await;
        let marker = "proxy-refresh-target-ok";
        let old_port = unused_local_port().await;
        let new_target = spawn_one_shot_http_server(marker).await;

        set_sunshine_target(format!("http://127.0.0.1:{}", old_port));
        *TEST_REFRESH_TARGET.lock().unwrap() = Some(new_target);

        let request = axum::http::Request::builder()
            .method(axum::http::Method::GET)
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = proxy_handler(request).await;
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);

        *TEST_REFRESH_TARGET.lock().unwrap() = None;

        assert!(status.is_success(), "unexpected status: {status}");
        assert!(
            text.contains(marker),
            "proxy did not retry against refreshed target: {text}"
        );
    }

    #[tokio::test]
    async fn ai_api_bypasses_fast_fail_cache() {
        let _guard = TEST_LOCK.lock().await;
        let marker = "ai-fast-fail-bypass-ok";
        let new_target = spawn_one_shot_http_server(marker).await;

        set_sunshine_target(new_target);
        mark_unavailable();

        let request = axum::http::Request::builder()
            .method(axum::http::Method::GET)
            .uri("/api/ai/config")
            .body(Body::empty())
            .unwrap();

        let response = proxy_handler(request).await;
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);

        reset_fast_fail();

        assert!(
            status.is_success(),
            "unexpected status: {status}, body: {text}"
        );
        assert!(
            text.contains(marker),
            "AI API request was short-circuited instead of proxied: {text}"
        );
    }

    #[tokio::test]
    async fn ai_api_connection_failure_does_not_mark_sunshine_unavailable() {
        let _guard = TEST_LOCK.lock().await;
        let old_port = unused_local_port().await;
        let target = format!("http://127.0.0.1:{}", old_port);

        set_sunshine_target(target.clone());
        *TEST_REFRESH_TARGET.lock().unwrap() = Some(target);
        mark_available();

        let request = axum::http::Request::builder()
            .method(axum::http::Method::GET)
            .uri("/api/ai/config")
            .body(Body::empty())
            .unwrap();

        let response = proxy_handler(request).await;
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);

        *TEST_REFRESH_TARGET.lock().unwrap() = None;
        reset_fast_fail();

        assert_eq!(
            status,
            axum::http::StatusCode::BAD_GATEWAY,
            "unexpected status/body: {text}"
        );
        assert!(
            !text.contains("Sunshine service is unavailable"),
            "AI proxy failure should not reuse Sunshine unavailable wording: {text}"
        );
        assert!(
            SUNSHINE_AVAILABLE.load(std::sync::atomic::Ordering::Relaxed),
            "AI proxy failure should not poison the Sunshine fast-fail cache"
        );
    }
}
