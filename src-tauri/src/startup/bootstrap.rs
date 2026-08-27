use std::time::Instant;

use tauri::AppHandle;

pub(crate) fn spawn_background_startup_tasks(app: AppHandle) {
    std::thread::spawn(move || {
        run_background_task("activity_reseed", || {
            crate::bootstrap::startup_reseed_activity(&app)
        });
        run_background_task("session_scan", || {
            crate::bootstrap::startup_session_scan(&app)
        });
        run_background_task("search_index", || {
            crate::bootstrap::startup_search_index(&app)
        });
        run_background_task("task_scan", || crate::bootstrap::startup_task_scan(&app));
    });
}

pub(crate) fn spawn_legacy_statusline_cleanup() {
    if let Err(error) =
        spawn_legacy_cleanup(crate::session_scanner::accounts::legacy_statusline::retire_once)
    {
        tracing::warn!(error = %error, "Legacy Claude status-line cleanup failed to start");
    }
}

fn spawn_legacy_cleanup<F>(cleanup: F) -> Result<std::thread::JoinHandle<()>, String>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .name("claude-statusline-retire".to_string())
        .spawn(cleanup)
        .map_err(|error| error.to_string())
}

// Background startup never installs or refreshes tool credentials or usage
// bridges. It only retires the bridge shipped by the previous app version.

fn run_background_task<F>(task_group: &'static str, task: F)
where
    F: FnOnce(),
{
    super::telemetry::emit_startup_background_task_started(task_group);
    let started_at = Instant::now();
    task();
    super::telemetry::emit_startup_background_task_completed(
        task_group,
        started_at.elapsed().as_millis() as u64,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_cleanup_is_spawned_without_blocking_startup() {
        // Regression: d91737a ran the config scan and settings rewrite inline
        // before Tauri built the app, delaying the first frame and dropping
        // the removal event before the structured log sink existed.
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let before = Instant::now();

        let handle = spawn_legacy_cleanup(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .expect("cleanup thread");

        started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("cleanup started");
        assert!(before.elapsed() < std::time::Duration::from_millis(250));
        release_tx.send(()).unwrap();
        handle.join().unwrap();
    }
}
