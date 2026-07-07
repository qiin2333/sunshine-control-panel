use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};
use std::{sync::Mutex, time::Duration};
use log::{debug, error, info, warn};

use crate::{toolbar, update, utils, windows, moonlight_web};

// 托盘图标 ID
const TRAY_ID: &str = "main-tray";

// 防止睡眠状态管理
static PREVENT_SLEEP_STATE: Mutex<bool> = Mutex::new(false);

// Sunshine 用户模式状态管理
#[cfg(target_os = "windows")]
static SUNSHINE_USER_MODE_STATE: Mutex<bool> = Mutex::new(false);

// 当前语言状态管理 ("zh" 或 "en")
static CURRENT_LOCALE: Mutex<Option<String>> = Mutex::new(None);

/// 托盘菜单翻译结构
struct TrayStrings {
    open_website: &'static str,
    vdd_settings: &'static str,
    restart_user_mode: &'static str,
    show_toolbar: &'static str,
    prevent_sleep: &'static str,
    rtss_control: &'static str,
    host_performance: &'static str,
    log_console: &'static str,
    open_desktop: &'static str,
    web_stream: &'static str,
    debug_page: &'static str,
    check_update: &'static str,
    about: &'static str,
    quit: &'static str,
    language: &'static str,
    tooltip: &'static str,
    tooltip_admin: &'static str,
}

const ZH_STRINGS: TrayStrings = TrayStrings {
    open_website: "🌐 打开官网",
    vdd_settings: "📱 设置虚拟显示器（VDD）",
    restart_user_mode: "☀ 用户模式运行 Sunshine",
    show_toolbar: "🐾 显示桌宠",
    prevent_sleep: "💤 不许睡",
    rtss_control: "🎯 RTSS 控制",
    host_performance: "📊 主机性能",
    log_console: "🔍 打开日志控制台",
    open_desktop: "🖥️ 打开桌面 UI",
    web_stream: "🌙 Web 串流服务",
    debug_page: "🐛 打开调试页面",
    check_update: "🔄 检查更新",
    about: "ℹ️ 关于",
    quit: "退出程序",
    language: "🌍 语言 / Language",
    tooltip: "Sunshine GUI",
    tooltip_admin: "Sunshine GUI (管理员)",
};

const EN_STRINGS: TrayStrings = TrayStrings {
    open_website: "🌐 Open Website",
    vdd_settings: "📱 Virtual Display (VDD)",
    restart_user_mode: "☀ Run Sunshine in User Mode",
    show_toolbar: "🐾 Show Toolbar",
    prevent_sleep: "💤 Prevent Sleep",
    rtss_control: "🎯 RTSS Control",
    host_performance: "📊 Host Performance",
    log_console: "🔍 Log Console",
    open_desktop: "🖥️ Desktop UI",
    web_stream: "🌙 Web Streaming",
    debug_page: "🐛 Debug Page",
    check_update: "🔄 Check for Updates",
    about: "ℹ️ About",
    quit: "Quit",
    language: "🌍 语言 / Language",
    tooltip: "Sunshine GUI",
    tooltip_admin: "Sunshine GUI (Admin)",
};

fn get_tray_strings() -> &'static TrayStrings {
    let locale = CURRENT_LOCALE.lock().unwrap();
    match locale.as_deref() {
        Some("en") => &EN_STRINGS,
        _ => &ZH_STRINGS,
    }
}

fn get_current_locale() -> String {
    CURRENT_LOCALE.lock().unwrap().clone().unwrap_or_else(|| "zh".to_string())
}

#[cfg(target_os = "windows")]
mod power {
    const ES_CONTINUOUS: u32 = 0x80000000;
    const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
    const ES_AWAYMODE_REQUIRED: u32 = 0x00000040;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(es_flags: u32) -> u32;
    }

    pub fn enable_prevent_sleep() -> Result<(), &'static str> {
        let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED;
        unsafe {
            if SetThreadExecutionState(flags) == 0 {
                return Err("SetThreadExecutionState 调用失败");
            }
        }
        Ok(())
    }

    pub fn disable_prevent_sleep() -> Result<(), &'static str> {
        unsafe {
            if SetThreadExecutionState(ES_CONTINUOUS) == 0 {
                return Err("SetThreadExecutionState 调用失败");
            }
        }
        Ok(())
    }
}

/// 创建系统托盘
pub fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    init_sunshine_user_mode_state(app);

    let menu = build_tray_menu(app)?;
    let s = get_tray_strings();
    let tooltip = if utils::is_running_as_admin().unwrap_or(false) {
        s.tooltip_admin
    } else {
        s.tooltip
    };

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip(tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, .. } => handle_tray_click(tray.app_handle()),
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => handle_tray_double_click(tray.app_handle()),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// 初始化 Sunshine 用户模式状态（仅 Windows）
#[cfg(target_os = "windows")]
fn init_sunshine_user_mode_state<R: Runtime>(app: &AppHandle<R>) {
    // 使用默认值 false，避免阻塞启动
    *SUNSHINE_USER_MODE_STATE.lock().unwrap() = false;
    
    // 异步更新 Sunshine 用户模式状态（不阻塞启动；阻塞的 sc/tasklist 放在 spawn_blocking 中）
    let _app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        match tokio::task::spawn_blocking(crate::sunshine::is_sunshine_running_in_user_mode_impl).await {
            Ok(Ok(is_user_mode)) => {
                *SUNSHINE_USER_MODE_STATE.lock().unwrap() = is_user_mode;
                debug!("✅ Sunshine 用户模式状态已异步更新: {}", is_user_mode);
            }
            Ok(Err(e)) => {
                debug!("⚠️ 检查 Sunshine 用户模式状态失败: {}", e);
            }
            Err(e) => {
                debug!("⚠️ spawn_blocking 检查用户模式失败: {}", e);
            }
        }
    });
}

/// 构建托盘菜单
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let s = get_tray_strings();
    let current_locale = get_current_locale();

    let open_website = MenuItem::with_id(app, "open_website", s.open_website, true, None::<&str>)?;
    let vdd_settings = MenuItem::with_id(app, "vdd_settings", s.vdd_settings, true, None::<&str>)?;
    
    let show_toolbar = MenuItem::with_id(app, "show_toolbar", s.show_toolbar, true, None::<&str>)?;

    let rtss_control = MenuItem::with_id(app, "rtss_control", s.rtss_control, true, None::<&str>)?;
    let host_performance = MenuItem::with_id(app, "host_performance", s.host_performance, true, None::<&str>)?;
    let log_console = MenuItem::with_id(app, "log_console", s.log_console, true, None::<&str>)?;
    #[cfg(any(debug_assertions, feature = "beta"))]
    let web_stream = MenuItem::with_id(app, "web_stream", s.web_stream, true, None::<&str>)?;
    let check_update = MenuItem::with_id(app, "check_update", s.check_update, true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", s.about, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", s.quit, true, None::<&str>)?;

    // 语言子菜单
    let lang_zh = CheckMenuItem::with_id(app, "lang_zh", "中文", true, current_locale == "zh", None::<&str>)?;
    let lang_en = CheckMenuItem::with_id(app, "lang_en", "English", true, current_locale == "en", None::<&str>)?;
    let lang_submenu = Submenu::with_id_and_items(app, "language", s.language, true, &[&lang_zh, &lang_en])?;

    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;

    #[cfg(target_os = "windows")]
    let restart_user_mode = {
        let is_user_mode = *SUNSHINE_USER_MODE_STATE.lock().unwrap();
        CheckMenuItem::with_id(app, "restart_user_mode", s.restart_user_mode, true, is_user_mode, None::<&str>)?
    };

    #[cfg(target_os = "windows")]
    let prevent_sleep = {
        let is_preventing = *PREVENT_SLEEP_STATE.lock().unwrap();
        CheckMenuItem::with_id(app, "prevent_sleep", s.prevent_sleep, true, is_preventing, None::<&str>)?
    };

    let open_desktop = MenuItem::with_id(app, "open_desktop", s.open_desktop, true, None::<&str>)?;
    #[cfg(debug_assertions)]
    let debug_page = MenuItem::with_id(app, "debug_page", s.debug_page, true, None::<&str>)?;
    #[cfg(debug_assertions)]
    let separator_debug = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&open_website, &separator1, &vdd_settings];

    #[cfg(target_os = "windows")]
    items.push(&restart_user_mode);

    items.push(&show_toolbar);

    #[cfg(target_os = "windows")]
    items.push(&prevent_sleep);

    items.push(&rtss_control);
    items.push(&host_performance);
    items.push(&log_console);
    items.push(&open_desktop);
    #[cfg(any(debug_assertions, feature = "beta"))]
    items.push(&web_stream);

    #[cfg(debug_assertions)]
    items.extend([&separator_debug as &dyn tauri::menu::IsMenuItem<R>, &debug_page]);

    items.extend([&separator2 as &dyn tauri::menu::IsMenuItem<R>, &check_update, &about, &lang_submenu, &separator3, &quit]);

    Menu::with_items(app, &items)
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
            utils::open_url_in_browser("https://www.alkaidlab.com/");
        }
        "open_desktop" => {
            info!("🖥️ 托盘菜单：打开桌面 UI");
            if let Err(e) = windows::open_desktop_window(app) {
                error!("❌ 打开桌面 UI 失败: {}", e);
            }
        }
        "vdd_settings" => open_vdd_settings(app),
        #[cfg(target_os = "windows")]
        "restart_user_mode" => toggle_sunshine_mode(app),
        "show_toolbar" => toggle_toolbar(app),
        "rtss_control" => {
            info!("🎯 托盘菜单：打开 RTSS 控制");
            toolbar::create_tool_window_internal(app, "rtss");
        }
        "host_performance" => {
            info!("📊 托盘菜单：打开主机性能监控");
            toolbar::create_tool_window_internal(app, "performance");
        }
        "log_console" => windows::open_log_console(app),
        "web_stream" => open_web_stream_settings(app),
        #[cfg(target_os = "windows")]
        "prevent_sleep" => toggle_prevent_sleep(),
        #[cfg(debug_assertions)]
        "debug_page" => {
            info!("🐛 托盘菜单：打开调试页面");
            windows::open_debug_page(app);
        }
        "check_update" => check_for_updates(app),
        "about" => {
            info!("ℹ️ 托盘菜单：显示关于对话框");
            let _ = windows::open_about_window(app);
        }
        "quit" => {
            info!("🚪 托盘菜单：退出应用");
            #[cfg(target_os = "windows")]
            cleanup_prevent_sleep();
            moonlight_web::cleanup();
            std::process::exit(0);
        }
        "lang_zh" => switch_tray_locale(app, "zh"),
        "lang_en" => switch_tray_locale(app, "en"),
        _ => warn!("⚠️ 未知的托盘菜单事件: {}", menu_id),
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

/// 从托盘打开 Web 串流设置
fn open_web_stream_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        info!("🌙 托盘菜单：打开 Web 串流设置");
        windows::show_and_activate_window(&window);
        let _ = window.emit("open-web-stream", ());
    }
}

/// 更新 Sunshine 用户模式状态
#[cfg(target_os = "windows")]
async fn update_sunshine_mode_state(check_label: &str) {
    let is_user_mode = tokio::task::spawn_blocking(crate::sunshine::is_sunshine_running_in_user_mode_impl)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false);
    *SUNSHINE_USER_MODE_STATE.lock().unwrap() = is_user_mode;
    info!("✅ Sunshine 用户模式状态已更新({}): {}", check_label, is_user_mode);
}

/// 切换 Sunshine 运行模式
#[cfg(target_os = "windows")]
fn toggle_sunshine_mode<R: Runtime>(app: &AppHandle<R>) {
    info!("🔄 托盘菜单：切换 Sunshine 模式");
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match crate::sunshine::toggle_sunshine_mode().await {
            Ok(msg) => {
                info!("✅ {}", msg);
                emit_message(&app_handle, "success", &msg);

                // 切换由 UAC 提升的 PowerShell 在后台执行，需预留 UAC + stop + start
                // 两次检查以减少"中间状态"导致的误判
                tokio::time::sleep(Duration::from_secs(6)).await;
                update_sunshine_mode_state("首次").await;

                tokio::time::sleep(Duration::from_secs(5)).await;
                update_sunshine_mode_state("二次").await;
            }
            Err(e) => {
                error!("❌ 切换 Sunshine 模式失败: {}", e);
                emit_message(&app_handle, "error", &format!("切换失败: {}", e));
            }
        }
    });
}

/// 切换工具栏显示/隐藏
fn toggle_toolbar<R: Runtime>(app: &AppHandle<R>) {
    info!("🔧 托盘菜单：切换工具栏显示/隐藏");

    if let Some(toolbar_window) = app.get_webview_window("toolbar") {
        let _ = toolbar_window.close();
    } else {
        if let Err(e) = toolbar::create_toolbar_window_internal(app) {
            error!("❌ 创建工具栏失败: {}", e);
        }
    }
}

/// 检查更新（托盘菜单触发，`manual = true`）
fn check_for_updates<R: Runtime>(app: &AppHandle<R>) {
    info!("🔄 托盘菜单：检查更新");
    let app_handle = app.clone();

    if let Some(window) = app.get_webview_window("main") {
        windows::show_and_activate_window(&window);
    }

    let include_prerelease = update::get_include_prerelease(app);
    tauri::async_runtime::spawn(async move {
        match update::check_for_updates_internal(true, include_prerelease).await {
            Ok(Some(update_info)) => {
                if update_info.is_latest {
                    info!("✅ 已是最新版本: {}", update_info.version);
                } else {
                    info!("🎉 发现新版本: {}", update_info.version);
                }
                update::save_last_check_time(&app_handle);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-available", &update_info);
                }
            }
            Ok(None) => {
                // manual=true 时此分支不会触发
                debug!("✅ 已是最新版本");
                update::save_last_check_time(&app_handle);
            }
            Err(e) => {
                error!("❌ 检查更新失败: {}", e);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("update-check-result", serde_json::json!({
                        "error": e
                    }));
                }
            }
        }
    });
}

/// 发送消息到主窗口
fn emit_message<R: Runtime>(app: &AppHandle<R>, msg_type: &str, message: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("show-message", serde_json::json!({
            "type": msg_type,
            "message": message
        }));
    }
}


/// 切换防止睡眠功能
#[cfg(target_os = "windows")]
fn toggle_prevent_sleep() {
    let mut state = PREVENT_SLEEP_STATE.lock().unwrap();
    let new_state = !*state;

    let result = if new_state {
        info!("🌙 托盘菜单：启用防止睡眠");
        power::enable_prevent_sleep()
    } else {
        info!("💤 托盘菜单：禁用防止睡眠");
        power::disable_prevent_sleep()
    };

    match result {
        Ok(()) => *state = new_state,
        Err(e) => error!("❌ 切换防止睡眠失败: {}", e),
    }
}

/// 清理防止睡眠状态（在应用退出时调用）
#[cfg(target_os = "windows")]
pub fn cleanup_prevent_sleep() {
    if *PREVENT_SLEEP_STATE.lock().unwrap() {
        match power::disable_prevent_sleep() {
            Ok(()) => info!("✅ 已清理防止睡眠状态"),
            Err(e) => error!("❌ 清理防止睡眠状态失败: {}", e),
        }
    }
}

/// 从托盘菜单切换语言
fn switch_tray_locale<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    info!("🌍 托盘菜单：切换语言为 {}", locale);
    *CURRENT_LOCALE.lock().unwrap() = Some(locale.to_string());
    rebuild_tray_menu(app);
    // 通知前端同步语言
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("tray-locale-changed", locale);
    }
    // 同时通知 desktop 窗口
    if let Some(window) = app.get_webview_window("desktop") {
        let _ = window.emit("tray-locale-changed", locale);
    }
}

/// 重建托盘菜单（语言切换后调用）
fn rebuild_tray_menu<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match build_tray_menu(app) {
            Ok(menu) => {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    error!("❌ 重建托盘菜单失败: {}", e);
                }
                // 更新 tooltip
                let s = get_tray_strings();
                let tooltip = if utils::is_running_as_admin().unwrap_or(false) {
                    s.tooltip_admin
                } else {
                    s.tooltip
                };
                let _ = tray.set_tooltip(Some(tooltip));
            }
            Err(e) => error!("❌ 构建托盘菜单失败: {}", e),
        }
    }
}

/// Tauri 命令：前端通知 tray 同步语言
#[tauri::command]
pub fn set_tray_locale(app: AppHandle, locale: String) {
    info!("🌍 前端同步语言到托盘: {}", locale);
    *CURRENT_LOCALE.lock().unwrap() = Some(locale);
    rebuild_tray_menu(&app);
}

/// Tauri 命令：前端获取当前 tray 语言
#[tauri::command]
pub fn get_tray_locale() -> Option<String> {
    CURRENT_LOCALE.lock().unwrap().clone()
}
