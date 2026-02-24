//! Process activity tracker — detects whether a CLI tool process is doing
//! meaningful work using `/proc` signals.
//!
//! Two detection strategies, matched to each tool's behavior:
//!
//! **Claude Code** — consecutive-poll hysteresis via `/proc/PID/io`.
//! Claude writes almost continuously during streaming/thinking, producing
//! sustained rchar deltas of 900+ bytes/500ms. Two consecutive above-threshold
//! polls reliably confirm activity while filtering single-sample spikes from
//! focus events or tmux switching.
//!
//! **Gemini** — TCP socket presence via `/proc/PID/fd` + `/proc/PID/net/tcp`.
//! Gemini creates HTTPS connections to its API endpoint on demand and closes
//! them when idle at the prompt. Any ESTABLISHED TCP connection to remote
//! port 443 owned by the process means an API call is in flight. This gives
//! instant active detection (connection opens) AND instant idle detection
//! (connection closes) with no decay timers or tradeoffs.
//!
//! **Codex** — session file mtime only (no proc-level signal used).
//! Codex maintains HTTP keep-alive connections to :443 indefinitely after
//! finishing work, making TCP socket presence useless as an idle indicator.
//! The session JSONL file mtime (checked in `idle.rs`) provides reliable
//! detection with ~9s active→idle latency (5s threshold + 4s hysteresis).
//!
//! Empirically confirmed (Feb 2026):
//! - Gemini idle at prompt: 0 ESTABLISHED connections to :443
//! - Gemini working (API call): 1+ ESTABLISHED connections to :443
//! - Codex idle at prompt: 1 ESTABLISHED connection (HTTP keep-alive, persistent)
//! - Codex working (API call): 1-2+ connections (indistinguishable from idle at count=1)
//! - Claude idle: 0-240 bytes/500ms keepalive in rchar
//! - Claude thinking: 900+ bytes/500ms sustained in rchar

use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Mutex;

/// Minimum rchar delta (bytes) per poll interval to trigger activity.
///
/// 500 bytes gives clean separation with 2x margin above Claude's idle noise
/// (0-240 bytes) and catches even the smallest Codex/Gemini bursts (7K+).
const ACTIVE_IO_THRESHOLD: u64 = 500;

/// HTTPS port in hex as it appears in `/proc/PID/net/tcp` remote_address field.
const PORT_443_HEX: &str = ":01BB";

/// Per-PID tracking state for IO activity detection (Claude only).
struct IoState {
    /// Previous rchar value (for computing delta).
    prev_rchar: u64,
    /// Whether the PREVIOUS poll showed activity above threshold.
    was_active: bool,
}

/// IO tracking state keyed by PID, protected by a mutex for thread safety.
static IO_STATE: Mutex<Option<HashMap<u32, IoState>>> = Mutex::new(None);

/// Read `rchar` (total bytes read, including network) from `/proc/PID/io`.
///
/// Returns `None` if the file can't be read (process gone, permissions, etc.).
fn read_rchar(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("rchar: ") {
            return val.trim().parse().ok();
        }
    }
    None
}

/// Check if a Claude process is active using consecutive-poll hysteresis.
///
/// Returns `true` only when BOTH the previous and current poll intervals show
/// rchar delta above threshold. This eliminates single-sample spikes from
/// focus events, tmux pane switching, etc., while reliably detecting sustained
/// activity (thinking, streaming, tool use).
pub fn is_process_active_hysteresis(pid: u32) -> bool {
    let current = match read_rchar(pid) {
        Some(v) => v,
        None => return false,
    };

    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let (active_now, confirmed) = match map.get(&pid) {
        Some(state) => {
            let delta = current.saturating_sub(state.prev_rchar);
            let active_now = delta >= ACTIVE_IO_THRESHOLD;
            let confirmed = active_now && state.was_active;
            (active_now, confirmed)
        }
        None => (false, false),
    };

    map.insert(pid, IoState {
        prev_rchar: current,
        was_active: active_now,
    });

    confirmed
}

// ---------------------------------------------------------------------------
// TCP socket detection (Codex/Gemini)
// ---------------------------------------------------------------------------

/// Check if a process has active HTTPS connections (TCP to remote port 443).
///
/// This is the primary activity signal for Codex and Gemini. These tools
/// create TCP connections to their API endpoints on demand and close them
/// when idle at the prompt. The presence of any ESTABLISHED connection
/// to port 443 means an API call is in flight.
///
/// Implementation: reads `/proc/PID/fd/` for socket inodes, then cross-
/// references with `/proc/PID/net/tcp{,6}` for ESTABLISHED connections
/// to remote port 0x01BB (443).
pub fn has_api_connections(pid: u32) -> bool {
    let socket_inodes = collect_socket_inodes(pid);
    if socket_inodes.is_empty() {
        return false;
    }
    has_established_443(&socket_inodes, pid)
}

/// Collect socket inodes from `/proc/PID/fd/`.
///
/// Reads each symlink in the fd directory. Socket entries look like
/// `socket:[12345]` — we extract the inode number.
fn collect_socket_inodes(pid: u32) -> HashSet<u64> {
    let fd_dir = format!("/proc/{pid}/fd");
    let mut inodes = HashSet::new();

    let entries = match fs::read_dir(&fd_dir) {
        Ok(entries) => entries,
        Err(_) => return inodes,
    };

    for entry in entries.flatten() {
        if let Ok(target) = fs::read_link(entry.path()) {
            let target_str = target.to_string_lossy();
            if let Some(inode) = parse_socket_inode(&target_str) {
                inodes.insert(inode);
            }
        }
    }

    inodes
}

/// Parse a socket inode from a `/proc/PID/fd/N` symlink target.
///
/// Example: `"socket:[12345]"` → `Some(12345)`
fn parse_socket_inode(target: &str) -> Option<u64> {
    target
        .strip_prefix("socket:[")
        .and_then(|s| s.strip_suffix(']'))
        .and_then(|s| s.parse().ok())
}

/// Check `/proc/PID/net/tcp{,6}` for ESTABLISHED connections to remote port 443
/// owned by this process (matching socket inodes).
///
/// `/proc/PID/net/tcp` is per-network-namespace (not per-process), so we must
/// cross-reference with the process's own socket inodes to filter correctly.
fn has_established_443(socket_inodes: &HashSet<u64>, pid: u32) -> bool {
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

/// Parse a single line from `/proc/net/tcp{,6}` and check if it represents
/// an ESTABLISHED connection to remote port 443 owned by one of our inodes.
///
/// Line format (whitespace-separated fields):
/// ```text
///   sl  local_address  rem_address  st  tx:rx  tr:tm  retrnsmt  uid  timeout  inode  ...
///    0  1              2            3   4      5      6         7    8        9
/// ```
///
/// - Field 2 (rem_address): `AABBCCDD:PORT` (IPv4) or 32-hex `:PORT` (IPv6)
/// - Field 3 (st): `01` = ESTABLISHED
/// - Field 9 (inode): socket inode number
fn is_established_443_line(line: &str, socket_inodes: &HashSet<u64>) -> bool {
    let mut fields = line.split_whitespace();

    // Skip to field 2 (rem_address) — fields 0=sl, 1=local, 2=remote
    let _sl = match fields.next() { Some(v) => v, None => return false };
    let _local = match fields.next() { Some(v) => v, None => return false };
    let remote = match fields.next() { Some(v) => v, None => return false };
    let state = match fields.next() { Some(v) => v, None => return false };

    // Must be ESTABLISHED (01) and remote port 443 (01BB)
    if state != "01" || !remote.ends_with(PORT_443_HEX) {
        return false;
    }

    // Skip fields 4-8 to get to field 9 (inode)
    for _ in 0..5 {
        if fields.next().is_none() {
            return false;
        }
    }

    let inode_str = match fields.next() { Some(v) => v, None => return false };
    let inode: u64 = match inode_str.parse() {
        Ok(i) => i,
        Err(_) => return false,
    };

    socket_inodes.contains(&inode)
}

/// Remove stale PIDs from the IO tracker that are no longer in the active set.
pub fn retain_pids(active_pids: &[u32]) {
    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- rchar tests --

    #[test]
    fn read_rchar_returns_none_for_nonexistent_pid() {
        assert!(read_rchar(999_999_999).is_none());
    }

    #[test]
    fn read_rchar_parses_current_process() {
        let pid = std::process::id();
        let result = read_rchar(pid);
        assert!(result.is_some());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn is_process_active_first_call_returns_false() {
        assert!(!is_process_active_hysteresis(999_999_998));
    }

    // -- Hysteresis tests (Claude) --

    #[test]
    fn hysteresis_requires_two_consecutive_active_polls() {
        let mut map: HashMap<u32, IoState> = HashMap::new();
        let pid = 888_888u32;

        map.insert(pid, IoState { prev_rchar: 1000, was_active: false });

        // Poll 2: large delta but previous was_active=false → not confirmed
        let state = map.get(&pid).unwrap();
        let delta = 2000u64.saturating_sub(state.prev_rchar);
        let active_now = delta >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(active_now);
        assert!(!confirmed);

        map.insert(pid, IoState { prev_rchar: 2000, was_active: active_now });

        // Poll 3: still large delta AND previous was_active=true → confirmed
        let state = map.get(&pid).unwrap();
        let delta = 3000u64.saturating_sub(state.prev_rchar);
        let active_now = delta >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(active_now);
        assert!(confirmed);
    }

    #[test]
    fn single_spike_not_reported_as_active() {
        let mut map: HashMap<u32, IoState> = HashMap::new();
        let pid = 888_889u32;

        map.insert(pid, IoState { prev_rchar: 1000, was_active: false });

        let state = map.get(&pid).unwrap();
        let active_now = 2000u64.saturating_sub(state.prev_rchar) >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(!confirmed);
        map.insert(pid, IoState { prev_rchar: 2000, was_active: active_now });

        let state = map.get(&pid).unwrap();
        let active_now = 2010u64.saturating_sub(state.prev_rchar) >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(!confirmed);
    }

    // -- Socket inode parsing --

    #[test]
    fn parse_socket_inode_valid() {
        assert_eq!(parse_socket_inode("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_inode("socket:[0]"), Some(0));
        assert_eq!(parse_socket_inode("socket:[999999999]"), Some(999999999));
    }

    #[test]
    fn parse_socket_inode_invalid() {
        assert_eq!(parse_socket_inode("pipe:[12345]"), None);
        assert_eq!(parse_socket_inode("anon_inode:[eventpoll]"), None);
        assert_eq!(parse_socket_inode("/dev/pts/2"), None);
        assert_eq!(parse_socket_inode("socket:[]"), None);
        assert_eq!(parse_socket_inode("socket:[abc]"), None);
    }

    // -- TCP line parsing --

    #[test]
    fn established_443_line_matches() {
        let inodes: HashSet<u64> = [12345].into_iter().collect();
        // Real /proc/net/tcp line format (IPv4)
        let line = "   0: 0100007F:C350 A04F68A0:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0";
        assert!(is_established_443_line(line, &inodes));
    }

    #[test]
    fn established_443_line_wrong_port() {
        let inodes: HashSet<u64> = [12345].into_iter().collect();
        // Port 80 (0050) instead of 443 (01BB)
        let line = "   0: 0100007F:C350 A04F68A0:0050 01 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0";
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn established_443_line_wrong_state() {
        let inodes: HashSet<u64> = [12345].into_iter().collect();
        // State 06 (TIME_WAIT) instead of 01 (ESTABLISHED)
        let line = "   0: 0100007F:C350 A04F68A0:01BB 06 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0";
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn established_443_line_wrong_inode() {
        let inodes: HashSet<u64> = [99999].into_iter().collect();
        // Inode 12345 not in our set
        let line = "   0: 0100007F:C350 A04F68A0:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0";
        assert!(!is_established_443_line(line, &inodes));
    }

    #[test]
    fn established_443_line_ipv6() {
        let inodes: HashSet<u64> = [54321].into_iter().collect();
        // IPv6 format line
        let line = "   0: 00000000000000000000000001000000:C350 00000000000000000000FFFF8E43C2BB:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 54321 1 0000000000000000 100 0 0 10 0";
        assert!(is_established_443_line(line, &inodes));
    }

    // -- collect_socket_inodes --

    #[test]
    fn collect_socket_inodes_nonexistent_pid() {
        let inodes = collect_socket_inodes(999_999_999);
        assert!(inodes.is_empty());
    }

    #[test]
    fn collect_socket_inodes_current_process() {
        // The test runner process should have at least some FDs open.
        // Whether any are sockets depends on the environment, but the
        // function should not panic or error.
        let _inodes = collect_socket_inodes(std::process::id());
    }

    // -- has_api_connections --

    #[test]
    fn has_api_connections_nonexistent_pid() {
        assert!(!has_api_connections(999_999_999));
    }

    #[test]
    fn has_api_connections_current_process_no_https() {
        // The test runner shouldn't have any HTTPS connections to :443
        assert!(!has_api_connections(std::process::id()));
    }

    // -- retain_pids --

    #[test]
    fn retain_pids_cleans_up_stale_entries() {
        {
            let mut guard = IO_STATE.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(
                777_777,
                IoState {
                    prev_rchar: 12345,
                    was_active: false,
                },
            );
        }

        retain_pids(&[1]);

        let guard = IO_STATE.lock().unwrap();
        let map = guard.as_ref().unwrap();
        assert!(!map.contains_key(&777_777));
    }
}
