use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use log::{info, warn, debug};
use crate::sunshine;

/// 检查当前进程是否具有管理员权限
#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
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

/// 以适当权限运行 bat 脚本：已有管理员权限则直接运行，否则提权
#[cfg(target_os = "windows")]
fn run_bat_elevated(bat_path: &std::path::Path) -> Result<(), String> {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    if is_elevated() {
        // 已有管理员权限，直接运行
        let output = Command::new("cmd")
            .args(&["/c", &bat_path.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("脚本执行失败: {}", stderr));
        }
    } else {
        // 需要提权
        let ps_cmd = format!(
            r#"Start-Process cmd -ArgumentList '/c','""{bat}""' -Verb RunAs -WindowStyle Hidden -Wait"#,
            bat = bat_path.display()
        );

        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("脚本执行失败: {}", stderr));
        }
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

    // 使用 PowerShell 的 Get-PnpDevice 检查设备状态
    // 排除已删除但仍有残留记录的幽灵设备（FriendlyName 为空）
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&[
            "-NoProfile", "-Command",
            "Get-PnpDevice -InstanceId 'ROOT\\HIDCLASS\\*' -ErrorAction SilentlyContinue | Where-Object { ($_.FriendlyName -like '*Virtual Mouse*' -or $_.HardwareID -contains 'Root\\ZakoVirtualMouse') -and $_.FriendlyName -ne $null -and $_.FriendlyName -ne '' } | Select-Object -First 1 Status, FriendlyName, Problem | ConvertTo-Json -Compress"
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
                let status = json.get("Status").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let friendly = json.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or("Zako Virtual Mouse");
                let problem = json.get("Problem").and_then(|v| v.as_u64()).unwrap_or(0);

                let running = status == "OK" && problem == 0;
                let status_text = if running {
                    format!("{} - 正常运行", friendly)
                } else if problem == 21 {
                    format!("{} - 需要重启", friendly)
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
    let sunshine_url = sunshine::get_sunshine_url().await
        .map_err(|e| format!("无法获取 Sunshine URL: {}", e))?;

    // 读取当前完整配置
    let config_path = PathBuf::from(sunshine::get_sunshine_install_path())
        .join("config")
        .join("sunshine.conf");

    let mut config_map = serde_json::Map::new();

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                config_map.insert(
                    key.trim().to_string(),
                    serde_json::json!(value.trim()),
                );
            }
        }
    }

    // 更新 virtual_mouse 字段
    let value_str = if enabled { "enabled" } else { "disabled" };
    config_map.insert("virtual_mouse".to_string(), serde_json::json!(value_str));

    debug!("📝 更新 virtual_mouse = {}", value_str);

    // 通过 Sunshine API 保存
    let config_url = format!("{}/api/config", sunshine_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client.post(&config_url)
        .json(&config_map)
        .send()
        .await
        .map_err(|e| format!("保存配置失败: {}", e))?;

    if response.status().is_success() {
        info!("✅ virtual_mouse 配置已更新: {}", value_str);
        Ok(format!("虚拟鼠标已{}", if enabled { "启用" } else { "禁用" }))
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!("保存配置失败 ({}): {}", status, body))
    }
}
