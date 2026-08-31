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

/// No `/proc` inventory on this platform; the session scanner selects another
/// inventory backend here (see `session_scanner::process`).
pub fn list_processes() -> Option<Vec<(u32, Vec<String>)>> {
    None
}

/// No per-PID stat lookup here: the `ps` inventory backend already carries
/// the PPID and TTY columns for every process it lists, so the session
/// scanner reads both from there (see `session_scanner::process`). `None`
/// means "unknown on this platform".
pub fn process_parent_and_tty(_pid: u32) -> Option<(u32, i64)> {
    None
}

/// Read one variable from another process's environment via `ps -Eww`.
///
/// macOS has no `/proc`, but `ps -E` appends a process's environment to its
/// command column — for processes owned by the same user, which is exactly the
/// case the scanner needs (`CLAUDE_CONFIG_DIR` on a session this user
/// launched). Returns `None` when the process is gone, belongs to another
/// user, or does not carry the variable; every caller has a default for that.
pub fn process_env_var(pid: u32, name: &str) -> Option<String> {
    let output = Command::new("ps")
        .args(["-Eww", "-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    env_var_from_ps_command(&String::from_utf8_lossy(&output.stdout), name)
}

/// Extract `NAME=value` from the `command` column of `ps -E`.
///
/// The column is `argv…` followed by the environment, space separated, so a
/// value runs until the next `KEY=` token starts (or the end of the line).
/// Only a token that starts a word is a candidate, so an argument such as
/// `--define=NAME=x` is not mistaken for the variable. A value that itself
/// contains something shaped like ` KEY=` is truncated there — `ps` output
/// carries no quoting that would let anyone do better.
fn env_var_from_ps_command(command: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    for line in command.lines() {
        let Some(start) = word_start_of(line, &needle) else {
            continue;
        };
        let rest = &line[start + needle.len()..];
        let end = rest
            .match_indices(' ')
            .find(|(index, _)| starts_env_token(&rest[index + 1..]))
            .map_or(rest.len(), |(index, _)| index);
        return Some(rest[..end].to_string());
    }
    None
}

/// Byte offset where `needle` starts a space-delimited word in `line`.
fn word_start_of(line: &str, needle: &str) -> Option<usize> {
    line.match_indices(needle)
        .find(|(index, _)| *index == 0 || line.as_bytes()[index - 1] == b' ')
        .map(|(index, _)| index)
}

/// Whether `rest` begins with an environment assignment (`KEY=`).
fn starts_env_token(rest: &str) -> bool {
    let mut chars = rest.char_indices();
    match chars.next() {
        Some((_, first)) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    for (index, c) in chars {
        if c == '=' {
            return index > 0;
        }
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    false
}

/// Process start time, for PID-reuse guards.
///
/// Linux-only (`/proc/<pid>/stat` field 22, the unit Claude Code records as
/// `procStart`). macOS has no comparable value in the registry record, so the
/// guard is skipped here rather than compared against a different clock.
pub fn process_start_ticks(_pid: u32) -> Option<u64> {
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

    // Regression: 06b432d added `process_env_var` for the Linux `/proc` reader
    // and stubbed macOS to `None`, so the native macOS daemon silently ignored
    // a session's `CLAUDE_CONFIG_DIR` and looked for its registry record and
    // transcript under the app's own `~/.claude` — the same "always yellow"
    // symptom the Linux fix removed. Exercises the real `ps -Eww` path.
    #[test]
    fn process_env_var_reads_child_environment() {
        let mut child = Command::new("sleep")
            .arg("30")
            .env("TAURHAUS_ENV_PROBE", "probe-42")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();

        // Until the child has exec'd, its environment is not the one we set.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut probe = process_env_var(pid, "TAURHAUS_ENV_PROBE");
        while probe.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
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
    fn env_var_from_ps_command_parses_the_environment_tail() {
        let line = "claude --continue PATH=/usr/bin CLAUDE_CONFIG_DIR=/Users/m1/.claude-account2 SHELL=/bin/zsh";
        assert_eq!(
            env_var_from_ps_command(line, "CLAUDE_CONFIG_DIR").as_deref(),
            Some("/Users/m1/.claude-account2")
        );
        // Last token on the line.
        assert_eq!(
            env_var_from_ps_command(line, "SHELL").as_deref(),
            Some("/bin/zsh")
        );
        // A value with spaces ends at the next `KEY=`.
        assert_eq!(
            env_var_from_ps_command(
                "claude CLAUDE_CONFIG_DIR=/Users/m1/My Cfg/.claude TERM=xterm",
                "CLAUDE_CONFIG_DIR"
            )
            .as_deref(),
            Some("/Users/m1/My Cfg/.claude")
        );
        // An argv flag that embeds the name is not the variable.
        assert_eq!(
            env_var_from_ps_command(
                "claude --define=CLAUDE_CONFIG_DIR=/nope TERM=xterm",
                "CLAUDE_CONFIG_DIR"
            ),
            None
        );
        assert_eq!(
            env_var_from_ps_command("claude TERM=xterm", "CLAUDE_CONFIG_DIR"),
            None
        );
    }

    #[test]
    fn process_start_ticks_is_not_available_on_macos() {
        assert!(process_start_ticks(std::process::id()).is_none());
    }
}
