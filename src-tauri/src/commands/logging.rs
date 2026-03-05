use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Managed state: append-only log file for frontend + backend logs.
pub struct LogFileState(pub Mutex<std::fs::File>);

const LOG_WRITE_WARN_THROTTLE_MS: u64 = 5_000;
static LAST_LOG_WRITE_WARN_MS: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub fn frontend_log(level: String, message: String, log_file: tauri::State<LogFileState>) {
    frontend_log_impl(&level, &message, log_file.inner());
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn should_emit_write_warning(now_ms: u64) -> bool {
    LAST_LOG_WRITE_WARN_MS
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            if now_ms.saturating_sub(last) >= LOG_WRITE_WARN_THROTTLE_MS {
                Some(now_ms)
            } else {
                None
            }
        })
        .is_ok()
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
    if let Err(error) = writeln!(f, "[{timestamp}] [{tag}] [frontend] {message}") {
        if should_emit_write_warning(now_millis()) {
            tracing::warn!(error = %error, "failed to write frontend log line");
        }
    }
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
