use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::github_download::{
    self, DEFAULT_CONNECT_TIMEOUT, DEFAULT_IDLE_TIMEOUT, DEFAULT_RESPONSE_TIMEOUT,
    DownloadAttemptPhase, DownloadRequest,
};

mod channel;
mod preferences;

use channel::ReleaseChannel;

const UPDATER_HELPER_ARG: &str = "--updater-helper";
const UPDATE_RESULT_ARG: &str = "--update-result";

// ========== 常量定义 ==========
const GITHUB_API_URL: &str = "https://api.github.com/repos/qiin2333/sunshine/releases";
const GITHUB_API_URL_LATEST: &str =
    "https://api.github.com/repos/qiin2333/sunshine/releases/latest";
const UPDATE_CHECK_INTERVAL: u64 = 4 * 60 * 60; // 4小时（秒）
const HTTP_TIMEOUT_SECS: u64 = 3;
const GITHUB_RELEASE_HOST: &str = "github.com";
const MAX_RELEASES_TO_CHECK: usize = 10; // 最多检查的发布数量

// GitHub API 加速代理列表（按优先级排序）
const API_PROXY_PREFIXES: &[&str] = &["https://ghapi.hackhub.cn/", "https://mirror.ghproxy.com/"];

// ========== 数据结构定义 ==========

/// 更新信息，`is_latest` 决定前端以“查看更新内容”还是“下载新版本”模式展示。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: Option<String>,
    pub download_name: Option<String>,
    pub download_size: Option<u64>,
    pub release_page: String,
    /// `true` = 已是最新（只读浏览），`false`（默认）= 有可用更新
    #[serde(default)]
    pub is_latest: bool,
}

/// GitHub Release 数据结构
#[derive(Debug, Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
    assets: Vec<GitHubAsset>,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    published_at: Option<String>,
}

/// GitHub Release Asset 数据结构
#[derive(Debug, Serialize, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum DownloadPhase {
    Connecting,
    Downloading,
    Retrying,
    Verifying,
    Complete,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DownloadErrorCode {
    SetupFailed,
    FilePreparationFailed,
    FileFinalizationFailed,
    SourcesExhausted,
}

#[derive(Debug, Serialize)]
pub(crate) struct DownloadCommandError {
    code: DownloadErrorCode,
    detail: String,
}

impl DownloadCommandError {
    fn new(code: DownloadErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdaterHelperState {
    installer_path: String,
    extension: String,
    target_version: Option<String>,
    gui_exe_path: String,
    result_path: String,
    parent_pid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UpdaterHelperResult {
    success: bool,
    exit_code: Option<i32>,
    target_version: Option<String>,
    message: String,
    finished_at: u64,
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct UpdaterPanelState {
    title: String,
    subtitle: String,
    step: usize,
    failed: bool,
    animation_tick: u32,
}

#[cfg(target_os = "windows")]
struct UpdaterAnimationFrame {
    width: i32,
    height: i32,
    bgra: Vec<u8>,
}

#[cfg(target_os = "windows")]
static UPDATER_PANEL_STATE: OnceLock<Arc<Mutex<UpdaterPanelState>>> = OnceLock::new();
static UPDATE_CHECKER_STARTED: OnceLock<()> = OnceLock::new();

#[cfg(target_os = "windows")]
static UPDATER_CONSTRUCTION_FRAMES: OnceLock<Option<Vec<UpdaterAnimationFrame>>> = OnceLock::new();

// ========== 版本相关 ==========

/// 获取当前 Sunshine 版本
async fn get_current_sunshine_version() -> Result<String, String> {
    use crate::sunshine;
    sunshine::get_sunshine_version().await
}

/// 规范化版本号（移除 v/V 前缀）
fn normalize_version(version: &str) -> String {
    version
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

/// 比较版本号，判断是否有新版本
fn is_new_version_available(current: &str, latest: &str) -> bool {
    let current = normalize_version(current);
    let latest = normalize_version(latest);

    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    let latest_parts: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();

    let max_len = current_parts.len().max(latest_parts.len());

    for i in 0..max_len {
        let current_part = current_parts.get(i).copied().unwrap_or(0);
        let latest_part = latest_parts.get(i).copied().unwrap_or(0);

        if latest_part > current_part {
            return true;
        } else if latest_part < current_part {
            return false;
        }
    }

    false
}

fn is_full_sunshine_windows_installer(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("sunshine") {
        return false;
    }

    let normalized: String = lower
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    let supported_extension = lower.ends_with(".exe") || lower.ends_with(".msi");

    supported_extension && normalized.contains("windowsinstaller")
}

/// 查找完整 Sunshine Windows 安装包。
fn find_best_download_asset(
    assets: &[GitHubAsset],
) -> (Option<String>, Option<String>, Option<u64>) {
    if let Some(asset) = assets
        .iter()
        .find(|asset| is_full_sunshine_windows_installer(&asset.name))
    {
        (
            Some(asset.browser_download_url.clone()),
            Some(asset.name.clone()),
            (asset.size > 0).then_some(asset.size),
        )
    } else {
        (None, None, None)
    }
}

// ========== HTTP 请求相关 ==========

/// 构建代理 URL
fn build_proxy_url(proxy: &str, original_url: &str) -> String {
    let url_without_protocol = original_url.trim_start_matches("https://");
    if proxy.ends_with('/') {
        format!("{}{}", proxy, url_without_protocol)
    } else {
        format!("{}/{}", proxy, url_without_protocol)
    }
}

/// 创建 GitHub API HTTP 客户端
fn create_api_http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
}

/// 尝试单个 GitHub API URL 请求
async fn try_api_request(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
    let response = client
        .get(url)
        .header("User-Agent", "Sunshine-Control-Panel")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if response.status().is_success() {
        debug!("✅ 请求成功，来源: {}", url);
        Ok(response)
    } else {
        Err(format!("HTTP状态码 {}", response.status().as_u16()))
    }
}

/// 按顺序尝试全部 GitHub API 地址。
async fn fetch_api_with_fallbacks(
    urls: &[String],
    timeout_secs: u64,
) -> Result<reqwest::Response, String> {
    let client = create_api_http_client(timeout_secs)?;

    for url in urls {
        match try_api_request(&client, url).await {
            Ok(response) => return Ok(response),
            Err(e) => warn!("⚠️ {}: {}", url, e),
        }
    }

    Err("所有请求方式都失败了".to_string())
}

/// 使用适当的加速代理获取GitHub API数据
async fn http_get_with_proxies(url: &str) -> Result<String, String> {
    // 构造尝试的URL列表：先直连，再尝试代理
    let mut urls_to_try = vec![url.to_string()];

    for proxy in API_PROXY_PREFIXES {
        urls_to_try.push(build_proxy_url(proxy, url));
    }

    let response = fetch_api_with_fallbacks(&urls_to_try, HTTP_TIMEOUT_SECS).await?;

    response
        .text()
        .await
        .map_err(|e| format!("读取响应内容失败: {}", e))
}

/// 获取所有发布版本（包括预发布）
async fn fetch_all_releases() -> Result<Vec<GitHubRelease>, String> {
    let json = http_get_with_proxies(GITHUB_API_URL).await?;

    let releases: Vec<GitHubRelease> =
        serde_json::from_str(&json).map_err(|e| format!("解析GitHub API响应失败: {}", e))?;

    Ok(releases)
}

/// 获取最新稳定版本
async fn fetch_latest_stable_release() -> Result<GitHubRelease, String> {
    let json = http_get_with_proxies(GITHUB_API_URL_LATEST).await?;

    let release: GitHubRelease =
        serde_json::from_str(&json).map_err(|e| format!("解析GitHub API响应失败: {}", e))?;

    Ok(release)
}

/// 获取发布版本列表（包含回退逻辑）
async fn get_releases() -> Result<Vec<GitHubRelease>, String> {
    match fetch_all_releases().await {
        Ok(releases) => Ok(releases),
        Err(e) => {
            warn!("⚠️ 获取所有发布版本失败: {}, 尝试获取最新稳定版本", e);
            let release = fetch_latest_stable_release().await?;
            Ok(vec![release])
        }
    }
}

/// 从 GitHub release 构建 `UpdateInfo`（`is_latest` 默认为 `false`，调用方按需设置）。
fn create_update_info(release: &GitHubRelease) -> UpdateInfo {
    let (download_url, download_name, download_size) = find_best_download_asset(&release.assets);

    UpdateInfo {
        version: release.tag_name.clone(),
        release_notes: release.body.clone(),
        download_url,
        download_name,
        download_size,
        release_page: release.html_url.clone(),
        is_latest: false,
    }
}

/// 检查更新核心逻辑。
///
/// `manual=true` 时已是最新也返回 `Some(info { is_latest: true })`，
/// `manual=false` 时已是最新返回 `None`。
pub async fn check_for_updates_internal(
    manual: bool,
    include_prerelease: bool,
) -> Result<Option<UpdateInfo>, String> {
    let channel = if include_prerelease {
        ReleaseChannel::IncludePrerelease
    } else {
        ReleaseChannel::Stable
    };
    check_for_updates_in_channel(manual, channel).await
}

async fn check_for_updates_in_channel(
    manual: bool,
    channel: ReleaseChannel,
) -> Result<Option<UpdateInfo>, String> {
    info!("🔍 开始检查更新... (通道: {})", channel.description());

    // 获取当前 Sunshine 版本
    let current_version = match get_current_sunshine_version().await {
        Ok(ver) => normalize_version(&ver),
        Err(e) => {
            warn!("⚠️ 获取 Sunshine 版本失败: {}, 使用默认版本 0.0.0", e);
            "0.0.0".to_string()
        }
    };

    // 获取发布版本列表
    let releases = get_releases().await?;

    if releases.is_empty() {
        return Err("未找到任何发布版本".to_string());
    }

    // 查找最新的可用发布版本
    let release = releases
        .iter()
        .take(MAX_RELEASES_TO_CHECK)
        .filter(|release| !release.draft)
        .find(|release| channel.matches(release.prerelease))
        .ok_or_else(|| "未找到可用的发布版本".to_string())?;

    let latest_version = normalize_version(&release.tag_name);

    info!(
        "📊 当前 Sunshine 版本: {}, 最新版本: {} ({})",
        current_version,
        latest_version,
        if release.prerelease {
            "预发布"
        } else {
            "稳定版"
        }
    );

    if !is_new_version_available(&current_version, &latest_version) {
        if manual {
            let mut info = create_update_info(release);
            info.is_latest = true;
            return Ok(Some(info));
        }
        return Ok(None);
    }

    // 存在可用的新版本
    let update_info = create_update_info(release);
    Ok(Some(update_info))
}

/// 获取当前时间戳（秒）
fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn try_run_updater_helper_from_args() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(index) = args.iter().position(|arg| arg == UPDATER_HELPER_ARG) else {
        return false;
    };

    let Some(state_path) = args.get(index + 1) else {
        return true;
    };

    #[cfg(target_os = "windows")]
    {
        let _ = run_updater_helper(Path::new(state_path));
    }

    true
}

pub fn emit_update_result_if_requested(app_handle: &AppHandle) {
    let args: Vec<String> = std::env::args().collect();
    let Some(index) = args.iter().position(|arg| arg == UPDATE_RESULT_ARG) else {
        return;
    };

    let Some(result_path) = args.get(index + 1) else {
        return;
    };

    let result_path = PathBuf::from(result_path);
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        match fs::read_to_string(&result_path)
            .ok()
            .and_then(|content| serde_json::from_str::<UpdaterHelperResult>(&content).ok())
        {
            Some(result) => {
                let _ = app.emit("update-install-result", result);
                if let Some(dir) = updater_cleanup_dir_for_result(&result_path) {
                    let _ = fs::remove_file(&result_path);
                    let _ = fs::remove_dir_all(dir);
                }
            }
            None => {
                let _ = app.emit(
                    "update-install-result",
                    serde_json::json!({
                        "success": false,
                        "exit_code": null,
                        "target_version": null,
                        "message": "Unable to read updater result.",
                        "finished_at": get_current_timestamp()
                    }),
                );
            }
        }
    });
}

fn updater_cleanup_dir_for_result(result_path: &Path) -> Option<PathBuf> {
    if result_path.file_name()?.to_string_lossy() != "result.json" {
        return None;
    }

    let parent = result_path.parent()?;
    let dir_name = parent.file_name()?.to_string_lossy();
    if !dir_name.starts_with("sunshine-updater-") {
        return None;
    }

    let temp_dir = std::env::temp_dir().canonicalize().ok()?;
    let parent = parent.canonicalize().ok()?;
    if parent.starts_with(temp_dir) {
        Some(parent)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn run_updater_helper(state_path: &Path) -> Result<(), String> {
    let state_content =
        fs::read_to_string(state_path).map_err(|e| format!("read updater state failed: {}", e))?;
    let state: UpdaterHelperState = serde_json::from_str(&state_content)
        .map_err(|e| format!("parse updater state failed: {}", e))?;

    run_updater_panel(state)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_updater_worker(state: UpdaterHelperState, hwnd: isize) {
    update_updater_panel(hwnd, 0, "准备更新", "正在等待控制面板关闭...");
    wait_for_parent_exit(state.parent_pid);

    if is_gui_component_installer(Path::new(&state.installer_path)) {
        update_updater_panel(
            hwnd,
            1,
            "正在更新 GUI",
            "Sunshine 核心和当前串流将保持运行。",
        );
    } else {
        update_updater_panel(
            hwnd,
            1,
            "正在关闭 Sunshine",
            "正在关闭 Sunshine 服务，随后安装更新...",
        );
        stop_sunshine_for_update();
    }

    update_updater_panel(
        hwnd,
        2,
        "正在安装更新",
        "这可能需要一两分钟，请不要重复启动。",
    );
    let install_result = run_installer_and_wait(&state);

    update_updater_panel(
        hwnd,
        3,
        "正在完成",
        "安装已结束，正在准备重新打开控制面板。",
    );
    let helper_result = match install_result {
        Ok(code) => UpdaterHelperResult {
            success: code == 0,
            exit_code: Some(code),
            target_version: state.target_version.clone(),
            message: if code == 0 {
                "Update installed successfully.".to_string()
            } else {
                format!("Installer exited with code {}.", code)
            },
            finished_at: get_current_timestamp(),
        },
        Err(error) => UpdaterHelperResult {
            success: false,
            exit_code: None,
            target_version: state.target_version.clone(),
            message: error,
            finished_at: get_current_timestamp(),
        },
    };

    if helper_result.success {
        update_updater_panel(hwnd, 3, "更新完成", "正在重新打开 Sunshine 控制面板。");
    } else {
        update_updater_panel(hwnd, 3, "更新未完成", "正在重新打开控制面板并显示结果。");
    }

    let _ = write_updater_result(&state.result_path, &helper_result);
    let _ = restart_gui_with_update_result(&state.gui_exe_path, &state.result_path);

    std::thread::sleep(Duration::from_millis(900));
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(windows::Win32::Foundation::HWND(hwnd as *mut _)),
            windows::Win32::UI::WindowsAndMessaging::WM_APP + 2,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}
fn is_gui_component_installer(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase().starts_with("sunshine-gui-setup-"))
        .unwrap_or(false)
}

#[cfg(test)]
mod component_installer_tests {
    use super::*;

    #[test]
    fn recognizes_gui_component_installer_name() {
        assert!(is_gui_component_installer(Path::new(
            "Sunshine-GUI-Setup-1.2.3.exe"
        )));
        assert!(!is_gui_component_installer(Path::new("Sunshine.exe")));
    }

    #[test]
    fn full_update_asset_ignores_gui_component() {
        let assets = vec![
            GitHubAsset {
                name: "Sunshine-GUI-Setup-1.2.3.exe".to_string(),
                browser_download_url: "https://example.invalid/gui".to_string(),
                size: 10,
            },
            GitHubAsset {
                name: "Sunshine.v1.2.3.WindowsInstaller.exe".to_string(),
                browser_download_url: "https://example.invalid/full".to_string(),
                size: 20,
            },
        ];

        let (url, name, size) = find_best_download_asset(&assets);
        assert_eq!(url.as_deref(), Some("https://example.invalid/full"));
        assert_eq!(
            name.as_deref(),
            Some("Sunshine.v1.2.3.WindowsInstaller.exe")
        );
        assert_eq!(size, Some(20));
    }

    #[test]
    fn full_update_asset_ignores_sidecar_and_portable_archives() {
        let assets = vec![
            GitHubAsset {
                name: "Sunshine.Ds5Sidecar.Windows-x64.zip".to_string(),
                browser_download_url: "https://example.invalid/sidecar".to_string(),
                size: 10,
            },
            GitHubAsset {
                name: "Sunshine.v1.2.3.WindowsPortable.zip".to_string(),
                browser_download_url: "https://example.invalid/portable".to_string(),
                size: 20,
            },
            GitHubAsset {
                name: "Sunshine.v1.2.3.WindowsInstaller.exe".to_string(),
                browser_download_url: "https://example.invalid/installer".to_string(),
                size: 30,
            },
        ];

        let (url, name, size) = find_best_download_asset(&assets);
        assert_eq!(url.as_deref(), Some("https://example.invalid/installer"));
        assert_eq!(
            name.as_deref(),
            Some("Sunshine.v1.2.3.WindowsInstaller.exe")
        );
        assert_eq!(size, Some(30));
    }

    #[test]
    fn full_update_returns_none_without_an_installer() {
        let assets = vec![
            GitHubAsset {
                name: "checksums.json".to_string(),
                browser_download_url: "https://example.invalid/checksums".to_string(),
                size: 10,
            },
            GitHubAsset {
                name: "Sunshine.Ds5Sidecar.Windows-x64.zip".to_string(),
                browser_download_url: "https://example.invalid/sidecar".to_string(),
                size: 20,
            },
        ];

        assert_eq!(find_best_download_asset(&assets), (None, None, None));
    }

    #[test]
    fn full_update_returns_none_for_gui_only_release() {
        let assets = vec![
            GitHubAsset {
                name: "Sunshine-GUI-Setup-1.2.3.exe".to_string(),
                browser_download_url: "https://example.invalid/gui-setup".to_string(),
                size: 10,
            },
            GitHubAsset {
                name: "sunshine-gui.exe".to_string(),
                browser_download_url: "https://example.invalid/gui-exe".to_string(),
                size: 20,
            },
        ];

        assert_eq!(find_best_download_asset(&assets), (None, None, None));
    }
}

#[cfg(target_os = "windows")]
fn run_updater_panel(state: UpdaterHelperState) -> Result<(), String> {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, HBRUSH, PAINTSTRUCT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
        HWND_TOPMOST, MSG, PostQuitMessage, RegisterClassW, SW_SHOW, SWP_SHOWWINDOW,
        SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WINDOW_STYLE,
        WM_APP, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_NCHITTEST, WM_PAINT, WM_TIMER, WNDCLASSW,
        WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };
    use windows::core::PCWSTR;

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let _ = unsafe { BeginPaint(hwnd, &mut ps) };
                let _ = unsafe { EndPaint(hwnd, &ps) };
                draw_updater_panel_cutout(hwnd);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_CLOSE => {
                update_updater_panel(
                    hwnd.0 as isize,
                    current_updater_step(),
                    "正在安装更新",
                    "更新正在进行，请稍等片刻。",
                );
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ =
                    unsafe { windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), 1) };
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            WM_TIMER => {
                tick_updater_panel();
                draw_updater_panel_cutout(hwnd);
                LRESULT(0)
            }
            WM_NCHITTEST => LRESULT(windows::Win32::UI::WindowsAndMessaging::HTCAPTION as isize),
            x if x == WM_APP + 1 => {
                draw_updater_panel_cutout(hwnd);
                LRESULT(0)
            }
            x if x == WM_APP + 2 => {
                let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    let state_arc = Arc::new(Mutex::new(UpdaterPanelState {
        title: "准备更新".to_string(),
        subtitle: "正在启动更新助手...".to_string(),
        step: 0,
        failed: false,
        animation_tick: 0,
    }));
    let _ = UPDATER_PANEL_STATE.set(state_arc);

    let class_name = to_wide_null("SunshineUpdaterPanel");
    let window_title = to_wide_null("Sunshine 正在更新");
    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(std::ptr::null_mut()),
        hbrBackground: HBRUSH(std::ptr::null_mut()),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };

    unsafe { RegisterClassW(&wnd_class) };

    let width = 560;
    let height = 240;
    let (x, y) = updater_panel_position(width, height);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            WINDOW_STYLE(WS_POPUP.0),
            x,
            y,
            width,
            height,
            None,
            None,
            Some(HINSTANCE(std::ptr::null_mut())),
            None,
        )
    }
    .map_err(|e| format!("create updater panel failed: {}", e))?;

    let hwnd_value = hwnd.0 as isize;
    std::thread::spawn(move || run_updater_worker(state, hwnd_value));

    let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_SHOWWINDOW,
        )
    };
    let _ = unsafe { SetTimer(Some(hwnd), 1, 90, None) };
    let _ = unsafe { SetForegroundWindow(hwnd) };
    draw_updater_panel_cutout(hwnd);
    let _ = unsafe { windows::Win32::Graphics::Gdi::UpdateWindow(hwnd) };

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        let _ = unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn update_updater_panel(hwnd: isize, step: usize, title: &str, subtitle: &str) {
    if let Some(state) = UPDATER_PANEL_STATE.get() {
        if let Ok(mut state) = state.lock() {
            state.step = step;
            state.title = title.to_string();
            state.subtitle = subtitle.to_string();
            state.failed = title.contains("未完成");
        }
    }

    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(windows::Win32::Foundation::HWND(hwnd as *mut _)),
            windows::Win32::UI::WindowsAndMessaging::WM_APP + 1,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

#[cfg(target_os = "windows")]
fn tick_updater_panel() {
    if let Some(state) = UPDATER_PANEL_STATE.get() {
        if let Ok(mut state) = state.lock() {
            state.animation_tick = state.animation_tick.wrapping_add(1);
        }
    }
}

#[cfg(target_os = "windows")]
fn current_updater_step() -> usize {
    UPDATER_PANEL_STATE
        .get()
        .and_then(|state| state.lock().ok().map(|state| state.step))
        .unwrap_or(1)
}

#[cfg(target_os = "windows")]
fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn updater_panel_position(width: i32, height: i32) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    if screen_width > width && screen_height > height {
        return ((screen_width - width) / 2, (screen_height - height) / 2);
    }

    (80, 80)
}

#[cfg(target_os = "windows")]
fn draw_updater_panel_cutout(hwnd: windows::Win32::Foundation::HWND) {
    use std::ffi::c_void;
    use windows::Win32::Foundation::{POINT, RECT, SIZE};
    use windows::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DT_LEFT, DT_WORDBREAK, DeleteDC,
        DeleteObject, GetDC, ReleaseDC, SelectObject, SetBkMode, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, ULW_ALPHA, UpdateLayeredWindow,
    };

    let state = UPDATER_PANEL_STATE
        .get()
        .and_then(|state| state.lock().ok().map(|state| state.clone()))
        .unwrap_or_else(|| UpdaterPanelState {
            title: "正在安装更新".to_string(),
            subtitle: "请稍等片刻。".to_string(),
            step: 1,
            failed: false,
            animation_tick: 0,
        });

    unsafe {
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width <= 0 || height <= 0 {
            return;
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: (width * height * 4) as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            ..Default::default()
        };

        let screen_hdc = GetDC(None);
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(None, &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = ReleaseDC(None, screen_hdc);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(bitmap.into());
            let _ = ReleaseDC(None, screen_hdc);
            return;
        }

        let buffer_len = (width * height * 4) as usize;
        let buffer = std::slice::from_raw_parts_mut(bits as *mut u8, buffer_len);
        buffer.fill(0);

        draw_updater_animation_frame(buffer, width, height, &state);
        draw_updater_bottom_copy(buffer, width, height);
        let mem_hdc = CreateCompatibleDC(Some(screen_hdc));
        if mem_hdc.is_invalid() {
            let _ = DeleteObject(bitmap.into());
            let _ = ReleaseDC(None, screen_hdc);
            return;
        }
        let old_bitmap = SelectObject(mem_hdc, bitmap.into());

        let status_line = format!("{} - {}", state.title, state.subtitle);
        let _ = SetBkMode(mem_hdc, TRANSPARENT);
        draw_text_styled(
            mem_hdc,
            &status_line,
            82,
            186,
            486,
            210,
            DT_LEFT | DT_WORDBREAK,
            13,
            500,
            windows::Win32::Foundation::COLORREF(0x00705F5A),
        );
        restore_gdi_text_alpha(buffer, width, height, 82, 186, 486, 210);

        let mut window_rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut window_rect);
        let destination = POINT {
            x: window_rect.left,
            y: window_rect.top,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = UpdateLayeredWindow(
            hwnd,
            Some(screen_hdc),
            Some(&destination as *const POINT),
            Some(&size as *const SIZE),
            Some(mem_hdc),
            Some(&source as *const POINT),
            windows::Win32::Foundation::COLORREF(0),
            Some(&blend as *const BLENDFUNCTION),
            ULW_ALPHA,
        );

        let _ = SelectObject(mem_hdc, old_bitmap);
        let _ = DeleteDC(mem_hdc);
        let _ = DeleteObject(bitmap.into());
        let _ = ReleaseDC(None, screen_hdc);
    }
}

#[cfg(target_os = "windows")]
fn draw_updater_animation_frame(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    state: &UpdaterPanelState,
) {
    let Some(frames) = updater_construction_frames() else {
        return;
    };
    if frames.is_empty() {
        return;
    }

    let frame_index = (state.animation_tick as usize) % frames.len();
    let frame = &frames[frame_index];
    let copy_width = width.min(frame.width).max(0) as usize;
    let copy_height = height.min(frame.height).max(0) as usize;
    let dst_stride = width as usize * 4;
    let src_stride = frame.width as usize * 4;

    for y in 0..copy_height {
        let dst_start = y * dst_stride;
        let src_start = y * src_stride;
        let byte_len = copy_width * 4;
        buffer[dst_start..dst_start + byte_len]
            .copy_from_slice(&frame.bgra[src_start..src_start + byte_len]);
    }
}

#[cfg(target_os = "windows")]
fn draw_updater_bottom_copy(buffer: &mut [u8], width: i32, height: i32) {
    fill_bgra_rect(buffer, width, height, 72, 186, 512, 224, (58, 63, 75, 220));
    fill_bgra_rect(
        buffer,
        width,
        height,
        64,
        178,
        504,
        216,
        (255, 245, 221, 255),
    );
    fill_bgra_rect(
        buffer,
        width,
        height,
        82,
        172,
        132,
        184,
        (255, 231, 151, 255),
    );
    fill_bgra_rect(
        buffer,
        width,
        height,
        438,
        210,
        488,
        222,
        (255, 231, 151, 255),
    );
}

#[cfg(target_os = "windows")]
fn fill_bgra_rect(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    rgba: (u8, u8, u8, u8),
) {
    let left = left.clamp(0, width) as usize;
    let top = top.clamp(0, height) as usize;
    let right = right.clamp(0, width) as usize;
    let bottom = bottom.clamp(0, height) as usize;
    if left >= right || top >= bottom {
        return;
    }

    let (r, g, b, a) = rgba;
    let premultiply = |channel: u8| ((channel as u16 * a as u16 + 127) / 255) as u8;
    let pixel = [premultiply(b), premultiply(g), premultiply(r), a];
    let stride = width as usize * 4;

    for y in top..bottom {
        let row = y * stride;
        for x in left..right {
            let index = row + x * 4;
            buffer[index..index + 4].copy_from_slice(&pixel);
        }
    }
}

#[cfg(target_os = "windows")]
fn restore_gdi_text_alpha(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) {
    let left = left.clamp(0, width) as usize;
    let top = top.clamp(0, height) as usize;
    let right = right.clamp(0, width) as usize;
    let bottom = bottom.clamp(0, height) as usize;
    if left >= right || top >= bottom {
        return;
    }

    let stride = width as usize * 4;
    for y in top..bottom {
        let row = y * stride;
        for x in left..right {
            let pixel = row + x * 4;
            // GDI writes the glyph RGB into a 32-bit DIB but clears its alpha byte.
            if buffer[pixel + 3] == 0
                && (buffer[pixel] != 0 || buffer[pixel + 1] != 0 || buffer[pixel + 2] != 0)
            {
                buffer[pixel + 3] = 255;
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn updater_construction_frames() -> Option<&'static [UpdaterAnimationFrame]> {
    UPDATER_CONSTRUCTION_FRAMES
        .get_or_init(decode_updater_construction_frames)
        .as_deref()
}

#[cfg(target_os = "windows")]
fn decode_updater_construction_frames() -> Option<Vec<UpdaterAnimationFrame>> {
    let sheet = image::load_from_memory(updater_construction_frames_bytes())
        .ok()?
        .to_rgba8();
    let (sheet_width, sheet_height) = sheet.dimensions();
    if sheet_height != 240 || sheet_width < 560 || sheet_width % 560 != 0 {
        return None;
    }

    let frame_width = 560;
    let frame_height = sheet_height;
    let frame_count = sheet_width / frame_width;
    let mut decoded = Vec::with_capacity(frame_count as usize);

    for frame_index in 0..frame_count {
        let mut bgra = Vec::with_capacity((frame_width * frame_height * 4) as usize);
        let left = frame_index * frame_width;
        for y in 0..frame_height {
            for x in 0..frame_width {
                let [r, g, b, a] = sheet.get_pixel(left + x, y).0;
                let premultiply = |channel: u8| ((channel as u16 * a as u16 + 127) / 255) as u8;
                bgra.push(premultiply(b));
                bgra.push(premultiply(g));
                bgra.push(premultiply(r));
                bgra.push(a);
            }
        }

        decoded.push(UpdaterAnimationFrame {
            width: frame_width as i32,
            height: frame_height as i32,
            bgra,
        });
    }

    Some(decoded)
}

#[cfg(target_os = "windows")]
fn updater_construction_frames_bytes() -> &'static [u8] {
    include_bytes!("../assets/updater-construction-frames.png")
}

#[cfg(target_os = "windows")]
unsafe fn draw_text_styled(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    format: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
    size: i32,
    weight: i32,
    color: windows::Win32::Foundation::COLORREF,
) {
    use windows::Win32::Graphics::Gdi::{
        CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DeleteObject,
        FF_DONTCARE, NONANTIALIASED_QUALITY, OUT_DEFAULT_PRECIS, SelectObject,
    };
    use windows::core::PCWSTR;

    let face = to_wide_null(if text.is_ascii() {
        "Cascadia Mono"
    } else {
        "SimSun"
    });
    let font = unsafe {
        CreateFontW(
            -size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            NONANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            PCWSTR(face.as_ptr()),
        )
    };
    let previous_font = unsafe { SelectObject(hdc, font.into()) };
    let _ = unsafe { windows::Win32::Graphics::Gdi::SetTextColor(hdc, color) };
    unsafe { draw_text(hdc, text, left, top, right, bottom, format) };
    let _ = unsafe { SelectObject(hdc, previous_font) };
    let _ = unsafe { DeleteObject(font.into()) };
}

#[cfg(target_os = "windows")]
unsafe fn draw_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    format: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = windows::Win32::Foundation::RECT {
        left,
        top,
        right,
        bottom,
    };
    let _ = unsafe { windows::Win32::Graphics::Gdi::DrawTextW(hdc, &mut wide, &mut rect, format) };
}

#[cfg(target_os = "windows")]
fn wait_for_parent_exit(parent_pid: u32) {
    if parent_pid == 0 || parent_pid == std::process::id() {
        return;
    }

    for _ in 0..45 {
        if !is_pid_running(parent_pid) {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(target_os = "windows")]
fn is_pid_running(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let filter = format!("PID eq {}", pid);
    let Ok(output) = Command::new("tasklist")
        .args(["/FI", &filter, "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return false;
    };

    let text = String::from_utf8_lossy(&output.stdout);
    text.contains(&pid.to_string())
}

#[cfg(target_os = "windows")]
fn run_installer_and_wait(state: &UpdaterHelperState) -> Result<i32, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let installer = Path::new(&state.installer_path);
    if !installer.exists() {
        return Err(format!("Installer not found: {}", state.installer_path));
    }

    let status = match state.extension.as_str() {
        "msi" => Command::new("msiexec")
            .args(["/i", &state.installer_path, "/qn", "/norestart"])
            .creation_flags(CREATE_NO_WINDOW)
            .status(),
        "exe" => Command::new(&state.installer_path)
            .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"])
            .creation_flags(CREATE_NO_WINDOW)
            .status(),
        other => return Err(format!("Unsupported installer extension: {}", other)),
    }
    .map_err(|e| format!("start installer failed: {}", e))?;

    Ok(status.code().unwrap_or(-1))
}

#[cfg(target_os = "windows")]
fn write_updater_result(result_path: &str, result: &UpdaterHelperResult) -> Result<(), String> {
    let result_path = Path::new(result_path);
    if let Some(parent) = result_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create result dir failed: {}", e))?;
    }

    let content = serde_json::to_string_pretty(result)
        .map_err(|e| format!("serialize updater result failed: {}", e))?;
    fs::write(result_path, content).map_err(|e| format!("write updater result failed: {}", e))
}

#[cfg(target_os = "windows")]
fn restart_gui_with_update_result(gui_exe_path: &str, result_path: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new(gui_exe_path)
        .args([UPDATE_RESULT_ARG, result_path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("restart control panel failed: {}", e))?;

    Ok(())
}

/// 获取上次检查时间
fn get_last_check_time<R: Runtime>(app: &AppHandle<R>) -> u64 {
    preferences::last_check_time(app)
}

/// 保存上次检查时间
pub(crate) fn save_last_check_time<R: Runtime>(app: &AppHandle<R>) {
    preferences::save_last_check_time(app);
}

/// 获取是否包含预发布版本的偏好
pub(crate) fn get_include_prerelease<R: Runtime>(app: &AppHandle<R>) -> bool {
    preferences::include_prerelease(app)
}

/// 设置是否包含预发布版本的偏好
fn set_include_prerelease<R: Runtime>(app: &AppHandle<R>, include: bool) {
    preferences::set_include_prerelease(app, include);
}

/// Tauri命令：获取是否包含预发布版本的偏好
#[tauri::command]
pub fn get_include_prerelease_preference(app: AppHandle) -> bool {
    get_include_prerelease(&app)
}

/// Tauri命令：设置是否包含预发布版本的偏好
#[tauri::command]
pub fn set_include_prerelease_preference(app: AppHandle, include: bool) {
    set_include_prerelease(&app, include);
}

/// Tauri 命令：手动检查更新
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let include_prerelease = get_include_prerelease(&app);
    let result = check_for_updates_internal(true, include_prerelease).await;
    save_last_check_time(&app);
    result
}

/// Tauri 命令：按首页中用户明确选择的发布通道检查更新。
///
/// 该命令不修改侧栏 Beta 偏好；它只保证原生更新器展示的版本与用户点击的
/// 稳定版/预发布版卡片一致。
#[tauri::command]
pub async fn check_for_updates_for_channel(
    app: AppHandle,
    channel: String,
) -> Result<Option<UpdateInfo>, String> {
    let channel = match channel.as_str() {
        "stable" => ReleaseChannel::Stable,
        "prerelease" => ReleaseChannel::Prerelease,
        _ => return Err(format!("不支持的更新通道: {channel}")),
    };

    let result = check_for_updates_in_channel(true, channel).await;
    save_last_check_time(&app);
    result
}

/// 检查是否需要自动更新
fn should_auto_check(last_check_time: u64) -> bool {
    let current_time = get_current_timestamp();
    current_time.saturating_sub(last_check_time) > UPDATE_CHECK_INTERVAL
}

/// 处理自动检查结果（`manual=false`，`Some` 必定是新版本）
fn handle_auto_check_result<R: Runtime>(
    app: &AppHandle<R>,
    result: Result<Option<UpdateInfo>, String>,
) {
    match result {
        Ok(Some(update_info)) => {
            if !crate::desktop_settings::load_desktop_settings_from_disk().update_notify {
                debug!("Update notification disabled by desktop settings");
                save_last_check_time(app);
                return;
            }
            info!("🎉 发现新版本: {}", update_info.version);
            save_last_check_time(app);

            // 发送事件到前端
            for label in ["main", "desktop"] {
                if let Some(window) = app.get_webview_window(label) {
                    let _ = window.emit("update-available", &update_info);
                }
            }
        }
        Ok(None) => {
            debug!("✅ 已是最新版本");
            save_last_check_time(app);
        }
        Err(e) => {
            error!("❌ 检查更新失败: {}", e);
        }
    }
}

/// 启动时自动检查更新（如果距离上次检查超过4小时）
pub fn check_for_updates_on_startup<R: Runtime + 'static>(app: AppHandle<R>) {
    let last_check_time = get_last_check_time(&app);

    if !should_auto_check(last_check_time) {
        debug!("⏰ 距离上次检查时间未超过4小时，跳过自动检查");
        return;
    }

    debug!("⏰ 距离上次检查已超过4小时，自动检查更新...");
    let app_clone = app.clone();
    let include_prerelease = get_include_prerelease(&app);
    tauri::async_runtime::spawn(async move {
        let result = check_for_updates_internal(false, include_prerelease).await;
        handle_auto_check_result(&app_clone, result);
    });
}

// ========== 进程管理 ==========

/// 停止 Windows 服务
#[cfg(target_os = "windows")]
fn stop_service_with_command(service_name: &str, command: &str, args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    match std::process::Command::new(command)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(result) if result.status.success() => {
            info!("✅ 成功停止服务: {}", service_name);
            true
        }
        Ok(result) => {
            let error_msg = String::from_utf8_lossy(&result.stderr);
            warn!("⚠️ 停止服务失败 {}: {}", service_name, error_msg.trim());
            false
        }
        Err(e) => {
            warn!("⚠️ 执行命令失败 {}: {}", service_name, e);
            false
        }
    }
}

/// 停止 Windows 服务
#[cfg(target_os = "windows")]
fn stop_windows_service(service_name: &str) {
    // 尝试使用 net stop
    if stop_service_with_command(service_name, "net", &["stop", service_name]) {
        return;
    }

    // 尝试使用 sc stop 作为备选
    stop_service_with_command(service_name, "sc", &["stop", service_name]);
}

/// 强制结束进程
#[cfg(target_os = "windows")]
fn kill_process(process_name: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("taskkill")
        .args(&["/IM", process_name, "/F", "/T"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// 通过 HTTP API 关闭 Sunshine（不需要管理员权限）
#[allow(dead_code)]
async fn stop_sunshine_via_api() -> Result<(), String> {
    use crate::sunshine;

    let sunshine_url = sunshine::get_sunshine_url().await?;
    let boom_url = format!("{}/api/boom", sunshine_url.trim_end_matches('/'));

    info!("🌐 尝试通过 HTTP API 关闭 Sunshine: {}", boom_url);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    match client.get(&boom_url).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() || status.as_u16() == 200 {
                info!("✅ 已通过 HTTP API 请求关闭 Sunshine");
                Ok(())
            } else if status.as_u16() == 401 {
                Err("需要身份验证（401）".to_string())
            } else {
                Err(format!("HTTP API 返回错误状态码: {}", status))
            }
        }
        Err(e) => Err(format!("通过 HTTP API 关闭失败: {}", e)),
    }
}

/// 停止 Sunshine 服务（使用服务管理器）
#[cfg(target_os = "windows")]
fn stop_sunshine_service() {
    stop_windows_service("SunshineService");
    stop_windows_service("sunshineservice");
    std::thread::sleep(Duration::from_secs(2));
}

/// 强制关闭所有 Sunshine 进程
#[cfg(target_os = "windows")]
fn force_kill_sunshine_processes() {
    kill_process("sunshine.exe");

    let current_pid = std::process::id();
    let ps_script = format!(
        "Get-Process -Name '*sunshine*' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
        current_pid
    );

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    std::thread::sleep(Duration::from_secs(2));
}

/// 关闭 Sunshine 服务和残留进程
#[cfg(target_os = "windows")]
fn stop_sunshine_for_update() {
    info!("正在关闭 Sunshine 以安装更新...");
    stop_sunshine_service();
    force_kill_sunshine_processes();
    info!("Sunshine 服务和相关进程已为更新关闭。");
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
async fn stop_sunshine_and_gui() -> Result<(), String> {
    info!("🛑 正在关闭Sunshine和GUI进程...");

    // 首先尝试通过 HTTP API 关闭
    match stop_sunshine_via_api().await {
        Ok(_) => {
            std::thread::sleep(Duration::from_secs(3));
        }
        Err(e) => {
            warn!("⚠️ {}", e);
            info!("🔄 回退到使用服务管理器关闭...");
            stop_sunshine_service();
        }
    }

    // 强制结束所有进程
    force_kill_sunshine_processes();

    info!("✅ Sunshine和GUI进程已关闭");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
async fn stop_sunshine_and_gui() -> Result<(), String> {
    Err("此功能仅支持Windows".to_string())
}

// ========== 下载相关 ==========

fn validate_download_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("下载地址格式无效: {}", error))?;
    if parsed.scheme() != "https" {
        return Err("下载地址必须使用 HTTPS".to_string());
    }
    if parsed.host_str() != Some(GITHUB_RELEASE_HOST) {
        return Err(format!("下载地址必须来自 {}", GITHUB_RELEASE_HOST));
    }
    Ok(())
}

fn validate_download_filename(filename: &str) -> Result<(), String> {
    if filename.trim().is_empty() {
        return Err("下载文件名不能为空".to_string());
    }
    if filename.contains('/') || filename.contains('\\') {
        return Err("下载文件名不能包含路径分隔符".to_string());
    }

    let mut components = Path::new(filename).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err("下载文件名必须是单个普通文件名".to_string());
    }

    Ok(())
}

/// 发送下载进度事件到前端
fn emit_download_progress(
    window: &tauri::WebviewWindow,
    progress: u32,
    total: u64,
    downloaded: u64,
    phase: DownloadPhase,
    source: Option<&str>,
) {
    let _ = window.emit(
        "download-progress",
        serde_json::json!({
            "progress": progress,
            "total": total,
            "downloaded": downloaded,
            "phase": phase,
            "source": source
        }),
    );
}

fn emit_install_progress(
    app_handle: &AppHandle,
    stage: &str,
    detail: Option<&str>,
    terminal: bool,
) {
    let _ = app_handle.emit(
        "install-progress",
        serde_json::json!({
            "stage": stage,
            "detail": detail.unwrap_or(""),
            "terminal": terminal
        }),
    );
}

fn remove_file_if_exists(
    path: &Path,
    error_code: DownloadErrorCode,
    action: &str,
) -> Result<(), DownloadCommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DownloadCommandError::new(
            error_code,
            format!("{}失败: {}", action, error),
        )),
    }
}

async fn finalize_download_file(
    partial_path: &Path,
    file_path: &Path,
) -> Result<(), DownloadCommandError> {
    remove_file_if_exists(
        file_path,
        DownloadErrorCode::FileFinalizationFailed,
        "替换旧安装包",
    )?;
    tokio::fs::rename(partial_path, file_path)
        .await
        .map_err(|error| {
            DownloadCommandError::new(
                DownloadErrorCode::FileFinalizationFailed,
                format!("完成下载文件失败: {}", error),
            )
        })
}

/// 下载更新文件（带真实进度报告）
#[tauri::command]
pub async fn download_update(
    url: String,
    filename: String,
    expected_size: Option<u64>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, DownloadCommandError> {
    validate_download_url(&url).map_err(|detail| {
        DownloadCommandError::new(DownloadErrorCode::FilePreparationFailed, detail)
    })?;
    validate_download_filename(&filename).map_err(|detail| {
        DownloadCommandError::new(DownloadErrorCode::FilePreparationFailed, detail)
    })?;

    info!("📥 开始下载更新: {}", filename);

    cleanup_old_installers();

    let download_dir = std::env::temp_dir();
    let file_path = download_dir.join(&filename);
    let partial_path = download_dir.join(format!("{}.part", filename));
    let window = app_handle.get_webview_window("main");

    remove_file_if_exists(
        &partial_path,
        DownloadErrorCode::FilePreparationFailed,
        "清理旧的临时下载文件",
    )?;

    let mut last_progress_percent = 0;
    let outcome = github_download::download_to_file_with_fallbacks(
        DownloadRequest {
            url: &url,
            destination: &partial_path,
            expected_size: expected_size.filter(|size| *size > 0),
            max_size: expected_size.filter(|size| *size > 0),
            user_agent: "Sunshine-Control-Panel updater",
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            overall_timeout: None,
        },
        |event| {
            let Some(win) = window.as_ref() else {
                return;
            };
            match event.phase {
                DownloadAttemptPhase::Connecting => {
                    last_progress_percent = 0;
                    emit_download_progress(win, 0, event.total, 0, DownloadPhase::Connecting, None);
                }
                DownloadAttemptPhase::Retrying => {
                    last_progress_percent = 0;
                    emit_download_progress(win, 0, event.total, 0, DownloadPhase::Retrying, None);
                }
                DownloadAttemptPhase::Downloading => {
                    let progress = if event.total == 0 {
                        0
                    } else {
                        (event.downloaded.saturating_mul(100) / event.total).min(99) as u32
                    };
                    if event.total == 0 || progress > last_progress_percent || event.downloaded == 0
                    {
                        last_progress_percent = progress;
                        emit_download_progress(
                            win,
                            progress,
                            event.total,
                            event.downloaded,
                            DownloadPhase::Downloading,
                            Some(event.source),
                        );
                    }
                }
            }
        },
    )
    .await
    .map_err(|error| {
        let code = if error.is_setup() {
            DownloadErrorCode::SetupFailed
        } else {
            DownloadErrorCode::SourcesExhausted
        };
        DownloadCommandError::new(code, error.to_string())
    })?;

    if let Some(ref win) = window {
        emit_download_progress(
            win,
            99,
            outcome.total,
            outcome.downloaded,
            DownloadPhase::Verifying,
            Some(outcome.source),
        );
    }
    if let Err(error) = finalize_download_file(&partial_path, &file_path).await {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(error);
    }

    info!(
        "✅ 下载完成: {} bytes，来源: {}",
        outcome.downloaded, outcome.source
    );
    if let Some(win) = window {
        emit_download_progress(
            &win,
            100,
            outcome.total,
            outcome.downloaded,
            DownloadPhase::Complete,
            Some(outcome.source),
        );
    }
    Ok(serde_json::json!({
        "success": true,
        "file_path": file_path.to_string_lossy().to_string(),
        "source": outcome.source
    }))
}

#[cfg(test)]
mod download_source_tests {
    use super::*;

    #[test]
    fn download_url_requires_https_and_the_github_release_host() {
        assert!(
            validate_download_url(
                "https://github.com/qiin2333/sunshine/releases/download/v1/setup.exe"
            )
            .is_ok()
        );

        for invalid_url in [
            "http://github.com/qiin2333/sunshine/releases/download/v1/setup.exe",
            "https://github.com.example.invalid/releases/download/v1/setup.exe",
            "https://example.invalid/qiin2333/sunshine/releases/download/v1/setup.exe",
            "not a url",
        ] {
            assert!(
                validate_download_url(invalid_url).is_err(),
                "unexpectedly accepted {invalid_url}"
            );
        }
    }

    #[test]
    fn download_filename_must_be_a_single_normal_component() {
        for valid_filename in ["Sunshine-Windows-1.2.3.exe", "archive..exe"] {
            assert!(
                validate_download_filename(valid_filename).is_ok(),
                "unexpectedly rejected {valid_filename}"
            );
        }

        for invalid_filename in [
            "",
            "   ",
            ".",
            "..",
            "../setup.exe",
            "..\\setup.exe",
            "folder/setup.exe",
            "folder\\setup.exe",
        ] {
            assert!(
                validate_download_filename(invalid_filename).is_err(),
                "unexpectedly accepted {invalid_filename}"
            );
        }
    }

    #[test]
    fn download_errors_expose_stable_language_neutral_codes() {
        let error = DownloadCommandError::new(
            DownloadErrorCode::SourcesExhausted,
            "diagnostic details stay out of the localized UI",
        );
        let serialized = serde_json::to_value(error).unwrap();

        assert_eq!(serialized["code"], "sources_exhausted");
        assert_eq!(
            serialized["detail"],
            "diagnostic details stay out of the localized UI"
        );
    }
}

// ========== 安装相关 ==========

#[cfg(target_os = "windows")]
fn create_updater_work_dir() -> Result<PathBuf, String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let temp_dir = std::env::temp_dir();
    for attempt in 0..100 {
        let work_dir = temp_dir.join(format!(
            "sunshine-updater-{}-{}-{}",
            now_ms,
            std::process::id(),
            attempt
        ));
        match fs::create_dir(&work_dir) {
            Ok(()) => return Ok(work_dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("create updater dir failed: {}", e)),
        }
    }

    Err("create updater dir failed: too many name collisions".to_string())
}

#[cfg(target_os = "windows")]
fn prepare_updater_helper(
    file_path: &str,
    extension: &str,
    target_version: Option<String>,
) -> Result<(PathBuf, PathBuf), String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("get current exe failed: {}", e))?;
    let work_dir = create_updater_work_dir()?;

    let helper_exe = work_dir.join("sunshine-updater-helper.exe");
    fs::copy(&current_exe, &helper_exe)
        .map_err(|e| format!("copy updater helper failed: {}", e))?;

    let state_path = work_dir.join("state.json");
    let result_path = work_dir.join("result.json");
    let state = UpdaterHelperState {
        installer_path: file_path.to_string(),
        extension: extension.to_string(),
        target_version,
        gui_exe_path: current_exe.to_string_lossy().to_string(),
        result_path: result_path.to_string_lossy().to_string(),
        parent_pid: std::process::id(),
    };

    let state_content = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("serialize updater state failed: {}", e))?;
    fs::write(&state_path, state_content)
        .map_err(|e| format!("write updater state failed: {}", e))?;

    Ok((helper_exe, state_path))
}

#[cfg(target_os = "windows")]
fn launch_updater_helper(helper_exe: &Path, state_path: &Path) -> Result<(), String> {
    launch_updater_helper_elevated(helper_exe, state_path)
}

#[cfg(target_os = "windows")]
fn launch_updater_helper_elevated(helper_exe: &Path, state_path: &Path) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let verb = to_wide_null("runas");
    let file = to_wide_null(&helper_exe.to_string_lossy());
    let params = to_wide_null(&format!(
        "{} \"{}\"",
        UPDATER_HELPER_ARG,
        state_path.to_string_lossy()
    ));
    let directory = helper_exe
        .parent()
        .map(|path| to_wide_null(&path.to_string_lossy()))
        .unwrap_or_else(|| to_wide_null(""));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        hwnd: HWND(std::ptr::null_mut()),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        // Keep the pixel progress helper visible after UAC approval.
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|e| format!("launch updater helper elevated failed: {}", e))
}

/// 安装更新文件
#[tauri::command]
pub async fn install_update(
    file_path: String,
    target_version: Option<String>,
    app_handle: AppHandle,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        info!("🔧 开始安装更新: {}", file_path);

        // 先启动更新助手；如果用户取消 UAC，串流仍然保持运行。
        emit_install_progress(&app_handle, "preparing", None, false);
        // 检查文件扩展名
        let path = std::path::Path::new(&file_path);
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(extension.as_str(), "msi" | "exe") {
            let error = format!("Unsupported installer extension: {}", extension);
            emit_install_progress(&app_handle, "failed", Some(&error), true);
            return Err(error);
        }

        emit_install_progress(&app_handle, "building-command", None, false);
        let (helper_exe, state_path) =
            match prepare_updater_helper(&file_path, &extension, target_version) {
                Ok(paths) => paths,
                Err(e) => {
                    emit_install_progress(&app_handle, "failed", Some(&e), true);
                    return Err(e);
                }
            };

        emit_install_progress(&app_handle, "launching-installer", None, false);
        if let Err(e) = launch_updater_helper(&helper_exe, &state_path) {
            emit_install_progress(&app_handle, "failed", Some(&e), true);
            return Err(e);
        }
        emit_install_progress(&app_handle, "installer-started", None, false);

        info!("✅ 安装程序已静默启动，正在安装...");

        // 延迟后退出当前GUI进程
        let app_clone = app_handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            emit_install_progress(&app_clone, "app-exiting", None, false);
            tokio::time::sleep(Duration::from_secs(1)).await;
            app_clone.exit(0);
        });

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持Windows".to_string())
    }
}

/// 清理临时目录中的旧安装包
fn cleanup_old_installers() {
    let temp_dir = std::env::temp_dir();

    info!("🧹 检查并清理临时目录中的旧安装包...");

    let entries = match std::fs::read_dir(&temp_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("⚠️ 无法读取临时目录: {}", e);
            return;
        }
    };

    let cleaned_count = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let ext = path.extension()?.to_str()?.to_lowercase();

            if !matches!(ext.as_str(), "msi" | "exe") {
                return None;
            }

            let file_name = path.file_name()?.to_str()?;
            let file_name_lower = file_name.to_lowercase();

            // 检查是否包含 sunshine 相关关键词
            let is_sunshine_installer =
                file_name_lower.contains("sunshine") || file_name_lower.starts_with("sunshine-");

            if !is_sunshine_installer {
                return None;
            }

            match std::fs::remove_file(&path) {
                Ok(_) => {
                    info!("✅ 已删除旧安装包: {}", file_name);
                    Some(())
                }
                Err(e) => {
                    debug!("⚠️ 无法删除 {}: {} (可能正在使用中)", file_name, e);
                    None
                }
            }
        })
        .count();

    if cleaned_count > 0 {
        info!("✅ 清理完成，共删除 {} 个旧安装包", cleaned_count);
    } else {
        debug!("✅ 未发现需要清理的旧安装包");
    }
}

/// 初始化更新偏好状态。
///
/// 必须在任何窗口或异步更新任务启动前调用，不能依赖主 WebView 是否在启动时创建。
pub fn init_update_preferences(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    preferences::init(app)
}

/// 启动清理和自动更新检查；偏好状态由 `init_update_preferences` 独立初始化。
pub fn start_update_checker<R: Runtime + 'static>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    if UPDATE_CHECKER_STARTED.set(()).is_err() {
        debug!("更新检查器已启动，跳过重复初始化");
        return Ok(());
    }

    // 在启动时清理旧的安装包（在检查更新之前）
    cleanup_old_installers();

    // 延迟自动检查更新：主要是等待主窗口/前端就绪（偏好已在后端启动时加载）
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        check_for_updates_on_startup(app_clone);
    });

    Ok(())
}

/// 主窗口按需创建并完成事件监听后启动自动检查。
#[tauri::command]
pub fn start_update_checker_when_ui_ready(app: tauri::AppHandle) -> Result<(), String> {
    start_update_checker(&app).map_err(|error| error.to_string())
}
