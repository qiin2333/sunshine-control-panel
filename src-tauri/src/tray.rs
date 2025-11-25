use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, MouseButton, TrayIconEvent},
    Manager, AppHandle, Runtime, Emitter
};
use std::time::Duration;
use log::{info, warn, error, debug};
use crate::utils;
use crate::toolbar;
use crate::update;
use crate::windows;

/// 创建系统托盘
pub fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // === 导航类菜单 ===
    let open_website = MenuItem::with_id(app, "open_website", "打开官网", true, None::<&str>)?;
    
    // === 功能工具类菜单 ===
    let vdd_settings = MenuItem::with_id(app, "vdd_settings", "设置虚拟显示器（VDD）", true, None::<&str>)?;
    let show_toolbar = MenuItem::with_id(app, "show_toolbar", "显示工具栏", true, None::<&str>)?;
    let log_console = MenuItem::with_id(app, "log_console", "打开日志控制台", true, None::<&str>)?;
    
    // === 开发环境调试菜单 ===
    #[cfg(debug_assertions)]
    let debug_page = MenuItem::with_id(app, "debug_page", "🐛 打开调试页面", true, None::<&str>)?;
    
    // === 应用管理类菜单 ===
    let check_update = MenuItem::with_id(app, "check_update", "检查更新", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
    
    // === 退出类菜单 ===
    let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
    
    // === 分隔符 ===
    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    #[cfg(debug_assertions)]
    let separator_debug = PredefinedMenuItem::separator(app)?;
    
    // 构建菜单：按类别分组
    #[cfg(debug_assertions)]
    let menu = Menu::with_items(app, &[
        &open_website,
        &separator1,
        &vdd_settings,
        &show_toolbar,
        &log_console,
        &separator_debug,
        &debug_page,
        &separator2,
        &check_update,
        &about,
        &separator3,
        &quit,
    ])?;
    
    #[cfg(not(debug_assertions))]
    let menu = Menu::with_items(app, &[
        &open_website,
        &separator1,
        &vdd_settings,
        &show_toolbar,
        &log_console,
        &separator2,
        &check_update,
        &about,
        &separator3,
        &quit,
    ])?;
    
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            handle_tray_menu_event(app, event.id().as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                    handle_tray_click(tray.app_handle());
                }
                TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    handle_tray_double_click(tray.app_handle());
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

/// 处理托盘单击事件
pub fn handle_tray_click<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        if let Some(window) = app.get_webview_window("main") {
            let is_visible = window.is_visible().unwrap_or(false);
            let is_minimized = window.is_minimized().unwrap_or(false);
            let is_focused = window.is_focused().unwrap_or(false);
            
            debug!("📊 窗口状态: visible={}, minimized={}, focused={}", is_visible, is_minimized, is_focused);
            
            if is_visible && !is_minimized && is_focused {
                debug!("🔽 单击：隐藏窗口");
                let _ = window.hide();
            } else {
                debug!("🔼 单击：显示窗口");
                windows::show_and_activate_window(&window);
            }
        }
    });
}

/// 处理托盘双击事件
pub fn handle_tray_double_click<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        debug!("🔼🔼 双击托盘：强制显示窗口");
        windows::show_and_activate_window(&window);
    }
}

/// 处理托盘菜单事件
pub fn handle_tray_menu_event<R: Runtime>(app: &AppHandle<R>, menu_id: &str) {
    match menu_id {
        "open_website" => {
            info!("🌐 托盘菜单：打开官网");
            utils::open_url_in_browser("https://sunshine-foundation.vercel.app/");
        }
        "vdd_settings" => {
            open_vdd_settings(app);
        }
        "show_toolbar" => {
            toggle_toolbar(app);
        }
        "log_console" => {
            windows::open_log_console(app);
        }
        #[cfg(debug_assertions)]
        "debug_page" => {
            info!("🐛 托盘菜单：打开调试页面");
            windows::open_debug_page(app);
        }
        "check_update" => {
            check_for_updates(app);
        }
        "about" => {
            info!("ℹ️ 托盘菜单：显示关于对话框");
            let _ = windows::open_about_window(app);
        }
        "quit" => {
            info!("🚪 托盘菜单：退出应用");
            std::process::exit(0);
        }
        _ => {
            warn!("⚠️ 未知的托盘菜单事件: {}", menu_id);
        }
    }
}

/// 打开 VDD 设置
fn open_vdd_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        info!("📱 托盘菜单：打开VDD设置");
        windows::show_and_activate_window(&window);
        let _ = window.emit("open-vdd-settings", ());
    }
}

/// 切换工具栏显示/隐藏
fn toggle_toolbar<R: Runtime>(app: &AppHandle<R>) {
    info!("🔧 托盘菜单：切换工具栏显示/隐藏");
    if let Some(toolbar_window) = app.get_webview_window("toolbar") {
        let _ = toolbar_window.close();
    } else if let Err(e) = toolbar::create_toolbar_window_internal(app) {
        error!("❌ 创建工具栏失败: {}", e);
    }
}

/// 检查更新
fn check_for_updates<R: Runtime>(app: &AppHandle<R>) {
    info!("🔄 托盘菜单：检查更新");
    let app_handle = app.clone();
    
    // 确保主窗口可见
    if let Some(window) = app.get_webview_window("main") {
        windows::show_and_activate_window(&window);
    }
    
    tauri::async_runtime::spawn(async move {
        match update::check_for_updates_internal(false).await {
            Ok(Some(update_info)) => {
                info!("🎉 发现新版本: {}", update_info.version);
                save_update_check_time(&app_handle);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-available", &update_info);
                }
            }
            Ok(None) => {
                info!("✅ 已是最新版本");
                save_update_check_time(&app_handle);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-check-result", serde_json::json!({
                        "is_latest": true,
                        "message": "已是最新版本"
                    }));
                }
            }
            Err(e) => {
                error!("❌ 检查更新失败: {}", e);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-check-result", serde_json::json!({
                        "is_latest": false,
                        "error": e
                    }));
                }
            }
        }
    });
}

/// 保存更新检查时间
fn save_update_check_time<R: Runtime>(app: &AppHandle<R>) {
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    if let Some(prefs) = app.try_state::<Arc<Mutex<update::UpdatePreferences>>>() {
        let mut prefs = prefs.lock().unwrap();
        prefs.last_check_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

