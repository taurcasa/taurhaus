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

// The Claude status-line bridge is deliberately not installed here. It is
// installed by the daemon, which the app starts on every platform, and one
// owner is the point: two installers bake two different executable paths into
// the same generated script and overwrite each other on every start. On
// Windows it would be worse than churn — account detection reaches the WSL
// home through its UNC path, so the app would write a bash script pointing at
// a Windows binary. See `daemon::server::run`.

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
