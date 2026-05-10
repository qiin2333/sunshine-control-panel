//! ViGEmBus 虚拟手柄驱动管理
//!
//! 复用 Sunshine 自带的 `scripts/install-gamepad.bat` / `scripts/uninstall-gamepad.bat`
//! —— 这两个 bat 已经长期在 Sunshine 安装/迁移流程里使用，自带版本检查、
//! 系统代理探测、注册表卸载查找等逻辑，无需重复实现。
//!
//! GUI 仅负责：
//! 1. 上报当前 `%SystemRoot%\System32\drivers\ViGEmBus.sys` 的版本与设备节点状态；
//! 2. 提权调用 install/uninstall bat（共用 `bat_runner`），日志落到
//!    `%TEMP%\sunshine-vigem-*.log` 方便排错。

use crate::sunshine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use crate::bat_runner;
#[cfg(target_os = "windows")]
use log::warn;

/// Sunshine 要求的最低 ViGEmBus 版本（major, minor）
const MIN_VIGEM_VERSION: (u32, u32) = (1, 17);

/// ViGEmBus 驱动状态
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VigemStatus {
    /// 驱动是否已安装（ViGEmBus.sys 文件存在）
    pub installed: bool,
    /// 驱动是否正常运行（总线设备节点 Status=OK）
    pub running: bool,
    /// 驱动文件版本（e.g. "1.22.0.0"），未安装则为空
    pub version: String,
    /// 是否满足 Sunshine 的最低版本要求 (>=1.17)
    pub version_ok: bool,
    /// 状态描述文本
    pub status_text: String,
    /// 驱动 sys 文件路径
    pub driver_path: String,
}

/// scripts 根目录（install-gamepad.bat / uninstall-gamepad.bat 直接在这里）
fn get_scripts_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path()).join("scripts")
}

/// 解析 "1.22.0.0" → Some((1, 22))，失败返回 None
fn parse_major_minor(v: &str) -> Option<(u32, u32)> {
    let mut it = v.split('.');
    let maj = it.next()?.parse().ok()?;
    let min = it.next()?.parse().ok()?;
    Some((maj, min))
}

/// PowerShell 探针的输出，捕捉 ViGEmBus 的所有可观测状态
#[cfg(target_os = "windows")]
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct VigemProbe {
    installed: bool,
    version: String,
    device_status: String,
    device_problem: u64,
    device_instance_id: String,
}

/// 检查 ViGEmBus 驱动文件版本与设备节点状态，返回结构化状态
#[cfg(target_os = "windows")]
fn check_vigem() -> VigemStatus {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 单次 PowerShell 调用拿到：sys 文件版本 + 总线设备状态
    let ps = r#"
$result = @{ installed = $false; version = ''; deviceStatus = 'NotFound'; deviceProblem = 0; deviceInstanceId = '' }
$sys = Join-Path $env:SystemRoot 'System32\drivers\ViGEmBus.sys'
if (Test-Path $sys) {
    $result.installed = $true
    try { $result.version = (Get-Item $sys).VersionInfo.FileVersion } catch {}
}
$dev = Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
       Where-Object { $_.HardwareID -contains 'ROOT\ViGEmBus' -or $_.InstanceId -like 'ROOT\VIRTUALGAMEPADEMULATIONBUS\*' } |
       Select-Object -First 1
if ($dev) {
    $result.deviceStatus = $dev.Status
    $result.deviceProblem = [int]$dev.Problem
    $result.deviceInstanceId = $dev.InstanceId
}
$result | ConvertTo-Json -Compress
"#;

    let driver_path = std::env::var("SystemRoot")
        .map(|r| format!("{}\\System32\\drivers\\ViGEmBus.sys", r))
        .unwrap_or_else(|_| "C:\\Windows\\System32\\drivers\\ViGEmBus.sys".to_string());

    let probe: VigemProbe = match Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["-NoProfile", "-Command", ps])
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            serde_json::from_str(&stdout).unwrap_or_else(|e| {
                warn!("解析 ViGEmBus 状态 JSON 失败: {} ({})", e, stdout);
                VigemProbe::default()
            })
        }
        Err(e) => {
            warn!("检测 ViGEmBus 失败: {}", e);
            return VigemStatus {
                installed: false,
                running: false,
                version: String::new(),
                version_ok: false,
                status_text: "检测失败".to_string(),
                driver_path,
            };
        }
    };

    let running = probe.installed && probe.device_status == "OK" && probe.device_problem == 0;
    let version_ok = parse_major_minor(&probe.version)
        .map_or(false, |(maj, min)| (maj, min) >= MIN_VIGEM_VERSION);

    let status_text = if !probe.installed {
        "未安装".to_string()
    } else if !version_ok {
        format!(
            "版本过旧 ({}) — 需要 {}.{} 或更高",
            probe.version, MIN_VIGEM_VERSION.0, MIN_VIGEM_VERSION.1
        )
    } else if running {
        if probe.device_instance_id.is_empty() {
            format!("ViGEmBus {} - 正常运行", probe.version)
        } else {
            format!(
                "ViGEmBus {} - 正常运行 ({})",
                probe.version, probe.device_instance_id
            )
        }
    } else if probe.device_problem == 21 {
        format!("ViGEmBus {} - 需要重启", probe.version)
    } else if probe.device_status == "NotFound" {
        format!("ViGEmBus {} - 已安装但总线设备未枚举", probe.version)
    } else {
        format!(
            "ViGEmBus {} - 状态: {} (问题码: {})",
            probe.version, probe.device_status, probe.device_problem
        )
    };

    VigemStatus {
        installed: probe.installed,
        running,
        version: probe.version,
        version_ok,
        status_text,
        driver_path,
    }
}

#[cfg(not(target_os = "windows"))]
fn check_vigem() -> VigemStatus {
    VigemStatus {
        installed: false,
        running: false,
        version: String::new(),
        version_ok: false,
        status_text: "仅支持 Windows".to_string(),
        driver_path: String::new(),
    }
}

/// 获取 ViGEmBus 驱动状态
#[tauri::command]
pub async fn get_vigem_status() -> Result<VigemStatus, String> {
    Ok(check_vigem())
}

/// 调用 scripts/<name> 并等待驱动栈稳定
#[cfg(target_os = "windows")]
async fn run_gamepad_script(
    name: &str,
    extra_args: &[&str],
    success_msg: &str,
    settle_secs: u64,
) -> Result<String, String> {
    let bat = get_scripts_path().join(name);
    if !bat.exists() {
        return Err(format!(
            "脚本不存在: {}。请确认 Sunshine 安装完整。",
            bat.display()
        ));
    }
    log::info!("调用 {} {} ...", name, extra_args.join(" "));
    bat_runner::run_elevated(&bat, "vigem", extra_args)?;
    tokio::time::sleep(tokio::time::Duration::from_secs(settle_secs)).await;
    Ok(success_msg.to_string())
}

/// 安装 / 更新 ViGEmBus 驱动（复用 scripts/install-gamepad.bat）
///
/// `force=true` 时传递 `force` 参数让 bat 跳过“已装 >=1.17 就 skip”的逻辑，
/// 强制拉取并重装最新 nefarius/vigembus 发行版。
#[tauri::command]
pub async fn install_vigem_driver(force: Option<bool>) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let force = force.unwrap_or(false);
        let args: &[&str] = if force { &["force"] } else { &[] };
        let msg = if force {
            "ViGEmBus 驱动已重新安装为最新版"
        } else {
            "ViGEmBus 驱动安装完成"
        };
        run_gamepad_script("install-gamepad.bat", args, msg, 2).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = force;
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 卸载 ViGEmBus 驱动（复用 scripts/uninstall-gamepad.bat）
#[tauri::command]
pub async fn uninstall_vigem_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        run_gamepad_script("uninstall-gamepad.bat", &[], "ViGEmBus 驱动已卸载", 2).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}
