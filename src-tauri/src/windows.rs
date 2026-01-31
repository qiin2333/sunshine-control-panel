use tauri::{Manager, AppHandle, Runtime, WebviewWindow};
use log::{info, error, debug};
use crate::proxy_server;

const MAIN_WINDOW_ID: &str = "main";
const ABOUT_WINDOW_ID: &str = "about";
const LOG_CONSOLE_WINDOW_ID: &str = "log_console";
const PIN_WINDOW_ID: &str = "pin_pairing";
const DESKTOP_WINDOW_ID: &str = "desktop";
#[cfg(debug_assertions)]
const DEBUG_PAGE_WINDOW_ID: &str = "debug_page";

/// 禁用窗口的右键菜单（仅在生产环境）
#[cfg(not(debug_assertions))]
pub fn disable_context_menu<R: Runtime>(window: &WebviewWindow<R>) {
    const DISABLE_CONTEXT_MENU_SCRIPT: &str = r#"
        (function() {
            document.addEventListener('contextmenu', e => { e.preventDefault(); return false; }, true);
            document.addEventListener('keydown', e => {
                if (e.keyCode === 123 || // F12
                    (e.ctrlKey && e.shiftKey && (e.keyCode === 73 || e.keyCode === 74)) || // Ctrl+Shift+I/J
                    (e.ctrlKey && e.keyCode === 85)) { // Ctrl+U
                    e.preventDefault();
                    return false;
                }
            }, true);
        })();
    "#;
    
    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        match window_clone.eval(DISABLE_CONTEXT_MENU_SCRIPT) {
            Ok(_) => debug!("✅ 已禁用右键菜单（生产环境）"),
            Err(e) => debug!("⚠️ 禁用右键菜单脚本注入失败: {}", e),
        }
    });
}

/// 开发环境不执行任何操作
#[cfg(debug_assertions)]
pub fn disable_context_menu<R: Runtime>(_window: &WebviewWindow<R>) {}

/// 显示并激活窗口（解决权限隔离问题）
pub fn show_and_activate_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    
    #[cfg(target_os = "windows")]
    force_activate_window_win32(window);
}

/// 使用 Windows API 强制激活窗口
#[cfg(target_os = "windows")]
fn force_activate_window_win32<R: Runtime>(window: &WebviewWindow<R>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, ShowWindow, BringWindowToTop, SW_RESTORE, SW_SHOW,
        AllowSetForegroundWindow, ASFW_ANY, GetWindowThreadProcessId
    };
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::Foundation::HWND;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use log::warn;
    
    // 直接从 Tauri 窗口获取 HWND，避免使用 FindWindowW（对隐藏窗口不可靠）
    let hwnd = match window.window_handle() {
        Ok(handle) => {
            match handle.as_raw() {
                RawWindowHandle::Win32(win32_handle) => {
                    HWND(win32_handle.hwnd.get() as *mut _)
                }
                _ => {
                    error!("❌ 无法获取 Win32 窗口句柄：不是 Win32 窗口");
                    return;
                }
            }
        }
        Err(e) => {
            error!("❌ 获取窗口句柄失败: {:?}", e);
            return;
        }
    };
    
    if hwnd.0.is_null() {
        error!("❌ 窗口句柄为空");
        return;
    }
    
    debug!("✅ 获取到窗口句柄: {:?}", hwnd);
    
    unsafe {
        // 获取目标窗口的线程ID用于诊断
        let mut target_pid: u32 = 0;
        let target_tid = GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
        let current_tid = GetCurrentThreadId();
        debug!("📊 当前线程: {}, 目标线程: {}, 目标进程: {}", current_tid, target_tid, target_pid);
        
        // 允许任何进程设置前台窗口
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        
        // 确保窗口可见并恢复
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);
        
        // 尝试设置为前台窗口
        let result = SetForegroundWindow(hwnd);
        
        if result.as_bool() {
            info!("✅ 已使用 Windows API 强制激活窗口");
        } else {
            warn!("⚠️ SetForegroundWindow 返回 FALSE，窗口可能未能激活到前台");
        }
    }
}

/// 获取或创建窗口的辅助函数
fn get_or_create_window<R: Runtime, F>(
    app: &AppHandle<R>,
    window_id: &str,
    builder_fn: F,
) -> Result<WebviewWindow<R>, String>
where
    F: FnOnce(&AppHandle<R>) -> Result<WebviewWindow<R>, tauri::Error>,
{
    if let Some(window) = app.get_webview_window(window_id) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }
    
    builder_fn(app).map_err(|e| format!("创建窗口失败: {}", e))
}

/// 打开关于窗口（单例模式）
pub fn open_about_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = get_or_create_window(app, ABOUT_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(app, ABOUT_WINDOW_ID, tauri::WebviewUrl::App("about/index.html".into()))
            .title("关于 Sunshine Control Panel")
            .inner_size(540.0, 620.0)
            .resizable(false)
            .maximizable(false)
            .minimizable(true)
            .decorations(true)
            .center()
            .build()
    })?;
    
    disable_context_menu(&window);
    info!("✅ 关于窗口已打开");
    Ok(())
}

/// 打开日志控制台窗口（单例模式）
pub fn open_log_console<R: Runtime>(app: &AppHandle<R>) {
    match get_or_create_window(app, LOG_CONSOLE_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(app, LOG_CONSOLE_WINDOW_ID, tauri::WebviewUrl::App("console/index.html".into()))
            .title("日志控制台")
            .inner_size(1000.0, 700.0)
            .resizable(true)
            .maximizable(true)
            .minimizable(true)
            .decorations(true)
            .center()
            .build()
    }) {
        Ok(window) => {
            disable_context_menu(&window);
            info!("✅ 日志控制台窗口已打开");
        }
        Err(e) => error!("❌ {}", e),
    }
}

/// 打开 PIN 配对窗口（单例模式）
pub fn open_pin_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window(PIN_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        info!("✅ PIN 窗口已激活");
        return Ok(());
    }
    
    let window = tauri::WebviewWindowBuilder::new(app, PIN_WINDOW_ID, tauri::WebviewUrl::App("pin/index.html".into()))
        .title("PIN 配对")
        .fullscreen(true)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建 PIN 窗口失败: {}", e))?;
    
    disable_context_menu(&window);
    
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = window.show();
    });
    
    info!("✅ PIN 配对窗口创建成功");
    Ok(())
}

/// 打开调试页面窗口（仅开发环境）
#[cfg(debug_assertions)]
pub fn open_debug_page<R: Runtime>(app: &AppHandle<R>) {
    match get_or_create_window(app, DEBUG_PAGE_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(app, DEBUG_PAGE_WINDOW_ID, tauri::WebviewUrl::App("console/drag-drop-demo.html".into()))
            .title("调试页面 - 拖拽测试")
            .inner_size(1200.0, 800.0)
            .resizable(true)
            .maximizable(true)
            .minimizable(true)
            .decorations(true)
            .disable_drag_drop_handler()
            .center()
            .build()
    }) {
        Ok(_) => info!("✅ 调试页面窗口已打开"),
        Err(e) => error!("❌ {}", e),
    }
}

/// 创建主窗口
pub fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    create_main_window_internal(app, true)
}

/// 创建隐藏的主窗口
pub fn create_main_window_hidden<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    create_main_window_internal(app, false)
}

fn create_main_window_internal<R: Runtime>(app: &AppHandle<R>, visible: bool) -> Result<(), Box<dyn std::error::Error>> {
    if app.get_webview_window(MAIN_WINDOW_ID).is_some() {
        debug!("主窗口已存在，跳过创建");
        return Ok(());
    }
    
    let visibility_desc = if visible { "" } else { "隐藏的" };
    info!("🪟 创建{}主窗口...", visibility_desc);
    
    let window = tauri::WebviewWindowBuilder::new(app, MAIN_WINDOW_ID, tauri::WebviewUrl::App("placeholder.html".into()))
        .title("Sunshine Control Panel")
        .inner_size(1280.0, 800.0)
        .min_inner_size(1024.0, 600.0)
        .center()
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .visible(visible)
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| format!("创建{}主窗口失败: {}", visibility_desc, e))?;
    
    disable_context_menu(&window);
    info!("✅ {}主窗口创建成功", visibility_desc);
    Ok(())
}

/// 创建桌面 UI 窗口
pub fn create_desktop_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    info!("🖥️ 创建桌面 UI 窗口...");
    
    let window = tauri::WebviewWindowBuilder::new(app, DESKTOP_WINDOW_ID, tauri::WebviewUrl::App("desktop/index.html".into()))
        .title("Sunshine Desktop")
        .inner_size(1600.0, 900.0)
        .min_inner_size(1024.0, 600.0)
        .center()
        .decorations(false)
        .transparent(false)
        .shadow(true)
        .visible(true)
        .maximized(true)
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| format!("创建桌面窗口失败: {}", e))?;
    
    disable_context_menu(&window);
    info!("✅ 桌面 UI 窗口创建成功");
    Ok(())
}

/// 打开桌面 UI 窗口（单例模式）
pub fn open_desktop_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DESKTOP_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        info!("✅ 桌面 UI 窗口已激活");
    } else {
        create_desktop_window(app).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 激活主窗口
pub fn activate_main_window(app: &tauri::AppHandle, target_url: Option<String>) { 
    info!("📱 activate_main_window 被调用，target_url: {:?}", target_url);
    
    let Some(window) = app.get_webview_window(MAIN_WINDOW_ID) else {
        error!("❌ 未找到主窗口 '{}'", MAIN_WINDOW_ID);
        // 列出所有现有窗口以便诊断
        let windows: Vec<_> = app.webview_windows().keys().cloned().collect();
        error!("   当前存在的窗口: {:?}", windows);
        return;
    };
    
    info!("📱 正在激活主窗口...");
    
    let is_visible = window.is_visible().unwrap_or(false);
    let is_minimized = window.is_minimized().unwrap_or(false);
    
    if is_minimized { let _ = window.unminimize(); }
    if !is_visible { let _ = window.show(); }
    let _ = window.set_focus();
    
    #[cfg(target_os = "windows")]
    force_activate_window_win32(&window);
    
    if let Some(url) = target_url {
        navigate_to_url(&window, &url);
    }
    
    // 短暂置顶以强制显示在前台
    let _ = window.set_always_on_top(true);
    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = window_clone.set_always_on_top(false);
    });
    
    info!("✅ 窗口激活完成");
}

/// 导航到指定 URL
fn navigate_to_url(window: &WebviewWindow, url: &str) {
    info!("🔄 正在导航到: {}", url);
    
    let Ok(parsed_url) = url::Url::parse(url) else {
        error!("❌ URL 解析失败: {}", url);
        return;
    };
    
    let path = format!(
        "{}{}",
        parsed_url.path(),
        parsed_url.query().map(|q| format!("?{}", q)).unwrap_or_default()
    );
    
    if path.contains("/pin") {
        info!("🔐 检测到 /pin 路径，跳过导航");
        return;
    }
    
    let proxy_url = proxy_server::get_proxy_url();
    let script = format!(
        r#"(function(){{ const iframe = document.querySelector('.sunshine-iframe'); if (iframe) iframe.src = '{}{}'; }})();"#,
        proxy_url, path
    );
    
    let _ = window.eval(&script);
    debug!("✅ 已发送导航命令");
}

/// 处理窗口事件
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        match window.label() {
            "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            "toolbar" => {
                if let Ok(position) = window.outer_position() {
                    crate::toolbar::save_toolbar_position_internal(
                        &window.app_handle(),
                        position.x as f64,
                        position.y as f64
                    );
                }
            }
            _ => {}
        }
    }
}
