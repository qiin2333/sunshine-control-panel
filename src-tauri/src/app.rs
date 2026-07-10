use crate::proxy_server;
use crate::sunshine;
use crate::toolbar;
use crate::tray;
use crate::update;
use crate::windows;
use log::{debug, error, info, warn};
use tauri::{App, AppHandle, Manager};

/// 应用程序状态
pub struct AppState {
    #[allow(dead_code)]
    pub main_window: std::sync::Mutex<Option<tauri::Window>>,
}

/// 应用程序初始化设置
pub fn setup_application(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let show_toolbar = args.iter().any(|arg| arg == "--toolbar" || arg == "-t");
    let show_desktop = args.iter().any(|arg| arg == "--desktop" || arg == "-d");
    let agent_only = args.iter().any(|arg| arg == "--hidden");
    let explicit_minimized = args.iter().any(|arg| arg == "--minimized");
    let desktop_settings = crate::desktop_settings::load_desktop_settings_from_disk();
    let send_to_client_paths = crate::file_transfer::parse_send_to_client_args(&args);
    let is_send_to_client = !send_to_client_paths.is_empty();
    let quick_share_folder_paths = crate::file_mapping::parse_quick_share_folder_args(&args);
    let is_quick_share_folder = !quick_share_folder_paths.is_empty();
    let url_contains_pin = args
        .iter()
        .find(|arg| arg.starts_with("--url="))
        .map_or(false, |arg| arg.contains("/pin"));

    let app_handle = app.handle().clone();
    windows::register_agent_restart();
    crate::desktop_settings::apply_startup_settings(&app_handle, &desktop_settings);
    let start_minimized = explicit_minimized || desktop_settings.start_minimized;

    // Agent-only startup keeps tray and session services alive without
    // allocating a hidden WebView. UI windows are created on demand.
    let main_window_created = if agent_only {
        info!("Starting Sunshine user agent without WebView windows");
        false
    } else if show_desktop {
        info!("🖥️ 检测到 --desktop 参数，启动桌面 UI 模式");
        if start_minimized {
            windows::create_desktop_window_hidden(&app_handle)?;
        } else {
            windows::create_desktop_window(&app_handle)?;
        }
        windows::create_main_window_hidden(&app_handle)?;
        false
    } else if !show_toolbar && !url_contains_pin && !is_send_to_client && !is_quick_share_folder {
        if start_minimized {
            windows::create_main_window_hidden(&app_handle)?;
        } else {
            windows::create_main_window(&app_handle)?;
        }
        true
    } else {
        false
    };

    tray::create_system_tray(&app_handle)?;
    #[cfg(target_os = "windows")]
    if let Err(e) = crate::shell_context_menu::install_file_transfer_menu() {
        warn!("⚠️  安装文件传输右键菜单失败: {}", e);
    }
    #[cfg(target_os = "windows")]
    if desktop_settings.file_mapping_menu_enabled {
        if let Err(e) = crate::shell_context_menu::install_file_mapping_menu() {
            warn!("install file mapping context menu failed: {}", e);
        }
    }
    register_global_shortcuts(app)?;
    setup_menu_event_handler(app);

    // 剪贴板同步：用户会话 agent 默认随面板启动；服务端如果禁用了则 SSE 自然失败，
    // 不需要额外开关。
    crate::clipboard::auto_start();

    if is_send_to_client {
        crate::file_transfer::dispatch_cli_send(send_to_client_paths);
    }
    if is_quick_share_folder {
        crate::file_mapping::dispatch_cli_quick_share(quick_share_folder_paths);
    }

    // 启动 WebView 心跳监控（检测渲染进程崩溃并自动恢复）
    windows::start_heartbeat_monitor(app.handle().clone());

    // 延迟任务
    tauri::async_runtime::spawn(async move {
        update::emit_update_result_if_requested(&app_handle);

        // PIN 配对窗口
        if url_contains_pin {
            info!("🔐 将在应用启动后打开 PIN 配对窗口");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Err(e) = windows::open_pin_window(&app_handle) {
                error!("❌ 创建 PIN 配对窗口失败: {}", e);
            }
        }

        // 工具栏窗口（非桌面模式下）
        if show_toolbar && !show_desktop {
            info!("🔧 将在应用启动后打开工具栏");
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            if let Err(e) = toolbar::create_toolbar_window_internal(&app_handle) {
                error!("❌ 创建工具栏失败: {}", e);
            }
        }

        // 更新检查（仅在主窗口启动时检查）
        if main_window_created || show_desktop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if let Err(e) = update::init_update_checker(&app_handle) {
                error!("❌ 初始化更新检查器失败: {}", e);
            }
        }
    });

    Ok(())
}

pub fn shutdown_application() {
    crate::clipboard::stop();
    #[cfg(target_os = "windows")]
    crate::tray::cleanup_prevent_sleep();
    crate::moonlight_web::cleanup();
    crate::windows::unregister_agent_restart();
}

/// 注册全局快捷键
fn register_global_shortcuts(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let app_handle = app.handle().clone();

    match app.handle().global_shortcut().on_shortcut(
        "CmdOrCtrl+Shift+Alt+T",
        move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                debug!("⌨️ 全局快捷键触发: CTRL+SHIFT+ALT+T");
                toggle_toolbar_window(&app_handle);
            }
        },
    ) {
        Ok(_) => {
            info!("⌨️ 全局快捷键已注册: CTRL+SHIFT+ALT+T");
        }
        Err(e) => {
            log::warn!(
                "⚠️  全局快捷键 CTRL+SHIFT+ALT+T 注册失败（可能已被其他程序占用）: {}",
                e
            );
            log::warn!("⚠️  工具栏快捷键不可用，但应用程序将继续正常运行");
        }
    }

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

/// 处理单实例逻辑
pub fn handle_single_instance(app: &AppHandle, args: Vec<String>) {
    info!("🔔 检测到第二个实例启动，激活现有窗口");
    info!("   启动参数: {:?}", args);

    let send_to_client_paths = crate::file_transfer::parse_send_to_client_args(&args);
    if !send_to_client_paths.is_empty() {
        info!("📤 检测到文件传输右键菜单请求");
        crate::file_transfer::dispatch_cli_send(send_to_client_paths);
        return;
    }

    // 诊断：列出当前所有窗口
    let quick_share_folder_paths = crate::file_mapping::parse_quick_share_folder_args(&args);
    if !quick_share_folder_paths.is_empty() {
        info!("file mapping quick share request detected");
        crate::file_mapping::dispatch_cli_quick_share(quick_share_folder_paths);
        return;
    }

    if args.iter().any(|arg| arg == "--hidden") {
        info!("User agent is already running; ignoring duplicate hidden startup");
        return;
    }

    let windows: Vec<_> = app.webview_windows().keys().cloned().collect();
    info!("📋 当前存在的窗口: {:?}", windows);

    // 检查是否要打开桌面 UI
    if args.iter().any(|arg| arg == "--desktop" || arg == "-d") {
        info!("🖥️ 检测到 --desktop 参数，打开桌面 UI");
        if let Err(e) = windows::open_desktop_window(app) {
            error!("❌ 打开桌面 UI 失败: {}", e);
        }
        return;
    }

    // 检查是否要打开工具栏
    if args.iter().any(|arg| arg == "--toolbar" || arg == "-t") {
        info!("🔧 检测到 --toolbar 参数，打开工具栏");
        toggle_toolbar_window(app);
        return;
    }

    // 提取 URL 参数并激活主窗口
    let target_url = args
        .iter()
        .find(|arg| arg.starts_with("--url="))
        .map(|arg| arg.trim_start_matches("--url=").to_string());

    if let Some(url) = &target_url {
        info!("📍 检测到 URL 参数: {}", url);

        // 检测 URL 中是否包含 /pin 路径
        match sunshine::set_runtime_sunshine_url(url) {
            Ok(base_url) => {
                proxy_server::set_sunshine_target(base_url);
                proxy_server::reset_fast_fail();
            }
            Err(e) => warn!("Invalid Sunshine URL from second instance: {}", e),
        }

        if url.contains("/pin") {
            info!("🔐 检测到 /pin 路径，打开 PIN 配对窗口");
            if let Err(e) = windows::open_pin_window(app) {
                error!("❌ 打开 PIN 窗口失败: {}", e);
            }
            return;
        }
    }

    if app.get_webview_window("main").is_none() {
        if let Err(e) = windows::create_main_window(app) {
            error!("Failed to create main panel for second instance: {}", e);
            return;
        }
    }
    windows::activate_main_window(app, target_url);
}
