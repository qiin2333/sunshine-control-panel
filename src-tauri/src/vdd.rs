use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use quick_xml::de::from_str;
use quick_xml::se::to_string;
use crate::sunshine;
use log::{info, warn, error, debug};

/// 更新 VDD XML 文件中的 colour、logging 和 edid 节点
/// C++ 的 saveVddSettings 会保留这些字段，所以我们需要先写入
async fn update_vdd_xml_extra_fields(settings: &VddSettings) -> Result<(), String> {
    let vdd_xml_path = get_vdd_settings_path();
    
    // 读取现有 XML（如果存在）
    let mut vdd_settings = if vdd_xml_path.exists() {
        let content = fs::read_to_string(&vdd_xml_path)
            .map_err(|e| format!("读取 VDD XML 失败: {}", e))?;
        
        from_str::<VddSettings>(&content)
            .map_err(|e| format!("解析 VDD XML 失败: {}", e))?
    } else {
        // 如果文件不存在，使用默认配置
        debug!("  📄 VDD 配置文件不存在，使用默认配置");
        get_default_settings()
    };
    
    // 只更新 colour、logging 和 edid 字段（其他字段会被 C++ 更新）
    if let Some(ref colour) = settings.colour {
        vdd_settings.colour = Some(colour.clone());
        debug!("  ✓ 更新 colour 配置");
    }
    
    if let Some(ref logging) = settings.logging {
        vdd_settings.logging = Some(logging.clone());
        debug!("  ✓ 更新 logging 配置");
    }
    
    if let Some(ref edid) = settings.edid {
        vdd_settings.edid = Some(edid.clone());
        debug!("  ✓ 更新 edid 配置: CustomEdid={}, PreventSpoof={}, CeaOverride={}", 
               edid.custom_edid, edid.prevent_spoof, edid.edid_cea_override);
    }
    
    // 序列化回 XML
    let xml = to_string(&vdd_settings)
        .map_err(|e| format!("序列化 VDD XML 失败: {}", e))?;
    
    // 添加 XML 声明
    let full_xml = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}", xml);
    
    // 写入文件
    write_vdd_xml(&vdd_xml_path, &full_xml).await?;
    
    // 验证文件是否更新
    verify_vdd_xml(&vdd_xml_path)?;
    
    Ok(())
}

/// 写入 VDD XML 文件（Windows - 使用管理员权限）
#[cfg(target_os = "windows")]
async fn write_vdd_xml(vdd_xml_path: &PathBuf, content: &str) -> Result<(), String> {
    use std::process::Command;
    
    // 写入临时文件
    let temp_path = std::env::temp_dir().join(format!("vdd_extra_{}.xml", std::process::id()));
    debug!("  📝 写入临时文件: {:?}", temp_path);
    fs::write(&temp_path, content)
        .map_err(|e| format!("写入临时文件失败: {}", e))?;
    
    debug!("  📝 目标文件: {:?}", vdd_xml_path);
    
    // 先尝试使用 ShellExecuteW 触发 UAC 并复制
    let shell_execute_success = match elevated_copy_with_shell_execute(&temp_path, vdd_xml_path) {
        Ok(()) => {
            debug!("  🔧 已请求使用 ShellExecuteW 提权复制");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            match fs::read_to_string(vdd_xml_path) {
                Ok(written) if written == content => {
                    info!("  ✅ ShellExecuteW 提权复制成功");
                    true
                }
                Ok(_) => {
                    warn!("  ⚠️ ShellExecuteW 复制后内容不匹配，准备回退到 PowerShell");
                    false
                }
                Err(err) => {
                    warn!("  ⚠️ ShellExecuteW 复制后读取失败 ({}), 准备回退到 PowerShell", err);
                    false
                }
            }
        }
        Err(err) => {
            warn!("  ⚠️ ShellExecuteW 提权复制调用失败 ({}), 准备回退到 PowerShell", err);
            false
        }
    };
    
    if !shell_execute_success {
        // 使用 Start-Process 以管理员权限运行 PowerShell 复制命令
        let inner_command = format!(
            "Copy-Item -Path '{}' -Destination '{}' -Force",
            temp_path.display(),
            vdd_xml_path.display()
        );
        
        // 使用 -Verb RunAs 以管理员权限运行
        let ps_script = format!(
            r#"Start-Process powershell -ArgumentList '-NoProfile', '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait"#,
            inner_command.replace("'", "''") // PowerShell 中单引号需要双写转义
        );
        
        debug!("  🔧 执行 PowerShell 提权命令...");
        
        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_script])
            .spawn()
            .map_err(|e| {
                // 清理临时文件
                let _ = fs::remove_file(&temp_path);
                format!("执行 PowerShell 命令失败: {}", e)
            })?
            .wait()
            .map_err(|e| {
                // 清理临时文件
                let _ = fs::remove_file(&temp_path);
                format!("等待 PowerShell 命令完成失败: {}", e)
            })?;
        
        // 等待文件写入完成
        debug!("  ⏳ 等待文件写入完成...");
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // 验证文件是否成功写入
        if !output.success() {
            error!("  ❌ PowerShell 提权复制失败");
            
            // 尝试直接写入（可能会因权限不足而失败）
            warn!("  ⚠️ 尝试直接写入...");
            fs::write(vdd_xml_path, content)
                .map_err(|e| {
                    // 清理临时文件
                    let _ = fs::remove_file(&temp_path);
                    format!("写入失败，需要管理员权限: {}", e)
                })?;
            info!("  ✓ 直接写入成功");
        } else {
            info!("  ✅ PowerShell 提权复制成功");
        }
    }
    
    // 清理临时文件
    let _ = fs::remove_file(&temp_path);
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn elevated_copy_with_shell_execute(source: &Path, destination: &Path) -> Result<(), String> {
    use std::path::PathBuf;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    
    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0u16)).collect()
    }
    
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let cmd_path: PathBuf = Path::new(&system_root).join("System32").join("cmd.exe");
    
    if !cmd_path.exists() {
        return Err(format!("找不到 cmd.exe: {:?}", cmd_path));
    }
    
    let parameters = format!(
        r#"/C copy "{}" "{}" /Y"#,
        source.to_string_lossy(),
        destination.to_string_lossy()
    );
    
    let operation_w = to_wide("runas");
    let file_w = to_wide(&cmd_path.to_string_lossy());
    let parameters_w = to_wide(&parameters);
    
    unsafe {
        let result = ShellExecuteW(
            HWND(0),
            PCWSTR(operation_w.as_ptr()),
            PCWSTR(file_w.as_ptr()),
            PCWSTR(parameters_w.as_ptr()),
            PCWSTR::null(),
            SW_HIDE,
        );
        
        if result.0 as isize <= 32 {
            return Err(format!("ShellExecuteW 返回错误码 {}", result.0 as isize));
        }
    }
    
    Ok(())
}

/// 写入 VDD XML 文件（非 Windows 平台）
#[cfg(not(target_os = "windows"))]
async fn write_vdd_xml(vdd_xml_path: &PathBuf, content: &str) -> Result<(), String> {
    // 确保目录存在
    if let Some(parent) = vdd_xml_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
    fs::write(vdd_xml_path, content)
        .map_err(|e| format!("写入 VDD XML 失败: {}", e))?;
    
    debug!("  ✓ 已写入 colour 和 logging 到 XML");
    
    Ok(())
}

/// 验证 VDD XML 文件
fn verify_vdd_xml(vdd_xml_path: &PathBuf) -> Result<(), String> {
    if !vdd_xml_path.exists() {
        return Err("验证失败: 文件不存在".to_string());
    }
    
    let verify_content = fs::read_to_string(vdd_xml_path)
        .map_err(|e| format!("验证文件失败: {}", e))?;
    
    if verify_content.contains("<colour>") || verify_content.contains("<logging>") {
        debug!("  ✅ 验证: colour/logging 字段已写入");
    } else {
        warn!("  ⚠️  警告: 未在文件中找到 colour/logging 字段");
    }
    
    Ok(())
}

/// 读取完整的 sunshine.conf 配置文件为 Map
async fn read_full_sunshine_config() -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let config_path = PathBuf::from(sunshine::get_sunshine_install_path())
        .join("config")
        .join("sunshine.conf");
    
    let mut config_map = serde_json::Map::new();
    
    if !config_path.exists() {
        warn!("⚠️  配置文件不存在: {:?}", config_path);
        return Ok(config_map);
    }
    
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取 sunshine.conf 失败: {}", e))?;
    
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i].trim();
        
        // 跳过注释和空行
        if line.starts_with('#') || line.is_empty() {
            i += 1;
            continue;
        }
        
        // 解析 key = value 格式
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let mut value = value.trim().to_string();
            
            // 检查是否是多行值（以 [ 开始但不以 ] 结束）
            if value.starts_with('[') && !value.ends_with(']') {
                // 读取后续行直到找到 ]
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    value.push('\n');
                    value.push_str(next_line);
                    
                    if next_line.ends_with(']') {
                        break;
                    }
                    i += 1;
                }
            }
            
            config_map.insert(key, serde_json::json!(value));
        }
        
        i += 1;
    }
    
    debug!("📄 读取到 {} 个配置项", config_map.len());
    Ok(config_map)
}

/// 调用 Sunshine Config API 保存 VDD 配置
/// Sunshine 的 saveVddSettings() 会负责写入 vdd_settings.xml 文件
async fn sync_vdd_config_to_sunshine(settings: &VddSettings) -> Result<(), String> {
    // 从配置文件获取 Sunshine Web UI URL（动态读取端口）
    let sunshine_url = sunshine::get_sunshine_url().await
        .map_err(|e| format!("无法获取 Sunshine URL: {}", e))?;
    
    // 读取完整的现有配置，然后更新 VDD 相关的配置项
    // 这样可以避免丢失其他配置
    let mut config_data = read_full_sunshine_config().await?;
    
    debug!("🔄 合并 VDD 配置到现有配置中");
    
    // 更新分辨率配置 - 格式: [1920x1080,2560x1440] (不带引号)
    if !settings.resolutions.resolution.is_empty() {
        let resolutions: Vec<String> = settings.resolutions.resolution.iter()
            .map(|r| format!("{}x{}", r.width, r.height))
            .collect();
        
        // 序列化为 JSON 字符串，然后去掉引号，匹配前端格式
        let resolutions_json = serde_json::to_string(&resolutions)
            .unwrap_or_else(|_| "[]".to_string())
            .replace("\"", ""); // 去掉所有引号
        
        // 更新或插入到配置中
        config_data.insert("resolutions".to_string(), serde_json::json!(resolutions_json));
        debug!("  ✓ 分辨率: {}", resolutions_json);
    }
    
    // 更新刷新率配置（作为 fps） - 格式: [60,120,240]
    if !settings.global.g_refresh_rate.is_empty() {
        let fps_json = serde_json::to_string(&settings.global.g_refresh_rate)
            .unwrap_or_else(|_| "[]".to_string())
            .replace("\"", ""); // 去掉引号
        
        // 更新或插入到配置中
        config_data.insert("fps".to_string(), serde_json::json!(fps_json));
        debug!("  ✓ 刷新率: {}", fps_json);
    }
    
    // 更新 GPU 名称 - 格式: 普通字符串
    if !settings.gpu.friendlyname.is_empty() {
        config_data.insert("adapter_name".to_string(), serde_json::json!(settings.gpu.friendlyname));
        debug!("  ✓ GPU: {}", settings.gpu.friendlyname);
    }
    
    // 调用 Sunshine Config API
    let config_url = format!("{}/api/config", sunshine_url.trim_end_matches('/'));
    
    debug!("📡 调用 Sunshine Config API: {}", config_url);
    debug!("📝 配置数据: {:?}", config_data);
    
    // 使用 reqwest 发送 POST 请求
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // Sunshine 使用自签名证书
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    let response = client.post(&config_url)
        .json(&config_data)
        .send()
        .await
        .map_err(|e| format!("调用 Sunshine Config API 失败: {}", e))?;
    
    if response.status().is_success() {
        info!("✅ VDD 配置已通过 Sunshine API 保存 (状态: {})", response.status());
        Ok(())
    } else {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        Err(format!("Sunshine Config API 返回错误 (状态: {}): {}", status, error_body))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VddSettings {
    pub monitors: Monitors,
    pub gpu: Gpu,
    pub global: Global,
    pub resolutions: Resolutions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edid: Option<EdidConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Monitors {
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Gpu {
    pub friendlyname: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Global {
    #[serde(rename = "g_refresh_rate")]
    pub g_refresh_rate: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resolutions {
    #[serde(rename = "resolution")]
    pub resolution: Vec<Resolution>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Colour {
    #[serde(rename = "SDR10bit")]
    pub sdr10bit: bool,
    #[serde(rename = "HDRPlus")]
    pub hdr_plus: bool,
    #[serde(rename = "ColourFormat")]
    pub colour_format: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub logging: bool,
    pub debuglogging: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EdidConfig {
    #[serde(rename = "CustomEdid")]
    pub custom_edid: bool,
    #[serde(rename = "PreventSpoof")]
    pub prevent_spoof: bool,
    #[serde(rename = "EdidCeaOverride")]
    pub edid_cea_override: bool,
}

/// 获取 Sunshine 安装路径
fn get_sunshine_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path())
}

/// 从注册表读取 VDD 设置目录路径
#[cfg(target_os = "windows")]
fn get_vdd_base_path() -> Result<PathBuf, String> {
    use winreg::enums::*;
    use winreg::RegKey;
    
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let vdd_key = hklm
        .open_subkey(r"SOFTWARE\ZakoTech\ZakoDisplayAdapter")
        .map_err(|e| format!("无法打开注册表项: {}", e))?;
    
    let vdd_path: String = vdd_key
        .get_value("VDDPATH")
        .map_err(|e| format!("无法读取 VDDPATH: {}", e))?;
    
    Ok(PathBuf::from(vdd_path))
}

/// 从注册表读取 VDD 设置目录路径（非 Windows 平台回退）
#[cfg(not(target_os = "windows"))]
fn get_vdd_base_path() -> Result<PathBuf, String> {
    Err("VDD 仅支持 Windows 平台".to_string())
}

/// 获取 VDD 设置文件路径
fn get_vdd_settings_path() -> PathBuf {
    get_vdd_base_path()
        .unwrap_or_else(|_| PathBuf::from("C:\\VirtualDisplayDriver"))
        .join("vdd_settings.xml")
}

/// 获取 VDD 工具目录路径
fn get_vdd_tools_path() -> PathBuf {
    get_sunshine_path().join("tools").join("vdd")
}

/// 获取 VDD EDID 文件路径
fn get_vdd_edid_path() -> PathBuf {
    // VDD 驱动从注册表路径下的 user_edid.bin 读取自定义 EDID
    get_vdd_base_path()
        .unwrap_or_else(|_| PathBuf::from("C:\\VirtualDisplayDriver"))
        .join("user_edid.bin")
}

/// 获取 VDD 设置文件路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_settings_file_path() -> String {
    get_vdd_settings_path()
        .to_string_lossy()
        .to_string()
}

/// 获取 VDD 工具目录路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_tools_dir_path() -> String {
    get_vdd_tools_path()
        .to_string_lossy()
        .to_string()
}

/// 获取 VDD EDID 文件路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_edid_file_path() -> String {
    get_vdd_edid_path()
        .to_string_lossy()
        .to_string()
}

fn get_default_settings() -> VddSettings {
    VddSettings {
        monitors: Monitors { count: 1 },
        gpu: Gpu {
            friendlyname: String::new(),
        },
        global: Global {
            g_refresh_rate: vec![60, 120, 240],
        },
        resolutions: Resolutions {
            resolution: vec![],
        },
        colour: Some(Colour {
            sdr10bit: false,
            hdr_plus: false,
            colour_format: "RGB".to_string(),
        }),
        logging: Some(Logging {
            logging: false,
            debuglogging: false,
        }),
        edid: Some(EdidConfig {
            custom_edid: false,
            prevent_spoof: false,
            edid_cea_override: false,
        }),
    }
}

#[tauri::command]
pub async fn load_vdd_settings() -> Result<VddSettings, String> {
    let path = get_vdd_settings_path();
    
    if !path.exists() {
        return Ok(get_default_settings());
    }
    
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;
    
    debug!("📄 读取到的 XML 内容:\n{}", content);
    
    // 解析 XML
    let settings: VddSettings = from_str(&content)
        .map_err(|e| {
            error!("❌ XML 解析失败: {}", e);
            error!("📄 XML 内容:\n{}", content);
            format!("XML 解析失败: {}", e)
        })?;

    info!("✅ XML 解析成功！");
    debug!("🔍 解析后的 VDD 设置: {:?}", settings);
    debug!("🔍 解析后的 GPU 名称: {}", settings.gpu.friendlyname);
    debug!("🔍 解析后的分辨率数量: {}", settings.resolutions.resolution.len());
    debug!("🔍 解析后的全局刷新率: {:?}", settings.global.g_refresh_rate);
    
    Ok(settings)
}

#[tauri::command]
pub async fn save_vdd_settings(settings: VddSettings) -> Result<String, String> {
    info!("💾 开始保存 VDD 配置...");
    
    // 步骤1: 调用 Sunshine Config API 保存主要配置（resolutions, fps, adapter_name）
    // C++ 会写入 monitors, gpu, global, resolutions 字段
    sync_vdd_config_to_sunshine(&settings).await?;
    
    // 步骤2: 等待 C++ 完成文件写入
    debug!("⏳ 等待 Sunshine API 完成文件写入...");
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // 步骤3: 写入 colour、logging 和 edid 到 XML
    // 读取 C++ 刚写入的 XML，添加 colour、logging 和 edid，然后写回
    debug!("📝 写入 colour、logging 和 edid 字段...");
    update_vdd_xml_extra_fields(&settings).await?;
    
    // 步骤4: 通知 VDD 驱动重新加载配置
    #[cfg(target_os = "windows")]
    {
        debug!("🔄 通知 VDD 驱动重新加载...");
        let _ = exec_pipe_cmd("RELOAD_DRIVER".to_string()).await;
    }
    
    info!("✅ VDD 配置保存完成");
    Ok("保存成功".to_string())
}

#[tauri::command]
pub async fn exec_pipe_cmd(command: String) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Storage::FileSystem::*;
        use windows::Win32::Foundation::*;
        use windows::core::PCWSTR;
        
        tokio::task::spawn_blocking(move || {
            unsafe {
                let pipe_name = r"\\.\pipe\ZakoVDDPipe";
                let wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
                
                let handle = CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    FILE_GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    HANDLE::default(),
                );
                
                if handle.is_err() || handle.as_ref().unwrap().is_invalid() {
                    return Err("无法连接到管道".to_string());
                }
                
                let handle = handle.unwrap();
                
                // 转换为 UTF-16LE
                let cmd_wide: Vec<u16> = command.encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let buffer = cmd_wide.as_ptr() as *const u8;
                let buffer_len = (cmd_wide.len() * 2) as u32;
                
                let mut bytes_written = 0u32;
                let result = WriteFile(
                    handle,
                    Some(std::slice::from_raw_parts(buffer, buffer_len as usize)),
                    Some(&mut bytes_written),
                    None,
                );
                
                let _ = CloseHandle(handle);
                
                if result.is_ok() {
                    Ok(true)
                } else {
                    Err("写入管道失败".to_string())
                }
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(true)
    }
}

/// 验证 EDID 文件格式和 checksum
fn validate_edid(data: &[u8]) -> Result<(), String> {
    // EDID 必须是 128 或 256 字节
    if data.len() != 128 && data.len() != 256 {
        return Err(format!("EDID 文件大小无效: {} 字节（必须是 128 或 256 字节）", data.len()));
    }
    
    // 验证 EDID 头部 (前8字节应该是: 00 FF FF FF FF FF FF 00)
    let expected_header: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
    if data.len() >= 8 && &data[0..8] != &expected_header {
        return Err("EDID 头部格式无效".to_string());
    }
    
    // 计算并验证 checksum (第127字节)
    let mut sum: u32 = 0;
    for i in 0..127 {
        sum += data[i] as u32;
    }
    sum %= 256;
    
    let expected_checksum = if sum != 0 { (256 - sum) as u8 } else { 0 };
    
    if data[127] != expected_checksum {
        return Err(format!(
            "EDID checksum 无效: 期望 0x{:02X}，实际 0x{:02X}",
            expected_checksum, data[127]
        ));
    }
    
    Ok(())
}

/// 上传并保存 EDID 文件
#[tauri::command]
pub async fn upload_edid_file(file_data: Vec<u8>) -> Result<String, String> {
    info!("📤 开始上传 EDID 文件（{} 字节）", file_data.len());
    
    // 验证 EDID 数据
    validate_edid(&file_data)?;
    info!("✅ EDID 验证通过");
    
    let edid_path = get_vdd_edid_path();
    
    // 确保目录存在
    if let Some(parent) = edid_path.parent() {
        if !parent.exists() {
            #[cfg(target_os = "windows")]
            {
                use std::process::Command;
                
                // 使用管理员权限创建目录
                let ps_command = format!(
                    r#"Start-Process powershell -ArgumentList '-Command', 'New-Item -ItemType Directory -Force -Path \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait"#,
                    parent.display()
                );
                
                Command::new("powershell")
                    .args(&["-NoProfile", "-Command", &ps_command])
                    .spawn()
                    .map_err(|e| format!("创建目录失败: {}", e))?
                    .wait()
                    .map_err(|e| format!("等待创建目录完成失败: {}", e))?;
                
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            
            #[cfg(not(target_os = "windows"))]
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    
    // 写入临时文件
    let temp_path = std::env::temp_dir().join(format!("user_edid_{}.bin", std::process::id()));
    fs::write(&temp_path, &file_data)
        .map_err(|e| format!("写入临时文件失败: {}", e))?;
    
    // 使用管理员权限复制文件
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let ps_command = format!(
            r#"Start-Process powershell -ArgumentList '-Command', 'Copy-Item -Path \"{}\" -Destination \"{}\" -Force' -Verb RunAs -WindowStyle Hidden -Wait"#,
            temp_path.display(),
            edid_path.display()
        );
        
        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_command])
            .spawn()
            .map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                format!("复制 EDID 文件失败: {}", e)
            })?
            .wait()
            .map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                format!("等待复制完成失败: {}", e)
            })?;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        fs::copy(&temp_path, &edid_path)
            .map_err(|e| format!("复制 EDID 文件失败: {}", e))?;
    }
    
    // 清理临时文件
    let _ = fs::remove_file(&temp_path);
    
    // 验证文件是否成功写入
    if !edid_path.exists() {
        return Err("EDID 文件写入失败".to_string());
    }
    
    info!("✅ EDID 文件已保存到: {:?}", edid_path);
    Ok(format!("EDID 文件已保存: {}", edid_path.display()))
}

/// 读取当前的 EDID 文件
#[tauri::command]
pub fn read_edid_file() -> Result<Vec<u8>, String> {
    let edid_path = get_vdd_edid_path();
    
    if !edid_path.exists() {
        return Err("EDID 文件不存在".to_string());
    }
    
    let data = fs::read(&edid_path)
        .map_err(|e| format!("读取 EDID 文件失败: {}", e))?;
    
    // 验证读取的数据
    validate_edid(&data)?;
    
    Ok(data)
}

/// 删除自定义 EDID 文件
#[tauri::command]
pub async fn delete_edid_file() -> Result<String, String> {
    let edid_path = get_vdd_edid_path();
    
    if !edid_path.exists() {
        return Ok("EDID 文件不存在".to_string());
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let ps_command = format!(
            r#"Start-Process powershell -ArgumentList '-Command', 'Remove-Item -Path \"{}\" -Force' -Verb RunAs -WindowStyle Hidden -Wait"#,
            edid_path.display()
        );
        
        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_command])
            .spawn()
            .map_err(|e| format!("删除 EDID 文件失败: {}", e))?
            .wait()
            .map_err(|e| format!("等待删除完成失败: {}", e))?;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        fs::remove_file(&edid_path)
            .map_err(|e| format!("删除 EDID 文件失败: {}", e))?;
    }
    
    info!("✅ EDID 文件已删除");
    Ok("EDID 文件已删除".to_string())
}

#[tauri::command]
pub async fn uninstall_vdd_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        // 从注册表动态获取 VDD 工具路径
        let nefconw_exe = get_vdd_tools_path().join("nefconw.exe");
        
        if !nefconw_exe.exists() {
            return Err("找不到 nefconw.exe".to_string());
        }
        
        let command = format!(
            r#"'{}' --remove-device-node --hardware-id ROOT\iddsampledriver --class-guid 4d36e968-e325-11ce-bfc1-08002be10318"#,
            nefconw_exe.display()
        );
        
        let ps_command = format!(
            r#"Start-Process powershell -ArgumentList '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait"#,
            command
        );
        
        Command::new("powershell")
            .args(&["-Command", &ps_command])
            .spawn()
            .map_err(|e| e.to_string())?;
        
        Ok("已请求卸载虚拟显示器驱动".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}
