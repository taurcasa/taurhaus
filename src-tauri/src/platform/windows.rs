//! Windows platform implementation — no-op stubs for process inspection.
//!
//! On Windows, CLI tools (claude, codex, gemini) run inside WSL2, not as native
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

pub fn collect_socket_inodes(_pid: u32) -> Vec<u64> {
    Vec::new()
}

pub fn has_established_443(_pid: u32, _socket_inodes: &[u64]) -> bool {
    false
}

pub fn is_watch_limit_error(_error_msg: &str) -> bool {
    false
}

pub fn watch_limit_help() -> &'static str {
    "File watcher limit reached. Restart the application to recover."
}
