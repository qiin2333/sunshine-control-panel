use alkaidlab_native_tool_plugin_api::{
    GET_PLUGIN_API_SYMBOL, GetPluginApiFn, NATIVE_TOOL_ABI_V1, NativeToolHostV1,
    NativeToolLogLevel, NativeToolPluginV1, NativeToolResult,
};
use log::{debug, error, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::ExtractIconExW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::HICON;
#[cfg(target_os = "windows")]
use windows::core::{PCSTR, PCWSTR};

const PLUGIN_SHUTDOWN_TIMEOUT_MS: u32 = 2_000;
const PLUGIN_MONITOR_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct PluginDescriptor {
    id: &'static str,
    file_name: &'static str,
}

const PLUGINS: &[PluginDescriptor] = &[PluginDescriptor {
    id: "alkaidlab.stylus",
    file_name: "alkaidlab-plugin-stylus.dll",
}];

#[derive(Clone, Copy, Debug)]
enum NativePluginError {
    Unknown,
    NotFound,
    PathInvalid,
    LoadFailed,
    EntryMissing,
    AbiMismatch,
    IdMismatch,
    InitFailed,
    StartFailed,
}

impl NativePluginError {
    const fn code(self) -> &'static str {
        match self {
            Self::Unknown => "NATIVE_PLUGIN_UNKNOWN",
            Self::NotFound => "NATIVE_PLUGIN_NOT_FOUND",
            Self::PathInvalid => "NATIVE_PLUGIN_PATH_INVALID",
            Self::LoadFailed => "NATIVE_PLUGIN_LOAD_FAILED",
            Self::EntryMissing => "NATIVE_PLUGIN_ENTRY_MISSING",
            Self::AbiMismatch => "NATIVE_PLUGIN_ABI_MISMATCH",
            Self::IdMismatch => "NATIVE_PLUGIN_ID_MISMATCH",
            Self::InitFailed => "NATIVE_PLUGIN_INIT_FAILED",
            Self::StartFailed => "NATIVE_PLUGIN_START_FAILED",
        }
    }
}

struct LoadedPlugin {
    module: usize,
    api: NativeToolPluginV1,
    generation: u64,
}

#[derive(Default)]
struct PluginManager {
    loaded: HashMap<&'static str, LoadedPlugin>,
}

fn manager() -> &'static Mutex<PluginManager> {
    static MANAGER: OnceLock<Mutex<PluginManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(PluginManager::default()))
}

fn descriptor(tool_id: &str) -> Result<PluginDescriptor, NativePluginError> {
    PLUGINS
        .iter()
        .copied()
        .find(|plugin| plugin.id == tool_id)
        .ok_or(NativePluginError::Unknown)
}

fn plugin_path(file_name: &str) -> Result<PathBuf, NativePluginError> {
    let executable = std::env::current_exe().map_err(|_| NativePluginError::PathInvalid)?;
    let directory = executable
        .parent()
        .ok_or(NativePluginError::PathInvalid)?
        .canonicalize()
        .map_err(|_| NativePluginError::PathInvalid)?;
    let requested = directory.join(file_name);
    let metadata = std::fs::symlink_metadata(&requested).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            NativePluginError::NotFound
        } else {
            NativePluginError::PathInvalid
        }
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(NativePluginError::PathInvalid);
    }
    let canonical = requested
        .canonicalize()
        .map_err(|_| NativePluginError::PathInvalid)?;
    if canonical.parent() != Some(directory.as_path()) {
        return Err(NativePluginError::PathInvalid);
    }
    Ok(canonical)
}

unsafe extern "C" fn plugin_log(
    _context: *mut c_void,
    level: NativeToolLogLevel,
    message: *const c_char,
) {
    if message.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    match level {
        NativeToolLogLevel::Debug => debug!(target: "native_tool", "{message}"),
        NativeToolLogLevel::Info => info!(target: "native_tool", "{message}"),
        NativeToolLogLevel::Warning => warn!(target: "native_tool", "{message}"),
        NativeToolLogLevel::Error => error!(target: "native_tool", "{message}"),
    }
}

#[cfg(target_os = "windows")]
fn default_window_icons() -> (isize, isize) {
    static ICONS: OnceLock<(isize, isize)> = OnceLock::new();
    *ICONS.get_or_init(|| {
        let icon_path = crate::tray::default_window_icon_path()
            .ok()
            .or_else(|| std::env::current_exe().ok());
        let Some(icon_path) = icon_path else {
            return (0, 0);
        };
        let mut path: Vec<u16> = icon_path.as_os_str().encode_wide().collect();
        path.push(0);
        let mut large = HICON::default();
        let mut small = HICON::default();
        if unsafe {
            ExtractIconExW(
                PCWSTR(path.as_ptr()),
                0,
                Some(&mut large),
                Some(&mut small),
                1,
            )
        } == 0
        {
            return (0, 0);
        }
        (large.0 as isize, small.0 as isize)
    })
}

fn tool_id_matches(pointer: *const c_char, expected: &str) -> Result<bool, NativePluginError> {
    if pointer.is_null() {
        return Err(NativePluginError::IdMismatch);
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(|actual| actual == expected)
        .map_err(|_| NativePluginError::IdMismatch)
}

#[cfg(target_os = "windows")]
fn load_plugin(descriptor: PluginDescriptor) -> Result<LoadedPlugin, NativePluginError> {
    let path = plugin_path(descriptor.file_name)?;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let module = unsafe {
        LoadLibraryExW(
            PCWSTR(wide.as_ptr()),
            None,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    }
    .map_err(|_| NativePluginError::LoadFailed)?;

    let loaded = (|| {
        let symbol = unsafe { GetProcAddress(module, PCSTR(GET_PLUGIN_API_SYMBOL.as_ptr())) }
            .ok_or(NativePluginError::EntryMissing)?;
        let get_api: GetPluginApiFn = unsafe { std::mem::transmute(symbol) };
        let api_pointer = unsafe { get_api(NATIVE_TOOL_ABI_V1) };
        let api = unsafe { api_pointer.as_ref() }.ok_or(NativePluginError::AbiMismatch)?;
        if api.struct_size < size_of::<NativeToolPluginV1>() as u32
            || api.abi_version != NATIVE_TOOL_ABI_V1
        {
            return Err(NativePluginError::AbiMismatch);
        }
        if !tool_id_matches(api.tool_id, descriptor.id)? {
            return Err(NativePluginError::IdMismatch);
        }
        let initialize = api.initialize.ok_or(NativePluginError::AbiMismatch)?;
        if api.show.is_none()
            || api.request_close.is_none()
            || api.shutdown.is_none()
            || api.is_running.is_none()
            || api.can_unload.is_none()
        {
            return Err(NativePluginError::AbiMismatch);
        }
        let (default_window_icon, default_small_window_icon) = default_window_icons();
        let host = NativeToolHostV1 {
            struct_size: size_of::<NativeToolHostV1>() as u32,
            abi_version: NATIVE_TOOL_ABI_V1,
            context: std::ptr::null_mut(),
            log: Some(plugin_log),
            default_window_icon,
            default_small_window_icon,
        };
        if unsafe { initialize(&host) } != NativeToolResult::Ok {
            if let Some(shutdown) = api.shutdown {
                let _ = unsafe { shutdown(PLUGIN_SHUTDOWN_TIMEOUT_MS) };
            }
            return Err(NativePluginError::InitFailed);
        }
        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        Ok(LoadedPlugin {
            module: module.0 as usize,
            api: *api,
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
        })
    })();

    if loaded.is_err() {
        let _ = unsafe { FreeLibrary(module) };
    }
    loaded
}

#[cfg(not(target_os = "windows"))]
fn load_plugin(_descriptor: PluginDescriptor) -> Result<LoadedPlugin, NativePluginError> {
    Err(NativePluginError::LoadFailed)
}

fn show_plugin(plugin: &LoadedPlugin) -> Result<(), NativePluginError> {
    let show = plugin.api.show.ok_or(NativePluginError::AbiMismatch)?;
    match unsafe { show() } {
        NativeToolResult::Ok | NativeToolResult::AlreadyOpen => Ok(()),
        _ => Err(NativePluginError::StartFailed),
    }
}

fn start_unload_monitor(tool_id: &'static str, generation: u64) {
    if let Err(error) = std::thread::Builder::new()
        .name(format!("native-tool-monitor-{tool_id}"))
        .spawn(move || {
            loop {
                std::thread::sleep(PLUGIN_MONITOR_INTERVAL);
                let mut module_to_free = None;
                {
                    let Ok(mut manager) = manager().lock() else {
                        return;
                    };
                    let Some(plugin) = manager.loaded.get(tool_id) else {
                        return;
                    };
                    if plugin.generation != generation {
                        return;
                    }
                    let Some(is_running) = plugin.api.is_running else {
                        return;
                    };
                    if unsafe { is_running() } {
                        continue;
                    }
                    let Some(shutdown) = plugin.api.shutdown else {
                        return;
                    };
                    let Some(can_unload) = plugin.api.can_unload else {
                        return;
                    };
                    if unsafe { shutdown(PLUGIN_SHUTDOWN_TIMEOUT_MS) } == NativeToolResult::Ok
                        && unsafe { can_unload() }
                    {
                        module_to_free = manager.loaded.remove(tool_id).map(|plugin| plugin.module);
                    }
                }
                if let Some(module) = module_to_free {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = unsafe { FreeLibrary(HMODULE(module as *mut c_void)) };
                    }
                    info!(target: "native_tool", "Unloaded native tool plugin [{tool_id}]");
                    return;
                }
            }
        })
    {
        warn!(target: "native_tool", "Unable to start plugin unload monitor: {error}");
    }
}

fn open_native_tool_impl(tool_id: &str) -> Result<(), NativePluginError> {
    let descriptor = descriptor(tool_id)?;
    let mut manager = manager()
        .lock()
        .map_err(|_| NativePluginError::StartFailed)?;
    if let Some(plugin) = manager.loaded.get(descriptor.id) {
        return show_plugin(plugin);
    }

    let plugin = load_plugin(descriptor)?;
    if let Err(error) = show_plugin(&plugin) {
        if let Some(shutdown) = plugin.api.shutdown {
            let _ = unsafe { shutdown(PLUGIN_SHUTDOWN_TIMEOUT_MS) };
        }
        let unloadable = plugin.api.can_unload.map(|check| unsafe { check() }) == Some(true);
        if unloadable {
            #[cfg(target_os = "windows")]
            {
                let _ = unsafe { FreeLibrary(HMODULE(plugin.module as *mut c_void)) };
            }
        } else {
            let generation = plugin.generation;
            manager.loaded.insert(descriptor.id, plugin);
            drop(manager);
            start_unload_monitor(descriptor.id, generation);
        }
        return Err(error);
    }
    let generation = plugin.generation;
    manager.loaded.insert(descriptor.id, plugin);
    drop(manager);
    start_unload_monitor(descriptor.id, generation);
    info!(target: "native_tool", "Loaded native tool plugin [{}]", descriptor.id);
    Ok(())
}

#[derive(Serialize)]
pub struct NativeToolOpenResponse {
    tool_id: String,
    status: &'static str,
}

#[tauri::command]
pub async fn open_native_tool(tool_id: String) -> Result<NativeToolOpenResponse, String> {
    let requested_id = tool_id.clone();
    tokio::task::spawn_blocking(move || open_native_tool_impl(&requested_id))
        .await
        .map_err(|_| NativePluginError::StartFailed.code().to_string())?
        .map_err(|error| error.code().to_string())?;
    Ok(NativeToolOpenResponse {
        tool_id,
        status: "open",
    })
}

pub fn shutdown_all() {
    let plugins = manager()
        .lock()
        .map(|mut manager| {
            manager
                .loaded
                .drain()
                .map(|(_, plugin)| plugin)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for plugin in plugins {
        if let Some(request_close) = plugin.api.request_close {
            let _ = unsafe { request_close() };
        }
        let stopped = plugin
            .api
            .shutdown
            .map(|shutdown| unsafe { shutdown(PLUGIN_SHUTDOWN_TIMEOUT_MS) })
            == Some(NativeToolResult::Ok);
        let unloadable = plugin.api.can_unload.map(|check| unsafe { check() }) == Some(true);
        if stopped && unloadable {
            #[cfg(target_os = "windows")]
            {
                let _ = unsafe { FreeLibrary(HMODULE(plugin.module as *mut c_void)) };
            }
        } else {
            warn!(target: "native_tool", "Native tool plugin remained loaded during GUI shutdown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_unknown_tools() {
        assert!(matches!(
            descriptor("unknown.tool"),
            Err(NativePluginError::Unknown)
        ));
    }

    #[test]
    fn registry_uses_fixed_stylus_filename() {
        let plugin = descriptor("alkaidlab.stylus").expect("stylus plugin must be registered");
        assert_eq!(plugin.file_name, "alkaidlab-plugin-stylus.dll");
    }
}
