use super::*;
use futures_util::StreamExt as _;
use std::sync::atomic::{AtomicBool, Ordering};

const RECONNECT_DELAY: Duration = Duration::from_secs(3);
const LEGACY_POLL_INTERVAL: Duration = Duration::from_secs(3);
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const PROVIDER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
static PROVIDER_LEASE_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);

pub(super) fn start_tray_state_monitoring<R: Runtime + 'static>(app: &AppHandle<R>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_state_key: Option<(String, u64)> = None;
        let mut contract_error_visible = false;

        loop {
            match sunshine::get_tray_state().await {
                Ok(state) => {
                    contract_error_visible = false;
                    let supports_events = state
                        .capabilities
                        .iter()
                        .any(|capability| capability == "events-v1");
                    if state
                        .capabilities
                        .iter()
                        .any(|capability| capability == "provider-lease-v1")
                    {
                        start_provider_lease_monitor();
                    }
                    apply_if_new(&app_handle, &mut last_state_key, state);

                    if supports_events {
                        if let Err(e) = consume_event_stream(&app_handle, &mut last_state_key).await
                        {
                            debug!("Tray event stream ended: {}", e);
                        }
                        tokio::time::sleep(RECONNECT_DELAY).await;
                    } else {
                        tokio::time::sleep(LEGACY_POLL_INTERVAL).await;
                    }
                }
                Err(e) => {
                    mark_core_disconnected(&app_handle);
                    let is_contract_error = e.contains("tray protocol")
                        || e.contains("Tray state is missing")
                        || e.contains("tray owner");
                    if is_contract_error && !contract_error_visible {
                        warn!("Tray contract is unavailable: {}", e);
                        contract_error_visible = true;
                    } else {
                        debug!("Tray state unavailable: {}", e);
                    }
                    tokio::time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    });
}

fn start_provider_lease_monitor() {
    if PROVIDER_LEASE_MONITOR_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        loop {
            match sunshine::register_tray_provider().await {
                Ok(lease) if lease.status && !lease.lease_id.is_empty() => {
                    info!("Tray provider lease acquired");
                    loop {
                        tokio::time::sleep(PROVIDER_HEARTBEAT_INTERVAL).await;
                        if let Err(e) = sunshine::heartbeat_tray_provider(&lease.lease_id).await {
                            debug!("Tray provider lease ended: {}", e);
                            break;
                        }
                    }
                }
                Ok(lease) => debug!("Tray provider lease was not granted: {}", lease.error),
                Err(e) => debug!("Tray provider registration unavailable: {}", e),
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    });
}

async fn consume_event_stream<R: Runtime + 'static>(
    app: &AppHandle<R>,
    last_state_key: &mut Option<(String, u64)>,
) -> Result<(), String> {
    let sunshine_url = sunshine::get_local_sunshine_url().await?;
    let endpoint = format!("{}/api/tray/events", sunshine_url.trim_end_matches('/'));
    let response = sunshine::create_sse_https_client()?
        .get(endpoint)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| format!("Connect tray event stream failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Tray event stream returned status {}",
            response.status()
        ));
    }

    info!("Tray event stream connected");
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut health_check = tokio::time::interval(HEALTH_CHECK_INTERVAL);
    health_check.tick().await;

    loop {
        tokio::select! {
            chunk = stream.next() => match chunk {
                Some(Ok(bytes)) => {
                    buffer.extend_from_slice(&bytes);
                    while let Some(frame) = take_sse_frame(&mut buffer) {
                        if let Some(state) = parse_tray_state_event(&frame)? {
                            apply_if_new(app, last_state_key, state);
                        }
                    }
                }
                Some(Err(e)) => return Err(format!("Read tray event stream failed: {}", e)),
                None => return Err("Tray event stream closed".to_string()),
            },
            _ = health_check.tick() => {
                let state = sunshine::get_tray_state().await?;
                apply_if_new(app, last_state_key, state);
            }
        }
    }
}

fn apply_if_new<R: Runtime + 'static>(
    app: &AppHandle<R>,
    last_state_key: &mut Option<(String, u64)>,
    state: sunshine::TrayState,
) {
    let is_new = match last_state_key.as_ref() {
        Some((instance_id, revision)) if instance_id == &state.instance_id => {
            state.revision > *revision
        }
        _ => true,
    };
    if is_new {
        *last_state_key = Some((state.instance_id.clone(), state.revision));
        apply_tray_state_on_main_thread(app, state);
    }
}

fn mark_core_disconnected<R: Runtime + 'static>(app: &AppHandle<R>) {
    let disconnect_handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        apply_core_disconnected(&disconnect_handle);
    }) {
        debug!("Failed to schedule disconnected tray state: {}", e);
    }
}

fn take_sse_frame(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let (end, delimiter_len) = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|end| (end, 2))
        .or_else(|| {
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|end| (end, 4))
        })?;
    let frame = buffer.drain(..end).collect();
    buffer.drain(..delimiter_len);
    Some(frame)
}

fn parse_tray_state_event(frame: &[u8]) -> Result<Option<sunshine::TrayState>, String> {
    let frame =
        std::str::from_utf8(frame).map_err(|e| format!("Tray event is not valid UTF-8: {}", e))?;
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(None);
    }
    sunshine::parse_tray_state_json(&data).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_json(revision: u64) -> String {
        serde_json::json!({
            "protocol_version": 1,
            "instance_id": "core-instance",
            "owner": "gui",
            "capabilities": ["state-v1", "events-v1"],
            "revision": revision
        })
        .to_string()
    }

    #[test]
    fn parses_complete_and_fragmented_sse_frames() {
        let mut buffer = format!("event: tray-state\ndata: {}\n", state_json(7)).into_bytes();
        assert!(take_sse_frame(&mut buffer).is_none());
        buffer.extend_from_slice(b"\n");

        let frame = take_sse_frame(&mut buffer).expect("complete frame");
        let state = parse_tray_state_event(&frame)
            .expect("valid event")
            .expect("state payload");
        assert_eq!(state.revision, 7);
        assert!(buffer.is_empty());
    }

    #[test]
    fn ignores_sse_comment_frames() {
        assert!(
            parse_tray_state_event(b": keepalive")
                .expect("valid comment")
                .is_none()
        );
    }
}
