use log::{debug, warn};
use std::sync::Mutex;
use tauri::{AppHandle, Runtime};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_notification::NotificationExt;

#[cfg(target_os = "windows")]
use std::{path::PathBuf, sync::OnceLock};

#[cfg(target_os = "windows")]
static WINDOWS_AUMID_REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();
#[cfg(target_os = "windows")]
static WINDOWS_NOTIFICATION_ICON: OnceLock<Result<PathBuf, String>> = OnceLock::new();
static LAST_SHOWN_NOTIFICATION: Mutex<Option<(String, u64)>> = Mutex::new(None);

#[cfg(target_os = "windows")]
const WINDOWS_NOTIFICATION_ICON_BYTES: &[u8] = include_bytes!("../../icons/tray/sunshine.ico");
#[cfg(target_os = "windows")]
const WINDOWS_NOTIFICATION_ICON_FILE: &str = "notification-icon-v2.ico";

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConnectionChange {
    Connected(Option<String>),
    Disconnected(Option<String>),
}

#[derive(Debug, PartialEq, Eq)]
struct NotificationContent {
    title: String,
    body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotificationAction {
    OpenPin,
}

#[cfg(target_os = "windows")]
fn ensure_windows_notification_icon() -> Result<PathBuf, String> {
    WINDOWS_NOTIFICATION_ICON
        .get_or_init(|| {
            let directory = dirs::data_local_dir()
                .ok_or_else(|| "Failed to resolve local application data directory".to_string())?
                .join("Sunshine GUI");
            std::fs::create_dir_all(&directory).map_err(|error| {
                format!("Failed to create notification asset directory: {error}")
            })?;

            let path = directory.join(WINDOWS_NOTIFICATION_ICON_FILE);
            let icon_is_current = std::fs::read(&path)
                .map(|bytes| bytes == WINDOWS_NOTIFICATION_ICON_BYTES)
                .unwrap_or(false);
            if !icon_is_current {
                std::fs::write(&path, WINDOWS_NOTIFICATION_ICON_BYTES)
                    .map_err(|error| format!("Failed to write notification icon: {error}"))?;
            }
            Ok(path)
        })
        .clone()
}

#[cfg(target_os = "windows")]
fn ensure_windows_aumid_registered<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    WINDOWS_AUMID_REGISTRATION
        .get_or_init(|| {
            use winreg::RegKey;
            use winreg::enums::HKEY_CURRENT_USER;

            let identifier = app.config().identifier.clone();
            let icon_uri = ensure_windows_notification_icon()?
                .to_string_lossy()
                .to_string();
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let (key, _) = hkcu
                .create_subkey(format!(r"Software\Classes\AppUserModelId\{identifier}"))
                .map_err(|error| format!("Failed to register notification AUMID: {error}"))?;

            key.set_value("DisplayName", &"Sunshine")
                .and_then(|_| key.set_value("IconBackgroundColor", &"0"))
                .and_then(|_| key.set_value("IconUri", &icon_uri))
                .map_err(|error| format!("Failed to configure notification AUMID: {error}"))
        })
        .clone()
}

pub(super) fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        ensure_windows_aumid_registered(app)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Ok(())
    }
}

/// Show a native desktop notification using the same user preference as the
/// rest of the GUI notification surface. Tray menu state remains available
/// even when native notifications are disabled.
fn is_enabled() -> bool {
    crate::desktop_settings::load_desktop_settings_from_disk().notifications
}

fn connection_is_enabled() -> bool {
    let settings = crate::desktop_settings::load_desktop_settings_from_disk();
    settings.notifications && settings.connection_notify
}

const fn supports_actions() -> bool {
    cfg!(target_os = "windows")
}

fn supports_session_snapshots(state: &crate::sunshine::TrayState) -> bool {
    state
        .capabilities
        .iter()
        .any(|capability| capability == "sessions-v1")
}

fn connection_changes(
    previous: Option<&crate::sunshine::TrayState>,
    current: &crate::sunshine::TrayState,
) -> Vec<ConnectionChange> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    if previous.instance_id != current.instance_id {
        return Vec::new();
    }

    if supports_session_snapshots(previous) && supports_session_snapshots(current) {
        let mut changes = Vec::new();
        for session in &current.sessions {
            if !previous
                .sessions
                .iter()
                .any(|previous_session| previous_session.id == session.id)
            {
                changes.push(ConnectionChange::Connected(
                    (!session.client_name.trim().is_empty())
                        .then(|| session.client_name.trim().to_string()),
                ));
            }
        }
        for session in &previous.sessions {
            if !current
                .sessions
                .iter()
                .any(|current_session| current_session.id == session.id)
            {
                changes.push(ConnectionChange::Disconnected(
                    (!session.client_name.trim().is_empty())
                        .then(|| session.client_name.trim().to_string()),
                ));
            }
        }
        changes
    } else if matches!(previous.status.as_str(), "idle" | "paused") && current.status == "streaming"
    {
        vec![ConnectionChange::Connected(None)]
    } else if previous.status == "streaming" && matches!(current.status.as_str(), "idle" | "paused")
    {
        vec![ConnectionChange::Disconnected(None)]
    } else {
        Vec::new()
    }
}

fn connection_content(
    strings: &super::TrayStrings,
    change: ConnectionChange,
) -> NotificationContent {
    match change {
        ConnectionChange::Connected(client_name) => NotificationContent {
            title: strings.client_connected.to_string(),
            body: client_name
                .map(|name| {
                    strings
                        .client_connected_named
                        .replace("{name}", &super::menu::compact_menu_text(&name, 64))
                })
                .unwrap_or_else(|| strings.client_connected_detail.to_string()),
        },
        ConnectionChange::Disconnected(client_name) => NotificationContent {
            title: strings.client_disconnected.to_string(),
            body: client_name
                .map(|name| {
                    strings
                        .client_disconnected_named
                        .replace("{name}", &super::menu::compact_menu_text(&name, 64))
                })
                .unwrap_or_else(|| strings.client_disconnected_detail.to_string()),
        },
    }
}

fn core_notification_content(
    strings: &super::TrayStrings,
    state: &crate::sunshine::TrayState,
    supports_actions: bool,
) -> NotificationContent {
    let notification = &state.notification;
    if notification.action == "open_pin" {
        let client_name = state.pairing_client_name.trim();
        let title = if client_name.is_empty() {
            strings.incoming_pairing.to_string()
        } else {
            format!(
                "{} · {}",
                strings.incoming_pairing,
                super::menu::compact_menu_text(client_name, 64)
            )
        };
        return NotificationContent {
            title,
            body: if supports_actions {
                strings.pairing_instruction
            } else {
                strings.pairing_menu_instruction
            }
            .to_string(),
        };
    }

    NotificationContent {
        title: if notification.title.trim().is_empty() {
            strings.notification.to_string()
        } else {
            notification.title.trim().to_string()
        },
        body: notification.message.trim().to_string(),
    }
}

pub(super) fn show_connection_change_if_any<R: Runtime>(
    app: &AppHandle<R>,
    previous: Option<&crate::sunshine::TrayState>,
    current: &crate::sunshine::TrayState,
) {
    if !connection_is_enabled() {
        return;
    }

    let strings = super::get_tray_strings();
    for change in connection_changes(previous, current) {
        let content = connection_content(strings, change);
        if let Err(error) = show(app, &content.title, &content.body, None) {
            warn!("{}", error);
        }
    }
}

pub(super) fn show_core_notification_if_new<R: Runtime>(
    app: &AppHandle<R>,
    state: &crate::sunshine::TrayState,
) {
    let notification = &state.notification;
    if !notification.active || notification.id == 0 || !is_enabled() {
        return;
    }

    let key = (state.instance_id.clone(), notification.id);
    {
        let last_shown = LAST_SHOWN_NOTIFICATION.lock().unwrap();
        if last_shown.as_ref() == Some(&key) {
            return;
        }
    }

    let supports_actions = supports_actions();
    let content = core_notification_content(super::get_tray_strings(), state, supports_actions);
    let action = (notification.action == "open_pin" && supports_actions)
        .then_some(NotificationAction::OpenPin);
    match show(app, &content.title, &content.body, action) {
        Ok(()) => {
            *LAST_SHOWN_NOTIFICATION.lock().unwrap() = Some(key);
            debug!("Queued native tray notification {}", notification.id);
        }
        Err(error) => warn!("{}", error),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn show<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
    action: Option<NotificationAction>,
) -> Result<(), String> {
    if !is_enabled() {
        debug!("Native tray notification disabled by desktop settings");
        return Ok(());
    }

    // Unpackaged Win32 apps must register the same AUMID used by the toast
    // backend. Without it, Windows records the notification as delivered but
    // may never render a banner for the application.
    ensure_windows_aumid_registered(app)?;
    show_windows(app, title, body, action)
}

#[cfg(not(target_os = "windows"))]
pub(super) fn show<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
    action: Option<NotificationAction>,
) -> Result<(), String> {
    if !is_enabled() {
        debug!("Native tray notification disabled by desktop settings");
        return Ok(());
    }

    let mut builder = app.notification().builder().title(title).sound("Default");
    if !body.trim().is_empty() {
        builder = builder.body(body);
    }

    builder
        .show()
        .map_err(|error| format!("Failed to show native tray notification: {error}"))?;

    let _ = action;

    Ok(())
}

#[cfg(target_os = "windows")]
fn show_windows<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
    action: Option<NotificationAction>,
) -> Result<(), String> {
    use std::sync::mpsc;
    use tauri_winrt_notification::{IconCrop, Sound, Toast};

    let icon_path = ensure_windows_notification_icon()?;
    let mut toast = Toast::new(&app.config().identifier)
        .title(title)
        .icon(&icon_path, IconCrop::Circular, "Sunshine")
        .sound(Some(Sound::Default));
    if !body.trim().is_empty() {
        toast = toast.text1(body);
    }

    let Some(action) = action else {
        toast
            .show()
            .map_err(|error| format!("Failed to show native tray notification: {error:?}"))?;
        return Ok(());
    };

    let (sender, receiver) = mpsc::channel();
    let activated_sender = sender.clone();
    toast = toast
        .on_activated(move |selected_action| {
            let _ = activated_sender.send(selected_action.is_none());
            Ok(())
        })
        .on_dismissed(move |_| {
            let _ = sender.send(false);
            Ok(())
        });
    toast
        .show()
        .map_err(|error| format!("Failed to show native tray notification: {error:?}"))?;

    let app_handle = app.clone();
    std::thread::Builder::new()
        .name("sunshine-notification-action".to_string())
        .spawn(move || {
            if receiver.recv().unwrap_or(false) {
                match action {
                    NotificationAction::OpenPin => {
                        super::actions::open_pairing_window(&app_handle);
                    }
                }
            }
        })
        .map_err(|error| format!("Failed to start notification action listener: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tray_state(status: &str, instance_id: &str) -> crate::sunshine::TrayState {
        crate::sunshine::TrayState {
            status: status.to_string(),
            instance_id: instance_id.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn connection_changes_use_session_snapshot_names() {
        let mut idle = tray_state("idle", "core-instance");
        idle.capabilities.push("sessions-v1".to_string());
        let mut connected = tray_state("streaming", "core-instance");
        connected.capabilities.push("sessions-v1".to_string());
        connected.sessions.push(crate::sunshine::TrayClientSession {
            id: 7,
            client_name: "Living Room TV".to_string(),
        });

        assert_eq!(
            connection_changes(Some(&idle), &connected),
            vec![ConnectionChange::Connected(Some(
                "Living Room TV".to_string()
            ))]
        );
        assert_eq!(
            connection_changes(Some(&connected), &idle),
            vec![ConnectionChange::Disconnected(Some(
                "Living Room TV".to_string()
            ))]
        );
    }

    #[test]
    fn connection_changes_fall_back_to_streaming_transitions() {
        let idle = tray_state("idle", "core-instance");
        let streaming = tray_state("streaming", "core-instance");
        let paused = tray_state("paused", "core-instance");
        let notification = tray_state("notification", "core-instance");

        assert_eq!(
            connection_changes(Some(&idle), &streaming),
            vec![ConnectionChange::Connected(None)]
        );
        assert_eq!(
            connection_changes(Some(&streaming), &paused),
            vec![ConnectionChange::Disconnected(None)]
        );
        assert!(connection_changes(Some(&streaming), &notification).is_empty());
    }

    #[test]
    fn connection_changes_ignore_initial_state_and_core_restart() {
        let streaming = tray_state("streaming", "core-instance");
        let restarted = tray_state("streaming", "new-core-instance");

        assert!(connection_changes(None, &streaming).is_empty());
        assert!(connection_changes(Some(&streaming), &restarted).is_empty());
    }

    #[test]
    fn connection_copy_has_a_clear_title_and_named_detail() {
        assert_eq!(
            connection_content(
                &super::super::ZH_STRINGS,
                ConnectionChange::Connected(Some("客厅电视".to_string()))
            ),
            NotificationContent {
                title: "客户端已连接".to_string(),
                body: "「客厅电视」已连接到这台电脑。".to_string(),
            }
        );
        assert_eq!(
            connection_content(
                &super::super::EN_STRINGS,
                ConnectionChange::Disconnected(None)
            ),
            NotificationContent {
                title: "Client disconnected".to_string(),
                body: "The streaming connection has ended.".to_string(),
            }
        );
    }

    #[test]
    fn pairing_copy_names_the_client_and_explains_the_click_action() {
        let mut state = tray_state("pairing", "core-instance");
        state.pairing_client_name = "Living Room TV".to_string();
        state.notification.action = "open_pin".to_string();

        assert_eq!(
            core_notification_content(&super::super::EN_STRINGS, &state, true),
            NotificationContent {
                title: "Moonlight pairing request · Living Room TV".to_string(),
                body: "Click this notification to enter the 4-digit PIN shown in Moonlight."
                    .to_string(),
            }
        );
    }
}
