//! Foundation Sunshine release metadata and channel resolution.
//!
//! The Panel keeps the release source policy here so the UI, tray entry point,
//! startup check, and native update request all resolve the same candidates.

use futures_util::StreamExt;
use regex::Regex;
use reqwest::Url;
use serde::Deserialize;
use std::cmp::Ordering;
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};

use crate::github_download::DownloadCandidate;

const METADATA_COM_URL: &str =
    "https://www.alkaidlab.com/release-metadata/foundation-sunshine.json";
const METADATA_CN_URL: &str = "https://www.alkaidlab.cn/release-metadata/foundation-sunshine.json";
const CNB_RELEASES_URL: &str = "https://cnb.cool/AlkaidLab/foundation-sunshine-release/-/releases";
const GITHUB_LATEST_URL: &str =
    "https://api.github.com/repos/AlkaidLab/foundation-sunshine/releases/latest";
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/AlkaidLab/foundation-sunshine/releases?per_page=30";
const GITHUB_OWNER: &str = "AlkaidLab";
const GITHUB_REPOSITORY: &str = "foundation-sunshine";
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_RELEASE_NOTES_BYTES: usize = 128 * 1024;
const METADATA_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const METADATA_READ_TIMEOUT: Duration = Duration::from_secs(5);
const METADATA_SOURCE_TIMEOUT: Duration = Duration::from_secs(8);
const METADATA_OVERALL_TIMEOUT: Duration = Duration::from_secs(10);
const CN_RETRY_COUNT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateChannel {
    StableOnly,
    StableAndPrerelease,
    PrereleaseOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    pub(crate) asset_type: String,
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    pub(crate) url: String,
    pub(crate) fallback_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReleaseCandidate {
    pub(crate) version: String,
    pub(crate) prerelease: bool,
    pub(crate) github_release_id: Option<u64>,
    pub(crate) published_at: Option<String>,
    pub(crate) release_notes: String,
    pub(crate) release_page: String,
    pub(crate) assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReleaseCatalog {
    pub(crate) latest: Option<ReleaseCandidate>,
    pub(crate) pre_latest: Option<ReleaseCandidate>,
}

#[derive(Debug, Deserialize)]
struct MetadataDocument {
    schema: u32,
    kind: String,
    product: String,
    channels: MetadataChannels,
}

#[derive(Debug, Deserialize)]
struct MetadataChannels {
    latest: Option<MetadataRelease>,
    #[serde(rename = "pre-latest")]
    pre_latest: Option<MetadataRelease>,
}

#[derive(Debug, Deserialize)]
struct MetadataRelease {
    version: String,
    prerelease: bool,
    #[serde(rename = "githubReleaseId")]
    github_release_id: Option<u64>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    #[serde(rename = "releaseNotes", default)]
    release_notes: Option<String>,
    #[serde(default)]
    assets: Vec<MetadataAsset>,
}

#[derive(Debug, Deserialize)]
struct MetadataAsset {
    #[serde(rename = "type")]
    asset_type: String,
    name: String,
    size: u64,
    sha256: String,
    url: String,
    #[serde(rename = "fallbackUrl")]
    fallback_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    id: u64,
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CnbChecksumEntry {
    #[serde(alias = "Hash", alias = "hash")]
    hash: String,
    #[serde(alias = "File", alias = "file")]
    file: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestFailureKind {
    Retryable,
    Terminal,
}

#[derive(Debug)]
struct RequestFailure {
    kind: RequestFailureKind,
    detail: String,
}

impl RequestFailure {
    fn retryable(detail: impl Into<String>) -> Self {
        Self {
            kind: RequestFailureKind::Retryable,
            detail: detail.into(),
        }
    }

    fn terminal(detail: impl Into<String>) -> Self {
        Self {
            kind: RequestFailureKind::Terminal,
            detail: detail.into(),
        }
    }
}

fn create_metadata_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(METADATA_CONNECT_TIMEOUT)
        .read_timeout(METADATA_READ_TIMEOUT)
        // Metadata and GitHub API responses are trust anchors. Do not follow
        // a server-controlled redirect to another host; a redirected source
        // is treated as failed and the normal source fallback takes over.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("创建更新检查客户端失败: {error}"))
}

fn create_cnb_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(METADATA_CONNECT_TIMEOUT)
        .read_timeout(METADATA_READ_TIMEOUT)
        // CNB release assets may redirect to its object storage. The final
        // download is still checked against the metadata SHA-256.
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| format!("创建 CNB 更新检查客户端失败: {error}"))
}

async fn read_bounded_response(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, RequestFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(RequestFailure::terminal(format!(
            "响应超过 {limit} 字节限制"
        )));
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            RequestFailure::retryable(format!("读取更新响应失败: {}", error.without_url()))
        })?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(RequestFailure::terminal(format!(
                "响应超过 {limit} 字节限制"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn request_bytes(
    client: &reqwest::Client,
    url: &str,
    deadline: Instant,
    accept: &str,
    limit: usize,
) -> Result<Vec<u8>, RequestFailure> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(RequestFailure::retryable("更新检查整体超时"));
    }

    let request = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Sunshine-Control-Panel updater",
        )
        .header(reqwest::header::ACCEPT, accept)
        .timeout(remaining)
        .send();
    let response = match timeout(remaining, request).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            return Err(RequestFailure::retryable(format!(
                "请求失败: {}",
                error.without_url()
            )));
        }
        Err(_) => return Err(RequestFailure::retryable("请求超时")),
    };

    let status = response.status();
    if !status.is_success() {
        let failure = if status.is_server_error() {
            RequestFailure::retryable(format!("HTTP 状态码 {}", status.as_u16()))
        } else {
            RequestFailure::terminal(format!("HTTP 状态码 {}", status.as_u16()))
        };
        return Err(failure);
    }

    let remaining = deadline.saturating_duration_since(Instant::now());
    timeout(remaining, read_bounded_response(response, limit))
        .await
        .map_err(|_| RequestFailure::retryable("读取响应超时"))?
}

async fn fetch_metadata_endpoint(
    url: &'static str,
    retry_cn: bool,
    overall_deadline: Instant,
) -> Result<ReleaseCatalog, String> {
    let client = create_metadata_client()?;
    let deadline = std::cmp::min(overall_deadline, Instant::now() + METADATA_SOURCE_TIMEOUT);
    let max_attempts = if retry_cn { CN_RETRY_COUNT + 1 } else { 1 };
    let mut last_error = String::from("更新 metadata 请求失败");

    for attempt in 0..max_attempts {
        match request_bytes(
            &client,
            url,
            deadline,
            "application/json",
            MAX_METADATA_BYTES,
        )
        .await
        {
            Ok(body) => return parse_metadata(&body),
            Err(error) => {
                last_error = error.detail;
                if error.kind != RequestFailureKind::Retryable || attempt + 1 >= max_attempts {
                    break;
                }
                let backoff = Duration::from_millis(250 * (attempt as u64 + 1));
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                sleep(backoff.min(remaining)).await;
            }
        }
    }

    Err(last_error)
}

fn normalize_notes(notes: Option<String>) -> String {
    let mut notes = notes.unwrap_or_default().replace("\r\n", "\n");
    notes = notes.replace('\r', "\n");
    let notes = notes.trim().to_string();
    if notes.len() <= MAX_RELEASE_NOTES_BYTES {
        return notes;
    }

    let mut end = MAX_RELEASE_NOTES_BYTES;
    while !notes.is_char_boundary(end) {
        end -= 1;
    }
    notes[..end].to_string()
}

fn normalize_sha256(value: &str) -> Option<String> {
    let value = value.strip_prefix("sha256:").unwrap_or(value).trim();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn known_asset_type(asset_type: &str) -> bool {
    matches!(
        asset_type,
        "windows-x64-installer"
            | "windows-x64-portable"
            | "dualsense-sidecar-zip"
            | "dualsense-sidecar-manifest"
            | "checksum-json"
            | "checksum-text"
    )
}

fn classify_asset_name(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    let normalized = lower
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();

    if lower.starts_with("sunshine")
        && (lower.ends_with(".exe") || lower.ends_with(".msi"))
        && normalized.contains("windowsinstaller")
    {
        return Some("windows-x64-installer");
    }
    if lower.starts_with("sunshine") && lower.contains("portable") && lower.ends_with(".zip") {
        return Some("windows-x64-portable");
    }
    if lower.contains("ds5sidecar") && lower.ends_with(".zip") {
        return Some("dualsense-sidecar-zip");
    }
    if lower.contains("ds5sidecar") && lower.ends_with(".manifest.json") {
        return Some("dualsense-sidecar-manifest");
    }
    if lower == "checksums.json" {
        return Some("checksum-json");
    }
    if lower == "sha256sums.txt" {
        return Some("checksum-text");
    }
    None
}

fn encoded_release_page(tag: &str) -> String {
    let mut url = Url::parse("https://github.com/AlkaidLab/foundation-sunshine/releases/tag")
        .expect("built-in GitHub release URL must be valid");
    url.path_segments_mut()
        .expect("GitHub release URL must be a base URL")
        .push(tag);
    url.to_string()
}

fn encoded_download_url(tag: &str, name: &str) -> String {
    let mut url = Url::parse("https://github.com/AlkaidLab/foundation-sunshine/releases/download")
        .expect("built-in GitHub download URL must be valid");
    url.path_segments_mut()
        .expect("GitHub download URL must be a base URL")
        .push(tag)
        .push(name);
    url.to_string()
}

fn encoded_cnb_download_url(tag: &str, name: &str) -> String {
    let mut url =
        Url::parse("https://cnb.cool/AlkaidLab/foundation-sunshine-release/-/releases/download")
            .expect("built-in CNB download URL must be valid");
    url.path_segments_mut()
        .expect("CNB download URL must be a base URL")
        .push(tag)
        .push(name);
    url.to_string()
}

pub(crate) fn decode_segment(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("URL 编码不完整".to_string());
            }
            let high = (bytes[index + 1] as char)
                .to_digit(16)
                .ok_or_else(|| "URL 编码无效".to_string())?;
            let low = (bytes[index + 2] as char)
                .to_digit(16)
                .ok_or_else(|| "URL 编码无效".to_string())?;
            output.push((high * 16 + low) as u8);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| "URL 编码不是合法 UTF-8".to_string())
}

fn github_release_download_tag(url: &str, name: &str) -> Result<String, String> {
    let parsed = Url::parse(url).map_err(|error| format!("下载地址无效: {error}"))?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("组件下载地址必须是无凭据的 GitHub HTTPS 地址".to_string());
    }
    let segments = parsed
        .path_segments()
        .ok_or_else(|| "组件下载地址缺少路径".to_string())?
        .collect::<Vec<_>>();
    if segments.len() != 6
        || segments[0] != GITHUB_OWNER
        || segments[1] != GITHUB_REPOSITORY
        || segments[2] != "releases"
        || segments[3] != "download"
        || decode_segment(segments[5])? != name
    {
        return Err("组件下载地址不是 Foundation Sunshine Release 资产".to_string());
    }
    decode_segment(segments[4])
}

fn validate_release_download_url(url: &str, tag: &str, name: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|error| format!("下载地址无效: {error}"))?;
    if parsed.scheme() != "https"
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("下载地址必须是无凭据的 HTTPS 地址".to_string());
    }

    let segments = parsed
        .path_segments()
        .ok_or_else(|| "下载地址缺少路径".to_string())?
        .collect::<Vec<_>>();
    let host = parsed.host_str().unwrap_or_default();
    let expected_prefix = if host == "github.com" {
        [GITHUB_OWNER, GITHUB_REPOSITORY, "releases", "download"].as_slice()
    } else if host == "cnb.cool" {
        [
            GITHUB_OWNER,
            "foundation-sunshine-release",
            "-",
            "releases",
            "download",
        ]
        .as_slice()
    } else {
        return Err("下载地址主机不在允许范围内".to_string());
    };
    if segments.len() != expected_prefix.len() + 2
        || !segments
            .iter()
            .take(expected_prefix.len())
            .zip(expected_prefix)
            .all(|(actual, expected)| *actual == *expected)
    {
        return Err("下载地址不是 Foundation Sunshine Release 附件".to_string());
    }

    let encoded_tag = segments[expected_prefix.len()];
    let encoded_name = segments[expected_prefix.len() + 1];
    if decode_segment(encoded_tag)? != tag || decode_segment(encoded_name)? != name {
        return Err("下载地址的 tag 或文件名与 metadata 不一致".to_string());
    }
    Ok(())
}

fn parse_metadata(body: &[u8]) -> Result<ReleaseCatalog, String> {
    let document: MetadataDocument =
        serde_json::from_slice(body).map_err(|error| format!("metadata JSON 无效: {error}"))?;
    if document.schema != 1
        || document.kind != "release-metadata"
        || document.product != "foundation-sunshine"
    {
        return Err("metadata schema、kind 或 product 不匹配".to_string());
    }

    let latest = document
        .channels
        .latest
        .ok_or_else(|| "metadata 缺少 latest 通道".to_string())?;
    let latest = parse_metadata_release(latest, false)?;
    let pre_latest = document
        .channels
        .pre_latest
        .map(|release| parse_metadata_release(release, true))
        .transpose()?;

    Ok(ReleaseCatalog {
        latest: Some(latest),
        pre_latest,
    })
}

fn parse_metadata_release(
    release: MetadataRelease,
    expected_prerelease: bool,
) -> Result<ReleaseCandidate, String> {
    if release.version.trim().is_empty() || release.prerelease != expected_prerelease {
        return Err("metadata release 通道不匹配".to_string());
    }

    let mut assets = Vec::new();
    for asset in release.assets {
        if !known_asset_type(&asset.asset_type) {
            continue;
        }
        if asset.name.trim().is_empty() || asset.size == 0 {
            return Err("metadata 资产缺少有效名称或大小".to_string());
        }
        let sha256 = normalize_sha256(&asset.sha256)
            .ok_or_else(|| format!("metadata 资产 {} 的 SHA-256 无效", asset.name))?;
        validate_release_download_url(&asset.url, &release.version, &asset.name)?;
        let fallback_url = asset.fallback_url.filter(|url| !url.trim().is_empty());
        if let Some(fallback) = fallback_url.as_deref() {
            validate_release_download_url(fallback, &release.version, &asset.name)?;
        }
        assets.push(ReleaseAsset {
            asset_type: asset.asset_type,
            name: asset.name,
            size: asset.size,
            sha256,
            url: asset.url,
            fallback_url,
        });
    }

    Ok(ReleaseCandidate {
        version: release.version.clone(),
        prerelease: release.prerelease,
        github_release_id: release.github_release_id,
        published_at: release.published_at,
        release_notes: normalize_notes(release.release_notes),
        release_page: encoded_release_page(&release.version),
        assets,
    })
}

fn parse_github_asset(asset: GithubAsset, tag: &str) -> Option<ReleaseAsset> {
    let asset_type = classify_asset_name(&asset.name)?;
    let sha256 = normalize_sha256(asset.digest.as_deref()?)?;
    if asset.size == 0 {
        return None;
    }
    let url = asset.browser_download_url;
    if validate_release_download_url(&url, tag, &asset.name).is_err() {
        return None;
    }
    Some(ReleaseAsset {
        asset_type: asset_type.to_string(),
        name: asset.name,
        size: asset.size,
        sha256,
        url,
        fallback_url: None,
    })
}

fn candidate_from_github(release: GithubRelease) -> Option<ReleaseCandidate> {
    if release.draft {
        return None;
    }
    let assets = release
        .assets
        .into_iter()
        .filter_map(|asset| parse_github_asset(asset, &release.tag_name))
        .collect();
    Some(ReleaseCandidate {
        version: release.tag_name.clone(),
        prerelease: release.prerelease,
        github_release_id: Some(release.id),
        published_at: release.published_at,
        release_notes: normalize_notes(release.body),
        release_page: release
            .html_url
            .filter(|url| validate_github_release_page(url, &release.tag_name).is_ok())
            .unwrap_or_else(|| encoded_release_page(&release.tag_name)),
        assets,
    })
}

fn validate_github_release_page(url: &str, tag: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| "GitHub release 页面无效".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("GitHub release 页面来源不合法".to_string());
    }
    let segments = parsed
        .path_segments()
        .ok_or_else(|| "GitHub release 页面路径无效".to_string())?
        .collect::<Vec<_>>();
    if segments.len() != 5
        || segments[0] != GITHUB_OWNER
        || segments[1] != GITHUB_REPOSITORY
        || segments[2] != "releases"
        || segments[3] != "tag"
        || decode_segment(segments[4])? != tag
    {
        return Err("GitHub release 页面不是 Foundation Sunshine 页面".to_string());
    }
    Ok(())
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

pub(crate) fn compare_candidates(left: &ReleaseCandidate, right: &ReleaseCandidate) -> Ordering {
    if let (Some(left_id), Some(right_id)) = (left.github_release_id, right.github_release_id)
        && left_id != right_id
    {
        return left_id.cmp(&right_id);
    }
    if let (Some(left_time), Some(right_time)) = (&left.published_at, &right.published_at)
        && left_time != right_time
    {
        return left_time.cmp(right_time);
    }
    version_key(&left.version)
        .cmp(&version_key(&right.version))
        .then_with(|| match (left.prerelease, right.prerelease) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            _ => Ordering::Equal,
        })
        .then_with(|| left.github_release_id.cmp(&right.github_release_id))
        .then_with(|| left.published_at.cmp(&right.published_at))
        .then_with(|| left.version.cmp(&right.version))
}

pub(crate) fn candidate_is_newer_than(current_version: &str, candidate: &ReleaseCandidate) -> bool {
    version_key(&candidate.version) > version_key(current_version)
}

fn select_channel(catalog: &ReleaseCatalog, channel: UpdateChannel) -> Option<ReleaseCandidate> {
    match channel {
        UpdateChannel::StableOnly => catalog.latest.clone(),
        UpdateChannel::PrereleaseOnly => catalog.pre_latest.clone(),
        UpdateChannel::StableAndPrerelease => match (&catalog.latest, &catalog.pre_latest) {
            (Some(latest), Some(pre_latest)) => {
                if compare_candidates(pre_latest, latest) == Ordering::Greater {
                    Some(pre_latest.clone())
                } else {
                    Some(latest.clone())
                }
            }
            (Some(latest), None) => Some(latest.clone()),
            (None, Some(pre_latest)) => Some(pre_latest.clone()),
            (None, None) => None,
        },
    }
}

fn is_chinese_locale(locale: &str) -> bool {
    locale.trim().to_ascii_lowercase().starts_with("zh")
}

async fn current_locale_is_chinese() -> bool {
    crate::sunshine::get_sunshine_locale()
        .await
        .map(|locale| is_chinese_locale(&locale))
        .unwrap_or(false)
}

async fn fetch_github_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let client = create_metadata_client()?;
    let deadline = Instant::now() + METADATA_OVERALL_TIMEOUT;
    let body = request_bytes(
        &client,
        url,
        deadline,
        "application/vnd.github+json",
        4 * 1024 * 1024,
    )
    .await
    .map_err(|error| error.detail)?;
    serde_json::from_slice(&body).map_err(|error| format!("GitHub Release JSON 无效: {error}"))
}

async fn fetch_github_catalog(channel: UpdateChannel) -> Result<ReleaseCatalog, String> {
    let need_list = !matches!(channel, UpdateChannel::StableOnly);
    let latest_future = if matches!(channel, UpdateChannel::PrereleaseOnly) {
        None
    } else {
        Some(fetch_github_json::<GithubRelease>(GITHUB_LATEST_URL))
    };
    let list_future = if need_list {
        Some(fetch_github_json::<Vec<GithubRelease>>(GITHUB_RELEASES_URL))
    } else {
        None
    };

    let (latest_result, list_result) = match (latest_future, list_future) {
        (Some(latest), Some(list)) => {
            let (latest, list) = tokio::join!(latest, list);
            (latest.ok().and_then(candidate_from_github), list.ok())
        }
        (Some(latest), None) => (latest.await.ok().and_then(candidate_from_github), None),
        (None, Some(list)) => (None, list.await.ok()),
        (None, None) => (None, None),
    };

    let mut catalog = ReleaseCatalog {
        latest: latest_result,
        pre_latest: None,
    };
    if let Some(releases) = list_result {
        for candidate in releases.into_iter().filter_map(candidate_from_github) {
            if candidate.prerelease {
                let replace = catalog.pre_latest.as_ref().is_none_or(|current| {
                    compare_candidates(&candidate, current) == Ordering::Greater
                });
                if replace {
                    catalog.pre_latest = Some(candidate);
                }
            } else if catalog.latest.is_none()
                || catalog.latest.as_ref().is_some_and(|current| {
                    compare_candidates(&candidate, current) == Ordering::Greater
                })
            {
                catalog.latest = Some(candidate);
            }
        }
    }

    if matches!(channel, UpdateChannel::StableOnly) && catalog.latest.is_none() {
        return Err("GitHub 没有可用的稳定 Release".to_string());
    }
    if matches!(channel, UpdateChannel::PrereleaseOnly) && catalog.pre_latest.is_none() {
        return Err("GitHub 没有可用的 prerelease Release".to_string());
    }
    if matches!(channel, UpdateChannel::StableAndPrerelease)
        && catalog.latest.is_none()
        && catalog.pre_latest.is_none()
    {
        return Err("GitHub 没有可用的 Release".to_string());
    }
    Ok(catalog)
}

fn parse_cnb_download_links(html: &str) -> Result<(String, bool, Vec<(String, String)>), String> {
    let tag_re =
        Regex::new(r#"href="/AlkaidLab/foundation-sunshine-release/-/releases/tag/([^"?#]+)""#)
            .map_err(|error| error.to_string())?;
    let tag_match = tag_re
        .find(html)
        .ok_or_else(|| "CNB 页面缺少 latest Release tag".to_string())?;
    let encoded_tag = tag_re
        .captures(tag_match.as_str())
        .and_then(|capture| capture.get(1))
        .ok_or_else(|| "CNB 页面 latest Release tag 无效".to_string())?
        .as_str()
        .to_string();
    let tag = decode_segment(&encoded_tag)?;
    let release_window = &html[tag_match.start()..html.len().min(tag_match.end() + 4096)];
    let prerelease_re = Regex::new(r#"(?is)>\s*(?:pre-release|prerelease|预发布)\s*<"#)
        .map_err(|error| error.to_string())?;
    let is_prerelease = prerelease_re.is_match(release_window);

    let asset_re = Regex::new(
        r#"href="(/AlkaidLab/foundation-sunshine-release/-/releases/download/[^"?#]+)""#,
    )
    .map_err(|error| error.to_string())?;
    let mut assets = Vec::new();
    for capture in asset_re.captures_iter(html) {
        let path = capture.get(1).unwrap().as_str();
        let parsed = Url::parse(&format!("https://cnb.cool{path}"))
            .map_err(|error| format!("CNB 资产 URL 无效: {error}"))?;
        let segments = parsed
            .path_segments()
            .ok_or_else(|| "CNB 资产 URL 路径无效".to_string())?
            .collect::<Vec<_>>();
        if segments.len() != 7 || segments[5] != encoded_tag {
            continue;
        }
        let name = decode_segment(segments[6])?;
        if classify_asset_name(&name).is_some() {
            assets.push((name, parsed.to_string()));
        }
    }
    assets.sort_by(|left, right| left.0.cmp(&right.0));
    assets.dedup_by(|left, right| left.0 == right.0);
    Ok((tag, is_prerelease, assets))
}

async fn request_cnb_checksum(
    client: &reqwest::Client,
    url: &str,
    deadline: Instant,
) -> Result<std::collections::HashMap<String, String>, String> {
    let body = request_bytes(
        client,
        url,
        deadline,
        "application/json, text/plain",
        256 * 1024,
    )
    .await
    .map_err(|error| error.detail)?;
    if let Ok(entries) = serde_json::from_slice::<Vec<CnbChecksumEntry>>(&body) {
        return Ok(entries
            .into_iter()
            .filter_map(|entry| normalize_sha256(&entry.hash).map(|hash| (entry.file, hash)))
            .collect());
    }

    let text = String::from_utf8(body).map_err(|_| "CNB checksum 文件不是 UTF-8".to_string())?;
    let mut checksums = std::collections::HashMap::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(hash) = fields.next() else { continue };
        let Some(name) = fields.next() else { continue };
        if let Some(hash) = normalize_sha256(hash) {
            checksums.insert(name.trim_start_matches('*').to_string(), hash);
        }
    }
    Ok(checksums)
}

async fn probe_cnb_asset_size(
    client: &reqwest::Client,
    url: &str,
    deadline: Instant,
) -> Option<u64> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    let response = timeout(
        remaining,
        client
            .head(url)
            .header(
                reqwest::header::USER_AGENT,
                "Sunshine-Control-Panel updater",
            )
            .header(reqwest::header::RANGE, "bytes=0-0")
            .send(),
    )
    .await
    .ok()?
    .ok()?;
    let header_length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let is_partial = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    header_length
        .filter(|length| !is_partial && *length > 0)
        .or_else(|| {
            response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit('/').next())
                .and_then(|value| value.parse().ok())
        })
}

async fn fetch_cnb_catalog() -> Result<ReleaseCatalog, String> {
    let client = create_cnb_client()?;
    let deadline = Instant::now() + METADATA_OVERALL_TIMEOUT;
    let body = request_bytes(
        &client,
        CNB_RELEASES_URL,
        deadline,
        "text/html",
        MAX_METADATA_BYTES,
    )
    .await
    .map_err(|error| error.detail)?;
    let html = String::from_utf8(body).map_err(|_| "CNB Release 页面不是 UTF-8".to_string())?;
    let (tag, is_prerelease, links) = parse_cnb_download_links(&html)?;

    let checksum_url = links
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("checksums.json"))
        .or_else(|| {
            links
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        })
        .map(|(_, url)| url.clone())
        .ok_or_else(|| "CNB Release 缺少 checksum 文件".to_string())?;
    let checksums = request_cnb_checksum(&client, &checksum_url, deadline).await?;

    let mut assets = Vec::new();
    for (name, url) in links {
        let Some(asset_type) = classify_asset_name(&name) else {
            continue;
        };
        let Some(sha256) = checksums.get(&name).cloned() else {
            continue;
        };
        if asset_type == "checksum-json" || asset_type == "checksum-text" {
            continue;
        }
        let Some(size) = probe_cnb_asset_size(&client, &url, deadline).await else {
            continue;
        };
        let fallback_url = encoded_download_url(&tag, &name);
        assets.push(ReleaseAsset {
            asset_type: asset_type.to_string(),
            name,
            size,
            sha256,
            url,
            fallback_url: Some(fallback_url),
        });
    }
    if assets.is_empty() {
        return Err("CNB Release 没有可校验的更新资产".to_string());
    }

    let candidate = ReleaseCandidate {
        version: tag.clone(),
        prerelease: is_prerelease,
        github_release_id: None,
        published_at: None,
        release_notes: String::new(),
        release_page: encoded_release_page(&tag),
        assets,
    };
    Ok(ReleaseCatalog {
        latest: (!is_prerelease).then_some(candidate.clone()),
        pre_latest: is_prerelease.then_some(candidate),
    })
}

async fn ensure_channel_candidates(
    mut catalog: ReleaseCatalog,
    channel: UpdateChannel,
) -> Result<ReleaseCatalog, String> {
    if !matches!(channel, UpdateChannel::PrereleaseOnly) && catalog.latest.is_none() {
        match fetch_github_catalog(UpdateChannel::StableOnly).await {
            Ok(github_catalog) => catalog.latest = github_catalog.latest,
            Err(error) if matches!(channel, UpdateChannel::StableOnly) => return Err(error),
            Err(_) => {}
        }
    }

    if !matches!(channel, UpdateChannel::StableOnly) && catalog.pre_latest.is_none() {
        match fetch_github_catalog(UpdateChannel::PrereleaseOnly).await {
            Ok(github_catalog) => catalog.pre_latest = github_catalog.pre_latest,
            Err(error) if matches!(channel, UpdateChannel::PrereleaseOnly) => return Err(error),
            Err(_) => {}
        }
    }

    if matches!(channel, UpdateChannel::StableOnly) && catalog.latest.is_none() {
        return Err("没有可用的稳定 Release".to_string());
    }
    if matches!(channel, UpdateChannel::PrereleaseOnly) && catalog.pre_latest.is_none() {
        return Err("没有可用的 prerelease Release".to_string());
    }
    Ok(catalog)
}

pub(crate) async fn resolve_catalog(channel: UpdateChannel) -> Result<ReleaseCatalog, String> {
    if current_locale_is_chinese().await {
        let overall_deadline = Instant::now() + METADATA_OVERALL_TIMEOUT;
        let com = fetch_metadata_endpoint(METADATA_COM_URL, false, overall_deadline);
        let cn = fetch_metadata_endpoint(METADATA_CN_URL, true, overall_deadline);
        let (com, cn) = tokio::join!(com, cn);
        if let Ok(catalog) = com {
            return ensure_channel_candidates(catalog, channel).await;
        }
        if let Ok(catalog) = cn {
            return ensure_channel_candidates(catalog, channel).await;
        }

        if let Ok(catalog) = fetch_cnb_catalog().await {
            return ensure_channel_candidates(catalog, channel).await;
        }
        return fetch_github_catalog(channel).await;
    }

    fetch_github_catalog(channel).await
}

/// Resolve mirror candidates for a component whose exact release identity is
/// pinned by its installed manifest. Metadata is only used when it describes
/// the same asset and SHA-256; otherwise the tag from the canonical GitHub URL
/// is used to derive the CNB and GitHub candidates directly.
pub(crate) async fn resolve_component_download_candidates(
    asset_type: &str,
    asset_name: &str,
    expected_sha256: &str,
    canonical_url: &str,
) -> Result<Vec<DownloadCandidate>, String> {
    let expected_sha256 = normalize_sha256(expected_sha256)
        .ok_or_else(|| "组件 manifest 的 SHA-256 无效".to_string())?;
    let tag = github_release_download_tag(canonical_url, asset_name)?;
    let canonical_url = encoded_download_url(&tag, asset_name);
    let mut candidates = Vec::new();

    let mut add_candidate = |name: &'static str, url: String| {
        let Ok(parsed) = Url::parse(&url) else {
            return;
        };
        if parsed.scheme() != "https"
            || candidates
                .iter()
                .any(|item: &DownloadCandidate| item.url == url)
        {
            return;
        }
        candidates.push(DownloadCandidate { name, url });
    };

    if current_locale_is_chinese().await {
        // The normal metadata path already implements .com/.cn priority and
        // CNB/GitHub fallback. Do not accept a newer or different component:
        // Sidecar must match the installed Sunshine manifest exactly.
        if let Ok(catalog) = resolve_catalog(UpdateChannel::StableOnly).await {
            if let Some(release) = catalog.latest {
                for asset in release.assets.into_iter().filter(|asset| {
                    asset.asset_type == asset_type
                        && asset.name == asset_name
                        && asset.sha256.eq_ignore_ascii_case(&expected_sha256)
                }) {
                    add_candidate("official metadata", asset.url);
                    if let Some(fallback) = asset.fallback_url {
                        add_candidate("metadata fallback", fallback);
                    }
                }
            }
        }

        add_candidate("CNB", encoded_cnb_download_url(&tag, asset_name));
    }

    add_candidate("GitHub", canonical_url);
    if candidates.is_empty() {
        return Err("没有可用的组件下载地址".to_string());
    }
    Ok(candidates)
}

pub(crate) fn select_candidate(
    catalog: &ReleaseCatalog,
    channel: UpdateChannel,
) -> Option<ReleaseCandidate> {
    select_channel(catalog, channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_json() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema": 1,
            "kind": "release-metadata",
            "product": "foundation-sunshine",
            "channels": {
                "latest": {
                    "version": "v2026.925.152547.杂鱼",
                    "prerelease": false,
                    "githubReleaseId": 10,
                    "publishedAt": "2026-09-25T15:45:20Z",
                    "releaseNotes": "# stable",
                    "assets": [{
                        "type": "windows-x64-installer",
                        "name": "Sunshine.v2026.0925.WindowsInstaller.exe",
                        "size": 10,
                        "sha256": "a".repeat(64),
                        "url": "https://cnb.cool/AlkaidLab/foundation-sunshine-release/-/releases/download/v2026.925.152547.%E6%9D%82%E9%B1%BC/Sunshine.v2026.0925.WindowsInstaller.exe",
                        "fallbackUrl": "https://github.com/AlkaidLab/foundation-sunshine/releases/download/v2026.925.152547.%E6%9D%82%E9%B1%BC/Sunshine.v2026.0925.WindowsInstaller.exe"
                    }, {
                        "type": "future-asset",
                        "name": "ignored.bin",
                        "size": 1,
                        "sha256": "b".repeat(64),
                        "url": "https://example.invalid/ignored.bin"
                    }]
                },
                "pre-latest": {
                    "version": "v2026.924.120000.杂鱼",
                    "prerelease": true,
                    "githubReleaseId": 9,
                    "publishedAt": "2026-09-24T12:00:00Z",
                    "releaseNotes": "pre",
                    "assets": []
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn parses_metadata_and_ignores_unknown_assets() {
        let catalog = parse_metadata(&metadata_json()).unwrap();
        assert_eq!(catalog.latest.as_ref().unwrap().assets.len(), 1);
        assert!(catalog.pre_latest.is_some());
        assert_eq!(catalog.latest.as_ref().unwrap().release_notes, "# stable");
    }

    #[test]
    fn test_channel_picks_newer_stable_release_when_pre_is_older() {
        let catalog = parse_metadata(&metadata_json()).unwrap();
        let selected = select_candidate(&catalog, UpdateChannel::StableAndPrerelease).unwrap();
        assert!(!selected.prerelease);
    }

    #[test]
    fn test_channel_picks_newer_prerelease_when_it_is_newer() {
        let mut catalog = parse_metadata(&metadata_json()).unwrap();
        catalog.pre_latest.as_mut().unwrap().version = "v2026.930.010000.杂鱼".to_string();
        catalog.pre_latest.as_mut().unwrap().github_release_id = Some(11);
        let selected = select_candidate(&catalog, UpdateChannel::StableAndPrerelease).unwrap();
        assert!(selected.prerelease);
    }

    #[test]
    fn test_channel_uses_release_identity_when_numeric_versions_are_equal() {
        let mut catalog = parse_metadata(&metadata_json()).unwrap();
        catalog.pre_latest.as_mut().unwrap().version = "v2026.925.152547.beta".to_string();
        catalog.pre_latest.as_mut().unwrap().github_release_id = Some(11);
        let selected = select_candidate(&catalog, UpdateChannel::StableAndPrerelease).unwrap();
        assert!(selected.prerelease);
    }

    #[test]
    fn release_notes_are_trimmed_and_bounded_on_utf8_boundaries() {
        let mut body: serde_json::Value = serde_json::from_slice(&metadata_json()).unwrap();
        body["channels"]["latest"]["releaseNotes"] =
            serde_json::Value::String(format!("  {}  ", "杂".repeat(MAX_RELEASE_NOTES_BYTES)));
        let catalog = parse_metadata(&serde_json::to_vec(&body).unwrap()).unwrap();
        let notes = &catalog.latest.unwrap().release_notes;
        assert!(notes.len() <= MAX_RELEASE_NOTES_BYTES);
        assert!(notes.is_char_boundary(notes.len()));
        assert!(!notes.starts_with(' '));
    }

    #[test]
    fn release_page_and_download_urls_encode_unicode_segments() {
        let page = encoded_release_page("v2026.925.152547.杂鱼");
        let asset = encoded_download_url("v2026.925.152547.杂鱼", "安装包.exe");
        assert!(page.contains("%E6%9D%82%E9%B1%BC"));
        assert!(asset.contains("%E5%AE%89%E8%A3%85%E5%8C%85.exe"));
        assert!(!page.contains("/tag//"));
        assert!(!asset.contains("/download//"));
    }

    #[test]
    fn component_mirror_url_preserves_release_tag_and_asset_name() {
        let github = encoded_download_url("v2026.925.152547.杂鱼", "Sunshine.Ds5Sidecar.x64.zip");
        let tag = github_release_download_tag(&github, "Sunshine.Ds5Sidecar.x64.zip").unwrap();
        let cnb = encoded_cnb_download_url(&tag, "Sunshine.Ds5Sidecar.x64.zip");
        assert!(cnb.contains("cnb.cool/AlkaidLab/foundation-sunshine-release"));
        assert!(cnb.contains("%E6%9D%82%E9%B1%BC"));
        assert!(cnb.ends_with("Sunshine.Ds5Sidecar.x64.zip"));
    }

    #[test]
    fn component_mirror_url_rejects_another_repository() {
        assert!(
            github_release_download_tag(
                "https://github.com/other/project/releases/download/v1/Sunshine.Ds5Sidecar.x64.zip",
                "Sunshine.Ds5Sidecar.x64.zip",
            )
            .is_err()
        );
    }

    #[test]
    fn parses_cnb_unicode_tag_and_release_asset_links() {
        let html = r#"
          <a href="/AlkaidLab/foundation-sunshine-release/-/releases/tag/v2026.925.152547.%E6%9D%82%E9%B1%BC">latest</a>
          <a href="/AlkaidLab/foundation-sunshine-release/-/releases/download/v2026.925.152547.%E6%9D%82%E9%B1%BC/Sunshine.v2026.0925.WindowsInstaller.exe">download</a>
        "#;
        let (tag, is_prerelease, assets) = parse_cnb_download_links(html).unwrap();
        assert_eq!(tag, "v2026.925.152547.杂鱼");
        assert!(!is_prerelease);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].0, "Sunshine.v2026.0925.WindowsInstaller.exe");
    }

    #[test]
    fn parses_cnb_prerelease_label_in_latest_release_card() {
        let html = r#"
          <a href="/AlkaidLab/foundation-sunshine-release/-/releases/tag/v2026.1001.112543.%E6%9D%82%E9%B1%BC">latest</a>
          <svg><text>Pre-release</text></svg>
          <a href="/AlkaidLab/foundation-sunshine-release/-/releases/download/v2026.1001.112543.%E6%9D%82%E9%B1%BC/Sunshine.v2026.1001.WindowsInstaller.exe">download</a>
        "#;
        let (tag, is_prerelease, assets) = parse_cnb_download_links(html).unwrap();
        assert_eq!(tag, "v2026.1001.112543.杂鱼");
        assert!(is_prerelease);
        assert_eq!(assets.len(), 1);
    }

    #[test]
    fn version_comparison_handles_prerelease_and_numeric_builds() {
        let stable = ReleaseCandidate {
            version: "v2026.925.152547.杂鱼".to_string(),
            prerelease: false,
            github_release_id: Some(2),
            published_at: None,
            release_notes: String::new(),
            release_page: String::new(),
            assets: Vec::new(),
        };
        let pre = ReleaseCandidate {
            version: "v2026.926.010000.杂鱼".to_string(),
            prerelease: true,
            ..stable.clone()
        };
        assert_eq!(compare_candidates(&pre, &stable), Ordering::Greater);
        assert!(candidate_is_newer_than("v2026.925.152547.旧", &pre));
    }
}
