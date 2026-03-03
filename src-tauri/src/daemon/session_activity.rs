use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;
use crate::session_scanner::SessionState;

/// Scanner cadence for daemon-owned session activity tracking.
const SCAN_INTERVAL: Duration = Duration::from_millis(500);

/// Upper bound for long-poll wait time.
const MAX_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub version: u64,
    pub sessions: Vec<ClaudeSession>,
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
    sessions: Vec<ClaudeSession>,
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

fn event_signature(session: &ClaudeSession) -> SessionEventSignature {
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

fn activity_changed(prev: &[ClaudeSession], next: &[ClaudeSession]) -> bool {
    let mut prev_sig: Vec<SessionEventSignature> = prev.iter().map(event_signature).collect();
    let mut next_sig: Vec<SessionEventSignature> = next.iter().map(event_signature).collect();
    prev_sig.sort_by_key(|s| s.pid);
    next_sig.sort_by_key(|s| s.pid);
    prev_sig != next_sig
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

            loop {
                let sessions = crate::session_scanner::scan_sessions();
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

                next_tick += SCAN_INTERVAL;
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
