use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::coordination::activity_export::{
    enrich_runtime_sessions_with_team_membership, enrich_sessions_with_team_membership,
    export_activity_snapshots_for_sessions,
};
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::tmux::{self, TmuxFocusState};
use crate::session_scanner::SessionState;
use crate::session_scanner::{DisplaySession, RuntimeSession};

/// Scanner cadence for daemon-owned session activity tracking.
const ACTIVE_SCAN_INTERVAL: Duration = Duration::from_millis(500);
const IDLE_SCAN_INTERVAL: Duration = Duration::from_millis(1500);
const IDLE_STABLE_CYCLES_THRESHOLD: u32 = 30;
const ACTIVITY_EXPORT_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Upper bound for long-poll wait time.
const MAX_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub version: u64,
    pub sessions: Vec<DisplaySession>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSessionSnapshot {
    pub version: u64,
    pub display_sessions: Vec<DisplaySession>,
    pub runtime_sessions: Vec<RuntimeSession>,
    pub focus: Option<TmuxFocusState>,
    pub foreground_project_path: Option<String>,
    /// The latest scanner cycle was degraded: the sessions are the last good
    /// snapshot kept for continuity, not an observation.
    pub degraded: bool,
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
    display_sessions: Vec<DisplaySession>,
    runtime_sessions: Vec<RuntimeSession>,
    focus: Option<TmuxFocusState>,
    foreground_project_path: Option<String>,
    /// Set by a degraded cycle, cleared by the next healthy commit. Lives
    /// outside the versioned snapshot: a degraded cycle wakes no waiter.
    degraded: bool,
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

fn should_export_activity_snapshots(
    changed: bool,
    last_export_at: Option<Instant>,
    now: Instant,
) -> bool {
    changed
        || last_export_at
            .is_none_or(|last| now.duration_since(last) >= ACTIVITY_EXPORT_REFRESH_INTERVAL)
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
        self.hold_interval()
    }

    /// Current interval without advancing or resetting the idle streak.
    fn hold_interval(&self) -> Duration {
        if self.stable_idle_cycles >= IDLE_STABLE_CYCLES_THRESHOLD {
            IDLE_SCAN_INTERVAL
        } else {
            ACTIVE_SCAN_INTERVAL
        }
    }
}

/// One scanner cycle's inputs, as folded into the hub by `commit_cycle`.
struct ScanCycle {
    display_sessions: Vec<DisplaySession>,
    runtime_sessions: Vec<RuntimeSession>,
    focus: Option<TmuxFocusState>,
    foreground_project_path: Option<String>,
    /// The process inventory could not be read; the sessions are not an observation.
    degraded: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct CycleDecision {
    changed: bool,
    export_due: bool,
    interval: Duration,
}

/// Run one scanner cycle: scan, then enrich with team membership and read
/// the tmux focus state. A degraded scan produces an inert cycle.
fn scan_cycle(teams_dir: &Path) -> ScanCycle {
    let (mut display_sessions, mut runtime_sessions, degraded) =
        crate::session_scanner::scan_sessions_for_authoritative_snapshot();
    if degraded {
        return ScanCycle {
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus: None,
            foreground_project_path: None,
            degraded: true,
        };
    }

    enrich_sessions_with_team_membership(teams_dir, &mut display_sessions);
    enrich_runtime_sessions_with_team_membership(teams_dir, &mut runtime_sessions);
    let focus =
        tmux::read_focus_state(&crate::provider::platform_paths::PlatformPaths::app_data_root());
    let foreground_project_path = focus
        .as_ref()
        .and_then(|focus| tmux::resolve_focus_project_path(focus, &display_sessions));
    ScanCycle {
        display_sessions,
        runtime_sessions,
        focus,
        foreground_project_path,
        degraded: false,
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
            sessions: guard.display_sessions.clone(),
        }
    }

    /// Get the latest authoritative display/runtime snapshot immediately.
    pub fn runtime_snapshot(&self) -> RuntimeSessionSnapshot {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        RuntimeSessionSnapshot {
            version: guard.version,
            display_sessions: guard.display_sessions.clone(),
            runtime_sessions: guard.runtime_sessions.clone(),
            focus: guard.focus.clone(),
            foreground_project_path: guard.foreground_project_path.clone(),
            degraded: guard.degraded,
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
                    sessions: guard.display_sessions.clone(),
                },
            };
        }

        if wait_for.is_zero() {
            return SessionUpdate {
                changed: false,
                snapshot: SessionSnapshot {
                    version: guard.version,
                    sessions: guard.display_sessions.clone(),
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
                sessions: guard.display_sessions.clone(),
            },
        }
    }

    /// Fold one scan cycle into the hub state.
    ///
    /// Degraded cycles are inert: the previous snapshot stays, the version is
    /// not bumped, no export is due, and the cadence holds its interval. The
    /// preserved snapshot is marked degraded until the next healthy commit so
    /// consumers read it as continuity data, not as an observation.
    fn commit_cycle(
        &self,
        cycle: ScanCycle,
        cadence: &mut ScannerCadence,
        last_activity_export_at: Option<Instant>,
        now: Instant,
    ) -> CycleDecision {
        if cycle.degraded {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .degraded = true;
            return CycleDecision {
                changed: false,
                export_due: false,
                interval: cadence.hold_interval(),
            };
        }

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed = !state.initialized
            || activity_changed(&state.display_sessions, &cycle.display_sessions);
        // Keep latest metadata; only a change generates a new version/event.
        state.display_sessions = cycle.display_sessions;
        state.runtime_sessions = cycle.runtime_sessions;
        state.focus = cycle.focus;
        state.foreground_project_path = cycle.foreground_project_path;
        state.degraded = false;
        if changed {
            state.version = state.version.saturating_add(1);
            state.initialized = true;
            self.changed_cv.notify_all();
        }

        CycleDecision {
            changed,
            export_due: should_export_activity_snapshots(changed, last_activity_export_at, now),
            interval: cadence.next_interval(changed, &state.display_sessions),
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
            let mut last_activity_export_at: Option<Instant> = None;

            loop {
                let loop_started_at = Instant::now();
                let teams_dir = PlatformPaths::teams_dir();
                let cycle = scan_cycle(&teams_dir);

                let decision = hub.commit_cycle(
                    cycle,
                    &mut cadence,
                    last_activity_export_at,
                    loop_started_at,
                );
                if decision.export_due {
                    let export_stats = export_activity_snapshots_for_sessions(
                        &teams_dir,
                        &hub.snapshot().sessions,
                        Utc::now(),
                    );
                    last_activity_export_at = Some(loop_started_at);
                    if export_stats.write_failures > 0 {
                        tracing::warn!(
                            teams_exported = export_stats.teams_exported,
                            members_written = export_stats.members_written,
                            write_failures = export_stats.write_failures,
                            "activity snapshot export completed with write failures"
                        );
                    }
                }

                next_tick += decision.interval;
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
    use crate::session_scanner::idle::{set_binding_store_path_for_test, CODEX_TEST_LOCK};
    use crate::session_scanner::{clear_scan_cache, process, SCANNER_TEST_LOCK};
    use std::sync::atomic::AtomicU8;
    use tempfile::TempDir;

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

    fn cycle(display_sessions: Vec<DisplaySession>, degraded: bool) -> ScanCycle {
        ScanCycle {
            display_sessions,
            runtime_sessions: Vec::new(),
            focus: None,
            foreground_project_path: None,
            degraded,
        }
    }

    // Regression: latent since 9a66d1c. A timed-out `ps` produced an empty
    // session list; the hub treated it as a change, bumped the version (the UI
    // dropped every session icon), exported stall_no_active_process for every
    // team member and snapped the cadence back to fast. A degraded cycle must
    // leave the snapshot, version, export timer and cadence untouched.
    #[test]
    fn hub_does_not_bump_version_or_export_on_degraded() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let active = vec![session_with_state(SessionState::Active)];

        let first = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, None, now);
        assert!(first.changed);
        assert!(first.export_due);
        assert_eq!(hub.snapshot().version, 1);

        let export_overdue = Some(now - ACTIVITY_EXPORT_REFRESH_INTERVAL - Duration::from_secs(1));
        let degraded = hub.commit_cycle(
            cycle(Vec::new(), true),
            &mut cadence,
            export_overdue,
            now + Duration::from_secs(1),
        );
        assert_eq!(
            degraded,
            CycleDecision {
                changed: false,
                export_due: false,
                interval: ACTIVE_SCAN_INTERVAL,
            }
        );
        let snapshot = hub.snapshot();
        assert_eq!(snapshot.version, 1);
        assert_eq!(snapshot.sessions, active);
        assert_eq!(hub.runtime_snapshot().display_sessions, active);
        assert!(
            !hub.wait_for_update(1, Duration::ZERO).changed,
            "waiters must not wake on a degraded cycle"
        );

        // Control: a healthy empty scan is a real change.
        let emptied = hub.commit_cycle(
            cycle(Vec::new(), false),
            &mut cadence,
            Some(now),
            now + Duration::from_secs(2),
        );
        assert!(emptied.changed);
        assert!(emptied.export_due);
        assert_eq!(hub.snapshot().version, 2);
        assert!(hub.snapshot().sessions.is_empty());
    }

    // Regression: the hub preserved its last good sessions across degraded
    // cycles (correct for continuity) but exposed no degradation status, so
    // the daemon handed the cached runtime sessions to the Windows app as a
    // fresh observation and member identity detection could bind a stale
    // pane->transcript mapping. The snapshot reports `degraded` after a
    // degraded cycle and clears it on the next healthy commit.
    #[test]
    fn hub_reports_degraded_until_next_healthy_commit() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let active = vec![session_with_state(SessionState::Active)];

        assert!(
            !hub.runtime_snapshot().degraded,
            "fresh hub is not degraded"
        );

        let _ = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, None, now);
        assert!(!hub.runtime_snapshot().degraded);

        let _ = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        let snapshot = hub.runtime_snapshot();
        assert!(
            snapshot.degraded,
            "a degraded cycle must mark the preserved snapshot degraded"
        );
        assert_eq!(
            snapshot.display_sessions, active,
            "continuity: the last good sessions stay available"
        );
        assert_eq!(snapshot.version, 1, "degraded cycle still bumps no version");

        // A second degraded cycle keeps the flag.
        let _ = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        assert!(hub.runtime_snapshot().degraded);

        // The next healthy commit clears it, even without a session change.
        let healthy = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, Some(now), now);
        assert!(!healthy.changed);
        let snapshot = hub.runtime_snapshot();
        assert!(!snapshot.degraded, "healthy commit must clear degraded");
        assert_eq!(snapshot.version, 1);
    }

    #[test]
    fn cadence_holds_interval_across_degraded_cycle() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let idle = vec![session_with_state(SessionState::Idle)];

        let _ = hub.commit_cycle(cycle(idle.clone(), false), &mut cadence, None, now);
        for _ in 0..IDLE_STABLE_CYCLES_THRESHOLD {
            let _ = hub.commit_cycle(cycle(idle.clone(), false), &mut cadence, Some(now), now);
        }
        assert_eq!(cadence.hold_interval(), IDLE_SCAN_INTERVAL);

        let degraded = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        assert_eq!(degraded.interval, IDLE_SCAN_INTERVAL);
        let healthy = hub.commit_cycle(cycle(idle, false), &mut cadence, Some(now), now);
        assert!(!healthy.changed);
        assert_eq!(healthy.interval, IDLE_SCAN_INTERVAL);
    }

    const HUB_INVENTORY_HEALTHY: u8 = 0;
    const HUB_INVENTORY_FAILS: u8 = 1;
    const HUB_INVENTORY_EMPTY: u8 = 2;
    static HUB_INVENTORY_MODE: AtomicU8 = AtomicU8::new(HUB_INVENTORY_HEALTHY);
    const HUB_E2E_PID: u32 = 920_001;

    fn hub_inventory() -> Option<Vec<process::ProcessInfo>> {
        match HUB_INVENTORY_MODE.load(Ordering::SeqCst) {
            HUB_INVENTORY_FAILS => None,
            HUB_INVENTORY_EMPTY => Some(Vec::new()),
            _ => Some(vec![process::ProcessInfo {
                pid: HUB_E2E_PID,
                project_path: "/home/user/hub-e2e-project".to_string(),
                tty: "/dev/pts/9201".to_string(),
                args: "claude --continue".to_string(),
                cli_tool: CliTool::Claude,
            }]),
        }
    }

    // Regression: the scanner loop is the hub's only source of the degraded
    // flag; `hub_does_not_bump_version_or_export_on_degraded` injects the flag
    // directly. This drives the real scanner with an inventory source that
    // fails after one healthy read and asserts the hub stays inert.
    #[test]
    fn hub_ignores_degraded_cycle_from_real_scanner() {
        let _scanner = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _codex = CODEX_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let tmp = TempDir::new().expect("tempdir");
        set_binding_store_path_for_test(Some(tmp.path().join("codex-bindings.json")));
        clear_scan_cache();
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_HEALTHY, Ordering::SeqCst);
        process::set_inventory_provider_override(Some(hub_inventory));

        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();

        let healthy = hub.commit_cycle(scan_cycle(tmp.path()), &mut cadence, None, now);
        assert!(healthy.changed);
        assert!(healthy.export_due);
        let snapshot = hub.snapshot();
        assert_eq!(snapshot.version, 1);
        assert_eq!(
            snapshot
                .sessions
                .iter()
                .map(|session| session.pid)
                .collect::<Vec<_>>(),
            [HUB_E2E_PID]
        );

        HUB_INVENTORY_MODE.store(HUB_INVENTORY_FAILS, Ordering::SeqCst);
        let export_overdue = Some(now - ACTIVITY_EXPORT_REFRESH_INTERVAL - Duration::from_secs(1));
        let degraded = hub.commit_cycle(
            scan_cycle(tmp.path()),
            &mut cadence,
            export_overdue,
            now + Duration::from_secs(1),
        );
        assert_eq!(
            degraded,
            CycleDecision {
                changed: false,
                export_due: false,
                interval: ACTIVE_SCAN_INTERVAL,
            }
        );
        assert_eq!(hub.snapshot(), snapshot);
        assert!(!hub.wait_for_update(1, Duration::ZERO).changed);

        // Recovery: same sessions, no new version.
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_HEALTHY, Ordering::SeqCst);
        let recovered = hub.commit_cycle(scan_cycle(tmp.path()), &mut cadence, Some(now), now);
        assert!(!recovered.changed);
        assert_eq!(hub.snapshot().version, 1);

        // Teardown: a healthy empty scan prunes this test's trackers.
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_EMPTY, Ordering::SeqCst);
        let _ = scan_cycle(tmp.path());
        process::set_inventory_provider_override(None);
        set_binding_store_path_for_test(None);
        clear_scan_cache();
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_HEALTHY, Ordering::SeqCst);
    }

    #[test]
    fn activity_snapshot_export_runs_on_first_loop_even_without_change() {
        let now = Instant::now();
        assert!(should_export_activity_snapshots(false, None, now));
    }

    #[test]
    fn activity_snapshot_export_is_refreshed_after_interval_even_without_change() {
        let now = Instant::now();
        let last = now - ACTIVITY_EXPORT_REFRESH_INTERVAL - Duration::from_secs(1);
        assert!(should_export_activity_snapshots(false, Some(last), now));
    }

    #[test]
    fn activity_snapshot_export_skips_early_refresh_without_change() {
        let now = Instant::now();
        let last = now - ACTIVITY_EXPORT_REFRESH_INTERVAL + Duration::from_secs(1);
        assert!(!should_export_activity_snapshots(false, Some(last), now));
    }
}
