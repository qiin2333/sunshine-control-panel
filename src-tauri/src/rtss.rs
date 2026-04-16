// RTSS (RivaTuner Statistics Server) 集成模块
// 提供帧率限制控制、OSD 文字显示和实时监控功能

use serde::{Deserialize, Serialize};
use log::{info, debug, warn};
use std::ffi::CString;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

// ─── RTSS 共享内存结构 ────────────────────────────────────────

const RTSS_SHARED_MEMORY_SIGNATURE: u32 = 0x52545353; // 'RTSS'

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
static MONITORING_CONFIG: Lazy<Arc<Mutex<MonitoringConfig>>> =
    Lazy::new(|| Arc::new(Mutex::new(MonitoringConfig::default())));
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

/// 以管理员权限执行 rtss-cli 并捕获 stdout
fn run_rtss_cli_elevated(args: &[&str]) -> Result<String, String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::PCWSTR;

    let cli_path = get_rtss_cli_path()?;

    // 通过临时文件捕获 stdout
    let tmp_out = std::env::temp_dir().join("rtss_cli_elevated_out.txt");
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

// ─── Tauri 命令 ────────────────────────────────────────────

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

/// 写入 OSD 文本到 RTSS 共享内存
#[tauri::command]
pub async fn rtss_set_osd(text: String, owner: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let owner_name = owner.unwrap_or_else(|| "Sunshine".to_string());
        let shm = win::SharedMemoryHandle::open()?;

        unsafe {
            let header = &*(shm.ptr as *const RtssSharedMemoryHeader);

            if header.dwSignature != RTSS_SHARED_MEMORY_SIGNATURE {
                return Err("RTSS 共享内存签名不匹配".to_string());
            }

            let osd_arr_base = (shm.ptr as *mut u8).add(header.dwOSDArrOffset as usize);
            let osd_count = if header.dwOSDEntrySize > 0 {
                header.dwOSDArrSize
            } else {
                0
            };

            debug!(
                "🎯 RTSS SHM: OSDEntrySize={} OSDCount={} OSDOffset={}",
                header.dwOSDEntrySize, osd_count, header.dwOSDArrOffset
            );

            if osd_count == 0 {
                return Err("RTSS OSD 槽位数量为 0".to_string());
            }

            let owner_cstr = CString::new(owner_name.as_str()).unwrap();
            let text_cstr = CString::new(text.as_str()).unwrap();
            let mut target_slot: Option<*mut u8> = None;
            let mut first_empty_slot: Option<*mut u8> = None;
            let mut dead_process_slot: Option<*mut u8> = None;

            for i in 0..osd_count {
                let entry_ptr = osd_arr_base.add((i * header.dwOSDEntrySize) as usize);
                let osd_owner_ptr = entry_ptr.add(256);

                // 安全读取 owner 字符串（可能包含垃圾数据）
                let first_byte = *osd_owner_ptr;
                if first_byte == 0 {
                    // 真正的空槽位
                    if first_empty_slot.is_none() {
                        first_empty_slot = Some(entry_ptr);
                    }
                    continue;
                }

                // 读取 owner 名称，限制长度防止越界
                let owner_slice = std::slice::from_raw_parts(osd_owner_ptr, 256);
                let nul_pos = owner_slice.iter().position(|&b| b == 0).unwrap_or(255);
                let existing_owner = String::from_utf8_lossy(&owner_slice[..nul_pos]);

                if existing_owner == owner_name.as_str() {
                    target_slot = Some(entry_ptr);
                    break;
                }

                // 检查该 owner 进程是否还存活（可能是已退出的游戏留下的僵尸槽）
                if dead_process_slot.is_none() {
                    let osd_text_first_byte = *entry_ptr;
                    if osd_text_first_byte == 0 {
                        // OSD 文本为空的占用槽，可能已废弃
                        dead_process_slot = Some(entry_ptr);
                    }
                }
            }

            let slot = target_slot
                .or(first_empty_slot)
                .or(dead_process_slot)
                .ok_or_else(|| {
                    format!(
                        "没有可用的 RTSS OSD 槽位（共 {} 个槽位均被占用）",
                        osd_count
                    )
                })?;

            let text_bytes = text_cstr.as_bytes_with_nul();
            let owner_bytes = owner_cstr.as_bytes_with_nul();

            // 清空并写入 OSD 文本 (256 bytes)
            std::ptr::write_bytes(slot, 0, 256);
            std::ptr::copy_nonoverlapping(text_bytes.as_ptr(), slot, text_bytes.len().min(255));

            // 清空并写入 Owner (256 bytes, offset +256)
            let owner_ptr = slot.add(256);
            std::ptr::write_bytes(owner_ptr, 0, 256);
            std::ptr::copy_nonoverlapping(owner_bytes.as_ptr(), owner_ptr, owner_bytes.len().min(255));
        }

        info!("✅ RTSS OSD 已更新: {}", text);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}

/// 清除 Sunshine 的 OSD 文本
#[tauri::command]
pub async fn rtss_clear_osd(owner: Option<String>) -> Result<(), String> {
    rtss_set_osd(String::new(), owner).await
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
    {
        // 读取切换前状态
        let before = run_rtss_cli(&["limiter:get"])?
            .parse::<i32>().unwrap_or(-1);

        // 尝试普通权限切换
        let result = run_rtss_cli(&["limiter:toggle"])?;

        // 验证是否真的切换了
        let after = run_rtss_cli(&["limiter:get"])?
            .parse::<i32>().unwrap_or(-1);

        if before != after {
            let state = if after == 1 { "已启用" } else { "已禁用" };
            info!("🎯 RTSS 帧率限制器: {}", state);
            return Ok(after.to_string());
        }

        // 没变化 → 提升权限重试
        info!("🎯 普通权限切换失败, 尝试管理员权限...");
        run_rtss_cli_elevated(&["limiter:toggle"])?;

        std::thread::sleep(std::time::Duration::from_millis(200));
        let final_state = run_rtss_cli(&["limiter:get"])?
            .parse::<i32>().unwrap_or(-1);

        if before != final_state {
            let state = if final_state == 1 { "已启用" } else { "已禁用" };
            info!("🎯 RTSS 帧率限制器 (提升权限): {}", state);
            Ok(final_state.to_string())
        } else {
            // 仍然可能成功，返回 toggle 返回值
            Ok(result)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
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
    {
        let before = run_rtss_cli(&["overlay:get"])?
            .parse::<i32>().unwrap_or(-1);

        let result = run_rtss_cli(&["overlay:toggle"])?;

        let after = run_rtss_cli(&["overlay:get"])?
            .parse::<i32>().unwrap_or(-1);

        if before != after {
            let state = if after == 1 { "显示" } else { "隐藏" };
            info!("🎯 RTSS OSD: {}", state);
            return Ok(after.to_string());
        }

        // 提升权限重试
        info!("🎯 普通权限切换 OSD 失败, 尝试管理员权限...");
        run_rtss_cli_elevated(&["overlay:toggle"])?;

        std::thread::sleep(std::time::Duration::from_millis(200));
        let final_state = run_rtss_cli(&["overlay:get"])?
            .parse::<i32>().unwrap_or(-1);

        if before != final_state {
            let state = if final_state == 1 { "显示" } else { "隐藏" };
            info!("🎯 RTSS OSD (提升权限): {}", state);
            Ok(final_state.to_string())
        } else {
            Ok(result)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
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

/// 获取 Sunshine 进程统计
#[cfg(target_os = "windows")]
fn get_sunshine_process_stats() -> Option<ProcessStats> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use std::mem;

    // 通过 WMI 或 toolhelp 查找 sunshine.exe PID
    let pid = find_sunshine_pid()?;

    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        ).ok()?;

        // CPU 使用率 - 需要两次采样
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let _ = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);

        let kernel_time = filetime_to_u64(&kernel);
        let user_time = filetime_to_u64(&user);
        let total_cpu_time = kernel_time + user_time;

        // 系统时间
        let sys_time = GetSystemTimeAsFileTime();
        let sys_now = filetime_to_u64(&sys_time);
        let start_time = filetime_to_u64(&creation);
        let wall_time = sys_now.saturating_sub(start_time);

        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as f64)
            .unwrap_or(1.0);

        let cpu_percent = if wall_time > 0 {
            (total_cpu_time as f64 / wall_time as f64 * 100.0 / num_cpus).min(100.0)
        } else {
            0.0
        };

        // 内存
        let mut pmc: PROCESS_MEMORY_COUNTERS = mem::zeroed();
        pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let mem_mb = if GetProcessMemoryInfo(
            handle,
            &mut pmc,
            mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ).is_ok() {
            pmc.WorkingSetSize as f64 / 1024.0 / 1024.0
        } else {
            0.0
        };

        // 线程数 (通过 toolhelp)
        let thread_count = count_process_threads(pid);

        let _ = CloseHandle(handle);

        Some(ProcessStats { cpu_percent, mem_mb, thread_count })
    }
}

#[cfg(target_os = "windows")]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// 查找 sunshine.exe 进程 ID
#[cfg(target_os = "windows")]
fn find_sunshine_pid() -> Option<u32> {
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
                // 解析会话列表
                if let Some(sessions) = body.as_array() {
                    if let Some(session) = sessions.first() {
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
                        // codec 可能在其他字段中
                        if let Some(codec) = session.get("codec").and_then(|v| v.as_str()) {
                            map.insert("stream_codec".into(), codec.to_string());
                        }
                    }
                } else if body.is_object() {
                    // 可能是单个会话对象
                    if let Some(state) = body.get("state").and_then(|v| v.as_str()) {
                        map.insert("session_state".into(), state.to_string());
                    }
                }

                if !map.contains_key("session_state") {
                    map.insert("session_state".into(), "IDLE".into());
                }
            }
        }
        Err(e) => {
            debug!("获取 Sunshine 会话信息失败: {}", e);
            map.insert("session_state".into(), "N/A".into());
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

    // 标题
    if !config.header_text.is_empty() {
        let size_tag = if config.font_size > 0 { format!("<S={}>", config.font_size) } else { String::new() };
        parts.push(format!("{}<C={}>{}<C>", size_tag, config.title_color, config.header_text));
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

    parts.join("\n")
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
                if let Some(stats) = get_sunshine_process_stats() {
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

    // 清理 OSD
    let _ = rtss_set_osd(String::new(), Some(owner)).await;

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

    // 保存配置
    {
        let mut cfg = MONITORING_CONFIG.lock().await;
        *cfg = config.clone();
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
    info!("🎯 RTSS 监控已请求停止");
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
        let read_prop = |key: &str| -> Option<i32> {
            match run_rtss_cli(&["property:get", &prof, key]) {
                Ok(v) => v.trim().parse().ok(),
                Err(_) => None,
            }
        };

        // OSD 使用专用 overlay 命令获取
        let osd_enabled = match run_rtss_cli(&["overlay:get"]) {
            Ok(v) => v.trim().parse().ok(),
            Err(_) => read_prop("OSD"),
        };

        Ok(OsdProperties {
            osd_enabled,
            show_own_stats: read_prop("OSDShowOwnStatistics"),
            position_x: read_prop("OnScreenDisplayX"),
            position_y: read_prop("OnScreenDisplayY"),
            zoom: read_prop("OnScreenDisplayZoom"),
            coordinate_space: read_prop("OSDCoordinateSpace"),
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

        // 其他属性使用 property:set
        run_rtss_cli(&["property:set", &prof, &key, &value])?;

        // 验证
        let actual = run_rtss_cli(&["property:get", &prof, &key])?;
        if actual == value {
            return Ok("OK".to_string());
        }

        // 提升权限重试
        info!("🎯 普通权限 property:set 失败, 尝试管理员权限...");
        run_rtss_cli_elevated(&["property:set", &prof, &key, &value])?;

        // 等待并多次验证（提权操作可能有延迟）
        for i in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let actual2 = run_rtss_cli(&["property:get", &prof, &key])?;
            if actual2 == value {
                return Ok("OK".to_string());
            }
            debug!("🎯 验证第{}次: 期望={}, 实际={}", i + 1, value, actual2);
        }

        let final_val = run_rtss_cli(&["property:get", &prof, &key])?;
        Err(format!("属性设置失败: RTSS 可能未以管理员权限运行，或该属性被锁定 ({} 期望={}, 实际={})", key, value, final_val))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("RTSS 仅在 Windows 上可用".to_string())
    }
}