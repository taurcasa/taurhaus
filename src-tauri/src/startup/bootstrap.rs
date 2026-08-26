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
        run_background_task("claude_usage_statusline", install_claude_usage_statusline);
    });
}

/// Take the status-line seat in every detected Claude config dir.
///
/// Off the critical path on purpose: it probes `claude --version`, reads config
/// dirs and writes settings, and none of that may delay a window appearing. On
/// Windows the app sees no WSL config dirs and the daemon does this instead.
fn install_claude_usage_statusline() {
    let Ok(exe) = std::env::current_exe() else {
        tracing::debug!("Claude usage status line skipped: this build has no resolvable path");
        return;
    };
    crate::session_scanner::claude_statusline::install_statusline_for_detected_accounts(&exe);
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
