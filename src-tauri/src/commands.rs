use tauri::{AppHandle, Manager, Emitter};
use log::{info, debug};
use crate::windows;

/// 注意：菜单现在是气泡样式，直接在工具栏窗口内部渲染，此函数已弃用
#[tauri::command]
pub async fn show_toolbar_menu(_app: AppHandle) -> Result<(), String> {
    // 菜单现在是工具栏内部的气泡菜单，不需要创建独立窗口
    Ok(())
}

#[tauri::command]
pub async fn toggle_dark_mode(_window: tauri::Window) -> Result<bool, String> {
    // Tauri 通过前端控制主题，这里只是示例
    Ok(true)
}

#[tauri::command]
pub async fn open_tool_window(app: AppHandle, tool_name: String) -> Result<(), String> {
    info!("🔧 打开工具窗口: {}", tool_name);
    
    match tool_name.as_str() {
        "main" => {
            if let Some(window) = app.get_webview_window("main") {
                windows::show_and_activate_window(&window);
            }
        }
        "vdd" => {
            if let Some(window) = app.get_webview_window("main") {
                windows::show_and_activate_window(&window);
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "about" => {
            windows::open_about_window(&app)?;
        }
        _ => {
            return Err(format!("未知的工具名称: {}", tool_name));
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn fetch_speech_phrases() -> Result<Vec<String>, String> {
    debug!("💬 开始获取话术配置");
    
    let url = "https://raw.githubusercontent.com/qiin2333/qiin.github.io/assets/speech-phrases.json";
    
    let response = reqwest::get(url).await
        .map_err(|e| format!("请求失败: {}", e))?;
    
    let phrases = response.json::<Vec<String>>().await
        .map_err(|e| format!("解析失败: {}", e))?;
    
    info!("✅ 话术加载成功，共 {} 条", phrases.len());
    Ok(phrases)
}

