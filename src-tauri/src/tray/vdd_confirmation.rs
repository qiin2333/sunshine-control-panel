use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

const CONFIRMATION_TIMEOUT_MS: usize = 20_000;
static VISIBLE_CONFIRMATION_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn show<R: Runtime>(app: &AppHandle<R>, operation_id: u64) {
    if VISIBLE_CONFIRMATION_ID.swap(operation_id, Ordering::AcqRel) == operation_id {
        return;
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let keep = tauri::async_runtime::spawn_blocking(show_timed_confirmation)
            .await
            .unwrap_or(false);

        match sunshine::confirm_vdd_keep(operation_id, keep).await {
            Ok(response) => {
                if let Some(state) = response.tray_state {
                    apply_tray_state_on_main_thread(&app_handle, state);
                }
            }
            Err(e) => debug!("VDD keep confirmation {} ended: {}", operation_id, e),
        }

        let _ = VISIBLE_CONFIRMATION_ID.compare_exchange(
            operation_id,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    });
}

fn show_timed_confirmation() -> bool {
    use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use ::windows::Win32::UI::Controls::{
        TASKDIALOG_NOTIFICATIONS, TASKDIALOGCONFIG, TDCBF_NO_BUTTON, TDCBF_YES_BUTTON,
        TDF_CALLBACK_TIMER, TDM_CLICK_BUTTON, TDN_TIMER, TaskDialogIndirect,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{IDNO, IDYES, SendMessageW};
    use ::windows::core::{HRESULT, w};

    unsafe extern "system" fn callback(
        hwnd: HWND,
        notification: TASKDIALOG_NOTIFICATIONS,
        elapsed_ms: WPARAM,
        _lparam: LPARAM,
        _callback_data: isize,
    ) -> HRESULT {
        if notification == TDN_TIMER && elapsed_ms.0 >= CONFIRMATION_TIMEOUT_MS {
            unsafe {
                SendMessageW(
                    hwnd,
                    TDM_CLICK_BUTTON.0 as u32,
                    Some(WPARAM(IDNO.0 as usize)),
                    Some(LPARAM(0)),
                );
            }
        }
        HRESULT(0)
    }

    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_CALLBACK_TIMER,
        dwCommonButtons: TDCBF_YES_BUTTON | TDCBF_NO_BUTTON,
        pszWindowTitle: w!("Keep Virtual Display?"),
        pszMainInstruction: w!("Can you still see this screen?"),
        pszContent: w!(
            "Choose Yes to keep the virtual display. It will be closed automatically after 20 seconds if you do not confirm."
        ),
        nDefaultButton: IDYES.0,
        pfCallback: Some(callback),
        ..Default::default()
    };
    let mut selected_button = IDNO.0;
    unsafe {
        TaskDialogIndirect(&config, Some(&mut selected_button), None, None).is_ok()
            && selected_button == IDYES.0
    }
}
