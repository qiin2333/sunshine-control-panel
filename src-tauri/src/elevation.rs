//! Shared plumbing for elevated same-binary helper processes.
//!
//! Privileged operations re-launch the current executable with the `runas`
//! verb and speak to the parent over a per-invocation named pipe. Every
//! helper stack needs the same skeleton: a hardened pipe whose name embeds a
//! fresh UUID, the elevated launch, a connect race that notices an early
//! helper exit (UAC cancellation) instead of waiting out the timeout,
//! verification that the pipe client is the process this parent spawned, a
//! poll-based exit-code wait, lifetime job binding so grandchildren die with
//! the helper, and a native elevation probe. This module owns those parts;
//! protocols and operations stay with each stack.

/// Pipe name for one elevated invocation. `stack` names the calling feature
/// (e.g. "usbip", "dualsense") so concurrent stacks never collide.
pub(crate) fn pipe_name(stack: &str, token: uuid::Uuid) -> String {
    format!(r"\\.\pipe\sunshine-{stack}-{token}")
}

/// Create the parent side of the helper pipe. `first_pipe_instance` makes
/// CreateNamedPipe fail if the name already exists (no local squatting) and
/// `reject_remote_clients` confines clients to this machine; the embedded
/// UUID makes the name unguessable.
pub(crate) fn create_hardened_pipe(
    name: &str,
    error_code: &str,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, String> {
    use tokio::net::windows::named_pipe::ServerOptions;

    ServerOptions::new()
        .access_inbound(true)
        .access_outbound(true)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(name)
        .map_err(|error| format!("{error_code}: unable to create administrator IPC: {error}"))
}

/// Launch this executable elevated with the given helper arguments. Returns
/// as soon as the UAC consent is granted or canceled.
pub(crate) async fn spawn_helper(
    arguments: Vec<String>,
    error_code: &str,
) -> Result<crate::utils::ElevatedProcess, String> {
    tokio::task::spawn_blocking(move || {
        let encoded: Vec<&str> = arguments.iter().map(String::as_str).collect();
        crate::utils::launch_current_executable_elevated(
            &encoded,
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("{error_code}: administrator launch task failed: {error}"))?
    .map_err(|error| format!("{error_code}: administrator authorization was canceled: {error}"))
}

/// Poll the elevated process for up to `timeout`. Returns `None` when it is
/// still running when the timeout elapses.
pub(crate) async fn wait_for_exit(
    process: &crate::utils::ElevatedProcess,
    timeout: std::time::Duration,
) -> Result<Option<i32>, String> {
    let wait = async {
        loop {
            if let Some(exit_code) = process.exit_code()? {
                return Ok::<_, String>(exit_code);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    };

    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result.map(Some),
        Err(_) => Ok(None),
    }
}

/// Prefer a ready pipe connection over a ready helper exit: `biased` polls the
/// connect future first, so a helper that exits after connecting still yields
/// a successful connection.
pub(crate) async fn wait_for_pipe_connection<Connect, HelperExit>(
    connect: Connect,
    helper_exit: HelperExit,
    error_code: &str,
) -> Result<(), String>
where
    Connect: std::future::Future<Output = std::io::Result<()>>,
    HelperExit: std::future::Future<Output = Result<Option<i32>, String>>,
{
    tokio::select! {
        biased;
        connected = connect => connected.map_err(|error| {
            format!("{error_code}: administrator IPC connection failed: {error}")
        }),
        status = helper_exit => {
            let status = status
                .map_err(|error| format!("{error_code}: administrator helper wait failed: {error}"))?;
            let Some(status) = status else {
                return Err(format!("{error_code}: administrator authorization timed out"));
            };
            Err(format!(
                "{error_code}: administrator authorization was canceled or the helper exited early ({status})"
            ))
        },
    }
}

/// Accept one helper connection and prove it belongs to the process this
/// parent spawned: an early helper exit (UAC cancellation, crash) loses the
/// race immediately instead of consuming the whole connect timeout, and a
/// client whose PID does not match the spawned child is rejected.
pub(crate) async fn connect_verified(
    server: &mut tokio::net::windows::named_pipe::NamedPipeServer,
    process: &crate::utils::ElevatedProcess,
    connect_timeout: std::time::Duration,
    error_code: &str,
) -> Result<(), String> {
    let mut helper_exit = Box::pin(wait_for_exit(process, connect_timeout));
    wait_for_pipe_connection(server.connect(), &mut helper_exit, error_code).await?;
    verify_client_process_id(server, process.process_id(), error_code)
}

/// The pipe server must never serve a client other than the elevated process
/// this parent spawned for this invocation.
pub(crate) fn verify_client_process_id(
    server: &tokio::net::windows::named_pipe::NamedPipeServer,
    expected_process_id: u32,
    error_code: &str,
) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;

    let mut client_process_id = 0;
    unsafe {
        GetNamedPipeClientProcessId(HANDLE(server.as_raw_handle()), &mut client_process_id)
            .map_err(|error| {
                format!("{error_code}: unable to authenticate administrator IPC client: {error}")
            })?;
    }
    if client_process_id == 0 || client_process_id != expected_process_id {
        return Err(format!(
            "{error_code}: administrator IPC client is not the process authorized for this session"
        ));
    }
    Ok(())
}

/// Connect to the parent's helper pipe from inside the elevated child,
/// retrying while the pipe is still being created or is busy.
pub(crate) async fn connect_helper_pipe(
    name: &str,
    error_code: &str,
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, String> {
    use tokio::net::windows::named_pipe::ClientOptions;

    for _ in 0..100 {
        match ClientOptions::new().read(true).write(true).open(name) {
            Ok(client) => return Ok(client),
            Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(error) => {
                return Err(format!(
                    "{error_code}: unable to connect to the administrator operation: {error}"
                ));
            }
        }
    }
    Err(format!("{error_code}: administrator operation pipe was unavailable"))
}

static HELPER_JOB: once_cell::sync::OnceCell<std::os::windows::io::OwnedHandle> =
    once_cell::sync::OnceCell::new();

/// Bind the elevated helper and every process it launches to one lifetime
/// job. Windows kills the whole tree when this process's handles close, so
/// grandchildren (installers, downloaders) can never outlive the helper.
pub(crate) fn bind_lifetime_job(error_code: &str) -> Result<(), String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::core::PCWSTR;

    HELPER_JOB
        .get_or_try_init(|| unsafe {
            let raw_job = CreateJobObjectW(None, PCWSTR::null()).map_err(|error| {
                format!("{error_code}: unable to create the elevated helper job: {error}")
            })?;
            let job = std::os::windows::io::OwnedHandle::from_raw_handle(raw_job.0);
            let job_handle = HANDLE(job.as_raw_handle());
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job_handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
            .map_err(|error| {
                format!("{error_code}: unable to configure the elevated helper job: {error}")
            })?;
            AssignProcessToJobObject(job_handle, GetCurrentProcess()).map_err(|error| {
                format!("{error_code}: unable to bind the elevated helper lifetime: {error}")
            })?;
            Ok(job)
        })
        .map(|_| ())
}

/// Native elevation probe: reads the current process token instead of
/// spawning PowerShell. A directly-launched (non-UAC) helper fails this.
pub(crate) fn is_elevated() -> bool {
    crate::utils::is_running_as_admin().unwrap_or(false)
}

/// Quote one argument the way the C runtime parses command lines, so a
/// re-launched process receives it verbatim. Rejects NUL bytes.
pub(crate) fn quote_windows_argument(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("elevated process argument contains a NUL byte".to_string());
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    Ok(quoted)
}

/// Launch an arbitrary program with the Windows `runas` verb.
///
/// Blocks until the UAC consent is granted or canceled, then returns the
/// process handle for exit waiting. `program` should be an absolute path —
/// callers resolve system binaries from `%SystemRoot%` rather than trusting
/// PATH. Arguments are C-runtime quoted.
pub(crate) fn launch_elevated(
    program: &std::ffi::OsStr,
    arguments: &[&str],
    show_window: i32,
) -> Result<crate::utils::ElevatedProcess, String> {
    let parameters = arguments
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    launch_elevated_raw(program, &parameters, show_window)
}

/// Launch an arbitrary program with the Windows `runas` verb, passing
/// `raw_parameters` to `lpParameters` verbatim — for consumers (cmd.exe)
/// whose command-line dialect is not the C-runtime quoting rules.
///
/// Blocks until the UAC consent is granted or canceled.
pub(crate) fn launch_elevated_raw(
    program: &std::ffi::OsStr,
    raw_parameters: &str,
    show_window: i32,
) -> Result<crate::utils::ElevatedProcess, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::core::PCWSTR;

    fn os_to_wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let verb = os_to_wide("runas".as_ref());
    let executable = os_to_wide(program);
    let parameters = os_to_wide(raw_parameters.as_ref());
    let mut execute_info = SHELLEXECUTEINFOW::default();
    execute_info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    execute_info.fMask = SEE_MASK_NOCLOSEPROCESS;
    execute_info.hwnd = HWND(std::ptr::null_mut());
    execute_info.lpVerb = PCWSTR(verb.as_ptr());
    execute_info.lpFile = PCWSTR(executable.as_ptr());
    execute_info.lpParameters = PCWSTR(parameters.as_ptr());
    execute_info.nShow = show_window;

    unsafe {
        ShellExecuteExW(&mut execute_info)
            .map_err(|error| format!("Windows could not start the elevated process: {error}"))?;
    }
    if execute_info.hProcess.0.is_null() {
        return Err("Windows did not return an elevated process handle".to_string());
    }
    use std::os::windows::io::FromRawHandle;
    Ok(crate::utils::ElevatedProcess::from_owned_handle(unsafe {
        std::os::windows::io::OwnedHandle::from_raw_handle(execute_info.hProcess.0)
    }))
}

/// Resolve a system binary as an absolute path, independent of PATH.
pub(crate) fn system_binary(name: &str) -> std::path::PathBuf {
    let system_root =
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    std::path::Path::new(&system_root).join("System32").join(name)
}

/// Run one cmd.exe command line elevated (hidden window) and wait for its exit
/// code.
///
/// The command is passed as `/d /s /c "<command_line>"`: with `/s`, cmd strips
/// exactly the outer quote pair and executes the rest verbatim, so the
/// command's own quoting travels unmodified. `command_line` must therefore
/// contain no NUL bytes and must quote its own paths; it is NOT re-escaped —
/// cmd does not understand the C-runtime `\"` convention.
pub(crate) fn run_cmd_elevated(
    command_line: &str,
    timeout: std::time::Duration,
) -> Result<i32, String> {
    if command_line.contains('\0') {
        return Err("elevated command contains a NUL byte".to_string());
    }
    // Switches must stay unquoted: cmd applies its /c quote-stripping rule
    // only when the switches are bare. With /s, exactly the outer quote pair
    // around the command is stripped and the rest executes verbatim.
    let parameters = format!("/d /s /c \"{command_line}\"");
    let process = launch_elevated_raw(
        system_binary("cmd.exe").as_os_str(),
        &parameters,
        windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
    )?;
    process
        .wait_for_exit_blocking(timeout)?
        .ok_or_else(|| "elevated command timed out".to_string())
}

/// Run one PowerShell `-Command <inner>` elevated (hidden window) and wait
/// for its exit code. `inner` is passed verbatim: callers already quote their
/// own values with PowerShell single quotes, and the C-runtime quoting at the
/// launch layer protects the argument boundary — no further escaping here.
pub(crate) async fn run_powershell_elevated(
    inner_command: &str,
    timeout: std::time::Duration,
) -> Result<i32, String> {
    let inner = inner_command.to_string();
    tokio::task::spawn_blocking(move || {
        let process = launch_elevated(
            system_binary(r"WindowsPowerShell\v1.0\powershell.exe").as_os_str(),
            &["-NoProfile", "-Command", &inner],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )?;
        process
            .wait_for_exit_blocking(timeout)?
            .ok_or_else(|| "elevated PowerShell command timed out".to_string())
    })
    .await
    .map_err(|error| format!("elevated PowerShell task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::quote_windows_argument;

    #[test]
    fn quotes_arguments_like_the_c_runtime() {
        assert_eq!(quote_windows_argument("abc").unwrap(), "\"abc\"");
        assert_eq!(quote_windows_argument("a b").unwrap(), "\"a b\"");
        assert_eq!(quote_windows_argument("a\"b").unwrap(), "\"a\\\"b\"");
        assert_eq!(quote_windows_argument("a\\b").unwrap(), "\"a\\b\"");
        assert_eq!(quote_windows_argument("a\\").unwrap(), "\"a\\\\\"");
        assert_eq!(quote_windows_argument("a\\\\\"b").unwrap(), "\"a\\\\\\\\\\\"b\"");
        assert!(quote_windows_argument("a\0b").is_err());
    }
}