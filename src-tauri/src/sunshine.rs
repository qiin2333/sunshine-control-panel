use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SunshineConfig {
    pub port: Option<String>,
    pub adapter_name: Option<String>,
    pub resolutions: Option<String>,
    pub fps: Option<String>,
}

fn get_sunshine_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // 尝试从注册表读取 Sunshine 安装路径
        use winreg::enums::*;
        use winreg::RegKey;
        
        // 优先尝试 HKLM\SOFTWARE\LizardByte\Sunshine
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        
        // 尝试多个可能的注册表位置
        let registry_paths = vec![
            r"SOFTWARE\LizardByte\Sunshine",
            r"SOFTWARE\WOW6432Node\LizardByte\Sunshine",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine",
        ];
        
        for reg_path in registry_paths {
            if let Ok(sunshine_key) = hklm.open_subkey(reg_path) {
                // 尝试读取 InstallLocation 或 InstallPath
                for key_name in &["InstallLocation", "InstallPath", "Path", ""] {
                    if let Ok(path) = sunshine_key.get_value::<String, _>(key_name) {
                        let install_path = PathBuf::from(path);
                        if install_path.exists() {
                            println!("✅ 从注册表读取到 Sunshine 路径: {:?}", install_path);
                            return install_path;
                        }
                    }
                }
            }
        }
        
        // 如果注册表读取失败，尝试默认路径
        let default_paths = vec![
            PathBuf::from(r"C:\Program Files\Sunshine"),
            PathBuf::from(r"C:\Program Files (x86)\Sunshine"),
        ];
        
        for path in default_paths {
            if path.exists() {
                println!("✅ 使用默认 Sunshine 路径: {:?}", path);
                return path;
            }
        }
        
        // 最后的降级方案
        eprintln!("⚠️  无法找到 Sunshine 安装路径，使用默认路径");
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

    let output = Command::new(sunshine_exe)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 解析版本号
    let patterns = vec![
        regex::Regex::new(r"Sunshine\s+v?([\d.]+)").ok(),
        regex::Regex::new(r"version\s*:?\s*v?([\d.]+)").ok(),
        regex::Regex::new(r"v?(\d+\.\d+\.\d+)").ok(),
        regex::Regex::new(r"(\d+\.\d+)").ok(),
    ];
    
    for pattern in patterns.iter().flatten() {
        if let Some(cap) = pattern.captures(&combined) {
            if let Some(version) = cap.get(1) {
                return Ok(version.as_str().to_string());
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

/// 获取代理服务器 URL（用于主题同步）
#[tauri::command]
pub async fn get_sunshine_proxy_url() -> Result<String, String> {
    Ok("http://localhost:48081/".to_string())
}

#[tauri::command]
pub async fn get_sunshine_url() -> Result<String, String> {
    // 首先检查命令行参数
    let args: Vec<String> = std::env::args().collect();
    
    // 查找 --url= 参数
    for arg in &args {
        if arg.starts_with("--url=") {
            let url = arg.trim_start_matches("--url=");
            return Ok(url.to_string());
        }
    }
    
    // 如果没有命令行参数，从配置文件读取
    let config = parse_sunshine_config().await?;
    
    let port = config.port
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(47989);
    
    // Sunshine Web UI 端口通常是配置端口 + 1
    let web_port = port + 1;
    
    // 使用 127.0.0.1 而不是 localhost，避免 IPv6 解析问题
    Ok(format!("https://127.0.0.1:{}", web_port))
}

#[tauri::command]
pub fn get_command_line_url() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    
    for arg in &args {
        if arg.starts_with("--url=") {
            return Some(arg.trim_start_matches("--url=").to_string());
        }
    }
    
    None
}


