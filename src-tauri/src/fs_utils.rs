use std::path::PathBuf;
use crate::sunshine;
use log::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

/// 扫描到的应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedApp {
    pub name: String,
    pub cmd: String,
    #[serde(rename = "working-dir")]
    pub working_dir: String,
    pub source_path: String,
    #[serde(rename = "app-type")]
    pub app_type: String,
    #[serde(rename = "is-game", skip_serializing_if = "Option::is_none")]
    pub is_game: Option<bool>,
}

/// 平台游戏库扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformGame {
    pub name: String,
    pub app_id: String,
    pub platform: String,        // "steam", "epic", "gog"
    pub install_dir: String,
    pub exe_path: String,
    pub cmd: String,
    #[serde(rename = "working-dir")]
    pub working_dir: String,
    #[serde(rename = "cover-url", skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(rename = "size-on-disk", skip_serializing_if = "Option::is_none")]
    pub size_on_disk: Option<u64>,
}

/// 快捷方式解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LnkInfo {
    pub name: String,
    #[serde(rename = "targetPath")]
    pub target_path: String,
    #[serde(rename = "workingDir")]
    pub working_dir: String,
    pub arguments: String,
}

/// 获取 ICC 颜色配置文件列表
#[tauri::command]
pub async fn get_icc_file_list() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let color_dir = std::env::var("windir")
            .map(|windir| PathBuf::from(windir).join("System32\\spool\\drivers\\color"))
            .unwrap_or_else(|_| PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color"));
        
        match std::fs::read_dir(&color_dir) {
            Ok(entries) => {
                let mut files = Vec::new();
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Some(file_name) = entry.file_name().to_str() {
                            // 只包含 .icc 和 .icm 文件
                            if file_name.ends_with(".icc") || file_name.ends_with(".icm") {
                                files.push(file_name.to_string());
                            }
                        }
                    }
                }
                files.sort();  // 按字母顺序排序
                Ok(files)
            }
            Err(e) => Err(format!("读取目录失败: {}", e)),
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec![])  // 非 Windows 系统返回空列表
    }
}

/// 读取指定目录的文件列表
#[tauri::command]
pub async fn read_directory(path: String) -> Result<Vec<String>, String> {
    match std::fs::read_dir(&path) {
        Ok(entries) => {
            let mut files = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(file_name) = entry.file_name().to_str() {
                        files.push(file_name.to_string());
                    }
                }
            }
            files.sort();
            Ok(files)
        }
        Err(e) => Err(format!("读取目录失败: {}", e)),
    }
}

/// 读取图片文件并返回 Base64 编码的 Data URL
#[tauri::command]
pub async fn read_image_as_data_url(path: String) -> Result<String, String> {
    use std::fs;
    use std::path::Path;
    
    // 读取文件
    let file_bytes = fs::read(&path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    
    debug!("📖 读取文件成功: {}, 大小: {} bytes", path, file_bytes.len());
    
    // 根据扩展名确定 MIME 类型
    let path_obj = Path::new(&path);
    let extension = path_obj.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let mime_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png", // 默认
    };
    
    // 转换为 Base64
    use base64::{Engine as _, engine::general_purpose};
    let base64 = general_purpose::STANDARD.encode(&file_bytes);
    
    // 构造 Data URL
    let data_url = format!("data:{};base64,{}", mime_type, base64);
    
    debug!("✅ Data URL 生成成功, MIME: {}, Base64 长度: {}", mime_type, base64.len());
    
    Ok(data_url)
}

/// 复制图片文件到 Sunshine assets 目录
/// 返回相对于 Sunshine Web 服务器的 URL 路径（/boxart/xxx.jpg）
#[tauri::command]
pub async fn copy_image_to_assets(source_path: String) -> Result<String, String> {
    use std::fs;
    use std::path::Path;
    
    let source = Path::new(&source_path);
    
    // 验证源文件存在
    if !source.exists() {
        return Err(format!("源文件不存在: {}", source_path));
    }
    
    // 获取 Sunshine 安装路径
    let sunshine_path = PathBuf::from(sunshine::get_sunshine_install_path());
    let assets_dir = sunshine_path.join("assets");
    
    // 创建 assets 目录（如果不存在）
    fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;
    
    // 获取文件名
    let file_name = source.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无效的文件名".to_string())?;
    
    // 生成唯一文件名（避免覆盖）
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let extension = source.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let unique_name = format!("bg_{}_{}.{}", timestamp, file_name.replace(|c: char| !c.is_alphanumeric(), "_"), extension);
    
    // 目标路径
    let dest_path = assets_dir.join(&unique_name);
    
    // 复制文件
    fs::copy(source, &dest_path)
        .map_err(|e| format!("复制文件失败: {}", e))?;
    
    info!("✅ 图片已复制到: {:?}", dest_path);
    
    // 返回相对于 Sunshine Web 根目录的 URL 路径
    let web_url = format!("/boxart/{}", unique_name);
    
    Ok(web_url)
}

/// 清理 covers 目录中未被使用的封面图片
#[tauri::command]
pub async fn cleanup_unused_covers() -> Result<serde_json::Value, String> {
    use std::fs;
    use std::collections::HashSet;
    use serde_json::json;
    
    info!("🧹 开始清理无用封面...");
    
    // 获取 Sunshine config 目录
    let sunshine_path = PathBuf::from(sunshine::get_sunshine_install_path()).join("config");
    let covers_dir = sunshine_path.join("covers");
    let apps_json_path = sunshine_path.join("apps.json");
    
    debug!("📂 使用 covers 目录: {:?}", covers_dir);
    debug!("📄 使用 apps.json 路径: {:?}", apps_json_path);
    
    // 读取 apps.json 获取所有正在使用的图片
    let used_images: HashSet<String> = if apps_json_path.exists() {
        match fs::read_to_string(&apps_json_path) {
            Ok(content) => {
                // 检查文件内容是否为空或只包含空白字符
                let trimmed_content = content.trim();
                if trimmed_content.is_empty() {
                    warn!("⚠️  apps.json 文件为空，跳过解析");
                    HashSet::new()
                } else {
                    // 尝试解析 JSON
                    match serde_json::from_str::<serde_json::Value>(trimmed_content) {
                        Ok(apps) => {
                            let mut images = HashSet::new();
                            
                            if let Some(apps_array) = apps.get("apps").and_then(|a| a.as_array()) {
                                for app in apps_array {
                                    if let Some(image_path) = app.get("image-path").and_then(|p| p.as_str()) {
                                        // 跳过无效或默认图片
                                        if image_path.is_empty() || image_path == "desktop" {
                                            continue;
                                        }
                                        
                                        // 提取文件名（去除路径）
                                        let filename = image_path.split('/').last()
                                            .or_else(|| image_path.split('\\').last())
                                            .unwrap_or(image_path);
                                        
                                        if !filename.is_empty() && filename != "desktop" {
                                            // 始终保存文件名
                                            images.insert(filename.to_string());
                                            
                                            // 如果路径包含分隔符，也保存完整路径
                                            if image_path.contains('/') || image_path.contains('\\') {
                                                images.insert(image_path.to_string());
                                                debug!("  📌 使用中: {} (完整路径: {})", filename, image_path);
                                            } else {
                                                debug!("  📌 使用中: {}", filename);
                                            }
                                        }
                                    }
                                }
                            }
                            images
                        }
                        Err(e) => {
                            warn!("⚠️  解析 apps.json 失败: {}，跳过解析", e);
                            HashSet::new()
                        }
                    }
                }
            }
            Err(e) => {
                warn!("⚠️  读取 apps.json 失败: {}，跳过解析", e);
                HashSet::new()
            }
        }
    } else {
        debug!("📄 apps.json 不存在，跳过解析");
        HashSet::new()
    };
    
    debug!("  正在使用的封面数: {}", used_images.len());
    
    let mut deleted_count = 0;
    let mut freed_space: u64 = 0;
    let mut errors = Vec::new();
    
    // === 1. 清理 covers 目录中未使用的封面 ===
    if covers_dir.exists() {
        debug!("\n📂 扫描 covers 目录...");
        let entries = fs::read_dir(&covers_dir)
            .map_err(|e| format!("读取 covers 目录失败: {}", e))?;
        
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        // 更安全的检查：检查文件名是否在任何路径中被使用
                        let is_used = {
                            // 直接检查文件名
                            used_images.contains(filename) ||
                            // 检查是否有路径以这个文件名结尾
                            used_images.iter().any(|used_path| {
                                used_path.ends_with(&format!("/{}", filename)) ||
                                used_path.ends_with(&format!("\\{}", filename)) ||
                                used_path == filename
                            })
                        };
                        
                        if !is_used {
                            // 获取文件大小
                            let size = fs::metadata(&path)
                                .map(|m| m.len())
                                .unwrap_or(0);
                            
                            // 删除文件
                            match fs::remove_file(&path) {
                                Ok(_) => {
                                    debug!("  🗑️  [封面] {}", filename);
                                    deleted_count += 1;
                                    freed_space += size;
                                }
                                Err(e) => {
                                    let error_msg = format!("删除封面 {} 失败: {}", filename, e);
                                    error!("  ❌ {}", error_msg);
                                    errors.push(error_msg);
                                }
                            }
                        } else {
                            debug!("  ✅ [保护] {} (正在使用中)", filename);
                        }
                    }
                }
            }
        }
    }
    
    // === 2. 清理 config 目录中的 temp_ 临时文件 ===
    debug!("\n📂 扫描 config 目录中的临时文件...");
    if sunshine_path.exists() {
        match fs::read_dir(&sunshine_path) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        
                        if path.is_file() {
                            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                                // 删除 temp_ 开头的临时文件
                                if filename.starts_with("temp_") {
                                    let size = fs::metadata(&path)
                                        .map(|m| m.len())
                                        .unwrap_or(0);
                                    
                                    match fs::remove_file(&path) {
                                        Ok(_) => {
                                            debug!("  🗑️  [临时] {}", filename);
                                            deleted_count += 1;
                                            freed_space += size;
                                        }
                                        Err(e) => {
                                            let error_msg = format!("删除临时文件 {} 失败: {}", filename, e);
                                            error!("  ❌ {}", error_msg);
                                            errors.push(error_msg);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("读取 config 目录失败: {}", e);
                warn!("  ⚠️  {}", error_msg);
                // 不返回错误，继续执行
            }
        }
    }
    
    let message = if deleted_count > 0 {
        format!("成功删除 {} 个无用文件，释放 {:.2} KB", deleted_count, freed_space as f64 / 1024.0)
    } else {
        "没有发现需要清理的文件".to_string()
    };
    
    info!("\n✅ 清理完成: {}", message);
    
    Ok(json!({
        "success": true,
        "message": message,
        "deleted_count": deleted_count,
        "freed_space": freed_space,
        "errors": errors
    }))
}

/// 解析 Windows 快捷方式 (.lnk) 文件
#[tauri::command]
pub async fn resolve_lnk_target(lnk_path: String) -> Result<LnkInfo, String> {
    #[cfg(target_os = "windows")]
    {
        resolve_lnk_windows(&lnk_path)
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("快捷方式解析仅支持 Windows 系统".to_string())
    }
}

#[cfg(target_os = "windows")]
fn resolve_lnk_windows(lnk_path: &str) -> Result<LnkInfo, String> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::Interface;
    use std::path::Path;
    
    info!("🔗 解析快捷方式: {}", lnk_path);
    
    // 初始化 COM
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    
    let result = (|| -> Result<LnkInfo, String> {
        // 创建 ShellLink 对象
        let shell_link: IShellLinkW = unsafe {
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("创建 ShellLink 失败: {:?}", e))?
        };
        
        // 获取 IPersistFile 接口
        let persist_file: IPersistFile = shell_link.cast()
            .map_err(|e| format!("获取 IPersistFile 失败: {:?}", e))?;
        
        // 加载 .lnk 文件
        let wide_path: Vec<u16> = OsStr::new(lnk_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            persist_file.Load(
                windows::core::PCWSTR(wide_path.as_ptr()),
                STGM_READ,
            ).map_err(|e| format!("加载 .lnk 文件失败: {:?}", e))?;
        }
        
        // 获取目标路径
        let mut target_path_buf: [u16; 260] = [0; 260];
        let mut find_data: windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
        
        unsafe {
            shell_link.GetPath(
                &mut target_path_buf,
                &mut find_data,
                windows::Win32::UI::Shell::SLGP_RAWPATH.0 as u32,
            ).map_err(|e| format!("获取目标路径失败: {:?}", e))?;
        }
        
        let target_path = String::from_utf16_lossy(
            &target_path_buf[..target_path_buf.iter().position(|&c| c == 0).unwrap_or(target_path_buf.len())]
        );
        
        // 获取工作目录
        let mut working_dir_buf: [u16; 260] = [0; 260];
        unsafe {
            let _ = shell_link.GetWorkingDirectory(&mut working_dir_buf);
        }
        
        let working_dir = String::from_utf16_lossy(
            &working_dir_buf[..working_dir_buf.iter().position(|&c| c == 0).unwrap_or(working_dir_buf.len())]
        );
        
        // 获取参数
        let mut arguments_buf: [u16; 1024] = [0; 1024];
        unsafe {
            let _ = shell_link.GetArguments(&mut arguments_buf);
        }
        
        let arguments = String::from_utf16_lossy(
            &arguments_buf[..arguments_buf.iter().position(|&c| c == 0).unwrap_or(arguments_buf.len())]
        );
        
        // 从 lnk 文件名获取名称
        let lnk_file_path = Path::new(lnk_path);
        let name = lnk_file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        debug!("✅ 快捷方式解析成功:");
        debug!("   名称: {}", name);
        debug!("   目标: {}", target_path);
        debug!("   工作目录: {}", working_dir);
        debug!("   参数: {}", arguments);
        
        Ok(LnkInfo {
            name,
            target_path,
            working_dir,
            arguments,
        })
    })();
    
    // 清理 COM
    unsafe {
        CoUninitialize();
    }
    
    result
}

/// 扫描目录中的可执行文件和快捷方式
/// 返回找到的应用列表
#[tauri::command]
pub async fn scan_directory_for_apps(directory: String) -> Result<Vec<ScannedApp>, String> {
    use std::path::Path;
    
    info!("📂 开始扫描目录: {}", directory);
    
    let dir_path = Path::new(&directory);
    if !dir_path.exists() {
        return Err(format!("目录不存在: {}", directory));
    }
    
    if !dir_path.is_dir() {
        return Err(format!("路径不是目录: {}", directory));
    }
    
    let mut apps: Vec<ScannedApp> = Vec::new();
    
    // 支持的文件扩展名
    let supported_extensions = [".lnk", ".exe", ".bat", ".cmd", ".url"];
    
    // 递归扫描目录
    scan_directory_recursive(dir_path, &supported_extensions, &mut apps)?;
    
    info!("✅ 扫描完成，找到 {} 个应用", apps.len());
    Ok(apps)
}

/// 检测应用是否是游戏
/// 基于路径、文件名和常见游戏平台目录
fn detect_if_game(file_path: &str, name: &str, target_path: Option<&str>) -> bool {    
    let path_lower = file_path.to_lowercase();
    let name_lower = name.to_lowercase();
    let target_lower = target_path.map(|s| s.to_lowercase()).unwrap_or_default();
    
    // 不是 .exe 文件肯定不是游戏
    // 检查文件路径或目标路径是否以 .exe 结尾
    let is_exe = path_lower.ends_with(".exe") || 
                 target_lower.ends_with(".exe") ||
                 // 对于 .lnk 快捷方式，检查其目标是否是 .exe
                 (path_lower.ends_with(".lnk") && target_lower.ends_with(".exe"));
    
    if !is_exe && !path_lower.ends_with(".lnk") {
        return false;
    }
    
    // 对于 .lnk 文件，如果目标不是 .exe，也不是游戏
    if path_lower.ends_with(".lnk") && !target_lower.is_empty() && !target_lower.ends_with(".exe") {
        return false;
    }
    
    // 首先排除明显不是游戏的应用
    let exclude_keywords = [
        "uninstall", "卸载", "setup", "安装", "installer",
        "update", "更新", "updater", "patch",
        "config", "配置", "settings", "设置",
        "crash", "崩溃", "reporter", "report",
        "helper", "service", "daemon",
        "redist", "redistributable", "vcredist", "directx",
        "launcher_helper", "bootstrapper",
        "ue4prereqsetup", "dxsetup", "dotnet",
        // 常见非游戏应用
        "chrome", "firefox", "edge", "opera", "brave",
        "word", "excel", "powerpoint", "outlook", "onenote", "access",
        "visual studio", "vscode", "code", "notepad", "sublime",
        "git", "node", "python", "java", "ruby",
        "adobe", "photoshop", "illustrator", "premiere", "after effects",
        "spotify", "discord", "telegram", "wechat", "微信", "qq",
        "obs", "vlc", "potplayer", "media player",
        "7-zip", "winrar", "bandizip",
        "driver", "nvidia", "amd ", "intel",
        "antivirus", "defender", "kaspersky", "avast",
        "office", "onedrive", "teams",
        "terminal", "powershell", "cmd",
        "control panel", "控制面板",
        "explorer", "task manager", "任务管理器",
        "calculator", "计算器", "paint", "画图",
        "snipping", "截图",
    ];
    
    for keyword in &exclude_keywords {
        if name_lower.contains(keyword) || path_lower.ends_with(&format!("\\{}.exe", keyword)) {
            return false;
        }
    }
    
    // 游戏平台相关路径关键词（高置信度）
    let high_confidence_paths = [
        "\\steamapps\\common\\",
        "\\steam\\steamapps\\common\\",
        "\\epic games\\",
        "\\gog galaxy\\games\\",
        "\\gog games\\",
        "\\ubisoft\\ubisoft game launcher\\games\\",
        "\\origin games\\",
        "\\ea games\\",
        "\\battle.net\\",
        "\\riot games\\",
        "\\xbox games\\",
        "\\playnite\\",
    ];
    
    // 检查路径中是否包含高置信度的游戏平台路径
    for keyword in &high_confidence_paths {
        if path_lower.contains(keyword) || target_lower.contains(keyword) {
            return true;
        }
    }
    
    // 中等置信度：检查是否在 Program Files 下的 games 目录
    let medium_confidence_paths = [
        "\\program files\\games\\",
        "\\program files\\game\\",
        "\\program files (x86)\\games\\",
        "\\program files (x86)\\game\\",
    ];
    
    for keyword in &medium_confidence_paths {
        if path_lower.contains(keyword) || target_lower.contains(keyword) {
            // 额外检查：确保不是工具类应用
            let tool_indicators = ["tool", "editor", "sdk", "dev", "debug", "server", "manager", "launcher"];
            let is_tool = tool_indicators.iter().any(|t| name_lower.contains(t));
            if !is_tool {
                return true;
            }
        }
    }
    
    // 检查快捷方式来源目录（如果是从开始菜单的游戏文件夹扫描的）
    if path_lower.contains("\\start menu\\programs\\games\\") ||
       path_lower.contains("\\开始菜单\\程序\\游戏\\") {
        return true;
    }
    
    // 低置信度：仅基于文件名判断（需要更严格的条件）
    // 不再仅凭 "game" 关键词判断，因为误报率太高
    
    false
}

/// 递归扫描目录
fn scan_directory_recursive(
    dir_path: &std::path::Path,
    supported_extensions: &[&str],
    apps: &mut Vec<ScannedApp>,
) -> Result<(), String> {
    use std::fs;
    
    // 读取目录内容
    let entries = fs::read_dir(dir_path)
        .map_err(|e| format!("读取目录失败: {}", e))?;
    
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        let path = entry.path();
        
        // 如果是目录，递归扫描
        if path.is_dir() {
            // 跳过一些常见的系统目录和隐藏目录
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if dir_name.starts_with('.') || 
                   dir_name.eq_ignore_ascii_case("$RECYCLE.BIN") ||
                   dir_name.eq_ignore_ascii_case("System Volume Information") {
                    continue;
                }
            }
            
            // 递归扫描子目录，忽略权限错误
            let _ = scan_directory_recursive(&path, supported_extensions, apps);
            continue;
        }
        
        // 只处理文件
        if !path.is_file() {
            continue;
        }
        
        let _file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => format!(".{}", e.to_lowercase()),
            None => continue,
        };
        
        // 检查是否是支持的扩展名
        if !supported_extensions.contains(&ext.as_str()) {
            continue;
        }
        
        let file_path = path.to_string_lossy().to_string();
        debug!("📄 找到文件: {}", file_path);
        
        // 根据文件类型处理
        let scanned_app = match ext.as_str() {
            ".lnk" => {
                #[cfg(target_os = "windows")]
                {
                    process_lnk_file(&file_path)
                }
                #[cfg(not(target_os = "windows"))]
                {
                    None
                }
            }
            ".exe" => {
                process_exe_file(&file_path)
            }
            ".bat" | ".cmd" => {
                process_batch_file(&file_path)
            }
            ".url" => {
                process_url_file(&file_path)
            }
            _ => None,
        };
        
        if let Some(mut app) = scanned_app {
            // 检测是否是游戏
            let target_path = if app.app_type == "shortcut" {
                #[cfg(target_os = "windows")]
                {
                    resolve_lnk_windows(&file_path).ok()
                        .map(|lnk| lnk.target_path)
                }
                #[cfg(not(target_os = "windows"))]
                {
                    None
                }
            } else {
                None
            };
            
            let is_game = detect_if_game(&file_path, &app.name, target_path.as_deref());
            app.is_game = Some(is_game);
            apps.push(app);
        }
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn process_lnk_file(file_path: &str) -> Option<ScannedApp> {
    let lnk_info = resolve_lnk_windows(file_path).ok()?;
    
    let cmd = format!("\"{}\"", file_path);
    
    Some(ScannedApp {
        name: lnk_info.name,
        cmd,
        working_dir: String::new(),
        source_path: file_path.to_string(),
        app_type: "shortcut".to_string(),
        is_game: None, // 将在扫描时检测
    })
}

fn process_exe_file(file_path: &str) -> Option<ScannedApp> {
    use std::path::Path;
    
    let path = Path::new(file_path);
    let name = path.file_stem()?.to_str()?.to_string();
    let working_dir = path.parent()?.to_string_lossy().to_string();
    let cmd = format!("\"{}\"", file_path);
    
    Some(ScannedApp {
        name,
        cmd,
        working_dir,
        source_path: file_path.to_string(),
        app_type: "executable".to_string(),
        is_game: None, // 将在扫描时检测
    })
}

fn process_batch_file(file_path: &str) -> Option<ScannedApp> {
    use std::path::Path;
    
    let path = Path::new(file_path);
    let name = path.file_stem()?.to_str()?.to_string();
    let working_dir = path.parent()?.to_string_lossy().to_string();
    let cmd = format!("cmd /c \"{}\"", file_path);
    let ext = path.extension()?.to_str()?.to_lowercase();
    let app_type = if ext == "bat" { "batch" } else { "command" };
    
    Some(ScannedApp {
        name,
        cmd,
        working_dir,
        source_path: file_path.to_string(),
        app_type: app_type.to_string(),
        is_game: None, // 批处理和命令脚本通常不是游戏
    })
}

fn process_url_file(file_path: &str) -> Option<ScannedApp> {
    use std::path::Path;
    
    let path = Path::new(file_path);
    let name = path.file_stem()?.to_str()?.to_string();
    let cmd = format!("start \"\" \"{}\"", file_path);
    
    Some(ScannedApp {
        name,
        cmd,
        working_dir: String::new(),
        source_path: file_path.to_string(),
        app_type: "url".to_string(),
        is_game: None, // URL 文件通常不是游戏
    })
}

// ======================================================================
// 平台游戏库扫描
// ======================================================================

/// 扫描结果汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLibraryScanResult {
    pub steam: Vec<PlatformGame>,
    pub epic: Vec<PlatformGame>,
    pub gog: Vec<PlatformGame>,
    pub total: usize,
    pub scan_time_ms: u64,
}

/// 统一扫描所有游戏平台库
#[tauri::command]
pub async fn scan_game_libraries() -> Result<GameLibraryScanResult, String> {
    use std::time::Instant;

    let start = Instant::now();
    info!("🎮 开始扫描游戏平台库...");

    let steam = scan_steam_library();
    let epic = scan_epic_library();
    let gog = scan_gog_library();

    let total = steam.len() + epic.len() + gog.len();
    let elapsed = start.elapsed().as_millis() as u64;

    info!("✅ 游戏库扫描完成: Steam={}, Epic={}, GOG={}, 总计={}, 耗时={}ms",
        steam.len(), epic.len(), gog.len(), total, elapsed);

    Ok(GameLibraryScanResult {
        steam,
        epic,
        gog,
        total,
        scan_time_ms: elapsed,
    })
}

// ==================== Steam ====================

/// 扫描 Steam 游戏库
fn scan_steam_library() -> Vec<PlatformGame> {
    let mut games = Vec::new();

    // 查找 Steam 安装路径
    let steam_path = find_steam_path();
    let steam_path = match steam_path {
        Some(p) => p,
        None => {
            info!("Steam 未安装或未找到");
            return games;
        }
    };

    info!("📂 Steam 路径: {}", steam_path.display());

    // 读取 libraryfolders.vdf 获取所有库路径
    let library_folders = get_steam_library_folders(&steam_path);
    info!("📚 找到 {} 个 Steam 库路径", library_folders.len());

    for lib_path in &library_folders {
        let steamapps = lib_path.join("steamapps");
        if !steamapps.exists() {
            continue;
        }

        // 扫描 appmanifest_*.acf
        if let Ok(entries) = std::fs::read_dir(&steamapps) {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let fname = fname.to_string_lossy();
                if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                    if let Some(game) = parse_steam_acf(&entry.path(), &steamapps) {
                        games.push(game);
                    }
                }
            }
        }
    }

    games
}

/// 查找 Steam 安装路径
fn find_steam_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // 尝试注册表
        use winreg::RegKey;
        use winreg::enums::HKEY_LOCAL_MACHINE;

        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        {
            if let Ok(path) = hklm.get_value::<String, _>("InstallPath") {
                let p = PathBuf::from(&path);
                if p.exists() {
                    return Some(p);
                }
            }
        }

        // 备选：默认路径
        let default = PathBuf::from("C:\\Program Files (x86)\\Steam");
        if default.exists() {
            return Some(default);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").ok()?;
        let paths = [
            format!("{}/.steam/steam", home),
            format!("{}/.local/share/Steam", home),
        ];
        for p in &paths {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// 从 libraryfolders.vdf 读取所有 Steam 库路径
fn get_steam_library_folders(steam_path: &PathBuf) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    // Steam 自身路径永远是一个库
    folders.push(steam_path.clone());

    let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
    if !vdf_path.exists() {
        // 旧版本路径
        let alt = steam_path.join("config").join("libraryfolders.vdf");
        if alt.exists() {
            parse_library_folders_vdf(&alt, &mut folders);
        }
        return folders;
    }

    parse_library_folders_vdf(&vdf_path, &mut folders);
    folders
}

/// 简单的 VDF 解析器 — 提取 "path" 字段
fn parse_library_folders_vdf(path: &PathBuf, folders: &mut Vec<PathBuf>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("读取 VDF 失败: {}: {}", path.display(), e);
            return;
        }
    };

    // VDF 格式: "path"		"D:\\SteamLibrary"
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"path\"") {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                let p = PathBuf::from(value);
                if p.exists() && !folders.contains(&p) {
                    folders.push(p);
                }
            }
        }
    }
}

/// 解析 Steam appmanifest ACF 文件
fn parse_steam_acf(acf_path: &PathBuf, steamapps_dir: &PathBuf) -> Option<PlatformGame> {
    let content = std::fs::read_to_string(acf_path).ok()?;

    let app_id = extract_vdf_value(&content, "appid")?;
    let name = extract_vdf_value(&content, "name")?;
    let install_dir_name = extract_vdf_value(&content, "installdir")?;
    let size_str = extract_vdf_value(&content, "SizeOnDisk");

    // 排除 Steamworks 工具类
    let app_id_num: u64 = app_id.parse().unwrap_or(0);
    if app_id_num < 10 {
        return None; // Steam 自身的工具
    }

    // 排除名称包含工具/SDK/运行时等关键词的条目
    let name_lower = name.to_lowercase();
    let exclude_keywords = [
        "redistributable", " sdk", "dedicated server", "proton ",
        "steam linux runtime", "steamworks",
        "directx", "vcredist", "visual c++",
        "common redist", "mod tool", "editor",
        "soundtrack", "ost", "artbook", "art book",
        "benchmark", "demo", " test",
        "developer tool", "devkit",
    ];
    if exclude_keywords.iter().any(|kw| name_lower.contains(kw)) {
        return None;
    }

    // 检查 Steam ACF 中的 apptype（如果有的话），排除 tool / demo / music 类型
    let app_type = extract_vdf_value(&content, "apptype")
        .unwrap_or_default()
        .to_lowercase();
    if matches!(app_type.as_str(), "tool" | "demo" | "music" | "dlc" | "config" | "media") {
        return None;
    }

    let install_dir = steamapps_dir.join("common").join(&install_dir_name);
    let install_dir_str = install_dir.to_string_lossy().to_string();

    // 尝试找到主 exe
    let exe_path = find_main_exe(&install_dir).unwrap_or_default();

    // 使用 steam:// URL 启动（最可靠的方式）
    let cmd = format!("steam://rungameid/{}", app_id);

    let cover_url = Some(format!(
        "https://cdn.akamai.steamstatic.com/steam/apps/{}/header.jpg",
        app_id
    ));

    let size_on_disk = size_str.and_then(|s| s.parse::<u64>().ok());

    Some(PlatformGame {
        name,
        app_id,
        platform: "steam".to_string(),
        install_dir: install_dir_str,
        exe_path,
        cmd,
        working_dir: install_dir.to_string_lossy().to_string(),
        cover_url,
        size_on_disk,
    })
}

/// 从 VDF/ACF 内容中提取键值对
fn extract_vdf_value(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&pattern) {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// 在安装目录中查找主可执行文件
fn find_main_exe(install_dir: &PathBuf) -> Option<String> {
    if !install_dir.exists() {
        return None;
    }

    // 只搜索根目录和一层子目录
    let mut candidates: Vec<(PathBuf, u64)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(install_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().to_lowercase() == "exe" {
                        let name_lower = path.file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        // 排除工具
                        if !is_tool_exe(&name_lower) {
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            candidates.push((path, size));
                        }
                    }
                }
            }
        }
    }

    // 按大小排序，取最大的（通常是主程序）
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.first().map(|(p, _)| p.to_string_lossy().to_string())
}

/// 检查 exe 是否是工具/辅助程序
fn is_tool_exe(name_lower: &str) -> bool {
    let tools = [
        "uninstall", "uninst", "setup", "install", "update", "updater",
        "crash", "reporter", "helper", "service", "launcher_helper",
        "redist", "vcredist", "dxsetup", "dotnet", "ue4prereq",
        "bootstrapper", "cleanup", "repair",
    ];
    tools.iter().any(|t| name_lower.contains(t))
}

// ==================== Epic Games ====================

/// 扫描 Epic Games 库
fn scan_epic_library() -> Vec<PlatformGame> {
    let mut games = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Epic 清单目录
        let manifests_dir = std::env::var("ProgramData")
            .map(|pd| PathBuf::from(pd).join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests"))
            .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests"));

        if !manifests_dir.exists() {
            info!("Epic Games 清单目录不存在: {}", manifests_dir.display());
            return games;
        }

        info!("📂 Epic Games 清单目录: {}", manifests_dir.display());

        if let Ok(entries) = std::fs::read_dir(&manifests_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "item").unwrap_or(false) {
                    if let Some(game) = parse_epic_manifest(&path) {
                        games.push(game);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        info!("Epic Games 扫描仅支持 Windows");
    }

    games
}

/// 解析 Epic Games .item 清单文件（JSON 格式）
#[cfg(target_os = "windows")]
fn parse_epic_manifest(manifest_path: &PathBuf) -> Option<PlatformGame> {
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let display_name = json.get("DisplayName")?.as_str()?.to_string();
    let install_location = json.get("InstallLocation")?.as_str()?.to_string();
    let app_name = json.get("AppName")?.as_str()?.to_string();
    let launch_executable = json.get("LaunchExecutable")?.as_str()?.to_string();

    let install_dir = PathBuf::from(&install_location);
    let exe_path = install_dir.join(&launch_executable);
    let exe_str = exe_path.to_string_lossy().to_string();

    // Epic 启动命令
    let cmd = format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", app_name);

    let size_on_disk = json.get("InstallSize").and_then(|v| v.as_u64());

    Some(PlatformGame {
        name: display_name,
        app_id: app_name,
        platform: "epic".to_string(),
        install_dir: install_location,
        exe_path: exe_str,
        cmd,
        working_dir: install_dir.to_string_lossy().to_string(),
        cover_url: None, // Epic 没有简单的封面 URL
        size_on_disk,
    })
}

// ==================== GOG Galaxy ====================

/// 扫描 GOG Galaxy 游戏库
fn scan_gog_library() -> Vec<PlatformGame> {
    #[cfg(target_os = "windows")]
    {
        // GOG Galaxy 数据库是加密的 SQLite，改用注册表方式
        return scan_gog_from_registry();
    }

    #[cfg(not(target_os = "windows"))]
    {
        info!("GOG 扫描仅支持 Windows");
        return Vec::new();
    }
}

/// 通过 Windows 注册表扫描 GOG 游戏
#[cfg(target_os = "windows")]
fn scan_gog_from_registry() -> Vec<PlatformGame> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let mut games = Vec::new();

    let gog_key = match RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\WOW6432Node\\GOG.com\\Games")
    {
        Ok(key) => key,
        Err(_) => {
            info!("GOG 注册表键不存在");
            return games;
        }
    };

    if let Ok(subkeys) = gog_key.enum_keys().collect::<Result<Vec<String>, _>>() {
        for game_id in &subkeys {
            if let Ok(game_key) = gog_key.open_subkey(game_id) {
                let name: String = game_key.get_value("gameName").unwrap_or_default();
                let path: String = game_key.get_value("path").unwrap_or_default();
                let exe: String = game_key.get_value("exe").unwrap_or_default();

                if name.is_empty() || path.is_empty() {
                    continue;
                }

                let exe_path = if exe.is_empty() {
                    find_main_exe(&PathBuf::from(&path)).unwrap_or_default()
                } else {
                    exe.clone()
                };

                let cmd = if exe_path.is_empty() {
                    format!("goggalaxy://openGameView/{}", game_id)
                } else {
                    format!("\"{}\"", exe_path)
                };

                games.push(PlatformGame {
                    name,
                    app_id: game_id.clone(),
                    platform: "gog".to_string(),
                    install_dir: path.clone(),
                    exe_path,
                    cmd,
                    working_dir: path,
                    cover_url: None,
                    size_on_disk: None,
                });
            }
        }
    }

    games
}
