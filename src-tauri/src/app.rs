use tauri::{Manager, App, AppHandle};
use log::{info, error, debug};
use crate::toolbar;
use crate::windows;
use crate::tray;
use crate::sunshine;
use crate::proxy_server;
use crate::update;

/// 应用程序状态
pub struct AppState {
    #[allow(dead_code)]
    pub main_window: std::sync::Mutex<Option<tauri::Window>>,
}

/// 应用程序初始化设置
pub fn setup_application(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let show_toolbar = std::env::args().any(|arg| arg == "--toolbar" || arg == "-t");
    let app_handle = app.handle().clone();
    
    windows::create_main_window(&app_handle)?;
    tray::create_system_tray(&app_handle)?;
    register_global_shortcuts(app)?;
    setup_menu_event_handler(app);
    start_proxy_server_async();
    
    // 延迟任务：工具栏和更新检查
    tauri::async_runtime::spawn(async move {
        if show_toolbar {
            info!("🔧 检测到 --toolbar 参数，将在应用启动后打开工具栏");
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_handle) {
                error!("❌ 创建工具栏失败: {}", e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if let Err(e) = update::init_update_checker(&app_handle) {
            error!("❌ 初始化更新检查器失败: {}", e);
        }
    });
    
    Ok(())
}

/// 注册全局快捷键
fn register_global_shortcuts(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    
    let app_handle = app.handle().clone();
    
    app.handle().global_shortcut().on_shortcut("CmdOrCtrl+Shift+Alt+T", move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            debug!("⌨️ 全局快捷键触发: CTRL+SHIFT+ALT+T");
            toggle_toolbar_window(&app_handle);
        }
    })?;
    
    info!("⌨️ 全局快捷键已注册: CTRL+SHIFT+ALT+T");
    Ok(())
}

/// 切换工具栏窗口显示/隐藏
fn toggle_toolbar_window(app_handle: &AppHandle) {
    if let Some(toolbar_window) = app_handle.get_webview_window("toolbar") {
        debug!("🔧 工具栏已存在，关闭");
        let _ = toolbar_window.close();
    } else {
        debug!("🔧 工具栏不存在，创建");
        let app_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_clone) {
                error!("❌ 快捷键创建工具栏失败: {}", e);
            }
        });
    }
}

/// 设置全局菜单事件处理器
fn setup_menu_event_handler(app: &mut App) {
    let app_handle = app.handle().clone();
    app.handle().on_menu_event(move |_app, event| {
        let event_id = event.id().as_ref();
        if event_id.starts_with("toolbar_") {
            debug!("🔧 全局菜单事件: {:?}", event.id());
            toolbar::handle_toolbar_menu_event(&app_handle, event_id);
        }
    });
}

/// 异步启动代理服务器
fn start_proxy_server_async() {
    tauri::async_runtime::spawn(async {
        // 获取 Sunshine URL 并配置代理目标
        match sunshine::get_sunshine_url().await {
            Ok(url) => {
                info!("🎯 Sunshine URL: {}", url);
                let base_url = url.trim_end_matches('/').to_string();
                proxy_server::set_sunshine_target(base_url);
            }
            Err(e) => {
                log::warn!("⚠️  无法获取 Sunshine URL，使用默认: {}", e);
            }
        }
        
        // 启动代理服务器
        if let Err(e) = proxy_server::start_proxy_server().await {
            error!("❌ 代理服务器启动失败: {}", e);
        }
    });
}

/// 处理单实例逻辑
pub fn handle_single_instance(app: &AppHandle, args: Vec<String>) {
    info!("🔔 检测到第二个实例启动，激活现有窗口");
    debug!("   启动参数: {:?}", args);
    
    // 检查是否要打开工具栏
    if args.iter().any(|arg| arg == "--toolbar" || arg == "-t") {
        info!("🔧 检测到 --toolbar 参数，打开工具栏");
        toggle_toolbar_window(app);
        return;
    }
    
    // 提取 URL 参数并激活主窗口
    let target_url = args.iter()
        .find(|arg| arg.starts_with("--url="))
        .map(|arg| arg.trim_start_matches("--url=").to_string());
    
    if let Some(url) = &target_url {
        info!("📍 检测到 URL 参数: {}", url);
    }
    
    windows::activate_main_window(app, target_url);
}
