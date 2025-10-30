// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod vdd;
mod system;
mod sunshine;
mod utils;
mod proxy_server;
mod fs_utils;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, MouseButton},
    Manager, WindowEvent, AppHandle, Runtime, Emitter
};
use std::sync::Mutex;
use std::time::Duration;

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
async fn handle_toolbar_menu_action(app: AppHandle, action: String) -> Result<(), String> {
    println!("🔧 处理菜单操作: {}", action);
    handle_toolbar_menu_event(&app, &action);
    Ok(())
}

#[tauri::command]
async fn toggle_dark_mode(_window: tauri::Window) -> Result<bool, String> {
    // Tauri 通过前端控制主题，这里只是示例
    Ok(true)
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<bool, String> {
    if url.starts_with("http") {
        // Tauri 1.5 的 shell::open API 不需要 Scope
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(&["/c", "start", &url])
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn open_tool_window(app: AppHandle, tool_name: String) -> Result<(), String> {
    println!("🔧 打开工具窗口: {}", tool_name);
    
    match tool_name.as_str() {
        "main" => {
            // 打开主窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "vdd" => {
            // 打开 VDD 设置窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "about" => {
            // 打开关于窗口
            const ABOUT_WINDOW_ID: &str = "about";
            
            if let Some(window) = app.get_webview_window(ABOUT_WINDOW_ID) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                match tauri::WebviewWindowBuilder::new(
                    &app,
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
                {
                    Ok(_) => println!("✅ 关于窗口创建成功"),
                    Err(e) => eprintln!("❌ 创建关于窗口失败: {}", e),
                }
            }
        }
        _ => {
            return Err(format!("未知的工具名称: {}", tool_name));
        }
    }
    
    Ok(())
}

// 为工具栏窗口创建右键菜单
fn create_toolbar_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let main_panel = MenuItem::with_id(app, "toolbar_main", "控制面板", true, None::<&str>)?;
    let vdd_settings = MenuItem::with_id(app, "toolbar_vdd", "虚拟显示器 (VDD)", true, None::<&str>)?;
    let dpi_adjuster = MenuItem::with_id(app, "toolbar_dpi", "调整 DPI", true, None::<&str>)?;
    let bitrate = MenuItem::with_id(app, "toolbar_bitrate", "码率调整", true, None::<&str>)?;
    let close_toolbar = MenuItem::with_id(app, "toolbar_close", "关闭工具栏", true, None::<&str>)?;
    
    Menu::with_items(app, &[
        &main_panel,
        &vdd_settings,
        &dpi_adjuster,
        &bitrate,
        &close_toolbar,
    ])
}

// 辅助函数：创建工具窗口
fn create_tool_window_internal<R: Runtime>(app: &AppHandle<R>, tool_type: &str) {
    const TOOL_WINDOW_ID: &str = "tool_window";
    
    // 如果窗口已存在，先关闭它
    if let Some(window) = app.get_webview_window(TOOL_WINDOW_ID) {
        let _ = window.close();
    }
    
    // 创建工具窗口，通过 URL 参数传递工具类型
    let url = format!("tool-window/index.html?tool={}", tool_type);
    let title = format!("ZakoToolsWindow - {}", tool_type);
    println!("🔧 创建工具窗口 URL: {}", url);
    
    match tauri::WebviewWindowBuilder::new(
        app,
        TOOL_WINDOW_ID,
        tauri::WebviewUrl::App(url.into())
    )
    .title(&title)
    .fullscreen(true)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)  // 先隐藏，避免闪白
    .build()
    {
        Ok(window) => {
            // 等待一小段时间让内容加载，然后显示窗口
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let _ = window.show();
            });
        }
        Err(e) => {
            eprintln!("❌ 创建工具窗口失败: {}", e);
        }
    }
}

// 处理工具栏菜单事件
fn handle_toolbar_menu_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    match event_id {
        "main" | "toolbar_main" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "vdd" | "toolbar_vdd" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "dpi" | "toolbar_dpi" => {
            create_tool_window_internal(app, "dpi");
        }
        "bitrate" | "toolbar_bitrate" => {
            create_tool_window_internal(app, "bitrate");
        }
        "close" | "toolbar_close" => {
            if let Some(window) = app.get_webview_window("toolbar") {
                let _ = window.close();
            }
        }
        _ => {}
    }
}

// 内部泛型函数，用于创建工具栏窗口
fn create_toolbar_window_internal<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    const TOOLBAR_WINDOW_ID: &str = "toolbar";
    
    // 检查工具栏窗口是否已存在
    if app.get_webview_window(TOOLBAR_WINDOW_ID).is_some() {
        println!("🔧 工具栏窗口已存在");
        return Ok(());
    }
    
    println!("🔧 创建工具栏窗口");
    
    // 窗口大小和边距配置
    let toolbar_size = 240.0;  // 窗口大小（正方形，包含气泡菜单空间）
    let margin = 20.0;         // 距离屏幕边缘的边距
    
    // 先创建窗口在默认位置
    let window = match tauri::WebviewWindowBuilder::new(
        app,
        TOOLBAR_WINDOW_ID,
        tauri::WebviewUrl::App("toolbar/index.html".into())
    )
    .title("工具栏")
    .inner_size(toolbar_size, toolbar_size)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)  // 先隐藏，等设置好位置再显示
    .build()
    {
        Ok(win) => win,
        Err(e) => {
            eprintln!("❌ 创建工具栏窗口失败: {}", e);
            return Err(format!("创建工具栏窗口失败: {}", e));
        }
    };
    
    // 获取主显示器信息并设置到右下角
    if let Ok(monitor) = window.current_monitor() {
        if let Some(monitor) = monitor {
            let size = monitor.size();
            let scale_factor = monitor.scale_factor();
            
            // 计算逻辑像素尺寸
            let screen_width = size.width as f64 / scale_factor;
            let screen_height = size.height as f64 / scale_factor;
            
            // 计算右下角位置（考虑任务栏）
            let x = screen_width - toolbar_size - margin - 60.0;
            let y = screen_height - toolbar_size - margin - 80.0;
            
            println!("📍 屏幕尺寸: {}x{}, 缩放: {}, 工具栏位置: ({}, {})", 
                     screen_width, screen_height, scale_factor, x, y);
            
            // 设置位置
            if let Err(e) = window.set_position(tauri::PhysicalPosition::new(
                (x * scale_factor) as i32,
                (y * scale_factor) as i32
            )) {
                eprintln!("❌ 设置工具栏位置失败: {}", e);
            }
        }
    }
    
    // 显示窗口
    if let Err(e) = window.show() {
        eprintln!("❌ 显示工具栏窗口失败: {}", e);
    }
    
    println!("✅ 工具栏窗口创建成功");
    Ok(())
}

#[tauri::command]
async fn create_toolbar_window(app: AppHandle) -> Result<(), String> {
    create_toolbar_window_internal(&app)
}

fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open_website = MenuItem::with_id(app, "open_website", "打开官网", true, None::<&str>)?;
    let vdd_settings = MenuItem::with_id(app, "vdd_settings", "设置虚拟显示器（VDD）", true, None::<&str>)?;
    let show_toolbar = MenuItem::with_id(app, "show_toolbar", "显示工具栏", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
    
    let menu = Menu::with_items(app, &[
        &open_website,
        &vdd_settings,
        &show_toolbar,
        &about,
        &quit,
    ])?;
    
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .show_menu_on_left_click(false)  // 左键点击不显示菜单
        .on_menu_event(move |app, event| {
            handle_tray_menu_event(app, event.id().as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::TrayIconEvent;
            match event {
                // 单击托盘图标 - 显示/隐藏窗口
                TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                    handle_tray_click(tray.app_handle());
                }
                // 双击托盘图标 - 确保显示窗口
                TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    handle_tray_double_click(tray.app_handle());
                }
                // 右键点击托盘图标 - 显示菜单
                TrayIconEvent::Click { button: MouseButton::Right, .. } => {
                    // 右键点击显示菜单（默认行为）
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

fn handle_tray_click<R: Runtime>(app: &AppHandle<R>) {
    // 使用 tokio 延迟处理，避免与双击冲突
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // 延迟 200ms，如果在此期间发生双击，则会被双击事件覆盖
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        if let Some(window) = app.get_webview_window("main") {
            // 获取窗口的实际状态
            let is_visible = window.is_visible().unwrap_or(false);
            let is_minimized = window.is_minimized().unwrap_or(false);
            let is_focused = window.is_focused().unwrap_or(false);
            
            println!("📊 窗口状态: visible={}, minimized={}, focused={}", is_visible, is_minimized, is_focused);
            
            if is_visible && !is_minimized && is_focused {
                // 窗口当前可见、未最小化且有焦点 -> 隐藏
                println!("🔽 单击：隐藏窗口");
                let _ = window.hide();
            } else {
                // 其他情况 -> 显示并聚焦
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
    // 双击始终立即显示窗口（不等待延迟）
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
            // 使用外部浏览器打开官网
            println!("🌐 托盘菜单：打开官网");
            
            // 使用系统命令直接打开外部浏览器
            let website_url = "https://sunshine-foundation.vercel.app/";
            
            tauri::async_runtime::spawn(async move {
                println!("🌐 正在打开外部浏览器...");
                
                #[cfg(target_os = "windows")]
                {
                    match std::process::Command::new("cmd")
                        .args(&["/c", "start", "", website_url])
                        .spawn() 
                    {
                        Ok(_) => {
                            println!("✅ 已在外部浏览器中打开官网: {}", website_url);
                        }
                        Err(e) => {
                            eprintln!("❌ 打开官网失败: {}", e);
                        }
                    }
                }
                
                #[cfg(not(target_os = "windows"))]
                {
                    match std::process::Command::new("xdg-open")
                        .arg(website_url)
                        .spawn()
                    {
                        Ok(_) => {
                            println!("✅ 已在外部浏览器中打开官网: {}", website_url);
                        }
                        Err(e) => {
                            eprintln!("❌ 打开官网失败: {}", e);
                        }
                    }
                }
            });
        }
        "vdd_settings" => {
            // 首先确保主窗口可见并聚焦
            if let Some(window) = app.get_webview_window("main") {
                println!("📱 托盘菜单：打开VDD设置");
                
                // 显示并聚焦主窗口
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                
                // 发送事件到前端，让它在主窗口中打开VDD设置
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "show_toolbar" => {
            println!("🔧 托盘菜单：显示工具栏");
            if let Err(e) = create_toolbar_window_internal(app) {
                eprintln!("❌ 显示工具栏失败: {}", e);
            }
        }
        "about" => {
            println!("ℹ️ 托盘菜单：显示关于对话框");
            
            // 使用单例窗口
            let app_handle = app.clone();
            const ABOUT_WINDOW_ID: &str = "about";
            
            // 检查窗口是否已存在
            if let Some(window) = app_handle.get_webview_window(ABOUT_WINDOW_ID) {
                // 窗口已存在，聚焦并显示
                println!("📱 关于窗口已存在，激活窗口");
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                // 窗口不存在，创建新窗口
                match tauri::WebviewWindowBuilder::new(
                    &app_handle,
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
                {
                    Ok(_window) => {
                        println!("✅ 关于窗口创建成功");
                    }
                    Err(e) => {
                        eprintln!("❌ 创建关于窗口失败: {}", e);
                    }
                }
            }
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
            // 当检测到第二个实例启动时，显示并聚焦已有的主窗口
            println!("🔔 检测到第二个实例启动，激活现有窗口");
            
            if !args.is_empty() {
                println!("   启动参数: {:?}", args);
            }
            
            // 激活主窗口的函数
            let activate_window = || {
                if let Some(window) = app.get_webview_window("main") {
                    println!("📱 正在激活主窗口...");
                    
                    // 检查窗口当前状态
                    match (window.is_visible(), window.is_minimized()) {
                        (Ok(is_visible), Ok(is_minimized)) => {
                            println!("   当前状态: visible={}, minimized={}", is_visible, is_minimized);
                            
                            // 处理最小化状态
                            if is_minimized {
                                let _ = window.unminimize();
                                println!("   ✅ 已取消最小化");
                            }
                            
                            // 处理可见性
                            if !is_visible {
                                let _ = window.show();
                                println!("   ✅ 已显示窗口");
                            }
                            
                            // 聚焦窗口
                            let _ = window.set_focus();
                            println!("   ✅ 已聚焦窗口");
                            
                            // 短暂置顶以强制显示在前台
                            let window_clone = window.clone();
                            let _ = window.set_always_on_top(true);
                            
                            // 异步重置置顶状态
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                let _ = window_clone.set_always_on_top(false);
                            });
                            
                            println!("✅ 窗口激活完成");
                            true
                        }
                        (Err(e1), Err(e2)) => {
                            println!("❌ 无法获取窗口状态: visible={:?}, minimized={:?}", e1, e2);
                            false
                        }
                        _ => {
                            println!("⚠️  部分窗口状态获取失败，尝试直接激活");
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                            true
                        }
                    }
                } else {
                    println!("❌ 未找到主窗口 'main'");
                    false
                }
            };
            
            // 执行激活
            activate_window();
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 创建系统托盘
            create_system_tray(&app.handle())?;
            
            // 注册全局快捷键 CTRL+SHIFT+ALT+T 显示工具栏
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                use tauri_plugin_global_shortcut::ShortcutState;
                
                let app_handle = app.handle().clone();
                
                app.handle().global_shortcut().on_shortcut("CmdOrCtrl+Shift+Alt+T", move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        println!("⌨️ 全局快捷键触发: CTRL+SHIFT+ALT+T");
                        
                        // 切换工具栏显示/隐藏
                        if let Some(toolbar_window) = app_handle.get_webview_window("toolbar") {
                            // 工具栏已存在，关闭它
                            println!("🔧 工具栏已存在，关闭");
                            let _ = toolbar_window.close();
                        } else {
                            // 工具栏不存在，创建它
                            println!("🔧 工具栏不存在，创建");
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = create_toolbar_window_internal(&app_clone) {
                                    eprintln!("❌ 快捷键创建工具栏失败: {}", e);
                                }
                            });
                        }
                    }
                })?;
                
                println!("⌨️ 全局快捷键已注册: CTRL+SHIFT+ALT+T");
            }
            
            // 设置全局菜单事件处理
            let app_handle = app.handle().clone();
            app.handle().on_menu_event(move |_app, event| {
                let event_id = event.id().as_ref();
                if event_id.starts_with("toolbar_") {
                    println!("🔧 全局菜单事件: {:?}", event.id());
                    handle_toolbar_menu_event(&app_handle, event_id);
                }
            });
            
            // 获取 Sunshine URL 并配置代理目标
            tauri::async_runtime::spawn(async {
                // 尝试获取 Sunshine URL
                match sunshine::get_sunshine_url().await {
                    Ok(url) => {
                        println!("🎯 Sunshine URL: {}", url);
                        // 移除尾部的 /
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
            
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                    // 关闭时隐藏而不是退出
                    window.hide().unwrap();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            toggle_dark_mode,
            open_external_url,
            show_toolbar_menu,
            handle_toolbar_menu_action,
            system::get_current_dpi,
            system::set_desktop_dpi,
            open_tool_window,
            create_toolbar_window,
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
            sunshine::get_sunshine_proxy_url,
            sunshine::get_command_line_url,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
