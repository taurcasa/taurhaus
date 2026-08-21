//! Claude Code sessions registry — authoritative session identity and activity.
//!
//! Claude Code writes one JSON file per live session at
//! `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` and rewrites it on every status
//! change. The record carries the fields the scanner otherwise has to guess:
//! `sessionId`, `cwd`, `tmux` and a `status` the session sets about itself.
//!
//! Measured on this host (spike S1, Claude Code 2.1.238, 2026-08-21):
//!
//! - Observed statuses: `busy`, `idle`, `waiting` (a tool-permission prompt is
//!   pending). `shell` is documented by the plan but was not observed.
//! - The file is **edge-driven**: `updatedAt`/`statusUpdatedAt` advance only on
//!   a status change, and an idle session's file can be hours old. Staleness is
//!   therefore *not* evidence of anything and is never used here as an expiry.
//! - The rewrite lands 11–64 ms after the embedded `updatedAt`, so a scanner
//!   poll sees a transition within one cadence tick.
//! - The record appears only after the workspace-trust prompt is accepted and
//!   is removed when the session exits cleanly.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::provider::path::normalize_project_path;
use crate::session_scanner::SessionState;

use super::{age_secs_since_mtime, file_mtime, path_to_slug, IdleResult};

/// Directory holding the per-PID registry records.
const SESSIONS_SUBDIR: &str = "sessions";

/// Transcript directory under the same config root.
const PROJECTS_SUBDIR: &str = "projects";

/// First Claude Code release that writes the sessions registry.
///
/// Older releases leave the directory absent; the heuristic resolvers stay in
/// charge for them (`IdleResult::authoritative == false`).
const MIN_REGISTRY_VERSION: (u32, u32, u32) = (2, 1, 219);

/// One `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` record.
///
/// Unknown fields are ignored: Claude Code adds keys across releases.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(super) struct RegistryEntry {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub cwd: String,
    pub version: String,
    #[serde(default)]
    pub status: Option<String>,
    /// `<session>:@<window>.<pane>` of the pane the session runs in.
    #[serde(default)]
    pub tmux: Option<String>,
    /// Peer display name (`-n <agent_name>` or derived).
    #[serde(default)]
    pub name: Option<String>,
}

/// Registry records directory for a config root.
pub(super) fn sessions_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(SESSIONS_SUBDIR)
}

/// Read and parse `<config_dir>/sessions/<pid>.json`.
pub(super) fn read_entry(config_dir: &Path, pid: u32) -> Option<RegistryEntry> {
    let path = sessions_dir(config_dir).join(format!("{pid}.json"));
    let raw = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<RegistryEntry>(&raw) {
        Ok(entry) => Some(entry),
        Err(error) => {
            tracing::debug!(
                path = %path.display(),
                error = %error,
                "unparsable Claude sessions registry record"
            );
            None
        }
    }
}

/// Whether a registry record's `version` is new enough to be trusted.
pub(super) fn version_supported(version: &str) -> bool {
    parse_version(version).is_some_and(|parsed| parsed >= MIN_REGISTRY_VERSION)
}

fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts.next()?.trim().parse().ok()?;
    let patch_digits: String = parts
        .next()?
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    Some((major, minor, patch_digits.parse().ok()?))
}

/// Map a registry `status` onto a display state.
///
/// `waiting` is a pending tool-permission prompt (S1): the session is blocked
/// on the human, which is idle as far as the display is concerned. `shell` is
/// the user shelled out of the agent loop. An unrecognised status returns
/// `None` so the caller falls back to the heuristics instead of guessing.
pub(super) fn state_for_status(status: &str) -> Option<SessionState> {
    match status {
        "busy" => Some(SessionState::Active),
        "idle" | "waiting" | "shell" => Some(SessionState::Idle),
        _ => None,
    }
}

/// Authoritative idle result for `pid`, or `None` when the registry cannot
/// answer (absent, too old, another project, unknown status).
pub(super) fn detect_idle_from_registry(
    project_path: &str,
    pid: u32,
    config_dir: &Path,
) -> Option<IdleResult> {
    let entry = read_entry(config_dir, pid)?;

    if !version_supported(&entry.version) {
        tracing::debug!(
            pid,
            version = %entry.version,
            "Claude sessions registry record predates the supported version"
        );
        return None;
    }

    if normalize_project_path(&entry.cwd) != normalize_project_path(project_path) {
        tracing::debug!(
            pid,
            registry_cwd = %entry.cwd,
            project_path,
            "Claude sessions registry record belongs to another project"
        );
        return None;
    }

    let status = entry.status.as_deref()?;
    let state = state_for_status(status)?;

    let transcript = transcript_path(config_dir, &entry);
    let transcript_mtime = transcript.as_deref().and_then(file_mtime);

    Some(IdleResult {
        state,
        session_id: Some(entry.session_id),
        jsonl_path: transcript.map(|path| path.to_string_lossy().to_string()),
        last_output_age_secs: transcript_mtime.map(age_secs_since_mtime),
        authoritative: true,
    })
}

/// Transcript for a registry record, under the same config root.
///
/// This is the half of the fix the user sees: sessions launched with
/// `CLAUDE_CONFIG_DIR=~/.claude-account2` keep their transcripts there, so
/// looking under `~/.claude/projects` never finds them.
fn transcript_path(config_dir: &Path, entry: &RegistryEntry) -> Option<PathBuf> {
    let path = config_dir
        .join(PROJECTS_SUBDIR)
        .join(path_to_slug(&entry.cwd))
        .join(format!("{}.jsonl", entry.session_id));
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const PROJECT: &str = "/home/user/projects/taurhaus";
    const SESSION_ID: &str = "f3286b16-ffc7-4d16-915d-046705823a3d";

    fn write_record(config_dir: &Path, pid: u32, status: &str, version: &str) {
        write_record_for(config_dir, pid, status, version, PROJECT);
    }

    fn write_record_for(config_dir: &Path, pid: u32, status: &str, version: &str, cwd: &str) {
        let dir = sessions_dir(config_dir);
        fs::create_dir_all(&dir).unwrap();
        let record = format!(
            r#"{{"pid":{pid},"sessionId":"{SESSION_ID}","cwd":"{cwd}","startedAt":1787254157495,"version":"{version}","peerProtocol":1,"kind":"interactive","tmux":"taurhaus:@3.%3","name":"taurhaus-00","status":"{status}","updatedAt":1787327562655,"statusUpdatedAt":1787327562655}}"#
        );
        fs::write(dir.join(format!("{pid}.json")), record).unwrap();
    }

    fn write_transcript(config_dir: &Path, cwd: &str) {
        let dir = config_dir.join(PROJECTS_SUBDIR).join(path_to_slug(cwd));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{SESSION_ID}.jsonl")), "{}\n").unwrap();
    }

    #[test]
    fn busy_status_is_authoritative_active() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "busy", "2.1.237");
        write_transcript(tmp.path(), PROJECT);

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Active);
        assert!(result.authoritative);
        assert_eq!(result.session_id.as_deref(), Some(SESSION_ID));
        assert!(result
            .jsonl_path
            .as_deref()
            .is_some_and(|path| path.ends_with(&format!("{SESSION_ID}.jsonl"))));
    }

    #[test]
    fn idle_status_is_authoritative_idle() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "idle", "2.1.238");

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
    }

    #[test]
    fn shell_status_is_authoritative_idle() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "shell", "2.1.238");

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
    }

    // S1: a pending tool-permission prompt is its own status value. It reads as
    // idle today (the session is blocked on the human); a dedicated display
    // state is a separate product decision.
    #[test]
    fn waiting_on_permission_status_is_authoritative_idle() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "waiting", "2.1.238");

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
    }

    #[test]
    fn missing_record_is_not_authoritative() {
        let tmp = TempDir::new().unwrap();
        assert!(detect_idle_from_registry(PROJECT, 27051, tmp.path()).is_none());
    }

    #[test]
    fn record_older_than_the_supported_version_is_ignored() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "busy", "2.1.218");

        assert!(detect_idle_from_registry(PROJECT, 27051, tmp.path()).is_none());
        assert!(version_supported("2.1.219"));
        assert!(version_supported("2.1.238"));
        assert!(version_supported("2.2.0"));
        assert!(!version_supported("2.1.99"));
        assert!(!version_supported("2.0.999"));
        assert!(!version_supported("not-a-version"));
    }

    #[test]
    fn unknown_status_falls_back_to_heuristics() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "hibernating", "2.1.238");

        assert!(detect_idle_from_registry(PROJECT, 27051, tmp.path()).is_none());
        assert_eq!(state_for_status("busy"), Some(SessionState::Active));
        assert_eq!(state_for_status("nonsense"), None);
    }

    #[test]
    fn record_for_another_project_is_ignored() {
        let tmp = TempDir::new().unwrap();
        write_record_for(
            tmp.path(),
            27051,
            "busy",
            "2.1.238",
            "/home/user/projects/mesh",
        );

        assert!(detect_idle_from_registry(PROJECT, 27051, tmp.path()).is_none());
    }

    #[test]
    fn missing_transcript_still_yields_an_authoritative_state() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "busy", "2.1.238");

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Active);
        assert!(result.authoritative);
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn parses_the_live_record_shape() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "busy", "2.1.237");

        let entry = read_entry(tmp.path(), 27051).unwrap();

        assert_eq!(entry.session_id, SESSION_ID);
        assert_eq!(entry.cwd, PROJECT);
        assert_eq!(entry.tmux.as_deref(), Some("taurhaus:@3.%3"));
        assert_eq!(entry.name.as_deref(), Some("taurhaus-00"));
    }
}
