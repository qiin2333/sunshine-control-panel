use crate::sunshine;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 检查当前进程是否具有管理员权限
#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            s == "True"
        }
        Err(_) => false,
    }
}

/// 以适当权限运行 bat 脚本：已有管理员权限则直接运行，否则提权。
///
/// 关键点：必须把 bat 的真实 exit code 透传出来。
/// `Start-Process -Verb RunAs -Wait` 默认**不会**让 PowerShell 进程的
/// `$LASTEXITCODE` 反映子进程退出码，必须用 `-PassThru` 拿到 Process
/// 对象再读 `.ExitCode` 并 `exit` 出去，否则 bat 即使 `exit /b 87` 上层
/// 也会以为安装成功，前端只会看到“静默成功”。
///
/// 同时把 bat 的 stdout/stderr 落到 `%TEMP%\sunshine-vmouse-<name>.log`，
/// 失败时 Err 字符串里带上 exit code + 日志路径，前端能直接展示给用户。
///
/// 实现细节：提权场景下 `Start-Process` 不允许 RedirectStandardOutput，
/// 而把 `cmd /c "...bat... > log 2>&1"` 用 PowerShell 单字符串拼出来，
/// 多层 cmd/PS 引号嵌套很容易解析失败（cmd 直接 exit 1 而 bat 根本没跑）。
/// 因此走更稳的方式：在 `%TEMP%` 写一份只有几行的 wrapper.bat，里面自己
/// 重定向 + exit /b %ERRORLEVEL%，然后让 PowerShell 直接 Start-Process
/// 这份 wrapper（不需要任何引号嵌套）。
#[cfg(target_os = "windows")]
fn run_bat_elevated(bat_path: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let stem = bat_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("run");
    let log_path = std::env::temp_dir().join(format!("sunshine-vmouse-{}.log", stem));
    let log_str = log_path.to_string_lossy().into_owned();

    if is_elevated() {
        // 已有管理员权限，cmd 自己重定向把全部输出落盘
        let cmd_line = format!(
            r#""{bat}" > "{log}" 2>&1"#,
            bat = bat_path.display(),
            log = log_str
        );
        let output = Command::new("cmd")
            .args(&["/c", &cmd_line])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(format!("脚本执行失败 (exit {})，日志: {}", code, log_str));
        }
    } else {
        // 写一份 wrapper bat，避免 PS+cmd 多层引号嵌套
        let wrapper_path =
            std::env::temp_dir().join(format!("sunshine-vmouse-{}-wrap.bat", stem));
        {
            let mut f = std::fs::File::create(&wrapper_path)
                .map_err(|e| format!("创建 wrapper 失败: {}", e))?;
            // wrapper 内容：直接调原 bat，重定向输出到日志，把真 exit code 透传
            // chcp 65001 让中文输出不乱码
            writeln!(f, "@echo off").ok();
            writeln!(f, "chcp 65001 >nul").ok();
            writeln!(
                f,
                r#"call "{}" > "{}" 2>&1"#,
                bat_path.display(),
                log_str
            )
            .ok();
            writeln!(f, "exit /b %ERRORLEVEL%").ok();
        }

        let ps_cmd = format!(
            r#"$p = Start-Process cmd -ArgumentList '/c','{wrap}' -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"#,
            wrap = wrapper_path.display()
        );

        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        // 不立即清理 wrapper：失败时方便人工复现
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(format!("脚本执行失败 (exit {})，日志: {}", code, log_str));
        }
        // 成功才清理 wrapper
        let _ = std::fs::remove_file(&wrapper_path);
    }
    Ok(())
}

/// vmouse 驱动状态信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VmouseStatus {
    /// 驱动是否已安装（设备节点存在）
    pub installed: bool,
    /// 设备是否正常运行（无错误码）
    pub running: bool,
    /// 设备状态描述
    pub status_text: String,
    /// 驱动文件路径（如有）
    pub driver_path: String,
    /// sunshine.conf 中 virtual_mouse 配置值
    pub config_enabled: bool,
}

/// vmouse 驱动文件目录
fn get_vmouse_tools_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path())
        .join("tools")
        .join("vmouse")
}

/// vmouse bat 脚本目录（CMake 安装到 scripts/vmouse/）
fn get_vmouse_scripts_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path())
        .join("scripts")
        .join("vmouse")
}

/// 检查 vmouse 设备节点是否存在
#[cfg(target_os = "windows")]
fn check_device_installed() -> (bool, bool, String) {
    use std::process::Command;

    // 使用 PowerShell 的 Get-PnpDevice 检查设备状态。
    // ZakoVirtualMouse 的 ROOT\HIDCLASS 根设备在部分系统上 FriendlyName 为空，
    // 但 HardwareID/Status/Problem 仍然能准确反映设备节点是否存在并启动。
    // 因此不能用 FriendlyName 非空作为“已安装”的必要条件，否则 GUI 会在
    // 驱动实际创建 ROOT\HIDCLASS\xxxx 设备后仍显示“未安装”。
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&[
            "-NoProfile", "-Command",
            "Get-PnpDevice -InstanceId 'ROOT\\HIDCLASS\\*' -ErrorAction SilentlyContinue | Where-Object { $_.HardwareID -contains 'Root\\ZakoVirtualMouse' -or $_.FriendlyName -like '*Virtual Mouse*' } | Select-Object -First 1 Status, FriendlyName, Problem, InstanceId | ConvertTo-Json -Compress"
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if stdout.is_empty() || stdout == "null" {
                return (false, false, "未安装".to_string());
            }

            // 尝试解析 JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                let status = json
                    .get("Status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let friendly = json
                    .get("FriendlyName")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Zako Virtual Mouse");
                let instance_id = json
                    .get("InstanceId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let problem = json.get("Problem").and_then(|v| v.as_u64()).unwrap_or(0);

                let root_ok = status == "OK" && problem == 0;
                // ZakoVirtualMouse 是 UMDF + ROOT\HIDCLASS 架构，子 HID PDO
                // 的 InstanceId 形如 `HID\HIDCLASS\1&xxxx` —— **不带 VID/PID
                // 字符串**（VID/PID 只走 HidD_GetAttributes，不进 InstanceId）。
                // 因此必须通过根设备的 DEVPKEY_Device_Children 拿到子 PDO 列表
                // 再校验 svc=mouhid + Status=OK，否则 hid_present 永远 false，
                // 导致设备完全可用却显示“已安装 未激活”。
                let child_query = format!(
                    "$children = (Get-PnpDeviceProperty -InstanceId '{}' -KeyName DEVPKEY_Device_Children -ErrorAction SilentlyContinue).Data; if (-not $children) {{ '' }} else {{ Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | Where-Object {{ $children -contains $_.InstanceId -and $_.Service -eq 'mouhid' }} | Select-Object -First 1 Status, FriendlyName, Problem, InstanceId | ConvertTo-Json -Compress }}",
                    instance_id
                );
                let hid_output = Command::new("powershell")
                    .creation_flags(CREATE_NO_WINDOW)
                    .args(&["-NoProfile", "-Command", &child_query])
                    .output();
                let hid_present = hid_output
                    .ok()
                    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
                    .filter(|stdout| !stdout.is_empty() && stdout != "null")
                    .and_then(|stdout| serde_json::from_str::<serde_json::Value>(&stdout).ok())
                    .map(|hid| {
                        let hid_status = hid
                            .get("Status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");
                        let hid_problem = hid.get("Problem").and_then(|v| v.as_u64()).unwrap_or(0);
                        hid_status == "OK" && hid_problem == 0
                    })
                    .unwrap_or(false);

                let running = root_ok && hid_present;
                let status_text = if running {
                    if instance_id.is_empty() {
                        format!("{} - 正常运行", friendly)
                    } else {
                        format!("{} - 正常运行 ({})", friendly, instance_id)
                    }
                } else if problem == 21 {
                    format!("{} - 需要重启", friendly)
                } else if root_ok {
                    format!("{} - 已安装，但 HID 接口未枚举/驱动未运行", friendly)
                } else {
                    format!("{} - 状态: {} (问题码: {})", friendly, status, problem)
                };

                (true, running, status_text)
            } else {
                // JSON 解析失败但有输出，说明设备存在
                (true, false, format!("已安装（状态未知）"))
            }
        }
        Err(e) => {
            warn!("检测 vmouse 设备失败: {}", e);
            (false, false, "检测失败".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn check_device_installed() -> (bool, bool, String) {
    (false, false, "仅支持 Windows".to_string())
}

/// 读取 sunshine.conf 中的 virtual_mouse 配置
fn read_vmouse_config() -> bool {
    let config_path = PathBuf::from(sunshine::get_sunshine_install_path())
        .join("config")
        .join("sunshine.conf");

    if !config_path.exists() {
        return true; // 默认启用
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "virtual_mouse" {
                        let val = value.trim().to_lowercase();
                        return val == "enabled" || val == "true" || val == "1" || val == "yes";
                    }
                }
            }
            true // 默认启用
        }
        Err(_) => true,
    }
}

/// 获取 vmouse 驱动状态
#[tauri::command]
pub async fn get_vmouse_status() -> Result<VmouseStatus, String> {
    let (installed, running, status_text) = check_device_installed();
    let driver_path = get_vmouse_tools_path().to_string_lossy().to_string();
    let config_enabled = read_vmouse_config();

    Ok(VmouseStatus {
        installed,
        running,
        status_text,
        driver_path,
        config_enabled,
    })
}

/// 安装 vmouse 驱动（复用 install-vmouse.bat）
#[tauri::command]
pub async fn install_vmouse_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let scripts_dir = get_vmouse_scripts_path();
        let install_bat = scripts_dir.join("install-vmouse.bat");

        if !install_bat.exists() {
            return Err(format!(
                "安装脚本不存在: {}。请确认 Sunshine 安装完整。",
                install_bat.display()
            ));
        }

        info!("调用 install-vmouse.bat 安装虚拟鼠标驱动...");
        run_bat_elevated(&install_bat)?;

        // 等待驱动加载
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        info!("✅ vmouse 驱动安装完成");
        Ok("虚拟鼠标驱动安装完成".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 卸载 vmouse 驱动（复用 uninstall-vmouse.bat）
#[tauri::command]
pub async fn uninstall_vmouse_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let scripts_dir = get_vmouse_scripts_path();
        let uninstall_bat = scripts_dir.join("uninstall-vmouse.bat");

        if !uninstall_bat.exists() {
            return Err(format!(
                "卸载脚本不存在: {}。请确认 Sunshine 安装完整。",
                uninstall_bat.display()
            ));
        }

        info!("调用 uninstall-vmouse.bat 卸载虚拟鼠标驱动...");
        run_bat_elevated(&uninstall_bat)?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        info!("✅ vmouse 驱动卸载完成");
        Ok("虚拟鼠标驱动已卸载".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 设置 sunshine.conf 中的 virtual_mouse 配置
#[tauri::command]
pub async fn set_vmouse_config(enabled: bool) -> Result<String, String> {
    // 读取当前完整配置
    let mut config_map = crate::vdd::read_full_sunshine_config()
        .await
        .unwrap_or_default();

    // 更新 virtual_mouse 字段
    let value_str = if enabled { "enabled" } else { "disabled" };
    config_map.insert("virtual_mouse".to_string(), serde_json::json!(value_str));

    debug!("📝 更新 virtual_mouse = {}", value_str);

    // 通过 Sunshine API 保存
    sunshine::post_sunshine_config(&config_map).await?;
    info!("✅ virtual_mouse 配置已更新: {}", value_str);
    Ok(format!(
        "虚拟鼠标已{}",
        if enabled { "启用" } else { "禁用" }
    ))
}
