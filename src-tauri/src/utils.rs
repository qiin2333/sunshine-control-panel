use crate::sunshine;
use log::{debug, error, info};
use std::env;
use std::process::Command;
use tauri::Manager;

#[tauri::command]
pub async fn restart_graphics_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        // Resolve Sunshine install path from the registry-backed helper.
        let sunshine_path = std::path::PathBuf::from(sunshine::get_sunshine_install_path());
        let restart_exe = sunshine_path.join("tools").join("restart64.exe");

        if !restart_exe.exists() {
            return Err("restart64.exe was not found".to_string());
        }

        let ps_command = format!(
            r#"Start-Process '{}' -Verb RunAs -WindowStyle Hidden"#,
            restart_exe.display()
        );

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("powershell")
            .args(&["-Command", &ps_command])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;

        Ok("Requested graphics driver restart".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("This feature is only supported on Windows".to_string())
    }
}

/// Restart the GUI with administrator privileges.
#[tauri::command]
pub async fn restart_as_admin(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        let current_exe = env::current_exe()
            .map_err(|e| format!("Failed to resolve current executable: {}", e))?;

        info!("Preparing to restart GUI as administrator");
        debug!("Current executable: {:?}", current_exe);

        let exe_path = current_exe.to_string_lossy().to_string();
        let ps_command = format!(
            "Start-Sleep -Milliseconds 500; Start-Process -FilePath '{}' -Verb RunAs",
            exe_path.replace("'", "''")
        );

        debug!("PowerShell command: {}", ps_command);

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_command])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to start elevated instance: {}", e))?;

        info!("Requested elevated GUI restart after 500ms");

        tokio::spawn(async move {
            info!("Preparing to exit current GUI instance");

            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.close();
                debug!("Closed main window");
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            info!("Exiting current GUI instance");
            app_handle.exit(0);
        });

        Ok("Restarting with administrator privileges".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("This feature is only supported on Windows".to_string())
    }
}

/// Check whether the current process is running as administrator.
#[tauri::command]
pub fn is_running_as_admin() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{
            GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token: HANDLE = HANDLE::default();
            let process = GetCurrentProcess();

            if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
                return Ok(false);
            }

            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut return_length = 0u32;

            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_length,
            );

            CloseHandle(token).ok();

            if result.is_err() {
                return Ok(false);
            }

            Ok(elevation.TokenIsElevated != 0)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(unsafe { libc::geteuid() == 0 })
    }
}

/// Open a URL in the external browser.
pub fn open_url_in_browser(url: &str) {
    let url = url.to_string();

    tauri::async_runtime::spawn(async move {
        info!("Opening external browser");

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            if let Err(e) = Command::new("cmd")
                .args(&["/c", "start", "", &url])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
            {
                error!("Failed to open URL: {}", e);
            } else {
                info!("Opened in external browser: {}", url);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Err(e) = Command::new("xdg-open").arg(&url).spawn() {
                error!("Failed to open URL: {}", e);
            } else {
                info!("Opened in external browser: {}", url);
            }
        }
    });
}

/// Open a web or Windows Store URL. Local filesystem paths use a separate,
/// local-window-only command so remote WebUI content cannot open host files.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<bool, String> {
    let target = url.trim().trim_matches('"').to_string();
    if target.is_empty() {
        return Ok(false);
    }

    // Keep protocol launching in the native layer. Tauri's generic shell scope
    // does not allow Windows Store links by default, while ShellExecuteW does.
    let normalized_target = target.to_ascii_lowercase();
    let is_shell_uri = normalized_target.starts_with("http://")
        || normalized_target.starts_with("https://")
        || normalized_target.starts_with("ms-windows-store://");
    if !is_shell_uri {
        return Err("Only HTTP(S) and Microsoft Store URLs are allowed".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use ::windows::Win32::Foundation::HWND;
        use ::windows::Win32::UI::Shell::ShellExecuteW;
        use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use ::windows::core::PCWSTR;

        fn to_wide(value: &str) -> Vec<u16> {
            value.encode_utf16().chain(std::iter::once(0)).collect()
        }

        let operation = to_wide("open");
        let target_wide = to_wide(&target);
        let result = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(operation.as_ptr()),
                PCWSTR(target_wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = result.0 as isize;
        if code <= 32 {
            return Err(format!("Failed to open URI: ShellExecuteW code {}", code));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(true)
}

/// Open a configured application's existing local working directory.
#[tauri::command]
pub fn open_local_path(path: String, app_name: String) -> Result<bool, String> {
    let raw_path = path.trim().trim_matches('"');
    if app_name.trim().is_empty() {
        return Err("Application identity is required".to_string());
    }
    #[cfg(target_os = "windows")]
    if raw_path.starts_with(r"\\") || raw_path.starts_with(r"\\?\") || raw_path.starts_with(r"\\.\")
    {
        return Err("Network and device paths are not allowed".to_string());
    }
    let path = std::path::PathBuf::from(raw_path)
        .canonicalize()
        .map_err(|error| format!("Unable to resolve directory: {error}"))?;
    if !path.is_dir() {
        return Err(format!("Directory does not exist: {}", path.display()));
    }

    let apps_path = sunshine::config_dir().join("apps.json");
    let apps: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&apps_path)
            .map_err(|error| format!("Unable to read configured applications: {error}"))?,
    )
    .map_err(|error| format!("Unable to parse configured applications: {error}"))?;
    let allowed = apps
        .get("apps")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                let matches_name = entry.get("name").and_then(serde_json::Value::as_str)
                    == Some(app_name.as_str());
                let configured = entry
                    .get("working-dir")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .map(|value| value.trim_matches('"'))
                    .and_then(|value| std::path::PathBuf::from(value).canonicalize().ok());
                matches_name && configured.as_ref() == Some(&path)
            })
        });
    if !allowed {
        return Err(
            "Directory is not a configured working directory for this application".to_string(),
        );
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("explorer")
            .arg(&path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(true)
}

/// Execute an elevated PowerShell command.
#[cfg(target_os = "windows")]
pub fn execute_powershell_command(command: &str, error_context: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let ps_command = format!(
        "Start-Process powershell -ArgumentList '-NoProfile', '-Command', '{}' -Verb RunAs -WindowStyle Hidden",
        command.replace("'", "''")
    );

    Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| {
            error!("{}: {}", error_context, e);
            format!("{}: {}", error_context, e)
        })?;

    Ok(())
}
