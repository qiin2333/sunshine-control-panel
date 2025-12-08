use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use url::Url;
use log::{info, warn, error, debug};
use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SunshineConfig {
    pub port: Option<String>,
    pub adapter_name: Option<String>,
    pub resolutions: Option<String>,
    pub fps: Option<String>,
}

// 缓存 Sunshine 路径，避免重复查找和记录日志
static SUNSHINE_PATH_CACHE: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

fn get_sunshine_path() -> PathBuf {
    // 先检查缓存
    {
        let cache = SUNSHINE_PATH_CACHE.lock().unwrap();
        if let Some(ref cached_path) = *cache {
            return cached_path.clone();
        }
    }
    
    // 缓存未命中，查找路径
    let path = get_sunshine_path_internal();
    
    // 更新缓存
    {
        let mut cache = SUNSHINE_PATH_CACHE.lock().unwrap();
        *cache = Some(path.clone());
    }
    
    path
}

fn get_sunshine_path_internal() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        
        // 尝试多个可能的注册表位置
        let registry_paths = [
            r"SOFTWARE\LizardByte\Sunshine",
            r"SOFTWARE\WOW6432Node\LizardByte\Sunshine",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine",
        ];
        
        for reg_path in &registry_paths {
            if let Ok(sunshine_key) = hklm.open_subkey(reg_path) {
                // 尝试读取多个可能的键名
                for key_name in &["InstallLocation", "InstallPath", "Path", ""] {
                    if let Ok(path) = sunshine_key.get_value::<String, _>(key_name) {
                        let install_path = PathBuf::from(path);
                        if install_path.exists() {
                            info!("✅ 从注册表读取到 Sunshine 路径: {:?}", install_path);
                            return install_path;
                        }
                    }
                }
            }
        }
        
        // 尝试默认安装路径
        let default_paths = [
            PathBuf::from(r"C:\Program Files\Sunshine"),
            PathBuf::from(r"C:\Program Files (x86)\Sunshine"),
        ];
        
        for path in &default_paths {
            if path.exists() {
                info!("✅ 使用默认 Sunshine 路径: {:?}", path);
                return path.clone();
            }
        }
        
        warn!("⚠️  无法找到 Sunshine 安装路径，使用默认路径");
        PathBuf::from(r"C:\Program Files\Sunshine")
    }

    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/usr/local/sunshine")
    }
}

/// 获取 Sunshine 安装路径（暴露给前端）
#[tauri::command]
pub fn get_sunshine_install_path() -> String {
    get_sunshine_path()
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
pub async fn get_sunshine_version() -> Result<String, String> {
    let sunshine_exe = get_sunshine_path().join("sunshine.exe");
    
    if !sunshine_exe.exists() {
        return Ok("Unknown".to_string());
    }

    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        Command::new(sunshine_exe)
            .arg("--version")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string())?
    };

    #[cfg(not(target_os = "windows"))]
    let output = Command::new(sunshine_exe)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 按优先级匹配版本号模式
    let patterns = [
        r"v?(\d+\.\d+\.\d+\.杂鱼)",           // 完整版本号（含"杂鱼"）
        r"(\d+\.\d+\.\d+\.杂鱼)",             // 不带 v 前缀
        r"Sunshine\s+v?([\d.]+(?:\.杂鱼)?)",  // "Sunshine v..." 格式
        r"version\s*:?\s*v?([\d.]+(?:\.杂鱼)?)", // "version: ..." 格式
        r"v?(\d+\.\d+\.\d+(?:\.杂鱼)?)",      // 标准版本号
        r"(\d+\.\d+(?:\.杂鱼)?)",             // 简化版本号
    ];
    
    for pattern_str in &patterns {
        if let Ok(pattern) = regex::Regex::new(pattern_str) {
            if let Some(cap) = pattern.captures(&combined) {
                if let Some(version) = cap.get(1) {
                    let version_str = version.as_str().to_string();
                    debug!("✅ 解析到版本号: {}", version_str);
                    return Ok(version_str);
                }
            }
        }
    }
    
    Ok("Unknown".to_string())
}

#[tauri::command]
pub async fn parse_sunshine_config() -> Result<SunshineConfig, String> {
    let config_path = get_sunshine_path().join("config").join("sunshine.conf");
    
    if !config_path.exists() {
        return Ok(SunshineConfig {
            port: Some("47989".to_string()),
            adapter_name: None,
            resolutions: None,
            fps: None,
        });
    }
    
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| e.to_string())?;
    
    let mut config = SunshineConfig {
        port: None,
        adapter_name: None,
        resolutions: None,
        fps: None,
    };
    
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            
            match key {
                "port" => config.port = Some(value.to_string()),
                "adapter_name" => config.adapter_name = Some(value.to_string()),
                "resolutions" => config.resolutions = Some(value.to_string()),
                "fps" => config.fps = Some(value.to_string()),
                _ => {}
            }
        }
    }
    
    Ok(config)
}

const DEFAULT_SUNSHINE_PORT: u16 = 47989;
const DEFAULT_WEB_UI_PORT: u16 = 47990;

#[tauri::command]
pub async fn get_sunshine_url() -> Result<String, String> {
    // 优先检查命令行参数
    if let Some(url) = get_command_line_url() {
        return parse_url_to_base(&url).ok_or_else(|| url);
    }
    
    // 从配置文件读取端口
    let config = parse_sunshine_config().await?;
    
    let port = config.port
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_SUNSHINE_PORT);
    
    // Sunshine Web UI 端口 = 配置端口 + 1
    Ok(format!("https://127.0.0.1:{}", port + 1))
}

fn parse_url_to_base(url: &str) -> Option<String> {
    url::Url::parse(url).ok().map(|parsed| {
        let host = parsed.host_str().unwrap_or("127.0.0.1");
        let port = parsed.port().unwrap_or(DEFAULT_WEB_UI_PORT);
        format!("{}://{}:{}", parsed.scheme(), host, port)
    })
}

#[tauri::command]
pub fn get_command_line_url() -> Option<String> {
    std::env::args()
        .find_map(|arg| arg.strip_prefix("--url=").map(String::from))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub client_name: String,
    pub client_address: String,
    pub state: String,
    pub session_id: i32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,  // Current bitrate in Kbps
    pub host_audio: bool,
    pub enable_hdr: bool,
    pub enable_mic: bool,
    pub app_name: String,
    pub app_id: i32,
}

impl SessionInfo {
    fn from_json(session_obj: &serde_json::Value) -> Self {
        Self {
            client_name: session_obj
                .get("client_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            client_address: session_obj
                .get("client_address")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            state: session_obj
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
            session_id: session_obj
                .get("session_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
            width: session_obj
                .get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            height: session_obj
                .get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            fps: session_obj
                .get("fps")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            bitrate: session_obj
                .get("bitrate")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            host_audio: session_obj
                .get("host_audio")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            enable_hdr: session_obj
                .get("enable_hdr")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            enable_mic: session_obj
                .get("enable_mic")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            app_name: session_obj
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            app_id: session_obj
                .get("app_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
        }
    }
}

fn create_https_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // Sunshine 使用自签名证书
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

#[tauri::command]
pub async fn get_active_sessions() -> Result<Vec<SessionInfo>, String> {
    let sunshine_url = get_sunshine_url().await?;
    let sessions_url = format!("{}/api/runtime/sessions", sunshine_url.trim_end_matches('/'));
    
    debug!("📡 获取活动会话: {}", sessions_url);
    
    let client = create_https_client()?;
    
    let response = client
        .get(&sessions_url)
        .send()
        .await
        .map_err(|e| format!("请求会话信息失败: {}", e))?;
    
    let status = response.status();

    debug!("📡 获取 sessions 响应状态码: {}", status);
    
    // 检查 Content-Type
    let content_type = response.headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    
    let response_text = response.text().await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    debug!("📡 获取 sessions 响应内容: {}", response_text);
    
    // 如果是 404 或 XML 响应，返回空数组（没有会话是正常情况）
    if status == 404 || content_type.contains("xml") || response_text.trim_start().starts_with("<?xml") {
        debug!("⚠️ 没有活动会话 (404 或 XML 响应)");
        return Ok(Vec::new());
    }
    
    // 如果状态码不是成功，但也不是 404，返回错误
    if !status.is_success() {
        error!("❌ 错误响应: {}", response_text);
        return Err(format!("获取会话信息失败 (状态: {}): {}", status, response_text));
    }
    
    // 尝试解析 JSON
    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析 JSON 失败: {}，响应内容: {}", e, response_text))?;
    
    debug!("📡 解析后的 JSON: {:#}", json);
    
    // 检查 API 响应状态
    if let Some(success) = json.get("success").and_then(|v| v.as_bool()) {
        if !success {
            let error_msg = json.get("status_message")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            return Err(format!("API 返回错误: {}", error_msg));
        }
    }
    
    // 解析会话列表
    let sessions = json
        .get("sessions")
        .and_then(|v| v.as_array())
        .map(|sessions_array| {
            debug!("📡 找到 {} 个会话", sessions_array.len());
            sessions_array
                .iter()
                .map(SessionInfo::from_json)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            warn!("⚠️ 响应中没有 'sessions' 字段或不是数组");
            debug!("📡 JSON 结构: {:#}", json);
            Vec::new()
        });
    
    info!("✅ 获取到 {} 个活动会话", sessions.len());
    Ok(sessions)
}

#[tauri::command]
pub async fn change_bitrate(client_name: String, bitrate: u32) -> Result<String, String> {
    
    // 验证码率范围
    if !(1..=800000).contains(&bitrate) {
        return Err("码率值必须在 1-800000 Kbps 之间".to_string());
    }
    
    // 构建请求 URL
    let sunshine_url = get_sunshine_url().await?;
    let base_url = Url::parse(&sunshine_url)
        .map_err(|e| format!("解析 Sunshine URL 失败: {}", e))?;
    
    let mut change_bitrate_url = base_url.join("api/runtime/bitrate")
        .map_err(|e| format!("构建 URL 失败: {}", e))?;
    
    change_bitrate_url.query_pairs_mut()
        .append_pair("bitrate", &bitrate.to_string())
        .append_pair("clientname", &client_name);
    
    info!("📡 调整码率: {} -> {} Kbps", client_name, bitrate);
    debug!("📡 请求 URL: {}", change_bitrate_url);
    
    // 发送请求
    let client = create_https_client()?;
    let response = client
        .get(change_bitrate_url.as_str())
        .send()
        .await
        .map_err(|e| format!("请求调整码率失败: {}", e))?;
    
    let status = response.status();
    debug!("📡 HTTP 状态码: {}", status);
    
    // 读取响应内容
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    
    // 检查 HTTP 状态码
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "身份验证失败，请检查 Sunshine 配置".to_string(),
            403 => "访问被拒绝，仅允许 localhost 访问".to_string(),
            _ => format!("HTTP 错误 (状态码: {}): {}", status, response_text),
        });
        }
    
    // 解析 JSON 响应
    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析 JSON 失败: {}，响应内容: {}", e, response_text))?;
    
    debug!("📡 解析后的 JSON: {:#}", json);
    
    // 检查响应状态
    match json.get("success").and_then(|v| v.as_bool()) {
        Some(true) => {
            info!("✅ 码率调整成功");
            Ok(format!("码率已调整为 {} Kbps", bitrate))
        }
        Some(false) => {
            let error_msg = json.get("status_message")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            let status_code = json.get("status_code")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            
            error!("❌ 码率调整失败: {} (状态码: {})", error_msg, status_code);
            
            // 根据状态码提供详细提示
            let error_message = if status_code == 404 {
                format!("码率调整失败: {}\n\n提示：请确认客户端名称是否正确，或会话是否处于 RUNNING 状态", error_msg)
            } else {
                format!("码率调整失败: {}", error_msg)
            };
            
            Err(error_message)
        }
        None => {
            warn!("⚠️ 响应格式无效，无法解析 success 字段");
            Err("无效的响应格式".to_string())
        }
    }
}
