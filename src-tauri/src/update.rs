use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tauri::{AppHandle, Manager, Emitter, Runtime};
use log::{info, warn, error, debug};

// ========== 常量定义 ==========
const GITHUB_API_URL: &str = "https://api.github.com/repos/qiin2333/sunshine/releases";
const GITHUB_API_URL_LATEST: &str = "https://api.github.com/repos/qiin2333/sunshine/releases/latest";
const UPDATE_CHECK_INTERVAL: u64 = 4 * 60 * 60; // 4小时（秒）
const HTTP_TIMEOUT_SECS: u64 = 3;
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
const MAX_RETRY_ATTEMPTS: usize = 4;
const MAX_RELEASES_TO_CHECK: usize = 10; // 最多检查的发布数量

// GitHub API 加速代理列表（按优先级排序）
const API_PROXY_PREFIXES: &[&str] = &[
    "https://ghapi.hackhub.cn/",
    "https://mirror.ghproxy.com/",
];

// GitHub 下载加速代理列表
const DOWNLOAD_PROXY_PREFIXES: &[&str] = &[
    "https://ghfast.top/",
    "https://ghproxy.com/",
    "https://mirror.ghproxy.com/",
];

// ========== 数据结构定义 ==========

/// 更新信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: Option<String>,
    pub download_name: Option<String>,
    pub release_page: String,
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
}

/// 更新检查偏好设置
#[derive(Default)]
pub struct UpdatePreferences {
    pub last_check_time: u64,
    pub include_prerelease: bool,
}

// ========== 版本相关 ==========

/// 获取当前 Sunshine 版本
async fn get_current_sunshine_version() -> Result<String, String> {
    use crate::sunshine;
    sunshine::get_sunshine_version().await
}

/// 规范化版本号（移除 v/V 前缀）
fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').trim_start_matches('V').to_string()
}

/// 比较版本号，判断是否有新版本
fn is_new_version_available(current: &str, latest: &str) -> bool {
    let current = normalize_version(current);
    let latest = normalize_version(latest);
    
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
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

/// 查找最适合的下载资源（优先Windows安装包）
fn find_best_download_asset(assets: &[GitHubAsset]) -> (Option<String>, Option<String>) {
    // 优先选择Windows安装包
    for asset in assets {
        let name = asset.name.to_lowercase();
        if name.contains("windows") || name.ends_with(".msi") || name.ends_with(".exe") {
            return (
                Some(asset.browser_download_url.clone()),
                Some(asset.name.clone()),
            );
        }
    }
    
    // 如果没找到Windows安装包，选择第一个可用文件
    if let Some(asset) = assets.first() {
        (
            Some(asset.browser_download_url.clone()),
            Some(asset.name.clone()),
        )
    } else {
        (None, None)
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

/// 创建 HTTP 客户端
fn create_http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
}

/// 尝试单个 URL 请求
async fn try_single_request(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
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

/// 使用代理获取 HTTP 响应
async fn fetch_with_proxies(
    urls: &[String],
    max_attempts: usize,
    timeout_secs: u64,
) -> Result<reqwest::Response, String> {
    let client = create_http_client(timeout_secs)?;

    for url in urls.iter().take(max_attempts) {
        match try_single_request(&client, url).await {
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

    let response = fetch_with_proxies(&urls_to_try, MAX_RETRY_ATTEMPTS, HTTP_TIMEOUT_SECS).await?;
    
    response
        .text()
        .await
        .map_err(|e| format!("读取响应内容失败: {}", e))
}

/// 获取所有发布版本（包括预发布）
async fn fetch_all_releases() -> Result<Vec<GitHubRelease>, String> {
    let json = http_get_with_proxies(GITHUB_API_URL).await?;
    
    let releases: Vec<GitHubRelease> = serde_json::from_str(&json)
        .map_err(|e| format!("解析GitHub API响应失败: {}", e))?;
    
    Ok(releases)
}

/// 获取最新稳定版本
async fn fetch_latest_stable_release() -> Result<GitHubRelease, String> {
    let json = http_get_with_proxies(GITHUB_API_URL_LATEST).await?;
    
    let release: GitHubRelease = serde_json::from_str(&json)
        .map_err(|e| format!("解析GitHub API响应失败: {}", e))?;
    
    Ok(release)
}

/// 查找最新的可用发布版本（包括预发布）
fn find_latest_release(releases: &[GitHubRelease], include_prerelease: bool) -> Option<&GitHubRelease> {
    for release in releases.iter().take(MAX_RELEASES_TO_CHECK) {
        // 跳过草稿版本
        if release.draft {
            continue;
        }
        
        // 如果包含预发布版本，返回第一个（已按时间排序）
        if include_prerelease {
            return Some(release);
        }
        
        // 如果不包含预发布版本，只返回稳定版本
        if !release.prerelease {
            return Some(release);
        }
    }
    
    None
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

/// 创建更新信息
fn create_update_info(release: &GitHubRelease) -> UpdateInfo {
    let (download_url, download_name) = find_best_download_asset(&release.assets);
    
    UpdateInfo {
        version: release.tag_name.clone(),
        release_notes: release.body.clone(),
        download_url,
        download_name,
        release_page: release.html_url.clone(),
    }
}

/// 检查更新（内部函数）
pub async fn check_for_updates_internal(show_notification: bool, include_prerelease: bool) -> Result<Option<UpdateInfo>, String> {
    info!("🔍 开始检查更新... (包含预发布版本: {})", include_prerelease);
    
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
    let release = find_latest_release(&releases, include_prerelease)
        .ok_or_else(|| "未找到可用的发布版本".to_string())?;
    
    let latest_version = normalize_version(&release.tag_name);
    
    info!("📊 当前 Sunshine 版本: {}, 最新版本: {} ({})", 
        current_version, 
        latest_version,
        if release.prerelease { "预发布" } else { "稳定版" }
    );
    
    if !is_new_version_available(&current_version, &latest_version) {
        if show_notification {
            return Err("已是最新版本".to_string());
        }
        return Ok(None);
    }
    
    let update_info = create_update_info(release);
    Ok(Some(update_info))
}

// ========== 偏好设置管理 ==========

/// 获取当前时间戳（秒）
fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// 获取上次检查时间
fn get_last_check_time(app: &AppHandle) -> u64 {
    app.try_state::<Arc<Mutex<UpdatePreferences>>>()
        .map(|prefs| prefs.lock().unwrap().last_check_time)
        .unwrap_or(0)
}

/// 保存上次检查时间
fn save_last_check_time(app: &AppHandle) {
    if let Some(prefs) = app.try_state::<Arc<Mutex<UpdatePreferences>>>() {
        let mut prefs = prefs.lock().unwrap();
        prefs.last_check_time = get_current_timestamp();
    }
}

/// 获取是否包含预发布版本的偏好
pub(crate) fn get_include_prerelease<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Arc<Mutex<UpdatePreferences>>>()
        .map(|prefs| prefs.lock().unwrap().include_prerelease)
        .unwrap_or(false)
}

/// 设置是否包含预发布版本的偏好
fn set_include_prerelease<R: Runtime>(app: &AppHandle<R>, include: bool) {
    if let Some(prefs) = app.try_state::<Arc<Mutex<UpdatePreferences>>>() {
        let mut prefs = prefs.lock().unwrap();
        prefs.include_prerelease = include;
        info!("📝 更新偏好设置: 包含预发布版本 = {}", include);
    }
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

/// Tauri命令：手动检查更新
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let include_prerelease = get_include_prerelease(&app);
    let result = check_for_updates_internal(true, include_prerelease).await;
    save_last_check_time(&app);
    
    match result {
        Ok(Some(info)) => Ok(Some(info)),
        Ok(None) => Err("已是最新版本".to_string()),
        Err(e) => Err(e),
    }
}

/// 检查是否需要自动更新
fn should_auto_check(last_check_time: u64) -> bool {
    let current_time = get_current_timestamp();
    current_time.saturating_sub(last_check_time) > UPDATE_CHECK_INTERVAL
}

/// 处理自动检查结果
fn handle_auto_check_result(app: &AppHandle, result: Result<Option<UpdateInfo>, String>) {
    match result {
        Ok(Some(update_info)) => {
            info!("🎉 发现新版本: {}", update_info.version);
            save_last_check_time(app);
            
            // 发送事件到前端
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit("update-available", &update_info);
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
pub fn check_for_updates_on_startup(app: AppHandle) {
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
    let _ = std::process::Command::new("taskkill")
        .args(&["/IM", process_name, "/F", "/T"])
        .output();
}

/// 通过 HTTP API 关闭 Sunshine（不需要管理员权限）
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
        Err(e) => {
            Err(format!("通过 HTTP API 关闭失败: {}", e))
        }
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
    
    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .output();
    
    std::thread::sleep(Duration::from_secs(2));
}

/// 关闭Sunshine和GUI进程
#[cfg(target_os = "windows")]
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

/// 解析 GitHub release 下载链接
fn parse_github_release_download_url(url: &str) -> Option<(String, String, String, String)> {
    const GITHUB_PREFIX: &str = "https://github.com/";
    
    if !url.starts_with(GITHUB_PREFIX) {
        return None;
    }
    
    let rest = &url[GITHUB_PREFIX.len()..];
    let mut parts = rest.split('/');
    
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    
    if parts.next()? != "releases" || parts.next()? != "download" {
        return None;
    }
    
    let tag = parts.next()?.to_string();
    let filename = parts.collect::<Vec<_>>().join("/");
    
    if filename.is_empty() {
        return None;
    }
    
    Some((owner, repo, tag, filename))
}

/// 构建 jsDelivr CDN URL
fn build_jsdelivr_url(owner: &str, repo: &str, tag: &str, filename: &str) -> String {
    format!("https://cdn.jsdelivr.net/gh/{}/{}@{}/{}", owner, repo, tag, filename)
}

/// 构建下载 URL 列表
fn build_download_urls(original_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    
    // 优先尝试 jsDelivr CDN
    if let Some((owner, repo, tag, filename)) = parse_github_release_download_url(original_url) {
        urls.push(build_jsdelivr_url(&owner, &repo, &tag, &filename));
    }
    
    // 添加其他代理
    for proxy in DOWNLOAD_PROXY_PREFIXES {
        urls.push(format!("{}{}", proxy, original_url));
    }
    
    // 最后添加直连
    urls.push(original_url.to_string());
    
    urls
}

/// 发送下载进度事件到前端
fn emit_download_progress(
    window: &tauri::WebviewWindow,
    progress: u32,
    total: u64,
    downloaded: u64,
) {
    let _ = window.emit("download-progress", serde_json::json!({
        "progress": progress,
        "total": total,
        "downloaded": downloaded
    }));
}

/// 处理下载流
async fn download_stream(
    mut stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    file: &mut std::fs::File,
    total_size: u64,
    window: Option<&tauri::WebviewWindow>,
) -> Result<u64, String> {
    use std::io::Write;
    use futures_util::StreamExt;
    
    let mut downloaded: u64 = 0;
    let mut last_progress_percent: u32 = 0;
    
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("读取数据块失败: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;
        
        // 更新进度
        if total_size > 0 {
            let progress_percent = (downloaded * 100 / total_size) as u32;
            
            if progress_percent > last_progress_percent 
                || progress_percent >= 100 
                || downloaded == total_size 
            {
                last_progress_percent = progress_percent;
                
                if let Some(win) = window {
                    emit_download_progress(win, progress_percent, total_size, downloaded);
                }
                
                debug!("📊 下载进度: {}% ({}/{})", progress_percent, downloaded, total_size);
            }
        } else if let Some(win) = window {
            emit_download_progress(win, 0, 0, downloaded);
        }
    }
    
    Ok(downloaded)
}

/// 下载更新文件（带真实进度报告）
#[tauri::command]
pub async fn download_update(
    url: String,
    filename: String,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    info!("📥 开始下载更新: {}", filename);

    cleanup_old_installers();

    let download_dir = std::env::temp_dir();
    let file_path = download_dir.join(&filename);

    let urls_to_try = build_download_urls(&url);
    let response = fetch_with_proxies(&urls_to_try, MAX_RETRY_ATTEMPTS, DOWNLOAD_TIMEOUT_SECS).await?;

    let total_size = response.content_length().unwrap_or(0);
    debug!("📊 文件大小: {} bytes", total_size);

    let window = app_handle.get_webview_window("main");
    
    // 发送初始进度
    if let Some(ref win) = window {
        emit_download_progress(win, 0, total_size, 0);
    }

    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let stream = response.bytes_stream();
    let downloaded = download_stream(stream, &mut file, total_size, window.as_ref()).await?;

    info!("✅ 下载完成: {} bytes", downloaded);

    // 发送完成事件
    if let Some(win) = window {
        emit_download_progress(&win, 100, total_size, downloaded);
    }

    Ok(serde_json::json!({
        "success": true,
        "file_path": file_path.to_string_lossy().to_string(),
        "message": "下载完成"
    }))
}

// ========== 安装相关 ==========

/// 构建安装命令参数
#[cfg(target_os = "windows")]
fn build_install_command(file_path: &str, extension: &str) -> Result<String, String> {
    let escaped_path = file_path.replace("'", "''");
    
    match extension {
        "msi" => {
            // MSI 安装包：使用 /qn 完全静默安装
            Ok(format!(
                "Start-Process msiexec -ArgumentList '/i', '{}', '/qn', '/norestart' -Wait",
                escaped_path
            ))
        }
        "exe" => {
            // EXE 安装包：尝试多种静默参数
            Ok(format!(
                "Start-Process '{}' -ArgumentList '/VERYSILENT', '/SILENT', '/S', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-' -Wait",
                escaped_path
            ))
        }
        _ => Err(format!("不支持的安装包格式: {}", extension)),
    }
}

/// 启动安装程序
#[cfg(target_os = "windows")]
fn launch_installer(install_args: &str) -> Result<(), String> {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
    
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    Command::new("powershell")
        .args(&["-NoProfile", "-WindowStyle", "Hidden", "-Command", install_args])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动安装程序失败: {}", e))?;
    
    Ok(())
}

/// 安装更新文件
#[tauri::command]
pub async fn install_update(file_path: String, app_handle: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        info!("🔧 开始安装更新: {}", file_path);
        
        // 先关闭Sunshine和GUI
        stop_sunshine_and_gui().await?;
        
        // 检查文件扩展名
        let path = std::path::Path::new(&file_path);
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // 构建并启动安装命令
        let install_args = build_install_command(&file_path, &extension)?;
        launch_installer(&install_args)?;
        
        info!("✅ 安装程序已静默启动，正在安装...");
        
        // 延迟后退出当前GUI进程
        let app_clone = app_handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            info!("🚪 退出GUI进程，等待安装完成...");
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
            let is_sunshine_installer = file_name_lower.contains("sunshine")
                || file_name_lower.starts_with("sunshine-");
            
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

/// 初始化更新检查模块
pub fn init_update_checker(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let prefs = Arc::new(Mutex::new(UpdatePreferences::default()));
    app.manage(prefs);
    
    // 在启动时清理旧的安装包（在检查更新之前）
    cleanup_old_installers();
    
    // 延迟自动检查更新，等待前端初始化偏好设置
    // 前端会在 onMounted 时从 localStorage 读取偏好并同步到后端
    // 延迟 2 秒，给前端足够的时间初始化偏好设置
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        check_for_updates_on_startup(app_clone);
    });
    
    Ok(())
}
