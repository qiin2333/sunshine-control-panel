// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod vdd;
mod system;
mod sunshine;
mod utils;
mod proxy_server;
mod fs_utils;
mod toolbar;
mod update;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, MouseButton},
    Manager, WindowEvent, AppHandle, Runtime, Emitter
};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use url::Url;
use serde_json;

struct AppState {
    #[allow(dead_code)]
    main_window: Mutex<Option<tauri::Window>>,
}

// 注意：菜单现在是气泡样式，直接在工具栏窗口内部渲染，此函数已弃用
#[tauri::command]
async fn show_toolbar_menu(_app: AppHandle) -> Result<(), String> {
    // 菜单现在是工具栏内部的气泡菜单，不需要创建独立窗口
    Ok(())
}

#[tauri::command]
async fn toggle_dark_mode(_window: tauri::Window) -> Result<bool, String> {
    // Tauri 通过前端控制主题，这里只是示例
    Ok(true)
}


#[tauri::command]
async fn open_tool_window(app: AppHandle, tool_name: String) -> Result<(), String> {
    println!("🔧 打开工具窗口: {}", tool_name);
    
    match tool_name.as_str() {
        "main" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "vdd" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "about" => {
            open_about_window(&app)?;
        }
        _ => {
            return Err(format!("未知的工具名称: {}", tool_name));
        }
    }
    
    Ok(())
}

/// 打开关于窗口（单例模式）
fn open_about_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    const ABOUT_WINDOW_ID: &str = "about";
    
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        tauri::WebviewWindowBuilder::new(
            app,
            ABOUT_WINDOW_ID,
            tauri::WebviewUrl::App("about/index.html".into())
        )
        .title("关于 Sunshine Control Panel")
        .inner_size(540.0, 620.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(true)
        .decorations(true)
        .center()
        .build()
        .map_err(|e| format!("创建关于窗口失败: {}", e))?;
        
        println!("✅ 关于窗口创建成功");
    }
    
    Ok(())
}

#[tauri::command]
async fn fetch_speech_phrases() -> Result<Vec<String>, String> {
    println!("💬 开始获取话术配置");
    
    let url = "https://raw.githubusercontent.com/qiin2333/qiin.github.io/assets/speech-phrases.json";
    
    let response = reqwest::get(url).await
        .map_err(|e| format!("请求失败: {}", e))?;
    
    let phrases = response.json::<Vec<String>>().await
        .map_err(|e| format!("解析失败: {}", e))?;
    
    println!("✅ 话术加载成功，共 {} 条", phrases.len());
    Ok(phrases)
}

fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // === 导航类菜单 ===
    let open_website = MenuItem::with_id(app, "open_website", "打开官网", true, None::<&str>)?;
    
    // === 功能工具类菜单 ===
    let vdd_settings = MenuItem::with_id(app, "vdd_settings", "设置虚拟显示器（VDD）", true, None::<&str>)?;
    let show_toolbar = MenuItem::with_id(app, "show_toolbar", "显示工具栏", true, None::<&str>)?;
    
    // === 应用管理类菜单 ===
    let check_update = MenuItem::with_id(app, "check_update", "检查更新", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
    
    // === 退出类菜单 ===
    let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
    
    // === 分隔符 ===
    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    
    // 构建菜单：按类别分组
    let menu = Menu::with_items(app, &[
        &open_website,
        &separator1,
        &vdd_settings,
        &show_toolbar,
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
            use tauri::tray::TrayIconEvent;
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

fn handle_tray_click<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        if let Some(window) = app.get_webview_window("main") {
            let is_visible = window.is_visible().unwrap_or(false);
            let is_minimized = window.is_minimized().unwrap_or(false);
            let is_focused = window.is_focused().unwrap_or(false);
            
            println!("📊 窗口状态: visible={}, minimized={}, focused={}", is_visible, is_minimized, is_focused);
            
            if is_visible && !is_minimized && is_focused {
                println!("🔽 单击：隐藏窗口");
                let _ = window.hide();
            } else {
                println!("🔼 单击：显示窗口");
                if is_minimized {
                    let _ = window.unminimize();
                }
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

fn handle_tray_double_click<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        println!("🔼🔼 双击托盘：强制显示窗口");
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn handle_tray_menu_event<R: Runtime>(app: &AppHandle<R>, menu_id: &str) {
    match menu_id {
        "open_website" => {
            println!("🌐 托盘菜单：打开官网");
            utils::open_url_in_browser("https://sunshine-foundation.vercel.app/");
        }
        "vdd_settings" => {
            open_vdd_settings(app);
        }
        "show_toolbar" => {
            toggle_toolbar(app);
        }
        "check_update" => {
            check_for_updates(app);
        }
        "about" => {
            println!("ℹ️ 托盘菜单：显示关于对话框");
            let _ = open_about_window(app);
        }
        "quit" => {
            println!("🚪 托盘菜单：退出应用");
            std::process::exit(0);
        }
        _ => {
            println!("⚠️ 未知的托盘菜单事件: {}", menu_id);
        }
    }
}

/// 打开 VDD 设置
fn open_vdd_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        println!("📱 托盘菜单：打开VDD设置");
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("open-vdd-settings", ());
    }
}

/// 切换工具栏显示/隐藏
fn toggle_toolbar<R: Runtime>(app: &AppHandle<R>) {
    println!("🔧 托盘菜单：切换工具栏显示/隐藏");
    if let Some(toolbar_window) = app.get_webview_window("toolbar") {
        let _ = toolbar_window.close();
    } else if let Err(e) = toolbar::create_toolbar_window_internal(app) {
        eprintln!("❌ 创建工具栏失败: {}", e);
    }
}

/// 检查更新
fn check_for_updates<R: Runtime>(app: &AppHandle<R>) {
    println!("🔄 托盘菜单：检查更新");
    let app_handle = app.clone();
    
    // 确保主窗口可见
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    
    tauri::async_runtime::spawn(async move {
        use crate::update;
        match update::check_for_updates_internal(false).await {
            Ok(Some(update_info)) => {
                println!("🎉 发现新版本: {}", update_info.version);
                save_update_check_time(&app_handle);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-available", &update_info);
                }
            }
            Ok(None) => {
                println!("✅ 已是最新版本");
                save_update_check_time(&app_handle);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-check-result", serde_json::json!({
                        "is_latest": true,
                        "message": "已是最新版本"
                    }));
                }
            }
            Err(e) => {
                eprintln!("❌ 检查更新失败: {}", e);
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
    if let Some(prefs) = app.try_state::<Arc<Mutex<update::UpdatePreferences>>>() {
        let mut prefs = prefs.lock().unwrap();
        prefs.last_check_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

fn main() {
    // 设置环境变量以忽略证书错误
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--ignore-certificate-errors");
    }
    
    tauri::Builder::default()
        .manage(AppState {
            main_window: Mutex::new(None),
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_single_instance(app, args);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            setup_application(app)
        })
        .on_window_event(|window, event| {
            handle_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            toggle_dark_mode,
            show_toolbar_menu,
            toolbar::handle_toolbar_menu_action,
            toolbar::save_toolbar_position,
            system::get_current_dpi,
            system::set_desktop_dpi,
            open_tool_window,
            toolbar::create_toolbar_window,
            fetch_speech_phrases,
            vdd::get_vdd_settings_file_path,
            vdd::get_vdd_tools_dir_path,
            vdd::load_vdd_settings,
            vdd::save_vdd_settings,
            vdd::exec_pipe_cmd,
            system::get_gpus,
            system::get_system_info,
            sunshine::get_sunshine_install_path,
            sunshine::get_sunshine_version,
            sunshine::parse_sunshine_config,
            sunshine::get_sunshine_url,
            sunshine::get_command_line_url,
            sunshine::get_active_sessions,
            sunshine::change_bitrate,
            utils::open_external_url,
            utils::restart_graphics_driver,
            utils::restart_sunshine_service,
            utils::restart_as_admin,
            utils::is_running_as_admin,
            vdd::uninstall_vdd_driver,
            fs_utils::get_icc_file_list,
            fs_utils::read_directory,
            fs_utils::read_image_as_data_url,
            fs_utils::copy_image_to_assets,
            fs_utils::cleanup_unused_covers,
            update::check_for_updates,
            update::download_update,
            update::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 处理单实例逻辑
fn handle_single_instance(app: &tauri::AppHandle, args: Vec<String>) {
    println!("🔔 检测到第二个实例启动，激活现有窗口");
    
    if !args.is_empty() {
        println!("   启动参数: {:?}", args);
    }
    
    // 检查是否要打开工具栏
    if args.iter().any(|arg| arg == "--toolbar" || arg == "-t") {
        println!("🔧 检测到 --toolbar 参数，打开工具栏");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_clone) {
                eprintln!("❌ 创建工具栏失败: {}", e);
            }
        });
        return;
    }
    
    // 提取 URL 参数
    let target_url = args.iter()
        .find(|arg| arg.starts_with("--url="))
        .map(|arg| arg.trim_start_matches("--url=").to_string());
    
    if let Some(url) = &target_url {
        println!("📍 检测到 URL 参数: {}", url);
    }
    
    // 激活主窗口
    activate_main_window(app, target_url);
}

/// 激活主窗口
fn activate_main_window(app: &tauri::AppHandle, target_url: Option<String>) {
    let Some(window) = app.get_webview_window("main") else {
        println!("❌ 未找到主窗口 'main'");
        return;
    };
    
    println!("📱 正在激活主窗口...");
    
    // 获取窗口状态
    let is_visible = window.is_visible().unwrap_or(false);
    let is_minimized = window.is_minimized().unwrap_or(false);
    
    println!("   当前状态: visible={}, minimized={}", is_visible, is_minimized);
    
    // 恢复窗口状态
    if is_minimized {
        let _ = window.unminimize();
        println!("   ✅ 已取消最小化");
    }
    
    if !is_visible {
        let _ = window.show();
        println!("   ✅ 已显示窗口");
    }
    
    let _ = window.set_focus();
    println!("   ✅ 已聚焦窗口");
    
    // 处理 URL 导航
    if let Some(url) = target_url {
        navigate_to_url(&window, &url);
    }
    
    // 短暂置顶以强制显示在前台
    let _ = window.set_always_on_top(true);
    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = window_clone.set_always_on_top(false);
    });
    
    println!("✅ 窗口激活完成");
}

/// 导航到指定 URL
fn navigate_to_url(window: &tauri::WebviewWindow, url: &str) {
    println!("🔄 正在导航到: {}", url);
    
    let Ok(parsed_url) = Url::parse(url) else {
        println!("❌ URL 解析失败: {}", url);
        return;
    };
    
    let path = format!(
        "{}{}",
        parsed_url.path(),
        parsed_url.query().map(|q| format!("?{}", q)).unwrap_or_default()
    );
    
    let script = format!(
        r#"
        (function() {{
            const iframe = document.querySelector('.sunshine-iframe');
            if (iframe && iframe.contentWindow) {{
                iframe.src = 'http://localhost:48081{}';
                console.log('📍 导航到:', '{}');
            }}
        }})();
        "#,
        path, path
    );
    
    let _ = window.eval(&script);
    println!("✅ 已发送导航命令");
}

/// 应用程序初始化设置
fn setup_application(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 检查命令行参数（首次启动时）
    let args: Vec<String> = std::env::args().collect();
    let show_toolbar = args.iter().any(|arg| arg == "--toolbar" || arg == "-t");
    
    // 创建系统托盘
    create_system_tray(&app.handle())?;
    
    // 注册全局快捷键
    register_global_shortcuts(app)?;
    
    // 设置全局菜单事件处理
    setup_menu_event_handler(app);

    // 启动代理服务器
    start_proxy_server_async();
    
    // 如果指定了 --toolbar 参数，延迟打开工具栏（等待应用完全初始化）
    if show_toolbar {
        println!("🔧 检测到 --toolbar 参数，将在应用启动后打开工具栏");
        let app_handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_handle) {
                eprintln!("❌ 创建工具栏失败: {}", e);
            }
        });
    }
    
    // 推迟初始化更新检查器
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if let Err(e) = update::init_update_checker(&app_handle) {
            eprintln!("❌ 初始化更新检查器失败: {}", e);
        }
    });
    
    Ok(())
}

/// 注册全局快捷键
fn register_global_shortcuts(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    
    let app_handle = app.handle().clone();
    
    app.handle().global_shortcut().on_shortcut("CmdOrCtrl+Shift+Alt+T", move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("⌨️ 全局快捷键触发: CTRL+SHIFT+ALT+T");
            toggle_toolbar_window(&app_handle);
        }
    })?;
    
    println!("⌨️ 全局快捷键已注册: CTRL+SHIFT+ALT+T");
    Ok(())
}

/// 切换工具栏窗口显示/隐藏
fn toggle_toolbar_window(app_handle: &tauri::AppHandle) {
    if let Some(toolbar_window) = app_handle.get_webview_window("toolbar") {
        println!("🔧 工具栏已存在，关闭");
        let _ = toolbar_window.close();
    } else {
        println!("🔧 工具栏不存在，创建");
        let app_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_clone) {
                eprintln!("❌ 快捷键创建工具栏失败: {}", e);
            }
        });
    }
}

/// 设置全局菜单事件处理器
fn setup_menu_event_handler(app: &mut tauri::App) {
    let app_handle = app.handle().clone();
    app.handle().on_menu_event(move |_app, event| {
        let event_id = event.id().as_ref();
        if event_id.starts_with("toolbar_") {
            println!("🔧 全局菜单事件: {:?}", event.id());
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
                println!("🎯 Sunshine URL: {}", url);
                let base_url = url.trim_end_matches('/').to_string();
                proxy_server::set_sunshine_target(base_url);
            }
            Err(e) => {
                eprintln!("⚠️  无法获取 Sunshine URL，使用默认: {}", e);
            }
        }
        
        // 启动代理服务器
        if let Err(e) = proxy_server::start_proxy_server().await {
            eprintln!("❌ 代理服务器启动失败: {}", e);
        }
    });
}

/// 处理窗口事件
fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        match window.label() {
            "main" => {
                // 主窗口隐藏而不是关闭
                api.prevent_close();
                let _ = window.hide();
            }
            "toolbar" => {
                // 工具栏窗口关闭前保存位置
                if let Ok(position) = window.outer_position() {
                    let app = window.app_handle();
                    toolbar::save_toolbar_position_internal(&app, position.x as f64, position.y as f64);
                }
            }
            _ => {
                // 其他窗口正常关闭
            }
        }
    }
}
