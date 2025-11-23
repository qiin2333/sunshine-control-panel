use log::{Level, Log, Metadata, Record};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Emitter};
use serde::{Deserialize, Serialize};
use chrono::Local;
use std::fs;
use std::path::PathBuf;

/// 日志条目结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// 日志收集器状态
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
        // 提取文件名（只保留文件名，不包含路径）
        let file = record.file().map(|f| {
            f.split('/').last()
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

        // 添加到日志列表
        {
            let mut logs = self.logs.lock().unwrap();
            logs.push(entry.clone());
            
            // 限制日志数量
            if logs.len() > self.max_logs {
                logs.remove(0);
            }
        }

        // 发送事件到日志控制台窗口
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

/// 自定义日志记录器
pub struct TauriLogger {
    collector: Arc<LogCollector>,
    inner: env_logger::Logger,
}

impl TauriLogger {
    pub fn new(collector: Arc<LogCollector>) -> Self {
        let default_log_level = if cfg!(debug_assertions) { "debug" } else { "info" };
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| default_log_level.to_string());
        
        let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&log_level));
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
    }

    fn log(&self, record: &Record) {
        // 只收集日志，不调用 env_logger 的 log 方法（避免重复记录）
        // env_logger 只用于判断是否启用日志级别
        self.collector.add_log(record);
    }

    fn flush(&self) {
        // env_logger 的 flush 不需要调用
    }
}

static LOG_COLLECTOR: once_cell::sync::OnceCell<Arc<LogCollector>> = once_cell::sync::OnceCell::new();

/// 初始化日志系统
pub fn init_logger(app: AppHandle) {
    let collector = Arc::new(LogCollector::new(10000)); // 最多保存 10000 条日志
    collector.set_app_handle(app);
    
    LOG_COLLECTOR.set(collector.clone()).ok();
    
    let logger = Box::new(TauriLogger::new(collector));
    
    log::set_logger(Box::leak(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .expect("无法初始化日志系统");
}

/// 获取所有日志
#[tauri::command]
pub fn get_all_logs() -> Vec<LogEntry> {
    if let Some(collector) = LOG_COLLECTOR.get() {
        collector.get_logs()
    } else {
        Vec::new()
    }
}

/// 清空日志
#[tauri::command]
pub fn clear_logs() {
    if let Some(collector) = LOG_COLLECTOR.get() {
        collector.clear_logs();
    }
}

/// 导出日志到文件
#[tauri::command]
pub async fn export_logs(
    app: AppHandle,
    format: String, // "txt" 或 "json"
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;
    
    let logs = if let Some(collector) = LOG_COLLECTOR.get() {
        collector.get_logs()
    } else {
        return Err("日志收集器未初始化".to_string());
    };
    
    if logs.is_empty() {
        return Err("没有日志可导出".to_string());
    }
    
    // 生成文件名
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let default_filename = format!("sunshine_gui_logs_{}.{}", timestamp, format);
    
    // 使用 oneshot channel 来接收对话框结果
    let (tx, rx) = oneshot::channel();
    
    // 打开保存文件对话框
    app.dialog()
        .file()
        .set_file_name(&default_filename)
        .add_filter("文本文件", &["txt"])
        .add_filter("JSON文件", &["json"])
        .add_filter("所有文件", &["*"])
        .save_file(move |file_path_opt| {
            let _ = tx.send(file_path_opt);
        });
    
    // 等待用户选择文件
    let file_path = rx.await
        .map_err(|_| "无法接收对话框结果".to_string())?
        .ok_or_else(|| "用户取消了保存".to_string())?;
    
    // 将 FilePath 转换为 PathBuf
    let file_path: PathBuf = PathBuf::from(file_path.to_string());
    
    // 根据格式生成内容
    let content = match format.as_str() {
        "json" => {
            serde_json::to_string_pretty(&logs)
                .map_err(|e| format!("序列化JSON失败: {}", e))?
        }
        "txt" | _ => {
            let mut text = String::new();
            text.push_str(&format!("Sunshine Control Panel 日志导出\n"));
            text.push_str(&format!("导出时间: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
            text.push_str(&format!("日志总数: {}\n", logs.len()));
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
    
    // 写入文件
    fs::write(&file_path, content)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    
    let file_name = file_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文件")
        .to_string();
    
    Ok(format!("日志已导出到: {}", file_name))
}

