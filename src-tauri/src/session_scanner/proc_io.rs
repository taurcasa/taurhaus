//! Process IO activity tracker — reads `/proc/PID/io` to detect whether
//! a Claude Code process is doing meaningful work.
//!
//! During active work (including "thinking" / API waiting), `rchar` increases
//! by 900+ bytes per 500ms from network reads. When truly idle (waiting for
//! user input), only 0-240 bytes/500ms of keepalive traffic flows.
//!
//! We track `rchar` deltas between consecutive polls. Activity must be
//! sustained for 2 consecutive polls to confirm (hysteresis), preventing
//! single-sample spikes from focus events or tmux pane switching.

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

/// Minimum rchar delta (bytes) per poll interval to consider the process active.
///
/// Measured empirically:
/// - Idle keepalive: 0-240 bytes/500ms
/// - Thinking (API wait): 900+ bytes/500ms
/// - Active streaming: 200K-900K bytes/500ms
///
/// 500 bytes gives clean separation with 2x margin above idle noise.
const ACTIVE_IO_THRESHOLD: u64 = 500;

/// Per-PID tracking state for IO activity detection.
struct IoState {
    /// Previous rchar value (for computing delta).
    prev_rchar: u64,
    /// Whether the PREVIOUS poll showed activity above threshold.
    /// Used for hysteresis: we only report active when both the previous
    /// and current polls exceed the threshold (2 consecutive).
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

/// Check if a process is doing significant IO since the last poll.
///
/// Uses 2-poll hysteresis: returns `true` only when BOTH the previous and
/// current poll intervals show rchar delta above threshold. This eliminates
/// single-sample spikes from focus events, tmux pane switching, etc., while
/// reliably detecting sustained activity (thinking, streaming, tool use).
///
/// Adds ~500ms latency to detect activity start (one extra poll), which is
/// negligible since thinking phases typically last 5-30+ seconds.
pub fn is_process_active(pid: u32) -> bool {
    let current = match read_rchar(pid) {
        Some(v) => v,
        None => return false, // process gone or unreadable
    };

    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let (active_now, confirmed) = match map.get(&pid) {
        Some(state) => {
            let delta = current.saturating_sub(state.prev_rchar);
            let active_now = delta >= ACTIVE_IO_THRESHOLD;
            // Require 2 consecutive active polls
            let confirmed = active_now && state.was_active;
            (active_now, confirmed)
        }
        None => (false, false), // first observation, no baseline
    };

    map.insert(
        pid,
        IoState {
            prev_rchar: current,
            was_active: active_now,
        },
    );

    confirmed
}

/// Remove stale PIDs from the tracker that are no longer in the active set.
pub fn retain_pids(active_pids: &[u32]) {
    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
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
        assert!(result.is_some());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn is_process_active_first_call_returns_false() {
        assert!(!is_process_active(999_999_998));
    }

    #[test]
    fn hysteresis_requires_two_consecutive_active_polls() {
        // Simulate the hysteresis behavior using the internal state directly
        let mut map: HashMap<u32, IoState> = HashMap::new();
        let pid = 888_888u32;

        // Poll 1: no previous state → not active
        map.insert(pid, IoState { prev_rchar: 1000, was_active: false });

        // Poll 2: large delta but previous was_active=false → not confirmed
        let state = map.get(&pid).unwrap();
        let delta = 2000u64.saturating_sub(state.prev_rchar); // 1000 bytes
        let active_now = delta >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(active_now);
        assert!(!confirmed); // First spike: not confirmed

        map.insert(pid, IoState { prev_rchar: 2000, was_active: active_now });

        // Poll 3: still large delta AND previous was_active=true → confirmed!
        let state = map.get(&pid).unwrap();
        let delta = 3000u64.saturating_sub(state.prev_rchar); // 1000 bytes
        let active_now = delta >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(active_now);
        assert!(confirmed); // Two consecutive: confirmed
    }

    #[test]
    fn single_spike_not_reported_as_active() {
        // Simulate: idle → spike → idle
        let mut map: HashMap<u32, IoState> = HashMap::new();
        let pid = 888_889u32;

        // Baseline
        map.insert(pid, IoState { prev_rchar: 1000, was_active: false });

        // Spike: delta=1000 but was_active=false
        let state = map.get(&pid).unwrap();
        let active_now = 2000u64.saturating_sub(state.prev_rchar) >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(!confirmed); // Not reported
        map.insert(pid, IoState { prev_rchar: 2000, was_active: active_now });

        // Back to idle: delta=10
        let state = map.get(&pid).unwrap();
        let active_now = 2010u64.saturating_sub(state.prev_rchar) >= ACTIVE_IO_THRESHOLD;
        let confirmed = active_now && state.was_active;
        assert!(!confirmed); // Spike was absorbed by hysteresis
    }

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
