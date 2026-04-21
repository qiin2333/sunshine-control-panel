// ControllerMeta 集成模块
//
// ControllerMeta (https://github.com/HK560/ControllerMeta) 是一个基于 WebView2 的
// 高精度手柄分析工具。官方发布形式为单个 Windows EXE，本模块负责：
//   1. 检测本地是否已安装（%LOCALAPPDATA%\Sunshine\controllermeta\ControllerMeta.exe）
//   2. 查询 GitHub Releases 最新版本
//   3. 下载并落盘（带进度事件）
//   4. 启动 EXE（GUI 进程，不捕获窗口）

use log::{debug, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

// ========== 常量 ==========

const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/HK560/ControllerMeta/releases/latest";
const BINARY_NAME: &str = "ControllerMeta.exe";
const INSTALL_SUBDIR: &str = "controllermeta";
const DOWNLOAD_PROGRESS_EVENT: &str = "controllermeta-download-progress";

/// GitHub API 加速代理前缀（按优先级试探，最后才直连）
const API_PROXY_PREFIXES: &[&str] = &[
    "https://ghapi.hackhub.cn/",
    "https://mirror.ghproxy.com/",
];

/// GitHub Release 资产下载加速代理
const DOWNLOAD_PROXY_PREFIXES: &[&str] = &[
    "https://ghfast.top/",
    "https://ghproxy.com/",
    "https://mirror.ghproxy.com/",
];

/// 已启动的子进程句柄（仅用于判断是否由我们拉起）
static CHILD_PROCESS: Lazy<Mutex<Option<std::process::Child>>> =
    Lazy::new(|| Mutex::new(None));

// ========== 数据结构 ==========

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControllerMetaStatus {
    /// 是否已安装
    pub installed: bool,
    /// 是否正在运行
    pub running: bool,
    /// 安装目录
    pub install_path: String,
    /// 可执行文件完整路径
    pub binary_path: String,
    /// 已安装版本（来自 version.txt）
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControllerMetaRelease {
    pub version: String,
    pub download_url: Option<String>,
    pub download_name: Option<String>,
    pub download_size: u64,
    pub release_page: String,
    pub release_notes: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

// ========== 路径工具 ==========

fn get_install_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Sunshine")
            .join(INSTALL_SUBDIR);
    }
    // fallback：与控制面板同级
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join(INSTALL_SUBDIR);
        }
    }
    PathBuf::from(format!("./{}", INSTALL_SUBDIR))
}

fn get_binary_path() -> PathBuf {
    get_install_dir().join(BINARY_NAME)
}

fn get_version_file() -> PathBuf {
    get_install_dir().join("version.txt")
}

fn read_installed_version() -> String {
    std::fs::read_to_string(get_version_file())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

// ========== 进程检查 ==========

fn is_process_running() -> bool {
    // 先检查我们自己 spawn 的子进程
    {
        let mut guard = CHILD_PROCESS.lock().unwrap();
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => return true,
                Ok(Some(_)) | Err(_) => *guard = None,
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("tasklist")
            .args([
                "/FI",
                &format!("IMAGENAME eq {}", BINARY_NAME),
                "/NH",
            ])
            .creation_flags(0x08000000)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.to_lowercase().contains(&BINARY_NAME.to_lowercase());
        }
    }

    false
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn controllermeta_get_status() -> Result<ControllerMetaStatus, String> {
    let install_dir = get_install_dir();
    let binary_path = get_binary_path();
    let installed = binary_path.exists();
    let running = is_process_running();
    let version = if installed {
        read_installed_version()
    } else {
        String::new()
    };

    Ok(ControllerMetaStatus {
        installed,
        running,
        install_path: install_dir.to_string_lossy().to_string(),
        binary_path: binary_path.to_string_lossy().to_string(),
        version,
    })
}

#[tauri::command]
pub async fn controllermeta_check_release() -> Result<ControllerMetaRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .connect_timeout(std::time::Duration::from_secs(3))
        .user_agent("sunshine-control-panel")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 依次试探：直连 → 代理 1 → 代理 2……
    let mut urls = vec![GITHUB_API_LATEST.to_string()];
    for proxy in API_PROXY_PREFIXES {
        urls.push(format!("{}{}", proxy, GITHUB_API_LATEST));
    }

    let mut last_err = String::from("未尝试");
    for url in &urls {
        debug!("📦 查询 ControllerMeta 最新版本: {}", url);
        let result = client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await;

        let response = match result {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                last_err = format!("HTTP {}", r.status());
                warn!("⚠️ 请求失败 ({}): {}", url, last_err);
                continue;
            }
            Err(e) => {
                last_err = e.to_string();
                warn!("⚠️ 请求失败 ({}): {}", url, last_err);
                continue;
            }
        };

        match response.json::<GitHubRelease>().await {
            Ok(release) => {
                let asset = release
                    .assets
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(BINARY_NAME))
                    .or_else(|| {
                        release
                            .assets
                            .iter()
                            .find(|a| a.name.to_lowercase().ends_with(".exe"))
                    });

                let (download_url, download_name, download_size) = match asset {
                    Some(a) => (
                        Some(a.browser_download_url.clone()),
                        Some(a.name.clone()),
                        a.size,
                    ),
                    None => (None, None, 0),
                };

                return Ok(ControllerMetaRelease {
                    version: release.tag_name.clone(),
                    download_url,
                    download_name,
                    download_size,
                    release_page: release.html_url,
                    release_notes: release.body.unwrap_or_default(),
                });
            }
            Err(e) => {
                last_err = format!("解析失败: {}", e);
                warn!("⚠️ {}", last_err);
            }
        }
    }

    Err(format!(
        "查询 GitHub Release 失败（已尝试 {} 个地址）: {}",
        urls.len(),
        last_err
    ))
}

/// 下载 ControllerMeta.exe 到安装目录
///
/// 发送 `controllermeta-download-progress` 事件上报进度：
///   { progress: u32 (0-100), downloaded: u64, total: u64 }
#[tauri::command]
pub async fn controllermeta_download(
    url: String,
    version: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    info!("📥 开始下载 ControllerMeta: {}", url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent("sunshine-control-panel")
        .build()
        .map_err(|e| format!("创建客户端失败: {}", e))?;

    // 构造下载 URL 列表：代理优先（国内直连 GitHub Releases 常常超时），最后直连
    let mut urls: Vec<String> = DOWNLOAD_PROXY_PREFIXES
        .iter()
        .map(|p| format!("{}{}", p, url))
        .collect();
    urls.push(url.clone());

    let mut last_err = String::from("未尝试");
    let mut response_opt = None;
    for try_url in &urls {
        debug!("🔗 尝试下载: {}", try_url);
        match client
            .get(try_url)
            .header("Accept", "application/octet-stream")
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                info!("✅ 连接成功: {}", try_url);
                response_opt = Some(r);
                break;
            }
            Ok(r) => {
                last_err = format!("HTTP {}", r.status());
                warn!("⚠️ {} → {}", try_url, last_err);
            }
            Err(e) => {
                last_err = e.to_string();
                warn!("⚠️ {} → {}", try_url, last_err);
            }
        }
    }

    let response = response_opt
        .ok_or_else(|| format!("所有下载源均失败: {}", last_err))?;

    let total_size = response.content_length().unwrap_or(0);
    let install_dir = get_install_dir();
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("创建安装目录失败: {}", e))?;

    // 下载到临时文件，完成后再原子重命名到目标位置（避免覆盖运行中的 exe）
    let final_path = get_binary_path();
    let temp_path = install_dir.join(format!("{}.download", BINARY_NAME));
    // 若已存在残留临时文件，先清理
    if temp_path.exists() {
        let _ = std::fs::remove_file(&temp_path);
    }

    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let window = app_handle.get_webview_window("main");

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载数据失败: {}", e))?;
        std::io::Write::write_all(&mut file, &chunk)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;

        if let Some(ref w) = window {
            let progress = if total_size > 0 {
                (downloaded as f64 / total_size as f64 * 100.0) as u32
            } else {
                0
            };
            let _ = w.emit(
                DOWNLOAD_PROGRESS_EVENT,
                serde_json::json!({
                    "progress": progress,
                    "downloaded": downloaded,
                    "total": total_size,
                }),
            );
        }
    }
    drop(file);

    info!("✅ 下载完成 ({} bytes)", downloaded);

    // 如果目标文件正在运行，重命名会失败；返回友好提示
    if final_path.exists() {
        if let Err(e) = std::fs::remove_file(&final_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!(
                "无法覆盖已有版本，请先关闭 ControllerMeta 后重试: {}",
                e
            ));
        }
    }

    std::fs::rename(&temp_path, &final_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!("落盘失败: {}", e)
    })?;

    // 记录版本号
    if !version.is_empty() {
        let clean = version.trim_start_matches('v');
        let _ = std::fs::write(get_version_file(), clean);
    }

    info!("✅ ControllerMeta 安装完成: {:?}", final_path);
    Ok(final_path.to_string_lossy().to_string())
}

/// 启动 ControllerMeta（GUI 进程，fire-and-forget）
#[tauri::command]
pub async fn controllermeta_launch() -> Result<String, String> {
    let binary_path = get_binary_path();
    if !binary_path.exists() {
        return Err("ControllerMeta 未安装".to_string());
    }

    // 如果已在运行，不再重复启动
    if is_process_running() {
        info!("ℹ️ ControllerMeta 已在运行");
        return Ok("already-running".to_string());
    }

    let install_dir = get_install_dir();
    info!("🎮 启动 ControllerMeta: {:?}", binary_path);

    // 下载的 exe 会被 Windows 打上 Zone.Identifier（MOTW）标记，
    // 某些场景下会导致运行时 UAC 拦截或被 SmartScreen 阻断，先清理掉
    #[cfg(target_os = "windows")]
    {
        let zone_ads = format!("{}:Zone.Identifier", binary_path.to_string_lossy());
        let _ = std::fs::remove_file(&zone_ads);
    }

    #[cfg(target_os = "windows")]
    let child = {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS(0x00000008)：与父控制台脱钩
        // 不使用 CREATE_BREAKAWAY_FROM_JOB：若父进程所在的 Job 对象禁止 breakaway
        // 会触发 ERROR_ACCESS_DENIED (os error 5)
        std::process::Command::new(&binary_path)
            .current_dir(&install_dir)
            .creation_flags(0x00000008)
            .spawn()
            .map_err(|e| format!("启动 ControllerMeta 失败: {}", e))?
    };

    #[cfg(not(target_os = "windows"))]
    let child = std::process::Command::new(&binary_path)
        .current_dir(&install_dir)
        .spawn()
        .map_err(|e| format!("启动 ControllerMeta 失败: {}", e))?;

    let pid = child.id();
    *CHILD_PROCESS.lock().unwrap() = Some(child);
    Ok(format!("started:{}", pid))
}

#[tauri::command]
pub fn controllermeta_get_install_path() -> String {
    get_install_dir().to_string_lossy().to_string()
}

/// 卸载：删除整个安装目录
#[tauri::command]
pub async fn controllermeta_uninstall() -> Result<String, String> {
    if is_process_running() {
        return Err("ControllerMeta 正在运行，请先关闭后再卸载".to_string());
    }
    let install_dir = get_install_dir();
    if !install_dir.exists() {
        return Ok("not-installed".to_string());
    }
    std::fs::remove_dir_all(&install_dir)
        .map_err(|e| format!("删除安装目录失败: {}", e))?;
    info!("🗑️ ControllerMeta 已卸载: {:?}", install_dir);
    Ok("uninstalled".to_string())
}
