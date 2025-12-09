use axum::{
    extract::Request,
    response::{IntoResponse, Response},
    Router,
};
use tower_http::cors::CorsLayer;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU16, Ordering};
use log::{info, warn, error, debug};

/// 全局 Sunshine 目标 URL（动态配置）
static SUNSHINE_TARGET: Lazy<Arc<RwLock<String>>> = 
    Lazy::new(|| Arc::new(RwLock::new(String::from("https://localhost:47990"))));

/// 快速失败机制：记录 Sunshine 是否可用
static SUNSHINE_AVAILABLE: AtomicBool = AtomicBool::new(true);
static LAST_CHECK_TIME: AtomicU64 = AtomicU64::new(0);

/// 代理服务器实际使用的端口
static PROXY_PORT: AtomicU16 = AtomicU16::new(48081);

/// 快速失败冷却时间（秒）- 在此时间内不重试，超过后会重新尝试连接
const FAST_FAIL_COOLDOWN_SECS: u64 = 3;

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

/// 设置 Sunshine 目标 URL
pub fn set_sunshine_target(url: String) {
    if let Ok(mut target) = SUNSHINE_TARGET.write() {
        info!("🎯 代理目标已更新: {}", url);
        *target = url;
    }
}

/// 注入到 Sunshine 页面的 CSS 样式（编译时从文件读取）
const INJECT_STYLES: &str = include_str!("../inject-styles.css");

/// 注入的 JavaScript 脚本（编译时从文件读取）
const INJECT_SCRIPT: &str = include_str!("../inject-script.js");

/// 调皮的404页面（当Sunshine未启动时显示，编译时从文件读取）
const ERROR_404_PAGE: &str = include_str!("../error-404.html");

/// 启动本地代理服务器
pub async fn start_proxy_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .fallback(proxy_handler)
        .layer(CorsLayer::permissive());
    
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
            error!("❌ 代理服务器绑定端口失败: 端口 {}-{} 均被占用", PROXY_PORT_START, PROXY_PORT_END);
            return Err(format!("无法绑定端口 {}-{}", PROXY_PORT_START, PROXY_PORT_END).into());
        }
    };
    
    // 保存实际使用的端口
    PROXY_PORT.store(bound_port, Ordering::Relaxed);
    info!("🚀 Sunshine 代理服务器已启动: http://127.0.0.1:{}", bound_port);
    info!("   开始监听请求...");
    
    axum::serve(listener, app).await.map_err(|e| {
        error!("❌ 代理服务器运行失败: {}", e);
        e.into()
    })
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

/// 检查是否是连接错误
fn is_connection_error(error: &str) -> bool {
    const CONNECTION_ERROR_PATTERNS: &[&str] = &[
        "connection", "refused", "timed out", "timeout",
        "unreachable", "error sending request", "network", "dns"
    ];
    let error_lower = error.to_lowercase();
    CONNECTION_ERROR_PATTERNS.iter().any(|p| error_lower.contains(p))
}

/// 检查是否是 API 请求
#[inline]
fn is_api_request(path: &str) -> bool {
    path.starts_with("/api/")
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
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i+1..i+3]).unwrap_or(""),
                16
            ) {
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
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            r#"{"success":false,"error":"Sunshine service is unavailable"}"#
        ).into_response()
    } else {
        // 页面请求返回 HTML 错误页面
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            ERROR_404_PAGE
        ).into_response()
    }
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
    
    // 获取请求体
    let body = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(e) => {
            error!("❌ 读取请求体失败: {}", e);
            return (axum::http::StatusCode::BAD_REQUEST, "读取请求体失败").into_response();
        }
    };
    
    // 构建目标 URL
    let sunshine_base = SUNSHINE_TARGET.read()
        .map(|url| url.clone())
        .unwrap_or_else(|_| "https://localhost:47990".to_string());
    
    let target_url = if query.is_empty() {
        format!("{}{}", sunshine_base, path)
    } else {
        format!("{}{}?{}", sunshine_base, path, query)
    };
    
    #[cfg(debug_assertions)]
    if path == "/" || path.ends_with(".html") || path.starts_with("/api/") {
        debug!("📡 代理请求: {} {}", method, path);
    }
    
    // 快速失败检查：在冷却时间内直接返回错误，避免大量无效请求
    if should_fast_fail() {
        return service_unavailable_response(is_api);
    }
    
    // 请求 Sunshine
    match fetch_and_proxy(&target_url, &method, &headers, body).await {
        Ok(response) => {
            mark_available();
            response
        }
        Err(e) => {
            let error_str = e.to_string();
            error!("❌ 代理错误 [{}]: {}", path, error_str);
            
            if is_connection_error(&error_str) {
                mark_unavailable();
                service_unavailable_response(is_api)
            } else {
                if is_api {
                    (
                        axum::http::StatusCode::BAD_GATEWAY,
                        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                        format!(r#"{{"success":false,"error":"Proxy error: {}"}}"#, e)
                    ).into_response()
                } else {
                    (axum::http::StatusCode::BAD_GATEWAY, format!("代理错误: {}", e)).into_response()
                }
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
    let body = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(e) => {
            error!("❌ 读取请求体失败: {}", e);
            return (axum::http::StatusCode::BAD_REQUEST, "读取请求体失败").into_response();
        }
    };
    
    // 构建目标 URL
    let target_url = if path.starts_with("/steam-store/") {
        let api_path = path.strip_prefix("/steam-store").unwrap_or(path);
        let params = if query.is_empty() { "l=schinese&cc=CN" } else { query };
        format!("https://store.steampowered.com{}?{}", api_path, params)
    } else if path.starts_with("/steamgriddb/") {
        let api_path = path.strip_prefix("/steamgriddb").unwrap_or(path);
        format!("https://www.steamgriddb.com/api/v2{}?{}", api_path, query)
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            r#"{"success":false,"error":"Unknown Steam API path"}"#
        ).into_response();
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
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                format!(r#"{{"success":false,"error":"Steam API request failed: {}"}}"#, e)
            ).into_response()
        }
    }
}

/// 构建带 CORS 头的响应
async fn build_cors_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let resp_headers = response.headers().clone();
    
    match response.bytes().await {
        Ok(body_bytes) => {
            let mut builder = axum::http::Response::builder().status(status.as_u16());
            
            // 复制响应头（排除 CORS 和 transfer-encoding）
            for (key, value) in resp_headers.iter() {
                let key_str = key.as_str().to_lowercase();
                if !key_str.starts_with("access-control-") && key_str != "transfer-encoding" {
                    builder = builder.header(key.as_str(), value);
                }
            }
            
            // 添加 CORS 头部
            builder
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
                .header("Access-Control-Allow-Headers", "*")
                .body(axum::body::Body::from(body_bytes.to_vec()))
                .unwrap_or_else(|_| {
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "构建响应失败").into_response()
                })
        }
        Err(e) => {
            error!("❌ 读取响应失败: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                format!(r#"{{"success":false,"error":"Failed to read response: {}"}}"#, e)
            ).into_response()
        }
    }
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
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                r#"{"success":false,"error":"Missing or invalid URL parameter"}"#
            ).into_response();
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
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .map(|host| allowed_domains.iter().any(|d| host == *d || host.ends_with(&format!(".{}", d))))
        .unwrap_or(false);
    
    if !is_allowed {
        warn!("⚠️ 外部代理请求被拒绝（域名不在白名单）: {}", target_url);
        return (
            axum::http::StatusCode::FORBIDDEN,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            r#"{"success":false,"error":"Domain not allowed"}"#
        ).into_response();
    }
    
    debug!("🌐 外部代理请求: {}", target_url);
    
    // 获取请求体
    let body = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(e) => {
            error!("❌ 读取请求体失败: {}", e);
            return (axum::http::StatusCode::BAD_REQUEST, "读取请求体失败").into_response();
        }
    };
    
    // 发送请求
    let client = get_http_client();
    match send_request(client, &target_url, method, headers, &body).await {
        Ok(response) => {
            let status = response.status();
            let resp_headers = response.headers().clone();
            
            match response.bytes().await {
                Ok(body) => {
                    let mut builder = axum::http::Response::builder()
                        .status(status.as_u16());
                    
                    // 复制响应头（排除 CORS 相关头部，我们会添加自己的）
                    for (key, value) in resp_headers.iter() {
                        let key_str = key.as_str().to_lowercase();
                        if !key_str.starts_with("access-control-") 
                            && key_str != "transfer-encoding" 
                        {
                            builder = builder.header(key.as_str(), value);
                        }
                    }
                    
                    // 添加 CORS 头部
                    builder = builder
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
                        .header("Access-Control-Allow-Headers", "*");
                    
                    builder.body(axum::body::Body::from(body.to_vec()))
                        .unwrap_or_else(|_| {
                            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "构建响应失败").into_response()
                        })
                }
                Err(e) => {
                    error!("❌ 读取外部响应失败: {}", e);
                    (
                        axum::http::StatusCode::BAD_GATEWAY,
                        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                        format!(r#"{{"success":false,"error":"Failed to read response: {}"}}"#, e)
                    ).into_response()
                }
            }
        }
        Err(e) => {
            error!("❌ 外部代理请求失败: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                format!(r#"{{"success":false,"error":"External request failed: {}"}}"#, e)
            ).into_response()
        }
    }
}

/// 创建共享的 HTTP 客户端（连接复用，性能优化）
fn get_http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .pool_max_idle_per_host(20)
            .timeout(std::time::Duration::from_secs(5))
            .connect_timeout(std::time::Duration::from_millis(500))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// 发送 HTTP 请求的辅助函数
async fn send_request(
    client: &reqwest::Client,
    url: &str,
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    body: &[u8]
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
        if !matches!(key_str, "host" | "connection" | "content-length" | "transfer-encoding") {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(key_str, value_str);
            }
        }
    }
    
    if !body.is_empty() {
        req_builder = req_builder.body(body.to_vec());
    }
    
    req_builder.send().await
}

/// 获取并代理内容
async fn fetch_and_proxy(
    url: &str, 
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    body: Vec<u8>
) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    let client = get_http_client();
    
    // 尝试请求，HTTPS 失败时降级到 HTTP（仅限非连接错误）
    let response = match send_request(client, url, method, headers, &body).await {
        Ok(resp) => resp,
        Err(e) if url.starts_with("https://") && !is_connection_error(&e.to_string()) => {
            let http_url = url.replace("https://", "http://");
            warn!("⚠️  HTTPS 连接失败，尝试 HTTP: {}", http_url);
            send_request(client, &http_url, method, headers, &body).await?
        }
        Err(e) => return Err(e.into()),
    };
    
    let status = response.status();
    let resp_headers = response.headers().clone();
    let content_type = resp_headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html");
    
    let body_bytes = response.bytes().await?.to_vec();
    
    // 判断是否需要注入脚本
    let needs_injection = should_inject_script(url, content_type);
    let final_body = if needs_injection {
        inject_if_needed(body_bytes)
    } else {
        body_bytes
    };
    
    // 构建响应
    let mut res = axum::http::Response::builder().status(status.as_u16());
    
    for (key, value) in resp_headers.iter() {
        let key_str = key.as_str().to_lowercase();
        // 排除内容长度、传输编码、内容编码，以及需要注入时排除缓存相关头部
        if matches!(key_str.as_str(), "content-length" | "transfer-encoding" | "content-encoding") {
            continue;
        }
        if needs_injection && matches!(key_str.as_str(), "cache-control" | "etag" | "last-modified" | "expires") {
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
    
    Ok(res.body(axum::body::Body::from(final_body))?)
}

/// 判断是否应该注入脚本
fn should_inject_script(url: &str, content_type: &str) -> bool {
    if !content_type.contains("text/html") {
        return false;
    }
    
    let path = url.rsplit('/').next().unwrap_or("");
    matches!(path, "" | "apps" | "config" | "password" | "pin" | "troubleshooting" | "welcome")
        || url.ends_with(".html")
        || url.ends_with(".htm")
}

/// 如果需要则注入脚本
fn inject_if_needed(body: Vec<u8>) -> Vec<u8> {
    match String::from_utf8(body) {
        Ok(html) if !html.contains("主题同步脚本已加载") 
            && (html.contains("<html") || html.contains("<!DOCTYPE")) => {
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
