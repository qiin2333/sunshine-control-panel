// 工具栏窗口管理模块

use crate::windows;
use log::{debug, error, warn};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, Runtime};

// 获取工具栏配置文件路径
fn get_toolbar_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;

    // 确保目录存在
    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    }

    Ok(app_data_dir.join("toolbar_config.json"))
}

fn get_tool_window_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to get app data directory: {}", e))?;

    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("failed to create app data directory: {}", e))?;
    }

    Ok(app_data_dir.join("tool_window_config.json"))
}

// 内部保存工具栏位置函数（供窗口事件处理器使用）
pub fn save_toolbar_position_internal<R: Runtime>(app: &AppHandle<R>, x: f64, y: f64) {
    if let Ok(config_path) = get_toolbar_config_path(app) {
        let config = serde_json::json!({
            "x": x,
            "y": y
        });

        if let Err(e) = fs::write(&config_path, config.to_string()) {
            error!("❌ 保存工具栏位置失败: {}", e);
        } else {
            debug!("💾 工具栏位置已保存: ({}, {})", x, y);
        }
    }
}

pub fn save_tool_window_position_internal<R: Runtime>(app: &AppHandle<R>, x: f64, y: f64) {
    if let Ok(config_path) = get_tool_window_config_path(app) {
        let config = serde_json::json!({
            "x": x,
            "y": y
        });

        if let Err(e) = fs::write(&config_path, config.to_string()) {
            error!("failed to save tool window position: {}", e);
        } else {
            debug!("tool window position saved: ({}, {})", x, y);
        }
    }
}

// 保存工具栏位置（Tauri 命令）
#[tauri::command]
pub async fn save_toolbar_position(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    save_toolbar_position_internal(&app, x, y);
    Ok(())
}

// 加载工具栏位置
fn load_toolbar_position<R: Runtime>(app: &AppHandle<R>) -> Option<(f64, f64)> {
    let config_path = match get_toolbar_config_path(app) {
        Ok(path) => path,
        Err(e) => {
            error!("❌ 获取配置路径失败: {}", e);
            return None;
        }
    };

    if !config_path.exists() {
        return None;
    }

    match fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(config) => {
                let x = config["x"].as_f64()?;
                let y = config["y"].as_f64()?;
                debug!("📂 加载工具栏位置: ({}, {})", x, y);
                Some((x, y))
            }
            Err(e) => {
                error!("❌ 解析工具栏配置失败: {}", e);
                None
            }
        },
        Err(e) => {
            error!("❌ 读取工具栏配置失败: {}", e);
            None
        }
    }
}

fn load_tool_window_position<R: Runtime>(app: &AppHandle<R>) -> Option<(f64, f64)> {
    let config_path = match get_tool_window_config_path(app) {
        Ok(path) => path,
        Err(e) => {
            error!("failed to get tool window config path: {}", e);
            return None;
        }
    };

    if !config_path.exists() {
        return None;
    }

    match fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(config) => {
                let x = config["x"].as_f64()?;
                let y = config["y"].as_f64()?;
                debug!("loaded tool window position: ({}, {})", x, y);
                Some((x, y))
            }
            Err(e) => {
                error!("failed to parse tool window config: {}", e);
                None
            }
        },
        Err(e) => {
            error!("failed to read tool window config: {}", e);
            None
        }
    }
}

// Keep persisted floating panel positions visible after display or DPI changes.
fn clamp_tool_window_position<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    position: tauri::PhysicalPosition<i32>,
    logical_width: f64,
    logical_height: f64,
) -> tauri::PhysicalPosition<i32> {
    match window.current_monitor() {
        Ok(Some(monitor)) => windows::clamp_window_position_to_monitor(
            position,
            *monitor.position(),
            *monitor.size(),
            monitor.scale_factor(),
            logical_width,
            logical_height,
            16.0,
        ),
        _ => position,
    }
}

// 辅助函数：创建工具窗口
pub fn create_tool_window_internal<R: Runtime>(app: &AppHandle<R>, tool_type: &str) {
    const TOOL_WINDOW_ID: &str = "tool_window";
    const PERF_WINDOW_WIDTH: f64 = 340.0;
    const PERF_WINDOW_HEIGHT: f64 = 260.0;
    let is_performance_monitor = tool_type == "performance";

    // 如果窗口已存在，先关闭它
    if let Some(window) = app.get_webview_window(TOOL_WINDOW_ID) {
        let _ = window.close();
    }

    // 创建工具窗口，通过 URL 参数传递工具类型
    let url = format!("tool-window/index.html?tool={}", tool_type);
    let title = format!("ZakoToolsWindow - {}", tool_type);
    debug!("🔧 创建工具窗口 URL: {}", url);

    let mut builder =
        tauri::WebviewWindowBuilder::new(app, TOOL_WINDOW_ID, tauri::WebviewUrl::App(url.into()))
            .title(&title)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false);

    if is_performance_monitor {
        builder = builder
            .inner_size(PERF_WINDOW_WIDTH, PERF_WINDOW_HEIGHT)
            .min_inner_size(320.0, 180.0)
            .resizable(true);
    } else {
        builder = builder.fullscreen(true);
    }

    match builder.build() {
        Ok(window) => {
            // 在生产环境禁用右键菜单
            windows::disable_context_menu(&window);

            // 禁用自动填充和密码保存提示
            #[cfg(target_os = "windows")]
            windows::configure_webview_security(&window);

            // 开发模式下自动打开 DevTools
            #[cfg(debug_assertions)]
            {
                if !is_performance_monitor {
                    window.open_devtools();
                    let _ = window.set_always_on_top(false);
                    debug!("🔧 [开发模式] 工具窗口已自动打开 DevTools");
                }
            }

            // 等待一小段时间让内容加载，然后显示窗口
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if is_performance_monitor {
                    if let Some((x, y)) = load_tool_window_position(&window.app_handle()) {
                        let position = clamp_tool_window_position(
                            &window,
                            tauri::PhysicalPosition::new(x as i32, y as i32),
                            PERF_WINDOW_WIDTH,
                            PERF_WINDOW_HEIGHT,
                        );
                        let _ = window.set_position(position);
                    } else if let Ok(Some(monitor)) = window.current_monitor() {
                        let monitor_pos = monitor.position();
                        let monitor_size = monitor.size();
                        let scale = monitor.scale_factor();
                        let margin = (16.0 * scale) as i32;
                        let width = (PERF_WINDOW_WIDTH * scale) as i32;
                        let x = monitor_pos.x + monitor_size.width as i32 - width - margin;
                        let y = monitor_pos.y + margin;
                        let position = clamp_tool_window_position(
                            &window,
                            tauri::PhysicalPosition::new(x, y),
                            PERF_WINDOW_WIDTH,
                            PERF_WINDOW_HEIGHT,
                        );
                        let _ = window.set_position(position);
                    }
                }
                let _ = window.show();
            });
        }
        Err(e) => {
            error!("❌ 创建工具窗口失败: {}", e);
        }
    }
}

// 处理工具栏菜单事件
pub fn handle_toolbar_menu_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    fn show_main_window<R: Runtime>(window: &tauri::WebviewWindow<R>) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }

    fn ensure_main_window<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
        if let Some(window) = app.get_webview_window("main") {
            show_main_window(&window);
            Some(window)
        } else {
            if let Err(e) = windows::create_main_window(app) {
                error!("❌ 创建主窗口失败: {}", e);
                return None;
            }
            app.get_webview_window("main")
        }
    }

    match event_id {
        "main" | "toolbar_main" => {
            ensure_main_window(app);
        }
        "vdd" | "toolbar_vdd" => {
            if let Some(window) = ensure_main_window(app) {
                let _ = window.emit("open-vdd-settings", ());
            }
        }
        "dpi" | "toolbar_dpi" => {
            create_tool_window_internal(app, "dpi");
        }
        "bitrate" | "toolbar_bitrate" => {
            create_tool_window_internal(app, "bitrate");
        }
        "performance" | "toolbar_performance" => {
            create_tool_window_internal(app, "performance");
        }
        "shortcuts" | "toolbar_shortcuts" => {
            create_tool_window_internal(app, "shortcuts");
        }
        "pet" | "toolbar_pet" => {
            create_tool_window_internal(app, "pet");
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
pub fn create_toolbar_window_internal<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    const TOOLBAR_WINDOW_ID: &str = "toolbar";

    // 检查工具栏窗口是否已存在
    if app.get_webview_window(TOOLBAR_WINDOW_ID).is_some() {
        debug!("🔧 工具栏窗口已存在");
        return Ok(());
    }

    debug!("🔧 创建工具栏窗口");

    // 窗口大小和边距配置
    let toolbar_size = 240.0; // 窗口大小（紧凑布局：80px 图标 + 80px 气泡半径 × 2）
    let margin = 20.0; // 距离屏幕边缘的边距

    // 先创建窗口在默认位置
    let window = match tauri::WebviewWindowBuilder::new(
        app,
        TOOLBAR_WINDOW_ID,
        tauri::WebviewUrl::App("toolbar/index.html".into()),
    )
    .title("工具栏")
    .inner_size(toolbar_size, toolbar_size)
    .max_inner_size(toolbar_size, toolbar_size)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false) // 先隐藏，等设置好位置再显示
    .build()
    {
        Ok(win) => {
            // 在生产环境禁用右键菜单
            windows::disable_context_menu(&win);

            // 延迟 500ms 检查窗口尺寸（WebView2 初始化可能意外扩大窗口）
            let win_check = win.clone();
            let target = toolbar_size;
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if let Ok(size) = win_check.inner_size() {
                    let sf = win_check.scale_factor().unwrap_or(1.0);
                    let expected_phys = (target * sf) as u32;
                    if size.width != expected_phys || size.height != expected_phys {
                        warn!(
                            "⚠️ 窗口尺寸异常！期望 {}x{} 实际 {}x{}",
                            expected_phys, expected_phys, size.width, size.height
                        );
                        let _ = win_check.set_size(tauri::Size::Physical(
                            tauri::PhysicalSize::new(expected_phys, expected_phys),
                        ));
                    }
                }
            });

            win
        }
        Err(e) => {
            error!("❌ 创建工具栏窗口失败: {}", e);
            return Err(format!("创建工具栏窗口失败: {}", e));
        }
    };

    // 尝试加载保存的位置，如果没有则使用默认位置（右下角）
    if let Some((saved_x, saved_y)) = load_toolbar_position(app) {
        // 保存的坐标已经是物理像素，需要验证是否在屏幕范围内
        debug!("📂 读取保存的工具栏位置: ({}, {})", saved_x, saved_y);

        // 获取当前显示器信息进行边界检查
        if let Ok(monitor) = window.current_monitor() {
            if let Some(monitor) = monitor {
                let size = monitor.size();
                let scale_factor = monitor.scale_factor();

                // 计算逻辑像素尺寸
                let screen_width = size.width as f64 / scale_factor;
                let screen_height = size.height as f64 / scale_factor;

                // 转换保存的物理坐标为逻辑坐标（用于边界检查）
                let logical_x = saved_x / scale_factor;
                let logical_y = saved_y / scale_factor;

                // 边界保护：确保工具栏至少有一部分可见
                let min_visible = 50.0; // 至少 50px 可见
                let max_x = screen_width - min_visible;
                let max_y = screen_height - min_visible;

                // 检查是否越界
                let is_out_of_bounds = logical_x < -toolbar_size + min_visible
                    || logical_y < -toolbar_size + min_visible
                    || logical_x > max_x
                    || logical_y > max_y;

                if is_out_of_bounds {
                    warn!("⚠️  保存的位置越界，使用默认位置");
                    debug!(
                        "   屏幕尺寸: {}x{}, 保存位置(逻辑): ({}, {})",
                        screen_width, screen_height, logical_x, logical_y
                    );
                    // 使用默认位置（右下角）
                    let x = screen_width - toolbar_size - margin - 60.0;
                    let y = screen_height - toolbar_size - margin - 80.0;

                    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(
                        (x * scale_factor) as i32,
                        (y * scale_factor) as i32,
                    )) {
                        error!("❌ 设置默认位置失败: {}", e);
                    }
                } else {
                    // 位置有效，直接使用
                    debug!("✅ 位置有效，应用保存的位置");
                    if let Err(e) = window
                        .set_position(tauri::PhysicalPosition::new(saved_x as i32, saved_y as i32))
                    {
                        error!("❌ 设置工具栏位置失败: {}", e);
                    }
                }
            } else {
                // 无法获取显示器信息，直接使用保存的位置
                warn!("⚠️  无法获取显示器信息，直接使用保存的位置");
                if let Err(e) = window
                    .set_position(tauri::PhysicalPosition::new(saved_x as i32, saved_y as i32))
                {
                    error!("❌ 设置工具栏位置失败: {}", e);
                }
            }
        } else {
            // 无法获取显示器，直接使用保存的位置
            warn!("⚠️  无法获取当前显示器，直接使用保存的位置");
            if let Err(e) =
                window.set_position(tauri::PhysicalPosition::new(saved_x as i32, saved_y as i32))
            {
                error!("❌ 设置工具栏位置失败: {}", e);
            }
        }
    } else {
        // 获取主显示器信息并计算右下角位置
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

                debug!(
                    "📍 屏幕尺寸: {}x{}, 缩放: {}, 默认工具栏位置: ({}, {})",
                    screen_width, screen_height, scale_factor, x, y
                );

                // 转换为物理坐标
                if let Err(e) = window.set_position(tauri::PhysicalPosition::new(
                    (x * scale_factor) as i32,
                    (y * scale_factor) as i32,
                )) {
                    error!("❌ 设置工具栏位置失败: {}", e);
                }
            }
        }
    }

    // 显示窗口
    if let Err(e) = window.show() {
        error!("❌ 显示工具栏窗口失败: {}", e);
    }

    debug!("✅ 工具栏窗口创建成功");
    Ok(())
}

// Tauri 命令：创建工具栏窗口
#[tauri::command]
pub async fn create_toolbar_window(app: AppHandle) -> Result<(), String> {
    create_toolbar_window_internal(&app)
}

// Tauri 命令：处理工具栏菜单操作
#[tauri::command]
pub async fn handle_toolbar_menu_action(app: AppHandle, action: String) -> Result<(), String> {
    debug!("🔧 处理菜单操作: {}", action);
    handle_toolbar_menu_event(&app, &action);
    Ok(())
}
