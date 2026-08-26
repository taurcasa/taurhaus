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
//! - The record appears only after the workspace-trust prompt is accepted. It
//!   is removed on a clean exit, but an unclean one leaves it behind — this
//!   host holds six records for long-dead PIDs — so `procStart` (the writing
//!   process's `/proc/<pid>/stat` field 22) is checked as a PID-reuse guard.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::provider::path::normalize_project_path;
use crate::session_scanner::SessionState;

use super::{
    age_secs_since_mtime, classify_mtime, file_mtime, most_recent_mtime, newest_file_mtime,
    path_to_slug, ActivitySource, IdleResult, ACTIVE_THRESHOLD,
};

pub(super) struct ClaudeRegistryActivitySource<'a> {
    pub config_dir: &'a Path,
}

impl ActivitySource for ClaudeRegistryActivitySource<'_> {
    fn activity(
        &self,
        project_path: &str,
        pid: u32,
        _resolved: Option<&IdleResult>,
    ) -> Option<IdleResult> {
        detect_idle_from_registry(project_path, pid, self.config_dir)
    }
}

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
    /// Start time of the PID that wrote the record, in `/proc/<pid>/stat`
    /// field-22 clock ticks. A string on every observed release; kept as a raw
    /// JSON value so a future numeric spelling cannot make the whole record
    /// unparsable (which would drop the session entirely).
    #[serde(default, rename = "procStart")]
    pub proc_start: Option<serde_json::Value>,
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
/// `None` — the state then comes from the heuristic instead of a guess, while
/// the record's identity is kept (see `detect_idle_from_registry`).
pub(super) fn state_for_status(status: &str) -> Option<SessionState> {
    match status {
        "busy" => Some(SessionState::Active),
        "idle" | "waiting" | "shell" => Some(SessionState::Idle),
        _ => None,
    }
}

/// Idle result for `pid` from its own registry record, or `None` when the
/// registry cannot answer at all (absent, too old, another project, a record
/// left behind by a dead session that held this PID before).
///
/// Identity and activity are separate answers. A record that passes the gates
/// is PID-specific proof of *which* session this process runs, so its
/// `sessionId` and transcript are used whatever the status says. Only a status
/// this build understands makes the *state* authoritative; for anything else
/// the state falls back to the transcript heuristic with
/// `authoritative: false`, which is what the plan asks for and what keeps a
/// second pane in the same project from being handed this session's identity.
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

    if !proc_start_matches(pid, entry.proc_start.as_ref()) {
        tracing::debug!(
            pid,
            session_id = %entry.session_id,
            "Claude sessions registry record was written by an earlier process with this PID"
        );
        return None;
    }

    let transcript = transcript_path(config_dir, &entry);
    let latest_output = latest_output_mtime(config_dir, &entry, transcript.as_deref());

    let reported = entry.status.as_deref().and_then(state_for_status);
    if reported.is_none() {
        tracing::debug!(
            pid,
            status = entry.status.as_deref().unwrap_or("<missing>"),
            "unrecognised Claude sessions registry status; identity kept, state from the heuristic"
        );
    }

    let heuristic_state = latest_output
        .map(|mtime| classify_mtime(mtime, ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    Some(IdleResult {
        state: reported.unwrap_or(heuristic_state),
        session_id: Some(entry.session_id),
        jsonl_path: transcript.map(|path| path.to_string_lossy().to_string()),
        last_output_age_secs: latest_output.map(age_secs_since_mtime),
        authoritative: reported.is_some(),
    })
}

/// Whether the live process is the one that wrote the record.
///
/// Claude Code stores the writing process's start time (`/proc/<pid>/stat`
/// field 22) as `procStart`; records outlive unclean exits, and PIDs wrap, so
/// without this check a fresh session in the same project could inherit a dead
/// one's identity and frozen status. Unknown on either side — an older record,
/// a platform without the value — means "cannot disprove", and the record is
/// used.
fn proc_start_matches(pid: u32, recorded: Option<&serde_json::Value>) -> bool {
    let Some(recorded) = recorded.and_then(json_u64) else {
        return true;
    };
    let Some(actual) = crate::platform::process_start_ticks(pid) else {
        return true;
    };
    recorded == actual
}

/// `procStart` as a number, whether the record spells it as a string or a JSON
/// number.
fn json_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::String(text) => text.trim().parse().ok(),
        serde_json::Value::Number(number) => number.as_u64(),
        _ => None,
    }
}

/// Newest write by this session: its transcript, or a compaction subagent
/// writing under it while the transcript is quiet.
fn latest_output_mtime(
    config_dir: &Path,
    entry: &RegistryEntry,
    transcript: Option<&Path>,
) -> Option<std::time::SystemTime> {
    let subagents = config_dir
        .join(PROJECTS_SUBDIR)
        .join(path_to_slug(&entry.cwd))
        .join(&entry.session_id)
        .join("subagents");
    most_recent_mtime(
        transcript.and_then(file_mtime),
        newest_file_mtime(&subagents),
    )
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

/// The config root a transcript belongs to — `transcript_path` read backwards.
///
/// A Claude transcript is always `<config dir>/projects/<slug>/<id>.jsonl`, so
/// the account that owns a session is readable from the path alone. Anything
/// that does not have that shape returns `None` rather than a guess.
pub fn config_dir_for_transcript(transcript: &Path) -> Option<PathBuf> {
    let slug_dir = transcript.parent()?;
    let projects_dir = slug_dir.parent()?;
    if projects_dir.file_name()? != PROJECTS_SUBDIR {
        return None;
    }
    projects_dir.parent().map(Path::to_path_buf)
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

    // Regression: c9669ef returned `None` for a status this build does not
    // know, throwing away the record's PID-correct `sessionId` with it. The
    // caller then fell back to "newest transcript in the project", which in a
    // two-pane project is another pane's session. Identity is the record's
    // whatever the status says; only the *activity* stops being authoritative.
    #[test]
    fn unknown_status_keeps_the_identity_but_is_not_authoritative() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "hibernating", "2.1.238");
        write_transcript(tmp.path(), PROJECT);

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert!(!result.authoritative);
        assert_eq!(result.session_id.as_deref(), Some(SESSION_ID));
        assert!(result
            .jsonl_path
            .as_deref()
            .is_some_and(|path| path.ends_with(&format!("{SESSION_ID}.jsonl"))));
        // The fresh transcript is the only signal left, so the state is the
        // mtime heuristic — not a guess about what "hibernating" means.
        assert_eq!(result.state, SessionState::Active);

        assert_eq!(state_for_status("busy"), Some(SessionState::Active));
        assert_eq!(state_for_status("nonsense"), None);
    }

    #[test]
    fn missing_status_keeps_the_identity_but_is_not_authoritative() {
        let tmp = TempDir::new().unwrap();
        let dir = sessions_dir(tmp.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("27051.json"),
            format!(
                r#"{{"pid":27051,"sessionId":"{SESSION_ID}","cwd":"{PROJECT}","version":"2.1.238"}}"#
            ),
        )
        .unwrap();

        let result = detect_idle_from_registry(PROJECT, 27051, tmp.path()).unwrap();

        assert!(!result.authoritative);
        assert_eq!(result.session_id.as_deref(), Some(SESSION_ID));
        assert_eq!(result.state, SessionState::Idle);
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

    fn write_record_with_proc_start(config_dir: &Path, pid: u32, proc_start: &str) {
        let dir = sessions_dir(config_dir);
        fs::create_dir_all(&dir).unwrap();
        let record = format!(
            r#"{{"pid":{pid},"sessionId":"{SESSION_ID}","cwd":"{PROJECT}","procStart":{proc_start},"version":"2.1.238","status":"busy"}}"#
        );
        fs::write(dir.join(format!("{pid}.json")), record).unwrap();
    }

    // Regression: c9669ef adopted any `<pid>.json` whose version and cwd
    // matched, although every supported record carries `procStart` (verified
    // live: it equals `/proc/<pid>/stat` field 22) precisely as a PID-reuse
    // guard. Records outlive unclean exits — `~/.claude/sessions/` on this host
    // held six records for long-dead PIDs — and PIDs wrap here about every
    // 1.5 days, so a fresh `claude` landing on a stale PID in the same cwd
    // would inherit a dead session's id and frozen status, authoritatively.
    #[cfg(target_os = "linux")]
    #[test]
    fn record_with_foreign_proc_start_is_ignored() {
        let tmp = TempDir::new().unwrap();
        let pid = std::process::id();
        write_record_with_proc_start(tmp.path(), pid, r#""1""#);

        assert!(detect_idle_from_registry(PROJECT, pid, tmp.path()).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn record_with_the_processes_own_proc_start_is_used() {
        let tmp = TempDir::new().unwrap();
        let pid = std::process::id();
        let ticks = crate::platform::process_start_ticks(pid).expect("own start ticks");
        write_record_with_proc_start(tmp.path(), pid, &format!(r#""{ticks}""#));

        let result = detect_idle_from_registry(PROJECT, pid, tmp.path()).unwrap();

        assert_eq!(result.state, SessionState::Active);
        assert!(result.authoritative);
    }

    /// `procStart` is a string today; a future numeric spelling must not make
    /// the whole record unparsable (which would drop the session entirely).
    #[cfg(target_os = "linux")]
    #[test]
    fn numeric_proc_start_is_compared_the_same_way() {
        let tmp = TempDir::new().unwrap();
        let pid = std::process::id();
        let ticks = crate::platform::process_start_ticks(pid).expect("own start ticks");
        write_record_with_proc_start(tmp.path(), pid, &ticks.to_string());

        assert!(detect_idle_from_registry(PROJECT, pid, tmp.path()).is_some());

        write_record_with_proc_start(tmp.path(), pid, "1");
        assert!(detect_idle_from_registry(PROJECT, pid, tmp.path()).is_none());
    }

    // Regression: c9669ef taught the scanner to build a transcript path from a
    // config dir but left no way back, so a launch handed a session transcript
    // could not tell which subscription's history it belongs to.
    #[test]
    fn config_dir_for_transcript_inverts_the_transcript_path() {
        let tmp = TempDir::new().unwrap();
        write_record(tmp.path(), 27051, "busy", "2.1.238");
        write_transcript(tmp.path(), PROJECT);
        let entry = read_entry(tmp.path(), 27051).unwrap();
        let transcript = transcript_path(tmp.path(), &entry).expect("transcript");

        assert_eq!(
            config_dir_for_transcript(&transcript).as_deref(),
            Some(tmp.path())
        );
    }

    #[test]
    fn config_dir_for_transcript_rejects_a_foreign_layout() {
        for path in [
            "/home/user/stray.jsonl",
            "/home/user/.codex/sessions/2026/08/rollout-abc.jsonl",
            "/home/user/.claude/projects/session.jsonl",
        ] {
            assert_eq!(
                config_dir_for_transcript(Path::new(path)),
                None,
                "path {path}"
            );
        }
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
