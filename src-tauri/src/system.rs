use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub model: String,
    pub vram: u64,
}

/// Sunshine 进程内存使用信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessMemoryInfo {
    /// 进程名
    pub process_name: String,
    /// 进程 PID
    pub pid: u32,
    /// 工作集大小 (bytes) - 当前使用的物理内存
    pub working_set: u64,
    /// 峰值工作集 (bytes) - 历史最高物理内存
    pub peak_working_set: u64,
    /// 私有工作集 (bytes) - 不与其他进程共享的物理内存
    pub private_working_set: u64,
    /// 提交内存 (bytes) - 虚拟内存中实际使用的页面
    pub commit_size: u64,
}

/// 内存监控快照，包含所有 Sunshine 相关进程
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryMonitorSnapshot {
    /// 各进程内存详情
    pub processes: Vec<ProcessMemoryInfo>,
    /// 所有进程工作集总和 (bytes)
    pub total_working_set: u64,
    /// 所有进程私有工作集总和 (bytes)
    pub total_private_working_set: u64,
    /// 系统总物理内存 (bytes)
    pub system_total_memory: u64,
    /// 系统可用物理内存 (bytes)
    pub system_available_memory: u64,
    /// 采样时间戳 (ISO 8601)
    pub timestamp: String,
}

/// Sunshine 进程的启动时间信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessStartInfo {
    /// 进程启动时间（Unix 毫秒时间戳），0 表示未找到
    pub start_time_ms: u64,
    /// 进程名
    pub process_name: String,
    /// 进程 PID
    pub pid: u32,
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
        use serde::Deserialize;
        use wmi::WMIConnection;

        #[derive(Deserialize)]
        #[serde(rename = "Win32_VideoController")]
        #[serde(rename_all = "PascalCase")]
        struct VideoController {
            name: String,
            adapter_ram: Option<u64>,
        }

        let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;

        let results: Vec<VideoController> = wmi_con
            .raw_query("SELECT Name, AdapterRAM FROM Win32_VideoController")
            .map_err(|e| e.to_string())?;

        let gpu_names: Vec<String> = results
            .into_iter()
            .filter(|controller| controller.adapter_ram.unwrap_or(0) > 0)
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
pub async fn get_monitors() -> Result<Vec<String>, String> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let mut names = Vec::new();

    for monitor in monitors {
        let name = monitor.name().unwrap_or_default();
        let id = monitor.id().map(|id| id.to_string()).unwrap_or_default();
        let width = monitor.width().unwrap_or(0);
        let height = monitor.height().unwrap_or(0);
        let label = if !name.trim().is_empty() {
            name
        } else if !id.is_empty() {
            format!("Monitor-{}", id)
        } else {
            "Monitor".to_string()
        };

        if width > 0 && height > 0 {
            names.push(format!("{} ({}x{})", label, width, height));
        } else {
            names.push(label);
        }
    }

    names.sort();
    names.dedup();
    Ok(names)
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    use std::env;

    #[cfg(target_os = "windows")]
    {
        // 基础信息
        let arch = env::consts::ARCH.to_string();
        let platform = env::consts::OS.to_string();
        let tauri_version = tauri::VERSION.to_string();
        let app_version = env!("CARGO_PKG_VERSION").to_string(); // 从 Cargo.toml 获取真实版本号
        let build_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 使用 WMI 获取 Windows 系统信息
        let (os_version, memory_total, cpu_model) = match get_windows_system_info().await {
            Ok((os_ver, mem, cpu)) => (os_ver, Some(mem), Some(cpu)),
            Err(e) => {
                error!("获取 Windows 系统信息失败: {}", e);
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
            tauri_version: tauri::VERSION.to_string(), // 获取真实的 Tauri 版本
            app_version: env!("CARGO_PKG_VERSION").to_string(), // 从 Cargo.toml 获取真实版本号
            build_time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            memory_total: None,
            cpu_model: None,
        })
    }
}

#[cfg(target_os = "windows")]
async fn get_windows_system_info() -> Result<(String, u64, String), String> {
    use wmi::WMIConnection;

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

    let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;

    // 获取操作系统信息
    let os_results: Vec<OperatingSystem> = wmi_con
        .raw_query("SELECT Caption, TotalVisibleMemorySize FROM Win32_OperatingSystem")
        .map_err(|e| e.to_string())?;

    let os_info = os_results.first().ok_or("无法获取操作系统信息")?;

    let os_version = os_info.caption.clone();
    let memory_bytes = os_info
        .total_visible_memory_size
        .map(|kb| kb * 1024) // 转换为字节
        .unwrap_or(0);

    // 获取 CPU 信息
    let cpu_results: Vec<Processor> = wmi_con
        .raw_query("SELECT Name FROM Win32_Processor")
        .map_err(|e| e.to_string())?;

    let cpu_model = cpu_results
        .first()
        .map(|cpu| cpu.name.clone())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    Ok((os_version, memory_bytes, cpu_model))
}

#[tauri::command]
pub async fn get_current_dpi() -> Result<u32, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
        use windows::Win32::UI::HiDpi::GetDpiForMonitor;
        use windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI;

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
                    debug!(
                        "🖥️ 主显示器实时 DPI: {} x {} -> {}%",
                        dpi_x, dpi_y, percentage
                    );
                    Ok(percentage)
                }
                Err(e) => {
                    error!("❌ 获取显示器 DPI 失败: {:?}", e);

                    // 回退方案：使用系统 DPI
                    use windows::Win32::UI::HiDpi::GetDpiForSystem;
                    let dpi = GetDpiForSystem();
                    let percentage = (dpi as f32 / 96.0 * 100.0).round() as u32;
                    debug!("🖥️ 回退：使用系统 DPI: {} ({}%)", dpi, percentage);
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
    info!("🖥️ 设置桌面 DPI: {}%", dpi);

    #[cfg(target_os = "windows")]
    {
        use crate::sunshine;

        // 从 Sunshine 安装目录获取路径
        let setdpi_path = sunshine::install_dir().join("tools").join("SetDpi.exe");

        debug!("🔍 SetDpi.exe 路径: {:?}", setdpi_path);

        if setdpi_path.exists() {
            match std::process::Command::new(setdpi_path)
                .arg(dpi.to_string())
                .spawn()
            {
                Ok(_) => {
                    info!("✅ DPI 已设置为 {}%", dpi);
                    Ok(())
                }
                Err(e) => {
                    error!("❌ 执行 SetDpi.exe 失败: {}", e);
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

/// 获取 Sunshine 相关进程的内存使用信息
#[tauri::command]
pub async fn get_process_memory_info() -> Result<MemoryMonitorSnapshot, String> {
    tokio::task::spawn_blocking(get_process_memory_info_impl)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
fn get_process_memory_info_impl() -> Result<MemoryMonitorSnapshot, String> {
    // 用 tasklist 查找 Sunshine 相关进程的 PID
    let pids = find_sunshine_pids()?;

    let mut processes = Vec::new();

    for (pid, name) in &pids {
        match get_single_process_memory(*pid) {
            Ok(info) => processes.push(ProcessMemoryInfo {
                process_name: name.clone(),
                pid: *pid,
                working_set: info.0,
                peak_working_set: info.1,
                private_working_set: info.2,
                commit_size: info.3,
            }),
            Err(e) => {
                // WinAPI 失败（权限不足等），尝试 WMI 获取基础内存信息
                warn!(
                    "WinAPI 获取进程 {} (PID {}) 内存失败: {}，回退 WMI",
                    name, pid, e
                );
                if let Ok(ws) = get_process_memory_via_wmi(*pid) {
                    processes.push(ProcessMemoryInfo {
                        process_name: name.clone(),
                        pid: *pid,
                        working_set: ws,
                        peak_working_set: 0,
                        private_working_set: 0,
                        commit_size: 0,
                    });
                }
            }
        }
    }

    let total_working_set: u64 = processes.iter().map(|p| p.working_set).sum();
    let total_private_working_set: u64 = processes.iter().map(|p| p.private_working_set).sum();

    // 获取系统内存信息
    let (sys_total, sys_available) = get_system_memory_status().unwrap_or((0, 0));

    Ok(MemoryMonitorSnapshot {
        processes,
        total_working_set,
        total_private_working_set,
        system_total_memory: sys_total,
        system_available_memory: sys_available,
        timestamp: chrono::Local::now().to_rfc3339(),
    })
}

#[cfg(target_os = "windows")]
fn find_sunshine_pids() -> Result<Vec<(u32, String)>, String> {
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename = "Win32_Process")]
    #[serde(rename_all = "PascalCase")]
    struct ProcessInfo {
        name: String,
        process_id: u32,
    }

    let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;

    // 查询所有 sunshine 相关进程（不区分大小写）
    let results: Vec<ProcessInfo> = wmi_con
        .raw_query("SELECT Name, ProcessId FROM Win32_Process WHERE Name LIKE '%sunshine%'")
        .map_err(|e| e.to_string())?;

    let pids: Vec<(u32, String)> = results
        .into_iter()
        .filter(|p| {
            let lower = p.name.to_lowercase();
            // 排除 GUI 自身进程
            !lower.contains("sunshine-gui") && !lower.contains("sunshine_gui")
        })
        .map(|p| (p.process_id, p.name))
        .collect();

    debug!("找到 {} 个 Sunshine 相关进程: {:?}", pids.len(), pids);
    Ok(pids)
}

/// 通过 WMI 获取进程工作集（fallback，权限不足时使用）
#[cfg(target_os = "windows")]
fn get_process_memory_via_wmi(pid: u32) -> Result<u64, String> {
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename = "Win32_Process")]
    #[serde(rename_all = "PascalCase")]
    struct ProcMem {
        working_set_size: Option<u64>,
    }

    let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;
    let query = format!(
        "SELECT WorkingSetSize FROM Win32_Process WHERE ProcessId = {}",
        pid
    );
    let results: Vec<ProcMem> = wmi_con.raw_query(&query).map_err(|e| e.to_string())?;

    results
        .first()
        .and_then(|p| p.working_set_size)
        .ok_or_else(|| "WMI 未返回工作集数据".to_string())
}

/// 通过 Windows API 获取单个进程的精确内存信息
/// 返回 (working_set, peak_working_set, private_working_set, commit_size)
#[cfg(target_os = "windows")]
fn get_single_process_memory(pid: u32) -> Result<(u64, u64, u64, u64), String> {
    use std::mem;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .map_err(|e| format!("OpenProcess 失败: {}", e))?;

        let mut counters: PROCESS_MEMORY_COUNTERS_EX = mem::zeroed();
        counters.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;

        let result = K32GetProcessMemoryInfo(
            handle,
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
            mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        );

        let _ = CloseHandle(handle);

        if result.as_bool() {
            Ok((
                counters.WorkingSetSize as u64,
                counters.PeakWorkingSetSize as u64,
                counters.PrivateUsage as u64,
                counters.PagefileUsage as u64,
            ))
        } else {
            Err("K32GetProcessMemoryInfo 失败".to_string())
        }
    }
}

/// 获取系统总内存和可用内存
#[cfg(target_os = "windows")]
fn get_system_memory_status() -> Result<(u64, u64), String> {
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename = "Win32_OperatingSystem")]
    #[serde(rename_all = "PascalCase")]
    struct OsMemory {
        total_visible_memory_size: Option<u64>,
        free_physical_memory: Option<u64>,
    }

    let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;
    let results: Vec<OsMemory> = wmi_con
        .raw_query("SELECT TotalVisibleMemorySize, FreePhysicalMemory FROM Win32_OperatingSystem")
        .map_err(|e| e.to_string())?;

    if let Some(info) = results.first() {
        Ok((
            info.total_visible_memory_size.unwrap_or(0) * 1024, // KB → bytes
            info.free_physical_memory.unwrap_or(0) * 1024,
        ))
    } else {
        Err("无法获取系统内存信息".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn get_process_memory_info_impl() -> Result<MemoryMonitorSnapshot, String> {
    Ok(MemoryMonitorSnapshot {
        processes: vec![],
        total_working_set: 0,
        total_private_working_set: 0,
        system_total_memory: 0,
        system_available_memory: 0,
        timestamp: chrono::Local::now().to_rfc3339(),
    })
}

/// 获取 Sunshine 主进程的启动时间
#[tauri::command]
pub async fn get_sunshine_start_time() -> Result<ProcessStartInfo, String> {
    tokio::task::spawn_blocking(get_sunshine_start_time_impl)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
fn get_sunshine_start_time_impl() -> Result<ProcessStartInfo, String> {
    use wmi::WMIConnection;

    #[derive(Deserialize, Debug)]
    #[serde(rename = "Win32_Process")]
    #[serde(rename_all = "PascalCase")]
    struct ProcTime {
        name: String,
        process_id: u32,
        // WMI CIM_DATETIME 可能被 wmi crate 反序列化为多种类型
        // 使用 serde_json::Value 来灵活处理
        creation_date: Option<serde_json::Value>,
    }

    let wmi_con = WMIConnection::new().map_err(|e| e.to_string())?;

    // 查找 Sunshine 相关进程，在 Rust 端过滤 GUI 进程
    let results: Vec<ProcTime> = wmi_con
        .raw_query(
            "SELECT Name, ProcessId, CreationDate FROM Win32_Process \
             WHERE Name LIKE '%sunshine%'",
        )
        .map_err(|e| format!("WMI query failed: {}", e))?;

    debug!("WMI 进程查询结果: {:?}", results);

    // 过滤掉 GUI 自身进程，取第一个 Sunshine 主进程
    let proc = results.iter().find(|p| {
        let lower = p.name.to_lowercase();
        !lower.contains("sunshine-gui") && !lower.contains("sunshine_gui")
    });

    if let Some(proc) = proc {
        let start_ms = match &proc.creation_date {
            Some(serde_json::Value::String(s)) => {
                debug!("CreationDate (String): {}", s);
                parse_wmi_datetime(s).unwrap_or(0)
            }
            Some(serde_json::Value::Number(n)) => {
                // 可能直接是时间戳
                debug!("CreationDate (Number): {}", n);
                n.as_u64().unwrap_or(0)
            }
            Some(other) => {
                // 尝试将其他类型转为字符串后解析
                let s = other.to_string().replace('"', "");
                debug!("CreationDate (Other): {} -> {}", other, s);
                parse_wmi_datetime(&s).unwrap_or(0)
            }
            None => {
                debug!("CreationDate is None");
                0
            }
        };

        info!(
            "Sunshine 进程 {} (PID {}) 启动时间: {} ms",
            proc.name, proc.process_id, start_ms
        );

        Ok(ProcessStartInfo {
            start_time_ms: start_ms,
            process_name: proc.name.clone(),
            pid: proc.process_id,
        })
    } else {
        debug!("未找到 Sunshine 进程");
        Ok(ProcessStartInfo {
            start_time_ms: 0,
            process_name: String::new(),
            pid: 0,
        })
    }
}

/// 解析 WMI datetime 格式 "20260413175200.123456+480" → Unix 毫秒时间戳
#[cfg(target_os = "windows")]
fn parse_wmi_datetime(wmi_dt: &str) -> Option<u64> {
    // WMI datetime format: "YYYYMMDDHHmmss.ffffff±UUU"
    // 例: "20260413175200.123456+480"
    if wmi_dt.len() < 14 {
        return None;
    }

    let year: i32 = wmi_dt[0..4].parse().ok()?;
    let month: u32 = wmi_dt[4..6].parse().ok()?;
    let day: u32 = wmi_dt[6..8].parse().ok()?;
    let hour: u32 = wmi_dt[8..10].parse().ok()?;
    let min: u32 = wmi_dt[10..12].parse().ok()?;
    let sec: u32 = wmi_dt[12..14].parse().ok()?;

    // 获取 UTC 偏移分钟
    let offset_minutes: i32 = if let Some(pos) = wmi_dt.find('+') {
        wmi_dt[pos + 1..].parse().unwrap_or(0)
    } else if let Some(pos) = wmi_dt.rfind('-') {
        if pos > 14 {
            -(wmi_dt[pos + 1..].parse::<i32>().unwrap_or(0))
        } else {
            0
        }
    } else {
        0
    };

    use chrono::{FixedOffset, TimeZone};
    let offset = FixedOffset::east_opt(offset_minutes * 60)?;
    let dt = offset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()?;

    Some(dt.timestamp_millis() as u64)
}

#[cfg(not(target_os = "windows"))]
fn get_sunshine_start_time_impl() -> Result<ProcessStartInfo, String> {
    Ok(ProcessStartInfo {
        start_time_ms: 0,
        process_name: String::new(),
        pid: 0,
    })
}
