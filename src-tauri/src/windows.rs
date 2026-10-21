use crate::proxy_server;
use log::{debug, error, info, warn};
use std::sync::LazyLock;
use std::sync::Mutex;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow,
    webview::PageLoadEvent,
};

pub(crate) mod lifecycle;

#[derive(Default)]
struct ToolWindowMoveSave {
    version: u64,
    position: Option<PhysicalPosition<i32>>,
}

static TOOL_WINDOW_MOVE_SAVE: LazyLock<Mutex<ToolWindowMoveSave>> =
    LazyLock::new(|| Mutex::new(ToolWindowMoveSave::default()));

const MAIN_WINDOW_ID: &str = "main";
const ABOUT_WINDOW_ID: &str = "about";
const LOG_CONSOLE_WINDOW_ID: &str = "log_console";
const PIN_WINDOW_ID: &str = "pin_pairing";
const DESKTOP_WINDOW_ID: &str = "desktop";
const TOOL_WINDOW_ID: &str = "tool_window";
#[cfg(debug_assertions)]
const DEBUG_PAGE_WINDOW_ID: &str = "debug_page";

#[cfg(target_os = "windows")]
pub fn register_agent_restart() {
    use windows::Win32::System::Recovery::{
        REGISTER_APPLICATION_RESTART_FLAGS, RegisterApplicationRestart,
    };
    use windows::core::w;

    if let Err(e) =
        unsafe { RegisterApplicationRestart(w!("--hidden"), REGISTER_APPLICATION_RESTART_FLAGS(0)) }
    {
        warn!("Failed to register GUI agent restart: {}", e);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn register_agent_restart() {}

#[cfg(target_os = "windows")]
pub fn unregister_agent_restart() {
    use windows::Win32::System::Recovery::UnregisterApplicationRestart;

    if let Err(e) = unsafe { UnregisterApplicationRestart() } {
        debug!("Failed to unregister GUI agent restart: {}", e);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_agent_restart() {}

fn schedule_tool_window_position_save(app: AppHandle, position: PhysicalPosition<i32>) {
    let version = {
        let mut state = TOOL_WINDOW_MOVE_SAVE.lock().unwrap();
        state.version = state.version.wrapping_add(1);
        state.position = Some(position);
        state.version
    };

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;

        let position = {
            let state = TOOL_WINDOW_MOVE_SAVE.lock().unwrap();
            if state.version != version {
                return;
            }
            state.position
        };

        if let Some(position) = position {
            crate::toolbar::save_tool_window_position_internal(
                &app,
                position.x as f64,
                position.y as f64,
            );
        }
    });
}

fn clamp_axis(value: i32, min: i32, max: i32) -> i32 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

pub fn clamp_window_position_to_monitor(
    position: PhysicalPosition<i32>,
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    scale_factor: f64,
    logical_width: f64,
    logical_height: f64,
    margin_logical: f64,
) -> PhysicalPosition<i32> {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let margin = (margin_logical * scale).round() as i32;
    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    let min_x = monitor_position.x + margin;
    let min_y = monitor_position.y + margin;
    let max_x = monitor_position.x + monitor_size.width as i32 - width - margin;
    let max_y = monitor_position.y + monitor_size.height as i32 - height - margin;

    PhysicalPosition::new(
        clamp_axis(position.x, min_x, max_x),
        clamp_axis(position.y, min_y, max_y),
    )
}

fn constrain_tool_window_to_visible_area(
    window: &tauri::Window,
    logical_width: f64,
    logical_height: f64,
    margin_logical: f64,
) -> Result<(), String> {
    let Some(monitor) = window
        .current_monitor()
        .map_err(|e| format!("Failed to get current monitor: {}", e))?
    else {
        return Ok(());
    };
    let position = window
        .outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    let target_position = clamp_window_position_to_monitor(
        position,
        *monitor.position(),
        *monitor.size(),
        monitor.scale_factor(),
        logical_width,
        logical_height,
        margin_logical,
    );

    if target_position != position {
        window
            .set_position(target_position)
            .map_err(|e| format!("Failed to constrain tool window position: {}", e))?;
    }

    Ok(())
}

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

/// WebView 心跳命令：由前端 JS 定期调用，表明渲染进程仍然存活
#[tauri::command]
pub fn webview_heartbeat(webview: tauri::Webview, generation: u64) {
    lifecycle::heartbeat(webview.label(), generation);
}

/// Resize the About window to match its web content height.
#[tauri::command]
pub fn resize_about_window(window: tauri::Window, height: f64) -> Result<(), String> {
    if window.label() != ABOUT_WINDOW_ID {
        return Err("resize_about_window can only be called from the About window".to_string());
    }

    if !height.is_finite() {
        return Err("Invalid about window height".to_string());
    }

    let scale_factor = window
        .scale_factor()
        .map_err(|e| format!("Failed to get window scale factor: {}", e))?;
    let current_size = window
        .inner_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;
    let current_width = current_size.width as f64 / scale_factor;
    let current_height = current_size.height as f64 / scale_factor;
    let max_height = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| (monitor.size().height as f64 / scale_factor - 120.0).max(420.0))
        .unwrap_or(900.0);
    let target_height = height.clamp(420.0, max_height);

    if (current_height - target_height).abs() < 1.0 {
        return Ok(());
    }

    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            current_width,
            target_height,
        )))
        .map_err(|e| format!("Failed to resize about window: {}", e))
}

/// Resize the floating tool window to match its web content size.
// These synchronous commands execute on the GUI thread. Do not resize/reposition
// a caption while Windows owns its native move loop (there are no DOM pointers).
fn native_window_move_active(window: &tauri::Window) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GUI_INMOVESIZE, GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            GetGUIThreadInfo(
                GetWindowThreadProcessId(windows::Win32::Foundation::HWND(hwnd.0), None),
                &mut info,
            )
        }
        .map_err(|e| e.to_string())?;
        return Ok(info.flags.0 & GUI_INMOVESIZE.0 != 0);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Ok(false)
    }
}

#[tauri::command]
pub fn resize_tool_window(window: tauri::Window, width: f64, height: f64) -> Result<bool, String> {
    let nr = window.label() == "nr_overlay";
    if window.label() != TOOL_WINDOW_ID && !nr {
        return Err("resize_tool_window can only be called from an overlay".to_string());
    }
    if !width.is_finite() || !height.is_finite() {
        return Err("Invalid tool window size".to_string());
    }
    if native_window_move_active(&window)? {
        return Ok(false);
    }
    let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;
    let current_size = window.inner_size().map_err(|e| e.to_string())?;
    let min_height: f64 = if nr { 32.0 } else { 180.0 };
    let max_height = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| (monitor.size().height as f64 / scale_factor - 32.0).max(min_height))
        .unwrap_or(900.0);
    let target_width = width.clamp(if nr { 120.0 } else { 320.0 }, 560.0);
    let target_height = height.clamp(min_height, max_height);
    if (current_size.width as f64 / scale_factor - target_width).abs() >= 1.0
        || (current_size.height as f64 / scale_factor - target_height).abs() >= 1.0
    {
        // Read the current anchor here, not in an earlier frontend IPC request.
        let position = window.outer_position().map_err(|e| e.to_string())?;
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                target_width,
                target_height,
            )))
            .map_err(|e| e.to_string())?;
        if nr {
            window
                .set_position(tauri::PhysicalPosition::new(
                    position.x + current_size.width as i32
                        - (target_width * scale_factor).round() as i32,
                    position.y,
                ))
                .map_err(|e| e.to_string())?;
        }
    }
    constrain_tool_window_to_visible_area(
        &window,
        target_width,
        target_height,
        if nr { 0.0 } else { 16.0 },
    )?;
    Ok(true)
}

/// 显示并激活窗口（解决权限隔离问题）
pub fn show_and_activate_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();

    // 恢复 WebView 活跃状态（引擎级 + JS 级）
    lifecycle::presented(window);

    // 重置代理快速失败状态，确保恢复后首次请求不被拦截
    proxy_server::reset_fast_fail();

    #[cfg(target_os = "windows")]
    force_activate_window_win32(window);

    #[cfg(target_os = "windows")]
    if !matches!(window.label(), MAIN_WINDOW_ID | DESKTOP_WINDOW_ID) {
        refresh_webview_surface(window);
    }
    crate::power::refresh_ecoqos_state(window.app_handle());
}

/// 使用 Windows API 强制激活窗口
#[cfg(target_os = "windows")]
fn force_activate_window_win32<R: Runtime>(window: &WebviewWindow<R>) {
    use log::warn;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, GetWindowThreadProcessId, SW_RESTORE,
        SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    // 直接从 Tauri 窗口获取 HWND，避免使用 FindWindowW（对隐藏窗口不可靠）
    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(win32_handle) => HWND(win32_handle.hwnd.get() as *mut _),
            _ => {
                error!("❌ 无法获取 Win32 窗口句柄：不是 Win32 窗口");
                return;
            }
        },
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
        debug!(
            "📊 当前线程: {}, 目标线程: {}, 目标进程: {}",
            current_tid, target_tid, target_pid
        );

        // 允许任何进程设置前台窗口
        let _ = AllowSetForegroundWindow(ASFW_ANY);

        // 确保窗口可见并恢复
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);

        // 尝试设置为前台窗口
        let result = SetForegroundWindow(hwnd);

        if result.as_bool() {
            debug!("✅ 已使用 Windows API 强制激活窗口");
        } else {
            warn!("⚠️ SetForegroundWindow 返回 FALSE，窗口可能未能激活到前台");
        }
    }
}

/// 为窗口设置 DWM 圆角（仅 Windows 11+ 生效）
/// 让系统在 DWM 层面裁剪窗口圆角，无需 WebView2 透明合成，大幅降低 GPU 开销
#[cfg(target_os = "windows")]
fn apply_dwm_rounded_corners<R: Runtime>(window: &WebviewWindow<R>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut _),
            _ => return,
        },
        Err(_) => return,
    };

    unsafe {
        // DWMWA_WINDOW_CORNER_PREFERENCE = 33
        // DWMWCP_ROUND = 2  (标准圆角，约 8px)
        let preference: u32 = 2;
        let attr = windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(33i32);
        match DwmSetWindowAttribute(
            hwnd,
            attr,
            &preference as *const u32 as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        ) {
            Ok(_) => debug!("✅ 已设置 DWM 圆角 (DWMWCP_ROUND)"),
            Err(e) => debug!("⚠️ DWM 圆角设置失败（Windows 10 不支持）: {:?}", e),
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
        crate::power::refresh_ecoqos_state(app);
        return Ok(window);
    }

    let window = builder_fn(app).map_err(|e| format!("创建窗口失败: {}", e))?;
    crate::power::refresh_ecoqos_state(app);
    Ok(window)
}

/// 打开关于窗口（单例模式）
pub fn open_about_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = get_or_create_window(app, ABOUT_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(
            app,
            ABOUT_WINDOW_ID,
            tauri::WebviewUrl::App("about/index.html".into()),
        )
        .title("关于 Sunshine Control Panel")
        .inner_size(540.0, 620.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(true)
        .decorations(true)
        .center()
        .build()
    })?;

    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

    disable_context_menu(&window);
    debug!("✅ 关于窗口已打开");
    Ok(())
}

/// 打开日志控制台窗口（单例模式）
pub fn open_log_console<R: Runtime>(app: &AppHandle<R>) {
    match get_or_create_window(app, LOG_CONSOLE_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(
            app,
            LOG_CONSOLE_WINDOW_ID,
            tauri::WebviewUrl::App("console/index.html".into()),
        )
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
            #[cfg(target_os = "windows")]
            configure_webview_security(&window);
            disable_context_menu(&window);
            debug!("✅ 日志控制台窗口已打开");
        }
        Err(e) => error!("❌ {}", e),
    }
}

pub fn open_pin_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window(PIN_WINDOW_ID) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        debug!("✅ PIN 窗口已激活");
        crate::power::refresh_ecoqos_state(app);
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(
        app,
        PIN_WINDOW_ID,
        tauri::WebviewUrl::App("pin/index.html".into()),
    )
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

    // 禁用自动填充和密码保存提示
    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = window.show();
    });

    debug!("✅ PIN 配对窗口创建成功");
    crate::power::refresh_ecoqos_state(app);
    Ok(())
}

/// 打开调试页面窗口（仅开发环境）
#[cfg(debug_assertions)]
pub fn open_debug_page<R: Runtime>(app: &AppHandle<R>) {
    match get_or_create_window(app, DEBUG_PAGE_WINDOW_ID, |app| {
        tauri::WebviewWindowBuilder::new(
            app,
            DEBUG_PAGE_WINDOW_ID,
            tauri::WebviewUrl::App("console/drag-drop-demo.html".into()),
        )
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
        Ok(_) => debug!("✅ 调试页面窗口已打开"),
        Err(e) => error!("❌ {}", e),
    }
}

/// 创建主窗口
pub fn create_main_window<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    create_main_window_internal(app, true)
}

/// 创建隐藏的主窗口
pub fn create_main_window_hidden<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    create_main_window_internal(app, false)
}

fn create_main_window_internal<R: Runtime>(
    app: &AppHandle<R>,
    visible: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    proxy_server::ensure_started(app);
    if app.get_webview_window(MAIN_WINDOW_ID).is_some() {
        debug!("主窗口已存在，跳过创建");
        return Ok(());
    }

    let visibility_desc = if visible { "" } else { "隐藏的" };
    info!("🪟 创建{}主窗口...", visibility_desc);

    crate::tray::mark_main_panel_loading();
    let panel = lifecycle::register(MAIN_WINDOW_ID, visible);
    let page_panel = panel.clone();
    let window = tauri::WebviewWindowBuilder::new(
        app,
        MAIN_WINDOW_ID,
        tauri::WebviewUrl::App("sunshine-frame.html".into()),
    )
    .title("Sunshine Control Panel")
    .inner_size(1280.0, 800.0)
    .min_inner_size(1024.0, 600.0)
    .center()
    .decorations(false)
    // DWM clips the native window to rounded corners. Keeping WebView2
    // opaque avoids stale transparent composition surfaces after restore.
    .transparent(false)
    .shadow(false)
    .visible(false)
    .disable_drag_drop_handler()
    .initialization_script(panel.initialization_script())
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            page_panel.page_loaded(window);
        }
    })
    .build()
    .map_err(|e| {
        lifecycle::unregister(MAIN_WINDOW_ID, &panel);
        format!("创建{}主窗口失败: {}", visibility_desc, e)
    })?;

    panel.attach(&window);

    // 设置 DWM 圆角，让系统级合成器裁剪窗口圆角
    #[cfg(target_os = "windows")]
    apply_dwm_rounded_corners(&window);

    // 禁用自动填充和密码保存提示
    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

    disable_context_menu(&window);
    info!("✅ {}主窗口创建成功", visibility_desc);
    crate::power::refresh_ecoqos_state(app);
    Ok(())
}

/// 创建桌面 UI 窗口
pub fn create_desktop_window<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    create_desktop_window_internal(app, true)
}

pub fn create_desktop_window_hidden<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    create_desktop_window_internal(app, false)
}

fn create_desktop_window_internal<R: Runtime>(
    app: &AppHandle<R>,
    visible: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🖥️ 创建桌面 UI 窗口...");

    let panel = lifecycle::register(DESKTOP_WINDOW_ID, false);
    let page_panel = panel.clone();

    let window = tauri::WebviewWindowBuilder::new(
        app,
        DESKTOP_WINDOW_ID,
        tauri::WebviewUrl::App("desktop/index.html".into()),
    )
    .title("Sunshine Desktop")
    .inner_size(1600.0, 900.0)
    .min_inner_size(1024.0, 600.0)
    .center()
    .decorations(false)
    .transparent(false)
    .shadow(false)
    .visible(visible)
    .fullscreen(true)
    .disable_drag_drop_handler()
    .initialization_script(panel.initialization_script())
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            page_panel.page_loaded(window);
        }
    })
    .build()
    .map_err(|e| {
        lifecycle::unregister(DESKTOP_WINDOW_ID, &panel);
        format!("创建桌面窗口失败: {}", e)
    })?;

    panel.attach(&window);

    #[cfg(target_os = "windows")]
    apply_dwm_rounded_corners(&window);

    // 禁用自动填充和密码保存提示
    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

    disable_context_menu(&window);
    info!("✅ 桌面 UI 窗口创建成功");
    crate::power::refresh_ecoqos_state(app);
    Ok(())
}

/// 打开桌面 UI 窗口（单例模式）
pub fn open_desktop_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DESKTOP_WINDOW_ID) {
        show_and_activate_window(&window);
        debug!("✅ 桌面 UI 窗口已激活");
    } else {
        create_desktop_window(app).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 激活主窗口
pub fn activate_main_window(app: &tauri::AppHandle, target_url: Option<String>) {
    debug!(
        "📱 activate_main_window 被调用，target_url: {:?}",
        target_url
    );

    let created = if app.get_webview_window(MAIN_WINDOW_ID).is_none() {
        info!("主窗口尚未创建，正在从用户会话 agent 按需创建");
        if let Err(e) = create_main_window(app) {
            error!("❌ 按需创建主窗口失败: {}", e);
            return;
        }
        true
    } else {
        false
    };

    // Cold windows are revealed by the page-load hook after WebView2 has a
    // composited first frame. The startup page reads any runtime URL itself.
    if created {
        return;
    }

    let Some(window) = app.get_webview_window(MAIN_WINDOW_ID) else {
        error!("❌ 主窗口创建后仍不可用");
        return;
    };

    debug!("📱 正在激活主窗口...");

    show_and_activate_window(&window);

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

    debug!("✅ 窗口激活完成");
}

/// 导航到指定 URL
fn navigate_to_url(window: &WebviewWindow, url: &str) {
    debug!("🔄 正在导航到: {}", url);

    let Ok(parsed_url) = url::Url::parse(url) else {
        error!("❌ URL 解析失败: {}", url);
        return;
    };

    let path = format!(
        "{}{}",
        parsed_url.path(),
        parsed_url
            .query()
            .map(|q| format!("?{}", q))
            .unwrap_or_default()
    );

    if path.contains("/pin") {
        debug!("🔐 检测到 /pin 路径，跳过导航");
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

/// 配置 WebView2 安全设置（禁用浏览器自动填充和密码自动保存）
///
/// WebView2 默认会弹出自动填充/密码保存提示，在嵌入式管理面板中会干扰用户操作。
/// 通过 ICoreWebView2Settings4 接口在引擎级别禁用，比 HTML autocomplete="off" 更可靠。
#[cfg(target_os = "windows")]
pub(crate) fn configure_webview_security<R: Runtime>(window: &WebviewWindow<R>) {
    let label = window.label().to_string();
    let _ = window.with_webview(move |webview| {
        // 引入 webview2-com 的 COM 接口类型
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
        // Interface trait must match the webview2-com bindings used by wry.
        use wv2_windows_core::Interface;

        let controller = webview.controller();
        unsafe {
            let Ok(core_webview) = controller.CoreWebView2() else {
                log::warn!("⚠️ 无法获取 ICoreWebView2 [{}]", label);
                return;
            };
            // Main/desktop are configured once, at creation. Accept a self-signed
            // certificate only for the configured Core, never for external assets.
            if matches!(label.as_str(), MAIN_WINDOW_ID | DESKTOP_WINDOW_ID) {
                use webview2_com::Microsoft::Web::WebView2::Win32::{
                    COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW, ICoreWebView2_14,
                };
                use webview2_com::{ServerCertificateErrorDetectedEventHandler, take_pwstr};
                if let Ok(core14) = core_webview.cast::<ICoreWebView2_14>() {
                    let handler =
                        ServerCertificateErrorDetectedEventHandler::create(Box::new(|_, args| {
                            if let Some(args) = args {
                                let mut uri = wv2_windows_core::PWSTR::null();
                                args.RequestUri(&mut uri)?;
                                let request = take_pwstr(uri);
                                if sunshine_certificate_allowed(
                                    &request,
                                    &proxy_server::get_sunshine_target(),
                                ) {
                                    args.SetAction(
                                        COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW,
                                    )?;
                                }
                            }
                            Ok(())
                        }));
                    let mut token = 0;
                    if let Err(error) =
                        core14.add_ServerCertificateErrorDetected(&handler, &mut token)
                    {
                        log::warn!("Failed to configure Sunshine certificate handling: {error}");
                    }
                }
            }
            let Ok(settings) = core_webview.Settings() else {
                log::warn!("⚠️ 无法获取 ICoreWebView2Settings [{}]", label);
                return;
            };

            // Cast 到 Settings4（WebView2 SDK 1.0.902+，所有现代版本均支持）
            match settings.cast::<ICoreWebView2Settings4>() {
                Ok(settings4) => {
                    // 禁用地址/联系人等通用自动填充
                    let _ = settings4.SetIsGeneralAutofillEnabled(false);
                    // 禁用密码自动保存提示
                    let _ = settings4.SetIsPasswordAutosaveEnabled(false);
                    log::debug!("🔒 已禁用 WebView2 自动填充和密码保存 [{}]", label);
                }
                Err(e) => {
                    log::warn!(
                        "⚠️ 无法获取 ICoreWebView2Settings4（WebView2 版本可能过旧）[{}]: {:?}",
                        label,
                        e
                    );
                }
            }
        }
    });
}

fn sunshine_certificate_allowed(request: &str, target: &str) -> bool {
    let (Ok(request), Ok(target)) = (url::Url::parse(request), url::Url::parse(target)) else {
        return false;
    };
    request.scheme() == "https" && target.scheme() == "https" && request.origin() == target.origin()
}

#[cfg(test)]
mod certificate_tests {
    #[test]
    fn self_signed_exception_requires_the_configured_https_origin() {
        let core = "https://localhost:47990";
        assert!(super::sunshine_certificate_allowed(
            "https://localhost:47990/api/config",
            core
        ));
        assert!(super::sunshine_certificate_allowed(
            "https://192.168.1.10:5000/",
            "https://192.168.1.10:5000"
        ));
        for request in [
            "https://localhost:47991/",
            "https://localhost.evil.example:47990/",
            "https://github.com/",
            "http://localhost:47990/",
            "invalid",
        ] {
            assert!(!super::sunshine_certificate_allowed(request, core));
        }
        assert!(!super::sunshine_certificate_allowed(
            core,
            "http://localhost:47990"
        ));
    }
}

/// Re-submit the existing WebView2 bounds after a native window restore.
/// This invalidates stale composition tiles without navigating or changing
/// any page state.
#[cfg(target_os = "windows")]
fn refresh_webview_surface<R: Runtime>(window: &WebviewWindow<R>) {
    let label = window.label().to_string();
    let _ = window.with_webview(move |webview| {
        let controller = webview.controller();
        unsafe {
            let mut bounds = Default::default();
            if controller.Bounds(&mut bounds).is_ok()
                && let Err(error) = controller.SetBounds(bounds)
            {
                log::debug!("Failed to refresh WebView bounds [{}]: {:?}", label, error);
            }
            let _ = controller.NotifyParentWindowPositionChanged();
        }
    });
}

/// 处理窗口事件
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::Moved(position) => {
            if window.label() == TOOL_WINDOW_ID {
                schedule_tool_window_position_save(window.app_handle().clone(), *position);
            }
        }
        tauri::WindowEvent::CloseRequested { .. } => {
            match window.label() {
                "main" => {
                    // Let the WebView close so the user agent returns to its
                    // headless baseline. The tray and session services keep
                    // the application event loop alive.
                    crate::tray::mark_main_panel_loading();
                }
                "toolbar" => {
                    if let Ok(position) = window.outer_position() {
                        crate::toolbar::save_toolbar_position_internal(
                            &window.app_handle(),
                            position.x as f64,
                            position.y as f64,
                        );
                    }
                }
                TOOL_WINDOW_ID => {
                    if let Ok(position) = window.outer_position() {
                        crate::toolbar::save_tool_window_position_internal(
                            &window.app_handle(),
                            position.x as f64,
                            position.y as f64,
                        );
                    }
                }
                _ => {}
            }
        }
        tauri::WindowEvent::Destroyed => {
            // 延迟重估，等窗口从 Tauri 注册表移除后再判断是否回到托盘常驻态。
            let app_handle = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                crate::power::refresh_ecoqos_state(&app_handle);
            });
        }
        _ => {}
    }
}
