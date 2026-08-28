//! Process activity tracker — detects whether a CLI tool process is doing
//! meaningful work using `/proc` signals.
//!
//! Per-process read activity is tracked with consecutive-poll hysteresis.
//!
//! **Claude Code** — consecutive-poll hysteresis via `/proc/PID/io`.
//! Claude writes almost continuously during streaming/thinking, producing
//! sustained rchar deltas of 900+ bytes/500ms. Two consecutive above-threshold
//! polls reliably confirm activity while filtering single-sample spikes from
//! focus events or tmux switching.
//!
//! **Codex** — per-PID IO hysteresis + project file mtime fallback.
//! Codex maintains HTTP keep-alive connections to :443 indefinitely after
//! finishing work, making TCP socket presence useless as an idle indicator.
//! We therefore use `/proc/PID/io` hysteresis per process to distinguish
//! which Codex session is actively doing work. Project-level session file
//! mtime (from `idle.rs`) remains a fallback for single-session projects.
//!
//! Empirically confirmed (Feb 2026):
//! - Claude idle: 0-240 bytes/500ms keepalive in rchar
//! - Claude thinking: 900+ bytes/500ms sustained in rchar

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Minimum sustained read rate (bytes/second) that counts as activity.
///
/// Calibrated as the original 500 bytes per 500 ms poll: 2x margin above
/// Claude's idle noise (0-240 bytes/500ms) and far below the smallest
/// normal harness bursts (7K+). Expressed as a rate because the scanner cadence
/// is not fixed — it flips between 500 ms and 1500 ms
/// (`daemon::session_activity::{ACTIVE_SCAN_INTERVAL, IDLE_SCAN_INTERVAL}`).
const ACTIVE_IO_RATE_BYTES_PER_SEC: u64 = 1_000;

/// Shortest gap that counts as a new sample.
///
/// The scanner's own cadence is 500 ms at its fastest, but the app-side
/// fallback (`scan_sessions_for_display`, one classification per IPC call)
/// can poll the same PID milliseconds apart. Dividing an idle keep-alive read
/// by a few milliseconds turns it into tens of kB/s, so a sample that close
/// carries no new information and the stored one is kept.
const MIN_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

/// Per-PID tracking state for IO activity detection (Claude only).
struct IoState {
    /// Previous rchar value (for computing delta).
    prev_rchar: u64,
    /// When `prev_rchar` was sampled, so the delta can be turned into a rate.
    sampled_at: Instant,
    /// Whether the PREVIOUS poll showed activity above threshold.
    was_active: bool,
    /// What that poll answered, replayed for sub-cadence polls.
    confirmed: bool,
}

/// Outcome of one poll.
enum Poll {
    /// A fresh sample; store it and answer `confirmed`.
    Sampled { active_now: bool, confirmed: bool },
    /// Closer than `MIN_SAMPLE_INTERVAL`: keep the stored sample and repeat its
    /// answer.
    TooSoon(bool),
}

/// Whether an rchar delta observed over `elapsed` clears the activity rate.
fn is_active_rate(delta_bytes: u64, elapsed: Duration) -> bool {
    let elapsed_ms = u64::try_from(elapsed.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    delta_bytes.saturating_mul(1_000) / elapsed_ms >= ACTIVE_IO_RATE_BYTES_PER_SEC
}

/// One hysteresis step for a fresh rchar reading.
fn hysteresis_step(previous: Option<&IoState>, current: u64, now: Instant) -> Poll {
    match previous {
        Some(state) => {
            let elapsed = now.saturating_duration_since(state.sampled_at);
            if elapsed < MIN_SAMPLE_INTERVAL {
                return Poll::TooSoon(state.confirmed);
            }
            let delta = current.saturating_sub(state.prev_rchar);
            let active_now = is_active_rate(delta, elapsed);
            Poll::Sampled {
                active_now,
                confirmed: active_now && state.was_active,
            }
        }
        None => Poll::Sampled {
            active_now: false,
            confirmed: false,
        },
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

    match hysteresis_step(map.get(&pid), current, now) {
        Poll::TooSoon(confirmed) => confirmed,
        Poll::Sampled {
            active_now,
            confirmed,
        } => {
            map.insert(
                pid,
                IoState {
                    prev_rchar: current,
                    sampled_at: now,
                    was_active: active_now,
                    confirmed,
                },
            );
            confirmed
        }
    }
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

    /// One poll against the real hysteresis step, storing what it stores.
    fn poll(state: &mut Option<IoState>, rchar: u64, at: Instant) -> bool {
        match hysteresis_step(state.as_ref(), rchar, at) {
            Poll::TooSoon(confirmed) => confirmed,
            Poll::Sampled {
                active_now,
                confirmed,
            } => {
                *state = Some(IoState {
                    prev_rchar: rchar,
                    sampled_at: at,
                    was_active: active_now,
                    confirmed,
                });
                confirmed
            }
        }
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

    // Regression: c9669ef turned the fixed per-poll threshold into a rate but
    // put no floor under `elapsed`, so two classifications of the same PID a
    // few ms apart (the app-side `scan_sessions_for_display` fallback runs one
    // per IPC call) divided an idle 240-byte keep-alive read by ~5 ms and read
    // it as 48 kB/s of work. A sample that close carries no new information.
    #[test]
    fn sub_cadence_poll_does_not_amplify_noise() {
        let start = Instant::now();
        let mut state = None;

        assert!(!poll(&mut state, 1_000, start));
        // Idle keep-alive traffic on the real cadence: 100 bytes / 500 ms.
        assert!(!poll(&mut state, 1_100, start + Duration::from_millis(500)));
        // Two extra polls 5 ms apart: without a floor each 240-byte read is
        // 48 kB/s, and the second one confirms "active".
        assert!(!poll(&mut state, 1_340, start + Duration::from_millis(505)));
        assert!(!poll(&mut state, 1_580, start + Duration::from_millis(510)));
        // The next real poll still measures against the 500 ms sample: 480
        // bytes over 500 ms is 960 B/s, below the rate.
        assert!(!poll(
            &mut state,
            1_580,
            start + Duration::from_millis(1000)
        ));
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
                    confirmed: false,
                },
            );
        }

        retain_pids(&[1]);

        let guard = IO_STATE.lock().unwrap();
        let map = guard.as_ref().unwrap();
        assert!(!map.contains_key(&777_777));
    }
}
