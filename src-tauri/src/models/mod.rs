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
pub struct ActivityThresholds {
    pub active_days: i64,
    pub recent_days: i64,
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
    pub fn compute(last_activity_at: Option<&str>, thresholds: &ActivityThresholds, now: DateTime<Utc>) -> Self {
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
    pub fn from_project(project: &Project, thresholds: &ActivityThresholds, now: DateTime<Utc>) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            activity_state: ActivityState::compute(project.last_activity_at.as_deref(), thresholds, now),
            last_activity_at: project.last_activity_at.clone(),
            branch: project.cached_branch.clone(),
            is_dirty: project.cached_is_dirty,
        }
    }
}

/// Full project details sent to the frontend for the detail view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
pub struct SessionSummary {
    pub id: String,
    pub project_id: String,
    pub date: String,
    pub summary: String,
}

/// Full session detail including extensible metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalSettings {
    /// "windows_terminal" (default) or "custom"
    pub emulator: String,
    /// Command template when emulator is "custom".
    /// Placeholders: {distro}, {tmux_session}
    pub custom_command: String,
    /// Tmux layout strategy: "new_window" (default), "split", "per_project"
    pub tmux_layout: String,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            emulator: "windows_terminal".into(),
            custom_command: String::new(),
            tmux_layout: "new_window".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub scan_directories: Vec<String>,
    pub thresholds: ActivityThresholds,
    pub ignore_patterns: Vec<String>,
    pub daemon: DaemonSettings,
    #[serde(default)]
    pub code_theme: CodeThemeSettings,
    #[serde(default)]
    pub terminal: TerminalSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonSettings {
    pub port: u16,
    pub path: String,
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
pub struct DaemonStatus {
    /// "connected", "disconnected", "not_configured", "reconnecting"
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

// ---------------------------------------------------------------------------
// IPC response types (used by commands, implemented in later phases)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitStatus {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
pub struct CommitFile {
    pub path: String,
    /// One of: "added", "modified", "deleted", "renamed"
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffLine {
    pub origin: char,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub project_id: String,
    pub entity_type: String,
    pub file_path: Option<String>,
    pub snippet: String,
    pub score: f32,
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
}
