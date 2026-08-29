use serde::{Deserialize, Serialize};

use super::CliTool;

/// State of a detected CLI tool session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Claude is actively working (JSONL mtime < 5s ago).
    Active,
    /// Session is waiting for user input (JSONL mtime > 10s ago, process alive).
    Idle,
}

/// Confidence level for reported activity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivityConfidence {
    /// Process-level signal or deterministic file ownership.
    High,
    /// Project-scoped file signal used with single-session attribution.
    Medium,
    /// No direct attribution signal available.
    #[default]
    Low,
}

/// Attribution quality for the reported activity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivityAttribution {
    /// Activity was attributed to this exact process/session.
    Attributed,
    /// Project shows activity, but this process cannot be proven as owner.
    Unattributed,
    /// No active signal observed.
    #[default]
    None,
}

/// Grouping metadata used by sidebar session indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionGroupKind {
    MeshTeam,
    #[default]
    Standalone,
}

/// A detected CLI tool session for UI/display consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplaySession {
    /// Process ID of the CLI tool.
    pub pid: u32,
    /// Absolute path to the project directory (from /proc/PID/cwd).
    pub project_path: String,
    /// Terminal device (e.g., "/dev/pts/2").
    pub tty: String,
    /// The full command line args.
    pub args: String,
    /// Which CLI tool this session belongs to.
    pub cli_tool: CliTool,
    /// tmux session name (if mapped).
    pub tmux_session: Option<String>,
    /// tmux window index (if mapped).
    pub tmux_window: Option<String>,
    /// tmux pane ID (e.g., "%0") (if mapped).
    pub tmux_pane: Option<String>,
    /// tmux window name (if mapped).
    pub tmux_window_name: Option<String>,
    /// Session state: Active or Idle.
    pub state: SessionState,
    /// Whether proc-level IO/network detection reported recent active work.
    #[serde(default)]
    pub recent_io: bool,
    /// Seconds since the latest session output file change, if known.
    #[serde(default)]
    pub last_output_age_secs: Option<u64>,
    /// Confidence score for this session's current activity classification.
    #[serde(default)]
    pub activity_confidence: ActivityConfidence,
    /// Attribution quality for the current activity signal.
    #[serde(default)]
    pub activity_attribution: ActivityAttribution,
    /// Project has active session-file signal that could not be tied to this PID.
    #[serde(default)]
    pub project_unattributed_active: bool,
    /// Grouping mode used by session indicators.
    #[serde(default)]
    pub group_kind: SessionGroupKind,
    /// Stable grouping key when the session belongs to a managed team.
    #[serde(default)]
    pub group_id: Option<String>,
    /// User-facing grouping label when the session belongs to a managed team.
    #[serde(default)]
    pub group_label: Option<String>,
    /// Managed team member name associated with this session.
    #[serde(default)]
    pub member_name: Option<String>,
    /// Recent writes from live workflow subagents attached to this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
}

/// A detected CLI tool session with runtime transcript metadata preserved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSession {
    pub pid: u32,
    pub project_path: String,
    pub tty: String,
    pub args: String,
    pub cli_tool: CliTool,
    pub tmux_session: Option<String>,
    pub tmux_window: Option<String>,
    pub tmux_pane: Option<String>,
    pub tmux_window_name: Option<String>,
    pub state: SessionState,
    pub session_id: Option<String>,
    pub jsonl_path: Option<String>,
    #[serde(default)]
    pub recent_io: bool,
    #[serde(default)]
    pub last_output_age_secs: Option<u64>,
    #[serde(default)]
    pub activity_confidence: ActivityConfidence,
    #[serde(default)]
    pub activity_attribution: ActivityAttribution,
    #[serde(default)]
    pub project_unattributed_active: bool,
    #[serde(default)]
    pub group_kind: SessionGroupKind,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub group_label: Option<String>,
    #[serde(default)]
    pub member_name: Option<String>,
    /// Recent writes from live workflow subagents attached to this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
}

impl From<RuntimeSession> for DisplaySession {
    fn from(session: RuntimeSession) -> Self {
        Self {
            pid: session.pid,
            project_path: session.project_path,
            tty: session.tty,
            args: session.args,
            cli_tool: session.cli_tool,
            tmux_session: session.tmux_session,
            tmux_window: session.tmux_window,
            tmux_pane: session.tmux_pane,
            tmux_window_name: session.tmux_window_name,
            state: session.state,
            recent_io: session.recent_io,
            last_output_age_secs: session.last_output_age_secs,
            activity_confidence: session.activity_confidence,
            activity_attribution: session.activity_attribution,
            project_unattributed_active: session.project_unattributed_active,
            group_kind: session.group_kind,
            group_id: session.group_id,
            group_label: session.group_label,
            member_name: session.member_name,
            workflow_activity: session.workflow_activity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_state_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&SessionState::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&SessionState::Idle).unwrap(),
            "\"idle\""
        );
    }

    #[test]
    fn runtime_session_serializes_to_json() {
        let session = RuntimeSession {
            pid: 1234,
            project_path: "/home/user/projects/foo".to_string(),
            tty: "/dev/pts/2".to_string(),
            args: "claude --dangerously-skip-permissions".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: Some("0".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%3".to_string()),
            tmux_window_name: Some("foo".to_string()),
            state: SessionState::Active,
            session_id: Some("abc-123".to_string()),
            jsonl_path: Some(
                "/home/user/.claude/projects/-home-user-projects-foo/abc-123.jsonl".to_string(),
            ),
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: Some(crate::workflow_runs::WorkflowActivity {
                live_runs: 2,
                last_write_at: 1_800_000_000_000,
            }),
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["pid"], 1234);
        assert_eq!(json["project_path"], "/home/user/projects/foo");
        assert_eq!(json["cli_tool"], "claude");
        assert_eq!(json["state"], "active");
        assert_eq!(json["tmux_pane"], "%3");
        assert_eq!(json["session_id"], "abc-123");
        assert_eq!(json["activity_confidence"], "high");
        assert_eq!(json["activity_attribution"], "attributed");
        assert_eq!(json["group_kind"], "standalone");
        assert_eq!(json["workflow_activity"]["live_runs"], 2);
        assert_eq!(
            json["workflow_activity"]["last_write_at"],
            1_800_000_000_000_i64
        );
    }

    #[test]
    fn display_session_strips_runtime_metadata_on_serialize() {
        let session = DisplaySession {
            pid: 1234,
            project_path: "/home/user/projects/foo".to_string(),
            tty: "/dev/pts/2".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: SessionState::Idle,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::Low,
            activity_attribution: ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: None,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["state"], "idle");
        assert!(json["tmux_session"].is_null());
        assert!(json.get("session_id").is_none());
        assert!(json.get("jsonl_path").is_none());
    }

    #[test]
    fn runtime_session_sanitizes_to_display_session() {
        let runtime = RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            session_id: Some("sess-123".to_string()),
            jsonl_path: Some("/home/user/.codex/sessions/sess-123.jsonl".to_string()),
            recent_io: false,
            last_output_age_secs: Some(1),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: Some(crate::workflow_runs::WorkflowActivity {
                live_runs: 1,
                last_write_at: 1_800_000_000_000,
            }),
        };

        let display = DisplaySession::from(runtime);
        let json = serde_json::to_value(&display).unwrap();

        assert_eq!(display.pid, 42);
        assert_eq!(display.tmux_pane.as_deref(), Some("%7"));
        assert!(json.get("session_id").is_none());
        assert!(json.get("jsonl_path").is_none());
        assert_eq!(json["workflow_activity"]["live_runs"], 1);
    }
}
