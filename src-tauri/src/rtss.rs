// RTSS (RivaTuner Statistics Server) 集成模块
// 提供帧率限制控制、OSD 文字显示和实时监控功能

use log::{debug, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Mutex;

// ─── Busy flag RAII guard ─────────────────────────────────────

/// RAII guard: 进入 unsafe 区域时置 dwBusy=1，离开（含 panic）时自动置 0
#[cfg(target_os = "windows")]
struct BusyGuard {
    busy_ptr: *mut u32,
}

#[cfg(target_os = "windows")]
impl BusyGuard {
    /// # Safety
    /// `busy_ptr` 必须指向 RTSS 共享内存中有效的 `dwBusy` 字段。
    unsafe fn new(busy_ptr: *mut u32) -> Self {
        unsafe { std::ptr::write_volatile(busy_ptr, 1) };
        Self { busy_ptr }
    }
}

#[cfg(target_os = "windows")]
impl Drop for BusyGuard {
    fn drop(&mut self) {
        unsafe {
            std::ptr::write_volatile(self.busy_ptr, 0);
        }
    }
}

// ─── RTSS 共享内存结构 ────────────────────────────────────────

const RTSS_SHARED_MEMORY_SIGNATURE: u32 = 0x52545353; // 'RTSS'
const RTSS_OSD_TEXT_OFFSET: usize = 0;
const RTSS_OSD_TEXT_LEN: usize = 256;
const RTSS_OSD_OWNER_OFFSET: usize = 256;
const RTSS_OSD_OWNER_LEN: usize = 256;

/// RTSS 共享内存头部 (映射部分关键字段)
#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct RtssSharedMemoryHeader {
    dwSignature: u32,
    dwVersion: u32,
    dwAppEntrySize: u32,
    dwAppArrOffset: u32,
    dwAppArrSize: u32,
    dwOSDEntrySize: u32,
    dwOSDArrOffset: u32,
    dwOSDArrSize: u32,
    dwBusy: u32,
}

// ─── 返回给前端的数据结构 ────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RtssStatus {
    pub running: bool,
    pub version: String,
    pub osd_slot_count: u32,
    pub app_count: u32,
    pub cli_path: String,
    pub hooks_dll_path: String,
}

// ─── OSD 监控配置 ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitoringConfig {
    /// 更新间隔（毫秒）
    pub interval_ms: u64,
    /// 启用的指标列表
    pub metrics: Vec<String>,
    /// 标题颜色 (RRGGBB)
    pub title_color: String,
    /// 标签颜色 (RRGGBB)
    pub label_color: String,
    /// 数值颜色 (RRGGBB)
    pub value_color: String,
    /// 字号
    pub font_size: u32,
    /// 自定义头部文本
    pub header_text: String,
    /// CJK 矢量字体名（为空则不启用，推荐 "Microsoft YaHei"）
    #[serde(default)]
    pub cjk_font: String,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            metrics: vec![
                "session_state".into(),
                "stream_fps".into(),
                "stream_bitrate".into(),
            ],
            title_color: "FFD700".into(),
            label_color: "AAAAAA".into(),
            value_color: "00FF00".into(),
            font_size: 0, // 0 = RTSS 默认
            header_text: "☀ Foundation Sunshine".into(),
            cjk_font: String::new(),
        }
    }
}

/// 可用的监控指标定义
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetricDef {
    pub id: String,
    pub label_zh: String,
    pub label_en: String,
    pub group: String,
}

/// 监控快照数据 (返回给前端)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MonitoringSnapshot {
    pub active: bool,
    pub osd_text: String,
    pub metrics: std::collections::HashMap<String, String>,
}

// 全局监控状态
static MONITORING_ACTIVE: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// CPU 采样历史 (prev_cpu_time, prev_sys_time)，用于计算瞬时 CPU 使用率
#[cfg(target_os = "windows")]
static PREV_CPU_SAMPLE: Lazy<std::sync::Mutex<std::collections::HashMap<u32, (u64, u64)>>> =
    Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static MONITORING_SNAPSHOT: Lazy<Arc<Mutex<MonitoringSnapshot>>> =
    Lazy::new(|| Arc::new(Mutex::new(MonitoringSnapshot::default())));

// ─── Windows 平台：共享内存 RAII 封装 ────────────────────────

#[cfg(target_os = "windows")]
mod win {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Memory::*;
    use windows::core::*;
    use std::ffi::CString;

    /// RAII 共享内存句柄
    pub struct SharedMemoryHandle {
        pub ptr: *mut std::ffi::c_void,
        handle: HANDLE,
    }

    impl SharedMemoryHandle {
        pub fn open() -> std::result::Result<Self, String> {
            unsafe {
                let name = CString::new("RTSSSharedMemoryV2").unwrap();
                let handle = OpenFileMappingA(
                    FILE_MAP_READ.0 | FILE_MAP_WRITE.0,
                    false,
                    PCSTR(name.as_ptr() as *const u8),
                ).map_err(|e| format!("无法打开 RTSS 共享内存: {} (RTSS 可能未运行)", e))?;

                let view = MapViewOfFile(
                    handle,
                    FILE_MAP_READ | FILE_MAP_WRITE,
                    0,
                    0,
                    0,
                );

                if view.Value.is_null() {
                    let _ = CloseHandle(handle);
                    return Err("映射 RTSS 共享内存失败".to_string());
                }

                Ok(Self { ptr: view.Value, handle })
            }
        }
    }

    impl Drop for SharedMemoryHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.ptr });
                let _ = CloseHandle(self.handle);
            }
        }
    }

    unsafe impl Send for SharedMemoryHandle {}
}

// ─── 辅助函数 ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn detect_rtss_install_dir() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let paths = [
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Unwinder\RTSS"),
        (HKEY_LOCAL_MACHINE, r"Software\Unwinder\RTSS"),
    ];

    for (root, path) in &paths {
        if let Ok(key) = RegKey::predef(*root).open_subkey(path) {
            if let Ok(dir) = key.get_value::<String, _>("InstallDir") {
                if std::path::Path::new(&dir).exists() {
                    return Some(dir);
                }
            }
        }
    }

    let common_paths = [
        r"C:\Program Files (x86)\RivaTuner Statistics Server",
        r"C:\Program Files\RivaTuner Statistics Server",
    ];
    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn detect_rtss_install_dir() -> Option<String> {
    None
}

/// 执行 rtss-cli 命令
#[cfg(target_os = "windows")]
fn run_rtss_cli(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;

    let cli_path = get_rtss_cli_path()?;

    debug!("🎯 rtss-cli {}", args.join(" "));

    let output = std::process::Command::new(&cli_path)
        .args(args)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("执行 rtss-cli 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("rtss-cli 错误: {} {}", stdout, stderr));
    }

    Ok(stdout)
}

/// 获取 rtss-cli.exe 完整路径
fn get_rtss_cli_path() -> Result<std::path::PathBuf, String> {
    let install_dir = detect_rtss_install_dir()
        .ok_or("未检测到 RTSS 安装路径")?;
    let cli_path = std::path::Path::new(&install_dir).join("rtss-cli.exe");
    if !cli_path.exists() {
        return Err(format!(
            "未找到 rtss-cli.exe，请下载并放入 RTSS 安装目录: {}",
            install_dir
        ));
    }
    Ok(cli_path)
}

/// 获取 RTSS profile 文件路径
#[cfg(target_os = "windows")]
fn get_rtss_profile_path(profile: &str) -> Result<std::path::PathBuf, String> {
    let install_dir = detect_rtss_install_dir()
        .ok_or("未检测到 RTSS 安装路径")?;
    let path = std::path::Path::new(&install_dir).join("Profiles").join(profile);
    if !path.exists() {
        return Err(format!("RTSS profile 不存在: {}", path.display()));
    }
    Ok(path)
}

/// 读取 INI 文件中的值
#[cfg(target_os = "windows")]
fn get_ini_value(path: &std::path::Path, section: &str, key: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取 profile 失败: {}", e))?;

    let section_header = format!("[{}]", section);
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some(eq_pos) = trimmed.find('=') {
                let k = trimmed[..eq_pos].trim();
                if k == key {
                    return Ok(trimmed[eq_pos + 1..].trim().to_string());
                }
            }
        }
    }

    Err(format!("在 [{}] 中未找到 {} 键", section, key))
}

/// 修改 INI 文件中的值（支持 section）
#[cfg(target_os = "windows")]
fn set_ini_value(path: &std::path::Path, section: &str, key: &str, value: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取 profile 失败: {}", e))?;

    let section_header = format!("[{}]", section);
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut in_section = false;
    let mut found = false;

    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some(eq_pos) = trimmed.find('=') {
                let k = trimmed[..eq_pos].trim();
                if k == key {
                    *line = format!("{}={}", key, value);
                    found = true;
                    break;
                }
            }
        }
    }

    if !found {
        return Err(format!("在 [{}] 中未找到 {} 键", section, key));
    }

    let new_content = lines.join("\r\n") + "\r\n";

    // 尝试普通权限写入
    match std::fs::write(path, &new_content) {
        Ok(_) => {
            info!("🎯 RTSS profile 已更新: [{}] {}={}", section, key, value);
            Ok(())
        }
        Err(_) => {
            // 使用 PowerShell 提权写入
            info!("🎯 普通权限写入失败, 尝试管理员权限...");
            let tmp = std::env::temp_dir().join("rtss_profile_tmp");
            std::fs::write(&tmp, &new_content)
                .map_err(|e| format!("写入临时文件失败: {}", e))?;

            let ps_cmd = format!(
                "Copy-Item -Path '{}' -Destination '{}' -Force",
                tmp.display(), path.display()
            );
            let output = std::process::Command::new("powershell")
                .args(["-Command", &format!(
                    "Start-Process powershell -Verb RunAs -WindowStyle Hidden -ArgumentList '-Command {}' -Wait",
                    ps_cmd.replace('\'', "''")
                )])
                .output()
                .map_err(|e| format!("提权写入失败: {}", e))?;

            let _ = std::fs::remove_file(&tmp);

            if output.status.success() {
                info!("🎯 RTSS profile 管理员权限更新成功");
                Ok(())
            } else {
                Err("写入 RTSS profile 失败: 需要管理员权限".to_string())
            }
        }
    }
}

/// 以管理员权限执行 rtss-cli 并捕获 stdout
#[cfg(target_os = "windows")]
fn run_rtss_cli_elevated(args: &[&str]) -> Result<String, String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::PCWSTR;

    let cli_path = get_rtss_cli_path()?;

    // 通过临时文件捕获 stdout；先清理上次可能残留的旧文件
    let tmp_out = std::env::temp_dir().join("rtss_cli_elevated_out.txt");
    let _ = std::fs::remove_file(&tmp_out);
    // cmd /c "rtss-cli.exe arg1 arg2 > tmpfile 2>&1"
    let args_str = args.iter()
        .map(|a| if a.contains(' ') { format!("\"{}\"", a) } else { a.to_string() })
        .collect::<Vec<_>>().join(" ");
    let cmd_line = format!(
        "/c \"\"{}\" {} > \"{}\" 2>&1\"",
        cli_path.display(), args_str, tmp_out.display()
    );

    debug!("🎯 rtss-cli elevated: cmd {}", cmd_line);

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
    let params: Vec<u16> = format!("{}\0", cmd_line).encode_utf16().collect();

    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            SW_HIDE,
        );
        let code = result.0 as usize;
        if code <= 32 {
            return Err(format!("管理员权限执行失败 (code={})", code));
        }
    }

    // 等待执行完成（检测输出文件）
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if tmp_out.exists() {
            if let Ok(content) = std::fs::read_to_string(&tmp_out) {
                if !content.is_empty() {
                    let _ = std::fs::remove_file(&tmp_out);
                    return Ok(content.trim().to_string());
                }
            }
        }
    }

    // 超时后尝试最后一次读取
    if tmp_out.exists() {
        let content = std::fs::read_to_string(&tmp_out).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp_out);
        if !content.is_empty() {
            return Ok(content.trim().to_string());
        }
    }

    Err("管理员权限执行超时".to_string())
}

// ─── RTSS 状态查询 ─────────────────────────────────────────

/// 获取 RTSS 运行状态
#[tauri::command]
pub async fn get_rtss_status() -> Result<RtssStatus, String> {
    #[cfg(target_os = "windows")]
    {
        let install_dir = detect_rtss_install_dir().unwrap_or_default();

        let cli_path = if !install_dir.is_empty() {
            let p = std::path::Path::new(&install_dir).join("rtss-cli.exe");
            if p.exists() { p.to_string_lossy().to_string() } else { String::new() }
        } else {
            String::new()
        };

        let hooks_dll_path = if !install_dir.is_empty() {
            let p = std::path::Path::new(&install_dir).join("RTSSHooks64.dll");
            if p.exists() { p.to_string_lossy().to_string() } else { String::new() }
        } else {
            String::new()
        };

        let (running, version, osd_slot_count, app_count) = match win::SharedMemoryHandle::open() {
            Ok(shm) => {
                let header = unsafe { &*(shm.ptr as *const RtssSharedMemoryHeader) };
                let sig_ok = header.dwSignature == RTSS_SHARED_MEMORY_SIGNATURE;
                let ver = if sig_ok {
                    format!("{}.{}", header.dwVersion >> 16, header.dwVersion & 0xFFFF)
                } else {
                    String::new()
                };
                let osd_count = if sig_ok && header.dwOSDEntrySize > 0 {
                    header.dwOSDArrSize
                } else {
                    0
                };
                let app_c = if sig_ok && header.dwAppEntrySize > 0 {
                    header.dwAppArrSize
                } else {
                    0
                };
                (sig_ok, ver, osd_count, app_c)
            }
            Err(_) => (false, String::new(), 0, 0),
        };

        Ok(RtssStatus { running, version, osd_slot_count, app_count, cli_path, hooks_dll_path })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(RtssStatus {
            running: false, version: String::new(), osd_slot_count: 0,
            app_count: 0, cli_path: String::new(), hooks_dll_path: String::new(),
        })
    }
}

// ─── OSD 写入与释放 ─────────────────────────────────────────

/// 释放指定 owner 的所有 OSD 槽位（零填整个 entry，包括 owner）
#[cfg(target_os = "windows")]
fn release_osd_slots(owner_name: &str) -> Result<u32, String> {
    let shm = win::SharedMemoryHandle::open()?;
    let mut released = 0u32;

    unsafe {
        let header_ptr = shm.ptr as *mut RtssSharedMemoryHeader;
        let header = &*header_ptr;

        if header.dwSignature != RTSS_SHARED_MEMORY_SIGNATURE {
            return Err("RTSS 共享内存签名不匹配".to_string());
        }

        let entry_size = header.dwOSDEntrySize as usize;
        if entry_size == 0 { return Ok(0); }
        let osd_arr_base = (shm.ptr as *mut u8).add(header.dwOSDArrOffset as usize);
        let owner_offset = RTSS_OSD_OWNER_OFFSET.min(entry_size);
        let owner_field_len = RTSS_OSD_OWNER_LEN.min(entry_size.saturating_sub(owner_offset));
        let text_field_len = RTSS_OSD_TEXT_LEN.min(entry_size);

        // RAII busy guard: drop 时自动清 busy flag（含 panic）
        let _busy = BusyGuard::new(&mut (*header_ptr).dwBusy);

        for i in 0..header.dwOSDArrSize {
            let entry_ptr = osd_arr_base.add((i as usize) * entry_size);
            let owner_ptr = entry_ptr.add(owner_offset);

            let first_byte = std::ptr::read_volatile(owner_ptr);
            if first_byte == 0 { continue; }

            let owner_slice = std::slice::from_raw_parts(owner_ptr, owner_field_len);
            let nul_pos = owner_slice.iter().position(|&b| b == 0).unwrap_or(owner_field_len.saturating_sub(1));
            let existing = String::from_utf8_lossy(&owner_slice[..nul_pos]);

            if existing == owner_name {
                // 仅清理 text/owner 区，避免破坏 RTSS entry 其余元数据/缓冲区
                std::ptr::write_bytes(entry_ptr.add(RTSS_OSD_TEXT_OFFSET), 0, text_field_len);
                if owner_field_len > 0 {
                    std::ptr::write_bytes(entry_ptr.add(owner_offset), 0, owner_field_len);
                }
                released += 1;
                debug!("🎯 RTSS OSD slot {} released (owner: {})", i, owner_name);
            }
        }

        // busy flag 由 _busy guard 的 Drop 自动清除
    }

    if released > 0 {
        info!("✅ Released {} RTSS OSD slot(s) for '{}'", released, owner_name);
    }
    Ok(released)
}

/// 写入 OSD 文本到 RTSS 共享内存
#[tauri::command]
pub async fn rtss_set_osd(text: String, owner: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let owner_name = owner.unwrap_or_else(|| "Sunshine".to_string());

        // 空文本 → 释放所有该 owner 的 OSD 槽位
        if text.is_empty() {
            return release_osd_slots(&owner_name).map(|_| ());
        }

        let shm = win::SharedMemoryHandle::open()?;

        unsafe {
            let header_ptr = shm.ptr as *mut RtssSharedMemoryHeader;
            let header = &*header_ptr;

            if header.dwSignature != RTSS_SHARED_MEMORY_SIGNATURE {
                return Err("RTSS 共享内存签名不匹配".to_string());
            }

            let osd_arr_base = (shm.ptr as *mut u8).add(header.dwOSDArrOffset as usize);
            let osd_count = if header.dwOSDEntrySize > 0 {
                header.dwOSDArrSize
            } else {
                0
            };

            if osd_count == 0 {
                return Err("RTSS OSD 槽位数量为 0".to_string());
            }

            let owner_cstr = CString::new(owner_name.as_str())
                .map_err(|e| format!("owner 名称含非法 NUL 字节: {}", e))?;
            let text_cstr = CString::new(text.as_str())
                .map_err(|e| format!("OSD 文本含非法 NUL 字节: {}", e))?;

            let entry_size = header.dwOSDEntrySize as usize;
            let text_offset = RTSS_OSD_TEXT_OFFSET.min(entry_size);
            let text_field_len = RTSS_OSD_TEXT_LEN.min(entry_size.saturating_sub(text_offset));
            let owner_offset = RTSS_OSD_OWNER_OFFSET.min(entry_size);
            let owner_field_len = RTSS_OSD_OWNER_LEN.min(entry_size.saturating_sub(owner_offset));

            // ── 扫描槽位：找到已有的、记录空闲的、清理重复的 ──
            let mut target_slot: Option<(u32, *mut u8)> = None;
            let mut empty_slots: Vec<(u32, *mut u8)> = Vec::new();
            let mut last_foreign_occupied_idx: Option<u32> = None;

            // RAII busy guard: drop 时自动清 busy flag（含 panic）
            let _busy = BusyGuard::new(&mut (*header_ptr).dwBusy);

            for i in 0..osd_count {
                let entry_ptr = osd_arr_base.add((i as usize) * entry_size);
                let osd_owner_ptr = entry_ptr.add(owner_offset);

                let text_first_byte = std::ptr::read_volatile(entry_ptr.add(text_offset));
                let first_byte = std::ptr::read_volatile(osd_owner_ptr);
                if first_byte == 0 && text_first_byte == 0 {
                    empty_slots.push((i, entry_ptr));
                    continue;
                }

                let existing_owner = if first_byte != 0 {
                    let owner_slice = std::slice::from_raw_parts(osd_owner_ptr, owner_field_len);
                    let nul_pos = owner_slice.iter().position(|&b| b == 0).unwrap_or(owner_field_len.saturating_sub(1));
                    String::from_utf8_lossy(&owner_slice[..nul_pos]).to_string()
                } else {
                    String::new()
                };

                if !existing_owner.is_empty() && existing_owner == owner_name.as_str() {
                    if target_slot.is_none() {
                        // 第一个匹配的 → 复用
                        target_slot = Some((i, entry_ptr));
                    } else {
                        // 后续重复的 → 仅清理 text/owner 区释放
                        std::ptr::write_bytes(entry_ptr.add(text_offset), 0, text_field_len);
                        if owner_field_len > 0 {
                            std::ptr::write_bytes(entry_ptr.add(owner_offset), 0, owner_field_len);
                        }
                        debug!("🎯 cleaned up duplicate OSD slot {} for '{}'", i, owner_name);
                    }
                    continue;
                }

                last_foreign_occupied_idx = Some(i);
            }

            let preferred_empty_slot = if let Some(last_idx) = last_foreign_occupied_idx {
                empty_slots.iter().find(|(idx, _)| *idx > last_idx).copied()
            } else {
                empty_slots.first().copied()
            };

            let fallback_empty_slot = empty_slots.first().copied();

            let (slot_idx, slot) = target_slot
                .or(preferred_empty_slot)
                .or(fallback_empty_slot)
                .ok_or_else(|| {
                    // busy flag 由 _busy guard 的 Drop 自动清除
                    format!("没有可用的 RTSS OSD 槽位（共 {} 个均被占用）", osd_count)
                })?;

            let text_bytes = text_cstr.as_bytes_with_nul();
            let owner_bytes = owner_cstr.as_bytes_with_nul();
            let text_max = text_field_len;

            // 只清理 text/owner 区，避免伤及 RTSS entry 其余数据
            std::ptr::write_bytes(slot.add(text_offset), 0, text_field_len);
            if owner_field_len > 0 {
                std::ptr::write_bytes(slot.add(owner_offset), 0, owner_field_len);
            }
            std::ptr::copy_nonoverlapping(
                text_bytes.as_ptr(),
                slot.add(text_offset),
                text_bytes.len().min(text_max.saturating_sub(1).max(1))
            );

            // 写入 Owner
            let owner_ptr = slot.add(owner_offset);
            if owner_field_len > 0 {
                std::ptr::copy_nonoverlapping(
                    owner_bytes.as_ptr(),
                    owner_ptr,
                    owner_bytes.len().min(owner_field_len.saturating_sub(1).max(1))
                );
            }

            // busy flag 由 _busy guard 的 Drop 自动清除

            debug!("🎯 RTSS OSD using slot {} (last foreign occupied: {:?})", slot_idx, last_foreign_occupied_idx);
        }

        debug!("✅ RTSS OSD updated");
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 清除 Sunshine 的 OSD 文本（释放槽位）
#[tauri::command]
pub async fn rtss_clear_osd(owner: Option<String>) -> Result<(), String> {
    rtss_set_osd(String::new(), owner).await
}

// ─── 帧率限制与 OSD 开关 ────────────────────────────────────

/// 通过 rtss-cli 切换功能（try normal → verify → try elevated → verify）
#[cfg(target_os = "windows")]
fn toggle_rtss_feature(feature: &str, label: &str) -> Result<String, String> {
    let get_cmd = format!("{}:get", feature);
    let toggle_cmd = format!("{}:toggle", feature);

    let before = run_rtss_cli(&[&get_cmd])?
        .parse::<i32>().unwrap_or(-1);

    let result = run_rtss_cli(&[&toggle_cmd])?;

    let after = run_rtss_cli(&[&get_cmd])?
        .parse::<i32>().unwrap_or(-1);

    if before != after {
        info!("🎯 RTSS {}: {}", label, if after == 1 { "启用" } else { "禁用" });
        return Ok(after.to_string());
    }

    info!("🎯 普通权限切换 {} 失败, 尝试管理员权限...", label);
    run_rtss_cli_elevated(&[&toggle_cmd])?;

    std::thread::sleep(std::time::Duration::from_millis(200));
    let final_state = run_rtss_cli(&[&get_cmd])?
        .parse::<i32>().unwrap_or(-1);

    if before != final_state {
        info!("🎯 RTSS {} (提升权限): {}", label, if final_state == 1 { "启用" } else { "禁用" });
        Ok(final_state.to_string())
    } else {
        Ok(result)
    }
}

/// 通过 rtss-cli 设置帧率限制
#[tauri::command]
pub async fn rtss_set_framerate_limit(fps: i32, profile: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let is_global = profile.as_ref().map_or(true, |p| p.is_empty() || p.eq_ignore_ascii_case("global"));

        let set_args: Vec<String> = if is_global {
            vec!["limit:set".into(), fps.to_string()]
        } else {
            vec!["property:set".into(), profile.clone().unwrap(), "FramerateLimit".into(), fps.to_string()]
        };
        let get_args: Vec<String> = if is_global {
            vec!["limit:get".into()]
        } else {
            vec!["property:get".into(), profile.unwrap(), "FramerateLimit".into()]
        };

        info!("🎯 RTSS 设置帧率限制: {} (global: {})", fps, is_global);

        // 先尝试普通权限
        let set_refs: Vec<&str> = set_args.iter().map(|s| s.as_str()).collect();
        run_rtss_cli(&set_refs)?;

        // 验证是否生效
        let get_refs: Vec<&str> = get_args.iter().map(|s| s.as_str()).collect();
        let actual = run_rtss_cli(&get_refs)?
            .parse::<i32>().unwrap_or(-1);

        if actual == fps {
            return Ok("OK".to_string());
        }

        // 验证失败 → 权限不足，尝试提升权限执行
        info!("🎯 普通权限设置失败 (期望={}, 实际={}), 尝试管理员权限...", fps, actual);
        run_rtss_cli_elevated(&set_refs)?;

        // 再次验证
        std::thread::sleep(std::time::Duration::from_millis(200));
        let actual2 = run_rtss_cli(&get_refs)?
            .parse::<i32>().unwrap_or(-1);

        if actual2 == fps {
            Ok("OK".to_string())
        } else {
            Err(format!("帧率限制设置失败: 需要管理员权限 (期望={}, 实际={})", fps, actual2))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 获取当前全局帧率限制值
#[tauri::command]
pub async fn rtss_get_framerate_limit(profile: Option<String>) -> Result<i32, String> {
    #[cfg(target_os = "windows")]
    {
        let args: Vec<String> = if let Some(ref p) = profile {
            if p.is_empty() || p.eq_ignore_ascii_case("global") {
                vec!["limit:get".into()]
            } else {
                vec!["property:get".into(), p.clone(), "FramerateLimit".into()]
            }
        } else {
            vec!["limit:get".into()]
        };

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let stdout = run_rtss_cli(&arg_refs)?;
        stdout.parse::<i32>().map_err(|_| format!("无法解析帧率值: '{}'", stdout))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 切换 RTSS 帧率限制器启用/禁用
#[tauri::command]
pub async fn rtss_toggle_limiter() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    { toggle_rtss_feature("limiter", "帧率限制器") }

    #[cfg(not(target_os = "windows"))]
    { Err("RTSS 仅在 Windows 上可用".to_string()) }
}

/// 获取 RTSS 帧率限制器当前状态 (1=启用, 0=禁用)
#[tauri::command]
pub async fn rtss_get_limiter_status() -> Result<i32, String> {
    #[cfg(target_os = "windows")]
    {
        let stdout = run_rtss_cli(&["limiter:get"])?;
        stdout.parse::<i32>().map_err(|_| format!("无法解析限制器状态: '{}'", stdout))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 切换 RTSS OSD 显示/隐藏
#[tauri::command]
pub async fn rtss_toggle_overlay() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    { toggle_rtss_feature("overlay", "OSD") }

    #[cfg(not(target_os = "windows"))]
    { Err("RTSS 仅在 Windows 上可用".to_string()) }
}

// ─── 自动下载 rtss-cli ────────────────────────────────────

const RTSS_CLI_GITHUB_API: &str =
    "https://api.github.com/repos/xanderfrangos/rtss-cli/releases/latest";

/// 自动下载 rtss-cli.exe 到 RTSS 安装目录
#[tauri::command]
pub async fn rtss_download_cli() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let install_dir =
            detect_rtss_install_dir().ok_or("未检测到 RTSS 安装路径，请先安装 RTSS")?;

        let dest = std::path::Path::new(&install_dir).join("rtss-cli.exe");
        if dest.exists() {
            return Ok(format!("rtss-cli.exe 已存在: {}", dest.display()));
        }

        info!("🎯 正在从 GitHub 下载 rtss-cli.exe ...");

        // 1. 查询最新 release 获取下载 URL
        let client = reqwest::Client::builder()
            .user_agent("Sunshine-Control-Panel")
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let release: serde_json::Value = client
            .get(RTSS_CLI_GITHUB_API)
            .send()
            .await
            .map_err(|e| format!("请求 GitHub API 失败: {}", e))?
            .json()
            .await
            .map_err(|e| format!("解析 GitHub 响应失败: {}", e))?;

        let download_url = release["assets"]
            .as_array()
            .and_then(|assets| {
                assets.iter().find(|a| {
                    a["name"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case("rtss-cli.exe"))
                })
            })
            .and_then(|a| a["browser_download_url"].as_str())
            .ok_or("未在 GitHub Release 中找到 rtss-cli.exe 资产")?
            .to_string();

        // 2. 下载到临时文件
        let bytes = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| format!("下载 rtss-cli.exe 失败: {}", e))?
            .bytes()
            .await
            .map_err(|e| format!("读取下载内容失败: {}", e))?;

        if bytes.len() < 1024 {
            return Err("下载的文件太小，可能不是有效的可执行文件".into());
        }

        // 3. 先尝试直接写入
        if std::fs::write(&dest, &bytes).is_ok() {
            info!("🎯 rtss-cli.exe 下载完成: {} ({} bytes)", dest.display(), bytes.len());
            return Ok(dest.to_string_lossy().to_string());
        }

        // 4. 直接写入失败（权限不足），用提权方式复制
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("rtss-cli.exe");
        std::fs::write(&temp_path, &bytes)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;

        // 用 cmd /c copy 以管理员权限复制
        let params = format!(
            "/c copy /Y \"{}\" \"{}\"",
            temp_path.display(),
            dest.display()
        );

        info!("🎯 需要管理员权限，提权复制 rtss-cli.exe ...");

        let result = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("runas"),
                &HSTRING::from("cmd.exe"),
                &HSTRING::from(&params),
                None,
                SW_HIDE,
            )
        };

        // ShellExecuteW 返回值 > 32 表示成功
        let hinstance_val = result.0 as isize;
        if hinstance_val <= 32 {
            // 清理临时文件
            let _ = std::fs::remove_file(&temp_path);
            return Err("用户取消了管理员权限请求，或提权失败".into());
        }

        // 等待文件出现（提权命令是异步的）
        for _ in 0..30 {
            if dest.exists() {
                let _ = std::fs::remove_file(&temp_path);
                info!("🎯 rtss-cli.exe 提权复制完成: {}", dest.display());
                return Ok(dest.to_string_lossy().to_string());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        let _ = std::fs::remove_file(&temp_path);
        Err("提权复制超时，请手动复制 rtss-cli.exe 到 RTSS 目录".into())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

// ─── 监控指标定义 ──────────────────────────────────────────

/// 获取所有可用的监控指标定义
#[tauri::command]
pub async fn rtss_get_available_metrics() -> Vec<MetricDef> {
    vec![
        // 串流会话
        MetricDef {
            id: "session_state".into(), label_zh: "会话状态".into(),
            label_en: "Session".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_client".into(), label_zh: "客户端".into(),
            label_en: "Client".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_resolution".into(), label_zh: "分辨率".into(),
            label_en: "Resolution".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_fps".into(), label_zh: "串流帧率".into(),
            label_en: "Stream FPS".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_bitrate".into(), label_zh: "码率".into(),
            label_en: "Bitrate".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_codec".into(), label_zh: "编码格式".into(),
            label_en: "Codec".into(), group: "session".into(),
        },
        MetricDef {
            id: "stream_hdr".into(), label_zh: "HDR".into(),
            label_en: "HDR".into(), group: "session".into(),
        },
        MetricDef {
            id: "app_name".into(), label_zh: "应用名称".into(),
            label_en: "App".into(), group: "session".into(),
        },
        // 进程性能
        MetricDef {
            id: "process_cpu".into(), label_zh: "CPU 占用".into(),
            label_en: "CPU".into(), group: "process".into(),
        },
        MetricDef {
            id: "process_mem".into(), label_zh: "内存占用".into(),
            label_en: "Memory".into(), group: "process".into(),
        },
        MetricDef {
            id: "process_threads".into(), label_zh: "线程数".into(),
            label_en: "Threads".into(), group: "process".into(),
        },
    ]
}

// ─── 监控后台任务 ──────────────────────────────────────────

/// Sunshine 进程信息
#[cfg(target_os = "windows")]
struct ProcessStats {
    cpu_percent: f64,
    mem_mb: f64,
    thread_count: u32,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct SunshineProcessRef {
    pid: u32,
    name: String,
}

/// 获取 Sunshine 进程统计
#[cfg(target_os = "windows")]
fn get_sunshine_process_stats() -> Option<ProcessStats> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use std::mem;

    let processes = find_sunshine_processes();
    if processes.is_empty() {
        debug!("⚠ no Sunshine process found via WMI or ToolHelp");
        return None;
    }

    debug!("📊 aggregating process stats from: {:?}", processes);

    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0);

    let mut total_cpu_percent = 0.0;
    let mut total_mem_mb = 0.0;
    let mut total_thread_count = 0u32;
    let mut sampled_any = false;
    let current_pids: std::collections::HashSet<u32> =
        processes.iter().map(|proc| proc.pid).collect();

    let mut prev_samples = PREV_CPU_SAMPLE.lock().unwrap();

    for process in &processes {
        total_thread_count += count_process_threads(process.pid);

        unsafe {
            // Sunshine 可能同时有 GUI / 服务 / 会话进程，权限拿不到时也尽量保留内存和线程数据
            let handle_result = if let Ok(h) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                false,
                process.pid,
            ) {
                debug!("📊 OpenProcess OK (full access): {} ({})", process.name, process.pid);
                Some((h, true))
            } else if let Ok(h) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                process.pid,
            ) {
                debug!("📊 OpenProcess OK (limited): {} ({})", process.name, process.pid);
                Some((h, false))
            } else {
                debug!("⚠ OpenProcess failed: {} ({})", process.name, process.pid);
                None
            };

            if let Some((handle, can_read_mem)) = handle_result {
                sampled_any = true;

                let mut creation = FILETIME::default();
                let mut exit = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();

                if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).is_ok() {
                    let kernel_time = filetime_to_u64(&kernel);
                    let user_time = filetime_to_u64(&user);
                    let total_cpu_time = kernel_time + user_time;

                    let sys_time = GetSystemTimeAsFileTime();
                    let sys_now = filetime_to_u64(&sys_time);

                    let cpu_percent = if let Some((prev_cpu, prev_sys)) = prev_samples.get(&process.pid).copied() {
                        let cpu_delta = total_cpu_time.saturating_sub(prev_cpu);
                        let sys_delta = sys_now.saturating_sub(prev_sys);
                        if sys_delta > 0 {
                            (cpu_delta as f64 / sys_delta as f64 * 100.0 / num_cpus).max(0.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    prev_samples.insert(process.pid, (total_cpu_time, sys_now));
                    total_cpu_percent += cpu_percent;
                }

                let mem_mb = if can_read_mem {
                    let mut pmc: PROCESS_MEMORY_COUNTERS = mem::zeroed();
                    pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
                    if GetProcessMemoryInfo(
                        handle,
                        &mut pmc,
                        mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                    ).is_ok() {
                        pmc.WorkingSetSize as f64 / 1024.0 / 1024.0
                    } else {
                        get_process_memory_wmi(process.pid)
                    }
                } else {
                    get_process_memory_wmi(process.pid)
                };

                total_mem_mb += mem_mb;
                let _ = CloseHandle(handle);
            } else {
                total_mem_mb += get_process_memory_wmi(process.pid);
            }
        }
    }

    prev_samples.retain(|pid, _| current_pids.contains(pid));

    if !sampled_any && total_mem_mb <= 0.0 && total_thread_count == 0 {
        debug!("⚠ Sunshine processes found but no accessible stats were collected");
        return None;
    }

    Some(ProcessStats {
        cpu_percent: total_cpu_percent.min(100.0),
        mem_mb: total_mem_mb,
        thread_count: total_thread_count,
    })
}

#[cfg(target_os = "windows")]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// 查找所有 Sunshine 相关进程（优先复用 desktop UI 已验证的 WMI 路径）
#[cfg(target_os = "windows")]
fn find_sunshine_processes() -> Vec<SunshineProcessRef> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename = "Win32_Process")]
    #[serde(rename_all = "PascalCase")]
    struct ProcessInfo {
        name: String,
        process_id: u32,
    }

    if let Ok(wmi_con) = WMIConnection::new() {
        if let Ok(results) = wmi_con
            .raw_query::<ProcessInfo>("SELECT Name, ProcessId FROM Win32_Process WHERE Name LIKE '%sunshine%'")
        {
            let processes: Vec<SunshineProcessRef> = results
                .into_iter()
                .filter(|p| {
                    let lower = p.name.to_lowercase();
                    !lower.contains("sunshine-gui") && !lower.contains("sunshine_gui")
                })
                .map(|p| SunshineProcessRef {
                    pid: p.process_id,
                    name: p.name,
                })
                .collect();

            if !processes.is_empty() {
                return processes;
            }
        }
    }

    find_sunshine_pid_toolhelp()
        .map(|pid| vec![SunshineProcessRef {
            pid,
            name: "sunshine.exe".to_string(),
        }])
        .unwrap_or_default()
}

/// ToolHelp fallback：查找 sunshine.exe 进程 ID
#[cfg(target_os = "windows")]
fn find_sunshine_pid_toolhelp() -> Option<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..std::mem::zeroed()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len())]
                );
                if name.eq_ignore_ascii_case("sunshine.exe") {
                    let _ = windows::Win32::Foundation::CloseHandle(snapshot);
                    return Some(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }
    None
}

/// 计算进程线程数
#[cfg(target_os = "windows")]
fn count_process_threads(pid: u32) -> u32 {
    use windows::Win32::System::Diagnostics::ToolHelp::*;

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(h) => h,
            Err(_) => return 0,
        };

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..std::mem::zeroed()
        };

        let mut count = 0u32;
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    count += 1;
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        count
    }
}

/// 通过 WMI 获取进程工作集内存（MB）—— 无需 PROCESS_VM_READ 权限
#[cfg(target_os = "windows")]
fn get_process_memory_wmi(pid: u32) -> f64 {
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename = "Win32_Process")]
    #[serde(rename_all = "PascalCase")]
    struct ProcMem {
        working_set_size: Option<u64>,
    }

    let wmi_con = match wmi::WMIConnection::new() {
        Ok(c) => c,
        Err(_) => return 0.0,
    };
    let query = format!("SELECT WorkingSetSize FROM Win32_Process WHERE ProcessId = {}", pid);
    let results: Vec<ProcMem> = match wmi_con.raw_query(&query) {
        Ok(r) => r,
        Err(_) => return 0.0,
    };
    results.first()
        .and_then(|p| p.working_set_size)
        .map(|ws| ws as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0)
}

// ─── 会话信息与 OSD 格式化 ──────────────────────────────────

/// 从 Sunshine API 获取串流会话信息
async fn fetch_session_info() -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut map = HashMap::new();

    let proxy_url = crate::proxy_server::get_proxy_url();
    let url = format!("{}/api/runtime/sessions", proxy_url);

    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return map,
    };

    match client.get(&url).send().await {
        Ok(resp) => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                // API 返回结构: {success, total_sessions, sessions: [...]}
                let sessions = body.get("sessions").and_then(|v| v.as_array());
                if let Some(session) = sessions.and_then(|arr| arr.first()) {
                    if let Some(state) = session.get("state").and_then(|v| v.as_str()) {
                        map.insert("session_state".into(), state.to_string());
                    }
                    if let Some(client) = session.get("client_name").and_then(|v| v.as_str()) {
                        map.insert("stream_client".into(), client.to_string());
                    }
                    if let Some(w) = session.get("width").and_then(|v| v.as_i64()) {
                        if let Some(h) = session.get("height").and_then(|v| v.as_i64()) {
                            map.insert("stream_resolution".into(), format!("{}x{}", w, h));
                        }
                    }
                    if let Some(fps) = session.get("fps").and_then(|v| v.as_i64()) {
                        map.insert("stream_fps".into(), format!("{}", fps));
                    }
                    if let Some(bitrate) = session.get("bitrate").and_then(|v| v.as_i64()) {
                        map.insert("stream_bitrate".into(), format!("{} Kbps", bitrate));
                    }
                    if let Some(app) = session.get("app_name").and_then(|v| v.as_str()) {
                        map.insert("app_name".into(), app.to_string());
                    }
                    if let Some(hdr) = session.get("enable_hdr").and_then(|v| v.as_bool()) {
                        map.insert("stream_hdr".into(), if hdr { "ON" } else { "OFF" }.into());
                    }
                    if let Some(codec) = session.get("codec").and_then(|v| v.as_str()) {
                        map.insert("stream_codec".into(), codec.to_string());
                    }
                }

                if !map.contains_key("session_state") {
                    map.insert("session_state".into(), "IDLE".into());
                }
                // 无活跃会话时，为所有 session 指标填充默认值
                if map.get("session_state").map_or(false, |s| s == "IDLE") {
                    map.entry("stream_client".into()).or_insert_with(|| "-".into());
                    map.entry("stream_resolution".into()).or_insert_with(|| "-".into());
                    map.entry("stream_fps".into()).or_insert_with(|| "-".into());
                    map.entry("stream_bitrate".into()).or_insert_with(|| "-".into());
                    map.entry("stream_codec".into()).or_insert_with(|| "-".into());
                    map.entry("stream_hdr".into()).or_insert_with(|| "-".into());
                    map.entry("app_name".into()).or_insert_with(|| "-".into());
                }
            }
        }
        Err(e) => {
            debug!("获取 Sunshine 会话信息失败: {}", e);
            map.insert("session_state".into(), "N/A".into());
            map.insert("stream_client".into(), "N/A".into());
            map.insert("stream_resolution".into(), "N/A".into());
            map.insert("stream_fps".into(), "N/A".into());
            map.insert("stream_bitrate".into(), "N/A".into());
            map.insert("stream_codec".into(), "N/A".into());
            map.insert("stream_hdr".into(), "N/A".into());
            map.insert("app_name".into(), "N/A".into());
        }
    }

    map
}

/// 构建 OSD 格式化文本
fn format_osd_text(
    config: &MonitoringConfig,
    metrics: &std::collections::HashMap<String, String>,
) -> String {
    let mut parts = Vec::new();

    // 矢量字体前缀（RTSS Extended OSD 使用 <FR=fontname,height> 标签）
    // 如果用户没有设置 font_size，使用 -1 表示保持 RTSS 默认大小
    let font_prefix = if !config.cjk_font.is_empty() {
        let h = if config.font_size > 0 { config.font_size as i32 } else { -1 };
        format!("<FR={},{}>", config.cjk_font, h)
    } else {
        String::new()
    };

    // 标题
    if !config.header_text.is_empty() {
        let size_tag = if config.font_size > 0 && config.cjk_font.is_empty() {
            format!("<S={}>", config.font_size)
        } else {
            String::new()
        };
        parts.push(format!("{}{}<C={}>{}<C>", font_prefix, size_tag, config.title_color, config.header_text));
    } else if !font_prefix.is_empty() {
        parts.push(font_prefix);
    }

    // 标签映射
    fn label_for(id: &str) -> &'static str {
        match id {
            "session_state" => "State",
            "stream_client" => "Client",
            "stream_resolution" => "Res",
            "stream_fps" => "FPS",
            "stream_bitrate" => "Bitrate",
            "stream_codec" => "Codec",
            "stream_hdr" => "HDR",
            "app_name" => "App",
            "process_cpu" => "CPU",
            "process_mem" => "Mem",
            "process_threads" => "Threads",
            _ => "??",
        }
    }

    for metric_id in &config.metrics {
        if let Some(value) = metrics.get(metric_id.as_str()) {
            parts.push(format!(
                "<C={}>{}: <C={}>{}<C>",
                config.label_color,
                label_for(metric_id),
                config.value_color,
                value
            ));
        }
    }

    let result = parts.join("\n");

    // 无矢量字体时过滤非 ASCII 字符，避免 RTSS 光栅字体乱码
    if config.cjk_font.is_empty() {
        result.chars().map(|c| if c.is_ascii() { c } else { '*' }).collect()
    } else {
        result
    }
}

/// 监控循环主体
async fn monitoring_loop(config: MonitoringConfig) {
    let interval = std::time::Duration::from_millis(config.interval_ms.max(500));
    let owner = "Foundation Sunshine".to_string();

    info!("🎯 RTSS 监控任务已启动 (间隔 {}ms, 指标: {:?})", config.interval_ms, config.metrics);

    while MONITORING_ACTIVE.load(Ordering::Relaxed) {
        let mut metrics = std::collections::HashMap::new();

        // 是否需要会话信息
        let needs_session = config.metrics.iter().any(|m| {
            matches!(m.as_str(), "session_state" | "stream_client" | "stream_resolution"
                | "stream_fps" | "stream_bitrate" | "stream_codec" | "stream_hdr" | "app_name")
        });

        if needs_session {
            let session_info = fetch_session_info().await;
            metrics.extend(session_info);
        }

        // 是否需要进程统计
        #[cfg(target_os = "windows")]
        {
            let needs_process = config.metrics.iter().any(|m| {
                matches!(m.as_str(), "process_cpu" | "process_mem" | "process_threads")
            });

            if needs_process {
                match get_sunshine_process_stats() {
                    Some(stats) => {
                        if config.metrics.contains(&"process_cpu".to_string()) {
                            metrics.insert("process_cpu".into(), format!("{:.1}%", stats.cpu_percent));
                        }
                        if config.metrics.contains(&"process_mem".to_string()) {
                            metrics.insert("process_mem".into(), format!("{:.0} MB", stats.mem_mb));
                        }
                        if config.metrics.contains(&"process_threads".to_string()) {
                            metrics.insert("process_threads".into(), format!("{}", stats.thread_count));
                        }
                    }
                    None => {
                        debug!("⚠ get_sunshine_process_stats() returned None (pid not found or access denied)");
                    }
                }
            }
        }

        // 格式化并写入 OSD
        let osd_text = format_osd_text(&config, &metrics);

        // 更新快照
        {
            let mut snapshot = MONITORING_SNAPSHOT.lock().await;
            snapshot.active = true;
            snapshot.osd_text = osd_text.clone();
            snapshot.metrics = metrics;
        }

        // 写入 RTSS OSD
        if let Err(e) = rtss_set_osd(osd_text, Some(owner.clone())).await {
            warn!("RTSS OSD 写入失败: {}", e);
        }

        tokio::time::sleep(interval).await;
    }

    // 释放 OSD 槽位（零填整个 entry）
    #[cfg(target_os = "windows")]
    {
        let _ = release_osd_slots(&owner);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = rtss_set_osd(String::new(), Some(owner)).await;
    }

    {
        let mut snapshot = MONITORING_SNAPSHOT.lock().await;
        snapshot.active = false;
        snapshot.osd_text.clear();
        snapshot.metrics.clear();
    }

    info!("🎯 RTSS 监控任务已停止");
}

// ─── 监控 Tauri 命令 ────────────────────────────────────────

/// 启动 OSD 实时监控
#[tauri::command]
pub async fn rtss_start_monitoring(config: MonitoringConfig) -> Result<(), String> {
    if MONITORING_ACTIVE.load(Ordering::Relaxed) {
        // 先停止旧的
        MONITORING_ACTIVE.store(false, Ordering::Relaxed);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    MONITORING_ACTIVE.store(true, Ordering::Relaxed);

    tokio::spawn(async move {
        monitoring_loop(config).await;
    });

    Ok(())
}

/// 停止 OSD 实时监控
#[tauri::command]
pub async fn rtss_stop_monitoring() -> Result<(), String> {
    MONITORING_ACTIVE.store(false, Ordering::Relaxed);

    // 立即释放 OSD 槽位，不等后台循环
    #[cfg(target_os = "windows")]
    {
        let _ = release_osd_slots("Foundation Sunshine");
    }

    // 清除快照
    {
        let mut snapshot = MONITORING_SNAPSHOT.lock().await;
        snapshot.active = false;
        snapshot.osd_text.clear();
        snapshot.metrics.clear();
    }

    info!("🎯 RTSS 监控已停止并清除 OSD");
    Ok(())
}

/// 获取监控快照（当前指标值和 OSD 文本）
#[tauri::command]
pub async fn rtss_get_monitoring_status() -> MonitoringSnapshot {
    let snapshot = MONITORING_SNAPSHOT.lock().await;
    snapshot.clone()
}

// ─── OSD 属性配置 ──────────────────────────────────────────

/// OSD 属性快照
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OsdProperties {
    pub osd_enabled: Option<i32>,
    pub show_own_stats: Option<i32>,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
    pub zoom: Option<i32>,
    pub coordinate_space: Option<i32>,
}

/// 批量读取 OSD 属性
#[tauri::command]
pub async fn rtss_get_osd_properties(profile: Option<String>) -> Result<OsdProperties, String> {
    #[cfg(target_os = "windows")]
    {
        let prof = profile.unwrap_or_else(|| "Global".into());

        // OSD 使用专用 overlay 命令获取
        let osd_enabled = match run_rtss_cli(&["overlay:get"]) {
            Ok(v) => v.trim().parse().ok(),
            Err(_) => None,
        };

        // 其他属性从 profile 文件读取
        let profile_path = get_rtss_profile_path(&prof)?;
        let read_ini = |section: &str, key: &str| -> Option<i32> {
            get_ini_value(&profile_path, section, key)
                .ok()
                .and_then(|v| v.parse().ok())
        };

        Ok(OsdProperties {
            osd_enabled,
            show_own_stats: read_ini("OSD", "EnableStat"),
            position_x: read_ini("OSD", "PositionX"),
            position_y: read_ini("OSD", "PositionY"),
            zoom: read_ini("OSD", "ZoomRatio"),
            coordinate_space: read_ini("OSD", "CoordinateSpace"),
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 设置单个 OSD 属性
#[tauri::command]
pub async fn rtss_set_osd_property(
    key: String,
    value: String,
    profile: Option<String>,
) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let prof = profile.unwrap_or_else(|| "Global".into());
        info!("🎯 RTSS property:set {} {} = {}", prof, key, value);

        // OSD 属性使用专用 overlay 命令
        if key == "OSD" {
            let target = &value;
            // 先检查当前状态
            let current = run_rtss_cli(&["overlay:get"])?;
            if current.trim() == target.as_str() {
                return Ok("OK".to_string());
            }
            // 使用 overlay:set
            run_rtss_cli(&["overlay:set", target])?;
            std::thread::sleep(std::time::Duration::from_millis(100));
            let actual = run_rtss_cli(&["overlay:get"])?;
            if actual.trim() == target.as_str() {
                return Ok("OK".to_string());
            }
            // 提权重试
            run_rtss_cli_elevated(&["overlay:set", target])?;
            std::thread::sleep(std::time::Duration::from_millis(300));
            let actual2 = run_rtss_cli(&["overlay:get"])?;
            if actual2.trim() == target.as_str() {
                return Ok("OK".to_string());
            }
            return Err(format!("OSD 设置失败: 期望={}, 实际={}", target, actual2.trim()));
        }

        // 其他属性通过修改 profile 文件实现
        let profile_key = match key.as_str() {
            "OSDShowOwnStatistics" => ("OSD", "EnableStat"),
            "OnScreenDisplayX" => ("OSD", "PositionX"),
            "OnScreenDisplayY" => ("OSD", "PositionY"),
            "OnScreenDisplayZoom" => ("OSD", "ZoomRatio"),
            "OSDCoordinateSpace" => ("OSD", "CoordinateSpace"),
            _ => return Err(format!("未知的 OSD 属性: {}", key)),
        };

        let profile_path = get_rtss_profile_path(&prof)?;
        set_ini_value(&profile_path, profile_key.0, profile_key.1, &value)?;

        Ok("OK".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}