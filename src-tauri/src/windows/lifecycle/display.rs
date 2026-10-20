//! Native display events feed the same Panel state as window visibility.
use super::*;

struct Hook<R: Runtime> {
    window: WebviewWindow<R>,
    panel: Panel,
}

pub(super) fn install<R: Runtime>(window: &WebviewWindow<R>, panel: &Panel) {
    let (window, panel) = (window.clone(), panel.clone());
    let dispatch = window.clone();
    let _ = dispatch.run_on_main_thread(move || {
        use windows::Win32::UI::Shell::SetWindowSubclass;
        if panel.state.lock().unwrap().phase == Phase::Closed {
            return;
        }
        let Ok(hwnd) = window.hwnd() else { return };
        let context = Box::into_raw(Box::new(Hook { window, panel }));
        if !unsafe {
            SetWindowSubclass(
                windows::Win32::Foundation::HWND(hwnd.0),
                Some(callback::<R>),
                ID,
                context as usize,
            )
        }
        .as_bool()
        {
            unsafe {
                drop(Box::from_raw(context));
            }
            log::warn!("Failed to install WebView display subclass");
        }
    });
}

const ID: usize = 0x53535756;

unsafe extern "system" fn callback<R: Runtime>(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::{WM_DISPLAYCHANGE, WM_DPICHANGED, WM_NCDESTROY, WM_SHOWWINDOW},
    };
    let context = unsafe { &*(data as *const Hook<R>) };
    if message == WM_NCDESTROY {
        context.panel.close();
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(callback::<R>), ID);
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            drop(Box::from_raw(data as *mut Hook<R>));
            return result;
        }
    }
    if matches!(message, WM_DISPLAYCHANGE | WM_DPICHANGED) {
        let revision = context
            .panel
            .state
            .lock()
            .unwrap()
            .display_changed(Instant::now());
        if let Some(revision) = revision {
            let (window, panel) = (context.window.clone(), context.panel.clone());
            tauri::async_runtime::spawn(async move {
                // Coalesce display bursts; hidden panels retain the pending
                // refresh until they become visible, without waking up here.
                tokio::time::sleep(DISPLAY_DEBOUNCE).await;
                {
                    let state = panel.state.lock().unwrap();
                    if state.phase == Phase::Closed || state.display_revision != revision {
                        return;
                    }
                }
                reconcile(&window, &panel);
            });
        }
    } else if message == WM_SHOWWINDOW {
        // The HWND's visibility is updated after this notification returns.
        let (window, panel) = (context.window.clone(), context.panel.clone());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            reconcile(&window, &panel);
        });
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn refresh<R: Runtime>(window: &WebviewWindow<R>, panel: &Panel) {
    let revision = panel.state.lock().unwrap().pending_refresh(Instant::now());
    let Some(revision) = revision else { return };
    let panel = panel.clone();
    let _ = window.with_webview(move |webview| {
        {
            let state = panel.state.lock().unwrap();
            if state.pending_refresh(Instant::now()) != Some(revision) {
                return;
            }
        }
        let controller = webview.controller();
        let result = unsafe {
            let mut bounds = Default::default();
            controller
                .Bounds(&mut bounds)
                .and_then(|()| controller.SetBounds(bounds))
                .and_then(|()| controller.NotifyParentWindowPositionChanged())
        };
        panel
            .state
            .lock()
            .unwrap()
            .refreshed(revision, result.is_ok());
        if let Err(error) = result {
            log::warn!("Failed to refresh WebView after display change: {error}");
        }
    });
}
