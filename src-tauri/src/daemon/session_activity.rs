use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::coordination::activity_export::{
    default_activity_export_teams_dir, export_activity_snapshots_for_sessions,
};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::DisplaySession;
use crate::session_scanner::SessionState;

/// Scanner cadence for daemon-owned session activity tracking.
const ACTIVE_SCAN_INTERVAL: Duration = Duration::from_millis(500);
const IDLE_SCAN_INTERVAL: Duration = Duration::from_millis(1500);
const IDLE_STABLE_CYCLES_THRESHOLD: u32 = 30;

/// Upper bound for long-poll wait time.
const MAX_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub version: u64,
    pub sessions: Vec<DisplaySession>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionUpdate {
    pub changed: bool,
    pub snapshot: SessionSnapshot,
}

#[derive(Default)]
struct HubState {
    initialized: bool,
    version: u64,
    sessions: Vec<DisplaySession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionEventSignature {
    pid: u32,
    project_path: String,
    tty: String,
    cli_tool: CliTool,
    tmux_session: Option<String>,
    tmux_window: Option<String>,
    tmux_pane: Option<String>,
    tmux_window_name: Option<String>,
    state: SessionState,
    project_unattributed_active: bool,
}

fn event_signature(session: &DisplaySession) -> SessionEventSignature {
    SessionEventSignature {
        pid: session.pid,
        project_path: session.project_path.clone(),
        tty: session.tty.clone(),
        cli_tool: session.cli_tool,
        tmux_session: session.tmux_session.clone(),
        tmux_window: session.tmux_window.clone(),
        tmux_pane: session.tmux_pane.clone(),
        tmux_window_name: session.tmux_window_name.clone(),
        state: session.state,
        project_unattributed_active: session.project_unattributed_active,
    }
}

fn activity_changed(prev: &[DisplaySession], next: &[DisplaySession]) -> bool {
    let mut prev_sig: Vec<SessionEventSignature> = prev.iter().map(event_signature).collect();
    let mut next_sig: Vec<SessionEventSignature> = next.iter().map(event_signature).collect();
    prev_sig.sort_by_key(|s| s.pid);
    next_sig.sort_by_key(|s| s.pid);
    prev_sig != next_sig
}

#[derive(Debug, Default)]
struct ScannerCadence {
    stable_idle_cycles: u32,
}

impl ScannerCadence {
    fn next_interval(&mut self, changed: bool, sessions: &[DisplaySession]) -> Duration {
        let all_idle = sessions.iter().all(|s| s.state == SessionState::Idle);

        if changed || !all_idle {
            self.stable_idle_cycles = 0;
            return ACTIVE_SCAN_INTERVAL;
        }

        self.stable_idle_cycles = self.stable_idle_cycles.saturating_add(1);
        if self.stable_idle_cycles >= IDLE_STABLE_CYCLES_THRESHOLD {
            IDLE_SCAN_INTERVAL
        } else {
            ACTIVE_SCAN_INTERVAL
        }
    }
}

/// Global daemon-owned session scanner with a versioned snapshot.
///
/// The scanner runs in a single background thread and updates the snapshot
/// whenever the session list changes. Consumers can read the current snapshot
/// or block until a newer version is available.
pub struct SessionActivityHub {
    state: Mutex<HubState>,
    changed_cv: Condvar,
    scanner_started: AtomicBool,
}

impl SessionActivityHub {
    fn new() -> Self {
        Self {
            state: Mutex::new(HubState::default()),
            changed_cv: Condvar::new(),
            scanner_started: AtomicBool::new(false),
        }
    }

    /// Return the global hub instance and ensure its scanner thread is running.
    pub fn global() -> Arc<Self> {
        static HUB: OnceLock<Arc<SessionActivityHub>> = OnceLock::new();
        let hub = HUB
            .get_or_init(|| Arc::new(SessionActivityHub::new()))
            .clone();
        hub.ensure_scanner_thread();
        hub
    }

    /// Get the latest snapshot immediately (non-blocking).
    pub fn snapshot(&self) -> SessionSnapshot {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        SessionSnapshot {
            version: guard.version,
            sessions: guard.sessions.clone(),
        }
    }

    /// Wait until a version newer than `since_version` exists, or timeout.
    ///
    /// If `timeout` is zero, returns immediately with `changed = false` unless
    /// a newer version is already available.
    pub fn wait_for_update(&self, since_version: u64, timeout: Duration) -> SessionUpdate {
        let wait_for = timeout.min(MAX_WAIT);
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if guard.version > since_version {
            return SessionUpdate {
                changed: true,
                snapshot: SessionSnapshot {
                    version: guard.version,
                    sessions: guard.sessions.clone(),
                },
            };
        }

        if wait_for.is_zero() {
            return SessionUpdate {
                changed: false,
                snapshot: SessionSnapshot {
                    version: guard.version,
                    sessions: guard.sessions.clone(),
                },
            };
        }

        let (guard, _timeout) = self
            .changed_cv
            .wait_timeout_while(guard, wait_for, |s| s.version <= since_version)
            .unwrap_or_else(|e| e.into_inner());

        let changed = guard.version > since_version;
        SessionUpdate {
            changed,
            snapshot: SessionSnapshot {
                version: guard.version,
                sessions: guard.sessions.clone(),
            },
        }
    }

    fn ensure_scanner_thread(self: &Arc<Self>) {
        if self
            .scanner_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let hub = Arc::clone(self);
        thread::spawn(move || {
            let mut next_tick = Instant::now();
            let mut cadence = ScannerCadence::default();

            loop {
                let sessions = crate::session_scanner::scan_sessions_for_display();
                let export_stats = export_activity_snapshots_for_sessions(
                    &default_activity_export_teams_dir(),
                    &sessions,
                    Utc::now(),
                );
                if export_stats.write_failures > 0 {
                    tracing::warn!(
                        teams_exported = export_stats.teams_exported,
                        members_written = export_stats.members_written,
                        write_failures = export_stats.write_failures,
                        "activity snapshot export completed with write failures"
                    );
                }
                let mut state = hub.state.lock().unwrap_or_else(|e| e.into_inner());

                let changed = !state.initialized || activity_changed(&state.sessions, &sessions);
                if changed {
                    state.sessions = sessions;
                    state.version = state.version.saturating_add(1);
                    state.initialized = true;
                    hub.changed_cv.notify_all();
                } else {
                    // Keep latest metadata without generating a new version/event.
                    state.sessions = sessions;
                }

                let interval = cadence.next_interval(changed, &state.sessions);
                next_tick += interval;
                let now = Instant::now();
                if next_tick > now {
                    thread::sleep(next_tick.duration_since(now));
                } else {
                    // Scanner fell behind; reset cadence from "now".
                    next_tick = now;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_state(state: SessionState) -> DisplaySession {
        DisplaySession {
            pid: 1,
            project_path: "/tmp/project".to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%1".to_string()),
            tmux_window_name: Some("project".to_string()),
            state,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: crate::session_scanner::ActivityConfidence::Low,
            activity_attribution: crate::session_scanner::ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: crate::session_scanner::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn cadence_stays_fast_for_active_sessions() {
        let mut cadence = ScannerCadence::default();
        let active = vec![session_with_state(SessionState::Active)];

        for _ in 0..40 {
            assert_eq!(cadence.next_interval(false, &active), ACTIVE_SCAN_INTERVAL);
        }
    }

    #[test]
    fn cadence_widens_after_stable_idle_threshold() {
        let mut cadence = ScannerCadence::default();
        let idle = vec![session_with_state(SessionState::Idle)];

        for _ in 0..(IDLE_STABLE_CYCLES_THRESHOLD - 1) {
            assert_eq!(cadence.next_interval(false, &idle), ACTIVE_SCAN_INTERVAL);
        }
        assert_eq!(cadence.next_interval(false, &idle), IDLE_SCAN_INTERVAL);
    }

    #[test]
    fn cadence_snaps_back_to_fast_on_any_change() {
        let mut cadence = ScannerCadence::default();
        let idle = vec![session_with_state(SessionState::Idle)];

        for _ in 0..IDLE_STABLE_CYCLES_THRESHOLD {
            let _ = cadence.next_interval(false, &idle);
        }
        assert_eq!(cadence.next_interval(false, &idle), IDLE_SCAN_INTERVAL);
        assert_eq!(cadence.next_interval(true, &idle), ACTIVE_SCAN_INTERVAL);
    }
}
