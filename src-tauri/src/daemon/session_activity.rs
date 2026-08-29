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
use crate::session_scanner::tmux::{self, TmuxFocus};
use crate::session_scanner::{ActivityAttribution, ActivityConfidence, SessionState};
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
    pub account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    pub focus: Option<TmuxFocus>,
    pub focus_project_path: Option<String>,
    /// The latest scanner cycle was degraded: the sessions are the last good
    /// snapshot kept for continuity, not an observation.
    pub degraded: bool,
    /// Blackout-edge counter — see `SessionUpdate::degraded_revision`.
    pub degraded_revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionUpdate {
    pub changed: bool,
    pub snapshot: SessionSnapshot,
    /// tmux focus as of `snapshot.version`, read under the same lock.
    pub focus: Option<TmuxFocus>,
    pub focus_project_path: Option<String>,
    pub account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    /// The sessions in `snapshot` are the last good ones, kept for continuity
    /// while the scanner is blind — not an observation.
    pub degraded: bool,
    /// Monotonic count of degradation *edges* (healthy→degraded and back).
    ///
    /// A degraded cycle bumps no version — continuity data must not pass for a
    /// new authoritative snapshot — so this is the cursor that carries a
    /// blackout to the app: a waiter wakes when it moves, and a caller whose
    /// cursor is behind knows the interval it just spanned was not observed,
    /// even when the blackout began and ended inside one wait.
    pub degraded_revision: u64,
}

#[derive(Default)]
struct HubState {
    initialized: bool,
    version: u64,
    display_sessions: Vec<DisplaySession>,
    runtime_sessions: Vec<RuntimeSession>,
    account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    focus: Option<TmuxFocus>,
    focus_project_path: Option<String>,
    /// Set by a degraded cycle, cleared by the next healthy commit. Lives
    /// outside the versioned snapshot: a degraded cycle bumps no version.
    degraded: bool,
    /// Bumped on every `degraded` edge — the wait cursor for blackouts.
    degraded_revision: u64,
}

/// The activity half of the hub's change signature.
///
/// Confidence and attribution are part of it because the app presents them
/// (`src/lib/activitySignal.js`); `recent_io`, `last_output_age_secs`, and raw
/// workflow write times are deliberately excluded — they flip per poll and
/// would defeat change-gating.
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
    activity_confidence: ActivityConfidence,
    activity_attribution: ActivityAttribution,
    project_unattributed_active: bool,
    workflow_live_runs: Option<u32>,
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
        activity_confidence: session.activity_confidence,
        activity_attribution: session.activity_attribution,
        project_unattributed_active: session.project_unattributed_active,
        workflow_live_runs: session
            .workflow_activity
            .as_ref()
            .map(|activity| activity.live_runs),
    }
}

/// The focus half of the hub's change signature.
///
/// A focus-only move (window, pane, or the project it resolves to) is a real
/// change: it bumps the version and wakes the app's long poll.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusSignature {
    session: Option<String>,
    window_index: Option<String>,
    pane_id: Option<String>,
    project_path: Option<String>,
}

fn focus_signature(focus: Option<&TmuxFocus>, project_path: Option<&str>) -> FocusSignature {
    FocusSignature {
        session: focus.map(|focus| focus.session.clone()),
        window_index: focus.map(|focus| focus.window_index.clone()),
        pane_id: focus.map(|focus| focus.pane_id.clone()),
        project_path: project_path.map(str::to_string),
    }
}

fn activity_changed(prev: &[DisplaySession], next: &[DisplaySession]) -> bool {
    let mut prev_sig: Vec<SessionEventSignature> = prev.iter().map(event_signature).collect();
    let mut next_sig: Vec<SessionEventSignature> = next.iter().map(event_signature).collect();
    prev_sig.sort_by_key(|s| s.pid);
    next_sig.sort_by_key(|s| s.pid);
    prev_sig != next_sig
}

/// Activity export is due on an activity change or the periodic refresh.
/// A focus move is not activity: it writes no member activity file.
fn should_export_activity_snapshots(
    activity_moved: bool,
    last_export_at: Option<Instant>,
    now: Instant,
) -> bool {
    activity_moved
        || last_export_at
            .is_none_or(|last| now.duration_since(last) >= ACTIVITY_EXPORT_REFRESH_INTERVAL)
}

#[derive(Debug, Default)]
struct ScannerCadence {
    stable_idle_cycles: u32,
}

impl ScannerCadence {
    fn next_interval(
        &mut self,
        changed: bool,
        sessions: &[DisplaySession],
        focused: bool,
    ) -> Duration {
        let all_idle = sessions.iter().all(|s| s.state == SessionState::Idle);

        if changed || focused || !all_idle {
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
    account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    focus: Option<TmuxFocus>,
    focus_project_path: Option<String>,
    /// Some client reports its window as focused: someone is looking.
    focused: bool,
    /// The process inventory could not be read; the sessions are not an observation.
    degraded: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct CycleDecision {
    changed: bool,
    export_due: bool,
    interval: Duration,
}

/// Run one scanner cycle: scan, then enrich with team membership and probe
/// tmux for the focused client. A degraded scan produces an inert cycle: no
/// tmux probe, and the hub keeps its last known focus.
fn scan_cycle(teams_dir: &Path) -> ScanCycle {
    let (mut display_sessions, mut runtime_sessions, degraded) =
        crate::session_scanner::scan_sessions_for_authoritative_snapshot();
    if degraded {
        return ScanCycle {
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            account_observations: Vec::new(),
            focus: None,
            focus_project_path: None,
            focused: false,
            degraded: true,
        };
    }

    let account_observations =
        crate::session_scanner::accounts::observe_live_session_accounts(&runtime_sessions);
    enrich_sessions_with_team_membership(teams_dir, &mut display_sessions);
    enrich_runtime_sessions_with_team_membership(teams_dir, &mut runtime_sessions);
    let clients = tmux::list_clients();
    let focus = tmux::focus_from_clients(&clients);
    let focus_project_path = focus
        .as_ref()
        .and_then(|focus| tmux::resolve_focus_project_path(focus, &display_sessions));
    ScanCycle {
        display_sessions,
        runtime_sessions,
        account_observations,
        focus,
        focus_project_path,
        focused: tmux::any_client_focused(&clients),
        degraded: false,
    }
}

fn session_update(state: &HubState, changed: bool) -> SessionUpdate {
    SessionUpdate {
        changed,
        snapshot: SessionSnapshot {
            version: state.version,
            sessions: state.display_sessions.clone(),
        },
        focus: state.focus.clone(),
        focus_project_path: state.focus_project_path.clone(),
        account_observations: state.account_observations.clone(),
        degraded: state.degraded,
        degraded_revision: state.degraded_revision,
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
            account_observations: guard.account_observations.clone(),
            focus: guard.focus.clone(),
            focus_project_path: guard.focus_project_path.clone(),
            degraded: guard.degraded,
            degraded_revision: guard.degraded_revision,
        }
    }

    /// Wait until a newer snapshot version or a newer degradation revision
    /// exists, or timeout.
    ///
    /// Two cursors, because the hub has two kinds of news. A new version means
    /// the sessions changed; a new degradation revision means the scanner went
    /// blind or came back, which changes nothing about the sessions but does
    /// change whether they are an observation. `changed` reports the first
    /// only — waking on a blackout must never look like a fresh snapshot.
    ///
    /// If `timeout` is zero, returns immediately with `changed = false` unless
    /// a newer version is already available.
    pub fn wait_for_update(
        &self,
        since_version: u64,
        since_degraded_revision: u64,
        timeout: Duration,
    ) -> SessionUpdate {
        let wait_for = timeout.min(MAX_WAIT);
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if guard.version > since_version {
            return session_update(&guard, true);
        }

        if wait_for.is_zero() || guard.degraded_revision > since_degraded_revision {
            return session_update(&guard, false);
        }

        let (guard, _timeout) = self
            .changed_cv
            .wait_timeout_while(guard, wait_for, |s| {
                s.version <= since_version && s.degraded_revision <= since_degraded_revision
            })
            .unwrap_or_else(|e| e.into_inner());

        let changed = guard.version > since_version;
        session_update(&guard, changed)
    }

    /// Fold one scan cycle into the hub state.
    ///
    /// Degraded cycles are inert: the previous snapshot stays, the version is
    /// not bumped, no export is due, and the cadence holds its interval. The
    /// preserved snapshot is marked degraded until the next healthy commit so
    /// consumers read it as continuity data, not as an observation.
    ///
    /// Both degradation edges do bump `degraded_revision` and wake waiters:
    /// the sessions are unchanged, but whether they are an observation is not,
    /// and that is news the app cannot wait 20 s for.
    fn commit_cycle(
        &self,
        cycle: ScanCycle,
        cadence: &mut ScannerCadence,
        last_activity_export_at: Option<Instant>,
        now: Instant,
    ) -> CycleDecision {
        if cycle.degraded {
            {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if !state.degraded {
                    state.degraded = true;
                    state.degraded_revision = state.degraded_revision.saturating_add(1);
                    self.changed_cv.notify_all();
                }
            }
            return CycleDecision {
                changed: false,
                export_due: false,
                interval: cadence.hold_interval(),
            };
        }

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Both halves version the snapshot, but only activity is worth an
        // export: a focus move touches no member's activity file.
        let activity_moved = !state.initialized
            || activity_changed(&state.display_sessions, &cycle.display_sessions);
        let focus_moved =
            focus_signature(state.focus.as_ref(), state.focus_project_path.as_deref())
                != focus_signature(cycle.focus.as_ref(), cycle.focus_project_path.as_deref());
        let changed = activity_moved || focus_moved;
        // Keep latest metadata; only a change generates a new version/event.
        state.display_sessions = cycle.display_sessions;
        state.runtime_sessions = cycle.runtime_sessions;
        // Account observations are throttled scanner output, so an empty
        // cycle means "nothing new", not "forget the last binding". Keep the
        // latest observation while its project/tool still has a live session;
        // this gives the app's long poll a stable delivery window.
        let live_account_keys = state
            .runtime_sessions
            .iter()
            .map(|session| {
                (
                    crate::provider::path::normalize_project_path(&session.project_path),
                    session.cli_tool,
                )
            })
            .collect::<std::collections::HashSet<_>>();
        state.account_observations.retain(|observation| {
            live_account_keys.contains(&(
                crate::provider::path::normalize_project_path(&observation.project_path),
                observation.tool,
            ))
        });
        for observation in cycle.account_observations {
            state.account_observations.retain(|current| {
                current.tool != observation.tool
                    || crate::provider::path::normalize_project_path(&current.project_path)
                        != crate::provider::path::normalize_project_path(&observation.project_path)
            });
            state.account_observations.push(observation);
        }
        state.focus = cycle.focus;
        state.focus_project_path = cycle.focus_project_path;
        let recovered = state.degraded;
        state.degraded = false;
        if recovered {
            state.degraded_revision = state.degraded_revision.saturating_add(1);
        }
        if changed {
            state.version = state.version.saturating_add(1);
            state.initialized = true;
        }
        if changed || recovered {
            self.changed_cv.notify_all();
        }

        CycleDecision {
            changed,
            export_due: should_export_activity_snapshots(
                activity_moved,
                last_activity_export_at,
                now,
            ),
            interval: cadence.next_interval(changed, &state.display_sessions, cycle.focused),
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
    use std::sync::atomic::{AtomicU8, AtomicUsize};
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
            activity_confidence: ActivityConfidence::Low,
            activity_attribution: ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: crate::session_scanner::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: None,
        }
    }

    #[test]
    fn cadence_stays_fast_for_active_sessions() {
        let mut cadence = ScannerCadence::default();
        let active = vec![session_with_state(SessionState::Active)];

        for _ in 0..40 {
            assert_eq!(
                cadence.next_interval(false, &active, false),
                ACTIVE_SCAN_INTERVAL
            );
        }
    }

    #[test]
    fn cadence_widens_after_stable_idle_threshold() {
        let mut cadence = ScannerCadence::default();
        let idle = vec![session_with_state(SessionState::Idle)];

        for _ in 0..(IDLE_STABLE_CYCLES_THRESHOLD - 1) {
            assert_eq!(
                cadence.next_interval(false, &idle, false),
                ACTIVE_SCAN_INTERVAL
            );
        }
        assert_eq!(
            cadence.next_interval(false, &idle, false),
            IDLE_SCAN_INTERVAL
        );
    }

    #[test]
    fn cadence_snaps_back_to_fast_on_any_change() {
        let mut cadence = ScannerCadence::default();
        let idle = vec![session_with_state(SessionState::Idle)];

        for _ in 0..IDLE_STABLE_CYCLES_THRESHOLD {
            let _ = cadence.next_interval(false, &idle, false);
        }
        assert_eq!(
            cadence.next_interval(false, &idle, false),
            IDLE_SCAN_INTERVAL
        );
        assert_eq!(
            cadence.next_interval(true, &idle, false),
            ACTIVE_SCAN_INTERVAL
        );
    }

    fn cycle(display_sessions: Vec<DisplaySession>, degraded: bool) -> ScanCycle {
        ScanCycle {
            display_sessions,
            runtime_sessions: Vec::new(),
            account_observations: Vec::new(),
            focus: None,
            focus_project_path: None,
            focused: false,
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
            !hub.wait_for_update(1, 0, Duration::ZERO).changed,
            "a degraded cycle is never a session change (it does wake waiters \
             on its own revision — see the blackout tests below)"
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

    // Regression: 6c6f1cb taught the app to present a `degraded` record as
    // uncertain, but the hub only exposed the flag on `runtime_snapshot()` —
    // the long poll the session bridge lives on never carried it. A blind
    // scanner therefore left the last good indicator green indefinitely.
    // The flag rides the long-poll answer without bumping the version:
    // continuity data stays continuity data (see the test above).
    #[test]
    fn a_long_poll_answer_reports_the_degraded_flag() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let active = vec![session_with_state(SessionState::Active)];

        let _ = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, None, now);
        let healthy = hub.wait_for_update(0, 0, Duration::ZERO);
        assert!(healthy.changed);
        assert!(!healthy.degraded);

        let _ = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        let blind = hub.wait_for_update(1, 0, Duration::ZERO);
        assert!(
            !blind.changed,
            "a degraded cycle bumps no version, so it is never a change"
        );
        assert!(
            blind.degraded,
            "the long poll must report that the sessions it carries are not an observation"
        );
        assert_eq!(blind.snapshot.sessions, active, "continuity data is kept");

        let _ = hub.commit_cycle(cycle(active, false), &mut cadence, Some(now), now);
        assert!(!hub.wait_for_update(1, 0, Duration::ZERO).degraded);
    }

    // Regression: fa572d4 gave `degraded` a ride on the long-poll answer, but
    // the answer only comes back when the version moves or the bridge's 20 s
    // budget expires, and a degraded cycle bumps no version. A blackout that
    // started and ended inside one outstanding wait was therefore invisible:
    // the answer carried `degraded: false` and the app credited the blind
    // interval to whatever it last saw. Degradation edges now carry their own
    // revision, so a caller whose cursor is behind learns that the interval it
    // is about to measure was not observed — without the retained sessions
    // ever being promoted to a new authoritative snapshot.
    #[test]
    fn a_blackout_inside_one_wait_still_reaches_the_next_answer() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let active = vec![session_with_state(SessionState::Active)];

        let _ = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, None, now);
        let seed = hub.wait_for_update(0, 0, Duration::ZERO);
        assert!(seed.changed);
        assert_eq!(seed.degraded_revision, 0);

        // Both edges land while the app is parked in one long poll.
        let _ = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        let _ = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, Some(now), now);

        let answer = hub.wait_for_update(
            seed.snapshot.version,
            seed.degraded_revision,
            Duration::ZERO,
        );
        assert!(!answer.changed, "the session list itself did not move");
        assert!(!answer.degraded, "the scanner is healthy again");
        assert_eq!(answer.snapshot.version, seed.snapshot.version);
        assert_eq!(
            answer.degraded_revision, 2,
            "one bump per edge: went blind, came back"
        );
        assert!(
            answer.degraded_revision > seed.degraded_revision,
            "the caller must be able to see that its interval was not observed"
        );

        // Exactly once: nothing new to report at the caller's new cursor.
        let settled = hub.wait_for_update(
            answer.snapshot.version,
            answer.degraded_revision,
            Duration::ZERO,
        );
        assert!(!settled.changed);
        assert_eq!(settled.degraded_revision, 2);
    }

    // Regression: fa572d4 — same defect from the waiter's side. `wait_for_update`
    // blocked on the version alone, so neither edge of a scanner blackout woke
    // the bridge's long poll; the app learned about a blackout at most once every
    // 20 s, and only if it was still blind by then.
    #[test]
    fn a_parked_waiter_wakes_on_each_degradation_edge() {
        use std::sync::mpsc;

        let hub = Arc::new(SessionActivityHub::new());
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let active = vec![session_with_state(SessionState::Active)];
        let _ = hub.commit_cycle(cycle(active.clone(), false), &mut cadence, None, now);

        let (tx, rx) = mpsc::channel();
        let waiter_hub = Arc::clone(&hub);
        let waiter = thread::spawn(move || {
            let mut version = 1;
            let mut degraded_revision = 0;
            for _ in 0..2 {
                let update =
                    waiter_hub.wait_for_update(version, degraded_revision, Duration::from_secs(5));
                version = update.snapshot.version;
                degraded_revision = update.degraded_revision;
                if tx.send(update).is_err() {
                    return;
                }
            }
        });

        let _ = hub.commit_cycle(cycle(Vec::new(), true), &mut cadence, Some(now), now);
        let went_blind = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the waiter must wake when the scanner goes blind");
        assert!(went_blind.degraded);
        assert!(!went_blind.changed, "a blackout is not a session change");
        assert_eq!(went_blind.snapshot.version, 1);
        assert_eq!(went_blind.degraded_revision, 1);

        let _ = hub.commit_cycle(cycle(active, false), &mut cadence, Some(now), now);
        let recovered = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the waiter must wake when the scanner comes back");
        assert!(!recovered.degraded);
        assert!(!recovered.changed, "the same sessions are not a change");
        assert_eq!(recovered.snapshot.version, 1);
        assert_eq!(recovered.degraded_revision, 2);

        waiter.join().unwrap();
        assert_eq!(
            hub.wait_for_update(1, 2, Duration::from_millis(50))
                .degraded_revision,
            2,
            "each edge is reported exactly once"
        );
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
    //
    // Commit 07ab6c5 put a live `tmux list-clients` probe inside `scan_cycle`,
    // so this test started reading the developer's own tmux server: a window
    // switch between the healthy and the recovery cycle moved the focus half of
    // the change signature and failed `!recovered.changed` at random. The tmux
    // probe is scripted here for the same reason the inventory is.
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
        crate::session_scanner::tmux::set_list_clients_override(Some(scripted_clients));

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
        assert!(!hub.wait_for_update(1, 0, Duration::ZERO).changed);

        // Recovery: same sessions, no new version.
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_HEALTHY, Ordering::SeqCst);
        let recovered = hub.commit_cycle(scan_cycle(tmp.path()), &mut cadence, Some(now), now);
        assert!(!recovered.changed);
        assert_eq!(hub.snapshot().version, 1);

        // Teardown: a healthy empty scan prunes this test's trackers.
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_EMPTY, Ordering::SeqCst);
        let _ = scan_cycle(tmp.path());
        crate::session_scanner::tmux::set_list_clients_override(None);
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

    fn focus_at(session: &str, window_index: &str, pane_id: &str) -> TmuxFocus {
        TmuxFocus {
            session: session.to_string(),
            window_index: window_index.to_string(),
            pane_id: pane_id.to_string(),
        }
    }

    fn focus_cycle(
        display_sessions: Vec<DisplaySession>,
        focus: Option<TmuxFocus>,
        focus_project_path: Option<&str>,
    ) -> ScanCycle {
        ScanCycle {
            display_sessions,
            runtime_sessions: Vec::new(),
            account_observations: Vec::new(),
            focus,
            focus_project_path: focus_project_path.map(str::to_string),
            focused: false,
            degraded: false,
        }
    }

    // Regression: 3f0d541 built `SessionEventSignature` from the coarse
    // state alone, so a confidence or attribution downgrade on the same PID
    // never bumped the version and never reached the app — which now presents
    // those fields (`src/lib/activitySignal.js`).
    #[test]
    fn an_activity_confidence_change_is_a_change() {
        let previous = vec![session_with_state(SessionState::Active)];
        let mut next = previous.clone();
        next[0].activity_confidence = ActivityConfidence::High;

        assert!(activity_changed(&previous, &next));
    }

    #[test]
    fn an_activity_attribution_change_is_a_change() {
        let previous = vec![session_with_state(SessionState::Active)];
        let mut next = previous.clone();
        next[0].activity_attribution = ActivityAttribution::Attributed;

        assert!(activity_changed(&previous, &next));
    }

    // `recent_io` and `last_output_age_secs` flip on nearly every poll; both
    // stay out of the signature so change-gating keeps working.
    #[test]
    fn a_recent_io_or_output_age_flip_alone_is_not_a_change() {
        let previous = vec![session_with_state(SessionState::Active)];
        let mut next = previous.clone();
        next[0].recent_io = !next[0].recent_io;
        next[0].last_output_age_secs = Some(7);

        assert!(!activity_changed(&previous, &next));
    }

    // Regression: e2c4041 put the raw workflow transcript mtime in the hub
    // signature, so each subagent append woke the long poll and re-ran the
    // per-member activity export at the 500 ms scan cadence.
    #[test]
    fn a_workflow_write_with_the_same_live_run_count_is_not_a_change() {
        let mut previous = vec![session_with_state(SessionState::Active)];
        previous[0].workflow_activity = Some(crate::workflow_runs::WorkflowActivity {
            live_runs: 2,
            last_write_at: 1_800_000_000_100,
        });
        let mut next = previous.clone();
        next[0].workflow_activity = Some(crate::workflow_runs::WorkflowActivity {
            live_runs: 2,
            last_write_at: 1_800_000_000_900,
        });

        assert!(!activity_changed(&previous, &next));

        next[0].workflow_activity = Some(crate::workflow_runs::WorkflowActivity {
            live_runs: 3,
            last_write_at: 1_800_000_000_900,
        });
        assert!(activity_changed(&previous, &next));

        next[0].workflow_activity = None;
        assert!(activity_changed(&previous, &next));
    }

    // Regression: commits a53ad31 and f9c1e89. Focus travelled through tmux
    // hooks into a file the app watched, and the hub never versioned it, so a
    // focus-only change woke no waiter. Focus is a hub-owned snapshot field.
    #[test]
    fn hub_bumps_version_and_wakes_waiters_on_a_focus_only_change() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let sessions = vec![session_with_state(SessionState::Active)];

        let first = hub.commit_cycle(
            focus_cycle(sessions.clone(), None, None),
            &mut cadence,
            None,
            now,
        );
        assert!(first.changed);
        assert_eq!(hub.snapshot().version, 1);

        let focused = hub.commit_cycle(
            focus_cycle(
                sessions.clone(),
                Some(focus_at("taurhaus", "1", "%1")),
                Some("/tmp/project"),
            ),
            &mut cadence,
            Some(now),
            now,
        );
        assert!(focused.changed, "a focus-only change must bump the version");
        assert_eq!(hub.snapshot().version, 2);
        assert!(
            hub.wait_for_update(1, 0, Duration::ZERO).changed,
            "waiters must wake on a focus-only change"
        );
        let snapshot = hub.runtime_snapshot();
        assert_eq!(snapshot.focus, Some(focus_at("taurhaus", "1", "%1")));
        assert_eq!(
            snapshot.focus_project_path,
            Some("/tmp/project".to_string())
        );

        let repeat = hub.commit_cycle(
            focus_cycle(
                sessions.clone(),
                Some(focus_at("taurhaus", "1", "%1")),
                Some("/tmp/project"),
            ),
            &mut cadence,
            Some(now),
            now,
        );
        assert!(!repeat.changed, "unchanged focus must not bump the version");
        assert_eq!(hub.snapshot().version, 2);

        let moved_pane = hub.commit_cycle(
            focus_cycle(
                sessions.clone(),
                Some(focus_at("taurhaus", "1", "%2")),
                Some("/tmp/project"),
            ),
            &mut cadence,
            Some(now),
            now,
        );
        assert!(
            moved_pane.changed,
            "the pane is part of the focus signature"
        );
        assert_eq!(hub.snapshot().version, 3);

        let cleared = hub.commit_cycle(
            focus_cycle(sessions, None, None),
            &mut cadence,
            Some(now),
            now,
        );
        assert!(cleared.changed, "losing focus is a change");
        assert_eq!(hub.runtime_snapshot().focus, None);
    }

    // Regression: commit 07ab6c5 folded focus into the same `changed` flag that
    // decides `export_due`, so every tmux focus switch — up to twice a second —
    // re-ran roster loading, per-member tmux probes and activity-file writes.
    // Focus is not activity: the export is due on an activity change or the
    // 30 s refresh, nothing else.
    #[test]
    fn a_focus_only_change_does_not_trigger_an_activity_export() {
        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();
        let sessions = vec![session_with_state(SessionState::Active)];
        let focus = Some(focus_at("taurhaus", "1", "%1"));

        let first = hub.commit_cycle(
            focus_cycle(sessions.clone(), None, None),
            &mut cadence,
            None,
            now,
        );
        assert!(first.export_due, "the first cycle always exports");

        let moved = hub.commit_cycle(
            focus_cycle(sessions.clone(), focus.clone(), Some("/tmp/project")),
            &mut cadence,
            Some(now),
            now + Duration::from_secs(1),
        );
        assert!(moved.changed, "a focus-only move is still a version bump");
        assert!(!moved.export_due, "focus is not activity");

        // Control: an activity change still exports on the same cycle.
        let mut idle = sessions.clone();
        idle[0].state = SessionState::Idle;
        let activity = hub.commit_cycle(
            focus_cycle(idle.clone(), focus.clone(), Some("/tmp/project")),
            &mut cadence,
            Some(now),
            now + Duration::from_secs(2),
        );
        assert!(activity.changed);
        assert!(activity.export_due, "an activity change exports");

        // Control: the 30 s refresh still fires with nothing changed at all.
        let refresh = hub.commit_cycle(
            focus_cycle(idle, focus, Some("/tmp/project")),
            &mut cadence,
            Some(now - ACTIVITY_EXPORT_REFRESH_INTERVAL - Duration::from_secs(1)),
            now,
        );
        assert!(!refresh.changed);
        assert!(refresh.export_due, "the periodic refresh is unaffected");
    }

    #[test]
    fn cadence_holds_the_active_interval_while_a_client_is_focused() {
        let mut cadence = ScannerCadence::default();
        let idle = vec![session_with_state(SessionState::Idle)];

        for _ in 0..(IDLE_STABLE_CYCLES_THRESHOLD + 5) {
            assert_eq!(
                cadence.next_interval(false, &idle, true),
                ACTIVE_SCAN_INTERVAL,
                "someone is looking; hold the 500 ms cadence"
            );
        }

        for _ in 0..IDLE_STABLE_CYCLES_THRESHOLD {
            let _ = cadence.next_interval(false, &idle, false);
        }
        assert_eq!(
            cadence.next_interval(false, &idle, false),
            IDLE_SCAN_INTERVAL
        );
    }

    static CLIENT_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn scripted_clients() -> Vec<crate::session_scanner::tmux::TmuxClient> {
        CLIENT_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
        vec![crate::session_scanner::tmux::TmuxClient {
            flags: vec!["attached".to_string(), "focused".to_string()],
            session: "taurhaus".to_string(),
            window_index: "1".to_string(),
            pane_id: "%1".to_string(),
            activity: 100,
        }]
    }

    // Regression: latent since 9a66d1c (degraded scans) — a degraded cycle must
    // stay inert on the focus path too: no tmux probe, no focus mutation.
    #[test]
    fn degraded_cycle_does_not_probe_or_alter_focus() {
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
        crate::session_scanner::tmux::set_list_clients_override(Some(scripted_clients));
        CLIENT_PROBE_CALLS.store(0, Ordering::SeqCst);

        let hub = SessionActivityHub::new();
        let mut cadence = ScannerCadence::default();
        let now = Instant::now();

        let healthy = hub.commit_cycle(scan_cycle(tmp.path()), &mut cadence, None, now);
        assert!(healthy.changed);
        assert_eq!(CLIENT_PROBE_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(
            hub.runtime_snapshot().focus,
            Some(focus_at("taurhaus", "1", "%1"))
        );

        HUB_INVENTORY_MODE.store(HUB_INVENTORY_FAILS, Ordering::SeqCst);
        let degraded_cycle = scan_cycle(tmp.path());
        assert!(degraded_cycle.degraded);
        assert!(degraded_cycle.focus.is_none());
        assert_eq!(
            CLIENT_PROBE_CALLS.load(Ordering::SeqCst),
            1,
            "a degraded cycle must not probe tmux"
        );
        let degraded = hub.commit_cycle(degraded_cycle, &mut cadence, Some(now), now);
        assert!(!degraded.changed);
        assert_eq!(
            hub.runtime_snapshot().focus,
            Some(focus_at("taurhaus", "1", "%1")),
            "a degraded cycle must leave the last known focus in place"
        );

        HUB_INVENTORY_MODE.store(HUB_INVENTORY_EMPTY, Ordering::SeqCst);
        let _ = scan_cycle(tmp.path());
        crate::session_scanner::tmux::set_list_clients_override(None);
        process::set_inventory_provider_override(None);
        set_binding_store_path_for_test(None);
        clear_scan_cache();
        HUB_INVENTORY_MODE.store(HUB_INVENTORY_HEALTHY, Ordering::SeqCst);
    }
}
