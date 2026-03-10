//! Shared types for platform-specific process inspection.
//!
//! These types are the same across all platforms — only the implementation
//! of the functions that produce them differs.

use std::path::PathBuf;

/// Enriched process metadata gathered from OS-specific APIs.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    /// Working directory of the process (project path).
    pub cwd: Option<PathBuf>,
    /// TTY/pts path for the process's stdin (e.g., `/dev/pts/3`).
    pub tty: Option<String>,
}

/// Linux inotify usage for a single process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InotifyProcessStats {
    pub instance_count: u64,
    pub watch_count: u64,
}

/// Linux inotify usage summed across the current user.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InotifyUserStats {
    pub instance_count: u64,
    pub instance_limit: Option<u64>,
    pub instance_pct: Option<f64>,
}
