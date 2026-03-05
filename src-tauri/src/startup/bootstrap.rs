use tauri::AppHandle;

pub(crate) fn spawn_background_startup_tasks(app: AppHandle) {
    std::thread::spawn(move || {
        crate::bootstrap::startup_reseed_activity(&app);
        crate::bootstrap::startup_session_scan(&app);
        crate::bootstrap::startup_search_index(&app);
        crate::bootstrap::startup_task_scan(&app);
    });
}
