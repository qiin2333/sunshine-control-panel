use log::{debug, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

// ========== 常量 ==========
const GITHUB_API_RELEASES: &str =
    "https://api.github.com/repos/MrCreativ3001/moonlight-web-stream/releases";
const DEFAULT_BIND_PORT: u16 = 47790;
const SERVER_BINARY_NAME: &str = "web-server.exe";
const PROCESS_CHECK_NAME: &str = "web-server.exe";

// ========== 状态管理 ==========

/// 子进程句柄（由控制面板启动时持有）
static CHILD_PROCESS: Lazy<Mutex<Option<std::process::Child>>> = Lazy::new(|| Mutex::new(None));

/// 当前配置的绑定端口
static BIND_PORT: Lazy<Mutex<u16>> = Lazy::new(|| Mutex::new(DEFAULT_BIND_PORT));

// ========== 数据结构 ==========

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonlightWebStatus {
    /// 是否已安装（binary 存在）
    pub installed: bool,
    /// 是否正在运行
    pub running: bool,
    /// 安装路径
    pub install_path: String,
    /// 版本号
    pub version: String,
    /// Web UI 访问地址
    pub access_url: String,
    /// 绑定端口
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonlightWebConfig {
    #[serde(default)]
    pub web_server: WebServerConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webrtc: Option<WebRtcConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_settings: Option<serde_json::Value>,
    // 保留上游新增的未知字段
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<CertificateConfig>,
    #[serde(default)]
    pub url_path_prefix: String,
    #[serde(default = "default_true")]
    pub session_cookie_secure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_cookie_expiration: Option<SessionExpiration>,
    #[serde(default = "default_true")]
    pub first_login_create_admin: bool,
    #[serde(default = "default_true")]
    pub first_login_assign_global_hosts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_user_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forwarded_header: Option<serde_json::Value>,
    // 保留未知字段
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            certificate: None,
            url_path_prefix: String::new(),
            session_cookie_secure: false,
            session_cookie_expiration: None,
            first_login_create_admin: true,
            first_login_assign_global_hosts: true,
            default_user_id: None,
            forwarded_header: None,
            extra: std::collections::HashMap::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionExpiration {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CertificateConfig {
    pub private_key_pem: String,
    pub certificate_pem: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebRtcConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ice_servers: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range: Option<PortRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nat_1to1: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_types: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortRange {
    pub min: u16,
    pub max: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonlightWebRelease {
    pub version: String,
    pub download_url: Option<String>,
    pub download_name: Option<String>,
    pub release_page: String,
    pub release_notes: String,
}

fn default_bind_address() -> String {
    format!("0.0.0.0:{}", DEFAULT_BIND_PORT)
}

// ========== 路径工具 ==========

/// 获取 moonlight-web 安装目录（%LOCALAPPDATA%\Sunshine\moonlight-web）
fn get_install_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Sunshine")
            .join("moonlight-web");
    }
    // fallback: 与控制面板同级
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join("moonlight-web");
        }
    }
    PathBuf::from("./moonlight-web")
}

/// 获取 web-server 可执行文件路径
fn get_server_binary_path() -> PathBuf {
    get_install_dir().join(SERVER_BINARY_NAME)
}

/// 获取配置文件路径
fn get_config_path() -> PathBuf {
    get_install_dir().join("server").join("config.json")
}

// ========== Tauri 命令 ==========

/// 获取 moonlight-web 状态
#[tauri::command]
pub async fn moonlight_web_get_status() -> Result<MoonlightWebStatus, String> {
    let install_dir = get_install_dir();
    let binary_path = get_server_binary_path();
    let installed = binary_path.exists();
    let running = is_process_running();
    let port = *BIND_PORT.lock().unwrap();

    let version = if installed {
        get_installed_version().unwrap_or_else(|| "unknown".to_string())
    } else {
        String::new()
    };

    // 根据配置判断协议并生成访问 URL
    let access_url = if running {
        let scheme = if has_https_certificate() {
            "https"
        } else {
            "http"
        };
        format!("{}://localhost:{}", scheme, port)
    } else {
        String::new()
    };

    Ok(MoonlightWebStatus {
        installed,
        running,
        install_path: install_dir.to_string_lossy().to_string(),
        version,
        access_url,
        port,
    })
}

/// 启动 moonlight-web 服务
#[tauri::command]
pub async fn moonlight_web_start() -> Result<String, String> {
    let binary_path = get_server_binary_path();
    if !binary_path.exists() {
        return Err("moonlight-web 未安装，请先下载安装".to_string());
    }

    // 检查是否已在运行
    if is_process_running() {
        return Ok("moonlight-web 已在运行".to_string());
    }

    // 确保配置存在
    ensure_config_exists().await?;

    info!("🌐 启动 moonlight-web 服务: {:?}", binary_path);

    let install_dir = get_install_dir();

    #[cfg(target_os = "windows")]
    let child = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(&binary_path)
            .current_dir(&install_dir)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("启动 moonlight-web 失败: {}", e))?
    };

    #[cfg(not(target_os = "windows"))]
    let child = {
        std::process::Command::new(&binary_path)
            .current_dir(&install_dir)
            .spawn()
            .map_err(|e| format!("启动 moonlight-web 失败: {}", e))?
    };

    let pid = child.id();
    *CHILD_PROCESS.lock().unwrap() = Some(child);

    info!("✅ moonlight-web 已启动, PID: {}", pid);
    Ok(format!("moonlight-web 已启动 (PID: {})", pid))
}

/// 停止 moonlight-web 服务
#[tauri::command]
pub async fn moonlight_web_stop() -> Result<String, String> {
    // 先尝试停止我们自己启动的子进程
    {
        let mut guard = CHILD_PROCESS.lock().unwrap();
        if let Some(ref mut child) = *guard {
            info!("🛑 停止 moonlight-web 子进程 (PID: {})", child.id());
            let _ = child.kill();
            let _ = child.wait();
            *guard = None;
            info!("✅ moonlight-web 子进程已停止");
            return Ok("moonlight-web 已停止".to_string());
        }
    }

    // 如果不是我们启动的，通过 taskkill 停止
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("taskkill")
            .args(["/IM", PROCESS_CHECK_NAME, "/F"])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("执行 taskkill 失败: {}", e))?;

        if output.status.success() {
            info!("✅ moonlight-web 进程已终止");
            Ok("moonlight-web 已停止".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("⚠️ taskkill 输出: {}", stderr);
            Ok("moonlight-web 可能已经停止".to_string())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("非 Windows 平台暂不支持".to_string())
    }
}

/// 读取 moonlight-web 配置
#[tauri::command]
pub async fn moonlight_web_get_config() -> Result<MoonlightWebConfig, String> {
    let config_path = get_config_path();
    if !config_path.exists() {
        debug!("配置文件不存在，返回默认配置");
        return Ok(MoonlightWebConfig {
            web_server: WebServerConfig::default(),
            webrtc: None,
            default_settings: None,
            extra: std::collections::HashMap::new(),
        });
    }

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))
}

/// 保存 moonlight-web 配置
#[tauri::command]
pub async fn moonlight_web_save_config(mut config: MoonlightWebConfig) -> Result<String, String> {
    // 校验 bind_address 不为空
    if config.web_server.bind_address.trim().is_empty() {
        config.web_server.bind_address = default_bind_address();
    }

    let config_path = get_config_path();

    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    // 更新本地端口缓存
    if let Some(port) = parse_port_from_bind_address(&config.web_server.bind_address) {
        *BIND_PORT.lock().unwrap() = port;
    }

    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;

    std::fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;

    info!("✅ moonlight-web 配置已保存: {:?}", config_path);
    Ok("配置已保存".to_string())
}

/// 检查可用更新
#[tauri::command]
pub async fn moonlight_web_check_release() -> Result<MoonlightWebRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("sunshine-control-panel")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let url = format!("{}/latest", GITHUB_API_RELEASES);
    debug!("📦 检查 moonlight-web 最新版本: {}", url);

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("请求 GitHub API 失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API 返回错误: {}", response.status()));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("解析 Release 数据失败: {}", e))?;

    // 查找 Windows x86_64 的资源
    let asset = release.assets.iter().find(|a| {
        let name = a.name.to_lowercase();
        name.contains("windows")
            && name.contains("x86_64")
            && (name.ends_with(".zip") || name.ends_with(".tar.gz"))
    });

    Ok(MoonlightWebRelease {
        version: release.tag_name.clone(),
        download_url: asset.map(|a| a.browser_download_url.clone()),
        download_name: asset.map(|a| a.name.clone()),
        release_page: release.html_url,
        release_notes: release.body.unwrap_or_default(),
    })
}

/// 下载并安装 moonlight-web
#[tauri::command]
pub async fn moonlight_web_download(
    url: String,
    version: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    info!("📥 开始下载 moonlight-web: {}", url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .user_agent("sunshine-control-panel")
        .build()
        .map_err(|e| format!("创建客户端失败: {}", e))?;

    let response = client
        .get(&url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载失败, HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let install_dir = get_install_dir();

    // 使用系统临时目录下载，避免安装目录权限问题
    let temp_dir = std::env::temp_dir();
    let temp_ext =
        if url.to_lowercase().ends_with(".tar.gz") || url.to_lowercase().ends_with(".tgz") {
            ".tar.gz"
        } else {
            ".zip"
        };
    let temp_path = temp_dir.join(format!("moonlight-web_download{}", temp_ext));
    let mut file =
        std::fs::File::create(&temp_path).map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载数据失败: {}", e))?;
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;

        // 发送进度事件
        if let Some(window) = app_handle.get_webview_window("main") {
            let progress = if total_size > 0 {
                (downloaded as f64 / total_size as f64 * 100.0) as u32
            } else {
                0
            };
            let _ = window.emit(
                "moonlight-web-download-progress",
                serde_json::json!({
                    "progress": progress,
                    "downloaded": downloaded,
                    "total": total_size,
                }),
            );
        }
    }
    drop(file);

    info!("✅ 下载完成 ({} bytes), 开始解压...", downloaded);

    // 解压到临时目录，避免直接在安装目录操作导致权限问题
    let extract_dir = temp_dir.join("moonlight-web-extract");
    if extract_dir.exists() {
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("创建解压临时目录失败: {}", e))?;

    extract_archive(&temp_path, &extract_dir)?;

    // 清理下载的压缩包
    let _ = std::fs::remove_file(&temp_path);

    // 在临时解压目录中查找 web-server.exe 所在目录
    let source_dir = if let Some(found) = find_file_recursive(&extract_dir, SERVER_BINARY_NAME) {
        found.parent().unwrap_or(&extract_dir).to_path_buf()
    } else {
        extract_dir.clone()
    };

    // 确保安装目录存在
    std::fs::create_dir_all(&install_dir).map_err(|e| format!("创建安装目录失败: {}", e))?;

    // 使用 robocopy 将文件复制到安装目录（更可靠，处理权限和覆盖）
    copy_dir_contents(&source_dir, &install_dir)?;

    // 清理临时解压目录
    let _ = std::fs::remove_dir_all(&extract_dir);

    // 保存版本号
    if !version.is_empty() {
        let version_clean = version.trim_start_matches('v');
        let _ = std::fs::write(install_dir.join("version.txt"), version_clean);
    }

    info!("✅ moonlight-web 安装完成: {:?}", install_dir);
    Ok(install_dir.to_string_lossy().to_string())
}

/// 获取安装目录路径
#[tauri::command]
pub fn moonlight_web_get_install_path() -> String {
    get_install_dir().to_string_lossy().to_string()
}

/// 自动生成自签名 HTTPS 证书
#[tauri::command]
pub async fn moonlight_web_generate_cert() -> Result<CertificateConfig, String> {
    let install_dir = get_install_dir();
    let server_dir = install_dir.join("server");
    std::fs::create_dir_all(&server_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let key_path = server_dir.join("key.pem");
    let cert_path = server_dir.join("cert.pem");

    info!("🔐 正在生成自签名 HTTPS 证书...");

    // 使用 rcgen 生成自签名证书
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| format!("证书参数创建失败: {}", e))?;

    // 添加 SAN（Subject Alternative Names）
    params.subject_alt_names = vec![
        rcgen::SanType::DnsName(
            "localhost"
                .try_into()
                .map_err(|e| format!("DNS name 无效: {}", e))?,
        ),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
    ];

    // 设置有效期 10 年
    params.not_before = rcgen::date_time_ymd(2025, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);

    // 设置 DN
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Moonlight Web Stream");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Sunshine Control Panel");

    let key_pair = rcgen::KeyPair::generate().map_err(|e| format!("密钥生成失败: {}", e))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("证书签名失败: {}", e))?;

    // 写入文件
    std::fs::write(&key_path, key_pair.serialize_pem())
        .map_err(|e| format!("写入私钥失败: {}", e))?;
    std::fs::write(&cert_path, cert.pem()).map_err(|e| format!("写入证书失败: {}", e))?;

    info!("✅ 自签名证书已生成: {:?}, {:?}", cert_path, key_path);

    // 返回相对于 moonlight-web 安装目录的路径（web-server 工作目录）
    Ok(CertificateConfig {
        private_key_pem: "./server/key.pem".to_string(),
        certificate_pem: "./server/cert.pem".to_string(),
    })
}

// ========== 内部工具函数 ==========

/// 检测 moonlight-web 进程是否在运行
fn is_process_running() -> bool {
    // 先检查我们自己启动的子进程
    {
        let mut guard = CHILD_PROCESS.lock().unwrap();
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => return true, // 还在运行
                Ok(Some(_)) => {
                    *guard = None;
                } // 已退出
                Err(_) => {
                    *guard = None;
                }
            }
        }
    }

    // 再通过 tasklist 检查
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("tasklist")
            .args([
                "/FI",
                &format!("IMAGENAME eq {}", PROCESS_CHECK_NAME),
                "/NH",
            ])
            .creation_flags(0x08000000)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.to_lowercase().contains("web-server");
        }
    }

    false
}

/// 获取已安装版本
fn get_installed_version() -> Option<String> {
    // 尝试从 version.txt 读取
    let version_file = get_install_dir().join("version.txt");
    if let Ok(version) = std::fs::read_to_string(&version_file) {
        let version = version.trim().to_string();
        if !version.is_empty() {
            return Some(version);
        }
    }

    // 尝试从二进制获取
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let binary = get_server_binary_path();
        if let Ok(output) = std::process::Command::new(&binary)
            .arg("--version")
            .creation_flags(0x08000000)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // --version 输出格式: "web-server 2.4.0"，只取最后的版本号
            let version = stdout
                .trim()
                .rsplit_once(' ')
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| stdout.trim().to_string());
            if !version.is_empty() {
                return Some(version);
            }
        }
    }

    None
}

/// 确保配置文件存在（首次启动时自动生成安全默认配置）
async fn ensure_config_exists() -> Result<(), String> {
    let config_path = get_config_path();
    if config_path.exists() {
        // 尝试加载并验证配置
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            // 用 serde_json::Value 加载，检查并修复无效字段
            if let Ok(mut raw) = serde_json::from_str::<serde_json::Value>(&content) {
                let mut fixed = false;
                if let Some(ws) = raw.get_mut("web_server").and_then(|v| v.as_object_mut()) {
                    // 修复 session_cookie_expiration: null（上游需要 Duration 结构）
                    if ws
                        .get("session_cookie_expiration")
                        .map_or(false, |v| v.is_null())
                    {
                        ws.insert(
                            "session_cookie_expiration".to_string(),
                            serde_json::json!({"secs": 86400, "nanos": 0}),
                        );
                        fixed = true;
                    }
                }
                // 移除顶级 null 字段（上游不支持 null）
                if let Some(obj) = raw.as_object_mut() {
                    let null_keys: Vec<String> = obj
                        .iter()
                        .filter(|(_, v)| v.is_null())
                        .map(|(k, _)| k.clone())
                        .collect();
                    for key in null_keys {
                        obj.remove(&key);
                        fixed = true;
                    }
                }
                if fixed {
                    if let Ok(json) = serde_json::to_string_pretty(&raw) {
                        let _ = std::fs::write(&config_path, json);
                        info!("🔧 已自动修复 config.json 中的无效字段");
                    }
                }
            }

            // 加载端口到缓存
            if let Ok(reread) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<MoonlightWebConfig>(&reread) {
                    if let Some(port) =
                        parse_port_from_bind_address(&config.web_server.bind_address)
                    {
                        *BIND_PORT.lock().unwrap() = port;
                    }
                }
            }
        }
        return Ok(());
    }

    info!("🔧 生成 moonlight-web 默认配置...");

    let config = MoonlightWebConfig {
        web_server: WebServerConfig {
            bind_address: format!("0.0.0.0:{}", DEFAULT_BIND_PORT),
            certificate: None,
            url_path_prefix: String::new(),
            session_cookie_secure: false,
            session_cookie_expiration: Some(SessionExpiration {
                secs: 86400,
                nanos: 0,
            }),
            first_login_create_admin: true,
            first_login_assign_global_hosts: true,
            default_user_id: None,
            forwarded_header: None,
            extra: std::collections::HashMap::new(),
        },
        webrtc: None,
        default_settings: Some(serde_json::json!({
            "videoCodec": "h264",
            "fps": 60,
            "bitrate": 20000,
            "dataTransport": "auto",
        })),
        extra: std::collections::HashMap::new(),
    };

    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;

    std::fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;

    info!("✅ 默认配置已生成: {:?}", config_path);
    Ok(())
}

/// 从 bind_address 中解析端口
fn parse_port_from_bind_address(addr: &str) -> Option<u16> {
    addr.rsplit(':').next()?.parse().ok()
}

/// 检查配置中是否启用了 HTTPS 证书
fn has_https_certificate() -> bool {
    let config_path = get_config_path();
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<MoonlightWebConfig>(&content) {
            return config.web_server.certificate.is_some();
        }
    }
    false
}

/// 递归复制目录内容到目标目录
fn copy_dir_contents(src: &PathBuf, dest: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // robocopy 返回值 < 8 表示成功（0=无变化, 1=新文件复制, 2=额外文件等）
        let output = std::process::Command::new("robocopy")
            .args([
                &src.to_string_lossy().to_string(),
                &dest.to_string_lossy().to_string(),
                "/E",   // 递归包括空目录
                "/NFL", // 不列出文件名
                "/NDL", // 不列出目录名
                "/NJH", // 无 Job Header
                "/NJS", // 无 Job Summary
                "/NC",  // 无文件类
                "/NS",  // 无文件大小
                "/NP",  // 无进度
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("执行 robocopy 失败: {}", e))?;

        let exit_code = output.status.code().unwrap_or(-1);
        if exit_code >= 8 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("robocopy 失败 (exit {}): {}", exit_code, stderr));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix: use cp -a
        let output = std::process::Command::new("cp")
            .args(["-a", "-T", &src.to_string_lossy(), &dest.to_string_lossy()])
            .output()
            .map_err(|e| format!("执行 cp 失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("cp 失败: {}", stderr));
        }
        Ok(())
    }
}

/// 递归搜索文件
fn find_file_recursive(dir: &PathBuf, filename: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && entry.file_name().to_string_lossy() == filename {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, filename) {
                return Some(found);
            }
        }
    }
    None
}

/// 解压下载的压缩包
fn extract_archive(archive_path: &PathBuf, target_dir: &PathBuf) -> Result<(), String> {
    let name = archive_path.to_string_lossy().to_lowercase();

    if name.ends_with(".zip") {
        extract_zip(archive_path, target_dir)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive_path, target_dir)
    } else {
        Err(format!("不支持的压缩格式: {}", name))
    }
}

/// 解压 ZIP 文件
fn extract_zip(zip_path: &PathBuf, target_dir: &PathBuf) -> Result<(), String> {
    // 使用 PowerShell 解压（Windows 内置）
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let ps_cmd = format!(
            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            zip_path.to_string_lossy(),
            target_dir.to_string_lossy()
        );

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &ps_cmd,
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("执行解压命令失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("解压失败: {}", stderr));
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("非 Windows 平台暂不支持".to_string())
    }
}

/// 解压 tar.gz 文件
fn extract_tar_gz(archive_path: &PathBuf, target_dir: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("tar")
            .args([
                "-xzf",
                &archive_path.to_string_lossy(),
                "-C",
                &target_dir.to_string_lossy(),
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("执行 tar 解压失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tar 解压失败: {}", stderr));
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("非 Windows 平台暂不支持".to_string())
    }
}

/// 应用退出时清理子进程
pub fn cleanup() {
    let mut guard = CHILD_PROCESS.lock().unwrap();
    if let Some(ref mut child) = *guard {
        info!("🧹 清理 moonlight-web 子进程 (PID: {})", child.id());
        let _ = child.kill();
        let _ = child.wait();
        *guard = None;
    }
}
