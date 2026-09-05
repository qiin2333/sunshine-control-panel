// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bat_runner;
mod client_fingerprint_rules;
mod clipboard;
mod commands;
mod controller_hub;
mod controllermeta;
mod desktop_settings;
mod dualsense;
#[cfg(target_os = "windows")]
mod elevation;
mod file_mapping;
mod file_transfer;
mod fs_utils;
mod game_session;
mod github_download;
mod hwinfo;
mod logger;
mod moonlight_web;
mod native_tools;
mod power;
mod proxy_server;
mod rtss;
#[cfg(target_os = "windows")]
mod shell_context_menu;
mod sunshine;
mod system;
#[cfg(target_os = "windows")]
mod text_context;
mod toolbar;
mod tray;
mod tray_config;
mod update;
mod usbip;
mod utils;
mod vdd;
mod vdd_calibration;
#[cfg(target_os = "windows")]
mod vdd_ioctl;
mod vigem;
mod vmouse;
mod windows;

use log::info;
use std::sync::OnceLock;

static ASYNC_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn configure_async_runtime() {
    let worker_threads = std::thread::available_parallelism()
        .map(|count| count.get().min(4))
        .unwrap_or(2);
    let runtime = ASYNC_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .thread_name("sunshine-gui-worker")
            .enable_all()
            .build()
            .expect("failed to create the Sunshine GUI async runtime")
    });
    tauri::async_runtime::set(runtime.handle().clone());
}

#[cfg(target_os = "windows")]
fn configure_loopback_proxy_bypass() {
    // This runs before any helper path or async worker can create an HTTP
    // client, so every GUI-owned loopback request gets the same direct route.
    unsafe {
        // WebView2 uses Chromium's bypass syntax. Keep external requests on the
        // user's configured proxy while forcing every loopback form to direct.
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", [
            // 安全：忽略自签名证书错误（连接本地 Sunshine）
            "--ignore-certificate-errors",
            "--proxy-bypass-list=localhost;*.localhost;127.0.0.0/8;[::1]",
            // UI 音效：手柄输入不属于「用户手势」，不解锁 AudioContext；
            // 应用自身的导航反馈音不是网页自动播放媒体
            "--autoplay-policy=no-user-gesture-required",
            // 节流：激进的后台/隐藏标签页定时器节流
            "--enable-features=IntensiveWakeUpThrottling,ThrottleDisplayNoneAndVisibilityHiddenCrossOriginIframes",
            // GPU 优化：禁用 Edge 特有的 UI 覆盖层（减少不必要的 GPU 合成层）
            "--disable-features=msWebOOUI",
            // GPU 优化：禁用 GPU 着色器磁盘缓存，减少 VRAM 占用
            "--disable-gpu-shader-disk-cache",
            // GPU 优化：关闭 GPU 光栅化抗锯齿（控制面板 UI 无需 MSAA）
            "--gpu-rasterization-msaa-sample-count=0",
            // GPU 优化：限制渲染进程数量，减少 GPU 上下文切换开销
            "--renderer-process-limit=1",
        ].join(" "));

        // reqwest follows NO_PROXY/no_proxy. Preserve the user's existing
        // exclusions and add the same loopback scope used by WebView2.
        for key in ["NO_PROXY", "no_proxy"] {
            let mut value = std::env::var(key).unwrap_or_default();
            for entry in ["localhost", ".localhost", "127.0.0.0/8", "::1"] {
                if !value.split(',').any(|current| current.trim() == entry) {
                    if !value.is_empty() {
                        value.push(',');
                    }
                    value.push_str(entry);
                }
            }
            std::env::set_var(key, value);
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    configure_loopback_proxy_bypass();

    #[cfg(target_os = "windows")]
    utils::wait_for_elevated_restart_handoff();

    #[cfg(target_os = "windows")]
    if let Some(exit_code) = dualsense::try_handle_elevated_command() {
        std::process::exit(exit_code);
    }

    #[cfg(target_os = "windows")]
    if let Some(exit_code) = usbip::try_handle_elevated_command() {
        std::process::exit(exit_code);
    }

    #[cfg(target_os = "windows")]
    if let Some(exit_code) = vdd::try_handle_elevated_ioctl_command() {
        std::process::exit(exit_code);
    }

    if desktop_settings::try_remove_auto_start_from_args() {
        return;
    }
    if update::try_run_updater_helper_from_args() {
        return;
    }
    if sunshine::try_run_core_compatibility_check_from_args() {
        return;
    }

    // The agent mostly waits on local I/O. Bounding the shared runtime avoids
    // allocating one worker per logical CPU on high-core-count hosts. Proxy
    // environment variables must be finalized before worker threads start.
    configure_async_runtime();

    let builder = tauri::Builder::default()
        .manage(app::AppState {
            main_window: std::sync::Mutex::new(None),
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());

    let builder = if std::env::var_os("SUNSHINE_GUI_ALLOW_PARALLEL_INSTANCE").is_some() {
        builder
    } else {
        builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            app::handle_single_instance(app, args);
        }))
    };

    let application = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 初始化日志系统（需要在 setup 中获取 app handle）
            logger::init_logger(app.handle().clone());
            client_fingerprint_rules::start();
            info!("🚀 Sunshine Control Panel 启动中...");

            app::setup_application(app)
        })
        .on_window_event(|window, event| {
            windows::handle_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::toggle_dark_mode,
            toolbar::handle_toolbar_menu_action,
            toolbar::save_toolbar_position,
            toolbar::is_primary_mouse_button_pressed,
            system::get_current_dpi,
            system::set_desktop_dpi,
            native_tools::open_native_tool,
            commands::open_tool_window,
            game_session::launch_game,
            game_session::get_running_game,
            game_session::stop_running_game,
            game_session::get_game_stats,
            game_session::focus_running_game,
            toolbar::create_toolbar_window,
            commands::fetch_speech_phrases,
            commands::fetch_remote_bytes,
            commands::ai_api_proxy,
            commands::capture_screenshot,
            desktop_settings::get_desktop_settings,
            desktop_settings::save_desktop_settings,
            desktop_settings::load_toolbar_shortcut_status,
            desktop_settings::set_toolbar_shortcut_enabled,
            vdd::get_vdd_status,
            vdd::install_vdd_driver,
            vdd::set_vdd_keep_enabled,
            vdd::set_vdd_headless_create_enabled,
            vdd::get_vdd_tools_dir_path,
            vdd::get_vdd_edid_file_path,
            vdd::load_vdd_settings,
            vdd::save_vdd_settings,
            vdd::exec_vdd_cmd,
            vdd::upload_edid_file,
            vdd::read_edid_file,
            vdd::delete_edid_file,
            vdd::get_vdd_trace_status,
            vdd::start_vdd_trace,
            vdd::stop_vdd_trace,
            vdd::open_vdd_trace_folder,
            vdd_calibration::launch_windows_hdr_calibration,
            system::get_gpus,
            system::get_monitors,
            system::get_system_info,
            system::get_process_memory_info,
            system::get_sunshine_start_time,
            sunshine::get_sunshine_install_path,
            sunshine::get_sunshine_version,
            sunshine::parse_sunshine_config,
            sunshine::get_sunshine_url,
            sunshine::get_command_line_url,
            sunshine::get_sunshine_locale,
            sunshine::set_sunshine_locale,
            sunshine::get_active_sessions,
            sunshine::change_bitrate,
            sunshine::toggle_sunshine_mode,
            sunshine::is_sunshine_running_in_user_mode,
            sunshine::restart_sunshine_in_user_mode,
            sunshine::restart_sunshine_service,
            proxy_server::get_proxy_url_command,
            proxy_server::get_proxy_health_check,
            proxy_server::refresh_sunshine_target,
            proxy_server::wait_for_proxy_ready,
            utils::open_external_url,
            utils::open_local_path,
            utils::restart_graphics_driver,
            utils::restart_as_admin,
            utils::is_running_as_admin,
            vdd::uninstall_vdd_driver,
            vmouse::get_vmouse_status,
            vmouse::install_vmouse_driver,
            vmouse::uninstall_vmouse_driver,
            vmouse::set_vmouse_config,
            controller_hub::get_controller_hub_config,
            controller_hub::save_controller_hub_config,
            controller_hub::get_virtual_microphone_status,
            controller_hub::test_virtual_microphone,
            vigem::get_vigem_status,
            vigem::install_vigem_driver,
            vigem::uninstall_vigem_driver,
            dualsense::dualsense_get_status,
            dualsense::dualsense_log_panel_opened,
            dualsense::dualsense_install,
            dualsense::dualsense_uninstall,
            dualsense::dualsense_set_config,
            dualsense::dualsense_set_haptics_tuning,
            dualsense::dualsense_self_test,
            usbip::usbip_get_status,
            usbip::usbip_install_transport,
            usbip::usbip_cleanup_transport,
            usbip::usbip_list_remote,
            usbip::usbip_attach,
            usbip::usbip_detach,
            fs_utils::get_icc_file_list,
            fs_utils::read_directory,
            fs_utils::read_image_as_data_url,
            fs_utils::copy_image_to_assets,
            fs_utils::cleanup_unused_covers,
            fs_utils::resolve_lnk_target,
            fs_utils::scan_directory_for_apps,
            fs_utils::scan_game_libraries,
            fs_utils::search_steam_covers,
            fs_utils::upload_steam_cover,
            fs_utils::save_text_file,
            update::check_for_updates,
            update::check_for_updates_for_channel,
            update::start_update_checker_when_ui_ready,
            update::get_include_prerelease_preference,
            update::set_include_prerelease_preference,
            update::download_update,
            update::install_update,
            logger::get_all_logs,
            logger::clear_logs,
            logger::export_logs,
            moonlight_web::moonlight_web_get_status,
            moonlight_web::moonlight_web_start,
            moonlight_web::moonlight_web_stop,
            moonlight_web::moonlight_web_get_config,
            moonlight_web::moonlight_web_save_config,
            moonlight_web::moonlight_web_check_release,
            moonlight_web::moonlight_web_download,
            moonlight_web::moonlight_web_get_install_path,
            moonlight_web::moonlight_web_generate_cert,
            controllermeta::controllermeta_get_status,
            controllermeta::controllermeta_check_release,
            controllermeta::controllermeta_download,
            controllermeta::controllermeta_launch,
            controllermeta::controllermeta_get_install_path,
            controllermeta::controllermeta_uninstall,
            clipboard::clipboard_sync_status,
            file_mapping::quick_share_folder,
            file_mapping::list_file_mappings,
            file_mapping::delete_file_mapping,
            file_mapping::update_file_mapping,
            file_mapping::install_file_mapping_menu,
            file_mapping::uninstall_file_mapping_menu,
            file_transfer::send_file_to_client,
            windows::webview_heartbeat,
            windows::resize_about_window,
            windows::resize_tool_window,
            rtss::get_rtss_status,
            rtss::rtss_set_osd,
            rtss::rtss_clear_osd,
            rtss::rtss_set_framerate_limit,
            rtss::rtss_get_framerate_limit,
            rtss::rtss_toggle_limiter,
            rtss::rtss_get_limiter_status,
            rtss::rtss_toggle_overlay,
            rtss::rtss_download_cli,
            rtss::rtss_get_available_metrics,
            rtss::rtss_start_monitoring,
            rtss::rtss_stop_monitoring,
            rtss::rtss_get_monitoring_status,
            rtss::rtss_get_osd_properties,
            rtss::rtss_set_osd_property,
            hwinfo::hwinfo_get_sensors,
            hwinfo::hwinfo_get_readings,
            hwinfo::hwinfo_check_available,
            tray::set_locale_preferences,
            tray::get_tray_locale,
            tray::main_panel_loading,
            tray::main_panel_ready,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    application.run(|_app, event| match event {
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        tauri::RunEvent::Exit => {
            native_tools::shutdown_all();
            app::shutdown_application();
        }
        _ => {}
    });
}
