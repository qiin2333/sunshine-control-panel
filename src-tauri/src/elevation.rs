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