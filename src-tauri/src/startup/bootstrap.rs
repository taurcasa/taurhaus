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
