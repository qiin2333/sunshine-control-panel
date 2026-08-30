//! Core HTTP settings client for the independent DualSense configuration.

use log::warn;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};

pub(crate) const CONFIG_APPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub(crate) const CORE_CONFIG_READ_ATTEMPTS: usize = 3;
pub(crate) const CORE_CONFIG_RETRY_DELAY: std::time::Duration =
    std::time::Duration::from_millis(75);

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct CoreDualSenseSettings {
    pub(crate) ds5_enabled: bool,
    pub(crate) ds5_audio_haptics: bool,
    pub(crate) ds5_legacy_haptics_strength: f64,
    pub(crate) ds5_legacy_haptics_curve: f64,
    pub(crate) ds5_legacy_haptics_noise_gate: f64,
    pub(crate) ds5_genshin_compatibility: bool,
}

impl Default for CoreDualSenseSettings {
    fn default() -> Self {
        Self {
            ds5_enabled: false,
            ds5_audio_haptics: true,
            ds5_legacy_haptics_strength: 1.0,
            ds5_legacy_haptics_curve: 0.5,
            ds5_legacy_haptics_noise_gate: 0.020,
            ds5_genshin_compatibility: false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CoreDualSenseResponse {
    pub(crate) status: bool,
    pub(crate) applied: bool,
    pub(crate) revision: u64,
    #[serde(default)]
    pub(crate) changed: Option<bool>,
    #[serde(flatten)]
    pub(crate) settings: CoreDualSenseSettings,
}

#[derive(Debug)]
pub(crate) struct CoreDualSenseSnapshot {
    pub(crate) response: CoreDualSenseResponse,
    pub(crate) entity_tag: Option<HeaderValue>,
}

#[derive(Debug)]
pub(crate) enum CoreDualSenseResponseError {
    Transfer(reqwest::Error),
    Message(String),
}

impl From<String> for CoreDualSenseResponseError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

pub(crate) fn validate_strong_entity_tag(value: HeaderValue) -> Result<HeaderValue, String> {
    const MAX_ENTITY_TAG_SIZE: usize = 512;
    let bytes = value.as_bytes();
    let valid = bytes.len() >= 2
        && bytes.len() <= MAX_ENTITY_TAG_SIZE
        && bytes.first() == Some(&b'"')
        && bytes.last() == Some(&b'"')
        && bytes[1..bytes.len() - 1]
            .iter()
            .copied()
            .all(|byte| byte == 0x21 || (0x23..=0x7e).contains(&byte) || byte >= 0x80);
    valid.then_some(value).ok_or_else(|| {
        "DS5-CFG-007: Sunshine returned an invalid DualSense configuration ETag".to_string()
    })
}

pub(crate) fn require_entity_tag(entity_tag: Option<HeaderValue>) -> Result<HeaderValue, String> {
    entity_tag.ok_or_else(|| {
        "DS5-CFG-007: Sunshine does not expose conditional DualSense configuration updates"
            .to_string()
    })
}

pub(crate) fn core_ds5_http_error(error_code: Option<&str>, status: reqwest::StatusCode) -> String {
    match error_code {
        Some("ds5_config_invalid") => {
            "DS5-CFG-005: the independent DualSense configuration is invalid".to_string()
        }
        Some("ds5_precondition_failed") => {
            "DS5-CFG-006: DualSense configuration changed during this save; refresh and try again"
                .to_string()
        }
        Some("ds5_precondition_required") | Some("ds5_if_match_invalid") => {
            "DS5-CFG-007: Sunshine rejected the conditional DualSense configuration protocol"
                .to_string()
        }
        _ if status == reqwest::StatusCode::PRECONDITION_FAILED => {
            "DS5-CFG-006: DualSense configuration changed during this save; refresh and try again"
                .to_string()
        }
        _ if status.as_u16() == 428 => {
            "DS5-CFG-007: Sunshine rejected the conditional DualSense configuration protocol"
                .to_string()
        }
        _ => format!(
            "DS5-CFG-001: Sunshine rejected the DualSense configuration request (HTTP {})",
            status.as_u16()
        ),
    }
}

pub(crate) fn validate_core_ds5_response(
    result: CoreDualSenseResponse,
) -> Result<CoreDualSenseResponse, String> {
    if !result.status || !result.applied || result.revision == 0 {
        return Err(
            "DS5-CFG-001: Sunshine rejected the DualSense configuration request".to_string(),
        );
    }
    let values = result.settings;
    if clamp_tuning(
        values.ds5_legacy_haptics_strength,
        values.ds5_legacy_haptics_curve,
        values.ds5_legacy_haptics_noise_gate,
    ) != Some((
        values.ds5_legacy_haptics_strength,
        values.ds5_legacy_haptics_curve,
        values.ds5_legacy_haptics_noise_gate,
    )) {
        return Err("DS5-CFG-001: Sunshine returned invalid DualSense values".to_string());
    }
    if values.ds5_genshin_compatibility && (!values.ds5_enabled || !values.ds5_audio_haptics) {
        return Err("DS5-CFG-001: Sunshine returned invalid DualSense values".to_string());
    }
    Ok(result)
}

pub(crate) async fn read_core_ds5_response(
    mut response: reqwest::Response,
) -> Result<CoreDualSenseSnapshot, CoreDualSenseResponseError> {
    const MAX_RESPONSE_BYTES: usize = 64 * 1024;
    let status = response.status();
    let entity_tag = response.headers().get(reqwest::header::ETAG).cloned();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err("DS5-CFG-001: DualSense configuration response is too large"
            .to_string()
            .into());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(CoreDualSenseResponseError::Transfer)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("DS5-CFG-001: DualSense configuration response is too large"
                .to_string()
                .into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let error_code = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|body| body.get("error_code")?.as_str().map(str::to_owned));
        return Err(core_ds5_http_error(error_code.as_deref(), status).into());
    }
    let result: CoreDualSenseResponse = serde_json::from_slice(&bytes).map_err(|error| {
        CoreDualSenseResponseError::Message(format!(
            "DS5-CFG-001: invalid DualSense configuration response: {error}"
        ))
    })?;
    let result = validate_core_ds5_response(result)?;
    let entity_tag = entity_tag.map(validate_strong_entity_tag).transpose()?;
    Ok(CoreDualSenseSnapshot {
        response: result,
        entity_tag,
    })
}

pub(crate) async fn get_core_ds5_settings() -> Result<CoreDualSenseSnapshot, String> {
    let base_url = crate::sunshine::get_sunshine_url().await.map_err(|_| {
        warn!("Unable to resolve the Sunshine address for DualSense configuration");
        "DS5-CFG-001: unable to read DualSense configuration".to_string()
    })?;
    let endpoint = format!("{}/api/dualsense/config", base_url.trim_end_matches('/'));
    let client = crate::sunshine::create_https_client()?;
    for attempt in 1..=CORE_CONFIG_READ_ATTEMPTS {
        let response = match client.get(&endpoint).send().await {
            Ok(response) => read_core_ds5_response(response).await,
            Err(error) => Err(CoreDualSenseResponseError::Transfer(error)),
        };
        match response {
            Ok(snapshot) => return Ok(snapshot),
            Err(CoreDualSenseResponseError::Transfer(error)) => {
                let timed_out = error.is_timeout();
                warn!(
                    "Unable to query DualSense configuration (attempt {attempt}/{CORE_CONFIG_READ_ATTEMPTS}): {}",
                    error.without_url()
                );
                if attempt < CORE_CONFIG_READ_ATTEMPTS && !timed_out {
                    tokio::time::sleep(CORE_CONFIG_RETRY_DELAY * attempt as u32).await;
                } else {
                    break;
                }
            }
            Err(CoreDualSenseResponseError::Message(message)) => return Err(message),
        }
    }
    Err("DS5-CFG-001: unable to read DualSense configuration".to_string())
}

pub(crate) async fn save_core_ds5_settings(
    settings: CoreDualSenseSettings,
    entity_tag: HeaderValue,
) -> Result<CoreDualSenseSnapshot, String> {
    let base_url = crate::sunshine::get_sunshine_url().await.map_err(|_| {
        warn!("Unable to resolve the Sunshine address for DualSense configuration");
        "DS5-CFG-003: unable to save DualSense configuration".to_string()
    })?;
    let response = crate::sunshine::create_https_client()?
        .post(format!(
            "{}/api/dualsense/config",
            base_url.trim_end_matches('/')
        ))
        .header(reqwest::header::IF_MATCH, entity_tag)
        .json(&settings)
        .send()
        .await
        .map_err(|error| {
            warn!(
                "Unable to save DualSense configuration: {}",
                error.without_url()
            );
            "DS5-CFG-003: unable to save DualSense configuration".to_string()
        })?;
    read_core_ds5_response(response).await.map_err(|error| {
        let message = match error {
            CoreDualSenseResponseError::Transfer(error) => {
                warn!(
                    "Unable to read the saved DualSense configuration response: {}",
                    error.without_url()
                );
                "DS5-CFG-001: unable to read DualSense configuration".to_string()
            }
            CoreDualSenseResponseError::Message(message) => message,
        };
        message.replacen("DS5-CFG-001:", "DS5-CFG-003:", 1)
    })
}

pub(crate) fn clamp_tuning(strength: f64, curve: f64, noise_gate: f64) -> Option<(f64, f64, f64)> {
    (strength.is_finite() && curve.is_finite() && noise_gate.is_finite()).then_some((
        strength.clamp(0.1, 4.0),
        curve.clamp(0.3, 2.0),
        noise_gate.clamp(0.002, 0.060),
    ))
}

pub(crate) fn update_config_fields(
    settings: &mut CoreDualSenseSettings,
    enabled: bool,
    audio_haptics: bool,
    genshin_compatibility: bool,
) {
    settings.ds5_enabled = enabled;
    settings.ds5_audio_haptics = audio_haptics;
    settings.ds5_genshin_compatibility = genshin_compatibility;
}

pub(crate) fn update_tuning_fields(
    settings: &mut CoreDualSenseSettings,
    strength: f64,
    curve: f64,
    noise_gate: f64,
) {
    settings.ds5_legacy_haptics_strength = strength;
    settings.ds5_legacy_haptics_curve = curve;
    settings.ds5_legacy_haptics_noise_gate = noise_gate;
}

pub(crate) async fn resolve_core_config<F, Fut>(
    confirmed: Option<CoreDualSenseResponse>,
    fetch: F,
) -> (CoreDualSenseSettings, u64, String)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<CoreDualSenseSnapshot, String>>,
{
    if let Some(response) = confirmed {
        return (response.settings, response.revision, String::new());
    }

    match fetch().await {
        Ok(snapshot) => (
            snapshot.response.settings,
            snapshot.response.revision,
            String::new(),
        ),
        Err(error) => {
            warn!("DualSense status could not read the configuration: {error}");
            (CoreDualSenseSettings::default(), 0, error)
        }
    }
}
