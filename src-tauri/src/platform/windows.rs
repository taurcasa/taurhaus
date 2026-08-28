//! Windows platform implementation — no-op stubs for process inspection.
//!
//! On Windows, CLI tools (claude, codex, agy) run inside WSL2, not as native
//! Windows processes. Low-level process inspection therefore remains stubbed
//! here; higher-level session scanning must be routed through the WSL daemon.

use std::path::PathBuf;

use super::{InotifyProcessStats, InotifyUserStats};

pub fn process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

pub fn process_tty(_pid: u32) -> Option<String> {
    None
}

/// No `/proc` inventory on this platform; the session scanner selects another
/// inventory backend here (see `session_scanner::process`).
pub fn list_processes() -> Option<Vec<(u32, Vec<String>)>> {
    None
}

/// CLI tools run inside WSL2 here, so their controlling terminal is read by
/// the WSL daemon's Linux implementation, not from a native Windows process.
pub fn process_has_controlling_terminal(_pid: u32) -> Option<bool> {
    None
}

/// CLI tools run inside WSL2 here, so their environment is read by the WSL
/// daemon's Linux implementation, not from a native Windows process.
pub fn process_env_var(_pid: u32, _name: &str) -> Option<String> {
    None
}

/// Process start time for PID-reuse guards; see the Linux implementation.
/// Native Windows PIDs are never the ones the scanner correlates.
pub fn process_start_ticks(_pid: u32) -> Option<u64> {
    None
}

pub fn process_inotify_stats(_pid: u32) -> Option<InotifyProcessStats> {
    None
}

pub fn current_user_inotify_stats() -> Option<InotifyUserStats> {
    None
}

pub fn process_has_open_path(_pid: u32, _target_path: &str) -> bool {
    false
}

pub fn process_rchar(_pid: u32) -> Option<u64> {
    None
}

pub fn is_watch_limit_error(_error_msg: &str) -> bool {
    false
}

pub fn watch_limit_help() -> &'static str {
    "File watcher limit reached. Restart the application to recover."
}
