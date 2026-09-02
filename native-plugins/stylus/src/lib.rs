#![cfg(target_os = "windows")]

mod analysis;
mod data;

use alkaidlab_native_tool_plugin_api::{
    NATIVE_TOOL_ABI_V1, NativeToolHostV1, NativeToolLogFn, NativeToolLogLevel, NativeToolPluginV1,
    NativeToolResult, resolve_window_icon,
};
use analysis::{MAX_GRAPH_SAMPLES, SamplingAnalysis, TraceSample};
use data::{
    EVENT_BUTTON_ONLY, EVENT_CANCEL, EVENT_CANCEL_ALL, EVENT_DOWN, EVENT_HOVER, EVENT_HOVER_LEAVE,
    EVENT_MOVE, EVENT_UP, MAX_SAMPLES, StylusData, StylusSample,
};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ffi::{c_char, c_void};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{
    COLORREF, GlobalFree, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE,
    WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_GUI_FONT, DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT,
    DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FF_DONTCARE, FW_NORMAL, FillRect, FrameRect, GRAY_BRUSH, GetDC, GetStockObject,
    GetTextExtentPoint32W, HALFTONE, HBITMAP, HBRUSH, HDC, HFONT, HGDIOBJ, HPEN, IntersectClipRect,
    InvalidateRect, LineTo, MoveToEx, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_DOT, PS_SOLID, ReleaseDC,
    RestoreDC, SRCCOPY, SaveDC, ScreenToClient, SelectObject, SetBkMode, SetStretchBltMode,
    SetTextColor, StretchBlt, TRANSPARENT, WHITE_BRUSH,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleHandleExW,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Performance::QueryPerformanceFrequency;
use windows::Win32::UI::Controls::BST_CHECKED;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem,
    GetDpiForWindow, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Input::Pointer::{
    GetPointerPenInfo, GetPointerPenInfoHistory, GetPointerTouchInfo, GetPointerTouchInfoHistory,
    GetPointerType, POINTER_FLAG_INCONTACT, POINTER_PEN_INFO, POINTER_TOUCH_INFO,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETSTYLE, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON,
    CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GWLP_USERDATA, GetClientRect, GetCursorPos, GetMessageExtraInfo, GetMessageW,
    GetWindowLongPtrW, HICON, HMENU, HTCLIENT, IDC_ARROW, IDC_CROSS, KillTimer, LoadCursorW,
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MINMAXINFO, MSG, MessageBoxW, MoveWindow,
    PEN_MASK_PRESSURE, PEN_MASK_ROTATION, PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_PEN, PT_TOUCH,
    PostMessageW, PostQuitMessage, RegisterClassExW, SW_RESTORE, SW_SHOWNORMAL, SWP_NOACTIVATE,
    SWP_NOZORDER, SendMessageW, SetCursor, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE,
    WM_APP, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERENTER, WM_POINTERLEAVE, WM_POINTERUP,
    WM_POINTERUPDATE, WM_SETCURSOR, WM_SETFONT, WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR, w};

const TOOL_ID: &[u8] = b"alkaidlab.stylus\0";
const PLUGIN_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
const SHOW_WINDOW_MESSAGE: u32 = WM_APP + 0x451;
const ANALYSIS_TRACE_POINTS: usize = MAX_GRAPH_SAMPLES + 1;
const START_TIMEOUT: Duration = Duration::from_secs(3);
const MOUSE_POINTER_SIGNATURE: u32 = 0xff51_5700;
const MOUSE_POINTER_SIGNATURE_MASK: u32 = 0xffff_ff00;
const MOUSE_POINTER_TOUCH_FLAG: u32 = 0x0000_0080;
const MOUSE_LEFT_BUTTON: usize = 0x0001;
const MAX_POINTER_HISTORY: u32 = 512;
const CLEAR_BUTTON_ID: usize = 1001;
const FILTER_MOUSE_CHECKBOX_ID: usize = 1002;
const RECORD_BUTTON_ID: usize = 1003;
const OPEN_RECORDING_FOLDER_BUTTON_ID: usize = 1004;
const IMPORT_BUTTON_ID: usize = 1005;
const COPY_REPORT_BUTTON_ID: usize = 1007;
const OPEN_LOG_BUTTON_ID: usize = 1008;
const DETAILS_CHECKBOX_ID: usize = 1009;
const LANGUAGE_COMBO_ID: usize = 1010;
const BASE_DPI: u32 = 96;
const BASE_CANVAS_TOP: i32 = 108;
const BASE_FOOTER_HEIGHT: i32 = 300;
const RECORDING_CHECKPOINT_SAMPLES: usize = 256;
const CF_UNICODETEXT_FORMAT: u32 = 13;
const UNKNOWN_ROTATION: u32 = 0xffff;
const UNKNOWN_TILT: i32 = 0xff;
const REPORT_TIMER_ID: usize = 1;
const REPORT_INTERVAL_MS: u32 = 5_000;
const REPAINT_TIMER_ID: usize = 2;
const REPAINT_INTERVAL_MS: u32 = 16;
const CLOSE_RETRY_TIMER_ID: usize = 3;
const CLOSE_RETRY_INTERVAL_MS: u32 = 50;
const LANGUAGE_AUTO: u8 = 0;
static LANGUAGE_MODE: AtomicU8 = AtomicU8::new(LANGUAGE_AUTO);

type TranslationCatalog = BTreeMap<String, BTreeMap<String, String>>;

struct SupportedLanguage {
    code: &'static str,
    label: &'static str,
}

const SUPPORTED_LANGUAGES: &[SupportedLanguage] = &[
    SupportedLanguage {
        code: "bg",
        label: "Български",
    },
    SupportedLanguage {
        code: "cs",
        label: "Čeština",
    },
    SupportedLanguage {
        code: "de",
        label: "Deutsch",
    },
    SupportedLanguage {
        code: "en",
        label: "English",
    },
    SupportedLanguage {
        code: "en_GB",
        label: "English (UK)",
    },
    SupportedLanguage {
        code: "en_US",
        label: "English (US)",
    },
    SupportedLanguage {
        code: "es",
        label: "Español",
    },
    SupportedLanguage {
        code: "fr",
        label: "Français",
    },
    SupportedLanguage {
        code: "it",
        label: "Italiano",
    },
    SupportedLanguage {
        code: "ja",
        label: "日本語",
    },
    SupportedLanguage {
        code: "pt",
        label: "Português",
    },
    SupportedLanguage {
        code: "ru",
        label: "Русский",
    },
    SupportedLanguage {
        code: "sv",
        label: "Svenska",
    },
    SupportedLanguage {
        code: "tr",
        label: "Türkçe",
    },
    SupportedLanguage {
        code: "zh",
        label: "简体中文",
    },
    SupportedLanguage {
        code: "zh_TW",
        label: "繁體中文",
    },
];

struct Recording {
    writer: BufWriter<File>,
    path: PathBuf,
    origin_performance_count: u64,
    origin_pointer_time: u32,
    last_timestamp_us: u64,
    pending_samples: usize,
    sample_count: usize,
    last_sample: Option<StylusSample>,
    truncation_written: bool,
}

#[derive(Clone, Copy)]
struct TracePoint {
    pointer_id: u32,
    x: i32,
    y: i32,
    pressure: u32,
    timestamp_us: u64,
    break_before: bool,
}

#[derive(Clone, Copy)]
struct PenAttributes {
    pressure_available: bool,
    tilt_x: i32,
    tilt_y: i32,
    tilt_available: bool,
    rotation: u32,
    rotation_available: bool,
}

struct CanvasCache {
    dc: HDC,
    bitmap: HBITMAP,
    original_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
    pen_last: HashMap<u32, TracePoint>,
    touch_last: HashMap<u32, TracePoint>,
    mouse_last: HashMap<u32, TracePoint>,
}

struct FrameCache {
    dc: HDC,
    bitmap: HBITMAP,
    original_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
}

struct UiFont(HFONT);

struct DrawingPens {
    dpi: u32,
    mouse: HPEN,
    touch: HPEN,
    pressure: Vec<HPEN>,
    graph_axis: HPEN,
    graph_threshold: HPEN,
    graph_sample: HPEN,
}

impl DrawingPens {
    fn new(dpi: u32) -> Option<Self> {
        let pens = Self {
            dpi,
            mouse: unsafe {
                CreatePen(
                    PS_SOLID,
                    scale_for_dpi(2, dpi).max(1),
                    COLORREF(0x0041_37dc),
                )
            },
            touch: unsafe {
                CreatePen(
                    PS_SOLID,
                    scale_for_dpi(3, dpi).max(1),
                    COLORREF(0x005a_8e28),
                )
            },
            pressure: (0..8)
                .map(|level| unsafe {
                    let minimum_width = scale_for_dpi(2, dpi).max(1);
                    let maximum_width = scale_for_dpi(12, dpi).max(minimum_width);
                    let line_width = minimum_width + (maximum_width - minimum_width) * level / 7;
                    CreatePen(PS_SOLID, line_width, COLORREF(0x00d9_5f16))
                })
                .collect(),
            graph_axis: unsafe { CreatePen(PS_SOLID, 1, COLORREF(0x00d4_cac3)) },
            graph_threshold: unsafe { CreatePen(PS_DOT, 1, COLORREF(0x0060_60d0)) },
            graph_sample: unsafe {
                CreatePen(
                    PS_SOLID,
                    scale_for_dpi(2, dpi).max(1),
                    COLORREF(0x005a_8e28),
                )
            },
        };
        if pens.mouse.is_invalid()
            || pens.touch.is_invalid()
            || pens.pressure.iter().any(|pen| pen.is_invalid())
            || pens.graph_axis.is_invalid()
            || pens.graph_threshold.is_invalid()
            || pens.graph_sample.is_invalid()
        {
            None
        } else {
            Some(pens)
        }
    }
}

impl Drop for DrawingPens {
    fn drop(&mut self) {
        for pen in self.pressure.iter().copied().chain([
            self.mouse,
            self.touch,
            self.graph_axis,
            self.graph_threshold,
            self.graph_sample,
        ]) {
            if !pen.is_invalid() {
                let _ = unsafe { DeleteObject(HGDIOBJ(pen.0)) };
            }
        }
    }
}

impl Drop for CanvasCache {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.original_bitmap);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

impl Drop for FrameCache {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.original_bitmap);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

impl Drop for UiFont {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(self.0.0)) };
        }
    }
}

impl TraceSample for TracePoint {
    fn timestamp_us(&self) -> u64 {
        self.timestamp_us
    }

    fn coordinates(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    fn breaks_stroke(&self) -> bool {
        self.break_before
    }
}

#[derive(Default)]
struct PenStats {
    down: u64,
    movement: u64,
    up: u64,
    hover: u64,
    cancel: u64,
    errors: u64,
    pressure_min: Option<u32>,
    pressure_max: Option<u32>,
    tilt_x_range: Option<(i32, i32)>,
    tilt_y_range: Option<(i32, i32)>,
    rotation_range: Option<(u32, u32)>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PromotedMouseKind {
    None,
    Pen,
    Touch,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DiagnosticMode {
    Idle,
    Running,
    Completed,
    Imported,
}

#[derive(Clone, Copy)]
enum RecentEventKind {
    PenDown,
    PenMove,
    PenUp,
    TouchDown,
    TouchMove,
    TouchUp,
    MouseDown,
    MouseMove,
    MouseUp,
}

#[derive(Clone, Copy)]
struct RecentInputEvent {
    kind: RecentEventKind,
    pointer_id: u32,
    x: i32,
    y: i32,
}

struct WindowState {
    pen_trace: VecDeque<TracePoint>,
    touch_trace: VecDeque<TracePoint>,
    mouse_trace: VecDeque<TracePoint>,
    analysis_trace: VecDeque<TracePoint>,
    analysis_stroke_point_count: usize,
    last_event: Option<RecentInputEvent>,
    last_pressure: u32,
    last_pressure_available: bool,
    last_tilt_x: i32,
    last_tilt_y: i32,
    last_tilt_available: bool,
    last_rotation: u32,
    last_rotation_available: bool,
    mouse_events: u64,
    mouse_contact_events: u64,
    promoted_mouse_events: u64,
    filtered_promoted_mouse_events: u64,
    filtered_promoted_mouse_contact_events: u64,
    promoted_touch_events: u64,
    touch_events: u64,
    touch_contacts: HashSet<u32>,
    pen_in_contact: bool,
    pen_trace_break_pending: bool,
    last_pointer_id: u32,
    mouse_contact: bool,
    filter_promoted_mouse: bool,
    show_details: bool,
    clear_button: HWND,
    filter_checkbox: HWND,
    record_button: HWND,
    open_recording_folder_button: HWND,
    import_button: HWND,
    copy_report_button: HWND,
    open_log_button: HWND,
    details_checkbox: HWND,
    language_combo: HWND,
    recording: Option<Recording>,
    recording_segment_open: bool,
    recording_truncated: bool,
    stylus_data: StylusData,
    mode: DiagnosticMode,
    stats: PenStats,
    sampling_analysis: SamplingAnalysis,
    sampling_analysis_dirty: bool,
    canvas_dirty: bool,
    canvas_cache: Option<CanvasCache>,
    frame_cache: Option<FrameCache>,
    drawing_pens: Option<DrawingPens>,
    ui_font: Option<UiFont>,
    canvas_cache_invalid: bool,
    canvas_cache_scale_pending: bool,
    repaint_timer_active: bool,
    last_report: String,
    log_path: PathBuf,
    log_writer: Option<BufWriter<File>>,
    dpi: u32,
    canvas_top: i32,
    language_combo_width: i32,
}

struct WindowCreateContext {
    state: Option<Box<Mutex<WindowState>>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            pen_trace: VecDeque::with_capacity(MAX_POINTER_HISTORY as usize),
            touch_trace: VecDeque::with_capacity(64),
            mouse_trace: VecDeque::with_capacity(64),
            analysis_trace: VecDeque::with_capacity(ANALYSIS_TRACE_POINTS),
            analysis_stroke_point_count: 0,
            last_event: None,
            last_pressure: 0,
            last_pressure_available: false,
            last_tilt_x: 0,
            last_tilt_y: 0,
            last_tilt_available: false,
            last_rotation: 0,
            last_rotation_available: false,
            mouse_events: 0,
            mouse_contact_events: 0,
            promoted_mouse_events: 0,
            filtered_promoted_mouse_events: 0,
            filtered_promoted_mouse_contact_events: 0,
            promoted_touch_events: 0,
            touch_events: 0,
            touch_contacts: HashSet::new(),
            pen_in_contact: false,
            pen_trace_break_pending: false,
            last_pointer_id: 0,
            mouse_contact: false,
            filter_promoted_mouse: false,
            show_details: false,
            clear_button: HWND::default(),
            filter_checkbox: HWND::default(),
            record_button: HWND::default(),
            open_recording_folder_button: HWND::default(),
            import_button: HWND::default(),
            copy_report_button: HWND::default(),
            open_log_button: HWND::default(),
            details_checkbox: HWND::default(),
            language_combo: HWND::default(),
            recording: None,
            recording_segment_open: false,
            recording_truncated: false,
            stylus_data: StylusData::default(),
            mode: DiagnosticMode::Idle,
            stats: PenStats::default(),
            sampling_analysis: SamplingAnalysis::default(),
            sampling_analysis_dirty: false,
            canvas_dirty: true,
            canvas_cache: None,
            frame_cache: None,
            drawing_pens: None,
            ui_font: None,
            canvas_cache_invalid: true,
            canvas_cache_scale_pending: false,
            repaint_timer_active: false,
            last_report: tr("status.waiting").to_string(),
            log_path: PathBuf::new(),
            log_writer: None,
            dpi: BASE_DPI,
            canvas_top: BASE_CANVAS_TOP,
            language_combo_width: 220,
        }
    }

    fn push_pen(&mut self, point: TracePoint, attributes: PenAttributes) {
        self.pen_trace.push_back(point);
        if point.break_before {
            self.analysis_trace.clear();
            self.analysis_stroke_point_count = 0;
        }
        let mut analysis_point = point;
        if self.analysis_trace.is_empty() {
            analysis_point.break_before = true;
        }
        self.analysis_trace.push_back(analysis_point);
        self.analysis_stroke_point_count = self.analysis_stroke_point_count.saturating_add(1);
        if self.analysis_trace.len() > ANALYSIS_TRACE_POINTS {
            self.analysis_trace.pop_front();
            if let Some(point) = self.analysis_trace.front_mut() {
                point.break_before = true;
            }
        }
        self.last_pressure = point.pressure;
        self.last_pressure_available = attributes.pressure_available;
        self.last_tilt_x = attributes.tilt_x;
        self.last_tilt_y = attributes.tilt_y;
        self.last_tilt_available = attributes.tilt_available;
        self.last_rotation = attributes.rotation;
        self.last_rotation_available = attributes.rotation_available;
        self.sampling_analysis_dirty = true;
        self.canvas_dirty = true;
        if attributes.pressure_available {
            self.stats.pressure_min = Some(
                self.stats
                    .pressure_min
                    .map_or(point.pressure, |value| value.min(point.pressure)),
            );
            self.stats.pressure_max = Some(
                self.stats
                    .pressure_max
                    .map_or(point.pressure, |value| value.max(point.pressure)),
            );
        }
        if attributes.tilt_available {
            self.stats.tilt_x_range = Some(self.stats.tilt_x_range.map_or(
                (attributes.tilt_x, attributes.tilt_x),
                |(minimum, maximum)| {
                    (
                        minimum.min(attributes.tilt_x),
                        maximum.max(attributes.tilt_x),
                    )
                },
            ));
            self.stats.tilt_y_range = Some(self.stats.tilt_y_range.map_or(
                (attributes.tilt_y, attributes.tilt_y),
                |(minimum, maximum)| {
                    (
                        minimum.min(attributes.tilt_y),
                        maximum.max(attributes.tilt_y),
                    )
                },
            ));
        }
        if attributes.rotation_available {
            self.stats.rotation_range = Some(self.stats.rotation_range.map_or(
                (attributes.rotation, attributes.rotation),
                |(minimum, maximum)| {
                    (
                        minimum.min(attributes.rotation),
                        maximum.max(attributes.rotation),
                    )
                },
            ));
        }
    }

    fn push_mouse(&mut self, point: TracePoint, promoted: PromotedMouseKind) {
        self.mouse_trace.push_back(point);
        self.count_mouse_event(promoted);
        self.canvas_dirty = true;
    }

    fn count_mouse_event(&mut self, promoted: PromotedMouseKind) {
        self.mouse_events = self.mouse_events.saturating_add(1);
        if promoted == PromotedMouseKind::Pen {
            self.promoted_mouse_events = self.promoted_mouse_events.saturating_add(1);
        } else if promoted == PromotedMouseKind::Touch {
            self.promoted_touch_events = self.promoted_touch_events.saturating_add(1);
        }
    }

    fn count_filtered_pen_mouse(&mut self, contact: bool) {
        self.promoted_mouse_events = self.promoted_mouse_events.saturating_add(1);
        self.filtered_promoted_mouse_events = self.filtered_promoted_mouse_events.saturating_add(1);
        if contact {
            self.filtered_promoted_mouse_contact_events = self
                .filtered_promoted_mouse_contact_events
                .saturating_add(1);
        }
    }

    fn push_touch(&mut self, point: TracePoint) {
        self.touch_trace.push_back(point);
        self.touch_events = self.touch_events.saturating_add(1);
        self.canvas_dirty = true;
    }
}

#[derive(Clone, Copy)]
struct HostServices {
    context: usize,
    log: Option<NativeToolLogFn>,
    default_window_icon: isize,
    default_small_window_icon: isize,
}

struct RuntimeState {
    initialized: bool,
    starting: bool,
    running: bool,
    stop_requested: bool,
    hwnd: isize,
    window_thread: Option<JoinHandle<()>>,
    host: HostServices,
}

struct Runtime {
    state: Mutex<RuntimeState>,
    changed: Condvar,
}

struct ThreadDpiGuard(DPI_AWARENESS_CONTEXT);

impl ThreadDpiGuard {
    fn enter() -> Self {
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for ThreadDpiGuard {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe {
                SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime {
        state: Mutex::new(RuntimeState {
            initialized: false,
            starting: false,
            running: false,
            stop_requested: false,
            hwnd: 0,
            window_thread: None,
            host: HostServices {
                context: 0,
                log: None,
                default_window_icon: 0,
                default_small_window_icon: 0,
            },
        }),
        changed: Condvar::new(),
    })
}

fn log_message(level: NativeToolLogLevel, message: &'static [u8]) {
    let host = runtime().state.lock().ok().map(|state| state.host);
    if let Some(HostServices {
        context,
        log: Some(callback),
        ..
    }) = host
    {
        unsafe { callback(context as *mut c_void, level, message.as_ptr().cast()) };
    }
}

fn host_window_icon(small: bool) -> HICON {
    let host = runtime().state.lock().ok().map(|state| state.host);
    let Some(host) = host else {
        return HICON::default();
    };
    let default_icon = if small {
        host.default_small_window_icon
    } else {
        host.default_window_icon
    };
    HICON(resolve_window_icon(0, default_icon) as *mut c_void)
}

fn pointer_id(wparam: WPARAM) -> u32 {
    (wparam.0 as u32) & 0xffff
}

unsafe fn window_state(hwnd: HWND) -> Option<MutexGuard<'static, WindowState>> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Mutex<WindowState>;
    let state = unsafe { pointer.as_ref() }?;
    state.try_lock().ok()
}

fn translations() -> &'static TranslationCatalog {
    static TRANSLATIONS: OnceLock<TranslationCatalog> = OnceLock::new();
    TRANSLATIONS.get_or_init(|| {
        serde_json::from_str(include_str!("../i18n.json"))
            .expect("stylus plugin translations must be valid JSON")
    })
}

fn active_language_code() -> &'static str {
    let selected = LANGUAGE_MODE.load(Ordering::Relaxed);
    if selected == LANGUAGE_AUTO {
        return language_code_for_langid(unsafe { GetUserDefaultUILanguage() });
    }
    SUPPORTED_LANGUAGES
        .get(selected.saturating_sub(1) as usize)
        .map_or("en", |language| language.code)
}

fn language_code_for_langid(language_id: u16) -> &'static str {
    match language_id {
        0x0409 => "en_US",
        0x0809 => "en_GB",
        0x0404 | 0x0c04 | 0x1404 => "zh_TW",
        0x0804 | 0x1004 => "zh",
        0x0416 => "pt",
        _ => match language_id & 0x03ff {
            0x0002 => "bg",
            0x0005 => "cs",
            0x0007 => "de",
            0x0009 => "en",
            0x000a => "es",
            0x000c => "fr",
            0x0010 => "it",
            0x0011 => "ja",
            0x0016 => "pt",
            0x0019 => "ru",
            0x001d => "sv",
            0x001f => "tr",
            _ => "en",
        },
    }
}

fn tr(key: &'static str) -> &'static str {
    translations()
        .get(active_language_code())
        .and_then(|locale| locale.get(key))
        .or_else(|| translations().get("en").and_then(|locale| locale.get(key)))
        .map(String::as_str)
        .unwrap_or(key)
}

fn apply_window_title(hwnd: HWND) {
    let title = wide(tr("window.title"));
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(title.as_ptr())) };
}

fn recent_event_text(event: Option<RecentInputEvent>) -> String {
    let Some(event) = event else {
        return tr("report.no_contact_event").to_string();
    };
    let name = match event.kind {
        RecentEventKind::PenDown => "PEN_DOWN",
        RecentEventKind::PenMove => "PEN_MOVE",
        RecentEventKind::PenUp => "PEN_UP",
        RecentEventKind::TouchDown => "TOUCH_DOWN",
        RecentEventKind::TouchMove => "TOUCH_MOVE",
        RecentEventKind::TouchUp => "TOUCH_UP",
        RecentEventKind::MouseDown => "MOUSE_DOWN",
        RecentEventKind::MouseMove => "MOUSE_MOVE",
        RecentEventKind::MouseUp => "MOUSE_UP",
    };
    format!(
        "{name} | pointerId={} | client=({}, {})",
        event.pointer_id, event.x, event.y
    )
}

fn recent_event_pointer_type(event: Option<RecentInputEvent>) -> &'static str {
    match event.map(|event| event.kind) {
        Some(RecentEventKind::PenDown | RecentEventKind::PenMove | RecentEventKind::PenUp) => "PEN",
        Some(
            RecentEventKind::TouchDown | RecentEventKind::TouchMove | RecentEventKind::TouchUp,
        ) => "TOUCH",
        Some(
            RecentEventKind::MouseDown | RecentEventKind::MouseMove | RecentEventKind::MouseUp,
        ) => "MOUSE",
        None => "-",
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn recording_directory() -> PathBuf {
    std::env::temp_dir()
        .join("Sunshine")
        .join("stylus-input-probe")
        .join("recordings")
}

fn timestamped_path(directory: &Path, prefix: &str, extension: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    directory.join(format!("{prefix}-{timestamp}.{extension}"))
}

fn initialize_run_log(state: &mut WindowState) {
    let directory = recording_directory();
    if std::fs::create_dir_all(&directory).is_err() {
        return;
    }
    state.log_path = timestamped_path(&directory, "stylus-input-probe", "log");
    let Ok(file) = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&state.log_path)
    else {
        state.log_path.clear();
        return;
    };
    let mut writer = BufWriter::with_capacity(16 * 1024, file);
    let _ = writer.write_all(&[0xef, 0xbb, 0xbf]);
    state.log_writer = Some(writer);
    write_run_log(state, "START | native stylus diagnostics");
}

fn write_run_log(state: &mut WindowState, message: &str) {
    let Some(writer) = state.log_writer.as_mut() else {
        return;
    };
    if writeln!(writer, "{message}").is_err() || writer.flush().is_err() {
        state.log_writer = None;
    }
}

fn point_in_canvas(hwnd: HWND, state: &WindowState, point: POINT) -> bool {
    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return false;
    }
    let canvas = canvas_rect(state, client);
    point_in_rect(canvas, point)
}

fn point_in_rect(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn canvas_rect(state: &WindowState, client: RECT) -> RECT {
    let margin = scale_for_dpi(16, state.dpi);
    RECT {
        left: client.left + margin,
        top: state.canvas_top,
        right: (client.right - margin).max(client.left + margin + 1),
        bottom: (client.bottom - scale_for_dpi(BASE_FOOTER_HEIGHT, state.dpi))
            .max(state.canvas_top + 1),
    }
}

fn schedule_canvas_repaint(hwnd: HWND, state: &mut WindowState) {
    state.canvas_dirty = true;
    if !state.repaint_timer_active {
        state.repaint_timer_active =
            unsafe { SetTimer(Some(hwnd), REPAINT_TIMER_ID, REPAINT_INTERVAL_MS, None) } != 0;
        if !state.repaint_timer_active {
            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
        }
    }
}

fn analyze_sampling(state: &WindowState) -> SamplingAnalysis {
    let mut analysis = analysis::analyze(&state.analysis_trace);
    analysis.point_count = state.analysis_stroke_point_count;
    analysis
}

#[derive(Clone, Copy)]
enum DiagnosisLevel {
    Waiting,
    Running,
    Normal,
    Warning,
    MouseOnly,
    Insufficient,
}

struct DiagnosisView {
    level: DiagnosisLevel,
    title: &'static str,
    summary: String,
}

fn diagnosis_view(state: &WindowState) -> DiagnosisView {
    if state.mode == DiagnosticMode::Idle {
        return DiagnosisView {
            level: DiagnosisLevel::Waiting,
            title: tr("conclusion.waiting_title"),
            summary: tr("conclusion.waiting_summary").to_string(),
        };
    }
    if state.mode == DiagnosticMode::Running {
        return DiagnosisView {
            level: DiagnosisLevel::Running,
            title: tr("conclusion.running_title"),
            summary: tr("conclusion.running_summary").to_string(),
        };
    }

    let pen_contact_events = state.stats.down + state.stats.movement + state.stats.up;
    if pen_contact_events == 0 {
        if state.mouse_contact_events != 0 || state.filtered_promoted_mouse_contact_events != 0 {
            return DiagnosisView {
                level: DiagnosisLevel::MouseOnly,
                title: tr("conclusion.mouse_only_title"),
                summary: tr("conclusion.mouse_only_summary").to_string(),
            };
        }
        return DiagnosisView {
            level: DiagnosisLevel::Insufficient,
            title: tr("conclusion.data_insufficient_title"),
            summary: tr("conclusion.data_insufficient_summary").to_string(),
        };
    }

    let mut issues = Vec::new();
    if state.stats.down == 0 || state.stats.movement == 0 || state.stats.up == 0 {
        issues.push(tr("conclusion.warning_incomplete_sequence"));
    }
    if state.stats.errors != 0 {
        issues.push(tr("conclusion.warning_pointer_error"));
    }
    if state.sampling_analysis.interval_p95_ms > 20.0 || state.sampling_analysis.over_33_3ms != 0 {
        issues.push(tr("conclusion.warning_sampling_gap"));
    }
    let compatibility_note = if state.promoted_mouse_events != 0 {
        tr("conclusion.compatibility_note")
    } else {
        ""
    };
    let pressure_note = if state.stats.pressure_min.is_none() {
        tr("conclusion.pressure_not_reported")
    } else {
        ""
    };
    if issues.is_empty() {
        DiagnosisView {
            level: DiagnosisLevel::Normal,
            title: tr("conclusion.normal_title"),
            summary: format!(
                "{}{}{}",
                tr("conclusion.normal_summary"),
                compatibility_note,
                pressure_note,
            ),
        }
    } else {
        DiagnosisView {
            level: DiagnosisLevel::Warning,
            title: tr("conclusion.warning_title"),
            summary: format!(
                "{}{}{}",
                issues.join(tr("punctuation.issue_separator")),
                compatibility_note,
                pressure_note,
            ),
        }
    }
}

fn diagnosis_color(level: DiagnosisLevel) -> COLORREF {
    match level {
        DiagnosisLevel::Waiting | DiagnosisLevel::Insufficient => COLORREF(0x00a0_9288),
        DiagnosisLevel::Running => COLORREF(0x00d9_7d2e),
        DiagnosisLevel::Normal => COLORREF(0x0066_8e3c),
        DiagnosisLevel::Warning => COLORREF(0x0046_aae6),
        DiagnosisLevel::MouseOnly => COLORREF(0x0041_37dc),
    }
}

fn sampling_report(state: &WindowState) -> String {
    let analysis = analyze_sampling(state);
    let pressure = match (state.stats.pressure_min, state.stats.pressure_max) {
        (Some(minimum), Some(maximum)) => format!("{minimum}..{maximum}"),
        _ => "none".to_string(),
    };
    let tilt = match (state.stats.tilt_x_range, state.stats.tilt_y_range) {
        (Some((x_min, x_max)), Some((y_min, y_max))) => {
            format!("x={x_min}..{x_max},y={y_min}..{y_max}")
        }
        _ => "none".to_string(),
    };
    let rotation = state.stats.rotation_range.map_or_else(
        || "none".to_string(),
        |(minimum, maximum)| format!("{minimum}..{maximum}"),
    );
    format!(
        "PT_PEN: down={}, move={}, up={}, hover={}, cancel={}, errors={} | pressure={}, tilt={}, rotation={} | samples={}, median={:.1}ms, p95={:.1}ms, p99={:.1}ms, max={:.1}ms, stddev={:.1}ms, >16.7ms={}, >20ms={}, >33.3ms={}, turn median={:.1}°, turn p95={:.1}° | touch={}, mouse={}, mouse-contact={}, pen-promoted={}, filtered-pen-mouse={}, filtered-pen-contact={}, touch-promoted={}, data-truncated={}",
        state.stats.down,
        state.stats.movement,
        state.stats.up,
        state.stats.hover,
        state.stats.cancel,
        state.stats.errors,
        pressure,
        tilt,
        rotation,
        analysis.point_count,
        analysis.interval_median_ms,
        analysis.interval_p95_ms,
        analysis.interval_p99_ms,
        analysis.interval_max_ms,
        analysis.interval_stddev_ms,
        analysis.over_16_7ms,
        analysis.over_20ms,
        analysis.over_33_3ms,
        analysis.turn_median_degrees,
        analysis.turn_p95_degrees,
        state.touch_events,
        state.mouse_events,
        state.mouse_contact_events,
        state.promoted_mouse_events,
        state.filtered_promoted_mouse_events,
        state.filtered_promoted_mouse_contact_events,
        state.promoted_touch_events,
        state
            .recording
            .as_ref()
            .is_some_and(|recording| recording.truncation_written)
            || state.recording_truncated
            || state.stylus_data.truncated,
    )
}

fn copy_text(hwnd: HWND, text: &str) -> bool {
    let content = wide(text);
    let bytes = content.len() * size_of::<u16>();
    let Ok(memory) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }) else {
        return false;
    };
    let target = unsafe { GlobalLock(memory) } as *mut u16;
    if target.is_null() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), target, content.len());
        let _ = GlobalUnlock(memory);
    }
    if unsafe { OpenClipboard(Some(hwnd)) }.is_err() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return false;
    }
    let success = unsafe { EmptyClipboard() }.is_ok()
        && unsafe { SetClipboardData(CF_UNICODETEXT_FORMAT, Some(HANDLE(memory.0))) }.is_ok();
    let _ = unsafe { CloseClipboard() };
    if !success {
        let _ = unsafe { GlobalFree(Some(memory)) };
    }
    success
}

fn select_data_path(hwnd: HWND) -> Option<PathBuf> {
    let recording_directory = recording_directory();
    let _ = std::fs::create_dir_all(&recording_directory);
    let mut path = vec![0u16; 32_768];
    let filter = wide(tr("dialog.import_filter"));
    let title = wide(tr("dialog.import_title"));
    let default_extension = wide("dat");
    let initial_directory = wide(&recording_directory.to_string_lossy());
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(path.as_mut_ptr()),
        nMaxFile: path.len() as u32,
        lpstrInitialDir: PCWSTR(initial_directory.as_ptr()),
        lpstrTitle: PCWSTR(title.as_ptr()),
        lpstrDefExt: PCWSTR(default_extension.as_ptr()),
        Flags: OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST,
        ..Default::default()
    };
    let selected = unsafe { GetOpenFileNameW(&mut dialog) }.as_bool();
    if !selected {
        return None;
    }
    let length = path
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(path.len());
    Some(PathBuf::from(String::from_utf16_lossy(&path[..length])))
}

fn update_record_button(state: &WindowState) {
    let label = wide(if state.mode == DiagnosticMode::Running {
        tr("button.stop_test")
    } else {
        tr("button.start_test")
    });
    let _ = unsafe { SetWindowTextW(state.record_button, PCWSTR(label.as_ptr())) };
    let enabled = state.mode != DiagnosticMode::Running;
    for control in [
        state.clear_button,
        state.import_button,
        state.filter_checkbox,
    ] {
        let _ = unsafe { EnableWindow(control, enabled) };
    }
}

fn set_control_text(control: HWND, key: &'static str) {
    let label = wide(tr(key));
    let _ = unsafe { SetWindowTextW(control, PCWSTR(label.as_ptr())) };
}

fn refresh_language_combo(state: &WindowState) {
    let _ = unsafe { SendMessageW(state.language_combo, CB_RESETCONTENT, None, None) };
    let automatic = wide(tr("language.auto"));
    let _ = unsafe {
        SendMessageW(
            state.language_combo,
            CB_ADDSTRING,
            None,
            Some(LPARAM(automatic.as_ptr() as isize)),
        )
    };
    for language in SUPPORTED_LANGUAGES {
        let label = wide(language.label);
        let _ = unsafe {
            SendMessageW(
                state.language_combo,
                CB_ADDSTRING,
                None,
                Some(LPARAM(label.as_ptr() as isize)),
            )
        };
    }
    let selected = LANGUAGE_MODE
        .load(Ordering::Relaxed)
        .min(SUPPORTED_LANGUAGES.len() as u8) as usize;
    let _ = unsafe {
        SendMessageW(
            state.language_combo,
            CB_SETCURSEL,
            Some(WPARAM(selected)),
            None,
        )
    };
}

fn refresh_translated_ui(hwnd: HWND, state: &mut WindowState) {
    apply_window_title(hwnd);
    set_control_text(state.clear_button, "button.clear_canvas");
    set_control_text(state.filter_checkbox, "checkbox.filter_promoted_mouse");
    set_control_text(state.open_recording_folder_button, "button.open_recordings");
    set_control_text(state.import_button, "button.import_data");
    set_control_text(state.copy_report_button, "button.copy_report");
    set_control_text(state.open_log_button, "button.open_log");
    set_control_text(state.details_checkbox, "checkbox.show_details");
    update_record_button(state);
    refresh_language_combo(state);
    layout_controls(hwnd, state);
}

fn create_ui_font(dpi: u32, size: i32, weight: i32) -> Option<UiFont> {
    let font = unsafe {
        CreateFontW(
            -scale_for_dpi(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        )
    };
    if font.is_invalid() {
        None
    } else {
        Some(UiFont(font))
    }
}

fn update_ui_font(state: &mut WindowState) {
    let old_font = create_ui_font(state.dpi, 13, FW_NORMAL.0 as i32)
        .and_then(|font| state.ui_font.replace(font));
    let raw_font = state.ui_font.as_ref().map_or_else(
        || unsafe { GetStockObject(DEFAULT_GUI_FONT).0 as usize },
        |font| font.0.0 as usize,
    );
    for control in [
        state.clear_button,
        state.filter_checkbox,
        state.record_button,
        state.open_recording_folder_button,
        state.import_button,
        state.copy_report_button,
        state.open_log_button,
        state.details_checkbox,
        state.language_combo,
    ] {
        let _ =
            unsafe { SendMessageW(control, WM_SETFONT, Some(WPARAM(raw_font)), Some(LPARAM(1))) };
    }
    drop(old_font);
}

fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi.max(BASE_DPI) as i64 + (BASE_DPI / 2) as i64) / BASE_DPI as i64) as i32
}

fn measured_control_width(
    hwnd: HWND,
    state: &WindowState,
    text: &str,
    minimum: i32,
    maximum: i32,
) -> i32 {
    let dc = unsafe { GetDC(Some(hwnd)) };
    if dc.is_invalid() {
        return scale_for_dpi(minimum, state.dpi);
    }
    let font = state.ui_font.as_ref().map_or_else(
        || unsafe { GetStockObject(DEFAULT_GUI_FONT) },
        |font| HGDIOBJ(font.0.0),
    );
    let old_font = unsafe { SelectObject(dc, font) };
    let mut text = text.encode_utf16().collect::<Vec<_>>();
    let mut size = SIZE::default();
    let measured = unsafe { GetTextExtentPoint32W(dc, &text, &mut size) }.as_bool();
    let _ = unsafe { SelectObject(dc, old_font) };
    let _ = unsafe { ReleaseDC(Some(hwnd), dc) };
    text.clear();
    let width = if measured {
        size.cx + scale_for_dpi(28, state.dpi)
    } else {
        scale_for_dpi(minimum, state.dpi)
    };
    width.clamp(
        scale_for_dpi(minimum, state.dpi),
        scale_for_dpi(maximum, state.dpi),
    )
}

fn layout_controls(hwnd: HWND, state: &mut WindowState) {
    let margin = scale_for_dpi(16, state.dpi);
    let language_width = std::iter::once(tr("language.auto"))
        .chain(SUPPORTED_LANGUAGES.iter().map(|language| language.label))
        .map(|label| measured_control_width(hwnd, state, label, 180, 280))
        .max()
        .unwrap_or_else(|| scale_for_dpi(180, state.dpi));
    state.language_combo_width = language_width;
    let button_height = scale_for_dpi(30, state.dpi);
    let button_gap = scale_for_dpi(8, state.dpi);
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let action_controls = [
        (state.clear_button, "button.clear_canvas"),
        (state.copy_report_button, "button.copy_report"),
        (state.import_button, "button.import_data"),
        (state.open_log_button, "button.open_log"),
    ];
    let mut action_widths =
        action_controls.map(|(_, key)| measured_control_width(hwnd, state, tr(key), 96, 180));
    let available_width = (client.right - margin * 2).max(0);
    let mut action_total = action_widths.iter().sum::<i32>() + button_gap * 3;
    if action_total > available_width {
        action_widths.fill(scale_for_dpi(96, state.dpi));
        action_total = action_widths.iter().sum::<i32>() + button_gap * 3;
    }
    let first_button_x = (client.right - margin - action_total).max(margin);
    let button_y = scale_for_dpi(42, state.dpi);
    let _ = unsafe {
        MoveWindow(
            state.language_combo,
            client.right - margin - language_width,
            scale_for_dpi(6, state.dpi),
            language_width,
            scale_for_dpi(240, state.dpi),
            true,
        )
    };
    let filter_width =
        measured_control_width(hwnd, state, tr("checkbox.filter_promoted_mouse"), 250, 340);
    let record_width = [tr("button.start_test"), tr("button.stop_test")]
        .into_iter()
        .map(|label| measured_control_width(hwnd, state, label, 96, 150))
        .max()
        .unwrap_or_else(|| scale_for_dpi(96, state.dpi));
    let recording_directory_width =
        measured_control_width(hwnd, state, tr("button.open_recordings"), 112, 190);
    let record_x = margin + filter_width + button_gap;
    let recording_directory_x = record_x + record_width + button_gap;
    let left_controls_right = recording_directory_x + recording_directory_width;
    let action_button_y = if first_button_x < left_controls_right + button_gap {
        state.canvas_top = scale_for_dpi(BASE_CANVAS_TOP + 36, state.dpi);
        scale_for_dpi(78, state.dpi)
    } else {
        state.canvas_top = scale_for_dpi(BASE_CANVAS_TOP, state.dpi);
        button_y
    };
    let mut action_x = first_button_x;
    for ((control, _), width) in action_controls.into_iter().zip(action_widths) {
        let _ = unsafe {
            MoveWindow(
                control,
                action_x,
                action_button_y,
                width,
                button_height,
                true,
            )
        };
        action_x += width + button_gap;
    }
    let _ = unsafe {
        MoveWindow(
            state.filter_checkbox,
            margin,
            button_y,
            filter_width,
            button_height,
            true,
        )
    };
    let _ = unsafe {
        MoveWindow(
            state.record_button,
            record_x,
            button_y,
            record_width,
            button_height,
            true,
        )
    };
    let _ = unsafe {
        MoveWindow(
            state.open_recording_folder_button,
            recording_directory_x,
            button_y,
            recording_directory_width,
            button_height,
            true,
        )
    };
    let canvas = canvas_rect(state, client);
    let details_width = measured_control_width(hwnd, state, tr("checkbox.show_details"), 180, 280);
    let _ = unsafe {
        MoveWindow(
            state.details_checkbox,
            margin,
            canvas.bottom + scale_for_dpi(204, state.dpi),
            details_width.min(available_width),
            button_height,
            true,
        )
    };
}

fn scale_trace_points(state: &mut WindowState, old_dpi: u32, new_dpi: u32) {
    if old_dpi == 0 || old_dpi == new_dpi {
        return;
    }
    for point in state
        .pen_trace
        .iter_mut()
        .chain(state.touch_trace.iter_mut())
        .chain(state.mouse_trace.iter_mut())
        .chain(state.analysis_trace.iter_mut())
    {
        point.x = ((point.x as i64 * new_dpi as i64) / old_dpi as i64) as i32;
        point.y = ((point.y as i64 * new_dpi as i64) / old_dpi as i64) as i32;
    }
    if let Some(cache) = state.canvas_cache.as_mut() {
        for point in cache
            .pen_last
            .values_mut()
            .chain(cache.touch_last.values_mut())
            .chain(cache.mouse_last.values_mut())
        {
            point.x = ((point.x as i64 * new_dpi as i64) / old_dpi as i64) as i32;
            point.y = ((point.y as i64 * new_dpi as i64) / old_dpi as i64) as i32;
        }
    }
    state.canvas_cache_invalid = true;
    state.canvas_cache_scale_pending = true;
}

fn open_path(hwnd: HWND, path: &Path) -> Result<(), String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            Some(hwnd),
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        Err(tr("error.open_recording_directory").to_string())
    } else {
        Ok(())
    }
}

fn open_recording_directory(hwnd: HWND) -> Result<(), String> {
    let directory = recording_directory();
    std::fs::create_dir_all(&directory)
        .map_err(|_| tr("error.create_recording_directory").to_string())?;
    open_path(hwnd, &directory)
}

fn reset_detection_state(state: &mut WindowState, mode: DiagnosticMode) {
    state.pen_trace.clear();
    state.touch_trace.clear();
    state.mouse_trace.clear();
    state.analysis_trace.clear();
    state.analysis_stroke_point_count = 0;
    state.last_event = None;
    state.stylus_data = StylusData::default();
    state.mode = mode;
    state.stats = PenStats::default();
    state.mouse_events = 0;
    state.mouse_contact_events = 0;
    state.promoted_mouse_events = 0;
    state.filtered_promoted_mouse_events = 0;
    state.filtered_promoted_mouse_contact_events = 0;
    state.promoted_touch_events = 0;
    state.touch_events = 0;
    state.touch_contacts.clear();
    state.canvas_cache_invalid = true;
    state.canvas_cache_scale_pending = false;
    state.pen_in_contact = false;
    state.pen_trace_break_pending = false;
    state.mouse_contact = false;
    state.last_pointer_id = 0;
    state.last_pressure = 0;
    state.last_pressure_available = false;
    state.last_tilt_x = 0;
    state.last_tilt_y = 0;
    state.last_tilt_available = false;
    state.last_rotation = 0;
    state.last_rotation_available = false;
    state.sampling_analysis = SamplingAnalysis::default();
    state.sampling_analysis_dirty = false;
    state.recording_truncated = false;
}

fn start_recording(state: &mut WindowState) -> Result<(), String> {
    let directory = recording_directory();
    std::fs::create_dir_all(&directory)
        .map_err(|_| tr("error.create_recording_directory").to_string())?;
    let path = timestamped_path(&directory, "stylus", "dat");
    let file = File::create(&path).map_err(|_| tr("error.create_recording_file").to_string())?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    data::write_header(&mut writer).map_err(|_| tr("error.write_recording_header").to_string())?;
    writer
        .flush()
        .map_err(|_| tr("error.flush_recording_header").to_string())?;
    state.recording = Some(Recording {
        writer,
        path: path.clone(),
        origin_performance_count: 0,
        origin_pointer_time: 0,
        last_timestamp_us: 0,
        pending_samples: 0,
        sample_count: 0,
        last_sample: None,
        truncation_written: false,
    });
    state.recording_segment_open = false;
    reset_detection_state(state, DiagnosticMode::Running);
    state.last_report = tr("status.test_started").to_string();
    write_run_log(state, &format!("RECORD | started | {}", path.display()));
    update_record_button(state);
    Ok(())
}

fn stop_recording(state: &mut WindowState) -> Result<(), String> {
    let cancellation_result = if state.pen_in_contact {
        record_cancel_sample(state)
    } else {
        Ok(())
    };
    let (result, path, sample_count) = if let Some(mut recording) = state.recording.take() {
        let path = recording.path.clone();
        let sample_count = recording.sample_count;
        let flush_result = recording
            .writer
            .flush()
            .map_err(|_| tr("error.finalize_recording").to_string());
        (
            cancellation_result.and(flush_result),
            Some(path),
            sample_count,
        )
    } else {
        (Ok(()), None, 0)
    };
    state.recording_segment_open = false;
    state.pen_in_contact = false;
    state.pen_trace_break_pending = false;
    state.mouse_contact = false;
    state.touch_contacts.clear();
    state.mode = DiagnosticMode::Completed;
    if let Some(path) = path {
        state.last_report = if result.is_ok() {
            tr("status.recording_saved").to_string()
        } else {
            tr("status.recording_finalize_failed").to_string()
        };
        write_run_log(
            state,
            &format!(
                "RECORD | stopped | samples={} | success={}",
                sample_count,
                result.is_ok()
            ),
        );
        write_run_log(state, &format!("RECORD | path={}", path.display()));
    }
    update_record_button(state);
    result
}

fn performance_frequency() -> u64 {
    static FREQUENCY: OnceLock<u64> = OnceLock::new();
    *FREQUENCY.get_or_init(|| {
        let mut frequency = 0i64;
        if unsafe { QueryPerformanceFrequency(&mut frequency) }.is_ok() && frequency > 0 {
            frequency as u64
        } else {
            0
        }
    })
}

fn recording_timestamp(recording: &mut Recording, pen_info: &POINTER_PEN_INFO) -> u64 {
    if recording.origin_performance_count == 0 {
        recording.origin_performance_count = pen_info.pointerInfo.PerformanceCount;
        recording.origin_pointer_time = pen_info.pointerInfo.dwTime;
    }
    let frequency = performance_frequency();
    let timestamp = if frequency != 0
        && pen_info.pointerInfo.PerformanceCount >= recording.origin_performance_count
    {
        let elapsed = pen_info.pointerInfo.PerformanceCount - recording.origin_performance_count;
        ((elapsed as u128 * 1_000_000u128) / frequency as u128) as u64
    } else {
        pen_info
            .pointerInfo
            .dwTime
            .wrapping_sub(recording.origin_pointer_time) as u64
            * 1_000
    };
    recording.last_timestamp_us = recording.last_timestamp_us.max(timestamp);
    recording.last_timestamp_us
}

fn pointer_timestamp_us(pen_info: &POINTER_PEN_INFO) -> u64 {
    let frequency = performance_frequency();
    if frequency != 0 {
        ((pen_info.pointerInfo.PerformanceCount as u128 * 1_000_000u128) / frequency as u128) as u64
    } else {
        pen_info.pointerInfo.dwTime as u64 * 1_000
    }
}

fn combined_tilt_degrees(tilt_x: i32, tilt_y: i32) -> i32 {
    let tangent = (tilt_x as f64)
        .to_radians()
        .tan()
        .hypot((tilt_y as f64).to_radians().tan());
    tangent.atan().to_degrees().round().clamp(0.0, 90.0) as i32
}

fn mark_recording_truncated(state: &mut WindowState) -> Result<(), String> {
    state.recording_truncated = true;
    let newly_written = if let Some(recording) = state.recording.as_mut() {
        if recording.truncation_written {
            false
        } else {
            writeln!(recording.writer, "# truncated=true")
                .map_err(|_| tr("error.write_truncation_marker").to_string())?;
            recording
                .writer
                .flush()
                .map_err(|_| tr("error.flush_truncation_marker").to_string())?;
            recording.truncation_written = true;
            true
        }
    } else {
        false
    };
    if newly_written {
        state.last_report = tr("status.recording_truncated").to_string();
        write_run_log(state, "DATA | sample limit reached | truncated=true");
    }
    Ok(())
}

fn append_recording_cancel(state: &mut WindowState) -> Result<(), String> {
    if !state.recording_segment_open || state.recording.is_none() {
        return Ok(());
    }
    if state
        .recording
        .as_ref()
        .is_some_and(|recording| recording.sample_count >= MAX_SAMPLES)
    {
        state.recording_segment_open = false;
        return mark_recording_truncated(state);
    }
    let Some(mut sample) = state
        .recording
        .as_ref()
        .and_then(|recording| recording.last_sample)
    else {
        state.recording_segment_open = false;
        return Ok(());
    };
    sample.event_type = EVENT_CANCEL;
    sample.pressure = 0.0;
    if let Some(recording) = state.recording.as_mut() {
        sample.timestamp_us = recording.last_timestamp_us;
        data::write_sample(&mut recording.writer, &sample)
            .map_err(|_| tr("error.write_cancel_event").to_string())?;
        recording
            .writer
            .flush()
            .map_err(|_| tr("error.flush_recording_data").to_string())?;
        recording.pending_samples = 0;
        recording.sample_count = recording.sample_count.saturating_add(1);
        recording.last_sample = Some(sample);
    }
    state.recording_segment_open = false;
    Ok(())
}

fn record_data_sample(
    state: &mut WindowState,
    message: u32,
    pen_info: &POINTER_PEN_INFO,
    point: POINT,
    canvas: RECT,
) -> Result<(), String> {
    if state.recording.is_none() {
        return Ok(());
    }
    if state
        .recording
        .as_ref()
        .is_some_and(|recording| recording.sample_count >= MAX_SAMPLES)
    {
        return mark_recording_truncated(state);
    }
    let contact = pen_info
        .pointerInfo
        .pointerFlags
        .contains(POINTER_FLAG_INCONTACT);
    let event_type = match message {
        WM_POINTERDOWN => EVENT_DOWN,
        WM_POINTERUP => EVENT_UP,
        WM_POINTERUPDATE if contact => EVENT_MOVE,
        WM_POINTERUPDATE => EVENT_HOVER,
        _ => return Ok(()),
    };
    let width = (canvas.right - canvas.left).max(1) as f64;
    let height = (canvas.bottom - canvas.top).max(1) as f64;
    let x = (point.x - canvas.left).max(0) as f64 / width;
    let y = (point.y - canvas.top).max(0) as f64 / height;
    let pressure = if pen_info.penMask & PEN_MASK_PRESSURE != 0 {
        (pen_info.pressure as f64 / 1024.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rotation = if pen_info.penMask & PEN_MASK_ROTATION != 0 {
        pen_info.rotation
    } else {
        UNKNOWN_ROTATION
    };
    let tilt = if rotation != UNKNOWN_ROTATION
        && pen_info.penMask & PEN_MASK_TILT_X != 0
        && pen_info.penMask & PEN_MASK_TILT_Y != 0
    {
        combined_tilt_degrees(pen_info.tiltX, pen_info.tiltY)
    } else {
        UNKNOWN_TILT
    };
    let recording = state.recording.as_mut().expect("recording was checked");
    let sample = StylusSample {
        timestamp_us: recording_timestamp(recording, pen_info),
        event_type,
        x: x.clamp(0.0, 1.0),
        y: y.clamp(0.0, 1.0),
        pressure,
        rotation,
        tilt,
    };
    data::write_sample(&mut recording.writer, &sample)
        .map_err(|_| tr("error.write_recording_data").to_string())?;
    recording.sample_count = recording.sample_count.saturating_add(1);
    recording.last_sample = Some(sample);
    recording.pending_samples += 1;
    if recording.pending_samples >= RECORDING_CHECKPOINT_SAMPLES || event_type == 2 {
        recording
            .writer
            .flush()
            .map_err(|_| tr("error.flush_recording_data").to_string())?;
        recording.pending_samples = 0;
    }
    state.recording_segment_open = matches!(event_type, EVENT_DOWN | EVENT_MOVE);
    Ok(())
}

fn record_cancel_sample(state: &mut WindowState) -> Result<(), String> {
    if !state.pen_in_contact {
        return Ok(());
    }
    state.pen_in_contact = false;
    state.pen_trace_break_pending = false;
    state.stats.cancel = state.stats.cancel.saturating_add(1);
    append_recording_cancel(state)
}

unsafe fn record_pen_info(
    hwnd: HWND,
    message: u32,
    pen_info: &POINTER_PEN_INFO,
    state: &mut WindowState,
    canvas: RECT,
) -> Option<String> {
    let mut point = pen_info.pointerInfo.ptPixelLocation;
    if !unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
        return None;
    }
    let contact = pen_info
        .pointerInfo
        .pointerFlags
        .contains(POINTER_FLAG_INCONTACT);
    if state.mode != DiagnosticMode::Running {
        return None;
    }
    let pressure_available = pen_info.penMask & PEN_MASK_PRESSURE != 0;
    let tilt_available =
        pen_info.penMask & PEN_MASK_TILT_X != 0 && pen_info.penMask & PEN_MASK_TILT_Y != 0;
    let rotation_available = pen_info.penMask & PEN_MASK_ROTATION != 0;
    let previous_pressure = state.last_pressure;
    state.last_pressure_available = pressure_available;
    if pressure_available {
        state.last_pressure = pen_info.pressure;
    }
    state.last_tilt_available = tilt_available;
    if tilt_available {
        state.last_tilt_x = pen_info.tiltX;
        state.last_tilt_y = pen_info.tiltY;
    }
    state.last_rotation_available = rotation_available;
    if rotation_available {
        state.last_rotation = pen_info.rotation;
    }
    let mut recording_error = None;
    if !point_in_rect(canvas, point) {
        if contact && state.pen_in_contact {
            if !state.pen_trace_break_pending
                && let Err(error) = append_recording_cancel(state)
            {
                let _ = stop_recording(state);
                state.pen_trace_break_pending = true;
                recording_error = Some(error);
            }
            state.pen_trace_break_pending = true;
        }
        if message == WM_POINTERUP && state.pen_in_contact {
            let _ = record_cancel_sample(state);
        }
        if message == WM_POINTERDOWN
            || message == WM_POINTERUP
            || (message == WM_POINTERUPDATE && !contact)
            || (message == WM_POINTERLEAVE && !contact)
        {
            state.pen_in_contact = false;
            state.pen_trace_break_pending = false;
        }
        return recording_error;
    }
    if let Err(error) = record_data_sample(state, message, pen_info, point, canvas) {
        let _ = stop_recording(state);
        recording_error = Some(error);
    }
    let was_contact = state.pen_in_contact;
    let starts_contact =
        message == WM_POINTERDOWN || (message == WM_POINTERUPDATE && contact && !was_contact);
    let draws = starts_contact
        || (message == WM_POINTERUPDATE && contact)
        || (message == WM_POINTERUP && was_contact);
    match message {
        WM_POINTERDOWN => state.stats.down = state.stats.down.saturating_add(1),
        WM_POINTERUPDATE if contact => {
            state.stats.movement = state.stats.movement.saturating_add(1)
        }
        WM_POINTERUP => state.stats.up = state.stats.up.saturating_add(1),
        _ => state.stats.hover = state.stats.hover.saturating_add(1),
    }
    if draws {
        state.last_pointer_id = pen_info.pointerInfo.pointerId;
        let break_before = starts_contact || state.pen_trace_break_pending;
        state.push_pen(
            TracePoint {
                pointer_id: pen_info.pointerInfo.pointerId,
                x: point.x,
                y: point.y,
                pressure: if !pressure_available {
                    0
                } else if pen_info.pressure == 0 {
                    previous_pressure.max(1)
                } else {
                    pen_info.pressure
                },
                timestamp_us: pointer_timestamp_us(pen_info),
                break_before,
            },
            PenAttributes {
                pressure_available,
                tilt_x: pen_info.tiltX,
                tilt_y: pen_info.tiltY,
                tilt_available,
                rotation: pen_info.rotation,
                rotation_available,
            },
        );
        state.last_event = Some(RecentInputEvent {
            kind: match message {
                WM_POINTERDOWN => RecentEventKind::PenDown,
                WM_POINTERUP => RecentEventKind::PenUp,
                _ => RecentEventKind::PenMove,
            },
            pointer_id: pen_info.pointerInfo.pointerId,
            x: point.x,
            y: point.y,
        });
        state.pen_trace_break_pending = false;
    }
    state.pen_in_contact = match message {
        WM_POINTERDOWN | WM_POINTERUPDATE => contact || starts_contact,
        WM_POINTERUP => {
            state.pen_trace_break_pending = false;
            false
        }
        _ => state.pen_in_contact,
    };
    recording_error
}

unsafe fn record_pen(hwnd: HWND, message: u32, wparam: WPARAM) {
    if unsafe { window_state(hwnd) }.is_none_or(|state| state.mode != DiagnosticMode::Running) {
        return;
    }
    let id = pointer_id(wparam);
    let mut pointer_type = Default::default();
    if unsafe { GetPointerType(id, &mut pointer_type) }.is_err() || pointer_type != PT_PEN {
        return;
    }

    let mut pen_info = POINTER_PEN_INFO::default();
    if unsafe { GetPointerPenInfo(id, &mut pen_info) }.is_err() {
        let mut cancellation_error = None;
        if let Some(mut state) = unsafe { window_state(hwnd) } {
            state.stats.errors = state.stats.errors.saturating_add(1);
            if message == WM_POINTERUP && state.pen_in_contact {
                if let Err(error) = record_cancel_sample(&mut state) {
                    let _ = stop_recording(&mut state);
                    cancellation_error = Some(error);
                }
                state.pen_in_contact = false;
                state.pen_trace_break_pending = false;
            }
        }
        if let Some(error) = cancellation_error {
            show_message(hwnd, &error, true);
        }
        return;
    }
    let mut history = Vec::new();
    let mut history_failed = false;
    if message == WM_POINTERUPDATE && pen_info.pointerInfo.historyCount > 1 {
        let mut count = pen_info.pointerInfo.historyCount.min(MAX_POINTER_HISTORY);
        history.resize(count as usize, POINTER_PEN_INFO::default());
        if unsafe { GetPointerPenInfoHistory(id, &mut count, Some(history.as_mut_ptr())) }.is_ok() {
            history.truncate(count as usize);
        } else {
            history.clear();
            history_failed = true;
        }
    }

    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    let mut error = None;
    if let Some(mut state) = unsafe { window_state(hwnd) } {
        if history_failed {
            state.stats.errors = state.stats.errors.saturating_add(1);
        }
        let canvas = canvas_rect(&state, client);
        let samples = if history.is_empty() {
            std::slice::from_ref(&pen_info)
        } else {
            history.as_slice()
        };
        for sample in samples.iter().rev() {
            if let Some(sample_error) =
                unsafe { record_pen_info(hwnd, message, sample, &mut state, canvas) }
                && error.is_none()
            {
                error = Some(sample_error);
            }
        }
        schedule_canvas_repaint(hwnd, &mut state);
    }
    if let Some(error) = error {
        show_message(hwnd, &error, true);
    }
}

fn touch_timestamp_us(touch_info: &POINTER_TOUCH_INFO) -> u64 {
    let frequency = performance_frequency();
    if frequency != 0 {
        ((touch_info.pointerInfo.PerformanceCount as u128 * 1_000_000u128) / frequency as u128)
            as u64
    } else {
        touch_info.pointerInfo.dwTime as u64 * 1_000
    }
}

unsafe fn record_touch_info(
    hwnd: HWND,
    message: u32,
    touch_info: &POINTER_TOUCH_INFO,
    state: &mut WindowState,
    canvas: RECT,
) {
    let mut point = touch_info.pointerInfo.ptPixelLocation;
    if !unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
        return;
    }
    let id = touch_info.pointerInfo.pointerId;
    let contact = touch_info
        .pointerInfo
        .pointerFlags
        .contains(POINTER_FLAG_INCONTACT);
    if state.mode != DiagnosticMode::Running {
        return;
    }
    let was_contact = state.touch_contacts.contains(&id);
    if !point_in_rect(canvas, point) {
        state.touch_contacts.remove(&id);
        return;
    }
    let starts_contact =
        message == WM_POINTERDOWN || (message == WM_POINTERUPDATE && contact && !was_contact);
    let draws = starts_contact
        || (message == WM_POINTERUPDATE && contact)
        || (message == WM_POINTERUP && was_contact);
    if draws {
        state.push_touch(TracePoint {
            pointer_id: id,
            x: point.x,
            y: point.y,
            pressure: 256,
            timestamp_us: touch_timestamp_us(touch_info),
            break_before: starts_contact,
        });
        state.last_event = Some(RecentInputEvent {
            kind: match message {
                WM_POINTERDOWN => RecentEventKind::TouchDown,
                WM_POINTERUP => RecentEventKind::TouchUp,
                _ => RecentEventKind::TouchMove,
            },
            pointer_id: id,
            x: point.x,
            y: point.y,
        });
    }
    if message == WM_POINTERUP || !contact {
        state.touch_contacts.remove(&id);
    } else if contact || starts_contact {
        state.touch_contacts.insert(id);
    }
}

unsafe fn record_touch(hwnd: HWND, message: u32, wparam: WPARAM) {
    let id = pointer_id(wparam);
    let mut pointer_type = Default::default();
    if unsafe { GetPointerType(id, &mut pointer_type) }.is_err() || pointer_type != PT_TOUCH {
        return;
    }
    let mut touch_info = POINTER_TOUCH_INFO::default();
    if unsafe { GetPointerTouchInfo(id, &mut touch_info) }.is_err() {
        return;
    }
    let mut history = Vec::new();
    if message == WM_POINTERUPDATE && touch_info.pointerInfo.historyCount > 1 {
        let mut count = touch_info.pointerInfo.historyCount.min(MAX_POINTER_HISTORY);
        history.resize(count as usize, POINTER_TOUCH_INFO::default());
        if unsafe { GetPointerTouchInfoHistory(id, &mut count, Some(history.as_mut_ptr())) }.is_ok()
        {
            history.truncate(count as usize);
        } else {
            history.clear();
        }
    }

    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    if let Some(mut state) = unsafe { window_state(hwnd) } {
        let canvas = canvas_rect(&state, client);
        let samples = if history.is_empty() {
            std::slice::from_ref(&touch_info)
        } else {
            history.as_slice()
        };
        for sample in samples.iter().rev() {
            unsafe { record_touch_info(hwnd, message, sample, &mut state, canvas) };
        }
        schedule_canvas_repaint(hwnd, &mut state);
    }
}

fn mouse_point(lparam: LPARAM) -> (i32, i32) {
    let raw = lparam.0 as u32;
    (
        (raw as u16 as i16) as i32,
        ((raw >> 16) as u16 as i16) as i32,
    )
}

fn promoted_mouse_kind(extra_info: usize) -> PromotedMouseKind {
    let value = extra_info as u32;
    if (value & MOUSE_POINTER_SIGNATURE_MASK) != MOUSE_POINTER_SIGNATURE {
        PromotedMouseKind::None
    } else if (value & MOUSE_POINTER_TOUCH_FLAG) != 0 {
        PromotedMouseKind::Touch
    } else {
        PromotedMouseKind::Pen
    }
}

fn should_filter_promoted_mouse(kind: PromotedMouseKind, enabled: bool) -> bool {
    enabled && kind == PromotedMouseKind::Pen
}

fn mouse_contact_transition(message: u32, wparam: WPARAM, was_contact: bool) -> (bool, bool) {
    match message {
        WM_LBUTTONDOWN => (true, true),
        WM_LBUTTONUP => (was_contact, false),
        WM_MOUSEMOVE => {
            let pressed = (wparam.0 & MOUSE_LEFT_BUTTON) != 0;
            (pressed, pressed)
        }
        _ => (false, was_contact),
    }
}

unsafe fn record_mouse(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    let (x, y) = mouse_point(lparam);
    let promoted = promoted_mouse_kind(unsafe { GetMessageExtraInfo() }.0 as usize);
    if let Some(mut state) = unsafe { window_state(hwnd) } {
        if state.mode != DiagnosticMode::Running || !point_in_canvas(hwnd, &state, POINT { x, y }) {
            state.mouse_contact = false;
            return;
        }
        let was_contact = state.mouse_contact;
        let (contact, next_contact) = mouse_contact_transition(message, wparam, was_contact);
        state.mouse_contact = next_contact;
        if should_filter_promoted_mouse(promoted, state.filter_promoted_mouse) {
            state.count_filtered_pen_mouse(contact);
            state.mouse_contact = false;
            schedule_canvas_repaint(hwnd, &mut state);
            return;
        }
        if contact {
            state.mouse_contact_events = state.mouse_contact_events.saturating_add(1);
            state.last_event = Some(RecentInputEvent {
                kind: match message {
                    WM_LBUTTONDOWN => RecentEventKind::MouseDown,
                    WM_LBUTTONUP => RecentEventKind::MouseUp,
                    _ => RecentEventKind::MouseMove,
                },
                pointer_id: 0,
                x,
                y,
            });
        }
        if contact {
            state.push_mouse(
                TracePoint {
                    pointer_id: 0,
                    x,
                    y,
                    pressure: 256,
                    timestamp_us: 0,
                    break_before: contact && !was_contact,
                },
                promoted,
            );
        } else {
            state.count_mouse_event(promoted);
        }
        schedule_canvas_repaint(hwnd, &mut state);
    }
}

fn show_message(hwnd: HWND, message: &str, error: bool) {
    let message = wide(message);
    let title = wide(tr("window.title"));
    let _ = unsafe {
        MessageBoxW(
            Some(hwnd),
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK
                | if error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        )
    };
}

fn load_data_into_canvas(hwnd: HWND, state: &mut WindowState, imported: StylusData) {
    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    let canvas = canvas_rect(state, client);
    let width = (canvas.right - canvas.left).max(1) as f64;
    let height = (canvas.bottom - canvas.top).max(1) as f64;
    state.pen_trace.clear();
    state.touch_trace.clear();
    state.mouse_trace.clear();
    state.analysis_trace.clear();
    state.analysis_stroke_point_count = 0;
    state.last_event = None;
    state.stats = PenStats::default();
    state.mouse_events = 0;
    state.mouse_contact_events = 0;
    state.promoted_mouse_events = 0;
    state.filtered_promoted_mouse_events = 0;
    state.filtered_promoted_mouse_contact_events = 0;
    state.promoted_touch_events = 0;
    state.touch_events = 0;
    state.touch_contacts.clear();
    state.canvas_cache_invalid = true;
    state.canvas_cache_scale_pending = false;
    state.pen_in_contact = false;
    state.pen_trace_break_pending = false;
    state.sampling_analysis = SamplingAnalysis::default();
    state.sampling_analysis_dirty = true;
    state.mouse_contact = false;

    for sample in &imported.samples {
        let point = TracePoint {
            pointer_id: 1,
            x: canvas.left + (sample.x * width).round() as i32,
            y: canvas.top + (sample.y * height).round() as i32,
            pressure: (sample.pressure * 1024.0).round().clamp(0.0, 1024.0) as u32,
            timestamp_us: sample.timestamp_us,
            break_before: !state.pen_in_contact || sample.event_type == EVENT_DOWN,
        };
        let attributes = PenAttributes {
            pressure_available: sample.pressure > 0.0,
            tilt_x: if sample.tilt == UNKNOWN_TILT {
                0
            } else {
                sample.tilt
            },
            tilt_y: 0,
            tilt_available: sample.tilt != UNKNOWN_TILT,
            rotation: if sample.rotation == UNKNOWN_ROTATION {
                0
            } else {
                sample.rotation
            },
            rotation_available: sample.rotation != UNKNOWN_ROTATION,
        };
        match sample.event_type {
            EVENT_DOWN => {
                state.stats.down = state.stats.down.saturating_add(1);
                state.pen_in_contact = true;
                state.push_pen(point, attributes);
            }
            EVENT_MOVE | EVENT_BUTTON_ONLY if state.pen_in_contact => {
                state.stats.movement = state.stats.movement.saturating_add(1);
                state.push_pen(point, attributes);
            }
            EVENT_MOVE | EVENT_BUTTON_ONLY => {
                state.stats.movement = state.stats.movement.saturating_add(1);
                state.pen_in_contact = true;
                state.push_pen(point, attributes);
            }
            EVENT_UP if state.pen_in_contact => {
                state.stats.up = state.stats.up.saturating_add(1);
                state.push_pen(point, attributes);
                state.pen_in_contact = false;
            }
            EVENT_CANCEL | EVENT_CANCEL_ALL => {
                state.stats.cancel = state.stats.cancel.saturating_add(1);
                state.pen_in_contact = false;
            }
            EVENT_HOVER | EVENT_HOVER_LEAVE => {
                state.stats.hover = state.stats.hover.saturating_add(1)
            }
            _ => {}
        }
        if let Some(kind) = match sample.event_type {
            EVENT_DOWN => Some(RecentEventKind::PenDown),
            EVENT_MOVE | EVENT_BUTTON_ONLY => Some(RecentEventKind::PenMove),
            EVENT_UP => Some(RecentEventKind::PenUp),
            _ => None,
        } {
            state.last_event = Some(RecentInputEvent {
                kind,
                pointer_id: 1,
                x: point.x,
                y: point.y,
            });
            state.last_pointer_id = 1;
        }
        state.last_pressure = point.pressure;
        state.last_pressure_available = sample.pressure > 0.0;
        state.last_tilt_available = sample.tilt != UNKNOWN_TILT;
        if state.last_tilt_available {
            state.last_tilt_x = sample.tilt;
            state.last_tilt_y = 0;
        }
        state.last_rotation_available = sample.rotation != UNKNOWN_ROTATION;
        if state.last_rotation_available {
            state.last_rotation = sample.rotation;
        }
    }
    state.pen_in_contact = false;
    state.stylus_data = imported;
    state.recording_truncated = state.stylus_data.truncated;
    state.mode = DiagnosticMode::Imported;
    state.last_report = format!(
        "{} {} {}",
        tr("status.imported_prefix"),
        state.stylus_data.samples.len(),
        tr("status.imported_suffix"),
    );
    write_run_log(
        state,
        &format!("IMPORT | samples={}", state.stylus_data.samples.len()),
    );
}

fn import_data(hwnd: HWND, state: &mut WindowState) {
    if state.mode == DiagnosticMode::Running {
        show_message(hwnd, tr("status.import_blocked_while_running"), false);
        return;
    }
    let Some(path) = select_data_path(hwnd) else {
        return;
    };
    match data::load(&path) {
        Ok(imported) => load_data_into_canvas(hwnd, state, imported),
        Err(error) => {
            write_run_log(state, &format!("IMPORT | failed | {error}"));
            show_message(hwnd, tr("error.import_data"), true);
        }
    }
}

unsafe fn draw_sampling_graph(
    dc: windows::Win32::Graphics::Gdi::HDC,
    bounds: RECT,
    analysis: &SamplingAnalysis,
    dpi: u32,
    axis_pen: HPEN,
    threshold_pen: HPEN,
    sample_pen: HPEN,
) {
    let background = unsafe { CreateSolidBrush(COLORREF(0x00ff_fcfa)) };
    if !background.is_invalid() {
        let _ = unsafe { FillRect(dc, &bounds, background) };
        let _ = unsafe { DeleteObject(HGDIOBJ(background.0)) };
    }
    let _ = unsafe { FrameRect(dc, &bounds, HBRUSH(GetStockObject(GRAY_BRUSH).0)) };
    let inset = scale_for_dpi(6, dpi);
    let title = if analysis.recent_intervals_ms.is_empty() {
        tr("graph.waiting").to_string()
    } else {
        format!(
            "{} {} | median {:.1} ms | P95 {:.1} ms | max {:.1} ms | >20 ms {}",
            tr("graph.recent_samples"),
            analysis.recent_intervals_ms.len(),
            analysis.interval_median_ms,
            analysis.interval_p95_ms,
            analysis.interval_max_ms,
            analysis.over_20ms,
        )
    };
    let mut title_wide: Vec<u16> = title.encode_utf16().collect();
    let mut title_rect = RECT {
        left: bounds.left + inset,
        top: bounds.top + inset,
        right: bounds.right - inset,
        bottom: bounds.top + scale_for_dpi(22, dpi),
    };
    let _ = unsafe {
        DrawTextW(
            dc,
            &mut title_wide,
            &mut title_rect,
            DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
        )
    };
    if analysis.recent_intervals_ms.is_empty() {
        return;
    }

    let graph = RECT {
        left: bounds.left + scale_for_dpi(38, dpi),
        top: bounds.top + scale_for_dpi(25, dpi),
        right: bounds.right - inset,
        bottom: bounds.bottom - scale_for_dpi(7, dpi),
    };
    let graph_max = (analysis.interval_max_ms * 1.15).ceil().max(24.0);
    let threshold_y = graph.bottom
        - ((20.0 / graph_max).clamp(0.0, 1.0) * (graph.bottom - graph.top) as f64).round() as i32;
    let original = unsafe { SelectObject(dc, HGDIOBJ(axis_pen.0)) };
    let _ = unsafe { MoveToEx(dc, graph.left, graph.bottom, None) };
    let _ = unsafe { LineTo(dc, graph.right, graph.bottom) };
    let _ = unsafe { MoveToEx(dc, graph.left, graph.top, None) };
    let _ = unsafe { LineTo(dc, graph.left, graph.bottom) };
    let _ = unsafe { SelectObject(dc, HGDIOBJ(threshold_pen.0)) };
    let _ = unsafe { MoveToEx(dc, graph.left, threshold_y, None) };
    let _ = unsafe { LineTo(dc, graph.right, threshold_y) };

    let mut maximum_label = format!("{graph_max:.0} ms")
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut maximum_rect = RECT {
        left: bounds.left + inset,
        top: graph.top - scale_for_dpi(3, dpi),
        right: graph.left - inset,
        bottom: graph.top + scale_for_dpi(16, dpi),
    };
    let _ = unsafe {
        DrawTextW(
            dc,
            &mut maximum_label,
            &mut maximum_rect,
            DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
        )
    };
    let mut threshold_label = "20 ms".encode_utf16().collect::<Vec<_>>();
    let mut threshold_rect = RECT {
        left: bounds.left + inset,
        top: threshold_y - scale_for_dpi(8, dpi),
        right: graph.left - inset,
        bottom: threshold_y + scale_for_dpi(8, dpi),
    };
    let _ = unsafe {
        DrawTextW(
            dc,
            &mut threshold_label,
            &mut threshold_rect,
            DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
        )
    };
    let _ = unsafe { SelectObject(dc, HGDIOBJ(sample_pen.0)) };
    for (index, interval) in analysis.recent_intervals_ms.iter().enumerate() {
        let x = if analysis.recent_intervals_ms.len() == 1 {
            graph.left
        } else {
            graph.left
                + index as i32 * (graph.right - graph.left)
                    / (analysis.recent_intervals_ms.len() - 1) as i32
        };
        let y = graph.bottom
            - ((interval / graph_max).clamp(0.0, 1.0) * (graph.bottom - graph.top) as f64).round()
                as i32;
        if index == 0 {
            let _ = unsafe { MoveToEx(dc, x, y, None) };
        } else {
            let _ = unsafe { LineTo(dc, x, y) };
        }
    }
    let _ = unsafe { SelectObject(dc, original) };
}

unsafe fn draw_trace(
    dc: HDC,
    trace: &VecDeque<TracePoint>,
    pens: &[HPEN],
    pressure_sensitive: bool,
) {
    let mut previous = HashMap::<u32, TracePoint>::with_capacity(8);
    unsafe { draw_trace_incremental(dc, trace, 0, &mut previous, pens, pressure_sensitive) };
}

unsafe fn draw_trace_incremental(
    dc: HDC,
    trace: &VecDeque<TracePoint>,
    start: usize,
    previous: &mut HashMap<u32, TracePoint>,
    pens: &[HPEN],
    pressure_sensitive: bool,
) {
    if pens.is_empty() || pens[0].is_invalid() {
        return;
    }
    let original = unsafe { SelectObject(dc, HGDIOBJ(pens[0].0)) };
    let mut selected_index = 0usize;
    for point in trace.iter().skip(start) {
        if let Some(from) = previous.get(&point.pointer_id).copied()
            && !point.break_before
        {
            let pen_index = if pressure_sensitive {
                (((point.pressure.max(1) * pens.len() as u32) / 1024).clamp(1, pens.len() as u32)
                    - 1) as usize
            } else {
                0
            };
            if pen_index != selected_index && !pens[pen_index].is_invalid() {
                let _ = unsafe { SelectObject(dc, HGDIOBJ(pens[pen_index].0)) };
                selected_index = pen_index;
            }
            let _ = unsafe { MoveToEx(dc, from.x, from.y, None) };
            let _ = unsafe { LineTo(dc, point.x, point.y) };
        }
        previous.insert(point.pointer_id, *point);
    }
    let _ = unsafe { SelectObject(dc, original) };
}

unsafe fn create_canvas_cache(compatible_dc: HDC, width: i32, height: i32) -> Option<CanvasCache> {
    let dc = unsafe { CreateCompatibleDC(Some(compatible_dc)) };
    let bitmap = unsafe { CreateCompatibleBitmap(compatible_dc, width, height) };
    if dc.is_invalid() || bitmap.is_invalid() {
        if !dc.is_invalid() {
            let _ = unsafe { DeleteDC(dc) };
        }
        if !bitmap.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        }
        return None;
    }
    let original_bitmap = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
    if original_bitmap.is_invalid() {
        let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        let _ = unsafe { DeleteDC(dc) };
        return None;
    }
    let bounds = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    let white = unsafe { GetStockObject(WHITE_BRUSH) };
    let _ = unsafe { FillRect(dc, &bounds, HBRUSH(white.0)) };
    Some(CanvasCache {
        dc,
        bitmap,
        original_bitmap,
        width,
        height,
        pen_last: HashMap::with_capacity(2),
        touch_last: HashMap::with_capacity(8),
        mouse_last: HashMap::with_capacity(2),
    })
}

unsafe fn create_frame_cache(compatible_dc: HDC, width: i32, height: i32) -> Option<FrameCache> {
    let dc = unsafe { CreateCompatibleDC(Some(compatible_dc)) };
    let bitmap = unsafe { CreateCompatibleBitmap(compatible_dc, width, height) };
    if dc.is_invalid() || bitmap.is_invalid() {
        if !dc.is_invalid() {
            let _ = unsafe { DeleteDC(dc) };
        }
        if !bitmap.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        }
        return None;
    }
    let original_bitmap = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
    if original_bitmap.is_invalid() {
        let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        let _ = unsafe { DeleteDC(dc) };
        return None;
    }
    Some(FrameCache {
        dc,
        bitmap,
        original_bitmap,
        width,
        height,
    })
}

unsafe fn update_canvas_cache(
    state: &mut WindowState,
    compatible_dc: HDC,
    client: RECT,
    canvas: RECT,
    mouse_pen: HPEN,
    touch_pen: HPEN,
    pressure_pens: &[HPEN],
) -> Option<HDC> {
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    let scale_existing = state.canvas_cache_scale_pending;
    let reset_existing = state.canvas_cache_invalid && !scale_existing;
    let must_grow = state
        .canvas_cache
        .as_ref()
        .is_some_and(|cache| cache.width < width || cache.height < height);
    let needs_rebuild =
        reset_existing || scale_existing || state.canvas_cache.is_none() || must_grow;

    if needs_rebuild {
        let mut old_cache = state.canvas_cache.take();
        let target_width = if reset_existing || scale_existing {
            width
        } else {
            old_cache
                .as_ref()
                .map_or(width, |cache| cache.width.max(width))
        };
        let target_height = if reset_existing || scale_existing {
            height
        } else {
            old_cache
                .as_ref()
                .map_or(height, |cache| cache.height.max(height))
        };
        let Some(mut new_cache) =
            (unsafe { create_canvas_cache(compatible_dc, target_width, target_height) })
        else {
            if !reset_existing {
                state.canvas_cache = old_cache;
            }
            return None;
        };

        if !reset_existing && let Some(mut old_cache) = old_cache.take() {
            if scale_existing {
                let _ = unsafe { SetStretchBltMode(new_cache.dc, HALFTONE) };
                let _ = unsafe {
                    StretchBlt(
                        new_cache.dc,
                        0,
                        0,
                        target_width,
                        target_height,
                        Some(old_cache.dc),
                        0,
                        0,
                        old_cache.width,
                        old_cache.height,
                        SRCCOPY,
                    )
                };
                new_cache.pen_last = std::mem::take(&mut old_cache.pen_last);
                new_cache.touch_last = std::mem::take(&mut old_cache.touch_last);
                new_cache.mouse_last = std::mem::take(&mut old_cache.mouse_last);
            } else {
                let _ = unsafe {
                    BitBlt(
                        new_cache.dc,
                        0,
                        0,
                        old_cache.width,
                        old_cache.height,
                        Some(old_cache.dc),
                        0,
                        0,
                        SRCCOPY,
                    )
                };
                new_cache.pen_last = std::mem::take(&mut old_cache.pen_last);
                new_cache.touch_last = std::mem::take(&mut old_cache.touch_last);
                new_cache.mouse_last = std::mem::take(&mut old_cache.mouse_last);
            }
        }

        state.canvas_cache = Some(new_cache);
        state.canvas_cache_invalid = false;
        state.canvas_cache_scale_pending = false;
    }

    let mut cache = state.canvas_cache.take()?;
    let saved = unsafe { SaveDC(cache.dc) };
    let _ = unsafe {
        IntersectClipRect(
            cache.dc,
            canvas.left,
            canvas.top,
            canvas.right,
            canvas.bottom,
        )
    };
    unsafe {
        draw_trace_incremental(
            cache.dc,
            &state.mouse_trace,
            0,
            &mut cache.mouse_last,
            std::slice::from_ref(&mouse_pen),
            false,
        );
        draw_trace_incremental(
            cache.dc,
            &state.touch_trace,
            0,
            &mut cache.touch_last,
            std::slice::from_ref(&touch_pen),
            false,
        );
        draw_trace_incremental(
            cache.dc,
            &state.pen_trace,
            0,
            &mut cache.pen_last,
            pressure_pens,
            true,
        );
    }
    if saved != 0 {
        let _ = unsafe { RestoreDC(cache.dc, saved) };
    }
    state.mouse_trace.clear();
    state.touch_trace.clear();
    state.pen_trace.clear();
    let dc = cache.dc;
    state.canvas_cache = Some(cache);
    Some(dc)
}

unsafe fn paint(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    if let Some(mut state) = unsafe { window_state(hwnd) } {
        state.canvas_dirty = false;
        if state.sampling_analysis_dirty {
            state.sampling_analysis = analyze_sampling(&state);
            state.sampling_analysis_dirty = false;
        }
        if state
            .drawing_pens
            .as_ref()
            .is_none_or(|pens| pens.dpi != state.dpi)
        {
            state.drawing_pens = DrawingPens::new(state.dpi);
        }
        let Some(drawing_pens) = state.drawing_pens.take() else {
            state.canvas_dirty = true;
            drop(state);
            let _ = unsafe { EndPaint(hwnd, &paint) };
            return;
        };
        let canvas = canvas_rect(&state, client);
        let cached_dc = unsafe {
            update_canvas_cache(
                &mut state,
                dc,
                client,
                canvas,
                drawing_pens.mouse,
                drawing_pens.touch,
                &drawing_pens.pressure,
            )
        };
        if state
            .frame_cache
            .as_ref()
            .is_none_or(|cache| cache.width != width || cache.height != height)
        {
            state.frame_cache = unsafe { create_frame_cache(dc, width, height) };
        }
        let frame_cache = state.frame_cache.take();
        let target_dc = frame_cache.as_ref().map_or(dc, |cache| cache.dc);
        let background = unsafe { CreateSolidBrush(COLORREF(0x00fa_f7f5)) };
        let white = unsafe { GetStockObject(WHITE_BRUSH) };
        if !background.is_invalid() {
            let _ = unsafe { FillRect(target_dc, &client, background) };
            let _ = unsafe { DeleteObject(HGDIOBJ(background.0)) };
        } else {
            let _ = unsafe { FillRect(target_dc, &client, HBRUSH(white.0)) };
        }
        if let Some(cached_dc) = cached_dc {
            let _ = unsafe {
                BitBlt(
                    target_dc,
                    canvas.left,
                    canvas.top,
                    (canvas.right - canvas.left).max(1),
                    (canvas.bottom - canvas.top).max(1),
                    Some(cached_dc),
                    canvas.left,
                    canvas.top,
                    SRCCOPY,
                )
            };
        } else {
            let _ = unsafe { FillRect(target_dc, &canvas, HBRUSH(white.0)) };
            let saved = unsafe { SaveDC(target_dc) };
            let _ = unsafe {
                IntersectClipRect(
                    target_dc,
                    canvas.left,
                    canvas.top,
                    canvas.right,
                    canvas.bottom,
                )
            };
            unsafe {
                draw_trace(
                    target_dc,
                    &state.mouse_trace,
                    std::slice::from_ref(&drawing_pens.mouse),
                    false,
                );
                draw_trace(
                    target_dc,
                    &state.touch_trace,
                    std::slice::from_ref(&drawing_pens.touch),
                    false,
                );
                draw_trace(target_dc, &state.pen_trace, &drawing_pens.pressure, true);
            }
            if saved != 0 {
                let _ = unsafe { RestoreDC(target_dc, saved) };
            }
        }
        let _ = unsafe { FrameRect(target_dc, &canvas, HBRUSH(GetStockObject(GRAY_BRUSH).0)) };
        let _ = unsafe { SetBkMode(target_dc, TRANSPARENT) };
        let body_font = state.ui_font.as_ref().map_or_else(
            || unsafe { GetStockObject(DEFAULT_GUI_FONT) },
            |font| HGDIOBJ(font.0.0),
        );
        let old_font = unsafe { SelectObject(target_dc, body_font) };
        let _ = unsafe { SetTextColor(target_dc, COLORREF(0x0066_5c55)) };

        let margin = scale_for_dpi(16, state.dpi);
        let instruction = if state.mode == DiagnosticMode::Running {
            tr("instruction.running")
        } else if state.mode == DiagnosticMode::Imported {
            tr("instruction.imported")
        } else if state.mode == DiagnosticMode::Completed {
            tr("instruction.completed")
        } else {
            tr("instruction.idle")
        };
        let mut instruction_text = instruction.encode_utf16().collect::<Vec<_>>();
        let mut instruction_rect = RECT {
            left: margin,
            top: scale_for_dpi(10, state.dpi),
            right: client.right - margin - state.language_combo_width - scale_for_dpi(8, state.dpi),
            bottom: scale_for_dpi(34, state.dpi),
        };
        let _ = unsafe {
            DrawTextW(
                target_dc,
                &mut instruction_text,
                &mut instruction_rect,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
            )
        };

        let status_background_rect = RECT {
            left: margin,
            top: state.canvas_top - scale_for_dpi(36, state.dpi),
            right: client.right - margin,
            bottom: state.canvas_top - scale_for_dpi(4, state.dpi),
        };
        let status_background = unsafe { CreateSolidBrush(COLORREF(0x00fc_f4ec)) };
        if !status_background.is_invalid() {
            let _ = unsafe { FillRect(target_dc, &status_background_rect, status_background) };
            let _ = unsafe { DeleteObject(HGDIOBJ(status_background.0)) };
        }
        let _ = unsafe {
            FrameRect(
                target_dc,
                &status_background_rect,
                HBRUSH(GetStockObject(GRAY_BRUSH).0),
            )
        };
        let _ = unsafe { SetTextColor(target_dc, COLORREF(0x002d_241e)) };

        let pressure_text = if state.last_pressure_available {
            format!(
                "{}/1024 ({:.1}%)",
                state.last_pressure,
                state.last_pressure as f64 * 100.0 / 1024.0
            )
        } else {
            tr("metric.not_reported").to_string()
        };
        let tilt_text = if state.last_tilt_available {
            format!("({}, {})", state.last_tilt_x, state.last_tilt_y)
        } else {
            tr("metric.not_reported").to_string()
        };
        let rotation_text = if state.last_rotation_available {
            format!("{}°", state.last_rotation)
        } else {
            tr("metric.not_reported").to_string()
        };
        let diagnosis = diagnosis_view(&state);
        let status = format!("● {} — {}", diagnosis.title, diagnosis.summary);
        let mut status_text = status.encode_utf16().collect::<Vec<_>>();
        let mut status_rect = RECT {
            left: margin,
            top: state.canvas_top - scale_for_dpi(32, state.dpi),
            right: client.right - margin,
            bottom: state.canvas_top - scale_for_dpi(8, state.dpi),
        };
        let _ = unsafe { SetTextColor(target_dc, diagnosis_color(diagnosis.level)) };
        let _ = unsafe {
            DrawTextW(
                target_dc,
                &mut status_text,
                &mut status_rect,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
            )
        };

        if state.last_event.is_none()
            && matches!(state.mode, DiagnosticMode::Idle | DiagnosticMode::Running)
        {
            let prompt = if state.mode == DiagnosticMode::Running {
                tr("prompt.running")
            } else {
                tr("prompt.idle")
            };
            let mut prompt_text = prompt.encode_utf16().collect::<Vec<_>>();
            let mut prompt_rect = RECT {
                left: canvas.left + scale_for_dpi(40, state.dpi),
                top: canvas.top + (canvas.bottom - canvas.top) / 2 - scale_for_dpi(34, state.dpi),
                right: canvas.right - scale_for_dpi(40, state.dpi),
                bottom: canvas.top
                    + (canvas.bottom - canvas.top) / 2
                    + scale_for_dpi(34, state.dpi),
            };
            let _ = unsafe { SetTextColor(target_dc, COLORREF(0x00b0_a49a)) };
            let _ = unsafe {
                DrawTextW(
                    target_dc,
                    &mut prompt_text,
                    &mut prompt_rect,
                    DT_CENTER | DT_WORDBREAK | DT_VCENTER,
                )
            };
        }

        if let Some(event) = state.last_event {
            let card_rect = RECT {
                left: (canvas.right - scale_for_dpi(226, state.dpi)).max(canvas.left + 8),
                top: canvas.top + scale_for_dpi(12, state.dpi),
                right: canvas.right - scale_for_dpi(12, state.dpi),
                bottom: canvas.top + scale_for_dpi(158, state.dpi),
            };
            let card_background = unsafe { CreateSolidBrush(COLORREF(0x00ff_fcfa)) };
            if !card_background.is_invalid() {
                let _ = unsafe { FillRect(target_dc, &card_rect, card_background) };
                let _ = unsafe { DeleteObject(HGDIOBJ(card_background.0)) };
            }
            let _ =
                unsafe { FrameRect(target_dc, &card_rect, HBRUSH(GetStockObject(GRAY_BRUSH).0)) };
            let sampling_rate = if state.sampling_analysis.interval_median_ms > 0.0 {
                format!(
                    "{:.0} Hz",
                    1000.0 / state.sampling_analysis.interval_median_ms
                )
            } else {
                "-".to_string()
            };
            let recent_interval = state
                .sampling_analysis
                .recent_intervals_ms
                .last()
                .map_or_else(|| "-".to_string(), |interval| format!("{interval:.1} ms"));
            let pen_event = matches!(
                event.kind,
                RecentEventKind::PenDown | RecentEventKind::PenMove | RecentEventKind::PenUp
            );
            let pointer_pressure = if pen_event {
                pressure_text.as_str()
            } else {
                "-"
            };
            let card = format!(
                "Pointer  {}\nID       {}\nPressure {}\nTilt     {}\nRotation {}\n{}  {}\n{}  {}",
                recent_event_pointer_type(Some(event)),
                event.pointer_id,
                pointer_pressure,
                if pen_event { tilt_text.as_str() } else { "-" },
                if pen_event {
                    rotation_text.as_str()
                } else {
                    "-"
                },
                tr("metric.rate"),
                sampling_rate,
                tr("metric.latest_gap"),
                recent_interval,
            );
            let mut card_text = card.encode_utf16().collect::<Vec<_>>();
            let mut card_text_rect = RECT {
                left: card_rect.left + scale_for_dpi(10, state.dpi),
                top: card_rect.top + scale_for_dpi(8, state.dpi),
                right: card_rect.right - scale_for_dpi(8, state.dpi),
                bottom: card_rect.bottom - scale_for_dpi(6, state.dpi),
            };
            let _ = unsafe { SetTextColor(target_dc, COLORREF(0x002d_241e)) };
            let _ = unsafe {
                DrawTextW(
                    target_dc,
                    &mut card_text,
                    &mut card_text_rect,
                    DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
                )
            };
        }

        let graph_bounds = RECT {
            left: margin,
            top: canvas.bottom + scale_for_dpi(8, state.dpi),
            right: client.right - margin,
            bottom: canvas.bottom + scale_for_dpi(90, state.dpi),
        };
        unsafe {
            draw_sampling_graph(
                target_dc,
                graph_bounds,
                &state.sampling_analysis,
                state.dpi,
                drawing_pens.graph_axis,
                drawing_pens.graph_threshold,
                drawing_pens.graph_sample,
            )
        };

        let sampling_rate = if state.sampling_analysis.interval_median_ms > 0.0 {
            format!(
                "{:.0} Hz",
                1000.0 / state.sampling_analysis.interval_median_ms
            )
        } else {
            "-".to_string()
        };
        let compatibility_count = if state.filter_promoted_mouse {
            format!(
                "{} {}",
                tr("metric.filtered_pen_mouse"),
                state.filtered_promoted_mouse_events
            )
        } else {
            format!("PEN→MOUSE {}", state.promoted_mouse_events)
        };
        let counts = format!(
            "PT_PEN {}    PT_TOUCH {}    MOUSE {}    {}",
            state.stats.down + state.stats.movement + state.stats.up,
            state.touch_events,
            state.mouse_contact_events,
            compatibility_count,
        );
        let timing = format!(
            "{} {}    Median {:.1} ms    P95 {:.1} ms    P99 {:.1} ms    Max {:.1} ms",
            tr("metric.rate"),
            sampling_rate,
            state.sampling_analysis.interval_median_ms,
            state.sampling_analysis.interval_p95_ms,
            state.sampling_analysis.interval_p99_ms,
            state.sampling_analysis.interval_max_ms,
        );
        let thresholds = format!(
            ">16.7 ms {}    >20 ms {}    >33.3 ms {}    StdDev {:.1} ms",
            state.sampling_analysis.over_16_7ms,
            state.sampling_analysis.over_20ms,
            state.sampling_analysis.over_33_3ms,
            state.sampling_analysis.interval_stddev_ms,
        );
        for (index, line) in [counts, timing, thresholds].into_iter().enumerate() {
            let mut text = line.encode_utf16().collect::<Vec<_>>();
            let top = canvas.bottom + scale_for_dpi(98 + index as i32 * 28, state.dpi);
            let mut rect = RECT {
                left: margin,
                top,
                right: client.right - margin,
                bottom: top + scale_for_dpi(24, state.dpi),
            };
            let _ = unsafe { SetTextColor(target_dc, COLORREF(0x002d_241e)) };
            let _ = unsafe {
                DrawTextW(
                    target_dc,
                    &mut text,
                    &mut rect,
                    DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
                )
            };
        }

        if state.show_details {
            let details = format!(
                "{}: {}\n{}",
                tr("metric.latest_event"),
                recent_event_text(state.last_event),
                sampling_report(&state),
            );
            let mut details_text = details.encode_utf16().collect::<Vec<_>>();
            let mut details_rect = RECT {
                left: margin,
                top: canvas.bottom + scale_for_dpi(236, state.dpi),
                right: client.right - margin,
                bottom: client.bottom - scale_for_dpi(8, state.dpi),
            };
            let _ = unsafe { SetTextColor(target_dc, COLORREF(0x0066_5c55)) };
            let _ = unsafe {
                DrawTextW(
                    target_dc,
                    &mut details_text,
                    &mut details_rect,
                    DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
                )
            };
        }

        let _ = unsafe { SelectObject(target_dc, old_font) };
        if frame_cache.is_some() {
            let _ = unsafe { BitBlt(dc, 0, 0, width, height, Some(target_dc), 0, 0, SRCCOPY) };
        }
        state.frame_cache = frame_cache;
        state.drawing_pens = Some(drawing_pens);
    }
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

unsafe fn create_button(
    parent: HWND,
    id: usize,
    label: &str,
    checkbox: bool,
) -> windows::core::Result<HWND> {
    let label = wide(label);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR(label.as_ptr()),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(if checkbox {
                    BS_AUTOCHECKBOX as u32
                } else {
                    BS_PUSHBUTTON as u32
                }),
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as *mut c_void)),
            None,
            None,
        )
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| match message {
        WM_NCCREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            if create.is_null() {
                return LRESULT(0);
            }
            let context =
                unsafe { ((*create).lpCreateParams as *mut WindowCreateContext).as_mut() };
            let Some(state) = context.and_then(|context| context.state.take()) else {
                return LRESULT(0);
            };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize) };
            LRESULT(1)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            apply_window_title(hwnd);
            let Ok(clear_button) =
                (unsafe { create_button(hwnd, CLEAR_BUTTON_ID, tr("button.clear_canvas"), false) })
            else {
                return LRESULT(-1);
            };
            let Ok(filter_checkbox) = (unsafe {
                create_button(
                    hwnd,
                    FILTER_MOUSE_CHECKBOX_ID,
                    tr("checkbox.filter_promoted_mouse"),
                    true,
                )
            }) else {
                return LRESULT(-1);
            };
            let Ok(record_button) =
                (unsafe { create_button(hwnd, RECORD_BUTTON_ID, tr("button.start_test"), false) })
            else {
                return LRESULT(-1);
            };
            let _ = unsafe {
                SendMessageW(
                    record_button,
                    BM_SETSTYLE,
                    Some(WPARAM(BS_DEFPUSHBUTTON as usize)),
                    Some(LPARAM(1)),
                )
            };
            let Ok(open_recording_folder_button) = (unsafe {
                create_button(
                    hwnd,
                    OPEN_RECORDING_FOLDER_BUTTON_ID,
                    tr("button.open_recordings"),
                    false,
                )
            }) else {
                return LRESULT(-1);
            };
            let Ok(import_button) =
                (unsafe { create_button(hwnd, IMPORT_BUTTON_ID, tr("button.import_data"), false) })
            else {
                return LRESULT(-1);
            };
            let Ok(copy_report_button) = (unsafe {
                create_button(hwnd, COPY_REPORT_BUTTON_ID, tr("button.copy_report"), false)
            }) else {
                return LRESULT(-1);
            };
            let Ok(open_log_button) =
                (unsafe { create_button(hwnd, OPEN_LOG_BUTTON_ID, tr("button.open_log"), false) })
            else {
                return LRESULT(-1);
            };
            let Ok(details_checkbox) = (unsafe {
                create_button(hwnd, DETAILS_CHECKBOX_ID, tr("checkbox.show_details"), true)
            }) else {
                return LRESULT(-1);
            };
            let language_combo = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("COMBOBOX"),
                    None,
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                            CBS_DROPDOWNLIST as u32,
                        ),
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(HMENU(LANGUAGE_COMBO_ID as *mut c_void)),
                    None,
                    None,
                )
            };
            let Ok(language_combo) = language_combo else {
                return LRESULT(-1);
            };
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                state.clear_button = clear_button;
                state.filter_checkbox = filter_checkbox;
                state.record_button = record_button;
                state.open_recording_folder_button = open_recording_folder_button;
                state.import_button = import_button;
                state.copy_report_button = copy_report_button;
                state.open_log_button = open_log_button;
                state.details_checkbox = details_checkbox;
                state.language_combo = language_combo;
                state.dpi = unsafe { GetDpiForWindow(hwnd) }.max(BASE_DPI);
                initialize_run_log(&mut state);
                layout_controls(hwnd, &mut state);
                update_ui_font(&mut state);
                refresh_translated_ui(hwnd, &mut state);
                if unsafe { SetTimer(Some(hwnd), REPORT_TIMER_ID, REPORT_INTERVAL_MS, None) } == 0 {
                    state.last_report = tr("status.report_timer_failed").to_string();
                    write_run_log(&mut state, "TIMER | report timer failed");
                }
            }
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let info = lparam.0 as *mut MINMAXINFO;
            if let Some(info) = unsafe { info.as_mut() }
                && let Some(state) = unsafe { window_state(hwnd) }
            {
                info.ptMinTrackSize.x = scale_for_dpi(760, state.dpi);
                info.ptMinTrackSize.y = scale_for_dpi(600, state.dpi);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                layout_controls(hwnd, &mut state);
            }
            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            LRESULT(0)
        }
        WM_COMMAND => {
            let command_id = wparam.0 & 0xffff;
            let notification = (wparam.0 >> 16) as u32;
            if command_id == LANGUAGE_COMBO_ID && notification == CBN_SELCHANGE {
                if let Some(mut state) = unsafe { window_state(hwnd) } {
                    let selected =
                        unsafe { SendMessageW(state.language_combo, CB_GETCURSEL, None, None).0 };
                    if (LANGUAGE_AUTO as isize..=SUPPORTED_LANGUAGES.len() as isize)
                        .contains(&selected)
                    {
                        LANGUAGE_MODE.store(selected as u8, Ordering::Relaxed);
                        refresh_translated_ui(hwnd, &mut state);
                    }
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            } else if notification == BN_CLICKED {
                if let Some(mut state) = unsafe { window_state(hwnd) } {
                    match command_id {
                        CLEAR_BUTTON_ID => {
                            reset_detection_state(&mut state, DiagnosticMode::Idle);
                            state.last_report = tr("status.canvas_cleared").to_string();
                            write_run_log(&mut state, "CLEAR");
                        }
                        FILTER_MOUSE_CHECKBOX_ID => {
                            state.filter_promoted_mouse = unsafe {
                                SendMessageW(state.filter_checkbox, BM_GETCHECK, None, None).0
                                    as u32
                            } == BST_CHECKED.0;
                            reset_detection_state(&mut state, DiagnosticMode::Idle);
                            state.last_report = if state.filter_promoted_mouse {
                                tr("status.filter_enabled")
                            } else {
                                tr("status.filter_disabled")
                            }
                            .to_string();
                            let filter_enabled = state.filter_promoted_mouse;
                            write_run_log(
                                &mut state,
                                &format!("FILTER_PROMOTED_MOUSE | enabled={}", filter_enabled),
                            );
                        }
                        DETAILS_CHECKBOX_ID => {
                            state.show_details = unsafe {
                                SendMessageW(state.details_checkbox, BM_GETCHECK, None, None).0
                                    as u32
                            } == BST_CHECKED.0;
                        }
                        RECORD_BUTTON_ID => {
                            let result = if state.mode == DiagnosticMode::Running {
                                stop_recording(&mut state)
                            } else {
                                start_recording(&mut state)
                            };
                            if let Err(error) = result {
                                show_message(hwnd, &error, true);
                            }
                        }
                        OPEN_RECORDING_FOLDER_BUTTON_ID => {
                            if let Err(error) = open_recording_directory(hwnd) {
                                show_message(hwnd, &error, true);
                            }
                        }
                        IMPORT_BUTTON_ID => import_data(hwnd, &mut state),
                        COPY_REPORT_BUTTON_ID => {
                            state.last_report = sampling_report(&state);
                            if !copy_text(hwnd, &state.last_report) {
                                show_message(hwnd, tr("error.copy_report"), true);
                            }
                        }
                        OPEN_LOG_BUTTON_ID => {
                            if state.log_path.as_os_str().is_empty()
                                || open_path(hwnd, &state.log_path).is_err()
                            {
                                show_message(hwnd, tr("error.open_run_log"), true);
                            }
                        }
                        _ => {}
                    }
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        SHOW_WINDOW_MESSAGE => {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
            let _ = unsafe { SetForegroundWindow(hwnd) };
            LRESULT(0)
        }
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERENTER | WM_POINTERLEAVE => {
            unsafe { record_pen(hwnd, message, wparam) };
            unsafe { record_touch(hwnd, message, wparam) };
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_POINTERCAPTURECHANGED => {
            let mut pointer_type = Default::default();
            if unsafe { GetPointerType(pointer_id(wparam), &mut pointer_type) }.is_ok()
                && pointer_type == PT_PEN
            {
                let mut cancellation_error = None;
                if let Some(mut state) = unsafe { window_state(hwnd) } {
                    if pointer_id(wparam) == state.last_pointer_id
                        && let Err(error) = record_cancel_sample(&mut state)
                    {
                        let _ = stop_recording(&mut state);
                        cancellation_error = Some(error);
                    }
                }
                if let Some(error) = cancellation_error {
                    show_message(hwnd, &error, true);
                }
            } else if pointer_type == PT_TOUCH
                && let Some(mut state) = unsafe { window_state(hwnd) }
            {
                state.touch_contacts.remove(&pointer_id(wparam));
                schedule_canvas_repaint(hwnd, &mut state);
            }
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                schedule_canvas_repaint(hwnd, &mut state);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEMOVE => {
            unsafe { record_mouse(hwnd, message, wparam, lparam) };
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_PAINT => {
            unsafe { paint(hwnd) };
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == REPORT_TIMER_ID => {
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                state.last_report = sampling_report(&state);
                let report = state.last_report.clone();
                write_run_log(&mut state, &format!("REPORT | {report}"));
            }
            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == REPAINT_TIMER_ID => {
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                let _ = unsafe { KillTimer(Some(hwnd), REPAINT_TIMER_ID) };
                state.repaint_timer_active = false;
                if state.canvas_dirty {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == CLOSE_RETRY_TIMER_ID => {
            let _ = unsafe { KillTimer(Some(hwnd), CLOSE_RETRY_TIMER_ID) };
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SETCURSOR => {
            let hit_test = (lparam.0 as u32 & 0xffff) as u32;
            if hit_test == HTCLIENT {
                let mut point = POINT::default();
                if unsafe { GetCursorPos(&mut point) }.is_ok()
                    && unsafe { ScreenToClient(hwnd, &mut point) }.as_bool()
                    && let Some(state) = unsafe { window_state(hwnd) }
                {
                    let cursor = unsafe {
                        LoadCursorW(
                            None,
                            if point_in_canvas(hwnd, &state, point) {
                                IDC_CROSS
                            } else {
                                IDC_ARROW
                            },
                        )
                    }
                    .unwrap_or_default();
                    let _ = unsafe { SetCursor(Some(cursor)) };
                    return LRESULT(1);
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_DPICHANGED => {
            if let Some(mut state) = unsafe { window_state(hwnd) } {
                let old_dpi = state.dpi;
                let new_dpi = (wparam.0 as u32 & 0xffff).max(BASE_DPI);
                scale_trace_points(&mut state, old_dpi, new_dpi);
                state.dpi = new_dpi;
                layout_controls(hwnd, &mut state);
                update_ui_font(&mut state);
            }
            let suggested = lparam.0 as *const RECT;
            if !suggested.is_null() {
                let rect = unsafe { *suggested };
                let _ = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let Some(mut state) = (unsafe { window_state(hwnd) }) else {
                // A modal dialog may temporarily hold the state guard while
                // pumping messages. Retry at a bounded interval instead of
                // spinning WM_CLOSE messages in that modal loop.
                let _ = unsafe {
                    SetTimer(
                        Some(hwnd),
                        CLOSE_RETRY_TIMER_ID,
                        CLOSE_RETRY_INTERVAL_MS,
                        None,
                    )
                };
                return LRESULT(0);
            };
            let _ = unsafe { KillTimer(Some(hwnd), CLOSE_RETRY_TIMER_ID) };
            let _ = stop_recording(&mut state);
            write_run_log(&mut state, "STOP | window closed");
            if let Some(writer) = state.log_writer.as_mut() {
                let _ = writer.flush();
            }
            drop(state);
            let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = unsafe { KillTimer(Some(hwnd), REPORT_TIMER_ID) };
            let _ = unsafe { KillTimer(Some(hwnd), REPAINT_TIMER_ID) };
            let _ = unsafe { KillTimer(Some(hwnd), CLOSE_RETRY_TIMER_ID) };
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let pointer =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Mutex<WindowState>;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            if !pointer.is_null() {
                drop(unsafe { Box::from_raw(pointer) });
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }));

    result.unwrap_or_else(|_| {
        log_message(
            NativeToolLogLevel::Error,
            b"Stylus plugin window callback panicked\0",
        );
        let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        LRESULT(0)
    })
}

fn module_instance() -> Option<HINSTANCE> {
    let mut module = HMODULE::default();
    let address = AlkaidLabNativeTool_GetApi as *const () as *const u16;
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(address),
            &mut module,
        )
        .ok()?;
    }
    Some(HINSTANCE(module.0))
}

fn publish_stopped() {
    if let Ok(mut state) = runtime().state.lock() {
        state.hwnd = 0;
        state.starting = false;
        state.running = false;
    }
    runtime().changed.notify_all();
}

fn run_window_thread() {
    let _dpi_guard = ThreadDpiGuard::enter();
    let Some(instance) = module_instance() else {
        log_message(
            NativeToolLogLevel::Error,
            b"Unable to resolve stylus plugin module\0",
        );
        publish_stopped();
        return;
    };

    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
        hIcon: host_window_icon(false),
        hIconSm: host_window_icon(true),
        lpszClassName: w!("AlkaidLabStylusPluginWindow"),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&class) } == 0 {
        log_message(
            NativeToolLogLevel::Error,
            b"Unable to register stylus plugin window\0",
        );
        publish_stopped();
        return;
    }

    let mut create_context = WindowCreateContext {
        state: Some(Box::new(Mutex::new(WindowState::new()))),
    };
    let window_title = wide(tr("window.title"));
    let initial_dpi = unsafe { GetDpiForSystem() }.max(BASE_DPI);
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("AlkaidLabStylusPluginWindow"),
            PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            scale_for_dpi(980, initial_dpi),
            scale_for_dpi(820, initial_dpi),
            None,
            None,
            Some(instance),
            Some((&mut create_context as *mut WindowCreateContext).cast()),
        )
    };
    let Ok(window) = window else {
        let _ = unsafe { UnregisterClassW(w!("AlkaidLabStylusPluginWindow"), Some(instance)) };
        log_message(
            NativeToolLogLevel::Error,
            b"Unable to create stylus plugin window\0",
        );
        publish_stopped();
        return;
    };
    apply_window_title(window);

    let close_immediately = if let Ok(mut state) = runtime().state.lock() {
        state.hwnd = window.0 as isize;
        state.starting = false;
        state.running = true;
        state.stop_requested
    } else {
        true
    };
    runtime().changed.notify_all();
    let _ = unsafe { ShowWindow(window, SW_SHOWNORMAL) };
    if close_immediately {
        let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }

    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let _ = unsafe { UnregisterClassW(w!("AlkaidLabStylusPluginWindow"), Some(instance)) };
    publish_stopped();
}

fn initialize_impl(host: *const NativeToolHostV1) -> NativeToolResult {
    let Some(host) = (unsafe { host.as_ref() }) else {
        return NativeToolResult::InvalidHost;
    };
    if host.struct_size < size_of::<NativeToolHostV1>() as u32
        || host.abi_version != NATIVE_TOOL_ABI_V1
    {
        return NativeToolResult::InvalidHost;
    }
    let Ok(mut state) = runtime().state.lock() else {
        return NativeToolResult::InternalError;
    };
    state.initialized = true;
    state.host = HostServices {
        context: host.context as usize,
        log: host.log,
        default_window_icon: host.default_window_icon,
        default_small_window_icon: host.default_small_window_icon,
    };
    NativeToolResult::Ok
}

fn show_impl() -> NativeToolResult {
    let stale_thread = {
        let Ok(mut state) = runtime().state.lock() else {
            return NativeToolResult::InternalError;
        };
        if !state.initialized {
            return NativeToolResult::InvalidHost;
        }
        if state.running && state.hwnd != 0 {
            let hwnd = HWND(state.hwnd as *mut c_void);
            let _ = unsafe { PostMessageW(Some(hwnd), SHOW_WINDOW_MESSAGE, WPARAM(0), LPARAM(0)) };
            return NativeToolResult::AlreadyOpen;
        }
        if state.starting {
            return NativeToolResult::AlreadyOpen;
        }
        let stale_thread = state.window_thread.take();
        state.starting = true;
        state.stop_requested = false;
        stale_thread
    };
    if let Some(thread) = stale_thread {
        let _ = thread.join();
    }

    let new_thread = match thread::Builder::new()
        .name("alkaidlab-stylus-plugin".to_string())
        .spawn(run_window_thread)
    {
        Ok(thread) => thread,
        Err(_) => {
            publish_stopped();
            return NativeToolResult::StartFailed;
        }
    };

    let Ok(mut state) = runtime().state.lock() else {
        return NativeToolResult::InternalError;
    };
    state.window_thread = Some(new_thread);
    let Ok((state, wait)) = runtime()
        .changed
        .wait_timeout_while(state, START_TIMEOUT, |state| state.starting)
    else {
        return NativeToolResult::InternalError;
    };
    if wait.timed_out() || !state.running {
        return NativeToolResult::StartFailed;
    }
    NativeToolResult::Ok
}

fn request_close_impl() -> NativeToolResult {
    let hwnd = {
        let Ok(mut state) = runtime().state.lock() else {
            return NativeToolResult::InternalError;
        };
        state.stop_requested = true;
        state.hwnd
    };
    if hwnd != 0 {
        let _ = unsafe {
            PostMessageW(
                Some(HWND(hwnd as *mut c_void)),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            )
        };
    }
    NativeToolResult::Ok
}

fn shutdown_impl(timeout_ms: u32) -> NativeToolResult {
    let _ = request_close_impl();
    let Ok(state) = runtime().state.lock() else {
        return NativeToolResult::InternalError;
    };
    let Ok((mut state, wait)) = runtime().changed.wait_timeout_while(
        state,
        Duration::from_millis(timeout_ms as u64),
        |state| state.starting || state.running,
    ) else {
        return NativeToolResult::InternalError;
    };
    if wait.timed_out() {
        return NativeToolResult::StopTimeout;
    }
    let thread = state.window_thread.take();
    state.initialized = false;
    state.host = HostServices {
        context: 0,
        log: None,
        default_window_icon: 0,
        default_small_window_icon: 0,
    };
    drop(state);
    if let Some(thread) = thread {
        let _ = thread.join();
    }
    NativeToolResult::Ok
}

fn can_unload_impl() -> bool {
    runtime()
        .state
        .lock()
        .map(|state| !state.starting && !state.running && state.window_thread.is_none())
        .unwrap_or(false)
}

fn is_running_impl() -> bool {
    runtime()
        .state
        .lock()
        .map(|state| state.starting || state.running)
        .unwrap_or(false)
}

unsafe extern "C" fn initialize(host: *const NativeToolHostV1) -> NativeToolResult {
    catch_unwind(AssertUnwindSafe(|| initialize_impl(host)))
        .unwrap_or(NativeToolResult::InternalError)
}

unsafe extern "C" fn show() -> NativeToolResult {
    catch_unwind(AssertUnwindSafe(show_impl)).unwrap_or(NativeToolResult::InternalError)
}

unsafe extern "C" fn request_close() -> NativeToolResult {
    catch_unwind(AssertUnwindSafe(request_close_impl)).unwrap_or(NativeToolResult::InternalError)
}

unsafe extern "C" fn shutdown(timeout_ms: u32) -> NativeToolResult {
    catch_unwind(AssertUnwindSafe(|| shutdown_impl(timeout_ms)))
        .unwrap_or(NativeToolResult::InternalError)
}

unsafe extern "C" fn can_unload() -> bool {
    catch_unwind(AssertUnwindSafe(can_unload_impl)).unwrap_or(false)
}

unsafe extern "C" fn is_running() -> bool {
    catch_unwind(AssertUnwindSafe(is_running_impl)).unwrap_or(false)
}

fn plugin_api() -> &'static NativeToolPluginV1 {
    static DISPLAY_NAME: OnceLock<Vec<u16>> = OnceLock::new();
    static API: OnceLock<NativeToolPluginV1> = OnceLock::new();
    let display_name = DISPLAY_NAME.get_or_init(|| {
        tr("window.title")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    });
    API.get_or_init(|| NativeToolPluginV1 {
        struct_size: size_of::<NativeToolPluginV1>() as u32,
        abi_version: NATIVE_TOOL_ABI_V1,
        tool_id: TOOL_ID.as_ptr().cast::<c_char>(),
        plugin_version: PLUGIN_VERSION.as_ptr().cast::<c_char>(),
        display_name: display_name.as_ptr(),
        initialize: Some(initialize),
        show: Some(show),
        request_close: Some(request_close),
        shutdown: Some(shutdown),
        is_running: Some(is_running),
        can_unload: Some(can_unload),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn AlkaidLabNativeTool_GetApi(
    host_abi_version: u32,
) -> *const NativeToolPluginV1 {
    if host_abi_version == NATIVE_TOOL_ABI_V1 {
        plugin_api()
    } else {
        null_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translation_keys_in_source(language: &str) -> Vec<String> {
        let source = include_str!("../i18n.json");
        let marker = format!("  \"{language}\": {{");
        let start = source
            .lines()
            .position(|line| line == marker)
            .expect("locale must exist");
        source
            .lines()
            .skip(start + 1)
            .take_while(|line| !line.starts_with("  }"))
            .filter_map(|line| {
                let line = line.trim_start();
                line.strip_prefix('"')
                    .and_then(|line| line.split_once("\":").map(|(key, _)| key.to_string()))
            })
            .collect()
    }

    fn test_recording_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alkaidlab-stylus-plugin-{name}-{}-{}.dat",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn recording_state(path: &Path) -> WindowState {
        let file = File::create(path).unwrap();
        let mut writer = BufWriter::new(file);
        data::write_header(&mut writer).unwrap();
        let sample = StylusSample {
            timestamp_us: 1_000,
            event_type: EVENT_MOVE,
            x: 0.5,
            y: 0.5,
            pressure: 0.5,
            rotation: 0,
            tilt: 0,
        };
        data::write_sample(&mut writer, &sample).unwrap();
        let mut state = WindowState::new();
        state.recording = Some(Recording {
            writer,
            path: path.to_path_buf(),
            origin_performance_count: 0,
            origin_pointer_time: 0,
            last_timestamp_us: sample.timestamp_us,
            pending_samples: 1,
            sample_count: 1,
            last_sample: Some(sample),
            truncation_written: false,
        });
        state.recording_segment_open = true;
        state
    }

    #[test]
    fn exports_only_the_supported_abi() {
        let supported = unsafe { AlkaidLabNativeTool_GetApi(NATIVE_TOOL_ABI_V1) };
        let unsupported = unsafe { AlkaidLabNativeTool_GetApi(NATIVE_TOOL_ABI_V1 + 1) };
        assert!(!supported.is_null());
        assert!(unsupported.is_null());
    }

    #[test]
    fn translation_keys_are_complete_sorted_and_nonempty() {
        let catalog = translations();
        let english = catalog.get("en").unwrap();
        let chinese = catalog.get("zh").unwrap();
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            chinese.keys().collect::<Vec<_>>()
        );
        assert!(english.values().all(|value| !value.trim().is_empty()));
        assert!(chinese.values().all(|value| !value.trim().is_empty()));

        let required_keys = [
            "button.clear_canvas",
            "button.copy_report",
            "button.import_data",
            "button.open_log",
            "button.open_recordings",
            "button.start_test",
            "button.stop_test",
            "checkbox.filter_promoted_mouse",
            "checkbox.show_details",
            "conclusion.waiting_summary",
            "conclusion.waiting_title",
            "graph.recent_samples",
            "graph.waiting",
            "instruction.completed",
            "instruction.idle",
            "instruction.imported",
            "instruction.running",
            "language.auto",
            "metric.filtered_pen_mouse",
            "metric.latest_event",
            "metric.latest_gap",
            "metric.not_reported",
            "metric.rate",
            "prompt.idle",
            "prompt.running",
            "window.title",
        ];
        for language in SUPPORTED_LANGUAGES {
            let locale = catalog
                .get(language.code)
                .or_else(|| language.code.starts_with("en_").then_some(english));
            let locale = locale.unwrap_or_else(|| panic!("{} locale must exist", language.code));
            assert!(
                required_keys.iter().all(|key| locale.contains_key(*key)),
                "{} must translate every primary UI key",
                language.code
            );
            assert!(locale.values().all(|value| !value.trim().is_empty()));
        }

        for language in catalog.keys() {
            let keys = translation_keys_in_source(language);
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys, sorted, "{language} translation keys must be sorted");
            assert!(
                keys.iter().all(|key| english.contains_key(key)),
                "{language} contains an unknown translation key"
            );
        }
    }

    #[test]
    fn automatic_language_matches_supported_windows_locales() {
        assert_eq!(language_code_for_langid(0x0804), "zh");
        assert_eq!(language_code_for_langid(0x0404), "zh_TW");
        assert_eq!(language_code_for_langid(0x0409), "en_US");
        assert_eq!(language_code_for_langid(0x0809), "en_GB");
        assert_eq!(language_code_for_langid(0x0407), "de");
        assert_eq!(language_code_for_langid(0x0411), "ja");
        assert_eq!(language_code_for_langid(0xffff), "en");
    }

    #[test]
    fn initializes_and_shuts_down_without_opening_a_window() {
        let host = NativeToolHostV1 {
            struct_size: size_of::<NativeToolHostV1>() as u32,
            abi_version: NATIVE_TOOL_ABI_V1,
            context: std::ptr::null_mut(),
            log: None,
            default_window_icon: 0,
            default_small_window_icon: 0,
        };
        assert_eq!(initialize_impl(&host), NativeToolResult::Ok);
        assert_eq!(shutdown_impl(0), NativeToolResult::Ok);
        assert!(can_unload_impl());
    }

    #[test]
    fn distinguishes_pen_and_touch_promoted_mouse_messages() {
        assert!(matches!(
            promoted_mouse_kind(MOUSE_POINTER_SIGNATURE as usize),
            PromotedMouseKind::Pen
        ));
        assert!(matches!(
            promoted_mouse_kind((MOUSE_POINTER_SIGNATURE | MOUSE_POINTER_TOUCH_FLAG) as usize),
            PromotedMouseKind::Touch
        ));
        assert!(matches!(promoted_mouse_kind(0), PromotedMouseKind::None));
        assert!(should_filter_promoted_mouse(PromotedMouseKind::Pen, true));
        assert!(!should_filter_promoted_mouse(
            PromotedMouseKind::Touch,
            true
        ));
        assert!(!should_filter_promoted_mouse(PromotedMouseKind::Pen, false));
    }

    #[test]
    fn mouse_hover_does_not_start_a_draw_contact() {
        assert_eq!(
            mouse_contact_transition(WM_MOUSEMOVE, WPARAM(0), false),
            (false, false)
        );
        assert_eq!(
            mouse_contact_transition(WM_MOUSEMOVE, WPARAM(MOUSE_LEFT_BUTTON), false),
            (true, true)
        );
        assert_eq!(
            mouse_contact_transition(WM_LBUTTONUP, WPARAM(0), true),
            (true, false)
        );
    }

    #[test]
    fn filtered_pen_mouse_does_not_enter_visible_mouse_counters() {
        let mut state = WindowState::new();
        state.count_filtered_pen_mouse(true);
        assert_eq!(state.mouse_events, 0);
        assert_eq!(state.mouse_contact_events, 0);
        assert_eq!(state.promoted_mouse_events, 1);
        assert_eq!(state.filtered_promoted_mouse_events, 1);
        assert_eq!(state.filtered_promoted_mouse_contact_events, 1);
    }

    #[test]
    fn diagnosis_distinguishes_normal_and_mouse_only_results() {
        let mut normal = WindowState::new();
        normal.mode = DiagnosticMode::Completed;
        normal.stats.down = 1;
        normal.stats.movement = 10;
        normal.stats.up = 1;
        normal.sampling_analysis.interval_p95_ms = 8.0;
        assert!(matches!(
            diagnosis_view(&normal).level,
            DiagnosisLevel::Normal
        ));

        let mut mouse_only = WindowState::new();
        mouse_only.mode = DiagnosticMode::Completed;
        mouse_only.mouse_contact_events = 4;
        assert!(matches!(
            diagnosis_view(&mouse_only).level,
            DiagnosisLevel::MouseOnly
        ));
    }

    #[test]
    fn diagnosis_warns_about_incomplete_pen_sequence() {
        let mut state = WindowState::new();
        state.mode = DiagnosticMode::Completed;
        state.stats.down = 1;
        state.stats.movement = 10;
        assert!(matches!(
            diagnosis_view(&state).level,
            DiagnosisLevel::Warning
        ));
    }

    #[test]
    fn combines_windows_tilt_axes_into_protocol_tilt() {
        assert_eq!(combined_tilt_degrees(0, 0), 0);
        assert_eq!(combined_tilt_degrees(45, 0), 45);
        assert_eq!(combined_tilt_degrees(45, 45), 55);
    }

    #[test]
    fn canvas_boundary_writes_a_cancel_record() {
        let path = test_recording_path("cancel");
        let mut state = recording_state(&path);
        append_recording_cancel(&mut state).unwrap();
        assert!(!state.recording_segment_open);
        let recording = state.recording.as_ref().unwrap();
        assert_eq!(recording.last_sample.unwrap().event_type, EVENT_CANCEL);
        assert_eq!(recording.sample_count, 2);
        assert!(state.stylus_data.samples.is_empty());
        drop(state.recording.take());
        let contents = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert!(contents.lines().last().unwrap().starts_with("P 1000 4 "));
    }

    #[test]
    fn sample_limit_marker_is_written_only_once() {
        let path = test_recording_path("truncated");
        let mut state = recording_state(&path);
        mark_recording_truncated(&mut state).unwrap();
        mark_recording_truncated(&mut state).unwrap();
        drop(state.recording.take());
        let contents = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert_eq!(contents.matches("# truncated=true").count(), 1);
    }

    #[test]
    fn rendered_points_can_be_released_without_losing_analysis_window() {
        let mut state = WindowState::new();
        let attributes = PenAttributes {
            pressure_available: true,
            tilt_x: 0,
            tilt_y: 0,
            tilt_available: true,
            rotation: 0,
            rotation_available: true,
        };
        let point_count = ANALYSIS_TRACE_POINTS * 4;
        for index in 0..point_count {
            state.push_pen(
                TracePoint {
                    pointer_id: 1,
                    x: index as i32,
                    y: 0,
                    pressure: 512,
                    timestamp_us: index as u64 * 4_000,
                    break_before: index == 0,
                },
                attributes,
            );
        }
        state.pen_trace.clear();

        assert_eq!(state.analysis_trace.len(), ANALYSIS_TRACE_POINTS);
        assert_eq!(state.analysis_stroke_point_count, point_count);
        assert!(state.analysis_trace.front().unwrap().break_before);
        let analysis = analyze_sampling(&state);
        assert_eq!(analysis.point_count, point_count);
        assert_eq!(analysis.recent_intervals_ms.len(), MAX_GRAPH_SAMPLES);
        assert_eq!(analysis.interval_median_ms, 4.0);
    }
}
