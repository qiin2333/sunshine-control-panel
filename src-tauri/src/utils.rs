use reqwest;
use serde_json::Value;
use std::process::Command;
use crate::sunshine;
use std::env;
use tauri::Manager;

#[allow(dead_code)]
pub async fn send_http_request(
    hostname: &str,
    port: u16,
    path: &str,
    method: &str,
    data: Option<Value>,
) -> Result<String, String> {
    let url = format!("https://{}:{}{}", hostname, port, path);
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    
    let request = match method.to_uppercase().as_str() {
        "POST" => {
            let mut req = client.post(&url);
            if let Some(json_data) = data {
                req = req.json(&json_data);
            }
            req
        }
        "GET" => client.get(&url),
        _ => return Err("Unsupported HTTP method".to_string()),
    };
    
    let response = request
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let text = response.text().await.map_err(|e| e.to_string())?;
    Ok(text)
}

#[tauri::command]
pub async fn restart_graphics_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // 从注册表动态获取 Sunshine 安装路径
        let sunshine_path = std::path::PathBuf::from(sunshine::get_sunshine_install_path());
        let restart_exe = sunshine_path.join("tools").join("restart64.exe");
        
        if !restart_exe.exists() {
            return Err("找不到 restart64.exe".to_string());
        }
        
        // 使用 PowerShell 以管理员权限运行
        let ps_command = format!(
            r#"Start-Process '{}' -Verb RunAs -WindowStyle Hidden"#,
            restart_exe.display()
        );
        
        Command::new("powershell")
            .args(&["-Command", &ps_command])
            .spawn()
            .map_err(|e| e.to_string())?;
        
        Ok("已请求重启显卡驱动".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

#[tauri::command]
pub async fn restart_sunshine_service() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // 从注册表动态获取 Sunshine 安装路径
        let sunshine_path = std::path::PathBuf::from(sunshine::get_sunshine_install_path());
        
        // 构建重启命令
        let command = format!(
            "net stop sunshineservice; taskkill /IM sunshine.exe /F; cd '{}'; .\\sunshine.exe",
            sunshine_path.display()
        );
        
        // 使用正确的引号转义
        let ps_command = format!(
            "Start-Process powershell -ArgumentList '-NoProfile', '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait",
            command.replace("'", "''")  // PowerShell 中单引号需要双写转义
        );
        
        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_command])
            .spawn()
            .map_err(|e| e.to_string())?;
        
        Ok("已请求重启 Sunshine 服务".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 以管理员权限重启 GUI
#[tauri::command]
pub async fn restart_as_admin(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        
        // 获取当前可执行文件路径
        let current_exe = env::current_exe()
            .map_err(|e| format!("获取当前程序路径失败: {}", e))?;
        
        println!("🔄 准备以管理员权限重启 GUI");
        println!("   当前程序: {:?}", current_exe);
        
        // 使用 PowerShell 的 Start-Process -Verb RunAs 来提升权限
        let exe_path = current_exe.to_string_lossy().to_string();
        
        // 创建 PowerShell 命令来以管理员身份启动
        let ps_command = format!(
            "Start-Sleep -Milliseconds 500; Start-Process -FilePath '{}' -Verb RunAs",
            exe_path.replace("'", "''")  // 转义单引号
        );
        
        println!("   PowerShell 命令: {}", ps_command);
        
        // CREATE_NO_WINDOW = 0x08000000
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // 启动提升权限的新实例（PowerShell 会等待 500ms 后启动）
        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_command])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("启动管理员实例失败: {}", e))?;
        
        println!("✅ 已请求以管理员权限启动新实例（500ms 后）");
        
        // 立即退出当前实例，让新实例可以绑定端口
        tokio::spawn(async move {
            println!("🚪 准备退出当前实例...");
            
            // 先关闭主窗口
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.close();
                println!("   关闭主窗口");
            }
            
            // 短暂延迟后退出，让窗口关闭并释放端口 48081
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            println!("🚪 退出当前实例，释放端口 48081");
            app_handle.exit(0);
        });
        
        Ok("正在以管理员权限重启...".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows".to_string())
    }
}

/// 检查当前程序是否以管理员权限运行
#[tauri::command]
pub fn is_running_as_admin() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        
        unsafe {
            let mut token: HANDLE = HANDLE::default();
            let process = GetCurrentProcess();
            
            // 打开当前进程的访问令牌
            if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
                return Ok(false);
            }
            
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut return_length = 0u32;
            
            // 获取令牌提升信息
            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_length,
            );
            
            CloseHandle(token).ok();
            
            if result.is_err() {
                return Ok(false);
            }
            
            Ok(elevation.TokenIsElevated != 0)
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 系统检查 root 权限
        Ok(unsafe { libc::geteuid() == 0 })
    }
}

/// 在外部浏览器中打开 URL
pub fn open_url_in_browser(url: &str) {
    let url = url.to_string();
    
    tauri::async_runtime::spawn(async move {
        println!("🌐 正在打开外部浏览器...");
        
        #[cfg(target_os = "windows")]
        {
            if let Err(e) = Command::new("cmd")
                .args(&["/c", "start", "", &url])
                .spawn()
            {
                eprintln!("❌ 打开 URL 失败: {}", e);
            } else {
                println!("✅ 已在外部浏览器中打开: {}", url);
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            if let Err(e) = Command::new("xdg-open")
                .arg(&url)
                .spawn()
            {
                eprintln!("❌ 打开 URL 失败: {}", e);
            } else {
                println!("✅ 已在外部浏览器中打开: {}", url);
            }
        }
    });
}

/// Tauri 命令：在外部浏览器中打开 URL
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<bool, String> {
    if !url.starts_with("http") {
        return Ok(false);
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/c", "start", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    
    Ok(true)
}
