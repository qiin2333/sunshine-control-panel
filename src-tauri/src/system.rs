use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub model: String,
    pub vram: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub arch: String,
    pub platform: String,
    pub os_version: String,
    pub tauri_version: String,
    pub app_version: String,
    pub build_time: String,
    pub memory_total: Option<u64>,
    pub cpu_model: Option<String>,
}

#[tauri::command]
pub async fn get_gpus() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        use wmi::{COMLibrary, WMIConnection};
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[serde(rename = "Win32_VideoController")]
        #[serde(rename_all = "PascalCase")]
        struct VideoController {
            name: String,
            adapter_ram: Option<u64>,
        }

        let com_con = COMLibrary::new().map_err(|e| e.to_string())?;
        let wmi_con = WMIConnection::new(com_con).map_err(|e| e.to_string())?;
        
        let results: Vec<VideoController> = wmi_con
            .raw_query("SELECT Name, AdapterRAM FROM Win32_VideoController")
            .map_err(|e| e.to_string())?;
        
        let gpu_names: Vec<String> = results
            .into_iter()
            .filter(|controller| {
                controller.adapter_ram.unwrap_or(0) > 0
            })
            .map(|controller| controller.name)
            .collect();
        
        Ok(gpu_names)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec!["未知 GPU".to_string()])
    }
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    use std::env;
    
    #[cfg(target_os = "windows")]
    {
        // 基础信息
        let arch = env::consts::ARCH.to_string();
        let platform = env::consts::OS.to_string();
        let tauri_version = "2.8.5".to_string();
        let app_version = env!("CARGO_PKG_VERSION").to_string(); // 从 Cargo.toml 获取真实版本号
        let build_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        // 使用 WMI 获取 Windows 系统信息
        let (os_version, memory_total, cpu_model) = match get_windows_system_info().await {
            Ok((os_ver, mem, cpu)) => (os_ver, Some(mem), Some(cpu)),
            Err(e) => {
                eprintln!("获取 Windows 系统信息失败: {}", e);
                (format!("Windows {}", env::consts::OS), None, None)
            }
        };
        
        Ok(SystemInfo {
            arch,
            platform,
            os_version,
            tauri_version,
            app_version,
            build_time,
            memory_total,
            cpu_model,
        })
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 系统的处理
        Ok(SystemInfo {
            arch: env::consts::ARCH.to_string(),
            platform: env::consts::OS.to_string(),
            os_version: "Unknown".to_string(),
            tauri_version: "2.8.5".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(), // 从 Cargo.toml 获取真实版本号
            build_time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            memory_total: None,
            cpu_model: None,
        })
    }
}

#[cfg(target_os = "windows")]
async fn get_windows_system_info() -> Result<(String, u64, String), String> {
    use wmi::{COMLibrary, WMIConnection};
    
    #[derive(Deserialize)]
    #[serde(rename = "Win32_OperatingSystem")]
    #[serde(rename_all = "PascalCase")]
    struct OperatingSystem {
        caption: String,
        total_visible_memory_size: Option<u64>,
    }
    
    #[derive(Deserialize)]
    #[serde(rename = "Win32_Processor")]
    #[serde(rename_all = "PascalCase")]
    struct Processor {
        name: String,
    }
    
    let com_con = COMLibrary::new().map_err(|e| e.to_string())?;
    let wmi_con = WMIConnection::new(com_con).map_err(|e| e.to_string())?;
    
    // 获取操作系统信息
    let os_results: Vec<OperatingSystem> = wmi_con
        .raw_query("SELECT Caption, TotalVisibleMemorySize FROM Win32_OperatingSystem")
        .map_err(|e| e.to_string())?;
    
    let os_info = os_results.first()
        .ok_or("无法获取操作系统信息")?;
    
    let os_version = os_info.caption.clone();
    let memory_bytes = os_info.total_visible_memory_size
        .map(|kb| kb * 1024) // 转换为字节
        .unwrap_or(0);
    
    // 获取 CPU 信息
    let cpu_results: Vec<Processor> = wmi_con
        .raw_query("SELECT Name FROM Win32_Processor")
        .map_err(|e| e.to_string())?;
    
    let cpu_model = cpu_results.first()
        .map(|cpu| cpu.name.clone())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    
    Ok((os_version, memory_bytes, cpu_model))
}

#[tauri::command]
pub async fn get_current_dpi() -> Result<u32, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY
        };
        use windows::Win32::UI::HiDpi::GetDpiForMonitor;
        use windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI;
        use windows::Win32::Foundation::POINT;
        
        unsafe {
            // 获取主显示器
            let point = POINT { x: 0, y: 0 };
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY);
            
            // 获取显示器的有效 DPI
            let mut dpi_x: u32 = 0;
            let mut dpi_y: u32 = 0;
            
            match GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
                Ok(_) => {
                    // 转换为百分比（96 DPI = 100%）
                    let percentage = (dpi_x as f32 / 96.0 * 100.0).round() as u32;
                    println!("🖥️ 主显示器实时 DPI: {} x {} -> {}%", dpi_x, dpi_y, percentage);
                    Ok(percentage)
                }
                Err(e) => {
                    eprintln!("❌ 获取显示器 DPI 失败: {:?}", e);
                    
                    // 回退方案：使用系统 DPI
                    use windows::Win32::UI::HiDpi::GetDpiForSystem;
                    let dpi = GetDpiForSystem();
                    let percentage = (dpi as f32 / 96.0 * 100.0).round() as u32;
                    println!("🖥️ 回退：使用系统 DPI: {} ({}%)", dpi, percentage);
                    Ok(percentage)
                }
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(100) // 非 Windows 系统返回默认值
    }
}

#[tauri::command]
pub async fn set_desktop_dpi(dpi: u32) -> Result<(), String> {
    println!("🖥️ 设置桌面 DPI: {}%", dpi);
    
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        use crate::sunshine;
        
        // 从 Sunshine 安装目录获取路径
        let install_path = sunshine::get_sunshine_install_path();
        let setdpi_path = PathBuf::from(&install_path).join("tools").join("SetDpi.exe");
        
        println!("🔍 SetDpi.exe 路径: {:?}", setdpi_path);
        
        if setdpi_path.exists() {
            match std::process::Command::new(setdpi_path)
                .arg(dpi.to_string())
                .spawn()
            {
                Ok(_) => {
                    println!("✅ DPI 已设置为 {}%", dpi);
                    Ok(())
                }
                Err(e) => {
                    eprintln!("❌ 执行 SetDpi.exe 失败: {}", e);
                    Err(format!("执行失败: {}", e))
                }
            }
        } else {
            Err(format!("找不到 SetDpi.exe: {:?}", setdpi_path))
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("DPI 调整功能仅在 Windows 上可用".to_string())
    }
}

