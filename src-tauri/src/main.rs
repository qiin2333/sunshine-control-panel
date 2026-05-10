// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod vdd;
#[cfg(target_os = "windows")]
mod vdd_ioctl;
mod bat_runner;
mod vigem;
mod vmouse;
mod rtss;
mod hwinfo;
mod system;
mod sunshine;
mod utils;
mod proxy_server;
mod fs_utils;
mod toolbar;
mod update;
mod logger;
mod tray;
mod windows;
mod app;
mod commands;
mod moonlight_web;
mod controllermeta;
mod clipboard;

use log::info;

fn main() {
    // 设置 WebView2 浏览器参数以优化 GPU 占用和安全策略
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", [
            // 安全：忽略自签名证书错误（连接本地 Sunshine）
            "--ignore-certificate-errors",
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
    }
    
    tauri::Builder::default()
        .manage(app::AppState {
            main_window: std::sync::Mutex::new(None),
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            app::handle_single_instance(app, args);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 初始化日志系统（需要在 setup 中获取 app handle）
            logger::init_logger(app.handle().clone());
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
            system::get_current_dpi,
            system::set_desktop_dpi,
            commands::open_tool_window,
            commands::launch_app,
            toolbar::create_toolbar_window,
            commands::fetch_speech_phrases,
            commands::fetch_remote_bytes,
            commands::ai_api_proxy,
            commands::capture_screenshot,
            vdd::get_vdd_settings_file_path,
            vdd::get_vdd_tools_dir_path,
            vdd::get_vdd_edid_file_path,
            vdd::load_vdd_settings,
            vdd::save_vdd_settings,
            vdd::exec_pipe_cmd,
            vdd::upload_edid_file,
            vdd::read_edid_file,
            vdd::delete_edid_file,
            system::get_gpus,
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
            utils::open_external_url,
            utils::restart_graphics_driver,
            utils::restart_as_admin,
            utils::is_running_as_admin,
            vdd::uninstall_vdd_driver,
            vmouse::get_vmouse_status,
            vmouse::install_vmouse_driver,
            vmouse::uninstall_vmouse_driver,
            vmouse::set_vmouse_config,
            vigem::get_vigem_status,
            vigem::install_vigem_driver,
            vigem::uninstall_vigem_driver,
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
            windows::_webview_heartbeat,
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
            tray::set_tray_locale,
            tray::get_tray_locale,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
