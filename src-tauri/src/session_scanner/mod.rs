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

pub mod cli_tool;
pub mod compaction_extractor;
pub mod compaction_watcher;
pub mod control;
pub mod idle;
pub mod proc_io;
pub mod process;
pub mod tmux;

#[cfg(test)]
pub(crate) use cache::{
    clear_scan_cache, set_display_scan_compaction_hook, state_tracker_snapshot,
};
pub use cache::{latest_compaction_runtime_sessions, notify_tmux_changed};
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
