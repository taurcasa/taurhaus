//! Process IO activity tracker — reads `/proc/PID/io` to detect whether a
//! process is doing meaningful work (network IO from API calls) even when
//! the JSONL transcript hasn't been written to recently.
//!
//! Claude Code streams API responses over HTTPS. During the "thinking" gap
//! (between sending the request and receiving the first token), `rchar` in
//! `/proc/PID/io` still increases from network reads. When truly idle
//! (waiting for user input), only ~50-100 bytes/s of keepalive traffic flows.
//!
//! We track `rchar` deltas between consecutive polls. If the delta exceeds
//! a threshold, the process is considered active.

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

/// Minimum rchar delta (bytes) per poll interval to consider the process active.
/// Idle keepalive is ~50-100 bytes/500ms. Active API streaming is 10K+/500ms.
/// Set conservatively at 1KB to avoid false positives from keepalive noise.
const ACTIVE_IO_THRESHOLD: u64 = 1024;

/// Previous rchar values keyed by PID, protected by a mutex for thread safety.
static PREV_RCHAR: Mutex<Option<HashMap<u32, u64>>> = Mutex::new(None);

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

/// Check if a process is doing significant IO since the last poll.
///
/// Returns `true` if the process has read more than `ACTIVE_IO_THRESHOLD`
/// bytes since the previous call for this PID. On the first call for a PID,
/// returns `false` (no baseline to compare against).
///
/// Dead PIDs are automatically cleaned up when they disappear from the
/// provided set of active PIDs.
pub fn is_process_active(pid: u32) -> bool {
    let current = match read_rchar(pid) {
        Some(v) => v,
        None => return false, // process gone or unreadable
    };

    let mut guard = PREV_RCHAR.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let active = match map.get(&pid) {
        Some(&prev) => current.saturating_sub(prev) >= ACTIVE_IO_THRESHOLD,
        None => false, // first observation, no delta yet
    };

    map.insert(pid, current);
    active
}

/// Remove stale PIDs from the tracker that are no longer in the active set.
///
/// Call this once per scan cycle with the set of currently detected PIDs
/// to prevent the map from growing unboundedly.
pub fn retain_pids(active_pids: &[u32]) {
    let mut guard = PREV_RCHAR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_rchar_returns_none_for_nonexistent_pid() {
        assert!(read_rchar(999_999_999).is_none());
    }

    #[test]
    fn read_rchar_parses_current_process() {
        let pid = std::process::id();
        let result = read_rchar(pid);
        // Our own process should have read some bytes
        assert!(result.is_some());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn is_process_active_first_call_returns_false() {
        // Use a fake PID that won't exist — should return false (can't read)
        assert!(!is_process_active(999_999_998));
    }

    #[test]
    fn retain_pids_cleans_up_stale_entries() {
        // Insert a fake entry
        {
            let mut guard = PREV_RCHAR.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(777_777, 12345);
        }

        // Retain only PID 1 (not 777_777)
        retain_pids(&[1]);

        let guard = PREV_RCHAR.lock().unwrap();
        let map = guard.as_ref().unwrap();
        assert!(!map.contains_key(&777_777));
    }
}
