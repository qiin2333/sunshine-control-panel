//! A caller-owned lease for the host's shared frame-rate collection service.

pub(crate) const MAX_RENEW_INTERVAL_MS: u64 = 10_000;

pub(crate) struct FrameRateSubscription {
    client: reqwest::Client,
    endpoint: String,
    consumer_id: String,
}

impl FrameRateSubscription {
    /// Each consumer gets its own identity; renewals keep it until release.
    pub(crate) fn new(client: reqwest::Client, host_url: &str) -> Self {
        Self {
            client,
            endpoint: format!("{}/api/perf/frame-rates", host_url.trim_end_matches('/')),
            consumer_id: format!("diagnostic:{}", uuid::Uuid::new_v4()),
        }
    }

    /// Renew before reading the snapshot. Thirty seconds also covers brief reconnects.
    pub(crate) async fn renew(&self) -> Result<(), String> {
        self.set_enabled(true).await
    }

    /// Release only this consumer; other diagnostics retain their subscriptions.
    pub(crate) async fn release(&self) -> Result<(), String> {
        self.set_enabled(false).await
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        let response = self
            .client
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "consumer_id": self.consumer_id,
                "enabled": enabled,
                "lease_ms": 30_000,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err("host rejected frame-rate subscription".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FrameRateSubscription;
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use std::{collections::HashSet, sync::Arc};
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn independent_consumers_renew_and_release_only_their_own_lease() {
        let subscriptions = Arc::new(Mutex::new(HashSet::<String>::new()));
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let handler_subscriptions = subscriptions.clone();
        let handler_requests = requests.clone();
        let app = Router::new().route(
            "/api/perf/frame-rates",
            post(move |Json(body): Json<Value>| {
                let subscriptions = handler_subscriptions.clone();
                let requests = handler_requests.clone();
                async move {
                    requests.lock().await.push(body.clone());
                    let mut subscriptions = subscriptions.lock().await;
                    let id = body["consumer_id"].as_str().unwrap().to_owned();
                    if body["enabled"] == true {
                        subscriptions.insert(id);
                    } else {
                        subscriptions.remove(&id);
                    }
                    Json(json!({"success": true, "frame_rate_collection": {
                        "enabled": !subscriptions.is_empty(), "subscribers": subscriptions.len()
                    }}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let host_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let first = FrameRateSubscription::new(client.clone(), &host_url);
        let second = FrameRateSubscription::new(client, &host_url);
        first.renew().await.unwrap();
        first.renew().await.unwrap();
        assert_eq!(subscriptions.lock().await.len(), 1);
        second.renew().await.unwrap();
        assert_eq!(subscriptions.lock().await.len(), 2);
        first.release().await.unwrap();
        assert_eq!(subscriptions.lock().await.len(), 1);
        second.release().await.unwrap();
        assert!(subscriptions.lock().await.is_empty());
        let requests = requests.lock().await;
        assert_eq!(requests[0]["consumer_id"], requests[1]["consumer_id"]);
        assert_ne!(requests[0]["consumer_id"], requests[2]["consumer_id"]);
        assert_eq!(requests[0]["lease_ms"], 30_000);
        server.abort();
    }

    #[tokio::test]
    async fn rejects_http_errors_and_unsuccessful_host_responses() {
        for (status, body) in [
            (axum::http::StatusCode::NOT_FOUND, json!({})),
            (axum::http::StatusCode::OK, json!({"success": false})),
        ] {
            let app = Router::new().route(
                "/api/perf/frame-rates",
                post(move || {
                    let body = body.clone();
                    async move { (status, Json(body)) }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let host_url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            let subscription = FrameRateSubscription::new(reqwest::Client::new(), &host_url);
            assert!(subscription.renew().await.is_err());
            assert!(subscription.release().await.is_err());
            server.abort();
        }
    }
}
