use std::io::Write;
use std::sync::Mutex;

/// Managed state: append-only log file for frontend + backend logs.
pub struct LogFileState(pub Mutex<std::fs::File>);

#[tauri::command]
pub fn frontend_log(level: String, message: String, log_file: tauri::State<LogFileState>) {
    frontend_log_impl(&level, &message, log_file.inner());
}

fn frontend_log_impl(level: &str, message: &str, log_file: &LogFileState) {
    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
    let tag = match level {
        "error" => "ERR",
        "warn" => "WRN",
        "debug" => "DBG",
        _ => "INF",
    };
    let mut f = log_file.0.lock().unwrap_or_else(|e| e.into_inner());
    let _ = writeln!(f, "[{timestamp}] [{tag}] [frontend] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    fn test_log_file_state() -> (LogFileState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(tmp.path())
            .expect("open log");
        (LogFileState(Mutex::new(file)), tmp)
    }

    fn read_log(path: &std::path::Path) -> String {
        let mut f = std::fs::File::open(path).expect("open log for read");
        let mut out = String::new();
        f.read_to_string(&mut out).expect("read log");
        out
    }

    #[test]
    fn frontend_log_writes_expected_frontend_line() {
        let (state, tmp) = test_log_file_state();

        frontend_log_impl("warn", "hello from ui", &state);
        let output = read_log(tmp.path());

        assert!(output.contains("[WRN]"));
        assert!(output.contains("[frontend] hello from ui"));
    }

    #[test]
    fn frontend_log_recovers_from_poisoned_lock() {
        let (state, tmp) = test_log_file_state();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.0.lock().expect("lock");
            panic!("poison log lock");
        }));

        frontend_log_impl("error", "still writes", &state);
        let output = read_log(tmp.path());

        assert!(output.contains("[ERR]"));
        assert!(output.contains("[frontend] still writes"));
    }
}
