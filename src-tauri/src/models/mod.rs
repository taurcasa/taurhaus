use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Activity state (ADR-008)
// ---------------------------------------------------------------------------

/// Activity state, computed on every read from `last_activity_at` and
/// configurable thresholds.  Never stored in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityState {
    Active,
    Recent,
    Stale,
    Dormant,
}

/// Threshold configuration for activity state computation (ADR-008).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityThresholds {
    #[serde(alias = "active_days")]
    pub active_days: i64,
    #[serde(alias = "recent_days")]
    pub recent_days: i64,
    #[serde(alias = "stale_days")]
    pub stale_days: i64,
}

impl Default for ActivityThresholds {
    fn default() -> Self {
        Self {
            active_days: 7,
            recent_days: 30,
            stale_days: 90,
        }
    }
}

impl ActivityState {
    /// Compute the activity state for a given `last_activity_at` timestamp.
    /// If `last_activity_at` is `None` or unparseable, returns `Dormant`.
    pub fn compute(
        last_activity_at: Option<&str>,
        thresholds: &ActivityThresholds,
        now: DateTime<Utc>,
    ) -> Self {
        let ts = match last_activity_at.and_then(|s| s.parse::<DateTime<Utc>>().ok()) {
            Some(t) => t,
            None => return ActivityState::Dormant,
        };

        let days = (now - ts).num_days();

        if days < thresholds.active_days {
            ActivityState::Active
        } else if days < thresholds.recent_days {
            ActivityState::Recent
        } else if days < thresholds.stale_days {
            ActivityState::Stale
        } else {
            ActivityState::Dormant
        }
    }
}

// ---------------------------------------------------------------------------
// Project (ADR-007)
// ---------------------------------------------------------------------------

/// Database row for a project.  Used for persistence and query results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub last_activity_at: Option<String>,
    pub hero_preference: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Cached git branch name (populated by watcher/startup, may be None).
    pub cached_branch: Option<String>,
    /// Cached dirty status (populated by watcher/startup, may be None).
    pub cached_is_dirty: Option<bool>,
}

/// Lightweight project summary sent to the frontend for the sidebar list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub activity_state: ActivityState,
    pub last_activity_at: Option<String>,
    pub branch: Option<String>,
    pub is_dirty: Option<bool>,
}

impl ProjectSummary {
    /// Build a `ProjectSummary` from a database `Project` row.
    /// Git fields come from cached columns (may be None if not yet scanned).
    pub fn from_project(
        project: &Project,
        thresholds: &ActivityThresholds,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            activity_state: ActivityState::compute(
                project.last_activity_at.as_deref(),
                thresholds,
                now,
            ),
            last_activity_at: project.last_activity_at.clone(),
            branch: project.cached_branch.clone(),
            is_dirty: project.cached_is_dirty,
        }
    }
}

/// Full project details sent to the frontend for the detail view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub activity_state: ActivityState,
    pub last_activity_at: Option<String>,
    pub hero_preference: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub branch: Option<String>,
    pub is_dirty: Option<bool>,
}

// ---------------------------------------------------------------------------
// Session (ADR-009)
// ---------------------------------------------------------------------------

/// Lightweight session summary for sidebar/list display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub project_id: String,
    pub date: String,
    pub summary: String,
}

/// Full session detail including extensible metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub id: String,
    pub project_id: String,
    pub date: String,
    pub summary: String,
    pub next_steps: Vec<String>,
    pub open_questions: Vec<String>,
    pub metadata: serde_json::Value,
    pub file_path: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Relationship (ADR-010)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    pub source_project_id: String,
    pub target_project_id: String,
    pub relationship_type: String,
    pub detection_source: String,
    pub dismissed: bool,
    pub first_detected_at: String,
    pub last_seen_at: String,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodeThemeSettings {
    pub light: String,
    pub dark: String,
}

impl Default for CodeThemeSettings {
    fn default() -> Self {
        Self {
            light: "github-light".into(),
            dark: "github-dark-dimmed".into(),
        }
    }
}

/// Per-mode launch commands for a single CLI tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCommands {
    /// Command for "Continue" mode (resume latest session).
    #[serde(alias = "continue_cmd")]
    pub continue_cmd: String,
    /// Command for "Fresh" mode (start new session).
    pub fresh: String,
    /// Command for "Resume" mode (pick a session to resume).
    pub resume: String,
}

/// Per-tool launch command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliCommandSettings {
    pub claude: ToolCommands,
    pub codex: ToolCommands,
    pub gemini: ToolCommands,
}

impl Default for CliCommandSettings {
    fn default() -> Self {
        Self {
            claude: ToolCommands {
                continue_cmd: "claude --dangerously-skip-permissions --continue".into(),
                fresh: "claude --dangerously-skip-permissions".into(),
                resume: "claude --dangerously-skip-permissions --resume".into(),
            },
            codex: ToolCommands {
                continue_cmd: "codex --yolo".into(),
                fresh: "codex --yolo".into(),
                resume: "codex resume --last --yolo".into(),
            },
            gemini: ToolCommands {
                continue_cmd: "gemini --yolo --resume".into(),
                fresh: "gemini --yolo".into(),
                resume: "gemini --yolo --resume".into(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettings {
    /// Terminal emulator to use:
    /// - Windows: "windows_terminal" (default), "custom"
    /// - macOS: "iterm2" (default), "terminal_app", "ghostty", "custom"
    pub emulator: String,
    /// Command template when emulator is "custom".
    /// Placeholders: {distro}, {tmux_session}
    #[serde(alias = "custom_command")]
    pub custom_command: String,
    /// Tmux layout strategy: "new_window" (default), "split", "per_project"
    #[serde(alias = "tmux_layout")]
    pub tmux_layout: String,
    /// Per-tool launch command configuration.
    #[serde(default)]
    #[serde(alias = "cli_commands")]
    pub cli_commands: CliCommandSettings,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            emulator: default_emulator().into(),
            custom_command: String::new(),
            tmux_layout: "new_window".into(),
            cli_commands: CliCommandSettings::default(),
        }
    }
}

/// Platform-appropriate default terminal emulator.
fn default_emulator() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "iterm2"
    }
    #[cfg(target_os = "windows")]
    {
        "windows_terminal"
    }
    #[cfg(target_os = "linux")]
    {
        "default"
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(alias = "scan_directories")]
    pub scan_directories: Vec<String>,
    pub thresholds: ActivityThresholds,
    #[serde(alias = "ignore_patterns")]
    pub ignore_patterns: Vec<String>,
    pub daemon: DaemonSettings,
    #[serde(default)]
    #[serde(alias = "code_theme")]
    pub code_theme: CodeThemeSettings,
    #[serde(default)]
    pub terminal: TerminalSettings,
    #[serde(default)]
    #[serde(alias = "dark_mode")]
    pub dark_mode: bool,
    #[serde(default)]
    #[serde(alias = "project_dialog_last_path")]
    pub project_dialog_last_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonSettings {
    pub port: u16,
    pub path: String,
    #[serde(alias = "auto_start")]
    pub auto_start: bool,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            port: 17233,
            path: "~/.local/bin/taurhaus-daemon".to_string(),
            auto_start: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Daemon status (for IPC query)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatus {
    /// "connected", "disconnected", "not_configured", "reconnecting"
    /// Status string values intentionally remain snake_case for IPC compatibility
    /// with existing frontend event/status handling.
    pub status: String,
    pub version: Option<String>,
    /// Protocol version reported by the daemon (0 = old daemon without versioning).
    pub protocol_version: u32,
    /// Protocol version the app expects. If daemon < expected, daemon is stale.
    pub expected_protocol_version: u32,
    pub uptime_secs: Option<u64>,
    pub port: u16,
    pub wsl_distro: Option<String>,
}

/// Daemon installation status — used by FirstRunWizard and startup update check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonInstallStatus {
    /// Whether the daemon binary exists in WSL (~/.local/bin/taurhaus-daemon)
    pub installed: bool,
    /// Version of the installed daemon (from `--version` output), if installed.
    pub version: Option<String>,
    /// Version bundled with this app (from CARGO_PKG_VERSION).
    pub bundled_version: String,
    /// True if installed version < bundled version (needs update).
    pub needs_update: bool,
    /// Whether WSL is available and a distro is configured.
    pub wsl_available: bool,
    /// Human-readable error if detection failed.
    pub error: Option<String>,
}

/// Mesh installation status for coordination setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeshCompatibilityContract {
    pub version: String,
    pub protocol_version: u32,
    pub schema_version: u32,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeshCompatibilityIssue {
    pub code: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

/// Mesh installation status for coordination setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeshInstallStatus {
    /// Whether the mesh binary exists in ~/.local/bin/mesh (or WSL equivalent).
    pub installed: bool,
    /// Version of the installed mesh binary, if available.
    pub version: Option<String>,
    /// Version bundled with this app release.
    pub bundled_version: String,
    /// True when installed version differs from bundled version.
    pub needs_update: bool,
    /// Compatibility contract bundled with this taurhaus release.
    pub bundled_contract: MeshCompatibilityContract,
    /// Compatibility contract reported by the installed mesh binary, if readable.
    pub installed_contract: Option<MeshCompatibilityContract>,
    /// Structured compatibility issues between taurhaus and the installed mesh binary.
    pub compatibility_issues: Vec<MeshCompatibilityIssue>,
    /// Whether the execution environment is available (native or WSL).
    pub environment_available: bool,
    /// Human-readable error if detection failed.
    pub error: Option<String>,
}

/// Structured response for operational commands that previously returned strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
}

impl OperationResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// IPC response types (used by commands, implemented in later phases)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub message: String,
    #[serde(default)]
    pub body: Option<String>,
    pub author: String,
    pub date: String,
    /// Unix timestamp (seconds since epoch) for frontend grouping
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitRangeResult {
    pub commits: Vec<Commit>,
    pub files: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    /// One of: "added", "modified", "deleted", "renamed"
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub origin: char,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
    }

    fn default_thresholds() -> ActivityThresholds {
        ActivityThresholds::default()
    }

    #[test]
    fn ipc_models_serialize_camel_case_keys() {
        let summary = ProjectSummary {
            id: "p1".to_string(),
            name: "proj".to_string(),
            path: "/tmp/proj".to_string(),
            activity_state: ActivityState::Active,
            last_activity_at: Some("2026-01-01T00:00:00Z".to_string()),
            branch: Some("main".to_string()),
            is_dirty: Some(true),
        };
        let value = serde_json::to_value(summary).expect("serialize project summary");
        assert!(value.get("activityState").is_some());
        assert!(value.get("lastActivityAt").is_some());
        assert!(value.get("isDirty").is_some());
        assert!(value.get("activity_state").is_none());
    }

    #[test]
    fn settings_serialize_camel_case_nested_keys() {
        let settings = Settings::default();
        let value = serde_json::to_value(settings).expect("serialize settings");
        assert!(value.get("scanDirectories").is_some());
        assert!(value.get("darkMode").is_some());
        assert!(value.get("projectDialogLastPath").is_some());
        assert!(value.get("scan_directories").is_none());
    }

    // AC-1: Active for activity < 7 days ago
    #[test]
    fn activity_state_active() {
        let ts = "2025-06-12T00:00:00Z"; // 3 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Active
        );
    }

    // AC-1 boundary: exactly 6 days ago is still active
    #[test]
    fn activity_state_active_boundary() {
        let ts = "2025-06-09T12:00:00Z"; // 6 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Active
        );
    }

    // AC-2: Recent for 7-30 days
    #[test]
    fn activity_state_recent() {
        let ts = "2025-06-01T00:00:00Z"; // 14 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Recent
        );
    }

    // AC-2 boundary: exactly 7 days ago is recent (not active)
    #[test]
    fn activity_state_recent_boundary() {
        let ts = "2025-06-08T12:00:00Z"; // 7 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Recent
        );
    }

    // AC-3: Stale for 30-90 days
    #[test]
    fn activity_state_stale() {
        let ts = "2025-04-15T00:00:00Z"; // ~61 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Stale
        );
    }

    // AC-3 boundary: exactly 30 days ago is stale (not recent)
    #[test]
    fn activity_state_stale_boundary() {
        let ts = "2025-05-16T12:00:00Z"; // 30 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Stale
        );
    }

    // AC-4: Dormant for 90+ days
    #[test]
    fn activity_state_dormant() {
        let ts = "2025-01-01T00:00:00Z"; // ~165 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Dormant
        );
    }

    // AC-4 boundary: exactly 90 days ago is dormant
    #[test]
    fn activity_state_dormant_boundary() {
        let ts = "2025-03-17T12:00:00Z"; // 90 days ago
        assert_eq!(
            ActivityState::compute(Some(ts), &default_thresholds(), fixed_now()),
            ActivityState::Dormant
        );
    }

    // AC-4: None last_activity_at → Dormant
    #[test]
    fn activity_state_none_is_dormant() {
        assert_eq!(
            ActivityState::compute(None, &default_thresholds(), fixed_now()),
            ActivityState::Dormant
        );
    }

    // AC-5: Serialization round-trip
    #[test]
    fn project_serialization_roundtrip() {
        let project = Project {
            id: "abc-123".into(),
            name: "test-project".into(),
            path: "/home/user/test".into(),
            description: Some("A test project".into()),
            last_activity_at: Some("2025-06-12T10:00:00Z".into()),
            hero_preference: Some("session".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-06-12T10:00:00Z".into(),
            cached_branch: Some("main".into()),
            cached_is_dirty: Some(false),
        };

        let json = serde_json::to_string(&project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(project, back);
    }

    #[test]
    fn session_detail_serialization_roundtrip() {
        let session = SessionDetail {
            id: "s1".into(),
            project_id: "p1".into(),
            date: "2025-06-12".into(),
            summary: "Did some work".into(),
            next_steps: vec!["step 1".into(), "step 2".into()],
            open_questions: vec!["question 1".into()],
            metadata: serde_json::json!({"branch": "main", "files_changed": ["foo.rs"]}),
            file_path: "/path/to/handoff.md".into(),
            created_at: "2025-06-12T10:00:00Z".into(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let back: SessionDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(session, back);
    }

    #[test]
    fn activity_state_serializes_as_lowercase() {
        let json = serde_json::to_string(&ActivityState::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let json = serde_json::to_string(&ActivityState::Dormant).unwrap();
        assert_eq!(json, "\"dormant\"");
    }

    // AC-6: ProjectSummary from Project
    #[test]
    fn project_summary_from_project() {
        let project = Project {
            id: "p1".into(),
            name: "taurhaus".into(),
            path: "/home/user/taurhaus".into(),
            description: None,
            last_activity_at: Some("2025-06-12T00:00:00Z".into()),
            hero_preference: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
            cached_branch: None,
            cached_is_dirty: None,
        };

        let summary = ProjectSummary::from_project(&project, &default_thresholds(), fixed_now());
        assert_eq!(summary.id, "p1");
        assert_eq!(summary.name, "taurhaus");
        assert_eq!(summary.activity_state, ActivityState::Active);
        assert!(summary.branch.is_none());
    }

    // Cached git status populates ProjectSummary
    #[test]
    fn project_summary_uses_cached_git_data() {
        let project = Project {
            id: "p1".into(),
            name: "test".into(),
            path: "/path".into(),
            description: None,
            last_activity_at: Some("2025-06-12T00:00:00Z".into()),
            hero_preference: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
            cached_branch: Some("develop".into()),
            cached_is_dirty: Some(true),
        };

        let summary = ProjectSummary::from_project(&project, &default_thresholds(), fixed_now());
        assert_eq!(summary.branch, Some("develop".into()));
        assert_eq!(summary.is_dirty, Some(true));
    }

    // AC-7: Default thresholds are 7/30/90
    #[test]
    fn default_thresholds_values() {
        let t = ActivityThresholds::default();
        assert_eq!(t.active_days, 7);
        assert_eq!(t.recent_days, 30);
        assert_eq!(t.stale_days, 90);
    }

    #[test]
    fn cli_command_defaults_match_hardcoded_values() {
        let cmds = CliCommandSettings::default();
        // Claude
        assert_eq!(
            cmds.claude.continue_cmd,
            "claude --dangerously-skip-permissions --continue"
        );
        assert_eq!(cmds.claude.fresh, "claude --dangerously-skip-permissions");
        assert_eq!(
            cmds.claude.resume,
            "claude --dangerously-skip-permissions --resume"
        );
        // Codex
        assert_eq!(cmds.codex.continue_cmd, "codex --yolo");
        assert_eq!(cmds.codex.fresh, "codex --yolo");
        assert_eq!(cmds.codex.resume, "codex resume --last --yolo");
        // Gemini
        assert_eq!(cmds.gemini.continue_cmd, "gemini --yolo --resume");
        assert_eq!(cmds.gemini.fresh, "gemini --yolo");
        assert_eq!(cmds.gemini.resume, "gemini --yolo --resume");
    }

    #[test]
    fn cli_command_settings_serialization_roundtrip() {
        let cmds = CliCommandSettings::default();
        let json = serde_json::to_string(&cmds).unwrap();
        let back: CliCommandSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(cmds, back);
    }

    #[test]
    fn terminal_settings_default_includes_cli_commands() {
        let ts = TerminalSettings::default();
        assert_eq!(ts.cli_commands, CliCommandSettings::default());
    }

    #[test]
    fn terminal_settings_deserializes_without_cli_commands() {
        // Backward compat: old settings JSON without cli_commands field
        let json = r#"{"emulator":"iterm2","customCommand":"","tmuxLayout":"new_window"}"#;
        let ts: TerminalSettings = serde_json::from_str(json).unwrap();
        assert_eq!(ts.cli_commands, CliCommandSettings::default());
    }
}
