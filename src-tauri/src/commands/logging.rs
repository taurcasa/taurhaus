use std::io::Write;
use std::sync::Mutex;

/// Managed state: append-only log file for frontend + backend logs.
pub struct LogFileState(pub Mutex<std::fs::File>);

#[tauri::command]
pub fn frontend_log(level: String, message: String, log_file: tauri::State<LogFileState>) {
    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
    let tag = match level.as_str() {
        "error" => "ERR",
        "warn" => "WRN",
        "debug" => "DBG",
        _ => "INF",
    };
    let mut f = log_file.0.lock().unwrap_or_else(|e| e.into_inner());
    let _ = writeln!(f, "[{timestamp}] [{tag}] [frontend] {message}");
}
