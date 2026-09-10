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
//! `/proc/PID/io` hysteresis per process detects sustained turn IO; which
//! session a process IS comes from identity resolution (`idle/codex.rs`), not
//! from IO, because an idle 0.153.4 prompt already reads in bursts (below).
//! Project-level session file mtime (from `idle.rs`) remains a fallback for
//! single-session projects.
//!
//! Empirically confirmed (Feb 2026):
//! - Claude idle: 0-240 bytes/500ms keepalive in rchar
//! - Claude thinking: 900+ bytes/500ms sustained in rchar
//!
//! Codex 0.153.4 (2026-09-10, isolated zero-turn pane, 500 ms samples):
//! loaded-prompt background startup peaked at 779,043 B; after 11 s, median
//! 416 B, isolated peak 123,392 B and a 46,848/22,784 B adjacent pair.
//! Use 32 KiB/s for FOUR consecutive polls for Codex only: two polls of headroom
//! over the measured adjacent pair (2026-09-10 review). This rejects the
//! settled noise at both scanner cadences; pane readiness also overrides startup
//! IO. No real turn was captured: 64 KiB/s sustained is a synthetic margin check,
//! not a measured turn floor. Recheck both cadences on the next paid turn lane;
//! smaller turns retain the transcript/notify signals.

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

/// Codex's timer/file polling has a different noise floor from Claude's.
const CODEX_ACTIVE_IO_RATE: u64 = 32 * 1024;
#[derive(Default)]
struct CodexIoState {
    previous: Option<(u64, Instant)>,
    active_polls: u8,
}
impl CodexIoState {
    fn sample(&mut self, current: Option<u64>, now: Instant) -> bool {
        let Some(current) = current else {
            *self = Self::default();
            return false;
        };
        if let Some((previous, at)) = self.previous {
            let elapsed = now.saturating_duration_since(at);
            if elapsed < MIN_SAMPLE_INTERVAL {
                return self.active_polls >= 4;
            }
            let ms = u64::try_from(elapsed.as_millis())
                .unwrap_or(u64::MAX)
                .max(1);
            let active =
                current.saturating_sub(previous).saturating_mul(1000) / ms >= CODEX_ACTIVE_IO_RATE;
            self.active_polls = if active {
                self.active_polls.saturating_add(1).min(4)
            } else {
                0
            };
        }
        self.previous = Some((current, now));
        self.active_polls >= 4
    }
}
static CODEX_IO_STATE: Mutex<Option<HashMap<u32, CodexIoState>>> = Mutex::new(None);
#[cfg(test)]
thread_local! {
    pub(super) static CODEX_TEST_SAMPLE: std::cell::Cell<Option<(u32, u64, Instant)>> = const { std::cell::Cell::new(None) };
}
pub fn is_codex_process_active_hysteresis(pid: u32) -> bool {
    #[cfg(not(test))]
    let (current, now) = (read_rchar(pid), Instant::now());
    #[cfg(test)]
    let (current, now) = CODEX_TEST_SAMPLE
        .get()
        .filter(|(sample_pid, _, _)| *sample_pid == pid)
        .map(|(_, value, at)| (Some(value), at))
        .unwrap_or_else(|| (read_rchar(pid), Instant::now()));
    CODEX_IO_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .entry(pid)
        .or_default()
        .sample(current, now)
}

/// Remove stale PIDs from the IO tracker that are no longer in the active set.
pub fn retain_pids(active_pids: &[u32]) {
    let mut guard = IO_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
    if let Some(map) = CODEX_IO_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
    {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

// Measured 2026-09-10, Codex 0.153.4: samples.json in the readiness evidence packet.
#[cfg(test)]
pub(super) const CODEX_IDLE_RCHAR_DELTAS: [u64; 120] = [
    345950, 403395, 174959, 173327, 232116, 172943, 288249, 115306, 173391, 288633, 230740, 779043,
    152330, 173615, 596258, 60805, 115562, 57989, 57701, 57925, 482264, 4288, 256, 1280, 128, 384,
    1824, 27328, 800, 256, 192, 224, 123392, 256, 64, 960, 256, 128, 576, 256, 224, 640, 256, 896,
    64, 768, 64, 1088, 9216, 46848, 22784, 896, 1856, 576, 128, 64, 416, 224, 544, 608, 224, 288,
    320, 448, 256, 128, 128, 1024, 992, 448, 64, 5824, 320, 448, 1152, 320, 128, 320, 64, 64, 288,
    64, 704, 416, 64, 128, 1216, 576, 576, 256, 9984, 18880, 1120, 1664, 1664, 64, 416, 224, 1088,
    480, 224, 1280, 64, 448, 128, 960, 384, 64, 64, 10496, 7520, 224, 4741, 10784, 1344, 2144, 256,
    64, 512, 2592,
];

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: 6398bfa3 (#163), L2 run 3: Claude's IO calibration made
    // Codex's idle composer flap. Replay the measured settled-prompt tail for 90 s.
    #[test]
    fn codex_idle_prompt_measurement_filters_bursts_and_confirms_work() {
        for stride in [1, 3] {
            let start = Instant::now();
            let mut state = CodexIoState::default();
            let mut rchar = 1000;
            assert!(!state.sample(Some(rchar), start));
            let deltas: Vec<_> = CODEX_IDLE_RCHAR_DELTAS[22..]
                .iter()
                .cycle()
                .take(180)
                .collect();
            for (tick, chunk) in deltas.chunks(stride).enumerate() {
                rchar += chunk.iter().copied().sum::<u64>();
                assert!(
                    !state.sample(
                        Some(rchar),
                        start + Duration::from_millis(((tick + 1) * stride * 500) as u64)
                    ),
                    "idle burst at tick {tick}, stride {stride}"
                );
            }
            for tick in 1..=4 {
                rchar += 32768 * stride as u64; // Synthetic 64 KiB/s, not a model turn.
                assert_eq!(
                    state.sample(
                        Some(rchar),
                        start + Duration::from_millis(90000 + tick * stride as u64 * 500)
                    ),
                    tick == 4
                );
            }
            let end = start + Duration::from_millis(90000 + 4 * stride as u64 * 500);
            assert!(state.sample(Some(rchar + 256), end + Duration::from_millis(5)));
            assert!(!state.sample(None, end + Duration::from_millis(10)));
            assert!(!state.sample(Some(1), end + Duration::from_millis(500)));
        }
    }

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

        {
            let mut guard = CODEX_IO_STATE.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(777_777, CodexIoState::default());
        }
        retain_pids(&[1]);
        let codex = CODEX_IO_STATE.lock().unwrap();
        assert!(!codex.as_ref().unwrap().contains_key(&777_777));
        drop(codex);

        let guard = IO_STATE.lock().unwrap();
        let map = guard.as_ref().unwrap();
        assert!(!map.contains_key(&777_777));
    }
}
