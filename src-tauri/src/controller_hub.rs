use crate::sunshine;
use log::{debug, info};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// 控制器中心相关的 sunshine.conf 配置（仿真模式 / DS4 行为 / DSU 体感服务器）
#[derive(Debug, Serialize, Clone)]
pub struct ControllerHubConfig {
    pub gamepad: String,
    pub motion_as_ds4: bool,
    pub touchpad_as_ds4: bool,
    pub ds4_back_as_touchpad_click: bool,
    pub enable_dsu_server: bool,
    pub dsu_server_port: u16,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct MicrophoneStatus {
    pub success: bool,
    pub configured_backend: String,
    pub active_backend: String,
    pub fallback_reason: String,
    pub component_available: bool,
    pub online: bool,
    pub device_created: bool,
    pub host_streaming: bool,
    pub generation: u32,
    pub state: String,
    pub buffered_bytes: u32,
    pub underruns: u32,
    pub dropped_frames: u32,
    pub submit_errors: u32,
    pub last_error: i32,
    pub error_code: String,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct MicrophoneTestResult {
    pub success: bool,
    pub error_code: String,
    pub backend: String,
}

const MAX_DEVICE_API_RESPONSE_BYTES: usize = 64 * 1024;
static CONTROLLER_HUB_CONFIG_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn read_device_api_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
    operation: &str,
) -> Result<T, String> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_DEVICE_API_RESPONSE_BYTES as u64)
    {
        return Err(format!("{operation} response exceeds 64 KiB"));
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_DEVICE_API_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Unable to read {operation} response: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_DEVICE_API_RESPONSE_BYTES {
            return Err(format!("{operation} response exceeds 64 KiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let detail = String::from_utf8_lossy(&bytes);
        return Err(format!("{operation} failed ({status}): {detail}"));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("Invalid {operation} response: {error}"))
}

async fn device_api_url(path: &str) -> Result<String, String> {
    let base_url = sunshine::get_sunshine_url().await?;
    Ok(format!("{}{}", base_url.trim_end_matches('/'), path))
}

#[tauri::command]
pub async fn get_virtual_microphone_status() -> Result<MicrophoneStatus, String> {
    let endpoint = device_api_url("/api/microphone/status").await?;
    let response = sunshine::create_https_client()?
        .get(endpoint)
        .send()
        .await
        .map_err(|error| format!("Unable to query virtual microphone status: {error}"))?;
    read_device_api_response(response, "virtual microphone status").await
}

#[tauri::command]
pub async fn test_virtual_microphone() -> Result<MicrophoneTestResult, String> {
    let endpoint = device_api_url("/api/microphone/test").await?;
    let response = sunshine::create_https_client()?
        .post(endpoint)
        .send()
        .await
        .map_err(|error| format!("Unable to test the virtual microphone: {error}"))?;
    read_device_api_response(response, "virtual microphone test").await
}

const GAMEPAD_MODES: [&str; 4] = ["auto", "x360", "ds4", "ds5"];

/// 对齐 Sunshine 核心 to_bool 的接受列表（大小写不敏感）
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "enable" | "enabled" | "on" | "1" => Some(true),
        "false" | "no" | "disable" | "disabled" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// 读取控制器中心配置，缺失键回退到核心默认值
#[tauri::command]
pub async fn get_controller_hub_config() -> Result<ControllerHubConfig, String> {
    let config_map = crate::vdd::read_full_sunshine_config().await?;

    let get_str = |key: &str| -> Option<String> {
        config_map
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
    };

    let gamepad = get_str("gamepad")
        .filter(|v| GAMEPAD_MODES.contains(&v.as_str()))
        .unwrap_or_else(|| "auto".to_string());

    let get_bool = |key: &str, default: bool| -> bool {
        get_str(key).and_then(|v| parse_bool(&v)).unwrap_or(default)
    };

    let port = get_str("dsu_server_port")
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|p| (1024..=65535).contains(p))
        .unwrap_or(26760);

    Ok(ControllerHubConfig {
        gamepad,
        motion_as_ds4: get_bool("motion_as_ds4", true),
        touchpad_as_ds4: get_bool("touchpad_as_ds4", true),
        ds4_back_as_touchpad_click: get_bool("ds4_back_as_touchpad_click", true),
        enable_dsu_server: get_bool("enable_dsu_server", false),
        dsu_server_port: port,
    })
}

/// 保存控制器中心配置；仅更新传入的字段（None 跳过）。
/// 值一律以字符串写入（核心 /api/config 按字符串解析，布尔/数字直接写 JSON 类型会解析失败）。
#[tauri::command]
pub async fn save_controller_hub_config(
    gamepad: Option<String>,
    motion_as_ds4: Option<bool>,
    touchpad_as_ds4: Option<bool>,
    ds4_back_as_touchpad_click: Option<bool>,
    enable_dsu_server: Option<bool>,
    dsu_server_port: Option<u16>,
) -> Result<String, String> {
    if let Some(mode) = gamepad.as_deref()
        && !GAMEPAD_MODES.contains(&mode)
    {
        return Err(format!("无效的仿真模式: {mode}（可选: auto/x360/ds4/ds5）"));
    }
    if let Some(port) = dsu_server_port
        && !(1024..=65535).contains(&port)
    {
        return Err(format!("DSU 端口必须在 1024-65535 之间: {port}"));
    }

    // Serialize the read/modify/write cycle so independent controls cannot
    // overwrite one another when users change them in quick succession.
    let _config_guard = CONTROLLER_HUB_CONFIG_LOCK.lock().await;
    let mut config_map = crate::vdd::read_full_sunshine_config().await?;

    let mut changed: Vec<&str> = Vec::new();
    if let Some(mode) = gamepad.as_deref() {
        config_map.insert("gamepad".to_string(), serde_json::json!(mode));
        changed.push("gamepad");
    }
    for (key, value) in [
        ("motion_as_ds4", motion_as_ds4),
        ("touchpad_as_ds4", touchpad_as_ds4),
        ("ds4_back_as_touchpad_click", ds4_back_as_touchpad_click),
        ("enable_dsu_server", enable_dsu_server),
    ] {
        if let Some(v) = value {
            config_map.insert(
                key.to_string(),
                serde_json::json!(if v { "true" } else { "false" }),
            );
            changed.push(key);
        }
    }
    if let Some(port) = dsu_server_port {
        config_map.insert(
            "dsu_server_port".to_string(),
            serde_json::json!(port.to_string()),
        );
        changed.push("dsu_server_port");
    }

    if changed.is_empty() {
        return Ok("没有需要保存的更改".to_string());
    }

    debug!("📝 控制器中心更新配置项: {:?}", changed);
    sunshine::post_sunshine_config(&config_map).await?;
    info!("✅ 控制器中心配置已保存: {:?}", changed);
    Ok("控制器设置已保存，重启 Sunshine 后生效".to_string())
}
