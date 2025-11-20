use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tauri::{AppHandle, Manager, Emitter};

// ========== 常量定义 ==========
const GITHUB_API_URL: &str = "https://api.github.com/repos/qiin2333/sunshine/releases/latest";
const UPDATE_CHECK_INTERVAL: u64 = 4 * 60 * 60; // 4小时（秒）
const HTTP_TIMEOUT_SECS: u64 = 3;
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
const MAX_RETRY_ATTEMPTS: usize = 4;
const PROGRESS_UPDATE_THRESHOLD: u32 = 1; // 进度更新阈值（百分比）

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
}

/// 下载进度信息
#[derive(Debug, Serialize, Deserialize)]
struct DownloadProgress {
    progress: u32,
    total: u64,
    downloaded: u64,
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

/// 使用代理获取 HTTP 响应
async fn fetch_with_proxies(
    urls: &[String],
    max_attempts: usize,
    timeout_secs: u64,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    for url in urls.iter().take(max_attempts) {
        match client
            .get(url)
            .header("User-Agent", "Sunshine-Control-Panel")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    println!("✅ 请求成功，来源: {}", url);
                    return Ok(response);
                } else {
                    eprintln!(
                        "⚠️ HTTP状态码 {}: {}",
                        response.status().as_u16(),
                        url
                    );
                }
            }
            Err(e) => {
                eprintln!("⚠️ 请求失败: {} - {}", url, e);
            }
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

/// 检查更新（内部函数）
pub async fn check_for_updates_internal(show_notification: bool) -> Result<Option<UpdateInfo>, String> {
    println!("🔍 开始检查更新...");
    
    let json = http_get_with_proxies(GITHUB_API_URL).await?;
    
    let release: GitHubRelease = serde_json::from_str(&json)
        .map_err(|e| format!("解析GitHub API响应失败: {}", e))?;
    
    // 获取当前 Sunshine 版本
    let current_version = match get_current_sunshine_version().await {
        Ok(ver) => normalize_version(&ver),
        Err(e) => {
            eprintln!("⚠️ 获取 Sunshine 版本失败: {}, 使用默认版本 0.0.0", e);
            "0.0.0".to_string()
        }
    };
    let latest_version = normalize_version(&release.tag_name);
    
    println!("📊 当前 Sunshine 版本: {}, 最新版本: {}", current_version, latest_version);
    
    if !is_new_version_available(&current_version, &latest_version) {
        if show_notification {
            return Err("已是最新版本".to_string());
        }
        return Ok(None);
    }
    
    // 查找适合的下载文件（优先选择Windows安装包）
    let (download_url, download_name) = find_best_download_asset(&release.assets);
    
    let update_info = UpdateInfo {
        version: release.tag_name.clone(),
        release_notes: release.body.clone(),
        download_url,
        download_name,
        release_page: release.html_url.clone(),
    };
    
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

/// Tauri命令：手动检查更新
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    match check_for_updates_internal(true).await {
        Ok(Some(info)) => {
            save_last_check_time(&app);
            Ok(Some(info))
        }
        Ok(None) => {
            save_last_check_time(&app);
            Err("已是最新版本".to_string())
        }
        Err(e) => {
            save_last_check_time(&app);
            Err(e)
        }
    }
}


/// 启动时自动检查更新（如果距离上次检查超过4小时）
pub fn check_for_updates_on_startup(app: AppHandle) {
    let last_check_time = get_last_check_time(&app);
    let current_time = get_current_timestamp();
    
    if current_time.saturating_sub(last_check_time) > UPDATE_CHECK_INTERVAL {
        println!("⏰ 距离上次检查已超过4小时，自动检查更新...");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            match check_for_updates_internal(false).await {
                Ok(Some(update_info)) => {
                    println!("🎉 发现新版本: {}", update_info.version);
                    save_last_check_time(&app_clone);
                    
                    // 发送事件到前端，让前端显示更新通知
                    if let Some(window) = app_clone.get_webview_window("main") {
                        let _ = window.emit("update-available", &update_info);
                    }
                }
                Ok(None) => {
                    println!("✅ 已是最新版本");
                    save_last_check_time(&app_clone);
                }
                Err(e) => {
                    eprintln!("❌ 检查更新失败: {}", e);
                }
            }
        });
    } else {
        println!("⏰ 距离上次检查时间未超过4小时，跳过自动检查");
    }
}

// ========== 进程管理 ==========

/// 停止 Windows 服务
#[cfg(target_os = "windows")]
fn stop_windows_service(service_name: &str) {
    let _ = std::process::Command::new("net")
        .args(&["stop", service_name])
        .output();
}

/// 强制结束进程
#[cfg(target_os = "windows")]
fn kill_process(process_name: &str) {
    let _ = std::process::Command::new("taskkill")
        .args(&["/IM", process_name, "/F", "/T"])
        .output();
}

/// 关闭Sunshine和GUI进程
#[cfg(target_os = "windows")]
fn stop_sunshine_and_gui() -> Result<(), String> {
    println!("🛑 正在关闭Sunshine和GUI进程...");
    
    // 停止Sunshine服务（新旧服务名都尝试）
    stop_windows_service("SunshineService");
    stop_windows_service("sunshineservice");
    
    // 等待服务停止
    std::thread::sleep(Duration::from_secs(1));
    
    // 强制结束所有Sunshine进程
    kill_process("sunshine.exe");
    
    // 获取当前进程ID，避免关闭自己
    let current_pid = std::process::id();
    
    // 使用PowerShell安全地关闭其他GUI进程
    let ps_script = format!(
        "Get-Process -Name '*sunshine*' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
        current_pid
    );
    
    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .output();
    
    // 等待进程完全关闭
    std::thread::sleep(Duration::from_secs(2));
    
    println!("✅ Sunshine和GUI进程已关闭");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn stop_sunshine_and_gui() -> Result<(), String> {
    Err("此功能仅支持Windows".to_string())
}

/// 下载更新文件（带真实进度报告）
#[tauri::command]
pub async fn download_update(
    url: String,
    filename: String,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    use std::io::Write;
    use futures_util::StreamExt;

    println!("📥 开始下载更新: {}", filename);

    // 获取下载目录
    let download_dir = std::env::temp_dir();
    let file_path = download_dir.join(&filename);

    // 构建下载 URL 列表（包含代理和直连）
    let urls_to_try = build_download_urls(&url);

    // 尝试下载
    let response = fetch_with_proxies(&urls_to_try, MAX_RETRY_ATTEMPTS, DOWNLOAD_TIMEOUT_SECS)
        .await?;

    // 获取文件大小
    let total_size = response.content_length().unwrap_or(0);
    println!("📊 文件大小: {} bytes", total_size);

    // 发送初始进度事件
    if let Some(window) = app_handle.get_webview_window("main") {
        emit_download_progress(&window, 0, total_size, 0);
    }

    // 创建文件并流式下载
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_progress_percent: u32 = 0;

    // 流式下载并实时报告进度
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("读取数据块失败: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;

        // 计算并更新进度
        if total_size > 0 {
            let progress_percent = (downloaded * 100 / total_size) as u32;

            // 只在进度变化超过阈值时发送事件
            if progress_percent > last_progress_percent 
                || progress_percent >= 100 
                || downloaded == total_size 
            {
                last_progress_percent = progress_percent;

                if let Some(window) = app_handle.get_webview_window("main") {
                    emit_download_progress(&window, progress_percent, total_size, downloaded);
                }

                println!("📊 下载进度: {}% ({}/{})", progress_percent, downloaded, total_size);
            }
        } else {
            // 无法获取总大小时，至少报告已下载的字节数
            if let Some(window) = app_handle.get_webview_window("main") {
                emit_download_progress(&window, 0, 0, downloaded);
            }
        }
    }

    println!("✅ 下载完成: {} bytes", downloaded);

    // 发送完成事件
    if let Some(window) = app_handle.get_webview_window("main") {
        emit_download_progress(&window, 100, total_size, downloaded);
    }

    Ok(serde_json::json!({
        "success": true,
        "file_path": file_path.to_string_lossy().to_string(),
        "message": "下载完成"
    }))
}

// ========== 下载相关 ==========

/// 解析 GitHub release 下载链接，提取 owner、repo、tag、filename
/// 
/// 输入格式: `https://github.com/OWNER/REPO/releases/download/TAG/FILENAME`
/// 返回: `(owner, repo, tag, filename)`
fn parse_github_release_download_url(url: &str) -> Option<(String, String, String, String)> {
    const GITHUB_PREFIX: &str = "https://github.com/";
    
    if !url.starts_with(GITHUB_PREFIX) {
        return None;
    }
    
    let rest = &url[GITHUB_PREFIX.len()..];
    let mut parts = rest.split('/');
    
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    
    // 验证路径结构: releases/download/tag/filename
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

/// 构建下载 URL 列表（包含代理和直连）
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

// ========== 安装相关 ==========

/// 构建安装命令参数
/// 
/// 使用 `/passive` 模式显示安装进度条，但不要求用户交互
#[cfg(target_os = "windows")]
fn build_install_command(file_path: &str, extension: &str) -> Result<String, String> {
    let escaped_path = file_path.replace("'", "''");
    
    match extension {
        "msi" => {
            // /passive: 显示进度条但不要求用户交互
            // /norestart: 安装完成后不自动重启
            Ok(format!(
                "Start-Process msiexec -ArgumentList '/i', '{}', '/passive', '/norestart' -Verb RunAs -Wait",
                escaped_path
            ))
        }
        "exe" => {
            // 移除 /S 和 /silent 参数，让安装程序显示界面
            // 如果安装程序支持，可以使用 /SILENT 但显示进度条
            // 这里先尝试不静默，如果安装程序支持静默但显示进度，可以后续调整
            Ok(format!(
                "Start-Process '{}' -Verb RunAs -Wait",
                escaped_path
            ))
        }
        _ => Err(format!("不支持的安装包格式: {}", extension)),
    }
}

/// 安装更新文件
#[tauri::command]
pub async fn install_update(file_path: String, app_handle: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        println!("🔧 开始安装更新: {}", file_path);
        
        // 先关闭Sunshine和GUI
        stop_sunshine_and_gui()?;
        
        // 检查文件扩展名
        let path = std::path::Path::new(&file_path);
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // 构建安装命令
        let install_args = build_install_command(&file_path, &extension)?;
        
        println!("🔐 使用管理员权限启动安装程序（将显示安装进度）");
        
        // 不使用 CREATE_NO_WINDOW 标志，让安装程序窗口可见
        // 使用 -WindowStyle Normal 确保 PowerShell 窗口可见（如果需要）
        Command::new("powershell")
            .args(&["-NoProfile", "-WindowStyle", "Normal", "-Command", &install_args])
            .spawn()
            .map_err(|e| format!("启动安装程序失败: {}", e))?;
        
        println!("✅ 安装程序已启动，正在安装...");
        
        // 延迟后退出当前GUI进程
        let app_clone = app_handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("🚪 退出GUI进程，等待安装完成...");
            app_clone.exit(0);
        });
        
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持Windows".to_string())
    }
}

// ========== 模块初始化 ==========

/// 初始化更新检查模块
pub fn init_update_checker(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 初始化更新偏好设置
    let prefs = Arc::new(Mutex::new(UpdatePreferences::default()));
    app.manage(prefs);
    
    // 启动时自动检查更新
    let app_handle = app.handle().clone();
    // check_for_updates_on_startup(app_handle);
    
    Ok(())
}

