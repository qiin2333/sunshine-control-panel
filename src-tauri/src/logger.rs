use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// In-memory log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// Log collector state.
pub struct LogCollector {
    logs: Arc<Mutex<Vec<LogEntry>>>,
    max_logs: usize,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl LogCollector {
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::with_capacity(max_logs))),
            max_logs,
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&self, app: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(app);
    }

    pub fn add_log(&self, record: &Record) {
        // Keep only the file name, not the full path.
        let file = record.file().map(|f| {
            f.split('/')
                .last()
                .or_else(|| f.split('\\').last())
                .unwrap_or(f)
                .to_string()
        });

        let entry = LogEntry {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: match record.level() {
                Level::Error => "error".to_string(),
                Level::Warn => "warn".to_string(),
                Level::Info => "info".to_string(),
                Level::Debug => "debug".to_string(),
                Level::Trace => "trace".to_string(),
            },
            message: format!("{}", record.args()),
            target: Some(record.target().to_string()),
            file,
            line: record.line(),
        };

        // Append and cap the in-memory log list.
        {
            let mut logs = self.logs.lock().unwrap();
            logs.push(entry.clone());

            if logs.len() > self.max_logs {
                logs.remove(0);
            }
        }

        // Stream new entries to the log console when it is open.
        if let Ok(app_guard) = self.app_handle.lock() {
            if let Some(app) = app_guard.as_ref() {
                if let Some(window) = app.get_webview_window("log_console") {
                    let _ = window.emit("log-entry", &entry);
                }
            }
        }
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.lock().unwrap().clone()
    }

    pub fn clear_logs(&self) {
        self.logs.lock().unwrap().clear();
    }
}

/// Logger that forwards accepted records to the in-memory collector.
pub struct TauriLogger {
    collector: Arc<LogCollector>,
    inner: env_logger::Logger,
}

impl TauriLogger {
    pub fn new(collector: Arc<LogCollector>) -> Self {
        let default_log_level = "warn,tao=error,sunshine_gui=trace";
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| default_log_level.to_string());

        let mut builder =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&log_level));
        builder.format_timestamp_secs();
        builder.format_module_path(false);
        builder.format_target(false);

        Self {
            collector,
            inner: builder.build(),
        }
    }
}

impl Log for TauriLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.inner.enabled(metadata)
            && record_level_to_u8(metadata.level()) <= LOG_LEVEL_FILTER.load(Ordering::Relaxed)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // env_logger is used only for filtering; records are stored by LogCollector.
        self.collector.add_log(record);
    }

    fn flush(&self) {
        // Nothing to flush.
    }
}

static LOG_COLLECTOR: once_cell::sync::OnceCell<Arc<LogCollector>> =
    once_cell::sync::OnceCell::new();
static LOG_LEVEL_FILTER: AtomicU8 = AtomicU8::new(3);

fn level_to_u8(level: LevelFilter) -> u8 {
    match level {
        LevelFilter::Off => 0,
        LevelFilter::Error => 1,
        LevelFilter::Warn => 2,
        LevelFilter::Info => 3,
        LevelFilter::Debug => 4,
        LevelFilter::Trace => 5,
    }
}

fn level_from_str(level: &str) -> LevelFilter {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn record_level_to_u8(level: Level) -> u8 {
    match level {
        Level::Error => 1,
        Level::Warn => 2,
        Level::Info => 3,
        Level::Debug => 4,
        Level::Trace => 5,
    }
}

pub fn set_log_level(level: &str) {
    LOG_LEVEL_FILTER.store(level_to_u8(level_from_str(level)), Ordering::Relaxed);
}

/// Initialize logging.
pub fn init_logger(app: AppHandle) {
    let collector = Arc::new(LogCollector::new(10000));
    collector.set_app_handle(app);
    let settings = crate::desktop_settings::load_desktop_settings_from_disk();
    set_log_level(&settings.log_level);

    LOG_COLLECTOR.set(collector.clone()).ok();

    let logger = Box::new(TauriLogger::new(collector));

    log::set_logger(Box::leak(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .expect("Failed to initialize logger");
}

/// Return all collected logs.
#[tauri::command]
pub fn get_all_logs() -> Vec<LogEntry> {
    if let Some(collector) = LOG_COLLECTOR.get() {
        collector.get_logs()
    } else {
        Vec::new()
    }
}

/// Clear all collected logs.
#[tauri::command]
pub fn clear_logs() {
    if let Some(collector) = LOG_COLLECTOR.get() {
        collector.clear_logs();
    }
}

/// Export collected logs to a file.
#[tauri::command]
pub async fn export_logs(
    app: AppHandle,
    format: String, // "txt" or "json"
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;

    let logs = if let Some(collector) = LOG_COLLECTOR.get() {
        collector.get_logs()
    } else {
        return Err("Log collector is not initialized".to_string());
    };

    if logs.is_empty() {
        return Err("No logs to export".to_string());
    }

    // Generate default file name.
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let default_filename = format!("sunshine_gui_logs_{}.{}", timestamp, format);

    // Receive the save dialog result through a oneshot channel.
    let (tx, rx) = oneshot::channel();

    // Open save-file dialog.
    app.dialog()
        .file()
        .set_file_name(&default_filename)
        .add_filter("Text files", &["txt"])
        .add_filter("JSON files", &["json"])
        .add_filter("All files", &["*"])
        .save_file(move |file_path_opt| {
            let _ = tx.send(file_path_opt);
        });

    // Wait for the user to choose a file.
    let file_path = rx
        .await
        .map_err(|_| "Failed to receive dialog result".to_string())?
        .ok_or_else(|| "Save canceled".to_string())?;

    // Convert FilePath to PathBuf.
    let file_path: PathBuf = PathBuf::from(file_path.to_string());

    // Generate export content.
    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&logs)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?,
        "txt" | _ => {
            let mut text = String::new();
            text.push_str("Sunshine Control Panel Log Export\n");
            text.push_str(&format!(
                "Export time: {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ));
            text.push_str(&format!("Total logs: {}\n", logs.len()));
            text.push_str(&format!("{}\n\n", "=".repeat(80)));

            for log in &logs {
                let file_info = if let (Some(file), Some(line)) = (log.file.as_ref(), log.line) {
                    format!("{}:{}", file, line)
                } else if let Some(file) = log.file.as_ref() {
                    file.clone()
                } else {
                    "unknown".to_string()
                };

                text.push_str(&format!(
                    "[{}] [{}] [{}] {}\n",
                    log.timestamp,
                    log.level.to_uppercase(),
                    file_info,
                    log.message
                ));
            }
            text
        }
    };

    // Write export file.
    fs::write(&file_path, content).map_err(|e| format!("Failed to write file: {}", e))?;

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown file")
        .to_string();

    Ok(format!("Logs exported to: {}", file_name))
}
