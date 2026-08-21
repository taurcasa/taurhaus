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
//! **Codex** — per-PID IO hysteresis + project file mtime fallback.
//! Codex maintains HTTP keep-alive connections to :443 indefinitely after
//! finishing work, making TCP socket presence useless as an idle indicator.
//! We therefore use `/proc/PID/io` hysteresis per process to distinguish
//! which Codex session is actively doing work. Project-level session file
//! mtime (from `idle.rs`) remains a fallback for single-session projects.
//!
//! Empirically confirmed (Feb 2026):
//! - Gemini idle at prompt: 0 ESTABLISHED connections to :443
//! - Gemini working (API call): 1+ ESTABLISHED connections to :443
//! - Codex idle at prompt: 1 ESTABLISHED connection (HTTP keep-alive, persistent)
//! - Codex working (API call): 1-2+ connections (indistinguishable from idle at count=1)
//! - Claude idle: 0-240 bytes/500ms keepalive in rchar
//! - Claude thinking: 900+ bytes/500ms sustained in rchar

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Minimum sustained read rate (bytes/second) that counts as activity.
///
/// Calibrated as the original 500 bytes per 500 ms poll: 2x margin above
/// Claude's idle noise (0-240 bytes/500ms) and far below the smallest
/// Codex/Gemini bursts (7K+). Expressed as a rate because the scanner cadence
/// is not fixed — it flips between 500 ms and 1500 ms
/// (`daemon::session_activity::{ACTIVE_SCAN_INTERVAL, IDLE_SCAN_INTERVAL}`).
const ACTIVE_IO_RATE_BYTES_PER_SEC: u64 = 1_000;

/// Per-PID tracking state for IO activity detection (Claude only).
struct IoState {
    /// Previous rchar value (for computing delta).
    prev_rchar: u64,
    /// When `prev_rchar` was sampled, so the delta can be turned into a rate.
    sampled_at: Instant,
    /// Whether the PREVIOUS poll showed activity above threshold.
    was_active: bool,
}

/// Whether an rchar delta observed over `elapsed` clears the activity rate.
fn is_active_rate(delta_bytes: u64, elapsed: Duration) -> bool {
    let elapsed_ms = u64::try_from(elapsed.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    delta_bytes.saturating_mul(1_000) / elapsed_ms >= ACTIVE_IO_RATE_BYTES_PER_SEC
}

/// One hysteresis step: `(active_now, confirmed)` for a fresh rchar sample.
fn hysteresis_step(previous: Option<&IoState>, current: u64, now: Instant) -> (bool, bool) {
    match previous {
        Some(state) => {
            let delta = current.saturating_sub(state.prev_rchar);
            let active_now = is_active_rate(delta, now.saturating_duration_since(state.sampled_at));
            (active_now, active_now && state.was_active)
        }
        None => (false, false),
    }
}

/// IO tracking state keyed by PID, protected by a mutex for thread safety.
static IO_STATE: Mutex<Option<HashMap<u32, IoState>>> = Mutex::new(None);

/// Read `rchar` (total bytes read, including network) via platform-specific APIs.
///
/// Returns `None` if the data can't be read (process gone, permissions, etc.).
fn read_rchar(pid: u32) -> Option<u64> {
    crate::platform::process_rchar(pid)
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
    let now = Instant::now();

    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let (active_now, confirmed) = hysteresis_step(map.get(&pid), current, now);

    map.insert(
        pid,
        IoState {
            prev_rchar: current,
            sampled_at: now,
            was_active: active_now,
        },
    );

    confirmed
}

// ---------------------------------------------------------------------------
// TCP socket detection (Codex/Gemini)
// ---------------------------------------------------------------------------

/// Check if a process has active HTTPS connections (TCP to remote port 443).
///
/// This is the primary activity signal for Gemini. These tools create TCP
/// connections to their API endpoints on demand and close them when idle at
/// the prompt. Any ESTABLISHED connection to port 443 means an API call
/// is in flight.
///
/// Delegates to platform-specific socket inspection APIs.
pub fn has_api_connections(pid: u32) -> bool {
    let socket_inodes = crate::platform::collect_socket_inodes(pid);
    crate::platform::has_established_443(pid, &socket_inodes)
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

    /// One poll against the real hysteresis step.
    fn poll(state: &mut Option<IoState>, rchar: u64, at: Instant) -> bool {
        let (active_now, confirmed) = hysteresis_step(state.as_ref(), rchar, at);
        *state = Some(IoState {
            prev_rchar: rchar,
            sampled_at: at,
            was_active: active_now,
        });
        confirmed
    }

    #[test]
    fn hysteresis_requires_two_consecutive_active_polls() {
        let start = Instant::now();
        let mut state = None;

        assert!(!poll(&mut state, 1000, start));
        // Poll 2: large delta but the previous poll was quiet → not confirmed.
        assert!(!poll(&mut state, 2000, start + Duration::from_millis(500)));
        // Poll 3: still busy AND the previous poll was busy → confirmed.
        assert!(poll(&mut state, 3000, start + Duration::from_millis(1000)));
    }

    #[test]
    fn single_spike_not_reported_as_active() {
        let start = Instant::now();
        let mut state = None;

        assert!(!poll(&mut state, 1000, start));
        assert!(!poll(&mut state, 2000, start + Duration::from_millis(500)));
        assert!(!poll(&mut state, 2010, start + Duration::from_millis(1000)));
    }

    // Regression: 9a66d1c compared a poll's raw rchar delta against a fixed
    // 500-byte threshold, but the scanner cadence flips between 500 ms and
    // 1500 ms (`daemon::session_activity::{ACTIVE,IDLE}_SCAN_INTERVAL`, dual
    // cadence since 3291970). On the 1500 ms cadence the same 500 bytes is a
    // third of the calibrated rate, so idle keep-alive traffic read as work.
    #[test]
    fn rchar_activity_is_normalised_to_bytes_per_second() {
        // The calibration point: 500 bytes per 500 ms poll.
        assert!(is_active_rate(500, Duration::from_millis(500)));
        assert!(!is_active_rate(499, Duration::from_millis(500)));

        // The same 500 bytes over the idle cadence is a third of the rate.
        assert!(!is_active_rate(500, Duration::from_millis(1500)));
        assert!(is_active_rate(1500, Duration::from_millis(1500)));
    }

    #[test]
    fn hysteresis_uses_the_actual_poll_interval() {
        let start = Instant::now();

        // 500 bytes twice on the 500 ms cadence: confirmed active.
        let mut fast = None;
        poll(&mut fast, 1_000, start);
        poll(&mut fast, 1_500, start + Duration::from_millis(500));
        assert!(poll(&mut fast, 2_000, start + Duration::from_millis(1000)));

        // The same byte counts on the 1500 ms cadence: still idle.
        let mut slow = None;
        poll(&mut slow, 1_000, start);
        poll(&mut slow, 1_500, start + Duration::from_millis(1500));
        assert!(!poll(&mut slow, 2_000, start + Duration::from_millis(3000)));
    }

    // -- has_api_connections --
    // (Socket inode parsing and TCP line parsing tests are in platform::linux::tests)

    #[test]
    fn has_api_connections_nonexistent_pid() {
        assert!(!has_api_connections(999_999_999));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn has_api_connections_current_process_no_https() {
        // The test runner shouldn't have any HTTPS connections to :443.
        // Linux-only: on macOS, lsof may pick up transient system connections
        // (TLS trust evaluation, DNS-over-HTTPS) belonging to the process.
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
                    sampled_at: Instant::now(),
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
