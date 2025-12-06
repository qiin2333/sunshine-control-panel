use tauri::{Manager, AppHandle, Runtime, WebviewWindow};
use log::{info, error, debug};
use crate::proxy_server;

/// 禁用窗口的右键菜单（仅在生产环境）
/// 通过注入 JavaScript 来阻止默认的上下文菜单
#[cfg(not(debug_assertions))]
pub fn disable_context_menu<R: Runtime>(window: &WebviewWindow<R>) {
    let script = r#"
        (function() {
            // 禁用右键菜单
            document.addEventListener('contextmenu', function(e) {
                e.preventDefault();
                return false;
            }, true);
            
            // 禁用开发者工具快捷键（可选，根据需要启用）
            document.addEventListener('keydown', function(e) {
                // F12
                if (e.keyCode === 123) {
                    e.preventDefault();
                    return false;
                }
                // Ctrl+Shift+I
                if (e.ctrlKey && e.shiftKey && e.keyCode === 73) {
                    e.preventDefault();
                    return false;
                }
                // Ctrl+Shift+J
                if (e.ctrlKey && e.shiftKey && e.keyCode === 74) {
                    e.preventDefault();
                    return false;
                }
                // Ctrl+U (查看源代码)
                if (e.ctrlKey && e.keyCode === 85) {
                    e.preventDefault();
                    return false;
                }
            }, true);
        })();
    "#;
    
    // 延迟执行，确保 DOM 已加载
    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        use std::time::Duration;
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Err(e) = window_clone.eval(script) {
            debug!("⚠️ 禁用右键菜单脚本注入失败: {}", e);
        } else {
            debug!("✅ 已禁用右键菜单（生产环境）");
        }
    });
}

/// 开发环境不执行任何操作
#[cfg(debug_assertions)]
pub fn disable_context_menu<R: Runtime>(_window: &WebviewWindow<R>) {
    // 开发环境允许右键菜单
}

/// 显示并激活窗口（解决权限隔离问题）
pub fn show_and_activate_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    
    // 使用 Windows API 强制激活窗口（解决权限隔离问题）
    #[cfg(target_os = "windows")]
    {
        force_activate_window_win32(window);
    }
}

/// 使用 Windows API 强制激活窗口（解决权限隔离问题）
#[cfg(target_os = "windows")]
fn force_activate_window_win32<R: Runtime>(window: &WebviewWindow<R>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, ShowWindow, BringWindowToTop, SW_RESTORE, SW_SHOW,
        AllowSetForegroundWindow, ASFW_ANY, FindWindowW
    };
    use windows::core::PCWSTR;
    
    unsafe {
        // 获取窗口标题并查找窗口句柄
        let title = window.title().unwrap_or_default();
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd_result = FindWindowW(
            PCWSTR::null(),
            PCWSTR::from_raw(title_wide.as_ptr())
        );
        
        if let Ok(hwnd) = hwnd_result {
            if hwnd.0 != std::ptr::null_mut() {
                // 允许设置前台窗口（解决权限隔离问题）
                let _ = AllowSetForegroundWindow(ASFW_ANY);
                
                // 恢复并显示窗口
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = ShowWindow(hwnd, SW_SHOW);
                
                // 激活窗口
                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
                
                debug!("✅ 已使用 Windows API 强制激活窗口");
            }
        }
    }
}

/// 打开关于窗口（单例模式）
pub fn open_about_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    const ABOUT_WINDOW_ID: &str = "about";
    
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let window = tauri::WebviewWindowBuilder::new(
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
        
        // 在生产环境禁用右键菜单
        disable_context_menu(&window);
        
        info!("✅ 关于窗口创建成功");
    }
    
    Ok(())
}

/// 打开日志控制台窗口（单例模式）
pub fn open_log_console<R: Runtime>(app: &AppHandle<R>) {
    const LOG_CONSOLE_WINDOW_ID: &str = "log_console";
    
    if let Some(window) = app.get_webview_window(LOG_CONSOLE_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        match tauri::WebviewWindowBuilder::new(
            app,
            LOG_CONSOLE_WINDOW_ID,
            tauri::WebviewUrl::App("console/index.html".into())
        )
        .title("日志控制台")
        .inner_size(1000.0, 700.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .decorations(true)
        .center()
        .build()
        {
            Ok(window) => {
                // 在生产环境禁用右键菜单
                disable_context_menu(&window);
                info!("✅ 日志控制台窗口创建成功");
            }
            Err(e) => {
                error!("❌ 创建日志控制台窗口失败: {}", e);
            }
        }
    }
}

/// 打开调试页面窗口（单例模式，仅开发环境）
#[cfg(debug_assertions)]
pub fn open_debug_page<R: Runtime>(app: &AppHandle<R>) {
    const DEBUG_PAGE_WINDOW_ID: &str = "debug_page";
    
    if let Some(window) = app.get_webview_window(DEBUG_PAGE_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        match tauri::WebviewWindowBuilder::new(
            app,
            DEBUG_PAGE_WINDOW_ID,
            tauri::WebviewUrl::App("console/drag-drop-demo.html".into())
        )
        .title("调试页面 - 拖拽测试")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .decorations(true)
        .disable_drag_drop_handler() // allow HTML5 drag/drop APIs inside the webview
        .center()
        .build()
        {
            Ok(_window) => {
                // 调试页面窗口不需要禁用右键菜单（开发环境）
                info!("✅ 调试页面窗口创建成功");
            }
            Err(e) => {
                error!("❌ 创建调试页面窗口失败: {}", e);
            }
        }
    }
}

/// 创建主窗口
pub fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    const MAIN_WINDOW_ID: &str = "main";
    
    info!("🪟 创建主窗口...");
    
    let window = tauri::WebviewWindowBuilder::new(
        app,
        MAIN_WINDOW_ID,
        tauri::WebviewUrl::App("placeholder.html".into())
    )
    .title("Sunshine Control Panel")
    .inner_size(1280.0, 800.0)
    .min_inner_size(1024.0, 600.0)
    .center()
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .visible(true)
    .disable_drag_drop_handler() // 禁用原生拖拽，允许 HTML5 drag/drop API
    .build()
    .map_err(|e| format!("创建主窗口失败: {}", e))?;
    
    // 在生产环境禁用右键菜单
    disable_context_menu(&window);
    
    info!("✅ 主窗口创建成功（已禁用原生拖拽）");
    
    Ok(())
}

/// 创建桌面 UI 窗口（全屏/最大化模式）
pub fn create_desktop_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    const DESKTOP_WINDOW_ID: &str = "desktop";
    
    info!("🖥️ 创建桌面 UI 窗口...");
    
    let window = tauri::WebviewWindowBuilder::new(
        app,
        DESKTOP_WINDOW_ID,
        tauri::WebviewUrl::App("desktop/index.html".into())
    )
    .title("Sunshine Desktop")
    .inner_size(1600.0, 900.0)
    .min_inner_size(1024.0, 600.0)
    .center()
    .decorations(false)  // 自定义标题栏
    .transparent(false)
    .shadow(true)
    .visible(true)
    .maximized(true)     // 默认最大化
    .disable_drag_drop_handler()
    .build()
    .map_err(|e| format!("创建桌面窗口失败: {}", e))?;
    
    // 在生产环境禁用右键菜单
    disable_context_menu(&window);
    
    info!("✅ 桌面 UI 窗口创建成功");
    
    Ok(())
}

/// 打开桌面 UI 窗口（单例模式）
pub fn open_desktop_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    const DESKTOP_WINDOW_ID: &str = "desktop";
    
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
    let Some(window) = app.get_webview_window("main") else {
        error!("❌ 未找到主窗口 'main'");
        return;
    };
    
    info!("📱 正在激活主窗口...");
    
    // 获取窗口状态
    let is_visible = window.is_visible().unwrap_or(false);
    let is_minimized = window.is_minimized().unwrap_or(false);
    
    debug!("   当前状态: visible={}, minimized={}", is_visible, is_minimized);
    
    // 恢复窗口状态
    if is_minimized {
        let _ = window.unminimize();
        debug!("   ✅ 已取消最小化");
    }
    
    if !is_visible {
        let _ = window.show();
        debug!("   ✅ 已显示窗口");
    }
    
    let _ = window.set_focus();
    debug!("   ✅ 已聚焦窗口");
    
    // 使用 Windows API 强制激活窗口（解决权限隔离问题）
    #[cfg(target_os = "windows")]
    {
        force_activate_window_win32(&window);
    }
    
    // 处理 URL 导航
    if let Some(url) = target_url {
        navigate_to_url(&window, &url);
    }
    
    // 短暂置顶以强制显示在前台
    let _ = window.set_always_on_top(true);
    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        use std::time::Duration;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = window_clone.set_always_on_top(false);
    });
    
    info!("✅ 窗口激活完成");
}

/// 导航到指定 URL
fn navigate_to_url(window: &WebviewWindow, url: &str) {
    use url::Url;
    
    info!("🔄 正在导航到: {}", url);
    
    let Ok(parsed_url) = Url::parse(url) else {
        error!("❌ URL 解析失败: {}", url);
        return;
    };
    
    let path = format!(
        "{}{}",
        parsed_url.path(),
        parsed_url.query().map(|q| format!("?{}", q)).unwrap_or_default()
    );
    
    // 获取动态代理 URL
    let proxy_url = proxy_server::get_proxy_url();
    
    let script = format!(
        r#"
        (function() {{
            const iframe = document.querySelector('.sunshine-iframe');
            if (iframe && iframe.contentWindow) {{
                iframe.src = '{}{}';
                console.log('📍 导航到:', '{}');
            }}
        }})();
        "#,
        proxy_url, path, path
    );
    
    let _ = window.eval(&script);
    debug!("✅ 已发送导航命令");
}

/// 处理窗口事件
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::WindowEvent;
    
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
                    crate::toolbar::save_toolbar_position_internal(&app, position.x as f64, position.y as f64);
                }
            }
            _ => {
                // 其他窗口正常关闭
            }
        }
    }
}
