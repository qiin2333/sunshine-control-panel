use crate::proxy_server;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow,
    webview::PageLoadEvent,
};

struct HeartbeatState {
    created_at: std::time::Instant,
    last_beat: Option<std::time::Instant>,
}

impl HeartbeatState {
    fn new() -> Self {
        Self {
            created_at: std::time::Instant::now(),
            last_beat: None,
        }
    }
}

/// WebView heartbeat state is scoped to one window lifetime. Reusing a stale
/// timestamp for a newly-created window with the same label can make the
/// recovery monitor reload that window while its first page is still loading.
static HEARTBEAT_MAP: Lazy<Mutex<HashMap<String, HeartbeatState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Default)]
struct ToolWindowMoveSave {
    version: u64,
    position: Option<PhysicalPosition<i32>>,
}

static TOOL_WINDOW_MOVE_SAVE: Lazy<Mutex<ToolWindowMoveSave>> =
    Lazy::new(|| Mutex::new(ToolWindowMoveSave::default()));

/// 心跳超时阈值（秒）：超过此时间未收到心跳则认为渲染进程可能崩溃
const HEARTBEAT_STALE_SECS: u64 = 30;
/// Cold WebView2 startup and the first local-proxy navigation can be slower
/// than a normal heartbeat interval. Recovery must not disrupt that startup.
const HEARTBEAT_STARTUP_GRACE_SECS: u64 = 90;

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

/// WebView 可见性控制初始化脚本
/// 窗口最小化或隐藏到托盘时，模拟 Page Visibility API 状态变化，触发 Chromium 内置节流：
/// - hidden 页面的 setTimeout/setInterval 最小间隔从 4ms 变为 1000ms
/// - requestAnimationFrame 停止执行
/// - CSS 动画暂停
/// 同时通过注入 CSS 暂停所有 CSS 动画/过渡，进一步降低 GPU 合成开销
const WEBVIEW_VISIBILITY_INIT_SCRIPT: &str = r#"
(function() {
    let _hidden = false;
    let _pauseStyleEl = null;

    Object.defineProperty(document, 'hidden', {
        get: function() { return _hidden; },
        configurable: true,
    });
    Object.defineProperty(document, 'visibilityState', {
        get: function() { return _hidden ? 'hidden' : 'visible'; },
        configurable: true,
    });

    function pauseAnimations() {
        if (!_pauseStyleEl) {
            _pauseStyleEl = document.createElement('style');
            _pauseStyleEl.id = '__gpu_pause_animations';
            _pauseStyleEl.textContent = '*, *::before, *::after { animation-play-state: paused !important; transition: none !important; }';
        }
        if (!_pauseStyleEl.parentNode) {
            (document.head || document.documentElement).appendChild(_pauseStyleEl);
        }
    }

    function resumeAnimations() {
        if (_pauseStyleEl && _pauseStyleEl.parentNode) {
            _pauseStyleEl.parentNode.removeChild(_pauseStyleEl);
        }
    }

    window.__setWebviewVisibility = function(visible) {
        var wasHidden = _hidden;
        _hidden = !visible;
        if (wasHidden !== _hidden) {
            console.log('[WebView Visibility] ' + (_hidden ? '进入休眠' : '恢复活跃'));
            if (_hidden) {
                pauseAnimations();
            } else {
                resumeAnimations();
            }
            document.dispatchEvent(new Event('visibilitychange'));
        }
    };

    // WebView 心跳：每 10 秒向 Rust 后端发送心跳信号
    // 若渲染进程崩溃，心跳停止，后端检测到后可触发自动恢复
    setInterval(function() {
        try {
            if (window.__TAURI_INTERNALS__) {
                window.__TAURI_INTERNALS__.invoke('webview_heartbeat');
            }
        } catch(e) {}
    }, 10000);
    // 立即发送首次心跳
    try {
        if (window.__TAURI_INTERNALS__) {
            window.__TAURI_INTERNALS__.invoke('webview_heartbeat');
        }
    } catch(e) {}
})();
"#;

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
pub fn webview_heartbeat(webview: tauri::Webview) {
    let label = webview.label().to_string();
    let mut map = HEARTBEAT_MAP.lock().unwrap();
    let state = map.entry(label).or_insert_with(HeartbeatState::new);
    state.last_beat = Some(std::time::Instant::now());
}

fn begin_webview_heartbeat(window_id: &str) {
    HEARTBEAT_MAP
        .lock()
        .unwrap()
        .insert(window_id.to_string(), HeartbeatState::new());
}

fn end_webview_heartbeat(window_id: &str) {
    HEARTBEAT_MAP.lock().unwrap().remove(window_id);
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
#[tauri::command]
pub fn resize_tool_window(window: tauri::Window, width: f64, height: f64) -> Result<(), String> {
    if window.label() != TOOL_WINDOW_ID {
        return Err("resize_tool_window can only be called from the tool window".to_string());
    }

    if !width.is_finite() || !height.is_finite() {
        return Err("Invalid tool window size".to_string());
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
        .map(|monitor| (monitor.size().height as f64 / scale_factor - 32.0).max(180.0))
        .unwrap_or(900.0);
    let target_width = width.clamp(320.0, 560.0);
    let target_height = height.clamp(180.0, max_height);

    if (current_width - target_width).abs() >= 1.0 || (current_height - target_height).abs() >= 1.0
    {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                target_width,
                target_height,
            )))
            .map_err(|e| format!("Failed to resize tool window: {}", e))?;
    }

    constrain_tool_window_to_visible_area(&window, target_width, target_height, 16.0)
}

/// 通过 WebView2 COM API 强制重新加载页面（在渲染进程崩溃后仍可工作）
#[cfg(target_os = "windows")]
fn reload_webview_via_com<R: Runtime>(window: &WebviewWindow<R>) {
    let label = window.label().to_string();
    let _ = window.with_webview(move |webview| {
        let controller = webview.controller();
        unsafe {
            if let Ok(core_webview) = controller.CoreWebView2() {
                match core_webview.Reload() {
                    Ok(_) => log::info!("🔄 WebView 渲染进程恢复：已触发重新加载 [{}]", label),
                    Err(e) => log::warn!("⚠️ WebView 重新加载失败 [{}]: {:?}", label, e),
                }
            }
        }
    });
}

/// 检查指定窗口的心跳是否已超时，若超时则触发 COM 级重新加载
#[cfg(target_os = "windows")]
fn check_and_recover_webview<R: Runtime>(window: &WebviewWindow<R>) {
    let label = window.label().to_string();
    let is_stale = {
        let map = HEARTBEAT_MAP.lock().unwrap();
        match map.get(&label) {
            Some(state) if state.created_at.elapsed().as_secs() >= HEARTBEAT_STARTUP_GRACE_SECS => {
                state
                    .last_beat
                    .is_some_and(|last_beat| last_beat.elapsed().as_secs() > HEARTBEAT_STALE_SECS)
            }
            Some(_) | None => false,
        }
    };

    if is_stale {
        warn!(
            "💀 WebView 心跳超时 [{}]，渲染进程可能已崩溃，尝试恢复...",
            label
        );
        reload_webview_via_com(window);
        // 重置心跳时间，给恢复留出时间
        HEARTBEAT_MAP
            .lock()
            .unwrap()
            .insert(label, HeartbeatState::new());
    }
}

/// 启动 WebView 心跳监控后台任务
/// 定期检查可见窗口的心跳状态，若检测到崩溃则自动恢复
pub fn start_heartbeat_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 等待应用完全启动后再开始监控
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            // 只检查可见的主要窗口（main / desktop）
            for label in &[MAIN_WINDOW_ID, DESKTOP_WINDOW_ID] {
                if let Some(win) = app.get_webview_window(label) {
                    let is_visible = win.is_visible().unwrap_or(false);
                    let is_minimized = win.is_minimized().unwrap_or(true);

                    // 仅当窗口可见且未最小化时检查心跳
                    if is_visible && !is_minimized {
                        #[cfg(target_os = "windows")]
                        check_and_recover_webview(&win);
                    }
                }
            }
        }
    });
}

/// 显示并激活窗口（解决权限隔离问题）
pub fn show_and_activate_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();

    // 恢复 WebView 活跃状态（引擎级 + JS 级）
    set_webview_window_visibility(window, true);

    // 重置代理快速失败状态，确保恢复后首次请求不被拦截
    proxy_server::reset_fast_fail();

    // 检查 WebView 心跳，若渲染进程已崩溃则自动恢复
    #[cfg(target_os = "windows")]
    check_and_recover_webview(window);

    #[cfg(target_os = "windows")]
    force_activate_window_win32(window);
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
        return Ok(window);
    }

    builder_fn(app).map_err(|e| format!("创建窗口失败: {}", e))
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
    proxy_server::ensure_started();
    if app.get_webview_window(MAIN_WINDOW_ID).is_some() {
        debug!("主窗口已存在，跳过创建");
        return Ok(());
    }

    let visibility_desc = if visible { "" } else { "隐藏的" };
    info!("🪟 创建{}主窗口...", visibility_desc);

    crate::tray::mark_main_panel_loading();
    begin_webview_heartbeat(MAIN_WINDOW_ID);

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
    .transparent(true)
    .shadow(false)
    .visible(false)
    .disable_drag_drop_handler()
    .initialization_script(WEBVIEW_VISIBILITY_INIT_SCRIPT)
    .on_page_load(move |window, payload| {
        if visible && payload.event() == PageLoadEvent::Finished {
            tauri::async_runtime::spawn(async move {
                // WebView2 can report a completed DOM before its compositor
                // has submitted the first frame. Avoid exposing that black
                // frame, then present the built-in loading shell or panel.
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                show_and_activate_window(&window);
            });
        }
    })
    .build()
    .map_err(|e| {
        end_webview_heartbeat(MAIN_WINDOW_ID);
        format!("创建{}主窗口失败: {}", visibility_desc, e)
    })?;

    // 设置 DWM 圆角，让系统级合成器裁剪窗口圆角
    #[cfg(target_os = "windows")]
    apply_dwm_rounded_corners(&window);

    // 禁用自动填充和密码保存提示
    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

    disable_context_menu(&window);
    info!("✅ {}主窗口创建成功", visibility_desc);
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

    begin_webview_heartbeat(DESKTOP_WINDOW_ID);

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
    .initialization_script(WEBVIEW_VISIBILITY_INIT_SCRIPT)
    .build()
    .map_err(|e| {
        end_webview_heartbeat(DESKTOP_WINDOW_ID);
        format!("创建桌面窗口失败: {}", e)
    })?;

    #[cfg(target_os = "windows")]
    apply_dwm_rounded_corners(&window);

    // 禁用自动填充和密码保存提示
    #[cfg(target_os = "windows")]
    configure_webview_security(&window);

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

    let is_visible = window.is_visible().unwrap_or(false);
    let is_minimized = window.is_minimized().unwrap_or(false);

    if is_minimized {
        let _ = window.unminimize();
    }
    if !is_visible {
        let _ = window.show();
    }
    let _ = window.set_focus();

    // 恢复 WebView 活跃状态（引擎级 + JS 级）
    set_webview_window_visibility(&window, true);

    // 重置代理快速失败状态
    proxy_server::reset_fast_fail();

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

/// 通过 WebView2 原生 API 设置 IsVisible 状态（引擎级定时器节流 + 渲染暂停）
/// hidden 状态下 WebView2 会暂停渲染管线和 GPU 合成，大幅降低 GPU 占用
#[cfg(target_os = "windows")]
fn set_webview_native_visibility<R: Runtime>(window: &WebviewWindow<R>, visible: bool) {
    let label = window.label().to_string();
    let _ = window.with_webview(move |webview| {
        let controller = webview.controller();
        unsafe {
            // 核心：暂停/恢复 WebView 渲染管线
            // SetIsVisible(false) 会：
            //   - 停止 GPU 合成器帧生成（GPU 占用降至 ~0%）
            //   - 冻结 requestAnimationFrame 回调
            //   - 降低 setTimeout/setInterval 分辨率到 ~1000ms
            //   - 暂停 CSS 动画和过渡
            match controller.SetIsVisible(visible) {
                Ok(_) => log::debug!(
                    "{} WebView native IsVisible={} [{}]",
                    if visible { "👁️" } else { "💤" },
                    visible,
                    label
                ),
                Err(e) => log::debug!("⚠️ SetIsVisible({}) 失败 [{}]: {:?}", visible, label, e),
            }

            // 尝试设置默认背景色为不透明黑色（隐藏时减少 alpha 合成开销）
            // ICoreWebView2Controller2::put_DefaultBackgroundColor
            // 注：仅在窗口隐藏时设为不透明以降低 GPU，恢复时还原透明以保留圆角效果
            if !visible {
                // 不改变背景色，保持一致的视觉效果
                // 未来可以考虑：隐藏时 SetDefaultBackgroundColor 为不透明色进一步降低 GPU
            }
        }
    });
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
        // 引入 windows-core 0.61 的 Interface trait（与 webview2-com 版本一致）
        use wv2_windows_core::Interface;

        let controller = webview.controller();
        unsafe {
            let Ok(core_webview) = controller.CoreWebView2() else {
                log::warn!("⚠️ 无法获取 ICoreWebView2 [{}]", label);
                return;
            };
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

/// 设置 WebView 的可见性状态（引擎级 + JS 级双重控制）
fn set_webview_visibility(window: &tauri::Window, visible: bool) {
    let label = window.label().to_string();
    if let Some(webview_window) = window.app_handle().get_webview_window(&label) {
        set_webview_window_visibility(&webview_window, visible);
    }
}

/// 设置 WebviewWindow 的可见性状态（引擎级 + JS 级双重控制）
fn set_webview_window_visibility<R: Runtime>(ww: &WebviewWindow<R>, visible: bool) {
    // 1. 原生 WebView2 API：引擎级节流（暂停渲染、降低定时器频率）
    #[cfg(target_os = "windows")]
    set_webview_native_visibility(ww, visible);

    // 2. JS 层：通知前端代码（触发 visibilitychange 事件）
    let label = ww.label();
    let js = format!(
        "if(window.__setWebviewVisibility)window.__setWebviewVisibility({})",
        visible
    );
    if let Err(e) = ww.eval(&js) {
        debug!("⚠️ 设置 WebView 可见性失败 [{}]: {}", label, e);
    }
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
            end_webview_heartbeat(window.label());
            match window.label() {
                "main" => {
                    // Let the WebView close so the user agent returns to its
                    // headless baseline. The tray and session services keep
                    // the application event loop alive.
                    set_webview_visibility(window, false);
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
        tauri::WindowEvent::Focused(focused) => {
            // 仅对有 visibility init script 的窗口处理（main 和 desktop）
            let label = window.label();
            if label != MAIN_WINDOW_ID && label != DESKTOP_WINDOW_ID {
                return;
            }

            if *focused {
                // 窗口获得焦点时恢复 WebView 活跃状态
                set_webview_visibility(window, true);
                // 重置代理快速失败状态
                proxy_server::reset_fast_fail();
            } else {
                // 失去焦点时，延迟检查是否处于最小化或隐藏状态
                let app_handle = window.app_handle().clone();
                let label = window.label().to_string();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    if let Some(ww) = app_handle.get_webview_window(&label) {
                        let is_minimized = ww.is_minimized().unwrap_or(false);
                        let is_visible = ww.is_visible().unwrap_or(true);
                        if is_minimized || !is_visible {
                            set_webview_window_visibility(&ww, false);
                            debug!(
                                "💤 WebView 进入休眠 [{}]: minimized={}, visible={}",
                                label, is_minimized, is_visible
                            );
                        }
                    }
                });
            }
        }
        _ => {}
    }
}
