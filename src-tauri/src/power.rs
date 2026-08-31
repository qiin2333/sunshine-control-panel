//! Windows 效率模式（EcoQoS）状态开关。
//!
//! 仅在「完全托盘常驻」时启用：所有 WebView 窗口已关闭、无游戏会话、
//! Sunshine 核心未处于推流/暂停状态。任何窗口打开或会话活跃都会立即退出
//! 效率模式。WebView2 子进程会继承宿主进程的节流状态，因此只要还有一个
//! 可见面板/工具窗，就不能节流宿主进程。

use log::info;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Runtime};

/// 初值 true：`--hidden` 代理模式启动即处于 EcoQoS ON，无需打日志。
static ECOQOS_ENABLED: AtomicBool = AtomicBool::new(true);

/// 按当前应用状态重新评估并应用 EcoQoS。幂等、线程无关、可任意频率调用。
pub fn refresh_ecoqos_state<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "windows")]
    {
        let enable = app.webview_windows().is_empty()
            && !crate::game_session::is_game_session_active()
            && !crate::tray::is_tray_session_active();

        if ECOQOS_ENABLED.swap(enable, Ordering::Relaxed) != enable {
            info!(
                "⚡ 效率模式（EcoQoS）→ {}",
                if enable {
                    "ON（托盘常驻，面板已全部关闭）"
                } else {
                    "OFF（面板或会话活动）"
                }
            );
        }
        apply_ecoqos(enable);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "windows")]
fn apply_ecoqos(enable: bool) {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
        ProcessPowerThrottling, SetProcessInformation,
    };

    let state = PROCESS_POWER_THROTTLING_STATE {
        // Version 必须为 CURRENT_VERSION(1),传 0 会让 SetProcessInformation 静默失败。
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: if enable {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
    };

    // GetCurrentProcess 返回伪句柄，绝不能 CloseHandle。
    let result = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &state as *const PROCESS_POWER_THROTTLING_STATE as *const core::ffi::c_void,
            size_of_val(&state) as u32,
        )
    };
    if let Err(e) = result {
        log::warn!(
            "⚠️ SetProcessInformation(ProcessPowerThrottling) 失败: {}",
            e
        );
    }
}
