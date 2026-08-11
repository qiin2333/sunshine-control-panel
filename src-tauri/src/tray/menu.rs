use super::*;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

pub(super) fn compact_menu_text(value: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let visible_chars = max_chars.saturating_sub(3);
    format!(
        "{}...",
        value.chars().take(visible_chars).collect::<String>()
    )
}

pub(super) fn tray_status_label(
    s: &TrayStrings,
    state: Option<&sunshine::TrayState>,
    connection: CoreConnectionState,
) -> String {
    if connection != CoreConnectionState::Connected {
        let status = match connection {
            CoreConnectionState::Connecting => s.status_connecting,
            CoreConnectionState::Disconnected => s.status_disconnected,
            CoreConnectionState::Connected => unreachable!(),
        };
        return format!("Sunshine · {}", status);
    }

    let status = match state {
        Some(state) if state.status == "streaming" && !state.app_name.trim().is_empty() => {
            format!(
                "{}: {}",
                s.status_streaming,
                compact_menu_text(&state.app_name, 32)
            )
        }
        Some(state) if state.status == "streaming" => s.status_streaming.to_string(),
        Some(state) if state.status == "paused" && !state.app_name.trim().is_empty() => {
            format!(
                "{}: {}",
                s.status_paused,
                compact_menu_text(&state.app_name, 32)
            )
        }
        Some(state) if state.status == "paused" => s.status_paused.to_string(),
        Some(state)
            if state.status == "pairing" && !state.pairing_client_name.trim().is_empty() =>
        {
            format!(
                "{}: {}",
                s.status_pairing,
                compact_menu_text(&state.pairing_client_name, 32)
            )
        }
        Some(state) if state.status == "pairing" => s.status_pairing.to_string(),
        Some(state) if state.status == "notification" => s.status_notification.to_string(),
        Some(state) if state.status == "idle" => s.status_idle.to_string(),
        Some(_) => s.status_idle.to_string(),
        None => s.status_disconnected.to_string(),
    };

    format!("Sunshine · {}", status)
}

fn recovery_action_enabled(connection: CoreConnectionState, recovery_in_progress: bool) -> bool {
    connection == CoreConnectionState::Disconnected && !recovery_in_progress
}

pub(super) fn tray_notification_label(s: &TrayStrings, state: &sunshine::TrayState) -> String {
    let notification = &state.notification;
    if notification.action == "open_pin" {
        if state.pairing_client_name.trim().is_empty() {
            return s.complete_pairing.to_string();
        }
        return format!(
            "{}: {}",
            s.complete_pairing,
            compact_menu_text(&state.pairing_client_name, 32)
        );
    }
    if !notification.title.trim().is_empty() {
        return compact_menu_text(&notification.title, 48);
    }
    if !notification.message.trim().is_empty() {
        return compact_menu_text(&notification.message, 48);
    }
    s.notification.to_string()
}

/// Build a compact native menu. The tray is a fast command surface; detailed
/// configuration remains in the main panel.
pub(super) fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let s = get_tray_strings();
    let current_locale = get_current_locale();
    let (tray_state, connection, recovery_in_progress) = {
        let runtime = TRAY_RUNTIME_STATE.lock().unwrap();
        runtime.menu_snapshot()
    };
    let core_connected = connection == CoreConnectionState::Connected;
    let active_notification = tray_state
        .as_ref()
        .filter(|state| state.notification.active)
        .map(|state| state.notification.clone());
    let (
        core_tray_state_available,
        shutdown_available,
        vdd_active,
        vdd_keep_enabled,
        vdd_headless_create_enabled,
        vdd_cooldown,
    ) = tray_state
        .as_ref()
        .map(|state| {
            (
                core_connected,
                state
                    .capabilities
                    .iter()
                    .any(|capability| capability == "shutdown"),
                state.vdd.active,
                state.vdd.keep_enabled,
                state.vdd.headless_create_enabled,
                state.vdd.cooldown,
            )
        })
        .unwrap_or((false, false, false, false, false, false));

    let status = MenuItem::with_id(
        app,
        "tray_status",
        tray_status_label(s, tray_state.as_ref(), connection),
        false,
        None::<&str>,
    )?;
    let open_main_panel = MenuItem::with_id(
        app,
        "open_main_panel",
        s.open_main_panel,
        true,
        None::<&str>,
    )?;
    #[cfg(target_os = "windows")]
    let recover_service = MenuItem::with_id(
        app,
        "recover_service",
        s.recover_service,
        recovery_action_enabled(connection, recovery_in_progress),
        None::<&str>,
    )?;
    let notification_label = tray_state
        .as_ref()
        .filter(|state| state.notification.active)
        .map(|state| tray_notification_label(s, state))
        .unwrap_or_else(|| s.notification.to_string());
    let notification_item = MenuItem::with_id(
        app,
        "tray_notification_action",
        notification_label,
        active_notification.is_some(),
        None::<&str>,
    )?;
    let primary_separator = PredefinedMenuItem::separator(app)?;

    let open_desktop = MenuItem::with_id(app, "open_desktop", s.open_desktop, true, None::<&str>)?;
    #[cfg(target_os = "windows")]
    let auto_start = CheckMenuItem::with_id(
        app,
        "auto_start",
        s.auto_start,
        true,
        desktop_settings::is_combined_auto_start_enabled(),
        None::<&str>,
    )?;
    let open_sunshine = MenuItem::with_id(
        app,
        "open_sunshine",
        s.open_sunshine,
        core_connected,
        None::<&str>,
    )?;
    let interfaces_submenu = Submenu::with_id_and_items(
        app,
        "interfaces",
        s.interfaces_menu,
        true,
        &[&open_sunshine],
    )?;

    let vdd_settings = MenuItem::with_id(app, "vdd_settings", s.vdd_settings, true, None::<&str>)?;
    let display_separator = PredefinedMenuItem::separator(app)?;
    let vdd_create = MenuItem::with_id(
        app,
        "vdd_create",
        s.vdd_create,
        core_tray_state_available && !vdd_active && !vdd_cooldown,
        None::<&str>,
    )?;
    let vdd_close = MenuItem::with_id(
        app,
        "vdd_close",
        s.vdd_close,
        core_tray_state_available && vdd_active && !vdd_cooldown && !vdd_keep_enabled,
        None::<&str>,
    )?;
    let vdd_keep = CheckMenuItem::with_id(
        app,
        "vdd_toggle_keep_enabled",
        s.vdd_keep,
        core_tray_state_available,
        vdd_keep_enabled,
        None::<&str>,
    )?;
    let vdd_headless = CheckMenuItem::with_id(
        app,
        "vdd_toggle_headless_create",
        s.vdd_headless,
        core_tray_state_available,
        vdd_headless_create_enabled,
        None::<&str>,
    )?;
    let reset_display = MenuItem::with_id(
        app,
        "reset_display",
        s.reset_display,
        core_tray_state_available,
        None::<&str>,
    )?;
    let display_submenu = Submenu::with_id_and_items(
        app,
        "display",
        s.display_menu,
        true,
        &[
            &vdd_settings,
            &display_separator,
            &vdd_create,
            &vdd_close,
            &vdd_keep,
            &vdd_headless,
            &reset_display,
        ],
    )?;

    let show_toolbar = MenuItem::with_id(app, "show_toolbar", s.show_toolbar, true, None::<&str>)?;
    let host_performance = MenuItem::with_id(
        app,
        "host_performance",
        s.host_performance,
        true,
        None::<&str>,
    )?;
    let rtss_control = MenuItem::with_id(app, "rtss_control", s.rtss_control, true, None::<&str>)?;
    #[cfg(any(debug_assertions, feature = "beta"))]
    let web_stream = MenuItem::with_id(app, "web_stream", s.web_stream, true, None::<&str>)?;
    let tools_separator = PredefinedMenuItem::separator(app)?;
    let log_console = MenuItem::with_id(app, "log_console", s.log_console, true, None::<&str>)?;
    #[cfg(debug_assertions)]
    let debug_page = MenuItem::with_id(app, "debug_page", s.debug_page, true, None::<&str>)?;

    #[allow(unused_mut)]
    let mut tools_items: Vec<&dyn tauri::menu::IsMenuItem<R>> =
        vec![&show_toolbar, &host_performance, &rtss_control];
    #[cfg(any(debug_assertions, feature = "beta"))]
    tools_items.push(&web_stream);
    tools_items.push(&tools_separator);
    tools_items.push(&log_console);
    #[cfg(debug_assertions)]
    tools_items.push(&debug_page);
    let tools_submenu = Submenu::with_id_and_items(app, "tools", s.tools_menu, true, &tools_items)?;

    let lang_zh = CheckMenuItem::with_id(
        app,
        "lang_zh",
        "中文",
        true,
        current_locale == "zh",
        None::<&str>,
    )?;
    let lang_en = CheckMenuItem::with_id(
        app,
        "lang_en",
        "English",
        true,
        current_locale == "en",
        None::<&str>,
    )?;
    let lang_ja = CheckMenuItem::with_id(
        app,
        "lang_ja",
        "日本語",
        true,
        current_locale == "ja",
        None::<&str>,
    )?;
    let lang_submenu = Submenu::with_id_and_items(
        app,
        "language",
        s.language,
        true,
        &[&lang_zh, &lang_en, &lang_ja],
    )?;

    #[cfg(target_os = "windows")]
    let prevent_sleep = CheckMenuItem::with_id(
        app,
        "prevent_sleep",
        s.prevent_sleep,
        true,
        *PREVENT_SLEEP_STATE.lock().unwrap(),
        None::<&str>,
    )?;
    #[cfg(target_os = "windows")]
    let restart_user_mode = CheckMenuItem::with_id(
        app,
        "restart_user_mode",
        s.restart_user_mode,
        true,
        *SUNSHINE_USER_MODE_STATE.lock().unwrap(),
        None::<&str>,
    )?;

    let import_config = MenuItem::with_id(
        app,
        "import_config",
        s.import_config,
        core_connected,
        None::<&str>,
    )?;
    let export_config = MenuItem::with_id(
        app,
        "export_config",
        s.export_config,
        core_connected,
        None::<&str>,
    )?;
    let reset_config = MenuItem::with_id(
        app,
        "reset_config",
        s.reset_config,
        core_connected,
        None::<&str>,
    )?;
    let clear_cache = MenuItem::with_id(
        app,
        "clear_cache",
        s.clear_cache,
        core_tray_state_available,
        None::<&str>,
    )?;
    let advanced_submenu = Submenu::with_id_and_items(
        app,
        "advanced",
        s.advanced_menu,
        true,
        &[&reset_config, &clear_cache],
    )?;
    let settings_separator = PredefinedMenuItem::separator(app)?;
    let mut settings_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&lang_submenu];
    #[cfg(target_os = "windows")]
    settings_items.push(&prevent_sleep);
    #[cfg(target_os = "windows")]
    settings_items.push(&restart_user_mode);
    settings_items.push(&settings_separator);
    settings_items.push(&import_config);
    settings_items.push(&export_config);
    settings_items.push(&advanced_submenu);
    let settings_submenu =
        Submenu::with_id_and_items(app, "settings", s.settings_menu, true, &settings_items)?;

    let check_update = MenuItem::with_id(app, "check_update", s.check_update, true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", s.about, true, None::<&str>)?;
    let help_separator = PredefinedMenuItem::separator(app)?;
    let star_project = MenuItem::with_id(app, "star_project", s.star_project, true, None::<&str>)?;
    let visit_project_sunshine = MenuItem::with_id(
        app,
        "visit_project_sunshine",
        s.visit_project_sunshine,
        true,
        None::<&str>,
    )?;
    let visit_project_moonlight = MenuItem::with_id(
        app,
        "visit_project_moonlight",
        s.visit_project_moonlight,
        true,
        None::<&str>,
    )?;
    let help_submenu = Submenu::with_id_and_items(
        app,
        "help",
        s.help_menu,
        true,
        &[
            &check_update,
            &about,
            &help_separator,
            &star_project,
            &visit_project_sunshine,
            &visit_project_moonlight,
        ],
    )?;

    let final_separator = PredefinedMenuItem::separator(app)?;
    let restart = MenuItem::with_id(
        app,
        "restart",
        s.restart,
        core_tray_state_available,
        None::<&str>,
    )?;
    let shutdown = MenuItem::with_id(
        app,
        "shutdown",
        s.shutdown,
        shutdown_available,
        None::<&str>,
    )?;

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&status];
    #[cfg(target_os = "windows")]
    if connection == CoreConnectionState::Disconnected {
        items.push(&recover_service);
    }
    if active_notification.is_some() {
        items.push(&notification_item);
    }
    items.push(&primary_separator);
    items.push(&open_main_panel);
    items.push(&open_desktop);
    #[cfg(target_os = "windows")]
    items.push(&auto_start);
    items.push(&interfaces_submenu);
    items.push(&display_submenu);
    items.push(&tools_submenu);
    items.push(&settings_submenu);
    items.push(&help_submenu);
    items.push(&final_separator);
    items.push(&restart);
    items.push(&shutdown);

    Menu::with_items(app, &items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_is_available_only_while_core_is_disconnected() {
        assert!(recovery_action_enabled(
            CoreConnectionState::Disconnected,
            false
        ));
        assert!(!recovery_action_enabled(
            CoreConnectionState::Disconnected,
            true
        ));
        assert!(!recovery_action_enabled(
            CoreConnectionState::Connecting,
            false
        ));
        assert!(!recovery_action_enabled(
            CoreConnectionState::Connected,
            false
        ));
    }
}
