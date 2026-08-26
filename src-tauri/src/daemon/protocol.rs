/// Daemon protocol types for NDJSON communication over TCP.
///
/// Warning:
/// - `LIST_DISPLAY_SESSIONS` is the UI-safe session view and strips transcript
///   metadata.
/// - `LIST_RUNTIME_SESSIONS` preserves transcript metadata and must be used for
///   coordination/runtime correlation.
///
/// Request/response pairs are matched by `id`. Events are push-only
/// messages from daemon to client, distinguished by having an `event`
/// field instead of `id`.
use serde::{Deserialize, Serialize};

/// Protocol version — bump this whenever the daemon API changes in a way
/// that requires the app to be rebuilt against the new daemon.
///
/// The app checks this on connect. If the daemon's protocol version is
/// lower than what the app expects, it warns the user to rebuild the daemon.
/// v8: tmux focus became a field of the hub's versioned snapshot
/// (`wait_session_updates.focus`), the only live focus transport. A v7 daemon
/// omits it and would leave the app with a permanently dark indicator.
/// v9: the app explicitly selects the daemon's Codex compaction mode instead of
/// making the daemon guess the desktop settings database path.
pub const PROTOCOL_VERSION: u32 = 9;

// ---------------------------------------------------------------------------
// Envelope types (wire format)
// ---------------------------------------------------------------------------

/// A request sent from the Windows app to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    /// Auth token for daemon authentication. Added in protocol v4.
    /// Old clients without auth will send None (backward compatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

/// A response sent from the daemon to the Windows app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DaemonError>,
}

/// An error payload inside a DaemonResponse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonError {
    pub code: String,
    pub message: String,
}

/// A push event from the daemon (no request ID, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonEvent {
    pub event: String,
    pub data: serde_json::Value,
}

/// Any message that can arrive on the TCP stream from the daemon.
/// We deserialize into this to distinguish responses from events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DaemonMessage {
    Response(DaemonResponse),
    Event(DaemonEvent),
}

// ---------------------------------------------------------------------------
// Method constants
// ---------------------------------------------------------------------------

pub mod method {
    pub const PING: &str = "ping";
    pub const GIT_STATUS: &str = "git_status";
    pub const GIT_LOG: &str = "git_log";
    pub const GIT_LATEST_COMMIT_TIME: &str = "git_latest_commit_time";
    pub const FILE_TREE: &str = "file_tree";
    pub const READ_FILE: &str = "read_file";
    pub const READ_README: &str = "read_readme";
    pub const READ_ASSET: &str = "read_asset";
    pub const LIST_DIRECTORY: &str = "list_directory";
    pub const SCAN_SESSIONS: &str = "scan_sessions";
    pub const LIST_RUNTIME_SESSIONS: &str = "list_runtime_sessions";
    pub const WATCH: &str = "watch";
    pub const UNWATCH: &str = "unwatch";
    pub const SHUTDOWN: &str = "shutdown";
    pub const SET_CODEX_COMPACTION_MODE: &str = "set_codex_compaction_mode";

    // Command Center — session management
    pub const LIST_DISPLAY_SESSIONS: &str = "list_display_sessions";
    pub const GET_RUNTIME_SESSION_SNAPSHOT: &str = "get_runtime_session_snapshot";
    pub const WAIT_SESSION_UPDATES: &str = "wait_session_updates";
    pub const LAUNCH_SESSION: &str = "launch_session";
    pub const STOP_SESSION: &str = "stop_session";
    pub const NAVIGATE_TO_SESSION: &str = "navigate_to_session";

    // Task scanner
    pub const GET_PROJECT_TASKS: &str = "get_project_tasks";

    // Git range queries (for archived session enrichment)
    pub const GIT_COMMITS_IN_RANGE: &str = "git_commits_in_range";

    // Per-commit file changes (for Git tab detail view)
    pub const GIT_COMMIT_FILES: &str = "git_commit_files";

    // Per-file diff within a commit (for inline diff view)
    pub const GIT_COMMIT_DIFF: &str = "git_commit_diff";
}

pub mod event {
    pub const FILE_CHANGED: &str = "file_changed";
    pub const GIT_CHANGED: &str = "git_changed";
    pub const SESSION_FILE_CREATED: &str = "session_file_created";
}

// ---------------------------------------------------------------------------
// Method-specific param/result types
// ---------------------------------------------------------------------------

/// `ping` — health check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingResult {
    pub version: String,
    /// Protocol version for compatibility checking. Old daemons that don't
    /// include this field will deserialize as 0 (the default).
    #[serde(default)]
    pub protocol_version: u32,
    pub uptime_secs: u64,
    /// Canonical app-data root used by the daemon. Additive for older daemons.
    #[serde(default)]
    pub data_root: String,
}

/// `git_status` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetCodexCompactionModeParams {
    pub mode: crate::models::CodexCompactionMode,
}

/// `get_project_tasks` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectTasksParams {
    pub path: String,
    /// Optional scan cycle identifier for per-cycle daemon cache reuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_cycle_id: Option<u64>,
}

/// `git_log` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitLogParams {
    pub path: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

/// `git_commits_in_range` params — time-bounded commit query
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitsInRangeParams {
    pub path: String,
    pub after: String,  // RFC 3339 timestamp
    pub before: String, // RFC 3339 timestamp
    /// Optional cap for number of commits included in the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_limit: Option<usize>,
}

/// `git_commits_in_range` result — commits + file paths
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitsInRangeResult {
    pub commits: Vec<crate::models::Commit>,
    pub files: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
}

/// `git_commit_files` params — get files changed by a specific commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitFilesParams {
    pub path: String,
    pub hash: String,
}

/// `git_commit_files` result — list of changed files with status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitFilesResult {
    pub files: Vec<crate::models::CommitFile>,
}

/// `git_commit_diff` params — get diff hunks for a specific file in a commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitDiffParams {
    pub path: String,
    pub hash: String,
    pub file_path: String,
}

/// `git_commit_diff` result — diff hunks with line detail
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitDiffResult {
    pub hunks: Vec<crate::models::DiffHunk>,
}

/// `read_file` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadFileParams {
    pub path: String,
    pub relative: String,
}

/// `read_asset` result — binary data as base64
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadAssetResult {
    pub data: String, // base64-encoded
}

/// `scan_sessions` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanSessionsResult {
    pub paths: Vec<String>,
}

/// `watch`/`unwatch` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchResult {
    pub ok: bool,
}

/// `git_latest_commit_time` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatestCommitTimeResult {
    pub timestamp: Option<String>, // RFC 3339 or null
}

// ---------------------------------------------------------------------------
// Command Center — session management types
// ---------------------------------------------------------------------------

/// Launch mode for a new Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    /// `claude --dangerously-skip-permissions --continue`
    Continue,
    /// `claude --dangerously-skip-permissions`
    Fresh,
    /// `claude --dangerously-skip-permissions --resume`
    Resume,
}

/// `launch_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchSessionParams {
    pub project_path: String,
    pub mode: LaunchMode,
    /// Which CLI tool to launch. Defaults to Claude for backward compatibility.
    #[serde(default = "default_cli_tool")]
    pub cli_tool: crate::session_scanner::cli_tool::CliTool,
    /// Tmux layout strategy: "new_window", "split", "per_project".
    /// Defaults to "new_window" for backward compatibility.
    #[serde(default = "default_tmux_layout")]
    pub tmux_layout: String,
    /// Custom command to execute instead of the default for this tool/mode.
    /// Resolved from user settings on the app side. The daemon just executes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_override: Option<String>,
}

fn default_tmux_layout() -> String {
    "new_window".to_string()
}

fn default_cli_tool() -> crate::session_scanner::cli_tool::CliTool {
    crate::session_scanner::cli_tool::CliTool::Claude
}

/// `launch_session` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchSessionResult {
    /// Which tmux session the window was created in. Optional for backward
    /// compat with older daemons that don't send this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    pub tmux_window: String,
    pub tmux_pane: String,
}

/// `stop_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StopSessionParams {
    pub tmux_pane: String,
    /// Which CLI tool is running. Defaults to Claude for backward compatibility.
    #[serde(default = "default_cli_tool")]
    pub cli_tool: crate::session_scanner::cli_tool::CliTool,
}

/// `navigate_to_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigateToSessionParams {
    pub tmux_session: String,
    pub tmux_window: String,
    pub tmux_pane: String,
}

/// `wait_session_updates` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitSessionUpdatesParams {
    /// Client's last seen session snapshot version.
    #[serde(default)]
    pub since_version: u64,
    /// Max time to wait for a newer snapshot. Clamped server-side.
    #[serde(default = "default_wait_session_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_wait_session_timeout_ms() -> u64 {
    15_000
}

/// `wait_session_updates` result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitSessionUpdatesResult {
    /// Monotonic daemon-local version of the session snapshot.
    pub version: u64,
    /// Whether this response contains a version newer than `since_version`.
    pub changed: bool,
    /// Full session snapshot for the reported version.
    pub sessions: Vec<crate::session_scanner::DisplaySession>,
    /// tmux focus as of this version. Additive: older daemons omit it.
    #[serde(default)]
    pub focus: Option<crate::session_scanner::tmux::TmuxFocus>,
    /// Project path the focused tmux window belongs to, resolved by the hub.
    #[serde(default)]
    pub focus_project_path: Option<String>,
}

/// `get_runtime_session_snapshot` result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSessionSnapshotResult {
    pub version: u64,
    pub display_sessions: Vec<crate::session_scanner::DisplaySession>,
    pub runtime_sessions: Vec<crate::session_scanner::RuntimeSession>,
    /// tmux focus owned by the daemon hub. Serializes with the legacy
    /// `session`/`window` keys so an older app still decodes it.
    #[serde(default)]
    pub focus: Option<crate::session_scanner::tmux::TmuxFocus>,
    /// Legacy wire name for the hub's `focus_project_path`.
    pub foreground_project_path: Option<String>,
    /// The daemon scanner's latest cycle could not read its process inventory:
    /// the sessions are the hub's last good snapshot, not an observation, and
    /// must not bind identities or promote activity. Additive: older daemons
    /// omit the field and decode as `false` (their behavior so far).
    #[serde(default)]
    pub degraded: bool,
}

// ---------------------------------------------------------------------------
// Event data types
// ---------------------------------------------------------------------------

/// Data for `file_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileChangedData {
    pub path: String,
    pub files: Vec<String>,
}

/// Data for `git_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitChangedData {
    pub path: String,
}

/// Data for `session_file_created` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionFileCreatedData {
    pub path: String,
    pub file: String,
}

// ---------------------------------------------------------------------------
// Helper constructors
// ---------------------------------------------------------------------------

impl DaemonRequest {
    pub fn new(id: impl Into<String>, method: &str, params: impl Serialize) -> Self {
        Self {
            id: id.into(),
            method: method.to_string(),
            params: serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
            auth: None,
        }
    }

    /// Create a request with an auth token attached.
    pub fn with_auth(mut self, token: Option<String>) -> Self {
        self.auth = token;
        self
    }

    pub fn ping(id: impl Into<String>) -> Self {
        Self::new(id, method::PING, serde_json::Value::Null)
    }
}

impl DaemonResponse {
    pub fn ok(id: impl Into<String>, result: impl Serialize) -> Self {
        Self {
            id: id.into(),
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    pub fn err(id: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(DaemonError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

impl DaemonEvent {
    pub fn new(event: &str, data: impl Serialize) -> Self {
        Self {
            event: event.to_string(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Commit, FileTreeNode, GitStatus};
    use serde_json::json;

    #[test]
    fn request_serialization_roundtrip() {
        let req = DaemonRequest::new(
            "r1",
            method::GIT_STATUS,
            PathParams {
                path: "/home/user/projects/foo".to_string(),
            },
        );
        let json = serde_json::to_string(&req).unwrap();
        let back: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn request_serializes_to_expected_json() {
        let req = DaemonRequest::new(
            "r1",
            method::GIT_STATUS,
            PathParams {
                path: "/home/user/foo".to_string(),
            },
        );
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["id"], "r1");
        assert_eq!(v["method"], "git_status");
        assert_eq!(v["params"]["path"], "/home/user/foo");
    }

    #[test]
    fn response_ok_roundtrip() {
        let resp = DaemonResponse::ok(
            "r1",
            GitStatus {
                branch: Some("main".to_string()),
                is_dirty: false,
                ahead: 0,
                behind: 0,
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        let back: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(back.is_ok());
    }

    #[test]
    fn response_error_roundtrip() {
        let resp = DaemonResponse::err("r1", "NOT_FOUND", "Path does not exist");
        let json = serde_json::to_string(&resp).unwrap();
        let back: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(!back.is_ok());
    }

    #[test]
    fn response_ok_omits_error_field() {
        let resp = DaemonResponse::ok("r1", json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn response_error_omits_result_field() {
        let resp = DaemonResponse::err("r1", "ERR", "oops");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn event_roundtrip() {
        let evt = DaemonEvent::new(
            event::GIT_CHANGED,
            GitChangedData {
                path: "/home/user/foo".to_string(),
            },
        );
        let json = serde_json::to_string(&evt).unwrap();
        let back: DaemonEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, back);
    }

    #[test]
    fn daemon_message_distinguishes_response_from_event() {
        let resp_json = r#"{"id": "r1", "result": {"ok": true}}"#;
        let msg: DaemonMessage = serde_json::from_str(resp_json).unwrap();
        assert!(matches!(msg, DaemonMessage::Response(_)));

        let evt_json = r#"{"event": "git_changed", "data": {"path": "/foo"}}"#;
        let msg: DaemonMessage = serde_json::from_str(evt_json).unwrap();
        assert!(matches!(msg, DaemonMessage::Event(_)));
    }

    #[test]
    fn ping_result_roundtrip() {
        // Regression: commits a53ad31 (removal added) and f9c1e89 (None => remove-all)
        // exposed that daemon pings did not identify their data root authority.
        let ping = PingResult {
            version: "0.1.0".to_string(),
            protocol_version: PROTOCOL_VERSION,
            uptime_secs: 120,
            data_root: "/tmp/taurhaus-data".to_string(),
        };
        let json = serde_json::to_string(&ping).unwrap();
        let roundtrip: PingResult = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip, ping);
    }

    #[test]
    fn ping_result_old_daemon_without_protocol_version() {
        // Old daemons won't include protocol_version — should default to 0
        let json = r#"{"version":"0.1.0","uptime_secs":60}"#;
        let r: PingResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.protocol_version, 0);
        assert!(r.data_root.is_empty());
    }

    #[test]
    fn git_log_params_defaults() {
        let json = r#"{"path": "/foo"}"#;
        let params: GitLogParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 50);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn git_log_params_explicit() {
        let json = r#"{"path": "/foo", "limit": 10, "offset": 5}"#;
        let params: GitLogParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 10);
        assert_eq!(params.offset, 5);
    }

    #[test]
    fn read_file_params_roundtrip() {
        let p = ReadFileParams {
            path: "/home/user/foo".to_string(),
            relative: "src/main.rs".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ReadFileParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn response_result_can_hold_git_status() {
        let status = GitStatus {
            branch: Some("main".to_string()),
            is_dirty: true,
            ahead: 2,
            behind: 0,
        };
        let resp = DaemonResponse::ok("r1", &status);
        let result = resp.result.unwrap();
        let back: GitStatus = serde_json::from_value(result).unwrap();
        assert_eq!(back, status);
    }

    #[test]
    fn response_result_can_hold_commits() {
        let commits = vec![Commit {
            hash: "abc12345".into(),
            message: "Initial".into(),
            body: None,
            author: "Me".into(),
            date: "2h".into(),
            timestamp: 1740000000,
        }];
        let resp = DaemonResponse::ok("r1", &commits);
        let result = resp.result.unwrap();
        let back: Vec<Commit> = serde_json::from_value(result).unwrap();
        assert_eq!(back, commits);
    }

    #[test]
    fn response_result_can_hold_file_tree() {
        let tree = vec![FileTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            children: vec![],
        }];
        let resp = DaemonResponse::ok("r1", &tree);
        let result = resp.result.unwrap();
        let back: Vec<FileTreeNode> = serde_json::from_value(result).unwrap();
        assert_eq!(back, tree);
    }

    #[test]
    fn file_changed_event_data_roundtrip() {
        let data = FileChangedData {
            path: "/home/user/foo".to_string(),
            files: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        };
        let evt = DaemonEvent::new(event::FILE_CHANGED, &data);
        let back: FileChangedData = serde_json::from_value(evt.data).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn session_file_created_event_data_roundtrip() {
        let data = SessionFileCreatedData {
            path: "/home/user/foo".to_string(),
            file: ".claude/handoffs/2025-01-15-session.md".to_string(),
        };
        let evt = DaemonEvent::new(event::SESSION_FILE_CREATED, &data);
        let back: SessionFileCreatedData = serde_json::from_value(evt.data).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn ping_request_has_no_params() {
        let req = DaemonRequest::ping("r1");
        assert_eq!(req.method, "ping");
        assert!(req.params.is_null());
    }

    #[test]
    fn latest_commit_time_result_with_value() {
        let r = LatestCommitTimeResult {
            timestamp: Some("2025-06-15T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: LatestCommitTimeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn latest_commit_time_result_null() {
        let r = LatestCommitTimeResult { timestamp: None };
        let json = serde_json::to_string(&r).unwrap();
        let back: LatestCommitTimeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn scan_sessions_result_roundtrip() {
        let r = ScanSessionsResult {
            paths: vec![
                "/foo/.claude/handoffs/a.md".into(),
                "/foo/.claude/handoffs/b.md".into(),
            ],
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: ScanSessionsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn watch_result_roundtrip() {
        let r = WatchResult { ok: true };
        let json = serde_json::to_string(&r).unwrap();
        let back: WatchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn launch_session_params_roundtrip() {
        let p = LaunchSessionParams {
            project_path: "/home/user/proj".to_string(),
            mode: LaunchMode::Continue,
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
            tmux_layout: "new_window".to_string(),
            command_override: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: LaunchSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn launch_session_params_with_command_override() {
        let p = LaunchSessionParams {
            project_path: "/proj".to_string(),
            mode: LaunchMode::Fresh,
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
            tmux_layout: "new_window".to_string(),
            command_override: Some("my-custom-claude --flag".to_string()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("command_override"));
        let back: LaunchSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.command_override,
            Some("my-custom-claude --flag".to_string())
        );
    }

    #[test]
    fn launch_session_params_defaults_to_claude() {
        // Old daemon protocol without cli_tool field should default to Claude
        let json = r#"{"project_path":"/proj","mode":"fresh"}"#;
        let p: LaunchSessionParams = serde_json::from_str(json).unwrap();
        assert_eq!(
            p.cli_tool,
            crate::session_scanner::cli_tool::CliTool::Claude
        );
        assert_eq!(p.command_override, None);
    }

    #[test]
    fn launch_session_result_roundtrip() {
        let r = LaunchSessionResult {
            tmux_session: Some("0".to_string()),
            tmux_window: "proj".to_string(),
            tmux_pane: "%5".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: LaunchSessionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn launch_session_result_backward_compat() {
        // Old daemons don't send tmux_session — should deserialize with None
        let json = r#"{"tmux_window":"proj","tmux_pane":"%5"}"#;
        let r: LaunchSessionResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.tmux_session, None);
        assert_eq!(r.tmux_window, "proj");
    }

    #[test]
    fn stop_session_params_roundtrip() {
        let p = StopSessionParams {
            tmux_pane: "%3".to_string(),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: StopSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn stop_session_params_defaults_to_claude() {
        let json = r#"{"tmux_pane":"%3"}"#;
        let p: StopSessionParams = serde_json::from_str(json).unwrap();
        assert_eq!(
            p.cli_tool,
            crate::session_scanner::cli_tool::CliTool::Claude
        );
    }

    #[test]
    fn navigate_to_session_params_roundtrip() {
        let p = NavigateToSessionParams {
            tmux_session: "main".to_string(),
            tmux_window: "1".to_string(),
            tmux_pane: "%3".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: NavigateToSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn wait_session_updates_params_defaults() {
        let json = r#"{"since_version":42}"#;
        let p: WaitSessionUpdatesParams = serde_json::from_str(json).unwrap();
        assert_eq!(p.since_version, 42);
        assert_eq!(p.timeout_ms, 15_000);
    }

    #[test]
    fn wait_session_updates_result_roundtrip() {
        let r = WaitSessionUpdatesResult {
            version: 7,
            changed: true,
            sessions: vec![],
            focus: None,
            focus_project_path: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: WaitSessionUpdatesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    // Regression: the daemon hub kept its last good sessions across degraded
    // scanner cycles but the runtime-session protocol carried no degradation
    // status, so the Windows app read the cached snapshot as a fresh
    // observation. The flag travels on `get_runtime_session_snapshot`.
    #[test]
    fn runtime_session_snapshot_result_roundtrip_carries_degraded() {
        let r = RuntimeSessionSnapshotResult {
            version: 9,
            display_sessions: vec![],
            runtime_sessions: vec![],
            focus: None,
            foreground_project_path: None,
            degraded: true,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"degraded\":true"));
        let back: RuntimeSessionSnapshotResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    // Regression companion: the field is additive — a daemon built before it
    // omits it and must decode as a healthy snapshot (its behavior so far).
    #[test]
    fn runtime_session_snapshot_result_old_daemon_without_degraded() {
        let json = r#"{"version":2,"display_sessions":[],"runtime_sessions":[],"focus":null,"foreground_project_path":null}"#;
        let r: RuntimeSessionSnapshotResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.version, 2);
        assert!(!r.degraded);
    }

    fn focus_fixture() -> crate::session_scanner::tmux::TmuxFocus {
        crate::session_scanner::tmux::TmuxFocus {
            session: "taurhaus".to_string(),
            window_index: "2".to_string(),
            pane_id: "%9".to_string(),
        }
    }

    // Regression: commit 07ab6c5 deleted the hook -> tmux-focus.json -> inotify
    // chain and made `wait_session_updates` the only live focus transport, but
    // left PROTOCOL_VERSION at 7. A v7 daemon omits the focus fields, the app
    // bridge reads that absence as "nothing is focused" and the sidebar
    // indicator goes dark with no other source to recover from. Startup must
    // refuse such a daemon on ping instead.
    #[test]
    fn protocol_version_excludes_daemons_without_hub_owned_focus() {
        // The last version whose daemon produced focus through the hook chain.
        let hook_chain_daemon = 7;
        assert!(
            PROTOCOL_VERSION > hook_chain_daemon,
            "hub-owned focus changed the wire contract: bump PROTOCOL_VERSION so \
             startup replaces a pre-PR8 daemon instead of trusting its empty focus"
        );
    }

    #[test]
    fn wait_session_updates_result_roundtrips_focus() {
        let result = WaitSessionUpdatesResult {
            version: 7,
            changed: true,
            sessions: Vec::new(),
            focus: Some(focus_fixture()),
            focus_project_path: Some("/projects/mesh".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serde_json::from_str::<WaitSessionUpdatesResult>(&json).unwrap(),
            result
        );
    }

    #[test]
    fn wait_session_updates_result_decodes_without_focus() {
        // Old daemons omit the focus fields entirely.
        let json = r#"{"version":3,"changed":false,"sessions":[]}"#;
        let result: WaitSessionUpdatesResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.focus, None);
        assert_eq!(result.focus_project_path, None);
    }

    #[test]
    fn runtime_session_snapshot_result_roundtrips_focus() {
        let result = RuntimeSessionSnapshotResult {
            version: 4,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus: Some(focus_fixture()),
            foreground_project_path: Some("/projects/mesh".to_string()),
            degraded: false,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["focus"]["session"], "taurhaus");
        assert_eq!(json["focus"]["window"], "2");
        assert_eq!(
            serde_json::from_value::<RuntimeSessionSnapshotResult>(json).unwrap(),
            result
        );
    }

    #[test]
    fn runtime_session_snapshot_result_decodes_legacy_focus_payload() {
        // A daemon built before the hub-owned probe sends the hook file shape.
        let json = r#"{"version":1,"display_sessions":[],"runtime_sessions":[],"focus":{"session":"taurhaus","window":"2","timestamp":123},"foreground_project_path":null}"#;
        let result: RuntimeSessionSnapshotResult = serde_json::from_str(json).unwrap();
        let focus = result.focus.expect("legacy focus decodes");
        assert_eq!(focus.session, "taurhaus");
        assert_eq!(focus.window_index, "2");
        assert_eq!(focus.pane_id, "");

        let detached = r#"{"version":1,"display_sessions":[],"runtime_sessions":[],"focus":{"session":null,"window":null,"timestamp":null},"foreground_project_path":null}"#;
        let result: RuntimeSessionSnapshotResult = serde_json::from_str(detached).unwrap();
        assert_eq!(result.focus.expect("detached focus decodes").session, "");
    }

    #[test]
    fn codex_compaction_mode_params_roundtrip() {
        // Regression: 6fe0aa3 made the daemon guess the desktop settings DB path;
        // mode ownership now crosses the daemon protocol explicitly.
        let params = SetCodexCompactionModeParams {
            mode: crate::models::CodexCompactionMode::Transcript,
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        assert_eq!(
            serde_json::from_str::<SetCodexCompactionModeParams>(&json).expect("decode params"),
            params
        );
    }
}
