//! Windows service control without shell scripts or localized command output.

use std::time::{Duration, Instant};
use windows::Win32::Foundation::{ERROR_NO_MORE_FILES, ERROR_SERVICE_DOES_NOT_EXIST};
use windows::Win32::System::Services::*;
use windows::core::{Owned, PCWSTR, w};

const HELPER_ARG: &str = "--sunshine-service-helper";
const TIMEOUT: Duration = Duration::from_secs(30);

fn open_service(access: u32) -> Result<Option<Owned<SC_HANDLE>>, String> {
    unsafe {
        let manager = Owned::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|error| format!("open service manager failed: {error}"))?,
        );
        match OpenServiceW(
            *manager,
            w!("SunshineService"),
            access | SERVICE_QUERY_STATUS,
        ) {
            Ok(service) => Ok(Some(Owned::new(service))),
            Err(error) if error.code() == ERROR_SERVICE_DOES_NOT_EXIST.to_hresult() => Ok(None),
            Err(error) => Err(format!("open Sunshine service failed: {error}")),
        }
    }
}

fn status(service: SC_HANDLE) -> Result<SERVICE_STATUS_CURRENT_STATE, String> {
    let mut status = SERVICE_STATUS::default();
    unsafe { QueryServiceStatus(service, &mut status) }
        .map_err(|error| format!("query Sunshine service failed: {error}"))?;
    Ok(status.dwCurrentState)
}

fn wait_for_state(
    service: SC_HANDLE,
    expected: SERVICE_STATUS_CURRENT_STATE,
) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let current = status(service)?;
        if current == expected {
            return Ok(());
        }
        if expected == SERVICE_RUNNING && current == SERVICE_STOPPED {
            return Err("Sunshine service stopped before reaching the running state".to_string());
        }
        if Instant::now() >= deadline {
            return Err(format!("Sunshine service timed out in state {}", current.0));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn stop(service: SC_HANDLE) -> Result<(), String> {
    let mut current = status(service)?;
    if current == SERVICE_START_PENDING {
        wait_for_state(service, SERVICE_RUNNING)?;
        current = status(service)?;
    }
    if current == SERVICE_STOPPED {
        return Ok(());
    }
    if current != SERVICE_STOP_PENDING {
        let mut result = SERVICE_STATUS::default();
        unsafe { ControlService(service, SERVICE_CONTROL_STOP, &mut result) }
            .map_err(|error| format!("stop Sunshine service failed: {error}"))?;
    }
    wait_for_state(service, SERVICE_STOPPED)
}

pub(crate) fn stop_for_update() -> Result<(), String> {
    if let Some(service) = open_service(SERVICE_STOP)? {
        stop(*service)?;
    }
    Ok(())
}

fn process_ids(executable_name: &str) -> Result<Vec<u32>, String> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    unsafe {
        let snapshot = Owned::new(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|error| format!("snapshot processes failed: {error}"))?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut next = Process32FirstW(*snapshot, &mut entry);
        let mut processes = Vec::new();
        loop {
            match next {
                Ok(()) => {
                    let length = entry
                        .szExeFile
                        .iter()
                        .position(|value| *value == 0)
                        .unwrap_or(entry.szExeFile.len());
                    if String::from_utf16_lossy(&entry.szExeFile[..length])
                        .eq_ignore_ascii_case(executable_name)
                    {
                        processes.push(entry.th32ProcessID);
                    }
                    next = Process32NextW(*snapshot, &mut entry);
                }
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => return Err(format!("enumerate processes failed: {error}")),
            }
        }
        Ok(processes)
    }
}

pub(crate) fn is_user_mode() -> Result<bool, String> {
    if let Some(service) = open_service(0)?
        && status(*service)? != SERVICE_STOPPED
    {
        return Ok(false);
    }
    Ok(!process_ids("sunshine.exe")?.is_empty())
}

fn change_mode(user_mode: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::*;

    let root = crate::sunshine::install_dir();
    let executable = root
        .join("sunshine.exe")
        .canonicalize()
        .map_err(|error| format!("resolve Sunshine executable failed: {error}"))?;
    let service = open_service(SERVICE_STOP | SERVICE_START)?;
    if let Some(service) = &service {
        stop(**service)?;
    }
    // Preserve the existing mode switch's cleanup of leftover Core processes.
    // Hold each process handle through termination and exit waiting.
    for pid in process_ids("sunshine.exe")? {
        unsafe {
            let process = match OpenProcess(
                PROCESS_TERMINATE | PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                Ok(process) => Owned::new(process),
                Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => continue,
                Err(error) => return Err(format!("open Sunshine process failed: {error}")),
            };
            if WaitForSingleObject(*process, 0) == WAIT_OBJECT_0 {
                continue;
            }
            // A snapshot PID may have been reused; also leave other installations alone.
            let mut image = vec![0u16; 32768];
            let mut length = image.len() as u32;
            QueryFullProcessImageNameW(
                *process,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(image.as_mut_ptr()),
                &mut length,
            )
            .map_err(|error| format!("identify Sunshine process failed: {error}"))?;
            use std::os::windows::ffi::OsStringExt;
            let image =
                std::path::PathBuf::from(std::ffi::OsString::from_wide(&image[..length as usize]));
            if image.canonicalize().map_err(|error| error.to_string())? != executable {
                continue;
            }
            TerminateProcess(*process, 0)
                .map_err(|error| format!("stop Sunshine process failed: {error}"))?;
            if WaitForSingleObject(*process, 10_000) != WAIT_OBJECT_0 {
                return Err("Sunshine process did not exit".to_string());
            }
        }
    }
    if !user_mode && let Some(service) = service {
        unsafe { StartServiceW(*service, None) }
            .map_err(|error| format!("start Sunshine service failed: {error}"))?;
        wait_for_state(*service, SERVICE_RUNNING)
    } else {
        // No installed service retains the existing direct-launch fallback.
        std::process::Command::new(executable)
            .current_dir(root)
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("start Sunshine failed: {error}"))
    }
}

pub(crate) async fn run_mode_change(user_mode: bool) -> Result<(), String> {
    use tokio::io::AsyncReadExt;
    let token = uuid::Uuid::new_v4();
    let mut server = crate::elevation::create_hardened_pipe(
        &crate::elevation::pipe_name("service", token),
        "SERVICE-IPC-001",
    )?;
    let process = crate::elevation::spawn_helper(
        vec![
            HELPER_ARG.into(),
            token.to_string(),
            if user_mode { "user" } else { "service" }.into(),
        ],
        "SERVICE-UAC-001",
    )
    .await?;
    crate::elevation::connect_verified(&mut server, &process, TIMEOUT, "SERVICE-IPC-001").await?;
    let mut response = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(120),
        server.take(8193).read_to_end(&mut response),
    )
    .await
    .map_err(|_| "Sunshine mode switch timed out".to_string())?
    .map_err(|error| format!("read Sunshine mode switch result failed: {error}"))?;
    if response.len() > 8192 {
        return Err("Sunshine mode switch response is too large".to_string());
    }
    serde_json::from_slice::<Result<(), String>>(&response)
        .map_err(|error| format!("invalid Sunshine mode switch result: {error}"))?
}

pub(crate) fn try_handle_command() -> Option<i32> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next().as_deref() != Some(HELPER_ARG) {
        return None;
    }
    let Some(token) = arguments
        .next()
        .and_then(|value| uuid::Uuid::parse_str(&value).ok())
    else {
        return Some(2);
    };
    let user_mode = match arguments.next().as_deref() {
        Some("user") => true,
        Some("service") => false,
        _ => return Some(2),
    };
    if arguments.next().is_some() || !crate::elevation::is_elevated() {
        return Some(2);
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return Some(3),
    };
    Some(runtime.block_on(async {
        use tokio::io::AsyncWriteExt;
        let mut pipe = match crate::elevation::connect_helper_pipe(
            &crate::elevation::pipe_name("service", token),
            "SERVICE-IPC-001",
        )
        .await
        {
            Ok(pipe) => pipe,
            Err(_) => return 3,
        };
        // Do not bind a kill-on-close job: the newly launched Core must survive this helper.
        let result = change_mode(user_mode);
        let code = i32::from(result.is_err());
        let encoded = serde_json::to_vec(&result).expect("string result serialization");
        if pipe.write_all(&encoded).await.is_err() {
            return 3;
        }
        code
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_process_snapshot_is_readable() {
        let executable = std::env::current_exe().unwrap();
        let name = executable.file_name().unwrap().to_str().unwrap();
        assert!(process_ids(name).unwrap().contains(&std::process::id()));
    }

    #[test]
    fn scm_query_does_not_need_service_control_access() {
        if let Some(service) = open_service(0).unwrap() {
            status(*service).unwrap();
        }
    }
}
