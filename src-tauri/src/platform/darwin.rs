//! macOS platform implementation — process inspection via libproc.
//!
//! Stub implementation. Functions return None/empty until M07-M09 implement
//! the actual macOS equivalents using the `libproc` crate.

use std::path::PathBuf;

/// Read the working directory of a process.
///
/// TODO(M07): Use `libproc::proc_pidinfo()` with `PROC_PIDVNODEPATHINFO`.
pub fn process_cwd(_pid: u32) -> Option<PathBuf> {
    tracing::warn!("process_cwd not implemented on macOS");
    None
}

/// Read the TTY/pts path from a process's stdin.
///
/// TODO(M07): Use `libproc::proc_pidfdinfo()`.
pub fn process_tty(_pid: u32) -> Option<String> {
    tracing::warn!("process_tty not implemented on macOS");
    None
}

/// Read cumulative bytes read by a process.
///
/// TODO(M08): Use `proc_pid_rusage()` → `ri_diskio_bytesread`.
pub fn process_rchar(_pid: u32) -> Option<u64> {
    tracing::warn!("process_rchar not implemented on macOS");
    None
}

/// Collect socket inode numbers owned by a process.
///
/// TODO(M09): Use `proc_pidfdinfo()` with `PROC_PIDFDSOCKETINFO`.
pub fn collect_socket_inodes(_pid: u32) -> Vec<u64> {
    vec![]
}

/// Check if the process has ESTABLISHED TCP connections to port 443.
///
/// TODO(M09): Use `lsof` or `proc_pidfdinfo()`.
pub fn has_established_443(_pid: u32, _socket_inodes: &[u64]) -> bool {
    false
}
