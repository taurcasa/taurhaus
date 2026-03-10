//! macOS platform implementation — process inspection via `libproc` + `lsof`.
//!
//! Uses the `libproc` crate for direct process inspection (CWD, IO stats)
//! and falls back to `lsof` for TTY and TCP socket detection.

use std::path::PathBuf;
use std::process::Command;

use super::{InotifyProcessStats, InotifyUserStats};

/// Read the working directory of a process via `lsof`.
///
/// Runs `lsof -p PID -a -d cwd -F n` which outputs the CWD path.
/// Output format: first line is "p{PID}", second line is "n{path}".
pub fn process_cwd(pid: u32) -> Option<PathBuf> {
    // Try libproc first (faster, no subprocess)
    if let Some(cwd) = process_cwd_libproc(pid) {
        return Some(cwd);
    }
    // Fallback to lsof
    process_cwd_lsof(pid)
}

/// Read CWD using libproc's pidinfo with VnodePathInfo.
fn process_cwd_libproc(pid: u32) -> Option<PathBuf> {
    let pid = pid as i32;
    match libproc::libproc::proc_pid::pidinfo::<libproc::libproc::bsd_info::BSDInfo>(pid, 0) {
        Ok(_) => {
            // BSDInfo doesn't directly give CWD. Use pidpath as a quick check
            // that the process exists, then fall through to lsof.
            // TODO: When libproc exposes PROC_PIDVNODEPATHINFO, use that directly.
            None
        }
        Err(_) => None,
    }
}

/// Read CWD using lsof subprocess.
fn process_cwd_lsof(pid: u32) -> Option<PathBuf> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-a", "-d", "cwd", "-F", "n"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "n/path/to/dir" line (skip "p{PID}" line)
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}

/// Read the TTY/pts path from a process's stdin via `lsof`.
///
/// Runs `lsof -p PID -a -d 0 -F n` to get the target of fd 0 (stdin).
pub fn process_tty(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-a", "-d", "0", "-F", "n"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with("/dev/") {
                return Some(path.to_string());
            }
        }
    }
    None
}

pub fn process_inotify_stats(_pid: u32) -> Option<InotifyProcessStats> {
    None
}

pub fn current_user_inotify_stats() -> Option<InotifyUserStats> {
    None
}

/// Check whether a process currently has a specific file path open.
///
/// Not implemented on macOS yet (would require additional lsof parsing).
/// Returns false so scanner falls back to non-deterministic paths.
pub fn process_has_open_path(_pid: u32, _target_path: &str) -> bool {
    false
}

/// Read cumulative bytes read by a process via `libproc` rusage.
///
/// Uses `proc_pid_rusage()` to get `ri_diskio_bytesread` which tracks
/// cumulative disk IO. This is the macOS equivalent of Linux's
/// `/proc/PID/io` rchar field.
///
/// Note: This tracks disk IO only, not network IO. For Claude Code's
/// streaming detection, this should still work because Claude writes
/// to its session files during streaming.
pub fn process_rchar(pid: u32) -> Option<u64> {
    let pid = pid as i32;
    match libproc::libproc::pid_rusage::pidrusage::<libproc::libproc::pid_rusage::RUsageInfoV4>(pid)
    {
        Ok(rusage) => {
            // ri_diskio_bytesread tracks cumulative bytes read from disk.
            // Combined with byteswritten for a total IO picture.
            let total = rusage.ri_diskio_bytesread + rusage.ri_diskio_byteswritten;
            Some(total)
        }
        Err(_) => None,
    }
}

/// Collect socket "inodes" owned by a process.
///
/// On macOS there are no socket inodes like Linux. Instead, we collect
/// file descriptor numbers that are sockets. These are used as identifiers
/// for cross-referencing with TCP connection data.
///
/// Uses `lsof -p PID -i TCP -n -P -F nT` to list TCP connections.
/// Returns fd numbers as u64 for API compatibility with the Linux version.
pub fn collect_socket_inodes(pid: u32) -> Vec<u64> {
    // On macOS, we don't need separate inode collection — has_established_443
    // handles everything via lsof. Return empty to maintain API compatibility.
    // The socket_inodes parameter is ignored in has_established_443 on macOS.
    let _ = pid;
    vec![]
}

/// Check if the process has ESTABLISHED TCP connections to port 443.
///
/// Uses `lsof -p PID -i TCP -s TCP:ESTABLISHED -n -P` and checks for
/// connections to remote port 443.
///
/// The `_socket_inodes` parameter is ignored on macOS — lsof already
/// filters by PID, so we don't need cross-referencing.
pub fn has_established_443(pid: u32, _socket_inodes: &[u64]) -> bool {
    let output = match Command::new("lsof")
        .args([
            "-p",
            &pid.to_string(),
            "-i",
            "TCP",
            "-s",
            "TCP:ESTABLISHED",
            "-n",
            "-P",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Look for lines containing ":443" in the remote address
    // lsof output format: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
    // NAME field contains: host:port->remote:port
    for line in stdout.lines().skip(1) {
        // Skip header
        if line.contains("->") {
            // Extract remote part after "->"
            if let Some(remote) = line.split("->").nth(1) {
                if remote.contains(":443") {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a file watcher error indicates the system watch limit was hit.
///
/// On macOS, FSEvents doesn't have a per-user watch limit like Linux's inotify.
/// However, kqueue-based watchers can hit file descriptor limits.
pub fn is_watch_limit_error(error_msg: &str) -> bool {
    error_msg.contains("Too many open files") || error_msg.contains("kqueue")
}

/// User-facing message explaining how to fix watch limit errors.
pub fn watch_limit_help() -> &'static str {
    "Increase the open file limit with `ulimit -n` or reduce project count."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_cwd_returns_none_for_nonexistent_pid() {
        assert!(process_cwd(999_999_999).is_none());
    }

    #[test]
    fn process_tty_returns_none_for_nonexistent_pid() {
        assert!(process_tty(999_999_999).is_none());
    }

    #[test]
    fn process_rchar_returns_none_for_nonexistent_pid() {
        assert!(process_rchar(999_999_999).is_none());
    }

    #[test]
    fn collect_socket_inodes_returns_empty() {
        assert!(collect_socket_inodes(999_999_999).is_empty());
    }

    #[test]
    fn has_established_443_returns_false_for_nonexistent_pid() {
        assert!(!has_established_443(999_999_999, &[]));
    }

    #[test]
    fn is_watch_limit_error_detects_fd_limits() {
        assert!(is_watch_limit_error("Too many open files"));
        assert!(is_watch_limit_error("kqueue error"));
        assert!(!is_watch_limit_error("permission denied"));
    }

    #[test]
    fn process_cwd_works_for_self() {
        let pid = std::process::id();
        let cwd = process_cwd(pid);
        assert!(cwd.is_some(), "Should be able to read our own cwd");
    }

    #[test]
    fn process_rchar_works_for_self() {
        let pid = std::process::id();
        let rchar = process_rchar(pid);
        assert!(rchar.is_some(), "Should be able to read our own IO stats");
    }
}
