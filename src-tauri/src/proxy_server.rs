use axum::{
    extract::Request,
    response::{IntoResponse, Response},
    Router,
};
use tower_http::cors::CorsLayer;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 全局 Sunshine 目标 URL（动态配置）
static SUNSHINE_TARGET: Lazy<Arc<RwLock<String>>> = 
    Lazy::new(|| Arc::new(RwLock::new(String::from("https://localhost:47990"))));

/// 快速失败机制：记录 Sunshine 是否可用
static SUNSHINE_AVAILABLE: AtomicBool = AtomicBool::new(true);
static LAST_CHECK_TIME: AtomicU64 = AtomicU64::new(0);

/// 设置 Sunshine 目标 URL
pub fn set_sunshine_target(url: String) {
    if let Ok(mut target) = SUNSHINE_TARGET.write() {
        println!("🎯 代理目标已更新: {}", url);
        *target = url;
    }
}

/// 注入到 Sunshine 页面的 CSS 样式
const INJECT_STYLES: &str = r#"
<!-- Tauri 样式优化 -->
<style id="tauri-scrollbar-theme">
/* 完全隐藏滚动条 */
::-webkit-scrollbar {
  width: 0;
  height: 0;
  display: none;
}

/* Firefox */
* {
  scrollbar-width: none;
}

/* IE/Edge */
body {
  -ms-overflow-style: none;
}
body {
  padding-top: 72px;
}
.navbar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1000;
    margin-bottom: 72px;
}
.navbar-brand {
  margin-left: -48px;
  visibility: hidden;
}
#bd-theme {
  display: none;
}
</style>
"#;

/// 注入的 JavaScript 脚本（编译时从文件读取）
const INJECT_SCRIPT: &str = include_str!("../inject-script.js");

/// 调皮的404页面（当Sunshine未启动时显示，编译时从文件读取）
const ERROR_404_PAGE: &str = include_str!("../error-404.html");

/// 启动本地代理服务器
pub async fn start_proxy_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .fallback(proxy_handler)
        .layer(CorsLayer::permissive());
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 48081));
    println!("🚀 准备启动 Sunshine 代理服务器: http://{}", addr);
    
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            println!("✅ 代理服务器成功绑定到 http://{}", addr);
            println!("   开始监听请求...");
            
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("❌ 代理服务器运行失败: {}", e);
                return Err(e.into());
            }
            
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ 代理服务器绑定端口失败: {}", e);
            eprintln!("   端口 48081 可能被占用或权限不足");
            Err(e.into())
        }
    }
}

/// 代理处理器
async fn proxy_handler(req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let headers = req.headers().clone();
    
    // 获取请求体（消耗 req）
    let body = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(e) => {
            eprintln!("❌ 读取请求体失败: {}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                "读取请求体失败"
            ).into_response();
        }
    };
    
    // 构建目标 URL（从动态配置读取）
    let sunshine_base = SUNSHINE_TARGET.read()
        .map(|url| url.clone())
        .unwrap_or_else(|_| "https://localhost:47990".to_string());
    
    let target_url = if query.is_empty() {
        format!("{}{}", &sunshine_base, &path)
    } else {
        format!("{}{}?{}", &sunshine_base, &path, &query)
    };
    
    // 只在调试模式下打印主要请求
    #[cfg(debug_assertions)]
    if path == "/" || path.ends_with(".html") || path.starts_with("/api/") {
        println!("📡 代理请求: {} {}", method, &path);
    }
    
    // 快速失败检查：如果最近3秒内检测到 Sunshine 不可用，直接返回 404
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let last_check = LAST_CHECK_TIME.load(Ordering::Relaxed);
    
    if !SUNSHINE_AVAILABLE.load(Ordering::Relaxed) && (now - last_check) < 3 {
        // 3秒内检测过不可用，直接返回 404
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            ERROR_404_PAGE
        ).into_response();
    }
    
    // 请求 Sunshine
    match fetch_and_proxy(&target_url, &method, &headers, body).await {
        Ok(response) => {
            // 连接成功，标记为可用
            SUNSHINE_AVAILABLE.store(true, Ordering::Relaxed);
            response
        }
        Err(e) => {
            eprintln!("❌ 代理错误 [{}]: {}", path, e);
            eprintln!("   目标 URL: {}", target_url);
            eprintln!("   错误详情: {:?}", e);
            
            // 检查是否是连接错误
            let error_str = e.to_string().to_lowercase();
            let is_connection_error = error_str.contains("connection") 
                || error_str.contains("refused")
                || error_str.contains("timed out")
                || error_str.contains("timeout")
                || error_str.contains("unreachable")
                || error_str.contains("error sending request")
                || error_str.contains("network")
                || error_str.contains("dns");
            
            if is_connection_error {
                // 标记为不可用，记录检查时间
                SUNSHINE_AVAILABLE.store(false, Ordering::Relaxed);
                LAST_CHECK_TIME.store(now, Ordering::Relaxed);
                
                // 返回调皮的404页面
                (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    ERROR_404_PAGE
                ).into_response()
            } else {
                // 其他错误返回简单的错误信息
                let error_msg = format!("代理错误: {}", e);
            (
                axum::http::StatusCode::BAD_GATEWAY,
                error_msg
            ).into_response()
            }
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
            .pool_max_idle_per_host(20)  // 增加连接池
            .timeout(std::time::Duration::from_secs(5))  // 总超时
            .connect_timeout(std::time::Duration::from_millis(500))  // 500ms 快速检测
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
) -> Result<reqwest::Response, Box<dyn std::error::Error + Send + Sync>> {
    // 构建请求
    let mut req_builder = match method.as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => client.get(url),  // 默认使用 GET
    };
    
    // 复制请求头（排除一些特殊头部）
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        if !matches!(key_str, "host" | "connection" | "content-length" | "transfer-encoding") {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(key_str, value_str);
            }
        }
    }
    
    // 如果有请求体，添加它
    if !body.is_empty() {
        req_builder = req_builder.body(body.to_vec());
    }
    
    Ok(req_builder.send().await?)
}

/// 获取并代理内容
async fn fetch_and_proxy(
    url: &str, 
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    body: Vec<u8>
) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    let client = get_http_client();
    
    // 尝试 HTTPS，失败则降级到 HTTP
    let url_to_use = url.to_string();
    let response_result = send_request(client, &url_to_use, method, headers, &body).await;
    
    let response = match response_result {
        Ok(resp) => resp,
        Err(e) if url_to_use.starts_with("https://") => {
            let error_str = e.to_string().to_lowercase();
            // 如果是连接拒绝错误，直接返回，不要再尝试 HTTP
            if error_str.contains("refused") || error_str.contains("connection") {
                return Err(e);
            }
            // 其他 HTTPS 错误（如证书问题），尝试 HTTP
            let http_url = url_to_use.replace("https://", "http://");
            eprintln!("⚠️  HTTPS 连接失败，尝试 HTTP: {}", http_url);
            send_request(client, &http_url, method, headers, &body).await?
        }
        Err(e) => return Err(e),
    };
    let status = response.status();
    let headers = response.headers().clone();
    
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html");
    
    let body = response.bytes().await?.to_vec();
    
    // 只对主 HTML 页面注入脚本
    // 排除 API 响应、JSON、以及已经包含脚本的页面
    let is_main_page = matches!(
        url.rsplit('/').next().unwrap_or(""),
        "" | "apps" | "config" | "password" | "pin" | "troubleshooting" | "welcome"
    ) || url.ends_with(".html") || url.ends_with(".htm")
        && content_type.contains("text/html");
    
    let final_body = if is_main_page {
        match String::from_utf8(body.clone()) {
            Ok(html) => {
                // 检查是否已经包含主题脚本（避免重复注入）
                if html.contains("主题同步脚本已加载") {
                    body  // 已注入，直接返回原始数据
                } else if html.contains("<html") || html.contains("<!DOCTYPE") {
                    // 只在完整的 HTML 文档中注入
                    let modified = inject_theme_script(html);
                    modified.into_bytes()
                } else {
                    body  // 不是完整 HTML，返回原始数据
                }
            }
            Err(_) => body  // 无效 UTF-8，返回原始数据
        }
    } else {
        body
    };
    
    // 构建响应
    let mut res = axum::http::Response::builder()
        .status(status.as_u16());
    
    // 复制头部（排除一些可能导致问题的头部）
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        if !matches!(key_str, "content-length" | "transfer-encoding" | "content-encoding") {
            res = res.header(key, value);
        }
    }
    
    Ok(res.body(axum::body::Body::from(final_body))?)
}

/// 注入主题同步脚本到 HTML（优化版 - 减少字符串操作）
fn inject_theme_script(html: String) -> String {
    // 快速检查：如果没有 </head> 标签，直接返回
    if let Some(pos) = html.find("</head>") {
        let inject_content_size = INJECT_STYLES.len() + INJECT_SCRIPT.len() + 100;
        let mut result = String::with_capacity(html.len() + inject_content_size);
        
        result.push_str(&html[..pos]);
        
        // 注入 CSS 样式
        result.push_str("\n<!-- Tauri 样式优化 -->\n");
        result.push_str(INJECT_STYLES);
        
        // 注入 JavaScript 脚本
        result.push_str("\n<!-- Tauri 功能脚本 -->\n<script>\n");
        result.push_str(INJECT_SCRIPT);
        result.push_str("\n</script>\n");
        
        result.push_str(&html[pos..]);
        result
    } else {
        html  // 没有 </head>，不注入
    }
}

