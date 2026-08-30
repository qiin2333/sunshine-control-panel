#![cfg(target_os = "windows")]
//! Elevated (administrator) DualSense operations over a local named pipe.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::install::{dualsense_install_impl, dualsense_self_test_impl, dualsense_uninstall_impl};
use super::packages::{
    LocalComponentPackages, classify_local_component_packages, component_root,
    local_component_package_handoff_path,
};
use super::probe::ensure_no_active_session;
use super::{
    MAX_LOCAL_COMPONENT_PACKAGE_BYTES, MAX_LOCAL_COMPONENT_PACKAGES,
    MAX_LOCAL_COMPONENT_TOTAL_BYTES, emit_progress,
};

#[cfg(target_os = "windows")]
pub(crate) const ELEVATED_DS5_ARG: &str = "--elevated-dualsense";
#[cfg(target_os = "windows")]
pub(crate) const MAX_ELEVATED_MESSAGE_BYTES: usize = 64 * 1024;
#[cfg(target_os = "windows")]
pub(crate) const ELEVATION_CONNECT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(10);

#[cfg(target_os = "windows")]
pub(crate) static ELEVATED_HELPER_JOB: OnceCell<std::os::windows::io::OwnedHandle> =
    OnceCell::new();

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ElevatedOperation {
    Install,
    InstallLocal,
    TestStandard,
    TestComposite,
    Uninstall,
}

#[cfg(target_os = "windows")]
impl ElevatedOperation {
    pub(crate) fn as_arg(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::InstallLocal => "install-local",
            Self::TestStandard => "test-standard",
            Self::TestComposite => "test-composite",
            Self::Uninstall => "uninstall",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "install" => Some(Self::Install),
            "install-local" => Some(Self::InstallLocal),
            "test-standard" => Some(Self::TestStandard),
            "test-composite" => Some(Self::TestComposite),
            "uninstall" => Some(Self::Uninstall),
            _ => None,
        }
    }

    pub(crate) fn timeout(self) -> std::time::Duration {
        match self {
            // Installation can include three component downloads with a shared
            // thirty-minute budget each, followed by the USB/IP installer.
            Self::Install | Self::InstallLocal => std::time::Duration::from_secs(110 * 60),
            Self::TestStandard | Self::TestComposite => std::time::Duration::from_secs(90),
            Self::Uninstall => std::time::Duration::from_secs(120),
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) struct TemporaryPackageFile(PathBuf);

#[cfg(target_os = "windows")]
impl TemporaryPackageFile {
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(target_os = "windows")]
impl Drop for TemporaryPackageFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ElevatedMessage {
    Progress { stage: String, progress: u32 },
    Complete { data: serde_json::Value },
    Error { message: String },
}

#[cfg(target_os = "windows")]
/// Bind the elevated helper and all children it launches to one lifetime job.
fn bind_elevated_helper_lifetime() -> Result<(), String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::core::PCWSTR;

    ELEVATED_HELPER_JOB
        .get_or_try_init(|| unsafe {
            let raw_job = CreateJobObjectW(None, PCWSTR::null()).map_err(|error| {
                format!("DS5-PKG-003: unable to create elevated helper job: {error}")
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
                format!("DS5-PKG-003: unable to configure elevated helper job: {error}")
            })?;
            AssignProcessToJobObject(job_handle, GetCurrentProcess()).map_err(|error| {
                format!("DS5-PKG-003: unable to bind elevated helper lifetime: {error}")
            })?;
            Ok(job)
        })
        .map(|_| ())
}

#[cfg(target_os = "windows")]
pub(crate) fn elevated_pipe_name(token: uuid::Uuid) -> String {
    format!(r"\\.\pipe\sunshine-dualsense-{token}")
}

#[cfg(target_os = "windows")]
async fn connect_elevated_pipe(
    token: uuid::Uuid,
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, String> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let pipe_name = elevated_pipe_name(token);
    for _ in 0..100 {
        match ClientOptions::new().read(true).write(true).open(&pipe_name) {
            Ok(client) => return Ok(client),
            Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(error) => {
                return Err(format!(
                    "DS5-PKG-004: unable to connect to the administrator operation: {error}"
                ));
            }
        }
    }
    Err("DS5-PKG-004: administrator operation pipe was unavailable".to_string())
}

#[cfg(target_os = "windows")]
async fn receive_local_component_packages<R>(
    reader: &mut R,
    token: uuid::Uuid,
) -> Result<Vec<TemporaryPackageFile>, String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let root = component_root();
    receive_local_component_packages_into(reader, token, &root).await
}

#[cfg(target_os = "windows")]
pub(crate) async fn receive_local_component_packages_into<R>(
    reader: &mut R,
    token: uuid::Uuid,
    root: &Path,
) -> Result<Vec<TemporaryPackageFile>, String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut encoded_count = [0u8; 1];
    reader
        .read_exact(&mut encoded_count)
        .await
        .map_err(|error| {
            format!("DS5-PKG-002: unable to receive the local package count: {error}")
        })?;
    let count = usize::from(encoded_count[0]);
    if count == 0 || count > MAX_LOCAL_COMPONENT_PACKAGES {
        return Err("DS5-PKG-002: the number of selected local packages is invalid".to_string());
    }

    fs::create_dir_all(root).map_err(|error| {
        format!("DS5-PKG-002: unable to prepare the component directory: {error}")
    })?;
    let mut packages = Vec::with_capacity(count);
    let mut total_size = 0u64;
    for index in 0..count {
        let mut encoded_size = [0u8; std::mem::size_of::<u64>()];
        reader
            .read_exact(&mut encoded_size)
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to receive local package {index} size: {error}")
            })?;
        let size = u64::from_le_bytes(encoded_size);
        total_size = total_size.checked_add(size).ok_or_else(|| {
            "DS5-PKG-002: the selected local packages exceed the transfer limit".to_string()
        })?;
        if size == 0
            || size > MAX_LOCAL_COMPONENT_PACKAGE_BYTES
            || total_size > MAX_LOCAL_COMPONENT_TOTAL_BYTES
        {
            return Err("DS5-PKG-002: a selected local package is invalid".to_string());
        }

        let package =
            TemporaryPackageFile(local_component_package_handoff_path(root, token, index));
        let mut output = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(package.path())
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to create a component handoff file: {error}")
            })?;
        let mut limited = reader.take(size);
        let copied = tokio::io::copy(&mut limited, &mut output)
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to receive local package {index}: {error}")
            })?;
        if copied != size {
            return Err(format!(
                "DS5-PKG-002: local package {index} transfer ended early"
            ));
        }
        output.flush().await.map_err(|error| {
            format!("DS5-PKG-002: unable to finish local package {index} transfer: {error}")
        })?;
        packages.push(package);
    }
    Ok(packages)
}

#[cfg(target_os = "windows")]
async fn run_elevated_helper(operation: ElevatedOperation, token: uuid::Uuid) -> i32 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let Ok(pipe) = connect_elevated_pipe(token).await else {
        return 3;
    };
    let (mut pipe_reader, mut pipe_writer) = tokio::io::split(pipe);
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<ElevatedMessage>();
    let writer = tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            let mut encoded = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
            encoded.push(b'\n');
            pipe_writer
                .write_all(&encoded)
                .await
                .map_err(|error| error.to_string())?;
        }
        pipe_writer
            .shutdown()
            .await
            .map_err(|error| error.to_string())
    });
    let local_packages = if operation == ElevatedOperation::InstallLocal {
        receive_local_component_packages(&mut pipe_reader, token).await
    } else {
        Ok(Vec::new())
    };
    // After the optional package handoff, EOF means that the parent timed out
    // or failed. Long-running child processes are job-bound and terminate when
    // Windows closes this process's handles.
    let disconnect_watcher = tokio::spawn(async move {
        let mut control = [0u8; 1];
        let _ = pipe_reader.read(&mut control).await;
        std::process::exit(5);
    });

    let progress_sender = sender.clone();
    let progress = move |stage: &str, value: u32| {
        let _ = progress_sender.send(ElevatedMessage::Progress {
            stage: stage.to_string(),
            progress: value.min(100),
        });
    };
    let outcome: Result<serde_json::Value, String> = async {
        bind_elevated_helper_lifetime()?;
        if !crate::bat_runner::is_elevated() {
            return Err("DS5-PKG-004: administrator authorization was not granted".to_string());
        }
        ensure_no_active_session().await?;
        match operation {
            ElevatedOperation::Install | ElevatedOperation::InstallLocal => {
                let local_packages = local_packages?;
                let classified = if local_packages.is_empty() {
                    LocalComponentPackages::default()
                } else {
                    let package_paths = local_packages
                        .iter()
                        .map(|package| package.path().to_path_buf())
                        .collect::<Vec<_>>();
                    tokio::task::spawn_blocking(move || {
                        classify_local_component_packages(package_paths)
                    })
                    .await
                    .map_err(|error| {
                        format!("DS5-PKG-002: local package classification task failed: {error}")
                    })??
                };
                serde_json::to_value(dualsense_install_impl(&progress, classified).await?)
                    .map_err(|error| error.to_string())
            }
            ElevatedOperation::TestStandard => {
                dualsense_self_test_impl("standard".to_string()).await
            }
            ElevatedOperation::TestComposite => {
                dualsense_self_test_impl("composite".to_string()).await
            }
            ElevatedOperation::Uninstall => serde_json::to_value(dualsense_uninstall_impl().await?)
                .map_err(|error| error.to_string()),
        }
    }
    .await;

    let succeeded = outcome.is_ok();
    let final_message = match outcome {
        Ok(data) => ElevatedMessage::Complete { data },
        Err(message) => ElevatedMessage::Error { message },
    };
    let _ = sender.send(final_message);
    drop(progress);
    drop(sender);
    // The parent closes its pipe end after receiving the final message. Stop
    // treating that expected EOF as cancellation before allowing the writer
    // to finish and close its half of the connection.
    disconnect_watcher.abort();
    let _ = disconnect_watcher.await;
    let writer_result = writer.await;
    match writer_result {
        Ok(Ok(())) => {}
        Ok(Err(_)) | Err(_) => return 4,
    }
    if succeeded { 0 } else { 1 }
}

/// Handle a narrowly allowlisted elevated DualSense operation before Tauri or
/// WebView startup. No caller-provided paths or commands are accepted.
#[cfg(target_os = "windows")]
pub(crate) fn try_handle_elevated_command() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next()?.as_str() != ELEVATED_DS5_ARG {
        return None;
    }
    let operation = match args.next().as_deref().and_then(ElevatedOperation::parse) {
        Some(operation) => operation,
        None => return Some(2),
    };
    let token = match args
        .next()
        .and_then(|value| uuid::Uuid::parse_str(&value).ok())
    {
        Some(token) if args.next().is_none() => token,
        _ => return Some(2),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return Some(3),
    };
    Some(runtime.block_on(run_elevated_helper(operation, token)))
}

#[cfg(target_os = "windows")]
pub(crate) async fn read_limited_elevated_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<String>, String> {
    use tokio::io::AsyncBufReadExt;

    let mut bytes = Vec::with_capacity(1024);
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|error| format!("DS5-PKG-004: administrator IPC read failed: {error}"))?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index);
        if bytes.len().saturating_add(take) > MAX_ELEVATED_MESSAGE_BYTES {
            return Err(format!(
                "DS5-PKG-004: administrator IPC message exceeds {MAX_ELEVATED_MESSAGE_BYTES} bytes"
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("DS5-PKG-004: administrator IPC was not UTF-8: {error}"))
}

#[cfg(target_os = "windows")]
async fn wait_for_elevated_process_exit(
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

#[cfg(target_os = "windows")]
pub(crate) async fn wait_for_elevated_pipe_connection<Connect, HelperExit>(
    connect: Connect,
    helper_exit: HelperExit,
) -> Result<(), String>
where
    Connect: std::future::Future<Output = std::io::Result<()>>,
    HelperExit: std::future::Future<Output = Result<Option<i32>, String>>,
{
    tokio::select! {
        biased;
        connected = connect => connected.map_err(|error| {
            format!("DS5-PKG-004: administrator IPC connection failed: {error}")
        }),
        status = helper_exit => {
            let status = status
                .map_err(|error| format!("DS5-PKG-004: administrator helper wait failed: {error}"))?;
            let Some(status) = status else {
                return Err("DS5-PKG-004: administrator authorization timed out".to_string());
            };
            Err(format!(
                "DS5-PKG-004: administrator authorization was canceled or the helper exited early ({status})"
            ))
        },
    }
}

#[cfg(target_os = "windows")]
pub(crate) async fn run_elevated_operation(
    app: Option<&tauri::AppHandle>,
    operation: ElevatedOperation,
    selected_packages: &[PathBuf],
) -> Result<serde_json::Value, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ServerOptions;

    let token = uuid::Uuid::new_v4();
    let mut local_packages = if operation == ElevatedOperation::InstallLocal {
        if selected_packages.is_empty() || selected_packages.len() > MAX_LOCAL_COMPONENT_PACKAGES {
            return Err(
                "DS5-PKG-002: select between one and three local component packages".to_string(),
            );
        }
        let mut total_size = 0u64;
        let mut opened = Vec::with_capacity(selected_packages.len());
        for source in selected_packages {
            let file = tokio::fs::File::open(source).await.map_err(|error| {
                format!("DS5-PKG-002: unable to open a selected component package: {error}")
            })?;
            let metadata = file.metadata().await.map_err(|error| {
                format!("DS5-PKG-002: unable to inspect a selected component package: {error}")
            })?;
            total_size = total_size.checked_add(metadata.len()).ok_or_else(|| {
                "DS5-PKG-002: the selected local packages exceed the transfer limit".to_string()
            })?;
            if !metadata.is_file()
                || metadata.len() == 0
                || metadata.len() > MAX_LOCAL_COMPONENT_PACKAGE_BYTES
                || total_size > MAX_LOCAL_COMPONENT_TOTAL_BYTES
            {
                return Err("DS5-PKG-002: a selected component package is invalid".to_string());
            }
            opened.push((file, metadata.len()));
        }
        Some(opened)
    } else {
        if !selected_packages.is_empty() {
            return Err("DS5-PKG-002: local packages are not valid for this operation".to_string());
        }
        None
    };
    let pipe_name = elevated_pipe_name(token);
    let mut server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(true)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe_name)
        .map_err(|error| format!("DS5-PKG-004: unable to create administrator IPC: {error}"))?;
    let operation_arg = operation.as_arg();
    let token_arg = token.to_string();
    let elevated_process = tokio::task::spawn_blocking(move || {
        crate::utils::launch_current_executable_elevated(
            &[ELEVATED_DS5_ARG, operation_arg, &token_arg],
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0,
        )
    })
    .await
    .map_err(|error| format!("DS5-PKG-004: administrator launch task failed: {error}"))?
    .map_err(|error| {
        format!("DS5-PKG-004: unable to request administrator authorization: {error}")
    })?;
    let mut helper_exit = Box::pin(wait_for_elevated_process_exit(
        &elevated_process,
        ELEVATION_CONNECT_TIMEOUT,
    ));

    wait_for_elevated_pipe_connection(server.connect(), &mut helper_exit).await?;
    drop(helper_exit);
    if let Some(packages) = local_packages.take() {
        server
            .write_all(&[packages.len() as u8])
            .await
            .map_err(|error| {
                format!("DS5-PKG-002: unable to transfer the local package count: {error}")
            })?;
        for (file, size) in packages {
            server
                .write_all(&size.to_le_bytes())
                .await
                .map_err(|error| {
                    format!("DS5-PKG-002: unable to transfer a selected component package: {error}")
                })?;
            let mut limited = file.take(size);
            let copied = tokio::io::copy(&mut limited, &mut server)
                .await
                .map_err(|error| {
                    format!("DS5-PKG-002: unable to transfer a selected component package: {error}")
                })?;
            if copied != size {
                return Err(
                    "DS5-PKG-002: a selected component package changed while it was being transferred"
                    .to_string(),
            );
            }
        }
        server.flush().await.map_err(|error| {
            format!("DS5-PKG-002: unable to finish the local package transfer: {error}")
        })?;
    }

    let receive = async move {
        let mut reader = BufReader::new(server);
        let mut final_result = None;
        while let Some(line) = read_limited_elevated_line(&mut reader).await? {
            let message: ElevatedMessage = serde_json::from_str(&line).map_err(|error| {
                format!("DS5-PKG-004: invalid administrator IPC response: {error}")
            })?;
            match message {
                ElevatedMessage::Progress { stage, progress } => {
                    if let Some(app) = app {
                        emit_progress(app, &stage, progress);
                    }
                }
                ElevatedMessage::Complete { data } => final_result = Some(Ok(data)),
                ElevatedMessage::Error { message } => final_result = Some(Err(message)),
            }
        }
        Ok::<_, String>(final_result)
    };
    let final_result = match tokio::time::timeout(operation.timeout(), receive).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            // Dropping the pipe wakes the elevated helper's disconnect watcher.
            let _ = wait_for_elevated_process_exit(
                &elevated_process,
                std::time::Duration::from_secs(5),
            )
            .await;
            return Err(error);
        }
        Err(_) => {
            // Dropping the timed-out receive future closes the pipe and causes
            // the helper to terminate itself even though this process cannot
            // directly kill a high-integrity child.
            let _ = wait_for_elevated_process_exit(
                &elevated_process,
                std::time::Duration::from_secs(5),
            )
            .await;
            return Err("DS5-PKG-004: administrator operation timed out".to_string());
        }
    };
    let status =
        wait_for_elevated_process_exit(&elevated_process, std::time::Duration::from_secs(5))
            .await
            .map_err(|error| format!("DS5-PKG-004: administrator helper wait failed: {error}"))?
            .ok_or_else(|| "DS5-PKG-004: administrator helper did not exit".to_string())?;
    match final_result {
        Some(Ok(data)) if status == 0 => Ok(data),
        Some(Err(error)) => Err(error),
        _ => Err(format!(
            "DS5-PKG-004: administrator helper failed with exit code {}",
            status
        )),
    }
}
