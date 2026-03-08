/// Synthetic project ID for Claude tasks directory watch events.
///
/// This internal ID is routed through watcher/event pipelines to trigger
/// background task scans instead of normal project file handling.
pub const CLAUDE_TASKS_PROJECT_ID: &str = "__claude_tasks__";

/// Synthetic project ID for tmux focus file watch events.
///
/// This internal ID is routed through watcher/event pipelines to emit
/// `tmux-focus-changed` instead of normal project file handling.
pub const TMUX_FOCUS_PROJECT_ID: &str = "__tmux_focus__";

/// Prefix used by synthetic internal watcher project IDs.
pub const INTERNAL_PROJECT_ID_PREFIX: &str = "__";

/// Python bytecode cache directory name used in ignore filters.
pub const PYTHON_CACHE_DIR: &str = "__pycache__";
