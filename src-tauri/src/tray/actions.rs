use super::*;

pub fn handle_tray_menu_event<R: Runtime + 'static>(app: &AppHandle<R>, menu_id: &str) {
    let s = get_tray_strings();
    match menu_id {
        "open_main_panel" => open_main_panel_from_tray(app, "menu"),
        "open_sunshine" => open_sunshine_web_ui(app),
        "tray_notification_action" => handle_tray_notification_action(app),
        "open_desktop" => {
            info!("🖥️ 托盘菜单：打开桌面 UI");
            open_desktop_gui_from_tray(app, "menu");
        }
        #[cfg(target_os = "windows")]
        "auto_start" => toggle_auto_start(app),
        "vdd_settings" => open_vdd_settings(app),
        "vdd_create" => run_vdd_create_action(app, s.vdd_create, s.vdd_create_confirm),
        "vdd_close" => run_tray_action(app, "vdd_destroy", None),
        "vdd_toggle_keep_enabled" => run_vdd_toggle_action(
            app,
            "vdd_toggle_keep_enabled",
            |state| state.vdd.keep_enabled,
            s.vdd_keep,
            s.vdd_keep_confirm,
        ),
        "vdd_toggle_headless_create" => run_vdd_toggle_action(
            app,
            "vdd_toggle_headless_create",
            |state| state.vdd.headless_create_enabled,
            s.vdd_headless,
            s.vdd_headless_confirm,
        ),
        "import_config" => tray_config::import_config(app),
        "export_config" => tray_config::export_config(app),
        "reset_config" => tray_config::reset_config(app, s.reset_config, s.reset_config_confirm),
        "clear_cache" => {
            confirm_tray_action(app, s.clear_cache, s.clear_cache_confirm, "clear_app")
        }
        "reset_display" => confirm_tray_action(
            app,
            s.reset_display,
            s.reset_display_confirm,
            "reset_display_device_config",
        ),
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
        "prevent_sleep" => toggle_prevent_sleep(app),
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
        "shutdown" => shutdown_sunshine(app),
        "lang_zh" => switch_tray_locale(app, "zh"),
        "lang_en" => switch_tray_locale(app, "en"),
        "lang_ja" => switch_tray_locale(app, "ja"),
        "star_project" => utils::open_url_in_browser("https://www.alkaidlab.com/"),
        "visit_project_sunshine" => {
            utils::open_url_in_browser("https://github.com/AlkaidLab/foundation-sunshine")
        }
        "visit_project_moonlight" => {
            utils::open_url_in_browser("https://github.com/qiin2333/moonlight-vplus")
        }
        "restart" => restart_sunshine(app),
        _ => warn!("⚠️ 未知的托盘菜单事件: {}", menu_id),
    }
}

/// 打开 VDD 设置
fn handle_tray_notification_action<R: Runtime>(app: &AppHandle<R>) {
    let (notification, supports_ack) = CURRENT_TRAY_STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|state| {
            (
                Some(state.notification.clone()),
                state
                    .capabilities
                    .iter()
                    .any(|capability| capability == "notification-ack"),
            )
        })
        .unwrap_or((None, false));

    match notification
        .as_ref()
        .map(|notification| notification.action.as_str())
    {
        Some("open_pin") => {
            open_pairing_window(app);
        }
        Some("open_vdd_settings") => {
            open_vdd_settings(app);
            if supports_ack
                && let Some(notification_id) = notification
                    .as_ref()
                    .map(|notification| notification.id)
                    .filter(|notification_id| *notification_id != 0)
            {
                acknowledge_notification(app, notification_id);
            }
        }
        _ => {
            open_main_panel_from_tray(app, "notification");
            if supports_ack
                && let Some(notification_id) = notification
                    .as_ref()
                    .map(|notification| notification.id)
                    .filter(|notification_id| *notification_id != 0)
            {
                acknowledge_notification(app, notification_id);
            }
        }
    }
}

pub(super) fn open_pairing_window<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match sunshine::get_local_sunshine_url().await {
            Ok(url) => {
                proxy_server::set_sunshine_target(url);
                proxy_server::ensure_started();
                let pin_handle = app_handle.clone();
                if let Err(e) = app_handle.run_on_main_thread(move || {
                    if let Err(e) = windows::open_pin_window(&pin_handle) {
                        error!("Failed to open PIN window from tray notification: {}", e);
                    }
                }) {
                    error!(
                        "Failed to schedule PIN window from tray notification: {}",
                        e
                    );
                }
            }
            Err(e) => {
                error!("Failed to resolve local Sunshine URL for PIN window: {}", e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

fn acknowledge_notification<R: Runtime>(app: &AppHandle<R>, notification_id: u64) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match sunshine::acknowledge_tray_notification(notification_id).await {
            Ok(response) => {
                if let Some(state) = response.tray_state {
                    apply_tray_state_on_main_thread(&app_handle, state);
                }
            }
            Err(e) => debug!(
                "Notification {} could not be acknowledged: {}",
                notification_id, e
            ),
        }
    });
}

fn run_vdd_toggle_action<R: Runtime + 'static>(
    app: &AppHandle<R>,
    action: &'static str,
    current_value: fn(&sunshine::TrayState) -> bool,
    enable_title: &'static str,
    enable_message: &'static str,
) {
    let enabled = CURRENT_TRAY_STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|state| !current_value(state));

    // Windows toggles native check items before dispatching the click event.
    // Restore the authoritative Core state while confirmation is pending.
    refresh_menu(app);

    if enabled == Some(true) {
        with_vdd_prerequisite(app, move |app_handle| {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

            let dialog_handle = app_handle.clone();
            app_handle
                .dialog()
                .message(enable_message)
                .title(enable_title)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::YesNo)
                .show(move |confirmed| {
                    if confirmed {
                        run_tray_action(&dialog_handle, action, Some(true));
                    }
                });
        });
    } else {
        run_tray_action(app, action, enabled);
    }
}

fn run_vdd_create_action<R: Runtime + 'static>(
    app: &AppHandle<R>,
    title: &'static str,
    message: &'static str,
) {
    with_vdd_prerequisite(app, move |app_handle| {
        confirm_tray_action(&app_handle, title, message, "vdd_create");
    });
}

fn with_vdd_prerequisite<R, F>(app: &AppHandle<R>, on_ready: F)
where
    R: Runtime + 'static,
    F: FnOnce(AppHandle<R>) + Send + 'static,
{
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = crate::vdd::get_vdd_status().await;
        let ready = status.as_ref().is_ok_and(crate::vdd::VddStatus::is_usable);

        let main_handle = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            if ready {
                on_ready(main_handle);
                return;
            }

            let message = status
                .map(|status| status.status_text)
                .unwrap_or_else(|error| error);
            emit_message(&main_handle, "warning", &message);
            open_vdd_settings(&main_handle);
        });
    });
}

fn run_tray_action<R: Runtime>(app: &AppHandle<R>, action: &'static str, enabled: Option<bool>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match sunshine::post_tray_action(action, enabled).await {
            Ok(response) => {
                if let Some(state) = response.tray_state {
                    apply_tray_state_on_main_thread(&app_handle, state);
                }

                if response.status {
                    if !response.message.is_empty() {
                        emit_message(&app_handle, "success", &response.message);
                    }
                } else {
                    let message = if response.error.is_empty() {
                        "Tray action failed".to_string()
                    } else {
                        response.error
                    };
                    error!("Tray action '{}' failed: {}", action, message);
                    emit_message(&app_handle, "error", &message);
                }
            }
            Err(e) => {
                debug!("Tray action '{}' skipped: {}", action, e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

fn restart_sunshine<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match sunshine::post_tray_restart_action().await {
            Ok(Some(response)) if !response.status => {
                let message = if response.error.is_empty() {
                    "Sunshine restart was rejected".to_string()
                } else {
                    response.error
                };
                error!("Sunshine restart failed: {}", message);
                emit_message(&app_handle, "error", &message);
            }
            Ok(_) => debug!("Sunshine restart requested"),
            Err(error) => {
                error!("Sunshine restart request failed: {}", error);
                emit_message(&app_handle, "error", &error);
            }
        }
    });
}

fn shutdown_sunshine<R: Runtime>(app: &AppHandle<R>) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let strings = get_tray_strings();
    let app_handle = app.clone();
    app.dialog()
        .message(strings.shutdown_message)
        .title(strings.shutdown)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .show(move |confirmed| {
            if !confirmed {
                return;
            }

            let shutdown_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                match sunshine::post_tray_action("shutdown", None).await {
                    Ok(response) if response.status => shutdown_handle.exit(0),
                    Ok(response) => {
                        let message = if response.error.is_empty() {
                            "Sunshine shutdown was rejected".to_string()
                        } else {
                            response.error
                        };
                        error!("Sunshine shutdown failed: {}", message);
                        emit_message(&shutdown_handle, "error", &message);
                    }
                    Err(error) => {
                        error!("Sunshine shutdown request failed: {}", error);
                        emit_message(&shutdown_handle, "error", &error);
                    }
                }
            });
        });
}

fn open_sunshine_web_ui<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match sunshine::get_local_sunshine_url().await {
            Ok(url) => utils::open_url_in_browser(&url),
            Err(e) => {
                error!("Failed to resolve Sunshine URL from tray: {}", e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

fn confirm_tray_action<R: Runtime>(
    app: &AppHandle<R>,
    title: &'static str,
    message: &'static str,
    action: &'static str,
) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let app_handle = app.clone();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .show(move |confirmed| {
            if confirmed {
                run_tray_action(&app_handle, action, None);
            }
        });
}

fn open_vdd_settings<R: Runtime>(app: &AppHandle<R>) {
    open_main_window_view(app, "open-vdd-settings", "VDD settings");
}

/// 从托盘打开 Web 串流设置
fn open_web_stream_settings<R: Runtime>(app: &AppHandle<R>) {
    open_main_window_view(app, "open-web-stream", "Web stream settings");
}

fn open_main_window_view<R: Runtime + 'static>(
    app: &AppHandle<R>,
    event: &'static str,
    description: &'static str,
) {
    if app.get_webview_window("main").is_none()
        && let Err(e) = windows::create_main_window(app)
    {
        error!("Failed to create main window for {}: {}", description, e);
        return;
    }

    if let Some(window) = app.get_webview_window("main") {
        windows::show_and_activate_window(&window);
        emit_to_main_when_ready(app, event, serde_json::Value::Null);
    }
}

/// 更新 Sunshine 用户模式状态
#[cfg(target_os = "windows")]
async fn update_sunshine_mode_state<R: Runtime + 'static>(app: &AppHandle<R>, check_label: &str) {
    let is_user_mode =
        tokio::task::spawn_blocking(crate::sunshine::is_sunshine_running_in_user_mode_impl)
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(false);
    *SUNSHINE_USER_MODE_STATE.lock().unwrap() = is_user_mode;
    info!(
        "✅ Sunshine 用户模式状态已更新({}): {}",
        check_label, is_user_mode
    );
    let rebuild_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        rebuild_tray_menu(&rebuild_handle);
    });
}

/// 切换 Sunshine 运行模式
#[cfg(target_os = "windows")]
fn toggle_sunshine_mode<R: Runtime>(app: &AppHandle<R>) {
    info!("🔄 托盘菜单：切换 Sunshine 模式");
    refresh_menu(app);
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match crate::sunshine::toggle_sunshine_mode().await {
            Ok(msg) => {
                info!("✅ {}", msg);
                emit_message(&app_handle, "success", &msg);

                // 切换由 UAC 提升的 PowerShell 在后台执行，需预留 UAC + stop + start
                // 两次检查以减少"中间状态"导致的误判
                tokio::time::sleep(Duration::from_secs(6)).await;
                update_sunshine_mode_state(&app_handle, "首次").await;

                tokio::time::sleep(Duration::from_secs(5)).await;
                update_sunshine_mode_state(&app_handle, "二次").await;
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
    } else if let Err(e) = toolbar::create_toolbar_window_internal(app) {
        error!("❌ 创建工具栏失败: {}", e);
    }
}

/// 检查更新（托盘菜单触发，`manual = true`）
fn check_for_updates<R: Runtime>(app: &AppHandle<R>) {
    info!("🔄 托盘菜单：检查更新");
    if app.get_webview_window("main").is_none()
        && let Err(e) = windows::create_main_window(app)
    {
        error!("Failed to create main window for update check: {}", e);
        return;
    }
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
                match serde_json::to_value(&update_info) {
                    Ok(payload) => {
                        emit_to_main_when_ready(&app_handle, "update-available", payload)
                    }
                    Err(e) => error!("Failed to serialize update result: {}", e),
                }
            }
            Ok(None) => {
                // manual=true 时此分支不会触发
                debug!("✅ 已是最新版本");
                update::save_last_check_time(&app_handle);
            }
            Err(e) => {
                error!("❌ 检查更新失败: {}", e);
                emit_to_main_when_ready(
                    &app_handle,
                    "update-check-result",
                    serde_json::json!({ "error": e }),
                );
            }
        }
    });
}

/// 切换防止睡眠功能
#[cfg(target_os = "windows")]
fn toggle_auto_start<R: Runtime + 'static>(app: &AppHandle<R>) {
    let enabled = !desktop_settings::is_combined_auto_start_enabled();
    refresh_menu(app);
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            desktop_settings::set_combined_auto_start_enabled(enabled)
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);

        let menu_handle = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            match result {
                Ok(()) => info!("Combined Core and tray auto-start changed: {}", enabled),
                Err(error) => {
                    error!("Failed to update combined auto-start: {}", error);
                    emit_message(
                        &menu_handle,
                        "error",
                        &format!("Failed to update auto-start: {error}"),
                    );
                }
            }
            refresh_menu(&menu_handle);
        });
    });
}

#[cfg(target_os = "windows")]
fn toggle_prevent_sleep<R: Runtime>(app: &AppHandle<R>) {
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
    drop(state);
    rebuild_tray_menu(app);
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
