//! Session scanner — detects CLI tool sessions running in tmux.
//!
//! Combines three detection strategies:
//! 1. Process scanning (`/proc` on Linux, `ps` on macOS; fail-soft) — find supported CLI tool processes and project paths
//! 2. tmux mapping — map terminal TTYs to tmux pane/window IDs
//! 3. Idle detection — check tool-specific transcript/runtime signals to determine active vs idle
//!
//! State changes use bidirectional hysteresis: a transition (idle↔active)
//! only takes effect after 2 consecutive polls agree on the new state.
//! This eliminates flickering from transient signals in either direction.
//!
//! Warning:
//! - `DisplaySession` is the UI-safe view and intentionally strips transcript
//!   metadata such as `session_id` and `jsonl_path`.
//! - Coordination and other transcript-aware logic must use
//!   `RuntimeSession` via `scan_sessions_for_runtime()`.

mod cache;
mod classification;
mod daemon;
mod scans;
mod types;

pub mod accounts;
pub mod cli_tool;
pub mod control;
pub mod idle;
pub mod launch;
pub mod proc_io;
pub mod process;
pub mod tmux;
pub mod transcript_boundary;

#[cfg(test)]
pub(crate) use cache::{clear_scan_cache, state_tracker_snapshot};
pub use cache::{latest_runtime_sessions, notify_tmux_changed};
pub use cli_tool::CliTool;
pub use scans::{
    scan_sessions_for_authoritative_snapshot, scan_sessions_for_display, scan_sessions_for_runtime,
    scan_sessions_with,
};
pub use types::{
    ActivityAttribution, ActivityConfidence, DisplaySession, RuntimeSession, SessionGroupKind,
    SessionState,
};

/// Serializes tests that drive the scanner's process-global state (scan
/// cache, last-good inventories, hysteresis trackers, test overrides).
#[cfg(test)]
pub(crate) static SCANNER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Capture the `activity.state.changed` events one scan or classification
/// sequence emits.
///
/// The structured emitter is process-global, so the sink and the tap are
/// installed under the shared global-log guard and torn down on drop.
#[cfg(test)]
pub(crate) struct StateChangeCapture {
    _log_guard: crate::test_support::GlobalLogTestGuard,
    _sink_dir: tempfile::TempDir,
    _sink: crate::commands::logging::LogFileState,
    events: std::sync::mpsc::Receiver<serde_json::Value>,
}

#[cfg(test)]
impl StateChangeCapture {
    pub(crate) fn install() -> Self {
        let log_guard = crate::test_support::acquire_global_log_test_guard();
        let sink_dir = tempfile::tempdir().expect("temp dir");
        let sink =
            crate::commands::logging::LogFileState::new(sink_dir.path().join("activity.log.jsonl"))
                .expect("log state");
        crate::commands::logging::install_global_sink(&sink);
        let (sender, events) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(sender);
        Self {
            _log_guard: log_guard,
            _sink_dir: sink_dir,
            _sink: sink,
            events,
        }
    }

    /// Every `(from, to)` pair emitted for `pid`, in emission order.
    pub(crate) fn transitions_for(&self, pid: u32) -> Vec<(Option<SessionState>, SessionState)> {
        self.events
            .try_iter()
            .filter(|event| {
                event["event"] == "activity.state.changed" && event["fields"]["pid"] == pid
            })
            .map(|event| {
                (
                    serde_json::from_value(event["fields"]["from"].clone()).expect("from state"),
                    serde_json::from_value(event["fields"]["to"].clone()).expect("to state"),
                )
            })
            .collect()
    }
}

#[cfg(test)]
impl Drop for StateChangeCapture {
    fn drop(&mut self) {
        crate::commands::logging::clear_test_tap();
    }
}
