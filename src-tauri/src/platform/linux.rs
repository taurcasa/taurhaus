//! Linux platform implementation — process inspection via `/proc` filesystem.

use std::fs;
use std::path::PathBuf;

/// Read the working directory of a process from `/proc/{pid}/cwd`.
pub fn process_cwd(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

/// Read the TTY/pts path from a process's stdin fd (`/proc/{pid}/fd/0`).
pub fn process_tty(pid: u32) -> Option<String> {
    fs::read_link(format!("/proc/{pid}/fd/0"))
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Check whether a process currently has a specific file path open.
///
/// Reads `/proc/{pid}/fd/*` symlinks and compares canonicalized targets.
pub fn process_has_open_path(pid: u32, target_path: &str) -> bool {
    let target = std::path::Path::new(target_path);
    let target_canon = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());

    let fd_dir = format!("/proc/{pid}/fd");
    let entries = match fs::read_dir(&fd_dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        if let Ok(link_target) = fs::read_link(entry.path()) {
            // Ignore sockets/pipes/devices; only regular-ish paths can match.
            if !link_target.is_absolute() {
                continue;
            }
            let canon = link_target
                .canonicalize()
                .unwrap_or_else(|_| link_target.clone());
            if canon == target_canon {
                return true;
            }
        }
    }

    false
}

/// Read cumulative bytes read by a process from `/proc/{pid}/io`.
///
/// Returns the `rchar` value, which includes all reads (file, network, pipe).
/// Used for IO-based activity detection (Claude Code hysteresis).
pub fn process_rchar(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("rchar: ") {
            return val.trim().parse().ok();
        }
    }
    None
}

/// Collect socket inode numbers owned by a process.
///
/// Reads `/proc/{pid}/fd/` and extracts inodes from symlinks like `socket:[12345]`.
pub fn collect_socket_inodes(pid: u32) -> Vec<u64> {
    let fd_dir = format!("/proc/{pid}/fd");
    let entries = match fs::read_dir(&fd_dir) {
        Ok(entries) => entries,
        Err(_) => return vec![],
    };

    let mut inodes = Vec::new();
    for entry in entries.flatten() {
        if let Ok(target) = fs::read_link(entry.path()) {
            let s = target.to_string_lossy();
            if let Some(rest) = s.strip_prefix("socket:[") {
                if let Some(inode_str) = rest.strip_suffix(']') {
                    if let Ok(inode) = inode_str.parse::<u64>() {
                        inodes.push(inode);
                    }
                }
            }
        }
    }
    inodes
}

/// Check if the process has ESTABLISHED TCP connections to port 443.
///
/// Reads `/proc/{pid}/net/tcp` and `/proc/{pid}/net/tcp6`, matches
/// ESTABLISHED state (01) with remote port 443 (01BB), and cross-references
/// with the process's socket inodes.
pub fn has_established_443(pid: u32, socket_inodes: &[u64]) -> bool {
    if socket_inodes.is_empty() {
        return false;
    }

    for tcp_file in [
        format!("/proc/{pid}/net/tcp"),
        format!("/proc/{pid}/net/tcp6"),
    ] {
        let content = match fs::read_to_string(&tcp_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line in content.lines().skip(1) {
            if is_established_443_line(line, socket_inodes) {
                return true;
            }
        }
    }

    false
}

/// Parse a single line from /proc/net/tcp to check if it's an ESTABLISHED
/// connection to port 443 owned by one of our socket inodes.
fn is_established_443_line(line: &str, socket_inodes: &[u64]) -> bool {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 10 {
        return false;
    }

    // Field 3: state (01 = ESTABLISHED)
    if fields[3] != "01" {
        return false;
    }

    // Field 2: remote address (hex:hex). Port is after the colon.
    let remote = fields[2];
    if !remote.ends_with(":01BB") {
        return false;
    }

    // Field 9: inode
    if let Ok(inode) = fields[9].parse::<u64>() {
        socket_inodes.contains(&inode)
    } else {
        false
    }
}

/// Check if a file watcher error indicates the system watch limit was hit.
///
/// On Linux, inotify has a per-user watch limit (`fs.inotify.max_user_watches`).
/// The `notify` crate surfaces this as "No space left on device" or mentions "inotify".
pub fn is_watch_limit_error(error_msg: &str) -> bool {
    error_msg.contains("No space left on device") || error_msg.contains("inotify")
}

/// User-facing message explaining how to fix watch limit errors.
pub fn watch_limit_help() -> &'static str {
    "Increase fs.inotify.max_user_watches or reduce project count."
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
    fn process_has_open_path_false_for_nonexistent_pid() {
        assert!(!process_has_open_path(999_999_999, "/tmp/nonexistent"));
    }

    #[test]
    fn process_rchar_returns_none_for_nonexistent_pid() {
        assert!(process_rchar(999_999_999).is_none());
    }

    #[test]
    fn collect_socket_inodes_returns_empty_for_nonexistent_pid() {
        assert!(collect_socket_inodes(999_999_999).is_empty());
    }

    #[test]
    fn process_cwd_works_for_self() {
        // PID 1 should exist on Linux, or use our own PID
        let pid = std::process::id();
        let cwd = process_cwd(pid);
        assert!(cwd.is_some(), "Should be able to read our own cwd");
    }

    #[test]
    fn process_rchar_works_for_self() {
        let pid = std::process::id();
        let rchar = process_rchar(pid);
        assert!(rchar.is_some(), "Should be able to read our own IO stats");
        assert!(
            rchar.unwrap() > 0,
            "rchar should be > 0 for a running process"
        );
    }

    #[test]
    fn is_established_443_line_parses_correctly() {
        // Simulated /proc/net/tcp line (ESTABLISHED, remote port 443, inode 12345)
        let line = "  0: 0100007F:C350 0100007F:01BB 01 00000000:00000000 00:00000000 00000000  1000    0 12345 1 0000000000000000 100 0 0 10 0";
        let inodes = vec![12345];
        assert!(is_established_443_line(line, &inodes));
    }

    #[test]
    fn is_established_443_line_rejects_wrong_port() {
        let line = "  0: 0100007F:C350 0100007F:0050 01 00000000:00000000 00:00000000 00000000  1000    0 12345 1 0000000000000000 100 0 0 10 0";
        let inodes = vec![12345];
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn is_established_443_line_rejects_wrong_state() {
        // State 06 = TIME_WAIT
        let line = "  0: 0100007F:C350 0100007F:01BB 06 00000000:00000000 00:00000000 00000000  1000    0 12345 1 0000000000000000 100 0 0 10 0";
        let inodes = vec![12345];
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn is_established_443_line_rejects_wrong_inode() {
        let line = "  0: 0100007F:C350 0100007F:01BB 01 00000000:00000000 00:00000000 00000000  1000    0 99999 1 0000000000000000 100 0 0 10 0";
        let inodes = vec![12345];
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn has_established_443_returns_false_for_empty_inodes() {
        assert!(!has_established_443(1, &[]));
    }
}
