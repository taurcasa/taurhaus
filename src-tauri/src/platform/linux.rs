//! Linux platform implementation — process inspection via `/proc` filesystem.

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use super::{InotifyProcessStats, InotifyUserStats};

/// Upper bound for a single `/proc/{pid}/cmdline` read; `ps` applies the same cap.
const MAX_CMDLINE_BYTES: u64 = 128 * 1024;

/// List live processes as `(pid, argv)` from `/proc/*/cmdline`, sorted by pid.
///
/// argv keeps its element boundaries, which is what tells a quoted prompt from
/// a separate argument; callers that need the `ps -o args` shape join it
/// themselves. Processes that disappear mid-read or have no command line
/// (kernel threads, zombies) are skipped. Returns `None` only when `/proc`
/// itself cannot be listed.
pub fn list_processes() -> Option<Vec<(u32, Vec<String>)>> {
    let entries = fs::read_dir("/proc").ok()?;
    let mut processes: Vec<(u32, Vec<String>)> = entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            let argv = process_argv(pid)?;
            Some((pid, argv))
        })
        .collect();
    processes.sort_unstable_by_key(|(pid, _)| *pid);
    Some(processes)
}

/// Read a process's argv elements from `/proc/{pid}/cmdline`.
///
/// Returns `None` when the process is gone or has no command line.
pub fn process_argv(pid: u32) -> Option<Vec<String>> {
    let mut raw = Vec::new();
    fs::File::open(format!("/proc/{pid}/cmdline"))
        .ok()?
        .take(MAX_CMDLINE_BYTES)
        .read_to_end(&mut raw)
        .ok()?;
    let argv = cmdline_to_argv(&raw);
    if argv.is_empty() {
        None
    } else {
        Some(argv)
    }
}

/// Read a process's command line from `/proc/{pid}/cmdline`, argv joined by
/// spaces — the `ps -o args` shape, for fingerprints and logging.
///
/// Returns `None` when the process is gone or has no command line.
pub fn process_args(pid: u32) -> Option<String> {
    Some(process_argv(pid)?.join(" "))
}

/// Split `/proc/{pid}/cmdline` bytes into argv elements.
///
/// The kernel keeps the NUL delimiters, so a quoted prompt stays one element:
/// `grok "help me"` is `["grok", "help me"]`, not three tokens. Classification
/// needs those boundaries — see `session_scanner::process::detect_cli_tool_argv`.
pub fn cmdline_to_argv(raw: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(raw);
    let trimmed = text.trim_end_matches('\0');
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split('\0').map(str::to_string).collect()
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

/// Process start time in clock ticks since boot (`/proc/{pid}/stat` field 22).
///
/// Claude Code records exactly this value as `procStart` in its sessions
/// registry, which makes it a PID-reuse guard: a record whose `procStart`
/// differs from the live process was written by a dead session that happened
/// to hold the same PID.
pub fn process_start_ticks(pid: u32) -> Option<u64> {
    parse_start_ticks(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// Field 22 of a `/proc/<pid>/stat` line.
///
/// Field 2 (`comm`) is parenthesised and may itself contain spaces and
/// parentheses, so everything up to the last `)` is skipped; the tokens that
/// follow start at field 3.
fn parse_start_ticks(stat: &str) -> Option<u64> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

/// Parent PID and controlling terminal from `/proc/{pid}/stat` fields 4 and 7.
///
/// Both values come from one read because the session inventory needs them as
/// one coherent process snapshot.
pub fn process_parent_and_tty(pid: u32) -> Option<(u32, i64)> {
    parse_parent_and_tty(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// Whether a process has a controlling terminal (`/proc/{pid}/stat` field 7).
///
/// `tty_nr == 0` means the process was started with no controlling terminal —
/// a detached one-shot run, a daemon child — which `ps` prints as `?`. The
/// session scanner drops those from its inventory: they are not interactive
/// sessions. Returns `None` when the process is gone or its stat line cannot
/// be parsed, which the caller reads as "unknown", not as "no terminal".
pub fn process_has_controlling_terminal(pid: u32) -> Option<bool> {
    process_parent_and_tty(pid).map(|(_, tty_nr)| tty_nr != 0)
}

/// Fields 4 and 7 of a `/proc/<pid>/stat` line.
///
/// Field 2 (`comm`) is parenthesised and may itself contain spaces and
/// parentheses, so everything up to the last `)` is skipped; the tokens that
/// follow start at field 3.
fn parse_parent_and_tty(stat: &str) -> Option<(u32, i64)> {
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let ppid = fields.get(1)?.parse().ok()?;
    let tty_nr = fields.get(4)?.parse().ok()?;
    Some((ppid, tty_nr))
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
        let (_, argv) = processes
            .iter()
            .find(|(pid, _)| *pid == own)
            .expect("own pid listed");
        let own_argv0 = std::env::args().next().expect("argv[0]");
        assert_eq!(
            argv.first().map(String::as_str),
            Some(own_argv0.as_str()),
            "argv {argv:?} should start with argv[0] {own_argv0:?}"
        );
        assert!(processes.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn process_args_returns_none_for_nonexistent_pid() {
        assert!(process_args(999_999_999).is_none());
    }

    // Regression: commit 54c9103 let the joined command line be the only form
    // the session scanner ever saw, so a prompt element (`grok "help me"`) was
    // indistinguishable from separate argv entries. The NUL boundaries are
    // what the classifier needs; the join stays for fingerprints and logging.
    #[test]
    fn cmdline_to_argv_keeps_every_argv_element_whole() {
        assert_eq!(cmdline_to_argv(b"grok\0help me\0"), ["grok", "help me"]);
        assert_eq!(
            cmdline_to_argv(b"grok\0--\0--help explain this\0"),
            ["grok", "--", "--help explain this"]
        );
        assert_eq!(cmdline_to_argv(b"codex"), ["codex"]);
        assert!(cmdline_to_argv(b"").is_empty());
    }

    #[test]
    fn cmdline_to_argv_joins_back_into_the_ps_args_shape() {
        assert_eq!(
            cmdline_to_argv(b"node\0/usr/bin/claude\0--resume\0").join(" "),
            "node /usr/bin/claude --resume"
        );
        assert_eq!(cmdline_to_argv(b"").join(" "), "");
        assert_eq!(cmdline_to_argv(b"codex").join(" "), "codex");
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
    fn parse_start_ticks_skips_a_comm_with_spaces_and_parens() {
        // Field 3 is the state; fields 4..=22 follow, so field 22 is the 20th
        // token after the comm.
        let fields: Vec<String> = (4..=22).map(|field| field.to_string()).collect();
        let stat = format!("4242 (weird (name) here) S {}", fields.join(" "));
        assert_eq!(parse_start_ticks(&stat), Some(22));
        assert_eq!(parse_start_ticks("4242 (short) S 1 2 3"), None);
    }

    #[test]
    fn process_start_ticks_matches_proc_stat_for_self() {
        let pid = std::process::id();
        let ticks = process_start_ticks(pid).expect("own start ticks");
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let expected: u64 = stat
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .nth(19)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(ticks, expected);
        assert!(process_start_ticks(999_999_999).is_none());
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
    fn parse_parent_and_tty_reads_fields_4_and_7_past_a_complex_comm() {
        // Field 3 is the state; fields 4..=22 follow, so field 4 (ppid) is the
        // 2nd token and field 7 (tty_nr) is the 5th token after the comm.
        let fields: Vec<String> = (4..=22).map(|field| field.to_string()).collect();
        let stat = format!("4242 (weird (name) here) S {}", fields.join(" "));
        assert_eq!(parse_parent_and_tty(&stat), Some((4, 7)));
        assert_eq!(parse_parent_and_tty("4242 (short) S 1 2"), None);
    }

    // Regression: the session scanner drops processes with no controlling
    // terminal, so this reading is what keeps a detached `codex exec` one-shot
    // (tty_nr 0) out of the inventory and a pts-backed session in it.
    #[test]
    fn process_has_controlling_terminal_matches_proc_stat_for_self() {
        let pid = std::process::id();
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let tty_nr: i64 = stat
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .nth(4)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            process_has_controlling_terminal(pid),
            Some(tty_nr != 0),
            "tty_nr {tty_nr} must decide the controlling-terminal answer"
        );
    }

    #[test]
    fn process_has_controlling_terminal_is_none_for_nonexistent_pid() {
        assert_eq!(process_has_controlling_terminal(999_999_999), None);
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
}
