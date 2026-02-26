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
