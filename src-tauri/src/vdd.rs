use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use quick_xml::de::from_str;
use quick_xml::se::to_string;
use crate::sunshine;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VddSettings {
    pub monitors: Vec<Monitor>,
    pub gpu: Vec<Gpu>,
    pub global: Global,
    pub resolutions: Vec<Resolutions>,
    pub colour: Vec<Colour>,
    pub logging: Vec<Logging>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Monitor {
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Gpu {
    pub friendlyname: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Global {
    pub g_refresh_rate: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resolutions {
    pub resolution: Vec<Resolution>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resolution {
    pub width: Vec<u32>,
    pub height: Vec<u32>,
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

/// 获取 Sunshine 安装路径
fn get_sunshine_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path())
}

/// 获取 VDD 设置文件路径
fn get_vdd_settings_path() -> PathBuf {
    get_sunshine_path().join("config").join("vdd_settings.xml")
}

/// 获取 VDD 工具目录路径
fn get_vdd_tools_path() -> PathBuf {
    get_sunshine_path().join("tools").join("vdd")
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

fn get_default_settings() -> VddSettings {
    VddSettings {
        monitors: vec![Monitor { count: 1 }],
        gpu: vec![Gpu {
            friendlyname: vec![String::new()],
        }],
        global: Global {
            g_refresh_rate: vec![60, 120, 240],
        },
        resolutions: vec![],
        colour: vec![Colour {
            sdr10bit: false,
            hdr_plus: false,
            colour_format: "RGB".to_string(),
        }],
        logging: vec![Logging {
            logging: false,
            debuglogging: false,
        }],
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
    
    // 简化的 XML 解析
    let settings: VddSettings = from_str(&content)
        .unwrap_or_else(|_| get_default_settings());
    
    Ok(settings)
}

#[tauri::command]
pub async fn save_vdd_settings(settings: VddSettings) -> Result<String, String> {
    let path = get_vdd_settings_path();
    
    // 确保目录存在
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
    // 序列化为 XML
    let xml = to_string(&settings)
        .map_err(|e| format!("序列化失败: {}", e))?;
    
    // 写入文件（需要管理员权限）
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        // 写入临时文件
        let temp_path = std::env::temp_dir().join(format!("vdd_temp_{}.xml", std::process::id()));
        fs::write(&temp_path, &xml)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        
        // 使用 PowerShell 以管理员权限复制文件
        let ps_command = format!(
            r#"Start-Process powershell -Verb RunAs -ArgumentList "-Command `"Copy-Item '{}' '{}' -Force`"" -WindowStyle Hidden"#,
            temp_path.display(),
            path.display()
        );
        
        Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output()
            .map_err(|e| format!("执行命令失败: {}", e))?;
        
        // 等待一下确保文件写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // 通知驱动重新加载
        let _ = exec_pipe_cmd("RELOAD_DRIVER".to_string()).await;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        fs::write(&path, xml)
            .map_err(|e| format!("写入文件失败: {}", e))?;
    }
    
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


