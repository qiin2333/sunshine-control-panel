//! Shared, integrity-friendly downloader for GitHub Release assets.
//!
//! Callers remain responsible for cryptographic verification. This module only
//! provides consistent transport behavior: direct GitHub first, followed by the
//! configured acceleration mirrors, with bounded redirects, timeouts, size
//! checks, streaming writes, and retry cleanup.

use futures_util::StreamExt;
use log::{debug, info, warn};
use std::fmt;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufWriter};

pub(crate) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(20);

const GITHUB_RELEASE_HOST: &str = "github.com";
const DOWNLOAD_BUFFER_BYTES: usize = 1024 * 1024;
const DOWNLOAD_PROXY_PREFIXES: &[(&str, &str)] = &[
    ("ghfast.top", "https://ghfast.top/"),
    ("ghproxy.com", "https://ghproxy.com/"),
    ("mirror.ghproxy.com", "https://mirror.ghproxy.com/"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct DownloadSource {
    name: &'static str,
    url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DownloadAttemptPhase {
    Connecting,
    Downloading,
    Retrying,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DownloadProgress {
    pub(crate) phase: DownloadAttemptPhase,
    pub(crate) source: &'static str,
    pub(crate) downloaded: u64,
    pub(crate) total: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct DownloadOutcome {
    pub(crate) source: &'static str,
    pub(crate) downloaded: u64,
    pub(crate) total: u64,
}

pub(crate) struct DownloadRequest<'a> {
    pub(crate) url: &'a str,
    pub(crate) destination: &'a Path,
    pub(crate) expected_size: Option<u64>,
    pub(crate) max_size: Option<u64>,
    pub(crate) user_agent: &'a str,
    pub(crate) connect_timeout: Duration,
    pub(crate) response_timeout: Duration,
    pub(crate) idle_timeout: Duration,
    pub(crate) overall_timeout: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DownloadErrorKind {
    Setup,
    SourcesExhausted,
}

#[derive(Debug)]
pub(crate) struct DownloadError {
    kind: DownloadErrorKind,
    detail: String,
}

impl DownloadError {
    pub(crate) fn is_setup(&self) -> bool {
        self.kind == DownloadErrorKind::Setup
    }
}

impl fmt::Display for DownloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

fn validate_github_release_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("invalid download URL: {error}"))?;
    if parsed.scheme() != "https" {
        return Err("download URL must use HTTPS".to_string());
    }
    if parsed.host_str() != Some(GITHUB_RELEASE_HOST) {
        return Err(format!("download URL must use {GITHUB_RELEASE_HOST}"));
    }
    Ok(())
}

fn build_download_sources(original_url: &str) -> Result<Vec<DownloadSource>, String> {
    validate_github_release_url(original_url)?;
    let mut sources = vec![DownloadSource {
        name: "GitHub",
        url: original_url.to_string(),
    }];

    for (name, prefix) in DOWNLOAD_PROXY_PREFIXES {
        sources.push(DownloadSource {
            name,
            url: format!("{prefix}{original_url}"),
        });
    }
    Ok(sources)
}

fn create_download_client(request: &DownloadRequest<'_>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(request.connect_timeout)
        .read_timeout(request.idle_timeout)
        .redirect(reqwest::redirect::Policy::limited(10))
        .no_gzip()
        .no_deflate();
    if let Some(timeout) = request.overall_timeout {
        builder = builder.timeout(timeout);
    }
    builder
        .build()
        .map_err(|error| format!("could not create download client: {error}"))
}

fn validate_download_response(
    response: &reqwest::Response,
    expected_size: Option<u64>,
    max_size: Option<u64>,
) -> Result<(), String> {
    if !response.status().is_success() {
        return Err(format!("HTTP status {}", response.status().as_u16()));
    }

    if let Some(content_type) = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        let content_type = content_type.to_ascii_lowercase();
        if content_type.starts_with("text/")
            || content_type.contains("application/json")
            || content_type.contains("application/xml")
        {
            return Err(format!("response is not a release asset: {content_type}"));
        }
    }

    if response.content_length() == Some(0) {
        return Err("response is empty".to_string());
    }
    if let (Some(expected), Some(actual)) = (expected_size, response.content_length())
        && expected != actual
    {
        return Err(format!(
            "content length mismatch: expected {expected} bytes, got {actual} bytes"
        ));
    }
    if let (Some(limit), Some(actual)) = (max_size, response.content_length())
        && actual > limit
    {
        return Err(format!(
            "content length exceeds the {limit}-byte limit: {actual} bytes"
        ));
    }
    Ok(())
}

async fn request_source(
    client: &reqwest::Client,
    source: &DownloadSource,
    request: &DownloadRequest<'_>,
) -> Result<reqwest::Response, String> {
    let pending = client
        .get(&source.url)
        .header(reqwest::header::USER_AGENT, request.user_agent)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send();
    let response = match tokio::time::timeout(request.response_timeout, pending).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return Err(format!("request failed: {error}")),
        Err(_) => {
            return Err(format!(
                "server did not respond within {} seconds",
                request.response_timeout.as_secs()
            ));
        }
    };
    validate_download_response(&response, request.expected_size, request.max_size)?;
    debug!(
        "GitHub download source ready: {} ({})",
        source.name,
        response.url()
    );
    Ok(response)
}

async fn download_source_to_file<F>(
    client: &reqwest::Client,
    source: &DownloadSource,
    request: &DownloadRequest<'_>,
    on_progress: &mut F,
) -> Result<DownloadOutcome, String>
where
    F: FnMut(DownloadProgress),
{
    let response = request_source(client, source, request).await?;
    let response_size = response.content_length();
    let total = request.expected_size.or(response_size).unwrap_or(0);
    on_progress(DownloadProgress {
        phase: DownloadAttemptPhase::Downloading,
        source: source.name,
        downloaded: 0,
        total,
    });

    let file = tokio::fs::File::create(request.destination)
        .await
        .map_err(|error| format!("could not create partial download: {error}"))?;
    let mut file = BufWriter::with_capacity(DOWNLOAD_BUFFER_BYTES, file);
    let mut stream = response.bytes_stream();
    let mut downloaded = 0u64;
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|error| format!("download stream failed: {error}"))?;
        let next_size = downloaded
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "download size overflow".to_string())?;
        if let Some(limit) = request.max_size
            && next_size > limit
        {
            return Err(format!("download exceeds the {limit}-byte limit"));
        }
        if let Some(expected) = request.expected_size
            && next_size > expected
        {
            return Err(format!("download exceeds the expected {expected} bytes"));
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("could not write partial download: {error}"))?;
        downloaded = next_size;
        on_progress(DownloadProgress {
            phase: DownloadAttemptPhase::Downloading,
            source: source.name,
            downloaded,
            total,
        });
    }
    file.flush()
        .await
        .map_err(|error| format!("could not flush partial download: {error}"))?;
    drop(file);

    if downloaded == 0 {
        return Err("download is empty".to_string());
    }
    if let Some(required) = request.expected_size.or(response_size)
        && downloaded != required
    {
        return Err(format!(
            "download size mismatch: expected {required} bytes, got {downloaded} bytes"
        ));
    }
    Ok(DownloadOutcome {
        source: source.name,
        downloaded,
        total,
    })
}

pub(crate) async fn download_to_file_with_fallbacks<F>(
    request: DownloadRequest<'_>,
    on_progress: F,
) -> Result<DownloadOutcome, DownloadError>
where
    F: FnMut(DownloadProgress),
{
    if request.expected_size == Some(0) || request.max_size == Some(0) {
        return Err(DownloadError {
            kind: DownloadErrorKind::Setup,
            detail: "download size constraints must be greater than zero".to_string(),
        });
    }
    if let (Some(expected), Some(limit)) = (request.expected_size, request.max_size)
        && expected > limit
    {
        return Err(DownloadError {
            kind: DownloadErrorKind::Setup,
            detail: format!("expected size {expected} exceeds the {limit}-byte limit"),
        });
    }

    let sources = build_download_sources(request.url).map_err(|detail| DownloadError {
        kind: DownloadErrorKind::Setup,
        detail,
    })?;
    download_from_sources(request, &sources, on_progress).await
}

async fn download_from_sources<F>(
    request: DownloadRequest<'_>,
    sources: &[DownloadSource],
    mut on_progress: F,
) -> Result<DownloadOutcome, DownloadError>
where
    F: FnMut(DownloadProgress),
{
    let client = create_download_client(&request).map_err(|detail| DownloadError {
        kind: DownloadErrorKind::Setup,
        detail,
    })?;
    let mut errors = Vec::new();

    for (index, source) in sources.iter().enumerate() {
        on_progress(DownloadProgress {
            phase: if index == 0 {
                DownloadAttemptPhase::Connecting
            } else {
                DownloadAttemptPhase::Retrying
            },
            source: source.name,
            downloaded: 0,
            total: request.expected_size.unwrap_or(0),
        });
        info!("Trying GitHub download source: {}", source.name);
        match download_source_to_file(&client, source, &request, &mut on_progress).await {
            Ok(outcome) => return Ok(outcome),
            Err(error) => {
                warn!("GitHub download source {} failed: {}", source.name, error);
                errors.push(format!("{}: {error}", source.name));
                let _ = tokio::fs::remove_file(request.destination).await;
            }
        }
    }

    Err(DownloadError {
        kind: DownloadErrorKind::SourcesExhausted,
        detail: format!("all download sources failed: {}", errors.join("; ")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::StatusCode, response::Html, routing::get};

    #[test]
    fn sources_are_direct_first_then_follow_mirror_order() {
        let original = "https://github.com/example/project/releases/download/v1/asset.zip";
        let sources = build_download_sources(original).unwrap();
        assert_eq!(sources.len(), 1 + DOWNLOAD_PROXY_PREFIXES.len());
        assert_eq!(sources[0].name, "GitHub");
        assert_eq!(sources[0].url, original);
        for (source, (name, prefix)) in sources.iter().skip(1).zip(DOWNLOAD_PROXY_PREFIXES) {
            assert_eq!(source.name, *name);
            assert_eq!(source.url, format!("{prefix}{original}"));
        }
    }

    #[test]
    fn only_https_github_urls_can_be_mirrored() {
        for invalid in [
            "http://github.com/example/project/releases/download/v1/a.zip",
            "https://github.com.example.invalid/a.zip",
            "https://example.invalid/a.zip",
            "not a url",
        ] {
            assert!(
                build_download_sources(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn html_response_is_rejected_before_streaming() {
        let app = Router::new().route("/html", get(|| async { Html("proxy error") }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let request = DownloadRequest {
            url: "https://github.com/example/project/releases/download/v1/a.zip",
            destination: Path::new("unused"),
            expected_size: None,
            max_size: None,
            user_agent: "test",
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            overall_timeout: None,
        };
        let source = DownloadSource {
            name: "html",
            url: format!("http://{address}/html"),
        };
        let client = create_download_client(&request).unwrap();
        let error = request_source(&client, &source, &request)
            .await
            .unwrap_err();
        assert!(error.contains("response is not a release asset"));
        server.abort();
    }

    #[tokio::test]
    async fn failed_source_is_cleaned_up_before_fallback_succeeds() {
        let app = Router::new()
            .route("/fail", get(|| async { StatusCode::BAD_GATEWAY }))
            .route(
                "/asset",
                get(|| async { ([("content-type", "application/octet-stream")], "asset") }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let destination = std::env::temp_dir().join(format!(
            "sunshine-github-download-test-{}-{}.partial",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let request = DownloadRequest {
            url: "https://github.com/example/project/releases/download/v1/a.zip",
            destination: &destination,
            expected_size: Some(5),
            max_size: Some(5),
            user_agent: "test",
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            overall_timeout: None,
        };
        let sources = [
            DownloadSource {
                name: "broken",
                url: format!("http://{address}/fail"),
            },
            DownloadSource {
                name: "fallback",
                url: format!("http://{address}/asset"),
            },
        ];
        let mut phases = Vec::new();
        let outcome = download_from_sources(request, &sources, |progress| {
            phases.push((progress.phase, progress.source));
        })
        .await
        .unwrap();

        assert_eq!(outcome.source, "fallback");
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"asset");
        assert!(phases.contains(&(DownloadAttemptPhase::Retrying, "fallback")));
        let _ = tokio::fs::remove_file(&destination).await;
        server.abort();
    }
}
