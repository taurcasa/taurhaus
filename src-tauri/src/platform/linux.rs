//! Linux platform implementation — process inspection via `/proc` filesystem.

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use super::{InotifyProcessStats, InotifyUserStats};

/// Upper bound for a single `/proc/{pid}/cmdline` read; `ps` applies the same cap.
const MAX_CMDLINE_BYTES: u64 = 128 * 1024;

/// List live processes as `(pid, args)` from `/proc/*/cmdline`, sorted by pid.
///
/// `args` joins argv with single spaces, matching `ps -o args`. Processes that
/// disappear mid-read or have no command line (kernel threads, zombies) are
/// skipped. Returns `None` only when `/proc` itself cannot be listed.
pub fn list_processes() -> Option<Vec<(u32, String)>> {
    let entries = fs::read_dir("/proc").ok()?;
    let mut processes: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            let args = process_args(pid)?;
            Some((pid, args))
        })
        .collect();
    processes.sort_unstable_by_key(|(pid, _)| *pid);
    Some(processes)
}

/// Read a process's command line from `/proc/{pid}/cmdline`, argv joined by spaces.
///
/// Returns `None` when the process is gone or has no command line.
pub fn process_args(pid: u32) -> Option<String> {
    let mut raw = Vec::new();
    fs::File::open(format!("/proc/{pid}/cmdline"))
        .ok()?
        .take(MAX_CMDLINE_BYTES)
        .read_to_end(&mut raw)
        .ok()?;
    let args = cmdline_to_args(&raw);
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

fn cmdline_to_args(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .trim_end_matches('\0')
        .split('\0')
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read one variable from a process's initial environment (`/proc/{pid}/environ`).
///
/// Returns `None` when the process is gone, its environment is not readable
/// (other user), or the variable is not set.
pub fn process_env_var(pid: u32, name: &str) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/environ")).ok()?;
    String::from_utf8_lossy(&raw).split('\0').find_map(|entry| {
        entry
            .strip_prefix(name)?
            .strip_prefix('=')
            .map(str::to_string)
    })
}

/// Read the executable path of a process from `/proc/{pid}/exe`.
pub fn process_exe(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// Whether a process directory still exists under `/proc`.
pub fn process_exists(pid: u32) -> bool {
    fs::metadata(format!("/proc/{pid}")).is_ok()
}

/// Read inotify instance and watch-descriptor counts from `/proc/{pid}`.
pub fn process_inotify_stats(pid: u32) -> Option<InotifyProcessStats> {
    let fd_dir = format!("/proc/{pid}/fd");
    let fdinfo_dir = format!("/proc/{pid}/fdinfo");
    let entries = fs::read_dir(&fd_dir).ok()?;

    let mut inotify_fds = Vec::new();
    for entry in entries.flatten() {
        let Ok(target) = fs::read_link(entry.path()) else {
            continue;
        };
        if target.to_string_lossy() == "anon_inode:inotify" {
            inotify_fds.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    let mut watch_count = 0u64;
    for fd_name in &inotify_fds {
        let Ok(content) = fs::read_to_string(format!("{fdinfo_dir}/{fd_name}")) else {
            continue;
        };
        watch_count += content
            .lines()
            .filter(|line| line.starts_with("inotify wd:"))
            .count() as u64;
    }

    Some(InotifyProcessStats {
        instance_count: inotify_fds.len() as u64,
        watch_count,
    })
}

/// Read inotify instance totals for the current Unix user.
pub fn current_user_inotify_stats() -> Option<InotifyUserStats> {
    use std::os::unix::fs::MetadataExt as _;

    let user_uid = fs::metadata("/proc/self").ok()?.uid();
    let entries = fs::read_dir("/proc").ok()?;
    let mut instance_count = 0u64;

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if file_name.to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.uid() != user_uid {
            continue;
        }
        let Ok(pid) = file_name.to_string_lossy().parse::<u32>() else {
            continue;
        };
        if let Some(stats) = process_inotify_stats(pid) {
            instance_count += stats.instance_count;
        }
    }

    let instance_limit = fs::read_to_string("/proc/sys/fs/inotify/max_user_instances")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok());
    let instance_pct = instance_limit
        .filter(|limit| *limit > 0)
        .map(|limit| (instance_count as f64 / limit as f64) * 100.0);

    Some(InotifyUserStats {
        instance_count,
        instance_limit,
        instance_pct,
    })
}

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
    let target_canon = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());

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

/// Find the first process that owns a LISTEN socket on the given TCP port.
pub fn listening_process_on_port(port: u16) -> Option<u32> {
    let target_inodes = collect_listening_socket_inodes(port);
    if target_inodes.is_empty() {
        return None;
    }

    let entries = fs::read_dir("/proc").ok()?;
    let mut pids = entries
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let pid = file_name.to_string_lossy().parse::<u32>().ok()?;
            let inodes = collect_socket_inodes(pid);
            if inodes.iter().any(|inode| target_inodes.contains(inode)) {
                Some(pid)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    pids.sort_unstable();
    pids.into_iter().next()
}

fn collect_listening_socket_inodes(port: u16) -> Vec<u64> {
    let hex_port = format!("{port:04X}");
    let mut inodes = Vec::new();

    for tcp_file in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(content) = fs::read_to_string(tcp_file) else {
            continue;
        };

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            // LISTEN state.
            if fields[3] != "0A" {
                continue;
            }

            if !fields[1].ends_with(&format!(":{hex_port}")) {
                continue;
            }

            if let Ok(inode) = fields[9].parse::<u64>() {
                inodes.push(inode);
            }
        }
    }

    inodes.sort_unstable();
    inodes.dedup();
    inodes
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
    fn list_processes_includes_current_process_with_its_args() {
        let processes = list_processes().expect("/proc listable");
        let own = std::process::id();
        let (_, args) = processes
            .iter()
            .find(|(pid, _)| *pid == own)
            .expect("own pid listed");
        let own_argv0 = std::env::args().next().expect("argv[0]");
        assert!(
            args.starts_with(&own_argv0),
            "args {args:?} should start with argv[0] {own_argv0:?}"
        );
        assert!(processes.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn process_args_returns_none_for_nonexistent_pid() {
        assert!(process_args(999_999_999).is_none());
    }

    #[test]
    fn cmdline_to_args_joins_argv_with_spaces() {
        assert_eq!(
            cmdline_to_args(b"node\0/usr/bin/claude\0--resume\0"),
            "node /usr/bin/claude --resume"
        );
        assert_eq!(cmdline_to_args(b""), "");
        assert_eq!(cmdline_to_args(b"codex"), "codex");
    }

    #[test]
    fn process_env_var_reads_child_environment() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .env("TAURHAUS_ENV_PROBE", "probe-42")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();

        // Until the child has exec'd, /proc/{pid}/environ still shows the
        // parent's environment; wait for the new image to be in place.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut probe = process_env_var(pid, "TAURHAUS_ENV_PROBE");
        while probe.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            probe = process_env_var(pid, "TAURHAUS_ENV_PROBE");
        }

        assert_eq!(probe.as_deref(), Some("probe-42"));
        assert_eq!(process_env_var(pid, "TAURHAUS_ENV_PROBE_MISSING"), None);

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn process_env_var_returns_none_for_nonexistent_pid() {
        assert!(process_env_var(999_999_999, "PATH").is_none());
    }

    #[test]
    fn process_cwd_returns_none_for_nonexistent_pid() {
        assert!(process_cwd(999_999_999).is_none());
    }

    #[test]
    fn process_exe_returns_none_for_nonexistent_pid() {
        assert!(process_exe(999_999_999).is_none());
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
    fn process_exists_false_for_nonexistent_pid() {
        assert!(!process_exists(999_999_999));
    }

    #[test]
    fn process_inotify_stats_works_for_self() {
        let stats = process_inotify_stats(std::process::id()).expect("self inotify stats");
        assert!(stats.instance_count <= 4096);
        assert!(stats.watch_count <= 1_000_000);
    }

    #[test]
    fn current_user_inotify_stats_reads_limit() {
        let stats = current_user_inotify_stats().expect("current user inotify stats");
        assert!(stats.instance_count <= 100_000);
        assert!(
            stats.instance_limit.is_none_or(|limit| limit > 0),
            "if present, inotify instance limit should be positive"
        );
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
    fn listening_process_on_port_finds_current_process_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("listener addr").port();

        let pid = listening_process_on_port(port).expect("pid for listening socket");
        assert_eq!(pid, std::process::id());
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
