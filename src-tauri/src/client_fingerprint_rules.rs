use futures_util::StreamExt;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use reqwest::header::{ETAG, IF_NONE_MATCH};
use std::time::Duration;

use crate::sunshine;

const MAX_ENVELOPE_BYTES: usize = 512 * 1024;
const DEFAULT_REFRESH_HOURS: u64 = 24;
const MIN_REFRESH_HOURS: u64 = 1;
const MAX_REFRESH_HOURS: u64 = 168;
const CONFIG_RECHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);

static RULE_CLIENT: Lazy<Result<reqwest::Client, String>> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Sunshine-GUI/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Create client fingerprint rule client failed: {}", e))
});

enum FetchResult {
    NotModified,
    Candidate {
        envelope: String,
        etag: Option<String>,
    },
}

fn remote_rules_enabled(value: Option<&str>) -> bool {
    value.map_or(true, |value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "enabled" | "true" | "yes" | "on" | "1"
        )
    })
}

fn refresh_interval(value: Option<&str>) -> Duration {
    let hours = value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_REFRESH_HOURS)
        .clamp(MIN_REFRESH_HOURS, MAX_REFRESH_HOURS);
    Duration::from_secs(hours * 60 * 60)
}

async fn fetch_candidate(url: &str, etag: Option<&str>) -> Result<FetchResult, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| format!("Invalid client fingerprint rule URL: {}", e))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Client fingerprint rule URL must be an absolute HTTPS URL".to_string());
    }

    let client = RULE_CLIENT.as_ref().map_err(Clone::clone)?;
    let mut request = client.get(parsed);
    if let Some(etag) = etag {
        request = request.header(IF_NONE_MATCH, etag);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("Fetch client fingerprint rules failed: {}", e))?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(FetchResult::NotModified);
    }
    if !response.status().is_success() {
        return Err(format!(
            "Fetch client fingerprint rules failed with status {}",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length == 0 || length > MAX_ENVELOPE_BYTES as u64)
    {
        return Err("Client fingerprint rule envelope exceeds the size limit".to_string());
    }

    let response_etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Read client fingerprint rules failed: {}", e))?;
        if body.len().saturating_add(chunk.len()) > MAX_ENVELOPE_BYTES {
            return Err("Client fingerprint rule envelope exceeds the size limit".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    if body.is_empty() {
        return Err("Client fingerprint rule envelope is empty".to_string());
    }
    let envelope = String::from_utf8(body)
        .map_err(|_| "Client fingerprint rule envelope is not UTF-8".to_string())?;
    Ok(FetchResult::Candidate {
        envelope,
        etag: response_etag,
    })
}

async fn updater_loop() {
    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut accepted_etag: Option<String> = None;
    loop {
        let config = match sunshine::parse_sunshine_config_sync() {
            Ok(config) => config,
            Err(error) => {
                warn!(
                    "Read client fingerprint rule configuration failed: {}",
                    error
                );
                tokio::time::sleep(CONFIG_RECHECK_INTERVAL).await;
                continue;
            }
        };
        let interval = refresh_interval(config.client_fingerprint_rules_refresh_hours.as_deref());
        if !remote_rules_enabled(config.client_fingerprint_remote_rules.as_deref()) {
            debug!("Client fingerprint remote rules are disabled");
            tokio::time::sleep(CONFIG_RECHECK_INTERVAL).await;
            continue;
        }
        let Some(url) = config
            .client_fingerprint_rules_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
        else {
            debug!("Client fingerprint rule URL is not configured");
            tokio::time::sleep(CONFIG_RECHECK_INTERVAL).await;
            continue;
        };

        match fetch_candidate(url, accepted_etag.as_deref()).await {
            Ok(FetchResult::NotModified) => {
                debug!("Client fingerprint rule feed has not changed");
            }
            Ok(FetchResult::Candidate { envelope, etag }) => {
                match sunshine::post_client_fingerprint_rules(&envelope).await {
                    Ok(result) => {
                        accepted_etag = etag;
                        if result.installed {
                            info!(
                                "Installed client fingerprint rule revision {}",
                                result.revision
                            );
                        } else if result.unchanged {
                            debug!(
                                "Client fingerprint rule revision {} is already active",
                                result.revision
                            );
                        }
                    }
                    Err(error) => warn!("{}", error),
                }
            }
            Err(error) => warn!("{}", error),
        }

        tokio::time::sleep(interval).await;
    }
}

pub fn start() {
    tauri::async_runtime::spawn(updater_loop());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remote_rule_switch() {
        assert!(remote_rules_enabled(None));
        assert!(remote_rules_enabled(Some("enabled")));
        assert!(remote_rules_enabled(Some("TRUE")));
        assert!(!remote_rules_enabled(Some("disabled")));
    }

    #[test]
    fn bounds_refresh_interval() {
        assert_eq!(refresh_interval(Some("0")), Duration::from_secs(60 * 60));
        assert_eq!(
            refresh_interval(Some("999")),
            Duration::from_secs(168 * 60 * 60)
        );
        assert_eq!(
            refresh_interval(Some("bad")),
            Duration::from_secs(DEFAULT_REFRESH_HOURS * 60 * 60)
        );
    }
}
