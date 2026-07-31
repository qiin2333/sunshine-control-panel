#[cfg(target_os = "windows")]
use crate::bat_runner;
use crate::sunshine;
#[cfg(target_os = "windows")]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use log::{debug, error, info, warn};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const VDD_TRACE_SESSION_NAME: &str = "ZakoVDD-Diagnostics";
const VDD_TRACE_PROVIDER_GUID: &str = "{B254994F-46E6-4719-80A0-0A3AA50D6CE5}";
const VDD_TRACE_FILE_PREFIX: &str = "zako-vdd";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VddTraceStatus {
    pub running: bool,
    pub directory: String,
    pub latest_file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct VddStatus {
    pub state: String,
    pub installed: bool,
    pub running: bool,
    pub control_available: bool,
    pub installed_version: String,
    pub bundled_version: String,
    pub version_match: bool,
    pub monitor_active: bool,
    pub status_text: String,
}

impl VddStatus {
    pub fn is_usable(&self) -> bool {
        self.running
            && self.control_available
            && matches!(self.state.as_str(), "ready" | "degraded")
    }
}

fn probe_value(output: &str, key: &str) -> String {
    output
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(&format!("{}=", key)))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn vdd_probe_command_line(install_script: &Path) -> String {
    // Starting the `/C` command with `call` avoids cmd.exe's special handling
    // for a command line whose first character is a quote.
    format!(r#"call "{}" --probe-only"#, install_script.display())
}

#[cfg(target_os = "windows")]
fn run_vdd_probe(install_script: &Path) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new("cmd")
        .args(["/d", "/s", "/c"])
        .raw_arg(vdd_probe_command_line(install_script))
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn classify_vdd_state(
    installed: bool,
    running: bool,
    version_match: bool,
    control_available: bool,
    problem_code: u32,
) -> &'static str {
    if !installed {
        "not_installed"
    } else if problem_code == 14 {
        "reboot_required"
    } else if !running {
        "unhealthy"
    } else if !version_match || !control_available {
        "degraded"
    } else {
        "ready"
    }
}

#[cfg(target_os = "windows")]
fn probe_vdd_status() -> VddStatus {
    let install_script = get_sunshine_path().join("scripts").join("install-vdd.bat");
    if !install_script.exists() {
        return VddStatus {
            state: "payload_missing".to_string(),
            status_text: format!("VDD 安装脚本不存在: {}", install_script.display()),
            ..Default::default()
        };
    }

    let output = match run_vdd_probe(&install_script) {
        Ok(output) => output,
        Err(error) => {
            return VddStatus {
                state: "unknown".to_string(),
                status_text: format!("无法启动 VDD 状态检测: {}", error),
                ..Default::default()
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);
    if !output.status.success() || probe_value(&combined, "VDD_PROBE_OK") != "1" {
        let payload_missing = combined.contains("driver payload not found")
            || combined.contains("configuration template not found");
        return VddStatus {
            state: if payload_missing {
                "payload_missing"
            } else {
                "unknown"
            }
            .to_string(),
            status_text: if payload_missing {
                "Sunshine 安装目录中缺少 VDD 驱动文件，请修复或重新安装 Sunshine".to_string()
            } else {
                "无法读取 VDD 驱动状态".to_string()
            },
            ..Default::default()
        };
    }

    let installed = probe_value(&combined, "VDD_DEVICE_PRESENT") == "1";
    let pnp_status = probe_value(&combined, "CURRENT_VDD_STATUS");
    let problem_code = probe_value(&combined, "CURRENT_VDD_PROBLEM")
        .parse::<u32>()
        .unwrap_or(0);
    let installed_version = probe_value(&combined, "CURRENT_VDD_VERSION");
    let bundled_version = probe_value(&combined, "BUNDLED_VDD_VERSION");
    let running = installed && pnp_status.eq_ignore_ascii_case("OK") && problem_code == 0;
    let version_match = running
        && !installed_version.is_empty()
        && !bundled_version.is_empty()
        && installed_version.eq_ignore_ascii_case(&bundled_version);
    let control_available = crate::vdd_ioctl::interface_available();

    let classified_state = classify_vdd_state(
        installed,
        running,
        version_match,
        control_available,
        problem_code,
    );
    let (state, status_text) = if classified_state == "not_installed" {
        ("not_installed", "尚未安装虚拟显示器驱动".to_string())
    } else if classified_state == "reboot_required" {
        (
            "reboot_required",
            "虚拟显示器驱动需要重启 Windows 后才能使用".to_string(),
        )
    } else if classified_state == "unhealthy" {
        (
            "unhealthy",
            format!(
                "虚拟显示器驱动状态异常（状态: {}, 问题码: {}）",
                pnp_status, problem_code
            ),
        )
    } else if classified_state == "degraded" {
        (
            "degraded",
            if !version_match {
                format!(
                    "已安装版本 {} 与 Sunshine 随包版本 {} 不一致",
                    installed_version, bundled_version
                )
            } else {
                "驱动已安装，但现代控制接口不可用".to_string()
            },
        )
    } else {
        ("ready", "虚拟显示器驱动已就绪".to_string())
    };

    VddStatus {
        state: state.to_string(),
        installed,
        running,
        control_available,
        installed_version,
        bundled_version,
        version_match,
        monitor_active: false,
        status_text,
    }
}

#[cfg(not(target_os = "windows"))]
fn probe_vdd_status() -> VddStatus {
    VddStatus {
        state: "unsupported".to_string(),
        status_text: "虚拟显示器驱动仅支持 Windows".to_string(),
        ..Default::default()
    }
}

#[tauri::command]
pub async fn get_vdd_status() -> Result<VddStatus, String> {
    let mut status = tokio::task::spawn_blocking(probe_vdd_status)
        .await
        .map_err(|error| format!("虚拟显示器驱动状态检测失败，请重试: {error}"))?;
    if let Ok(tray_state) = sunshine::get_tray_state().await {
        status.monitor_active = tray_state.vdd.active;
    }
    Ok(status)
}

async fn set_vdd_tray_option(action: &str, enabled: bool) -> Result<String, String> {
    if enabled && !get_vdd_status().await?.is_usable() {
        return Err("虚拟显示器驱动尚未就绪，请先安装或修复驱动".to_string());
    }

    let response = sunshine::post_tray_action(action, Some(enabled)).await?;
    if !response.status {
        return Err(if response.error.is_empty() {
            "Sunshine 拒绝了 VDD 设置变更".to_string()
        } else {
            response.error
        });
    }
    Ok(response.message)
}

#[tauri::command]
pub async fn set_vdd_keep_enabled(enabled: bool) -> Result<String, String> {
    set_vdd_tray_option("vdd_toggle_keep_enabled", enabled).await
}

#[tauri::command]
pub async fn set_vdd_headless_create_enabled(enabled: bool) -> Result<String, String> {
    set_vdd_tray_option("vdd_toggle_headless_create", enabled).await
}

async fn ensure_vdd_driver_uninstall_is_safe() -> Result<(), String> {
    let sessions = sunshine::get_active_sessions().await.map_err(|error| {
        warn!("检查 Sunshine 串流状态失败: {error}");
        "无法确认 Sunshine 串流状态，请重试后再卸载虚拟显示器驱动".to_string()
    })?;
    if !sessions.is_empty() {
        return Err("串流进行中，停止所有串流后才能卸载虚拟显示器驱动".to_string());
    }

    let tray_state = sunshine::get_tray_state().await.map_err(|error| {
        warn!("检查 Sunshine 托盘状态失败: {error}");
        "无法确认 Sunshine 虚拟显示器状态，请重试后再卸载驱动".to_string()
    })?;
    if tray_state.vdd.active {
        return Err("虚拟显示器当前仍处于活动状态，请先从托盘关闭它".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn install_vdd_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let install_script = get_sunshine_path().join("scripts").join("install-vdd.bat");
        if !install_script.exists() {
            return Err(format!(
                "VDD 安装脚本不存在: {}。请修复或重新安装 Sunshine。",
                install_script.display()
            ));
        }

        info!("调用 install-vdd.bat 安装或修复虚拟显示器驱动...");
        tokio::task::spawn_blocking(move || bat_runner::run_elevated(&install_script, "vdd", &[]))
            .await
            .map_err(|error| format!("VDD 安装任务执行失败: {error}"))??;
        tokio::time::sleep(tokio::time::Duration::from_millis(750)).await;

        let status = get_vdd_status().await?;
        if !status.installed || !status.running {
            return Err(format!("VDD 安装后验证失败: {}", status.status_text));
        }
        Ok("虚拟显示器驱动已安装并通过验证".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 更新 VDD XML 文件中的 colour、cursor 和 edid 节点
/// C++ 的 saveVddSettings 会保留这些字段，所以我们需要先写入
async fn update_vdd_xml_extra_fields(settings: &VddSettings) -> Result<(), String> {
    let vdd_xml_path = get_vdd_settings_path();

    // 读取现有 XML（如果存在）
    let mut vdd_settings = if vdd_xml_path.exists() {
        let content =
            fs::read_to_string(&vdd_xml_path).map_err(|e| format!("读取 VDD XML 失败: {}", e))?;

        from_str::<VddSettings>(&content).map_err(|e| format!("解析 VDD XML 失败: {}", e))?
    } else {
        // 如果文件不存在，使用默认配置
        debug!("  📄 VDD 配置文件不存在，使用默认配置");
        get_default_settings()
    };

    // 只更新 colour、cursor 和 edid 字段（其他字段会被 C++ 更新）
    if let Some(ref colour) = settings.colour {
        vdd_settings.colour = Some(colour.clone());
        debug!("  ✓ 更新 colour 配置");
    }

    if let Some(ref cursor) = settings.cursor {
        vdd_settings.cursor = Some(cursor.clone());
        debug!(
            "  ✓ 更新 cursor 配置: HardwareCursor={}",
            cursor.hardware_cursor
        );
    } else if vdd_settings.cursor.is_none() {
        vdd_settings.cursor = default_cursor();
        debug!("  ✓ 补齐默认 cursor 配置");
    }

    if let Some(ref edid) = settings.edid {
        vdd_settings.edid = Some(edid.clone());
        debug!(
            "  ✓ 更新 edid 配置: CustomEdid={}, PreventSpoof={}, CeaOverride={}, Vrr={}",
            edid.custom_edid, edid.prevent_spoof, edid.edid_cea_override, edid.vrr
        );
    }

    // 序列化回 XML
    let xml = serialize_vdd_settings(&vdd_settings)?;

    // 添加 XML 声明
    let full_xml = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}", xml);

    // 写入文件
    write_vdd_xml(&vdd_xml_path, &full_xml).await?;

    // 验证文件是否更新
    verify_vdd_xml(&vdd_xml_path)?;

    Ok(())
}

fn serialize_vdd_settings(settings: &VddSettings) -> Result<String, String> {
    let mut xml = String::new();
    let mut ser = quick_xml::se::Serializer::with_root(&mut xml, Some("vdd_settings"))
        .map_err(|e| format!("创建 VDD XML 序列化器失败: {}", e))?;
    ser.indent(' ', 2);
    settings
        .serialize(ser)
        .map_err(|e| format!("序列化 VDD XML 失败: {}", e))?;
    Ok(xml)
}

/// 写入 VDD XML 文件（Windows - 使用管理员权限）
#[cfg(target_os = "windows")]
async fn write_vdd_xml(vdd_xml_path: &PathBuf, content: &str) -> Result<(), String> {
    // 写入临时文件
    let temp_path = std::env::temp_dir().join(format!("vdd_extra_{}.xml", std::process::id()));
    debug!("  📝 写入临时文件: {:?}", temp_path);
    fs::write(&temp_path, content).map_err(|e| format!("写入临时文件失败: {}", e))?;

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
                    warn!(
                        "  ⚠️ ShellExecuteW 复制后读取失败 ({}), 准备回退到 PowerShell",
                        err
                    );
                    false
                }
            }
        }
        Err(err) => {
            warn!(
                "  ⚠️ ShellExecuteW 提权复制调用失败 ({}), 准备回退到 PowerShell",
                err
            );
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

        debug!("  🔧 执行 PowerShell 提权命令...");

        let powershell_success = match run_elevated_powershell(&inner_command, "写入 VDD XML").await
        {
            Ok(()) => match fs::read_to_string(vdd_xml_path) {
                Ok(written) if written == content => {
                    info!("  ✅ PowerShell 提权复制成功");
                    true
                }
                Ok(_) => {
                    warn!("  ⚠️ PowerShell 复制后内容不匹配，准备回退到直接写入");
                    false
                }
                Err(err) => {
                    warn!("  ⚠️ PowerShell 复制后读取失败 ({err})，准备回退到直接写入");
                    false
                }
            },
            Err(err) => {
                warn!("  ⚠️ PowerShell 提权复制失败 ({err})，准备回退到直接写入");
                false
            }
        };

        if !powershell_success {
            error!("  ❌ PowerShell 提权复制失败");

            // 尝试直接写入（可能会因权限不足而失败）
            warn!("  ⚠️ 尝试直接写入...");
            fs::write(vdd_xml_path, content).map_err(|e| {
                // 清理临时文件
                let _ = fs::remove_file(&temp_path);
                format!("写入失败，需要管理员权限: {}", e)
            })?;
            info!("  ✓ 直接写入成功");
        }
    }

    // 清理临时文件
    let _ = fs::remove_file(&temp_path);

    Ok(())
}

#[cfg(target_os = "windows")]
fn elevated_copy_with_shell_execute(source: &Path, destination: &Path) -> Result<(), String> {
    use std::path::PathBuf;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::PCWSTR;

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
            Some(HWND(std::ptr::null_mut())),
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

#[cfg(target_os = "windows")]
async fn run_elevated_powershell(inner_command: &str, action_label: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let escaped_command = inner_command.replace("'", "''");
    let ps_script = format!(
        "$proc = Start-Process powershell -ArgumentList '-NoProfile', '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $proc.ExitCode",
        escaped_command
    );

    let status = Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("{}失败: {}", action_label, e))?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(format!("{}失败，PowerShell 退出码: {}", action_label, code));
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    Ok(())
}

/// 写入 VDD XML 文件（非 Windows 平台）
#[cfg(not(target_os = "windows"))]
async fn write_vdd_xml(vdd_xml_path: &PathBuf, content: &str) -> Result<(), String> {
    // 确保目录存在
    if let Some(parent) = vdd_xml_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    fs::write(vdd_xml_path, content).map_err(|e| format!("写入 VDD XML 失败: {}", e))?;

    debug!("  ✓ 已写入 VDD XML 扩展字段");

    Ok(())
}

/// 验证 VDD XML 文件
fn verify_vdd_xml(vdd_xml_path: &PathBuf) -> Result<(), String> {
    if !vdd_xml_path.exists() {
        return Err("验证失败: 文件不存在".to_string());
    }

    let verify_content =
        fs::read_to_string(vdd_xml_path).map_err(|e| format!("验证文件失败: {}", e))?;

    if verify_content.contains("<colour>")
        || verify_content.contains("<cursor>")
        || verify_content.contains("<edid>")
    {
        debug!("  ✅ 验证: VDD XML 扩展字段已写入");
    } else {
        warn!("  ⚠️  警告: 未在文件中找到 VDD XML 扩展字段");
    }

    Ok(())
}

/// 读取完整的 sunshine.conf 配置文件为 Map
pub async fn read_full_sunshine_config()
-> Result<serde_json::Map<String, serde_json::Value>, String> {
    let config_path = PathBuf::from(sunshine::get_sunshine_install_path())
        .join("config")
        .join("sunshine.conf");

    let mut config_map = serde_json::Map::new();

    if !config_path.exists() {
        warn!("⚠️  配置文件不存在: {:?}", config_path);
        return Ok(config_map);
    }

    let content =
        fs::read_to_string(&config_path).map_err(|e| format!("读取 sunshine.conf 失败: {}", e))?;

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
    // 读取完整的现有配置，然后更新 VDD 相关的配置项
    // 这样可以避免丢失其他配置
    let mut config_data = read_full_sunshine_config().await?;

    debug!("🔄 合并 VDD 配置到现有配置中");

    // 更新分辨率配置 - 格式: [1920x1080,2560x1440] (不带引号)
    if !settings.resolutions.resolution.is_empty() {
        let resolutions: Vec<String> = settings
            .resolutions
            .resolution
            .iter()
            .map(|r| format!("{}x{}", r.width, r.height))
            .collect();

        // 序列化为 JSON 字符串，然后去掉引号，匹配前端格式
        let resolutions_json = serde_json::to_string(&resolutions)
            .unwrap_or_else(|_| "[]".to_string())
            .replace("\"", ""); // 去掉所有引号

        // 更新或插入到配置中
        config_data.insert(
            "resolutions".to_string(),
            serde_json::json!(resolutions_json),
        );
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
    // 注意：VDD 的 vdd_settings.xml 模板里 <friendlyname> 默认是字面字符串 "default"
    // （在 VDD 侧表示「自动挑最佳 GPU」的哨兵值）。如果原样传给 Sunshine，
    // display_base.cpp 会把它当成精确 GPU 名 wstring 比较，导致所有 adapter 被跳过、
    // 报 "Failed to locate an output device" / 503 (AlkaidLab/foundation-sunshine#671)。
    // 这里把 "default"/"auto" 视为空，不写入 sunshine config。
    let trimmed = settings.gpu.friendlyname.trim();
    if !trimmed.is_empty()
        && !trimmed.eq_ignore_ascii_case("default")
        && !trimmed.eq_ignore_ascii_case("auto")
    {
        config_data.insert("adapter_name".to_string(), serde_json::json!(trimmed));
        debug!("  ✓ GPU: {}", trimmed);
    } else if !trimmed.is_empty() {
        debug!(
            "  ⚠ 忽略 VDD 哨兵 friendlyname={:?}，让 Sunshine 自动选卡",
            trimmed
        );
    }

    // 调用 Sunshine Config API
    debug!("📡 调用 Sunshine Config API");
    debug!("📝 配置数据: {:?}", config_data);

    sunshine::post_sunshine_config(&config_data).await?;
    info!("✅ VDD 配置已通过 Sunshine API 保存");
    Ok(())
}

fn default_true() -> bool {
    true
}

fn default_cursor_max_size() -> u32 {
    128
}

fn default_xor_cursor_support_level() -> u32 {
    2
}

fn default_edid_profile() -> String {
    "auto".to_string()
}

fn default_cursor() -> Option<Cursor> {
    Some(Cursor::default())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VddSettings {
    pub monitors: Monitors,
    pub gpu: Gpu,
    pub global: Global,
    pub resolutions: Resolutions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    #[serde(default = "default_cursor", skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
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
    pub g_refresh_rate: Vec<String>,
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
pub struct Cursor {
    #[serde(rename = "HardwareCursor", default = "default_true")]
    pub hardware_cursor: bool,
    #[serde(rename = "CursorMaxY", default = "default_cursor_max_size")]
    pub cursor_max_y: u32,
    #[serde(rename = "CursorMaxX", default = "default_cursor_max_size")]
    pub cursor_max_x: u32,
    #[serde(rename = "AlphaCursorSupport", default = "default_true")]
    pub alpha_cursor_support: bool,
    #[serde(
        rename = "XorCursorSupportLevel",
        default = "default_xor_cursor_support_level"
    )]
    pub xor_cursor_support_level: u32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            hardware_cursor: true,
            cursor_max_y: default_cursor_max_size(),
            cursor_max_x: default_cursor_max_size(),
            alpha_cursor_support: true,
            xor_cursor_support_level: default_xor_cursor_support_level(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EdidConfig {
    #[serde(rename = "CustomEdid")]
    pub custom_edid: bool,
    #[serde(rename = "PreventSpoof")]
    pub prevent_spoof: bool,
    #[serde(rename = "EdidCeaOverride")]
    pub edid_cea_override: bool,
    #[serde(rename = "EdidProfile", default = "default_edid_profile")]
    pub edid_profile: String,
    #[serde(rename = "Vrr", default)]
    pub vrr: bool,
}

/// 获取 Sunshine 安装路径
fn get_sunshine_path() -> PathBuf {
    PathBuf::from(sunshine::get_sunshine_install_path())
}

/// 从注册表读取 VDD 设置目录路径
#[cfg(target_os = "windows")]
fn get_vdd_base_path() -> Result<PathBuf, String> {
    use winreg::RegKey;
    use winreg::enums::*;

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

/// 获取 VDD ETW 采集文件目录
fn get_vdd_trace_dir() -> PathBuf {
    get_sunshine_path()
        .join("config")
        .join("logs")
        .join("vdd-traces")
}

fn quote_powershell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn quote_powershell_path(path: &Path) -> String {
    quote_powershell(path.to_string_lossy().as_ref())
}

fn latest_vdd_trace_file(trace_dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;

    for entry in fs::read_dir(trace_dir).ok()? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        let is_etl = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("etl"));

        if !is_etl {
            continue;
        }

        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            continue;
        };
        if match newest.as_ref() {
            Some((newest_modified, _)) => modified > *newest_modified,
            None => true,
        } {
            newest = Some((modified, path));
        }
    }

    newest.map(|(_, path)| path)
}

#[cfg(target_os = "windows")]
fn is_vdd_trace_running() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new("logman")
        .args(["query", VDD_TRACE_SESSION_NAME, "-ets"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_vdd_trace_running() -> bool {
    false
}

fn build_vdd_trace_status() -> VddTraceStatus {
    let trace_dir = get_vdd_trace_dir();
    let latest_file =
        latest_vdd_trace_file(&trace_dir).map(|path| path.to_string_lossy().into_owned());

    VddTraceStatus {
        running: is_vdd_trace_running(),
        directory: trace_dir.to_string_lossy().into_owned(),
        latest_file,
    }
}

/// 获取 VDD 设置文件路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_settings_file_path() -> String {
    get_vdd_settings_path().to_string_lossy().to_string()
}

/// 获取 VDD 工具目录路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_tools_dir_path() -> String {
    get_vdd_tools_path().to_string_lossy().to_string()
}

/// 获取 VDD EDID 文件路径（暴露给前端）
#[tauri::command]
pub fn get_vdd_edid_file_path() -> String {
    get_vdd_edid_path().to_string_lossy().to_string()
}

fn get_default_settings() -> VddSettings {
    VddSettings {
        monitors: Monitors { count: 1 },
        gpu: Gpu {
            friendlyname: String::new(),
        },
        global: Global {
            g_refresh_rate: vec!["60".to_string(), "120".to_string(), "240".to_string()],
        },
        resolutions: Resolutions { resolution: vec![] },
        colour: Some(Colour {
            sdr10bit: false,
            hdr_plus: false,
            colour_format: "RGB".to_string(),
        }),
        cursor: default_cursor(),
        edid: Some(EdidConfig {
            custom_edid: false,
            prevent_spoof: false,
            edid_cea_override: false,
            edid_profile: default_edid_profile(),
            vrr: false,
        }),
    }
}

#[tauri::command]
pub async fn load_vdd_settings() -> Result<VddSettings, String> {
    let path = get_vdd_settings_path();

    if !path.exists() {
        return Ok(get_default_settings());
    }

    let content = fs::read_to_string(&path).map_err(|e| format!("读取配置文件失败: {}", e))?;

    debug!("📄 读取到的 XML 内容:\n{}", content);

    // 解析 XML
    let settings: VddSettings = from_str(&content).map_err(|e| {
        error!("❌ XML 解析失败: {}", e);
        error!("📄 XML 内容:\n{}", content);
        format!("XML 解析失败: {}", e)
    })?;

    info!("✅ XML 解析成功！");
    debug!("🔍 解析后的 VDD 设置: {:?}", settings);
    debug!("🔍 解析后的 GPU 名称: {}", settings.gpu.friendlyname);
    debug!(
        "🔍 解析后的分辨率数量: {}",
        settings.resolutions.resolution.len()
    );
    debug!(
        "🔍 解析后的全局刷新率: {:?}",
        settings.global.g_refresh_rate
    );

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

    // 步骤3: 写入 colour、cursor 和 edid 到 XML
    // 读取 C++ 刚写入的 XML，添加 colour、cursor 和 edid，然后写回
    debug!("📝 写入 colour、cursor 和 edid 字段...");
    update_vdd_xml_extra_fields(&settings).await?;

    // 步骤4: 通知 VDD 驱动重新加载配置
    #[cfg(target_os = "windows")]
    {
        debug!("🔄 通知 VDD 驱动重新加载...");
        let _ = exec_vdd_cmd("RELOAD_DRIVER".to_string()).await;
    }

    info!("✅ VDD 配置保存完成");
    Ok("保存成功".to_string())
}

#[cfg(target_os = "windows")]
const ELEVATED_VDD_IOCTL_ARG: &str = "--elevated-vdd-ioctl";

#[cfg(target_os = "windows")]
fn is_allowed_elevated_vdd_command(command: &str) -> bool {
    matches!(
        command,
        "RELOAD_DRIVER" | "HARDWARECURSOR true" | "HARDWARECURSOR false"
    )
}

/// Handle the narrow, elevated helper mode before Tauri/WebView startup.
/// The command is allowlisted and encoded so it never becomes PowerShell syntax.
#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_ioctl_command() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next()?;
    if mode != ELEVATED_VDD_IOCTL_ARG {
        return None;
    }

    let encoded = match args.next().and_then(|arg| arg.into_string().ok()) {
        Some(encoded) if args.next().is_none() => encoded,
        _ => return Some(2),
    };
    let command = URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    let Some(command) = command else {
        return Some(2);
    };
    if !is_allowed_elevated_vdd_command(&command) {
        return Some(2);
    }

    match crate::vdd_ioctl::send_command(&command) {
        crate::vdd_ioctl::IoctlResult::Success => Some(0),
        crate::vdd_ioctl::IoctlResult::InterfaceMissing
        | crate::vdd_ioctl::IoctlResult::Failed { .. } => Some(1),
    }
}

#[tauri::command]
pub async fn exec_vdd_cmd(command: String) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        if !is_allowed_elevated_vdd_command(&command) {
            warn!("拒绝非白名单 VDD 控制命令");
            return Err("不允许的 VDD 控制命令".to_string());
        }

        use crate::vdd_ioctl;
        use windows::Win32::Foundation::ERROR_ACCESS_DENIED;

        let cmd_for_blocking = command.clone();
        let in_process = tokio::task::spawn_blocking(move || -> Result<bool, (bool, String)> {
            match vdd_ioctl::send_command(&cmd_for_blocking) {
                vdd_ioctl::IoctlResult::Success => return Ok(true),
                vdd_ioctl::IoctlResult::Failed {
                    message,
                    win32_error,
                } => {
                    let access_denied = win32_error == Some(ERROR_ACCESS_DENIED.0);
                    Err((access_denied, format!("vdd_ioctl: {message}")))
                }
                vdd_ioctl::IoctlResult::InterfaceMissing => {
                    Err((false, "VDD IOCTL interface missing".to_string()))
                }
            }
        })
        .await
        .map_err(|e| e.to_string())?;

        match in_process {
            Ok(v) => Ok(v),
            Err((true, msg)) => {
                warn!("  ⚠️ VDD 命令在普通权限下被拒 ({msg})，回退到提权执行");
                run_elevated_ioctl_command(&command).await?;
                Ok(true)
            }
            Err((false, msg)) => Err(msg),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(true)
    }
}

/// Re-run this executable in a minimal elevated mode that only sends an
/// allowlisted IOCTL command, without starting Tauri or a WebView.
#[cfg(target_os = "windows")]
async fn run_elevated_ioctl_command(command: &str) -> Result<(), String> {
    if !is_allowed_elevated_vdd_command(command) {
        return Err("不允许的 VDD 提权命令".to_string());
    }

    let executable = std::env::current_exe().map_err(|e| format!("无法定位控制面板程序: {e}"))?;
    let executable = executable.to_string_lossy().replace('\'', "''");
    let encoded = URL_SAFE_NO_PAD.encode(command.as_bytes());
    let inner = format!(
        "$p = Start-Process -FilePath '{}' -ArgumentList '{}','{}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode",
        executable, ELEVATED_VDD_IOCTL_ARG, encoded
    );
    run_elevated_powershell(&inner, "提权执行 VDD IOCTL").await
}

/// 验证 EDID 文件格式和 checksum
fn validate_edid(data: &[u8]) -> Result<(), String> {
    // EDID 必须是 128 或 256 字节
    if data.len() != 128 && data.len() != 256 {
        return Err(format!(
            "EDID 文件大小无效: {} 字节（必须是 128 或 256 字节）",
            data.len()
        ));
    }

    // 验证 EDID 头部 (前8字节应该是: 00 FF FF FF FF FF FF 00)
    let expected_header: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
    if data.len() >= 8 && &data[0..8] != &expected_header {
        return Err("EDID 头部格式无效".to_string());
    }

    // 每个 128 字节块都必须有独立的 checksum
    for (block_index, block) in data.chunks(128).enumerate() {
        let mut sum: u32 = 0;
        for byte in &block[..127] {
            sum += *byte as u32;
        }
        sum %= 256;

        let expected_checksum = if sum != 0 { (256 - sum) as u8 } else { 0 };
        if block[127] != expected_checksum {
            return Err(format!(
                "EDID 第 {} 个数据块 checksum 无效: 期望 0x{:02X}，实际 0x{:02X}",
                block_index + 1,
                expected_checksum,
                block[127]
            ));
        }
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
                let inner_command = format!(
                    "New-Item -ItemType Directory -Force -Path '{}' | Out-Null",
                    parent.display()
                );
                run_elevated_powershell(&inner_command, "创建 EDID 目录").await?;

                if !parent.exists() {
                    return Err(format!("创建目录失败: {}", parent.display()));
                }
            }

            #[cfg(not(target_os = "windows"))]
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }

    // 写入临时文件
    let temp_path = std::env::temp_dir().join(format!("user_edid_{}.bin", std::process::id()));
    fs::write(&temp_path, &file_data).map_err(|e| format!("写入临时文件失败: {}", e))?;

    // 使用管理员权限复制文件
    #[cfg(target_os = "windows")]
    {
        let inner_command = format!(
            "Copy-Item -Path '{}' -Destination '{}' -Force",
            temp_path.display(),
            edid_path.display()
        );

        if let Err(error) = run_elevated_powershell(&inner_command, "复制 EDID 文件").await {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        fs::copy(&temp_path, &edid_path).map_err(|e| format!("复制 EDID 文件失败: {}", e))?;
    }

    // 清理临时文件
    let _ = fs::remove_file(&temp_path);

    // 验证文件是否成功写入
    if !edid_path.exists() {
        return Err("EDID 文件写入失败".to_string());
    }

    let saved_data = fs::read(&edid_path).map_err(|e| format!("验证 EDID 文件失败: {}", e))?;
    if saved_data != file_data {
        return Err("EDID 文件写入校验失败，目标内容与上传内容不一致".to_string());
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

    let data = fs::read(&edid_path).map_err(|e| format!("读取 EDID 文件失败: {}", e))?;

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
        let inner_command = format!("Remove-Item -Path '{}' -Force", edid_path.display());
        run_elevated_powershell(&inner_command, "删除 EDID 文件").await?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        fs::remove_file(&edid_path).map_err(|e| format!("删除 EDID 文件失败: {}", e))?;
    }

    if edid_path.exists() {
        return Err("EDID 文件删除校验失败，目标文件仍然存在".to_string());
    }

    info!("✅ EDID 文件已删除");
    Ok("EDID 文件已删除".to_string())
}

#[tauri::command]
pub fn get_vdd_trace_status() -> VddTraceStatus {
    build_vdd_trace_status()
}

#[tauri::command]
pub async fn start_vdd_trace() -> Result<VddTraceStatus, String> {
    #[cfg(target_os = "windows")]
    {
        if is_vdd_trace_running() {
            return Ok(build_vdd_trace_status());
        }

        let trace_dir = get_vdd_trace_dir();
        let trace_file = trace_dir.join(format!(
            "{}-{}.etl",
            VDD_TRACE_FILE_PREFIX,
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        let command = format!(
            "New-Item -ItemType Directory -Force -Path {} | Out-Null; logman start {} -ets -p {} 0xFFFFFFFF 0x5 -o {}",
            quote_powershell_path(&trace_dir),
            quote_powershell(VDD_TRACE_SESSION_NAME),
            quote_powershell(VDD_TRACE_PROVIDER_GUID),
            quote_powershell_path(&trace_file)
        );

        run_elevated_powershell(&command, "启动 VDD ETW 采集").await?;
        Ok(build_vdd_trace_status())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("VDD ETW trace capture is only supported on Windows".to_string())
    }
}

#[tauri::command]
pub async fn stop_vdd_trace() -> Result<VddTraceStatus, String> {
    #[cfg(target_os = "windows")]
    {
        if !is_vdd_trace_running() {
            return Ok(build_vdd_trace_status());
        }

        let command = format!(
            "logman stop {} -ets",
            quote_powershell(VDD_TRACE_SESSION_NAME)
        );
        run_elevated_powershell(&command, "停止 VDD ETW 采集").await?;
        Ok(build_vdd_trace_status())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("VDD ETW trace capture is only supported on Windows".to_string())
    }
}

#[tauri::command]
pub fn open_vdd_trace_folder() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let trace_dir = get_vdd_trace_dir();
        fs::create_dir_all(&trace_dir)
            .map_err(|e| format!("Failed to create VDD ETW trace folder: {}", e))?;

        let trace_dir_text = trace_dir.to_string_lossy().into_owned();
        Command::new("cmd")
            .args(["/c", "start", "", &trace_dir_text])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open VDD ETW trace folder: {}", e))?;

        Ok(trace_dir_text)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Opening the VDD ETW trace folder is only supported on Windows".to_string())
    }
}

#[tauri::command]
pub async fn uninstall_vdd_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        ensure_vdd_driver_uninstall_is_safe().await?;
        let uninstall_script = get_sunshine_path()
            .join("scripts")
            .join("uninstall-vdd.bat");
        if !uninstall_script.exists() {
            return Err(format!(
                "VDD 卸载脚本不存在: {}。请修复或重新安装 Sunshine。",
                uninstall_script.display()
            ));
        }

        info!("调用 uninstall-vdd.bat 卸载虚拟显示器驱动...");
        tokio::task::spawn_blocking(move || {
            bat_runner::run_elevated(&uninstall_script, "vdd", &[])
        })
        .await
        .map_err(|error| format!("VDD 卸载任务执行失败: {error}"))??;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let status = get_vdd_status().await?;
        if status.installed {
            return Err(format!("VDD 卸载后验证失败: {}", status.status_text));
        }
        Ok("虚拟显示器驱动已卸载".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(state: &str, running: bool, control_available: bool) -> VddStatus {
        VddStatus {
            state: state.to_string(),
            running,
            control_available,
            ..Default::default()
        }
    }

    #[test]
    fn only_running_ready_or_degraded_status_with_control_is_usable() {
        assert!(status("ready", true, true).is_usable());
        assert!(status("degraded", true, true).is_usable());
        assert!(!status("ready", true, false).is_usable());
        assert!(!status("ready", false, true).is_usable());
        assert!(!status("not_installed", false, false).is_usable());
        assert!(!status("unhealthy", true, true).is_usable());
        assert!(!status("reboot_required", true, true).is_usable());
    }

    #[test]
    fn probe_value_prefers_the_last_machine_readable_result() {
        let output = "CURRENT_VDD_STATUS=Unknown\nnoise\nCURRENT_VDD_STATUS=OK\n";
        assert_eq!(probe_value(output, "CURRENT_VDD_STATUS"), "OK");
        assert_eq!(probe_value(output, "MISSING"), "");
    }

    #[test]
    fn probe_command_quotes_the_script_without_literal_backslashes() {
        let script = Path::new(r"C:\Program Files\Sunshine\scripts\install-vdd.bat");

        assert_eq!(
            vdd_probe_command_line(script),
            r#"call "C:\Program Files\Sunshine\scripts\install-vdd.bat" --probe-only"#
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn probe_command_executes_a_quoted_batch_path() {
        let directory =
            std::env::temp_dir().join(format!("sunshine vdd probe test {}", std::process::id()));
        let script = directory.join("probe script.bat");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&script, "@echo off\r\necho VDD_PROBE_OK=1\r\n").unwrap();

        let output = run_vdd_probe(&script).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = fs::remove_dir_all(&directory);

        assert!(
            output.status.success(),
            "status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(probe_value(&stdout, "VDD_PROBE_OK"), "1");
    }

    #[test]
    fn classifies_every_vdd_prerequisite_state() {
        assert_eq!(
            classify_vdd_state(false, false, false, false, 0),
            "not_installed"
        );
        assert_eq!(
            classify_vdd_state(true, false, false, false, 14),
            "reboot_required"
        );
        assert_eq!(
            classify_vdd_state(true, false, false, false, 10),
            "unhealthy"
        );
        assert_eq!(classify_vdd_state(true, true, false, true, 0), "degraded");
        assert_eq!(classify_vdd_state(true, true, true, false, 0), "degraded");
        assert_eq!(classify_vdd_state(true, true, true, true, 0), "ready");
    }

    const MINIMAL_VDD_XML_WITHOUT_CURSOR: &str = r#"
<vdd_settings>
    <monitors><count>1</count></monitors>
    <gpu><friendlyname></friendlyname></gpu>
    <global><g_refresh_rate>60</g_refresh_rate></global>
    <resolutions>
        <resolution><width>1920</width><height>1080</height></resolution>
    </resolutions>
</vdd_settings>
"#;

    #[test]
    fn missing_cursor_section_defaults_to_hardware_cursor_enabled() {
        let settings: VddSettings = from_str(MINIMAL_VDD_XML_WITHOUT_CURSOR).unwrap();
        let cursor = settings
            .cursor
            .expect("cursor defaults should be populated");

        assert!(cursor.hardware_cursor);
        assert_eq!(cursor.cursor_max_x, 128);
        assert_eq!(cursor.cursor_max_y, 128);
        assert!(cursor.alpha_cursor_support);
        assert_eq!(cursor.xor_cursor_support_level, 2);
    }

    #[test]
    fn default_settings_serialize_cursor_section() {
        let xml = serialize_vdd_settings(&get_default_settings()).unwrap();

        // 根标签必须是小写 vdd_settings，而非结构体名 VddSettings
        assert!(xml.contains("<vdd_settings>"));
        assert!(!xml.contains("<VddSettings>"));
        assert!(xml.contains("<cursor>"));
        assert!(xml.contains("<HardwareCursor>true</HardwareCursor>"));
        assert!(xml.contains("<CursorMaxX>128</CursorMaxX>"));
        assert!(xml.contains("<CursorMaxY>128</CursorMaxY>"));
        assert!(xml.contains("<AlphaCursorSupport>true</AlphaCursorSupport>"));
        assert!(xml.contains("<XorCursorSupportLevel>2</XorCursorSupportLevel>"));
    }

    #[test]
    fn explicit_hardware_cursor_false_is_preserved() {
        let xml = r#"
<vdd_settings>
    <monitors><count>1</count></monitors>
    <gpu><friendlyname></friendlyname></gpu>
    <global><g_refresh_rate>60</g_refresh_rate></global>
    <resolutions>
        <resolution><width>1920</width><height>1080</height></resolution>
    </resolutions>
    <cursor>
        <HardwareCursor>false</HardwareCursor>
        <CursorMaxY>64</CursorMaxY>
        <CursorMaxX>64</CursorMaxX>
        <AlphaCursorSupport>false</AlphaCursorSupport>
        <XorCursorSupportLevel>1</XorCursorSupportLevel>
    </cursor>
</vdd_settings>
"#;

        let settings: VddSettings = from_str(xml).unwrap();
        let cursor = settings.cursor.expect("cursor section should parse");

        assert!(!cursor.hardware_cursor);
        assert_eq!(cursor.cursor_max_x, 64);
        assert_eq!(cursor.cursor_max_y, 64);
        assert!(!cursor.alpha_cursor_support);
        assert_eq!(cursor.xor_cursor_support_level, 1);
    }

    #[test]
    fn missing_edid_profile_defaults_to_auto() {
        // <edid> 存在但缺 <EdidProfile>（老版本文件），应默认回 "auto"
        let xml = r#"
<vdd_settings>
    <monitors><count>1</count></monitors>
    <gpu><friendlyname></friendlyname></gpu>
    <global><g_refresh_rate>60</g_refresh_rate></global>
    <resolutions>
        <resolution><width>1920</width><height>1080</height></resolution>
    </resolutions>
    <edid>
        <CustomEdid>false</CustomEdid>
        <PreventSpoof>false</PreventSpoof>
        <EdidCeaOverride>false</EdidCeaOverride>
        <Vrr>false</Vrr>
    </edid>
</vdd_settings>
"#;

        let settings: VddSettings = from_str(xml).unwrap();
        let edid = settings.edid.expect("edid section should parse");

        assert_eq!(edid.edid_profile, "auto");
    }

    #[test]
    fn default_settings_serialize_edid_profile() {
        // round-trip 必须输出 <EdidProfile>，否则会重蹈丢标签覆辙
        let xml = serialize_vdd_settings(&get_default_settings()).unwrap();

        assert!(xml.contains("<EdidProfile>auto</EdidProfile>"));
    }

    #[test]
    fn explicit_edid_profile_is_preserved_through_round_trip() {
        // 非默认值（modern）必须能跨反序列化→序列化存活
        let xml = r#"
<vdd_settings>
    <monitors><count>1</count></monitors>
    <gpu><friendlyname></friendlyname></gpu>
    <global><g_refresh_rate>60</g_refresh_rate></global>
    <resolutions>
        <resolution><width>1920</width><height>1080</height></resolution>
    </resolutions>
    <edid>
        <CustomEdid>false</CustomEdid>
        <PreventSpoof>false</PreventSpoof>
        <EdidCeaOverride>false</EdidCeaOverride>
        <EdidProfile>modern</EdidProfile>
        <Vrr>false</Vrr>
    </edid>
</vdd_settings>
"#;

        let settings: VddSettings = from_str(xml).unwrap();
        assert_eq!(
            settings.edid.as_ref().expect("edid section").edid_profile,
            "modern"
        );

        let out = serialize_vdd_settings(&settings).unwrap();
        assert!(out.contains("<EdidProfile>modern</EdidProfile>"));
    }
}
