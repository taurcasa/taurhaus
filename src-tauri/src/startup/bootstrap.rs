use serde_json::{Map, Value};
use tauri::AppHandle;

pub(crate) fn spawn_background_startup_tasks(app: AppHandle) {
    std::thread::spawn(move || {
        emit_background_task_started("activity_reseed");
        crate::bootstrap::startup_reseed_activity(&app);
        emit_background_task_started("session_scan");
        crate::bootstrap::startup_session_scan(&app);
        emit_background_task_started("search_index");
        crate::bootstrap::startup_search_index(&app);
        emit_background_task_started("task_scan");
        crate::bootstrap::startup_task_scan(&app);
    });
}

fn emit_background_task_started(task_group: &'static str) {
    let mut fields = Map::new();
    fields.insert(
        "task_group".to_string(),
        Value::String(task_group.to_string()),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "startup.background_tasks.started",
        Some("Startup background task started".to_string()),
        fields,
    );
}
