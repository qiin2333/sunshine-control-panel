//! Host opt-in settings, sharing sunshine.conf with the Web UI.
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Serialize)]
pub struct ForwardingConfig {
    supported: bool,
    enabled: bool,
    port: u16,
}

fn parse_config(config: &Map<String, Value>) -> ForwardingConfig {
    let value = |key: &str| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    ForwardingConfig {
        supported: value("usb_forwarding_config_version") == "1",
        enabled: matches!(
            value("usb_forwarding_enabled")
                .to_ascii_lowercase()
                .as_str(),
            "true" | "yes" | "enable" | "enabled" | "on" | "1"
        ),
        port: value("usb_forwarding_port").parse().unwrap_or(0),
    }
}

#[tauri::command]
pub async fn usbip_get_forwarding_config() -> Result<ForwardingConfig, String> {
    let base = crate::sunshine::get_sunshine_url().await?;
    let response = crate::sunshine::create_https_client()?
        .get(format!("{}/api/config", base.trim_end_matches('/')))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let config: Map<String, Value> =
        crate::device_config::read_device_api_response(response, "USB forwarding configuration")
            .await?;
    if config.get("status").and_then(Value::as_str) != Some("true") {
        return Err("Unable to read Sunshine configuration".into());
    }
    Ok(parse_config(&config))
}

#[tauri::command]
pub async fn usbip_save_forwarding_config(enabled: bool, port: u16) -> Result<(), String> {
    if port != 0 && port < 1024 {
        return Err("USB forwarding port must be 0 or 1024–65535".into());
    }
    let _guard = crate::device_config::DEVICE_CONFIG_LOCK.lock().await;
    if !usbip_get_forwarding_config().await?.supported {
        return Err("Update Sunshine to configure client USB forwarding".into());
    }
    let mut config = crate::vdd::read_full_sunshine_config().await?;
    config.insert(
        "usb_forwarding_enabled".into(),
        Value::from(if enabled { "enabled" } else { "disabled" }),
    );
    config.insert("usb_forwarding_port".into(), Value::from(port.to_string()));
    crate::sunshine::post_sunshine_config(&config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_do_not_claim_support_for_old_hosts() {
        let parsed = parse_config(&Map::new());
        assert!(!parsed.supported);
        assert!(!parsed.enabled);
        assert_eq!(parsed.port, 0);
    }
    #[test]
    fn reads_shared_configuration_without_rewriting_explicit_ports() {
        let config = serde_json::json!({"usb_forwarding_config_version":"1",
            "usb_forwarding_enabled":"Enabled", "usb_forwarding_port":"47996"});
        let parsed = parse_config(config.as_object().unwrap());
        assert!(parsed.supported && parsed.enabled);
        assert_eq!(parsed.port, 47996);
    }
}
