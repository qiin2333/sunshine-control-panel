use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

#[cfg(target_os = "windows")]
use crate::desktop_settings;
use crate::{proxy_server, sunshine, toolbar, tray_config, update, utils, windows};

mod actions;
mod events;
mod icons;
mod main_panel;
mod menu;
mod notifications;
#[cfg(target_os = "windows")]
mod vdd_confirmation;

pub use actions::{cleanup_prevent_sleep, handle_tray_menu_event};
use menu::{build_tray_menu, tray_status_label};
#[cfg(test)]
use menu::{compact_menu_text, tray_notification_label};

// 托盘图标 ID
const TRAY_ID: &str = "main-tray";

// 防止睡眠状态管理
static PREVENT_SLEEP_STATE: Mutex<bool> = Mutex::new(false);

// Sunshine 用户模式状态管理
#[cfg(target_os = "windows")]
static SUNSHINE_USER_MODE_STATE: Mutex<bool> = Mutex::new(false);

// 当前语言状态管理 ("zh" 或 "en")
static CURRENT_LOCALE: Mutex<Option<String>> = Mutex::new(None);
static LOCALE_CHANGE_REVISION: AtomicU64 = AtomicU64::new(0);
static LOCALE_PERSIST_LOCK: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrayLocaleChangedPayload {
    locale: String,
    source: &'static str,
    request_id: Option<String>,
    revision: u64,
}

fn next_locale_change_revision() -> u64 {
    LOCALE_CHANGE_REVISION.fetch_add(1, Ordering::AcqRel) + 1
}

async fn persist_latest_locale(revision: u64, locale: String) -> Result<bool, String> {
    let _guard = LOCALE_PERSIST_LOCK.lock().await;
    if revision != LOCALE_CHANGE_REVISION.load(Ordering::Acquire) {
        return Ok(false);
    }

    sunshine::set_sunshine_locale(locale).await?;
    Ok(true)
}

fn emit_tray_locale_changed<R: Runtime>(
    app: &AppHandle<R>,
    payload: &TrayLocaleChangedPayload,
) -> Result<(), String> {
    app.emit("tray-locale-changed", payload)
        .map_err(|error| format!("Failed to broadcast locale change: {error}"))
}

// Last icon name applied from the Sunshine core state. This avoids repeatedly
// decoding the same .ico file during the polling loop.
static CURRENT_CORE_ICON: Mutex<Option<String>> = Mutex::new(None);
static MONITORED_STATE_RECEIPT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Default)]
struct RecoveryState {
    last_attempt: u64,
    active_attempt: Option<(u64, u64)>,
}

impl RecoveryState {
    fn begin(&mut self, monitored_state_baseline: u64) -> Option<u64> {
        if self.active_attempt.is_some() {
            return None;
        }
        self.last_attempt = self.last_attempt.wrapping_add(1).max(1);
        self.active_attempt = Some((self.last_attempt, monitored_state_baseline));
        Some(self.last_attempt)
    }

    fn finish(&mut self, attempt: u64) -> bool {
        if self.active_attempt.map(|active| active.0) != Some(attempt) {
            return false;
        }
        self.active_attempt = None;
        true
    }

    fn complete_from_monitored_state(&mut self, receipt: u64) -> bool {
        let Some((_, baseline)) = self.active_attempt else {
            return false;
        };
        if receipt <= baseline {
            return false;
        }
        self.active_attempt = None;
        true
    }

    fn is_in_progress(&self) -> bool {
        self.active_attempt.is_some()
    }
}

struct TrayRuntimeState {
    connection: CoreConnectionState,
    tray_state: Option<sunshine::TrayState>,
    recovery: RecoveryState,
}

impl TrayRuntimeState {
    fn menu_snapshot(&self) -> (Option<sunshine::TrayState>, CoreConnectionState, bool) {
        (
            self.tray_state.clone(),
            self.connection,
            self.recovery.is_in_progress(),
        )
    }

    fn begin_recovery_if_disconnected(&mut self, monitored_state_baseline: u64) -> Option<u64> {
        if self.connection != CoreConnectionState::Disconnected {
            return None;
        }
        self.recovery.begin(monitored_state_baseline)
    }

    fn apply_connected_state(
        &mut self,
        state: &sunshine::TrayState,
        monitored_state_receipt: Option<u64>,
    ) -> Option<(Option<sunshine::TrayState>, bool)> {
        if monitored_state_receipt.is_none() && self.connection != CoreConnectionState::Connected {
            return None;
        }
        // Only a monitor response accepted after recovery began is authoritative.
        // Action responses may already be in flight when the attempt starts.
        let recovery_completed = monitored_state_receipt
            .is_some_and(|receipt| self.recovery.complete_from_monitored_state(receipt));
        let connection_changed = self.connection != CoreConnectionState::Connected;
        self.connection = CoreConnectionState::Connected;

        let previous_state = self.tray_state.clone();
        let state_changed = self.tray_state.as_ref() != Some(state);
        self.tray_state = Some(state.clone());

        Some((
            previous_state,
            connection_changed || state_changed || recovery_completed,
        ))
    }

    fn mark_disconnected(&mut self) -> bool {
        let connection_changed = self.connection != CoreConnectionState::Disconnected;
        self.connection = CoreConnectionState::Disconnected;
        connection_changed || self.tray_state.take().is_some()
    }
}

static TRAY_RUNTIME_STATE: Mutex<TrayRuntimeState> = Mutex::new(TrayRuntimeState {
    connection: CoreConnectionState::Connecting,
    tray_state: None,
    recovery: RecoveryState {
        last_attempt: 0,
        active_attempt: None,
    },
});

static MAIN_PANEL_BRIDGE: main_panel::Bridge = main_panel::Bridge::new();

pub(crate) fn emit_message<R: Runtime>(app: &AppHandle<R>, msg_type: &str, message: &str) {
    let payload = serde_json::json!({
        "type": msg_type,
        "message": message
    });
    let mut has_visible_window = false;

    for label in ["main", "desktop"] {
        if let Some(window) = app.get_webview_window(label) {
            has_visible_window |= window.is_visible().unwrap_or(false);
            let _ = window.emit("show-message", &payload);
        }
    }

    if msg_type == "error" && !has_visible_window {
        use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

        app.dialog()
            .message(message)
            .title("Sunshine Error")
            .kind(MessageDialogKind::Error)
            .show(|_| {});
    }
}

/// 托盘菜单翻译结构
struct TrayStrings {
    status_idle: &'static str,
    status_streaming: &'static str,
    status_paused: &'static str,
    status_pairing: &'static str,
    status_notification: &'static str,
    status_connecting: &'static str,
    status_disconnected: &'static str,
    notification: &'static str,
    audio_output_change_title: &'static str,
    audio_output_change_body: &'static str,
    audio_output_unknown_device: &'static str,
    audio_output_change_allow: &'static str,
    audio_output_change_keep: &'static str,
    client_connected: &'static str,
    client_connected_detail: &'static str,
    client_connected_named: &'static str,
    suspicious_client_warning: &'static str,
    client_disconnected: &'static str,
    client_disconnected_detail: &'static str,
    client_disconnected_named: &'static str,
    incoming_pairing: &'static str,
    pairing_instruction: &'static str,
    pairing_menu_instruction: &'static str,
    complete_pairing: &'static str,
    open_main_panel: &'static str,
    interfaces_menu: &'static str,
    display_menu: &'static str,
    tools_menu: &'static str,
    settings_menu: &'static str,
    advanced_menu: &'static str,
    help_menu: &'static str,
    open_sunshine: &'static str,
    vdd_settings: &'static str,
    vdd_create: &'static str,
    vdd_create_confirm: &'static str,
    vdd_close: &'static str,
    vdd_keep: &'static str,
    vdd_keep_confirm: &'static str,
    vdd_headless: &'static str,
    vdd_headless_confirm: &'static str,
    import_config: &'static str,
    export_config: &'static str,
    reset_config: &'static str,
    reset_config_confirm: &'static str,
    clear_cache: &'static str,
    clear_cache_confirm: &'static str,
    reset_display: &'static str,
    reset_display_confirm: &'static str,
    restart_user_mode: &'static str,
    show_toolbar: &'static str,
    prevent_sleep: &'static str,
    rtss_control: &'static str,
    host_performance: &'static str,
    log_console: &'static str,
    open_desktop: &'static str,
    #[cfg(target_os = "windows")]
    auto_start: &'static str,
    #[cfg(any(debug_assertions, feature = "beta"))]
    web_stream: &'static str,
    #[cfg(debug_assertions)]
    debug_page: &'static str,
    check_update: &'static str,
    about: &'static str,
    shutdown: &'static str,
    shutdown_message: &'static str,
    language: &'static str,
    star_project: &'static str,
    visit_project_sunshine: &'static str,
    visit_project_moonlight: &'static str,
    restart: &'static str,
    recover_service: &'static str,
    recovery_timeout: &'static str,
    tooltip: &'static str,
    tooltip_admin: &'static str,
}

const ZH_STRINGS: TrayStrings = TrayStrings {
    status_idle: "空闲",
    status_streaming: "串流中",
    status_paused: "串流已暂停",
    status_pairing: "等待配对",
    status_notification: "有新通知",
    status_connecting: "正在连接",
    status_disconnected: "未连接",
    notification: "通知",
    audio_output_change_title: "音频输出切换已暂停",
    audio_output_change_body: "是否允许切换到“{name}”？允许后，本次串流的客户端将停止收到主机音频。",
    audio_output_unknown_device: "请求的音频设备",
    audio_output_change_allow: "允许本次切换",
    audio_output_change_keep: "保持当前输出",
    client_connected: "客户端已连接",
    client_connected_detail: "已建立新的串流连接。",
    client_connected_named: "「{name}」已连接到这台电脑。",
    suspicious_client_warning: "警告：您很可能正在受到未知侵权客户端的侵害。该判断基于客户端网络特征，可能存在误判。",
    client_disconnected: "客户端已断开",
    client_disconnected_detail: "串流连接已结束。",
    client_disconnected_named: "「{name}」已从这台电脑断开。",
    incoming_pairing: "Moonlight 配对请求",
    pairing_instruction: "点击此通知，输入 Moonlight 上显示的 4 位 PIN。",
    pairing_menu_instruction: "打开 Sunshine 托盘菜单中的“完成配对”，输入 Moonlight 上显示的 4 位 PIN。",
    complete_pairing: "完成配对",
    open_main_panel: "打开主面板",
    interfaces_menu: "其他界面",
    display_menu: "显示",
    tools_menu: "工具",
    settings_menu: "设置",
    advanced_menu: "高级",
    help_menu: "帮助",
    open_sunshine: "Web 管理界面",
    vdd_settings: "基地显示器设置（ZakoVDD）",
    vdd_create: "创建基地显示器",
    vdd_create_confirm: "创建基地显示器可能会更改 Windows 显示布局。是否继续？",
    vdd_close: "关闭基地显示器",
    vdd_keep: "串流结束后保留",
    vdd_keep_confirm: "启用后，串流结束时将保留基地显示器，并可能更改 Windows 显示布局。是否继续？",
    vdd_headless: "无头模式自动创建",
    vdd_headless_confirm: "这是实验性恢复功能：未检测到显示器时会自动创建基地显示器，并可能更改当前显示布局。是否继续？",
    import_config: "导入配置",
    export_config: "导出配置",
    reset_config: "重置配置",
    reset_config_confirm: "这将清空 sunshine.conf，并让 Sunshine 恢复默认配置。是否继续？",
    clear_cache: "清理缓存",
    clear_cache_confirm: "这将终止当前运行的应用，以便重建缓存状态。是否继续？",
    reset_display: "重置显示",
    reset_display_confirm: "这将重置已保存的显示设备状态。是否继续？",
    restart_user_mode: "以用户模式运行 Sunshine",
    show_toolbar: "显示桌宠",
    prevent_sleep: "阻止系统休眠",
    rtss_control: "RTSS 控制",
    host_performance: "主机性能",
    log_console: "日志控制台",
    open_desktop: "大屏模式",
    #[cfg(target_os = "windows")]
    auto_start: "开机运行",
    #[cfg(any(debug_assertions, feature = "beta"))]
    web_stream: "Web 串流服务",
    #[cfg(debug_assertions)]
    debug_page: "调试页面",
    check_update: "检查更新",
    about: "关于 Sunshine",
    shutdown: "退出 Sunshine",
    shutdown_message: "这将停止 Sunshine 服务、GUI 和托盘。确定要退出吗？",
    language: "语言",
    star_project: "项目主页",
    visit_project_sunshine: "Sunshine 源代码",
    visit_project_moonlight: "Moonlight 源代码",
    restart: "重启 Sunshine",
    recover_service: "重新启动 Sunshine 服务",
    recovery_timeout: "Sunshine 服务未能在等待时间内恢复，请检查服务日志后重试。",
    tooltip: "Sunshine GUI",
    tooltip_admin: "Sunshine GUI (管理员)",
};

const EN_STRINGS: TrayStrings = TrayStrings {
    status_idle: "Idle",
    status_streaming: "Streaming",
    status_paused: "Stream paused",
    status_pairing: "Pairing",
    status_notification: "New notification",
    status_connecting: "Connecting",
    status_disconnected: "Disconnected",
    notification: "Notification",
    audio_output_change_title: "Audio output change paused",
    audio_output_change_body: "Allow switching to “{name}”? The streaming client will stop receiving host audio.",
    audio_output_unknown_device: "the requested audio device",
    audio_output_change_allow: "Allow This Change",
    audio_output_change_keep: "Keep Current Output",
    client_connected: "Client connected",
    client_connected_detail: "A new streaming connection is ready.",
    client_connected_named: "“{name}” connected to this computer.",
    suspicious_client_warning: "Warning: You are very likely being affected by an unknown infringing client. This heuristic assessment may be inaccurate.",
    client_disconnected: "Client disconnected",
    client_disconnected_detail: "The streaming connection has ended.",
    client_disconnected_named: "“{name}” disconnected from this computer.",
    incoming_pairing: "Moonlight pairing request",
    pairing_instruction: "Click this notification to enter the 4-digit PIN shown in Moonlight.",
    pairing_menu_instruction: "Open Complete Pairing from the Sunshine tray menu and enter the 4-digit PIN shown in Moonlight.",
    complete_pairing: "Complete pairing",
    open_main_panel: "Open Main Panel",
    interfaces_menu: "Other Interfaces",
    display_menu: "Display",
    tools_menu: "Tools",
    settings_menu: "Settings",
    advanced_menu: "Advanced",
    help_menu: "Help",
    open_sunshine: "Web Management UI",
    vdd_settings: "Virtual Display Settings",
    vdd_create: "Create Virtual Display",
    vdd_create_confirm: "This creates a virtual display and may change the Windows display layout. Continue?",
    vdd_close: "Close Virtual Display",
    vdd_keep: "Keep After Streaming",
    vdd_keep_confirm: "Keep mode leaves the virtual display enabled after streaming ends and may change the Windows display layout. Continue?",
    vdd_headless: "Auto-create When Headless",
    vdd_headless_confirm: "This experimental recovery feature creates a virtual display when no display is detected and may change the active display layout. Continue?",
    import_config: "Import Config",
    export_config: "Export Config",
    reset_config: "Reset Config",
    reset_config_confirm: "This clears sunshine.conf and restores Sunshine defaults. Continue?",
    clear_cache: "Clear Cache",
    clear_cache_confirm: "This terminates the running application so cached state can be rebuilt. Continue?",
    reset_display: "Reset Display",
    reset_display_confirm: "This resets persisted display-device state. Continue?",
    restart_user_mode: "Run Sunshine in User Mode",
    show_toolbar: "Show Desktop Pet",
    prevent_sleep: "Prevent System Sleep",
    rtss_control: "RTSS Control",
    host_performance: "Host Performance",
    log_console: "Log Console",
    open_desktop: "Large Screen Mode",
    #[cfg(target_os = "windows")]
    auto_start: "Run at Startup",
    #[cfg(any(debug_assertions, feature = "beta"))]
    web_stream: "Web Streaming",
    #[cfg(debug_assertions)]
    debug_page: "Debug Page",
    check_update: "Check for Updates",
    about: "About Sunshine",
    shutdown: "Exit Sunshine",
    shutdown_message: "This will stop the Sunshine service, GUI, and tray. Continue?",
    language: "Language",
    star_project: "Project Website",
    visit_project_sunshine: "Sunshine Source Code",
    visit_project_moonlight: "Moonlight Source Code",
    restart: "Restart Sunshine",
    recover_service: "Restart Sunshine Service",
    recovery_timeout: "The Sunshine service did not recover in time. Check the service log and try again.",
    tooltip: "Sunshine GUI",
    tooltip_admin: "Sunshine GUI (Admin)",
};

const JA_STRINGS: TrayStrings = TrayStrings {
    status_idle: "待機中",
    status_streaming: "ストリーミング中",
    status_paused: "ストリーム一時停止",
    status_pairing: "ペアリング待機中",
    status_notification: "新しい通知",
    status_connecting: "接続中",
    status_disconnected: "未接続",
    notification: "通知",
    audio_output_change_title: "音声出力の切り替えを保留しました",
    audio_output_change_body: "「{name}」への切り替えを許可しますか？許可すると、ストリーミングクライアントではホストの音声を受信できなくなります。",
    audio_output_unknown_device: "要求された音声デバイス",
    audio_output_change_allow: "今回の切り替えを許可",
    audio_output_change_keep: "現在の出力を維持",
    client_connected: "クライアントが接続しました",
    client_connected_detail: "新しいストリーミング接続が確立されました。",
    client_connected_named: "「{name}」がこのコンピューターに接続しました。",
    suspicious_client_warning: "警告：不明な権利侵害クライアントが使用されている可能性が非常に高いです。この判定はヒューリスティックであり、誤判定の可能性があります。",
    client_disconnected: "クライアントが切断されました",
    client_disconnected_detail: "ストリーミング接続が終了しました。",
    client_disconnected_named: "「{name}」がこのコンピューターから切断されました。",
    incoming_pairing: "Moonlight ペアリング要求",
    pairing_instruction: "この通知をクリックし、Moonlight に表示された 4 桁の PIN を入力してください。",
    pairing_menu_instruction: "Sunshine のトレイメニューから「ペアリングを完了」を開き、Moonlight に表示された 4 桁の PIN を入力してください。",
    complete_pairing: "ペアリングを完了",
    open_main_panel: "メインパネルを開く",
    interfaces_menu: "その他のインターフェース",
    display_menu: "ディスプレイ",
    tools_menu: "ツール",
    settings_menu: "設定",
    advanced_menu: "詳細設定",
    help_menu: "ヘルプ",
    open_sunshine: "Web 管理画面",
    vdd_settings: "仮想ディスプレイ設定",
    vdd_create: "仮想ディスプレイを作成",
    vdd_create_confirm: "仮想ディスプレイを作成すると、Windows のディスプレイ配置が変更される場合があります。続行しますか？",
    vdd_close: "仮想ディスプレイを閉じる",
    vdd_keep: "ストリーミング後も保持",
    vdd_keep_confirm: "有効にすると、ストリーミング終了後も仮想ディスプレイが保持され、Windows のディスプレイ配置が変更される場合があります。続行しますか？",
    vdd_headless: "ヘッドレス時に自動作成",
    vdd_headless_confirm: "これは実験的な復旧機能です。ディスプレイが検出されない場合に仮想ディスプレイを作成し、現在の配置が変更される場合があります。続行しますか？",
    import_config: "設定をインポート",
    export_config: "設定をエクスポート",
    reset_config: "設定をリセット",
    reset_config_confirm: "sunshine.conf を消去し、Sunshine の既定設定に戻します。続行しますか？",
    clear_cache: "キャッシュを消去",
    clear_cache_confirm: "キャッシュ状態を再構築するため、実行中のアプリケーションを終了します。続行しますか？",
    reset_display: "ディスプレイをリセット",
    reset_display_confirm: "保存されているディスプレイデバイスの状態をリセットします。続行しますか？",
    restart_user_mode: "ユーザーモードで Sunshine を実行",
    show_toolbar: "デスクトップペットを表示",
    prevent_sleep: "システムのスリープを防止",
    rtss_control: "RTSS コントロール",
    host_performance: "ホストパフォーマンス",
    log_console: "ログコンソール",
    open_desktop: "大画面モード",
    #[cfg(target_os = "windows")]
    auto_start: "起動時に実行",
    #[cfg(any(debug_assertions, feature = "beta"))]
    web_stream: "Web ストリーミング",
    #[cfg(debug_assertions)]
    debug_page: "デバッグページ",
    check_update: "更新を確認",
    about: "Sunshine について",
    shutdown: "Sunshine を終了",
    shutdown_message: "Sunshine サービス、GUI、トレイを停止します。続行しますか？",
    language: "言語",
    star_project: "プロジェクトサイト",
    visit_project_sunshine: "Sunshine ソースコード",
    visit_project_moonlight: "Moonlight ソースコード",
    restart: "Sunshine を再起動",
    recover_service: "Sunshine サービスを再起動",
    recovery_timeout: "Sunshine サービスが時間内に復旧しませんでした。サービスログを確認して再試行してください。",
    tooltip: "Sunshine GUI",
    tooltip_admin: "Sunshine GUI (管理者)",
};

fn get_tray_strings() -> &'static TrayStrings {
    let locale = CURRENT_LOCALE.lock().unwrap();
    match locale.as_deref() {
        Some("zh") => &ZH_STRINGS,
        Some("ja") => &JA_STRINGS,
        _ => &EN_STRINGS,
    }
}

fn get_current_locale() -> String {
    CURRENT_LOCALE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "en".to_string())
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
pub fn create_system_tray<R: Runtime + 'static>(app: &AppHandle<R>) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    init_sunshine_user_mode_state(app);

    if let Err(error) = notifications::initialize(app) {
        warn!("{}", error);
    }

    initialize_tray_locale();
    build_owned_system_tray(app)?;
    events::start_tray_state_monitoring(app);

    Ok(())
}

fn try_normalize_tray_locale(locale: &str) -> Option<&'static str> {
    let locale = locale.trim().to_ascii_lowercase().replace('_', "-");
    if locale == "zh" || locale.starts_with("zh-") {
        Some("zh")
    } else if locale == "ja" || locale.starts_with("ja-") {
        Some("ja")
    } else if locale == "en" || locale.starts_with("en-") {
        Some("en")
    } else {
        None
    }
}

fn normalize_tray_locale(locale: &str) -> &'static str {
    try_normalize_tray_locale(locale).unwrap_or("en")
}

fn resolve_initial_tray_locale(
    config: Option<&sunshine::SunshineConfig>,
    system_locale: Option<&str>,
) -> String {
    config
        .and_then(|config| config.locale.as_deref())
        .filter(|locale| !locale.trim().is_empty())
        .map(normalize_tray_locale)
        .or_else(|| system_locale.and_then(try_normalize_tray_locale))
        .unwrap_or("en")
        .to_string()
}

#[cfg(target_os = "windows")]
fn locale_from_windows_ui_language(language_id: u16) -> Option<&'static str> {
    // Match the primary language portion of the Windows LANGID.
    match language_id & 0x03ff {
        0x0004 => Some("zh"),
        0x0011 => Some("ja"),
        0x0009 => Some("en"),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn get_system_locale() -> Option<String> {
    use ::windows::Win32::Globalization::GetUserDefaultUILanguage;

    locale_from_windows_ui_language(unsafe { GetUserDefaultUILanguage() }).map(str::to_string)
}

#[cfg(not(target_os = "windows"))]
fn get_system_locale() -> Option<String> {
    std::env::var("LC_ALL")
        .ok()
        .filter(|locale| !locale.is_empty())
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .filter(|locale| !locale.is_empty())
        })
}

fn initialize_tray_locale() {
    let config = match sunshine::parse_sunshine_config_sync() {
        Ok(config) => Some(config),
        Err(error) => {
            warn!("Failed to read persisted tray locale: {}", error);
            None
        }
    };
    let system_locale = get_system_locale();
    let locale = resolve_initial_tray_locale(config.as_ref(), system_locale.as_deref());
    info!("🌍 托盘菜单：启动语言为 {}", locale);
    *CURRENT_LOCALE.lock().unwrap() = Some(locale);
}

fn build_owned_system_tray<R: Runtime + 'static>(app: &AppHandle<R>) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }
    let menu = build_tray_menu(app)?;
    let tooltip = default_tray_tooltip();
    let tray_icon = icons::load_initial_tray_icon(app);

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(tray_icon)
        .tooltip(tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } => handle_tray_click(tray.app_handle()),
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => handle_tray_double_click(tray.app_handle()),
            _ => {}
        })
        .build(app)?;

    info!("GUI tray created for the current user session");

    Ok(())
}

fn open_desktop_gui_from_tray<R: Runtime>(app: &AppHandle<R>, source: &str) {
    if let Err(e) = windows::open_desktop_window(app) {
        error!("Failed to open desktop GUI from tray {}: {}", source, e);
    }
}

fn open_main_panel_from_tray<R: Runtime>(app: &AppHandle<R>, source: &str) {
    if let Some(window) = app.get_webview_window("main") {
        windows::show_and_activate_window(&window);
        return;
    }

    if let Err(e) = windows::create_main_window(app) {
        error!("Failed to open main panel from tray {}: {}", source, e);
    }
}

pub fn mark_main_panel_loading() {
    MAIN_PANEL_BRIDGE.mark_loading();
}

fn emit_to_main_when_ready<R: Runtime>(
    app: &AppHandle<R>,
    event: &str,
    payload: serde_json::Value,
) {
    MAIN_PANEL_BRIDGE.emit_or_queue(app, event, payload);
}

#[tauri::command]
pub fn main_panel_loading() {
    mark_main_panel_loading();
}

#[tauri::command]
pub fn main_panel_ready(app: AppHandle) {
    MAIN_PANEL_BRIDGE.mark_ready(&app);
}

fn default_tray_tooltip() -> &'static str {
    let s = get_tray_strings();
    if utils::is_running_as_admin().unwrap_or(false) {
        s.tooltip_admin
    } else {
        s.tooltip
    }
}

fn tray_tooltip_from_state(state: &sunshine::TrayState) -> String {
    let tooltip = state.tooltip.trim();
    if !(tooltip.is_empty() || state.status == "idle" && tooltip == "Sunshine") {
        return tooltip.to_string();
    }

    match state.status.as_str() {
        "idle" => "Sunshine - Idle".to_string(),
        "streaming" if !state.app_name.is_empty() => format!("Streaming {}", state.app_name),
        "streaming" => "Streaming".to_string(),
        "paused" if !state.app_name.is_empty() => format!("Stream paused: {}", state.app_name),
        "paused" => "Stream paused".to_string(),
        "pairing" if !state.pairing_client_name.is_empty() => {
            format!("Pairing request: {}", state.pairing_client_name)
        }
        "pairing" => "Pairing request".to_string(),
        "notification" if !state.notification.title.trim().is_empty() => {
            state.notification.title.trim().to_string()
        }
        "notification" if !state.notification.message.trim().is_empty() => {
            state.notification.message.trim().to_string()
        }
        "notification" => "Sunshine notification".to_string(),
        _ => default_tray_tooltip().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tray_state(status: &str) -> sunshine::TrayState {
        sunshine::TrayState {
            status: status.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn idle_tooltip_shows_explicit_state() {
        let mut state = tray_state("idle");
        state.tooltip = "Sunshine".to_string();

        assert_eq!(tray_tooltip_from_state(&state), "Sunshine - Idle");
    }

    #[test]
    fn explicit_tooltip_is_preserved() {
        let mut state = tray_state("idle");
        state.tooltip = "Sunshine - Ready".to_string();

        assert_eq!(tray_tooltip_from_state(&state), "Sunshine - Ready");
    }

    #[test]
    fn streaming_tooltip_includes_app_name() {
        let mut state = tray_state("streaming");
        state.app_name = "Steam Big Picture".to_string();

        assert_eq!(
            tray_tooltip_from_state(&state),
            "Streaming Steam Big Picture"
        );
    }

    #[test]
    fn paused_tooltip_includes_app_name() {
        let mut state = tray_state("paused");
        state.app_name = "Desktop".to_string();

        assert_eq!(tray_tooltip_from_state(&state), "Stream paused: Desktop");
    }

    #[test]
    fn pairing_tooltip_includes_client_name() {
        let mut state = tray_state("pairing");
        state.pairing_client_name = "Moonlight".to_string();

        assert_eq!(
            tray_tooltip_from_state(&state),
            "Pairing request: Moonlight"
        );
    }

    #[test]
    fn notification_tooltip_prefers_title_then_message() {
        let mut state = tray_state("notification");
        state.notification.title = "Update available".to_string();
        state.notification.message = "A new Sunshine build is ready".to_string();
        assert_eq!(tray_tooltip_from_state(&state), "Update available");

        state.notification.title.clear();
        assert_eq!(
            tray_tooltip_from_state(&state),
            "A new Sunshine build is ready"
        );
    }

    #[test]
    fn pairing_notification_label_names_the_client() {
        let mut state = tray_state("pairing");
        state.pairing_client_name = "Moonlight Client".to_string();
        state.notification.active = true;
        state.notification.action = "open_pin".to_string();

        assert_eq!(
            tray_notification_label(&ZH_STRINGS, &state),
            "完成配对: Moonlight Client"
        );
    }

    #[test]
    fn menu_status_includes_streamed_app() {
        let mut state = tray_state("streaming");
        state.app_name = "Desktop".to_string();

        assert_eq!(
            tray_status_label(&ZH_STRINGS, Some(&state), CoreConnectionState::Connected),
            "Sunshine · 串流中: Desktop"
        );
    }

    #[test]
    fn menu_status_uses_connecting_without_core_state() {
        assert_eq!(
            tray_status_label(&EN_STRINGS, None, CoreConnectionState::Connecting),
            "Sunshine · Connecting"
        );
    }

    #[test]
    fn menu_status_distinguishes_disconnected_core() {
        assert_eq!(
            tray_status_label(&EN_STRINGS, None, CoreConnectionState::Disconnected),
            "Sunshine · Disconnected"
        );
    }

    #[test]
    fn tray_locale_normalizes_supported_language_variants() {
        assert_eq!(normalize_tray_locale("zh-CN"), "zh");
        assert_eq!(normalize_tray_locale("ja-JP"), "ja");
        assert_eq!(normalize_tray_locale("en-US"), "en");
        assert_eq!(normalize_tray_locale("fr-FR"), "en");
    }

    #[test]
    fn persisted_ui_locale_wins_during_initialization() {
        let config = sunshine::SunshineConfig {
            locale: Some("ja".to_string()),
            ..Default::default()
        };

        assert_eq!(
            resolve_initial_tray_locale(Some(&config), Some("zh-CN")),
            "ja"
        );
    }

    #[test]
    fn system_locale_is_used_only_when_ui_locale_is_missing() {
        let config = sunshine::SunshineConfig {
            locale: Some("zh_TW".to_string()),
            ..Default::default()
        };
        assert_eq!(
            resolve_initial_tray_locale(Some(&config), Some("en-US")),
            "zh"
        );

        assert_eq!(resolve_initial_tray_locale(None, Some("ja-JP")), "ja");
        assert_eq!(resolve_initial_tray_locale(None, None), "en");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_ui_language_maps_supported_tray_languages() {
        assert_eq!(locale_from_windows_ui_language(0x0804), Some("zh"));
        assert_eq!(locale_from_windows_ui_language(0x0411), Some("ja"));
        assert_eq!(locale_from_windows_ui_language(0x0409), Some("en"));
        assert_eq!(locale_from_windows_ui_language(0x040c), None);
    }

    #[test]
    fn unsupported_persisted_ui_locale_maps_to_english() {
        let config = sunshine::SunshineConfig {
            locale: Some("fr".to_string()),
            ..Default::default()
        };

        assert_eq!(
            resolve_initial_tray_locale(Some(&config), Some("ja-JP")),
            "en"
        );
    }

    #[test]
    fn primary_desktop_actions_have_clear_localized_labels() {
        assert_eq!(ZH_STRINGS.open_desktop, "大屏模式");
        assert_eq!(ZH_STRINGS.show_toolbar, "显示桌宠");
        #[cfg(target_os = "windows")]
        assert_eq!(ZH_STRINGS.auto_start, "开机运行");
        assert_eq!(EN_STRINGS.open_desktop, "Large Screen Mode");
        #[cfg(target_os = "windows")]
        assert_eq!(EN_STRINGS.auto_start, "Run at Startup");
        assert_eq!(ZH_STRINGS.shutdown, "退出 Sunshine");
        assert_eq!(EN_STRINGS.shutdown, "Exit Sunshine");
    }

    #[test]
    fn native_confirmations_use_localized_tray_text() {
        assert!(ZH_STRINGS.vdd_create_confirm.contains("基地显示器"));
        assert!(ZH_STRINGS.vdd_keep_confirm.contains("串流结束"));
        assert!(ZH_STRINGS.vdd_headless_confirm.contains("实验性"));
        assert!(ZH_STRINGS.reset_config_confirm.contains("默认配置"));
        assert!(ZH_STRINGS.clear_cache_confirm.contains("缓存状态"));
        assert!(ZH_STRINGS.reset_display_confirm.contains("显示设备"));
    }

    #[test]
    fn long_dynamic_menu_text_is_compacted() {
        assert_eq!(compact_menu_text("1234567890", 8), "12345...");
        assert_eq!(compact_menu_text("  short  ", 8), "short");
    }

    #[test]
    fn recovery_state_rejects_duplicate_and_stale_completion() {
        let mut recovery = RecoveryState::default();

        let first = recovery.begin(7).expect("first recovery should start");
        assert!(recovery.is_in_progress());
        assert_eq!(recovery.begin(7), None);
        assert!(!recovery.finish(first + 1));
        assert!(recovery.finish(first));
        assert!(!recovery.is_in_progress());

        let second = recovery.begin(11).expect("second recovery should start");
        assert_ne!(second, first);
        assert!(!recovery.finish(first));
        assert!(!recovery.complete_from_monitored_state(11));
        assert!(recovery.complete_from_monitored_state(12));
        assert!(!recovery.complete_from_monitored_state(13));
    }

    #[test]
    fn runtime_state_applies_connection_transitions_atomically() {
        let mut runtime = TrayRuntimeState {
            connection: CoreConnectionState::Connecting,
            tray_state: None,
            recovery: RecoveryState::default(),
        };
        assert_eq!(runtime.begin_recovery_if_disconnected(3), None);

        assert!(runtime.mark_disconnected());
        assert!(!runtime.mark_disconnected());
        assert!(runtime.begin_recovery_if_disconnected(3).is_some());

        let state = tray_state("idle");
        assert!(runtime.apply_connected_state(&state, None).is_none());
        let (previous, should_rebuild) = runtime
            .apply_connected_state(&state, Some(4))
            .expect("fresh monitor state should be accepted");
        assert!(previous.is_none());
        assert!(should_rebuild);

        let (_, connection, recovery_in_progress) = runtime.menu_snapshot();
        assert_eq!(connection, CoreConnectionState::Connected);
        assert!(!recovery_in_progress);
        assert!(
            !runtime
                .apply_connected_state(&state, Some(5))
                .expect("connected monitor state should be accepted")
                .1
        );
    }
}

fn apply_tray_state<R: Runtime + 'static>(
    app: &AppHandle<R>,
    state: &sunshine::TrayState,
    monitored_state_receipt: Option<u64>,
) {
    let applied = {
        let mut runtime = TRAY_RUNTIME_STATE.lock().unwrap();
        runtime.apply_connected_state(state, monitored_state_receipt)
    };
    let Some((previous_state, should_rebuild_menu)) = applied else {
        debug!("Ignored non-monitor tray state while Core is unavailable");
        return;
    };

    if state.owner != "gui" {
        if app.remove_tray_by_id(TRAY_ID).is_some() {
            info!(
                "Removed GUI tray because core tray owner is '{}'",
                state.owner
            );
            *CURRENT_CORE_ICON.lock().unwrap() = None;
        }
        return;
    }

    notifications::show_connection_change_if_any(app, previous_state.as_ref(), state);

    if let Err(e) = build_owned_system_tray(app) {
        error!("Failed to create GUI tray for core owner: {}", e);
        return;
    }

    notifications::show_core_notification_if_new(app, state);

    if should_rebuild_menu {
        rebuild_tray_menu(app);
    }

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let tooltip = tray_tooltip_from_state(state);
        if let Err(e) = tray.set_tooltip(Some(tooltip.as_str())) {
            debug!("Failed to update tray tooltip from core state: {}", e);
        }
    }
    icons::apply_tray_icon(app, state.icon.as_str());
}

fn apply_core_disconnected<R: Runtime + 'static>(app: &AppHandle<R>) {
    let should_rebuild_menu = TRAY_RUNTIME_STATE.lock().unwrap().mark_disconnected();

    if let Err(e) = build_owned_system_tray(app) {
        error!(
            "Failed to keep GUI tray available while Core is unavailable: {}",
            e
        );
        return;
    }
    if should_rebuild_menu {
        rebuild_tray_menu(app);
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let tooltip =
            tray_status_label(get_tray_strings(), None, CoreConnectionState::Disconnected);
        let _ = tray.set_tooltip(Some(tooltip.as_str()));
    }
    icons::apply_disconnected_tray_icon(app);
}

fn apply_tray_state_on_main_thread<R: Runtime + 'static>(
    app: &AppHandle<R>,
    state: sunshine::TrayState,
) {
    let app_handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        apply_tray_state(&app_handle, &state, None);
    }) {
        debug!("Failed to schedule tray state update on main thread: {}", e);
    }
}

fn apply_monitored_tray_state_on_main_thread<R: Runtime + 'static>(
    app: &AppHandle<R>,
    state: sunshine::TrayState,
) {
    let receipt = MONITORED_STATE_RECEIPT.fetch_add(1, Ordering::AcqRel) + 1;
    let app_handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        apply_tray_state(&app_handle, &state, Some(receipt));
    }) {
        debug!(
            "Failed to schedule monitored tray state update on main thread: {}",
            e
        );
    }
}

/// 初始化 Sunshine 用户模式状态（仅 Windows）
#[cfg(target_os = "windows")]
fn init_sunshine_user_mode_state<R: Runtime + 'static>(app: &AppHandle<R>) {
    // 使用默认值 false，避免阻塞启动
    *SUNSHINE_USER_MODE_STATE.lock().unwrap() = false;

    // 异步更新 Sunshine 用户模式状态（不阻塞启动；阻塞的 sc/tasklist 放在 spawn_blocking 中）
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        match tokio::task::spawn_blocking(crate::sunshine::is_sunshine_running_in_user_mode_impl)
            .await
        {
            Ok(Ok(is_user_mode)) => {
                let changed = {
                    let mut state = SUNSHINE_USER_MODE_STATE.lock().unwrap();
                    let changed = *state != is_user_mode;
                    *state = is_user_mode;
                    changed
                };
                debug!("✅ Sunshine 用户模式状态已异步更新: {}", is_user_mode);
                if changed {
                    let rebuild_handle = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        rebuild_tray_menu(&rebuild_handle);
                    });
                }
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

/// 处理托盘单击事件
pub fn handle_tray_click<R: Runtime>(app: &AppHandle<R>) {
    open_main_panel_from_tray(app, "click");
}
/// 处理托盘双击事件
pub fn handle_tray_double_click<R: Runtime>(app: &AppHandle<R>) {
    open_main_panel_from_tray(app, "double click");
}

fn apply_next_tray_locale<R: Runtime>(app: &AppHandle<R>, locale: &str) -> (u64, String) {
    let locale = normalize_tray_locale(locale).to_string();
    let (revision, changed) = {
        let mut current_locale = CURRENT_LOCALE.lock().unwrap();
        let revision = next_locale_change_revision();
        let changed = current_locale.as_deref() != Some(locale.as_str());
        *current_locale = Some(locale.clone());
        (revision, changed)
    };

    if changed {
        rebuild_tray_menu(app);
    }

    (revision, locale)
}

/// 从托盘菜单切换语言
fn switch_tray_locale<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    info!("🌍 托盘菜单：切换语言为 {}", locale);
    let (revision, locale) = apply_next_tray_locale(app, locale);
    let locale_to_persist = locale.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = persist_latest_locale(revision, locale_to_persist.clone()).await {
            warn!(
                "Failed to persist Sunshine locale '{}': {}",
                locale_to_persist, error
            );
        }
    });
    // 所有已打开的 webview 都维护独立的 Vue 状态，需要一起同步。
    let payload = TrayLocaleChangedPayload {
        locale,
        source: "tray",
        request_id: None,
        revision,
    };
    if let Err(error) = emit_tray_locale_changed(app, &payload) {
        warn!("{error}; retrying once");
        if let Err(retry_error) = emit_tray_locale_changed(app, &payload) {
            warn!("{retry_error}");
        }
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
                let tooltip = {
                    let runtime = TRAY_RUNTIME_STATE.lock().unwrap();
                    if runtime.connection == CoreConnectionState::Connected {
                        runtime
                            .tray_state
                            .as_ref()
                            .map(tray_tooltip_from_state)
                            .unwrap_or_else(|| default_tray_tooltip().to_string())
                    } else {
                        tray_status_label(get_tray_strings(), None, runtime.connection)
                    }
                };
                let _ = tray.set_tooltip(Some(tooltip));
            }
            Err(e) => error!("❌ 构建托盘菜单失败: {}", e),
        }
    }
}

/// Tauri 命令：前端通知 tray 同步语言
pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>) {
    rebuild_tray_menu(app);
}

#[tauri::command]
pub async fn set_locale_preferences(
    app: AppHandle,
    locale: String,
    request_id: Option<String>,
) -> Result<(), String> {
    info!("🌍 前端同步 UI 与托盘语言: {}", locale);
    let (revision, locale) = apply_next_tray_locale(&app, &locale);
    if !persist_latest_locale(revision, locale.clone()).await? {
        return Ok(());
    }
    if revision == LOCALE_CHANGE_REVISION.load(Ordering::Acquire) {
        emit_tray_locale_changed(
            &app,
            &TrayLocaleChangedPayload {
                locale,
                source: "frontend",
                request_id,
                revision,
            },
        )?;
    }
    Ok(())
}

/// Tauri 命令：前端获取当前 tray 语言
#[tauri::command]
pub fn get_tray_locale() -> Option<String> {
    CURRENT_LOCALE.lock().unwrap().clone()
}
