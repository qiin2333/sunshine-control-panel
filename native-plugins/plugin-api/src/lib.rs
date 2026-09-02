use std::ffi::{c_char, c_void};

pub const NATIVE_TOOL_ABI_V1: u32 = 1;

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeToolResult {
    Ok = 0,
    AlreadyOpen = 1,
    InvalidHost = -1,
    StartFailed = -2,
    StopTimeout = -3,
    InternalError = -4,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeToolLogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

pub type NativeToolLogFn =
    unsafe extern "C" fn(context: *mut c_void, level: NativeToolLogLevel, message: *const c_char);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeToolHostV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub context: *mut c_void,
    pub log: Option<NativeToolLogFn>,
    // Borrowed HICON values owned by the host. Plugins must not destroy them.
    pub default_window_icon: isize,
    pub default_small_window_icon: isize,
}

pub const fn resolve_window_icon(custom_icon: isize, default_icon: isize) -> isize {
    if custom_icon != 0 {
        custom_icon
    } else {
        default_icon
    }
}

pub type InitializeFn = unsafe extern "C" fn(host: *const NativeToolHostV1) -> NativeToolResult;
pub type ShowFn = unsafe extern "C" fn() -> NativeToolResult;
pub type RequestCloseFn = unsafe extern "C" fn() -> NativeToolResult;
pub type ShutdownFn = unsafe extern "C" fn(timeout_ms: u32) -> NativeToolResult;
pub type IsRunningFn = unsafe extern "C" fn() -> bool;
pub type CanUnloadFn = unsafe extern "C" fn() -> bool;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeToolPluginV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub tool_id: *const c_char,
    pub plugin_version: *const c_char,
    pub display_name: *const u16,
    pub initialize: Option<InitializeFn>,
    pub show: Option<ShowFn>,
    pub request_close: Option<RequestCloseFn>,
    pub shutdown: Option<ShutdownFn>,
    pub is_running: Option<IsRunningFn>,
    pub can_unload: Option<CanUnloadFn>,
}

// The ABI table is immutable after construction and contains only pointers to
// static strings and exported functions. Implementations must keep those
// targets alive for the complete lifetime of the loaded DLL.
unsafe impl Send for NativeToolPluginV1 {}
unsafe impl Sync for NativeToolPluginV1 {}

pub type GetPluginApiFn = unsafe extern "C" fn(host_abi_version: u32) -> *const NativeToolPluginV1;

pub const GET_PLUGIN_API_SYMBOL: &[u8] = b"AlkaidLabNativeTool_GetApi\0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_window_icon_overrides_the_host_default() {
        assert_eq!(resolve_window_icon(11, 22), 11);
        assert_eq!(resolve_window_icon(0, 22), 22);
        assert_eq!(resolve_window_icon(0, 0), 0);
    }
}
