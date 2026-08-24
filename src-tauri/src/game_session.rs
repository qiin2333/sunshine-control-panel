//! Tracked game launches for the big-screen shell.
//!
//! The shell used to launch through `commands::launch_app`, which started a
//! command and forgot about it. In big-screen mode that left three holes: the
//! `prep-cmd` undo steps configured by the launch assistant never ran, the
//! fullscreen shell kept covering the game it just started, and the library
//! could not show what was running. This module owns the launch instead — it
//! keeps the process handle, waits for the exit, and drives the undo / window /
//! playtime side of the lifecycle.
//!
//! Only one game is tracked at a time, which matches the shell's single
//! `launchingApp` slot.

use std::collections::HashMap;
use std::fs;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

const DESKTOP_WINDOW_LABEL: &str = "desktop";
const STATS_FILE: &str = "game-stats.json";

/// Wait slice used while polling the tracked process, short enough to notice a
/// stop request quickly without spinning.
const POLL_SLICE_MS: u32 = 400;

/// A launched process that exits this fast is usually a launcher shim (Steam,
/// Epic, a .lnk trampoline), so look for the real game instead of declaring the
/// session over.
const LAUNCHER_EXIT_GRACE: Duration = Duration::from_secs(30);

/// How long to keep looking for the real game process after a shim exits.
const ADOPT_SEARCH: Duration = Duration::from_secs(20);

/// Interval between adoption scans.
const ADOPT_SCAN_INTERVAL: Duration = Duration::from_millis(750);

// ===== Public payloads =====

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningGameInfo {
    pub app_name: String,
    pub pid: u32,
    pub started_at_ms: u64,
    pub elapsed_seconds: u64,
    /// True once the originally launched process was replaced by a process we
    /// adopted by executable name.
    pub adopted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExitedEvent {
    pub app_name: String,
    pub seconds: u64,
    /// True when the exit was caused by `stop_running_game`.
    pub stopped: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub app_name: String,
    /// False when the launch succeeded but no process could be watched, so the
    /// shell must not show a running-game bar or wait for an exit.
    pub tracked: bool,
    pub pid: Option<u32>,
    /// Machine-readable reason tracking was skipped, for the UI to explain.
    /// One of `no-handle`, `detached-only`.
    pub untracked_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStat {
    #[serde(default)]
    pub total_seconds: u64,
    #[serde(default)]
    pub launch_count: u32,
    #[serde(default)]
    pub last_played_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GameStats {
    #[serde(default)]
    pub games: HashMap<String, GameStat>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    /// Minimize the fullscreen shell after a successful launch and restore it
    /// when the game exits.
    #[serde(default)]
    pub auto_yield: bool,
}

// ===== Tracked state =====

struct RunningState {
    app_name: String,
    pid: u32,
    started_at: Instant,
    started_at_ms: u64,
    adopted: bool,
    generation: u64,
    stop: Arc<AtomicBool>,
}

fn running_slot() -> &'static Mutex<Option<RunningState>> {
    static SLOT: OnceLock<Mutex<Option<RunningState>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

fn snapshot_running() -> Option<RunningGameInfo> {
    let guard = running_slot().lock().ok()?;
    let state = guard.as_ref()?;
    Some(RunningGameInfo {
        app_name: state.app_name.clone(),
        pid: state.pid,
        started_at_ms: state.started_at_ms,
        elapsed_seconds: state.started_at.elapsed().as_secs(),
        adopted: state.adopted,
    })
}

// ===== apps.json field helpers =====

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn value_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "enabled" => Some(true),
            "false" | "0" | "no" | "disabled" | "" => Some(false),
            _ => None,
        },
        Value::Number(value) => Some(value.as_i64().unwrap_or(0) != 0),
        _ => None,
    }
}

/// One `prep-cmd` entry's undo half, kept so the monitor can unwind the launch.
#[derive(Clone, Debug)]
struct UndoStep {
    command: String,
    elevated: bool,
}

fn collect_undo_steps(app: &Value) -> Vec<UndoStep> {
    let Some(entries) = app.get("prep-cmd").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            Some(UndoStep {
                command: value_string(entry, "undo")?,
                elevated: value_bool(entry.get("elevated")).unwrap_or(false),
            })
        })
        .collect()
}

// ===== Win32 launch primitives =====

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0u16)).collect()
}

fn error_desc(code: isize) -> &'static str {
    match code {
        0 | 8 => "Out of memory",
        2 => "File not found",
        3 => "Path not found",
        5 => "Access denied",
        11 => "Invalid format",
        26 => "Sharing violation",
        27 => "Incomplete file association",
        28 => "DDE request timed out",
        29 => "DDE transaction failed",
        30 => "DDE busy",
        31 => "No associated application",
        32 => "DLL not found",
        _ => "Unknown error",
    }
}

/// One `file` + `params` split to hand to `ShellExecuteExW`.
struct Attempt {
    file: String,
    params: String,
}

/// Reproduce the historical launch parsing ladder: a quoted executable is
/// authoritative, otherwise try the whole string as a file (so paths with spaces
/// and URI handlers work) before splitting on the first space.
fn launch_attempts(command: &str) -> Vec<Attempt> {
    let trimmed = command.trim();
    if trimmed.starts_with('"') {
        if let Some(end) = trimmed[1..].find('"') {
            let file = trimmed[1..=end].to_string();
            let params = if end + 2 < trimmed.len() {
                trimmed[end + 2..].trim().to_string()
            } else {
                String::new()
            };
            return vec![Attempt { file, params }];
        }
    }

    let mut attempts = vec![Attempt {
        file: trimmed.to_string(),
        params: String::new(),
    }];
    if let Some(space) = trimmed.find(' ') {
        attempts.push(Attempt {
            file: trimmed[..space].to_string(),
            params: trimmed[space + 1..].trim().to_string(),
        });
    }
    attempts
}

struct SpawnedProcess {
    handle: Option<OwnedHandle>,
    pid: Option<u32>,
    /// Lowercased basename of the executable that actually started, used to
    /// adopt the real game when a launcher shim exits.
    exe_name: Option<String>,
}

/// Run one command through the shell, keeping the process handle when Windows
/// gives us one. `hInstApp` carries the same legacy status codes that
/// `ShellExecuteW` returns, so the error messages stay identical to before.
fn shell_execute(
    file: &str,
    params: &str,
    working_dir: Option<&[u16]>,
    elevated: bool,
) -> (isize, Option<OwnedHandle>, Option<u32>) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let verb = to_wide(if elevated { "runas" } else { "open" });
    let file_wide = to_wide(file);
    let params_wide = to_wide(params);

    let mut info = SHELLEXECUTEINFOW::default();
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.hwnd = HWND(std::ptr::null_mut());
    info.lpVerb = PCWSTR(verb.as_ptr());
    info.lpFile = PCWSTR(file_wide.as_ptr());
    info.lpParameters = PCWSTR(params_wide.as_ptr());
    info.lpDirectory = working_dir.map_or(PCWSTR::null(), |dir| PCWSTR(dir.as_ptr()));
    info.nShow = SW_SHOWNORMAL.0;

    let ok = unsafe { ShellExecuteExW(&mut info) }.is_ok();
    let status = info.hInstApp.0 as isize;
    if !ok {
        return (status, None, None);
    }

    if info.hProcess.0.is_null() {
        // Document verbs and URI handlers routed to an already-running process
        // return success without a handle.
        return (status, None, None);
    }

    let pid = unsafe { GetProcessId(info.hProcess) };
    let handle = unsafe { OwnedHandle::from_raw_handle(info.hProcess.0) };
    (status, Some(handle), (pid != 0).then_some(pid))
}

fn exe_basename(file: &str) -> Option<String> {
    let name = file
        .trim()
        .trim_matches('"')
        .rsplit(['\\', '/'])
        .next()?
        .to_ascii_lowercase();
    name.ends_with(".exe").then_some(name)
}

/// Launch one command, returning the process handle when the shell provided
/// one. Preserves the original error-code preference: when the whole-string
/// attempt fails with "file/path not found", report the split attempt's error.
fn spawn_command(
    command: &str,
    elevated: bool,
    working_dir: Option<&[u16]>,
    label: &str,
) -> Result<SpawnedProcess, String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Ok(SpawnedProcess {
            handle: None,
            pid: None,
            exe_name: None,
        });
    }

    let attempts = launch_attempts(trimmed);
    let mut first_status = 0isize;

    for (index, attempt) in attempts.iter().enumerate() {
        info!(
            "  [{} try{}] file: {}, params: {}",
            label,
            index + 1,
            attempt.file,
            attempt.params
        );
        let (status, handle, pid) =
            shell_execute(&attempt.file, &attempt.params, working_dir, elevated);
        if status > 32 {
            return Ok(SpawnedProcess {
                handle,
                pid,
                exe_name: exe_basename(&attempt.file),
            });
        }
        if index == 0 {
            first_status = status;
        } else {
            // Keep the more useful of the two failures.
            let best = if first_status == 2 || first_status == 3 {
                status
            } else {
                first_status
            };
            return Err(format!(
                "{} failed: {} (code {})",
                label,
                error_desc(best),
                best
            ));
        }
    }

    Err(format!(
        "{} failed: {} (code {})",
        label,
        error_desc(first_status),
        first_status
    ))
}

/// Run a command and wait for it to finish, used for `prep-cmd` undo steps so
/// they unwind in order instead of racing each other.
fn run_and_wait(command: &str, elevated: bool, working_dir: Option<&[u16]>, label: &str) {
    match spawn_command(command, elevated, working_dir, label) {
        Ok(spawned) => {
            if let Some(handle) = spawned.handle {
                // Undo steps are short; cap the wait so a hung script cannot
                // block the rest of the unwind forever.
                wait_for_exit(&handle, 15_000);
            }
        }
        Err(error) => warn!("{}", error),
    }
}

/// Returns true when the process exited within `timeout_ms`.
fn wait_for_exit(handle: &OwnedHandle, timeout_ms: u32) -> bool {
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::WaitForSingleObject;

    let raw = HANDLE(handle.as_raw_handle());
    unsafe { WaitForSingleObject(raw, timeout_ms) == WAIT_OBJECT_0 }
}

fn open_for_wait(pid: u32) -> Option<OwnedHandle> {
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
    }
    .ok()?;
    Some(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}

fn find_processes_by_name(exe_name_lower: &str) -> Vec<u32> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let own_pid = std::process::id();
    let mut found = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return found;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
                let pid = entry.th32ProcessID;
                if name == exe_name_lower && pid != own_pid && pid != 0 {
                    found.push(pid);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    found
}

/// Current time as a Windows FILETIME tick count, comparable with process
/// creation times.
fn now_filetime() -> u64 {
    use windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime;

    let stamp = unsafe { GetSystemTimeAsFileTime() };
    ((stamp.dwHighDateTime as u64) << 32) | stamp.dwLowDateTime as u64
}

fn process_start_filetime(handle: &OwnedHandle) -> Option<u64> {
    use windows::Win32::Foundation::{FILETIME, HANDLE};
    use windows::Win32::System::Threading::GetProcessTimes;

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(
            HANDLE(handle.as_raw_handle()),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    }
    .ok()?;
    Some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
}

fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    match std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) if output.status.success() => info!("Terminated game process tree {}", pid),
        Ok(output) => warn!(
            "taskkill on {} exited with {}: {}",
            pid,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => warn!("Could not run taskkill on {}: {}", pid, error),
    }
}

// ===== Playtime stats =====

fn stats_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|dir| dir.join("Sunshine GUI").join(STATS_FILE))
        .ok_or_else(|| "Cannot resolve the user config directory".to_string())
}

fn stats_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn load_stats() -> GameStats {
    let Ok(path) = stats_path() else {
        return GameStats::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return GameStats::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_stats(stats: &GameStats) -> Result<(), String> {
    let path = stats_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(stats).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| error.to_string())
}

/// Apply one change to a game's stats under the file lock so the launch and
/// exit writes cannot clobber each other.
fn update_stat(app_name: &str, apply: impl FnOnce(&mut GameStat)) {
    let _guard = stats_lock().lock();
    let mut stats = load_stats();
    apply(stats.games.entry(app_name.to_string()).or_default());
    if let Err(error) = save_stats(&stats) {
        warn!("Could not persist game stats: {}", error);
    }
}

// ===== Shell window handoff =====

fn yield_desktop_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(DESKTOP_WINDOW_LABEL) {
        if window.is_visible().unwrap_or(false) {
            info!("Minimizing the big-screen shell so the game is visible");
            let _ = window.minimize();
        }
    }
}

fn reclaim_desktop_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(DESKTOP_WINDOW_LABEL) {
        info!("Restoring the big-screen shell after the game exited");
        crate::windows::show_and_activate_window(&window);
    }
}

/// Bring the tracked game's main window to the front so "back to game" is real
/// instead of just hiding the shell and hoping the game is next in z-order.
fn focus_window_of_process(pid: u32) -> bool {
    use std::ffi::c_void;
    use windows::Win32::Foundation::{FALSE, HWND, LPARAM, TRUE};
    use windows::Win32::UI::WindowsAndMessaging::{
        ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GW_OWNER, GetWindow,
        GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE,
        SetForegroundWindow, ShowWindow,
    };
    use windows::core::BOOL;

    struct Search {
        pid: u32,
        found: *mut c_void,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };

        let mut owner_pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner_pid)) };
        if owner_pid != search.pid {
            return TRUE;
        }
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return TRUE;
        }
        // Owned windows are dialogs and tool palettes; we want the top-level one.
        let owner = unsafe { GetWindow(hwnd, GW_OWNER) }
            .map(|window| window.0)
            .unwrap_or(std::ptr::null_mut());
        if !owner.is_null() {
            return TRUE;
        }
        if unsafe { GetWindowTextLengthW(hwnd) } == 0 {
            return TRUE;
        }

        search.found = hwnd.0;
        FALSE
    }

    let mut search = Search {
        pid,
        found: std::ptr::null_mut(),
    };
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
    if search.found.is_null() {
        return false;
    }

    let hwnd = HWND(search.found);
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd).as_bool()
    }
}

// ===== Monitor =====

struct MonitorContext {
    app: AppHandle,
    app_name: String,
    generation: u64,
    stop: Arc<AtomicBool>,
    undo_steps: Vec<UndoStep>,
    working_dir: Option<Vec<u16>>,
    auto_yield: bool,
    exe_name: Option<String>,
    /// FILETIME captured before the launch, used to reject adoption candidates
    /// that were already running.
    launched_at_filetime: u64,
}

/// Wait for the game to exit, adopting the real process when the launched one
/// turns out to be a shim, then unwind the launch.
fn monitor_game(mut handle: OwnedHandle, context: MonitorContext) {
    let started = Instant::now();
    let mut current_pid = current_pid_for(context.generation);
    let mut adopted = false;

    loop {
        if context.stop.load(Ordering::Acquire) {
            if let Some(pid) = current_pid {
                kill_process_tree(pid);
            }
            // Give the tree a moment to actually go away before unwinding.
            wait_for_exit(&handle, 5_000);
            break;
        }

        if !wait_for_exit(&handle, POLL_SLICE_MS) {
            continue;
        }

        // The watched process is gone. If it died fast enough to look like a
        // launcher shim, try to find the process it spawned.
        if adopted || started.elapsed() >= LAUNCHER_EXIT_GRACE {
            break;
        }
        let Some(exe_name) = context.exe_name.as_deref() else {
            break;
        };
        match adopt_process(exe_name, current_pid.unwrap_or(0), &context) {
            Some((adopted_handle, adopted_pid)) => {
                info!(
                    "Adopted {} (pid {}) after the launched process exited early",
                    exe_name, adopted_pid
                );
                handle = adopted_handle;
                current_pid = Some(adopted_pid);
                adopted = true;
                mark_adopted(context.generation, adopted_pid);
            }
            None => break,
        }
    }

    let stopped = context.stop.load(Ordering::Acquire);
    let seconds = started.elapsed().as_secs();

    // Only clear the slot if it still belongs to this launch; a newer launch
    // must not be wiped by an older monitor finishing late.
    let owned = clear_running_if_current(context.generation);
    if !owned {
        info!(
            "Skipping teardown for a superseded launch of {}",
            context.app_name
        );
        return;
    }

    for step in context.undo_steps.iter().rev() {
        info!("Running prep-cmd undo: {}", step.command);
        run_and_wait(
            &step.command,
            step.elevated,
            context.working_dir.as_deref(),
            "prep-cmd undo",
        );
    }

    update_stat(&context.app_name, |stat| {
        stat.total_seconds = stat.total_seconds.saturating_add(seconds);
        stat.last_played_ms = now_ms();
    });

    if context.auto_yield {
        reclaim_desktop_window(&context.app);
    }

    let _ = context.app.emit(
        "game-exited",
        GameExitedEvent {
            app_name: context.app_name.clone(),
            seconds,
            stopped,
        },
    );
    info!(
        "Game session ended: {} after {}s",
        context.app_name, seconds
    );
}

/// Look for the real game process after a launcher shim exited.
///
/// Only processes that started at or after this launch are eligible: matching on
/// the executable name alone would happily adopt an already-running Steam or
/// Epic client and then never see an exit, leaving the shell stuck on "running"
/// and skipping the undo steps.
fn adopt_process(
    exe_name: &str,
    exclude_pid: u32,
    context: &MonitorContext,
) -> Option<(OwnedHandle, u32)> {
    let deadline = Instant::now() + ADOPT_SEARCH;
    while Instant::now() < deadline {
        if context.stop.load(Ordering::Acquire) {
            return None;
        }
        for pid in find_processes_by_name(exe_name) {
            if pid == exclude_pid {
                continue;
            }
            let Some(handle) = open_for_wait(pid) else {
                continue;
            };
            match process_start_filetime(&handle) {
                Some(started) if started >= context.launched_at_filetime => {
                    return Some((handle, pid));
                }
                // A process we cannot time is not worth the risk of a wrong adoption.
                _ => continue,
            }
        }
        std::thread::sleep(ADOPT_SCAN_INTERVAL);
    }
    None
}

fn current_pid_for(generation: u64) -> Option<u32> {
    let guard = running_slot().lock().ok()?;
    let state = guard.as_ref()?;
    (state.generation == generation).then_some(state.pid)
}

fn mark_adopted(generation: u64, pid: u32) {
    if let Ok(mut guard) = running_slot().lock() {
        if let Some(state) = guard.as_mut() {
            if state.generation == generation {
                state.pid = pid;
                state.adopted = true;
            }
        }
    }
}

fn clear_running_if_current(generation: u64) -> bool {
    let Ok(mut guard) = running_slot().lock() else {
        return false;
    };
    match guard.as_ref() {
        Some(state) if state.generation == generation => {
            *guard = None;
            true
        }
        _ => false,
    }
}

// ===== Commands =====

/// Launch an app from the big-screen shell and track it.
///
/// Runs `prep-cmd` do steps, then `detached` commands, then the main command.
/// The returned `tracked` flag tells the shell whether an exit will follow.
#[tauri::command]
pub async fn launch_game(
    app_handle: AppHandle,
    app: Value,
    options: Option<LaunchOptions>,
) -> Result<LaunchResult, String> {
    let options = options.unwrap_or_default();
    let app_name = value_string(&app, "name").unwrap_or_else(|| "Unknown".to_string());

    if let Some(running) = snapshot_running() {
        return Err(format!("already-running:{}", running.app_name));
    }

    let cmd = value_string(&app, "cmd").unwrap_or_default();
    let detached: Vec<String> = app
        .get("detached")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();

    if cmd.is_empty() && detached.is_empty() {
        return Err("Launch command cannot be empty".to_string());
    }

    let elevated = value_bool(app.get("elevated")).unwrap_or(false);
    let working_dir = value_string(&app, "working-dir").map(|dir| to_wide(&dir));
    let undo_steps = collect_undo_steps(&app);
    // Captured before anything runs so adoption can reject processes that were
    // already alive when the user pressed launch.
    let launched_at_filetime = now_filetime();
    let watch_override = value_string(&app, "watch-process").map(|name| {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".exe") {
            lower
        } else {
            format!("{}.exe", lower)
        }
    });

    let prep_do: Vec<UndoStep> = app
        .get("prep-cmd")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(UndoStep {
                        command: value_string(entry, "do")?,
                        elevated: value_bool(entry.get("elevated")).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    info!("Launching tracked app '{}': {}", app_name, cmd);

    let launch_app_name = app_name.clone();
    let launch = tokio::task::spawn_blocking(move || -> Result<SpawnedProcess, String> {
        let dir = working_dir.as_deref();

        for step in &prep_do {
            info!("Running prep-cmd do: {}", step.command);
            run_and_wait(&step.command, step.elevated, dir, "prep-cmd");
        }

        for command in &detached {
            // Detached commands are fire-and-forget by definition.
            if let Err(error) = spawn_command(command, elevated, dir, "detached") {
                warn!("{}", error);
            }
        }

        if cmd.is_empty() {
            return Ok(SpawnedProcess {
                handle: None,
                pid: None,
                exe_name: None,
            });
        }

        spawn_command(&cmd, elevated, dir, "launch")
    })
    .await
    .map_err(|error| format!("Launch task failed: {}", error))??;

    let SpawnedProcess {
        handle,
        pid,
        exe_name,
    } = launch;
    let exe_name = watch_override.or(exe_name);

    let Some(handle) = handle else {
        let reason = if pid.is_none() && exe_name.is_none() {
            "detached-only"
        } else {
            "no-handle"
        };
        info!(
            "Launched '{}' without a trackable process ({})",
            launch_app_name, reason
        );
        update_stat(&launch_app_name, |stat| {
            stat.launch_count = stat.launch_count.saturating_add(1);
            stat.last_played_ms = now_ms();
        });
        return Ok(LaunchResult {
            app_name: launch_app_name,
            tracked: false,
            pid: None,
            untracked_reason: Some(reason.to_string()),
        });
    };

    let pid = pid.unwrap_or(0);
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let stop = Arc::new(AtomicBool::new(false));
    let started_at_ms = now_ms();

    if let Ok(mut guard) = running_slot().lock() {
        *guard = Some(RunningState {
            app_name: launch_app_name.clone(),
            pid,
            started_at: Instant::now(),
            started_at_ms,
            adopted: false,
            generation,
            stop: Arc::clone(&stop),
        });
    }

    update_stat(&launch_app_name, |stat| {
        stat.launch_count = stat.launch_count.saturating_add(1);
        stat.last_played_ms = started_at_ms;
    });

    let _ = app_handle.emit(
        "game-launched",
        RunningGameInfo {
            app_name: launch_app_name.clone(),
            pid,
            started_at_ms,
            elapsed_seconds: 0,
            adopted: false,
        },
    );

    if options.auto_yield {
        yield_desktop_window(&app_handle);
    }

    let context = MonitorContext {
        app: app_handle,
        app_name: launch_app_name.clone(),
        generation,
        stop,
        undo_steps,
        working_dir: value_string(&app, "working-dir").map(|dir| to_wide(&dir)),
        auto_yield: options.auto_yield,
        exe_name,
        launched_at_filetime,
    };

    std::thread::Builder::new()
        .name("fd-game-monitor".into())
        .spawn(move || monitor_game(handle, context))
        .map_err(|error| format!("Could not start the game monitor: {}", error))?;

    Ok(LaunchResult {
        app_name: launch_app_name,
        tracked: true,
        pid: (pid != 0).then_some(pid),
        untracked_reason: None,
    })
}

#[tauri::command]
pub async fn get_running_game() -> Result<Option<RunningGameInfo>, String> {
    Ok(snapshot_running())
}

/// Ask the tracked game to exit. The monitor still owns the teardown, so the
/// undo steps and the window handoff run exactly once.
#[tauri::command]
pub async fn stop_running_game() -> Result<bool, String> {
    let stop = {
        let guard = running_slot()
            .lock()
            .map_err(|_| "Game state lock poisoned".to_string())?;
        guard.as_ref().map(|state| Arc::clone(&state.stop))
    };
    match stop {
        Some(flag) => {
            flag.store(true, Ordering::Release);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn get_game_stats() -> Result<GameStats, String> {
    Ok(load_stats())
}

/// Raise the tracked game and step the shell out of the way.
///
/// Returns false when no game is tracked or it has no visible top-level window,
/// so the caller can fall back to just minimizing the shell.
#[tauri::command]
pub async fn focus_running_game(app_handle: AppHandle) -> Result<bool, String> {
    let Some(info) = snapshot_running() else {
        return Ok(false);
    };

    let pid = info.pid;
    let focused = tokio::task::spawn_blocking(move || focus_window_of_process(pid))
        .await
        .map_err(|error| format!("Focus task failed: {}", error))?;

    // Even when SetForegroundWindow is refused, getting the shell out of the way
    // is what the user asked for.
    yield_desktop_window(&app_handle);
    Ok(focused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_commands_produce_a_single_attempt() {
        let attempts = launch_attempts("\"C:\\Program Files\\Game\\game.exe\" -windowed");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].file, "C:\\Program Files\\Game\\game.exe");
        assert_eq!(attempts[0].params, "-windowed");
    }

    #[test]
    fn unquoted_commands_try_the_whole_string_before_splitting() {
        let attempts = launch_attempts("C:\\Games\\My Game\\game.exe -dx12");
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].file, "C:\\Games\\My Game\\game.exe -dx12");
        assert_eq!(attempts[0].params, "");
        assert_eq!(attempts[1].file, "C:\\Games\\My");
        assert_eq!(attempts[1].params, "Game\\game.exe -dx12");
    }

    #[test]
    fn uri_handlers_stay_a_single_attempt() {
        let attempts = launch_attempts("steam://rungameid/440");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].file, "steam://rungameid/440");
    }

    #[test]
    fn only_executables_yield_an_adoption_hint() {
        assert_eq!(
            exe_basename("C:\\Games\\Foo\\Game.EXE"),
            Some("game.exe".to_string())
        );
        assert_eq!(exe_basename("steam://rungameid/440"), None);
        assert_eq!(exe_basename("C:\\docs\\readme.txt"), None);
    }

    #[test]
    fn undo_steps_only_include_entries_that_have_one() {
        let app = serde_json::json!({
            "prep-cmd": [
                { "do": "set-res.exe 4k", "undo": "set-res.exe restore", "elevated": true },
                { "do": "notify.exe" },
                { "do": "", "undo": "cleanup.exe" }
            ]
        });
        let steps = collect_undo_steps(&app);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].command, "set-res.exe restore");
        assert!(steps[0].elevated);
        assert_eq!(steps[1].command, "cleanup.exe");
        assert!(!steps[1].elevated);
    }
}
