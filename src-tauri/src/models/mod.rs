use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch_base::ResolvedBase;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountMemoryOrigin {
    Pinned,
    LastUsed,
}

impl AccountMemoryOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::LastUsed => "last_used",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountMemory {
    pub account_id: String,
    pub origin: AccountMemoryOrigin,
}

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
    #[serde(default)]
    pub account_memory: HashMap<String, AccountMemory>,
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
    pub account_memory: HashMap<String, AccountMemory>,
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
            account_memory: project.account_memory.clone(),
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
    pub account_memory: HashMap<String, AccountMemory>,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Detection-backed account facts carried from the daemon command boundary to
/// pure managed-launch rendering. Tokens and provider credentials never enter
/// this shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedLaunchAccount {
    pub id: String,
    pub label: String,
    pub dir: std::path::PathBuf,
    pub logged_in: bool,
    pub is_default: bool,
}

/// Per-tool launch command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct CliCommandSettings {
    pub claude: ToolCommands,
    pub codex: ToolCommands,
    pub agy: ToolCommands,
    pub grok: ToolCommands,
    /// Runtime-only launch input. The coordination command resolves managed hook
    /// trust before pipeline rendering; this is never persisted or sent to the UI.
    #[serde(skip)]
    pub codex_bypass_hook_trust: bool,
    /// Runtime-only managed-launch input for Codex's per-turn notify command.
    /// User-authored bases remain untouched and unmanaged launches leave this
    /// unset.
    #[serde(skip)]
    pub codex_notify_executable: Option<std::path::PathBuf>,
    /// Runtime-only account-selector values for managed team members. The
    /// coordination boundary resolves these once; pure pipeline rendering
    /// never probes the ambient process environment.
    #[serde(skip)]
    pub account_selector_dirs: HashMap<String, std::path::PathBuf>,
    /// Runtime-only detection snapshot used to resolve persisted member ids at
    /// render time. Empty means detection was unavailable and forces the
    /// explicit registry-home fallback.
    #[serde(skip)]
    pub managed_accounts: HashMap<CliTool, Vec<ManagedLaunchAccount>>,
    /// Runtime-only shell-resolved bases for managed team launches. Missing
    /// entries mean resolution was unavailable and render fail-soft literally.
    #[serde(skip)]
    pub resolved_bases: HashMap<(CliTool, crate::daemon::protocol::LaunchMode), ResolvedBase>,
    /// Runtime-only copy of the harness setting that owns Grok compaction
    /// hooks. It is carried to the daemon with launch intents but omitted from
    /// persisted/default command settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grok_hooks_enabled: Option<bool>,
}

impl Default for CliCommandSettings {
    fn default() -> Self {
        Self {
            claude: crate::session_scanner::cli_tool::spec(CliTool::Claude)
                .default_commands
                .clone(),
            codex: crate::session_scanner::cli_tool::spec(CliTool::Codex)
                .default_commands
                .clone(),
            agy: crate::session_scanner::cli_tool::spec(CliTool::Agy)
                .default_commands
                .clone(),
            grok: crate::session_scanner::cli_tool::spec(CliTool::Grok)
                .default_commands
                .clone(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_selector_dirs: HashMap::new(),
            managed_accounts: HashMap::new(),
            resolved_bases: HashMap::new(),
            grok_hooks_enabled: None,
        }
    }
}

impl CliCommandSettings {
    pub fn get(&self, tool: CliTool) -> &ToolCommands {
        crate::session_scanner::cli_tool::command_settings_for(self, tool)
    }

    pub fn get_mut(&mut self, tool: CliTool) -> Option<&mut ToolCommands> {
        crate::session_scanner::cli_tool::command_settings_for_mut(self, tool)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppPlatform {
    Linux,
    Macos,
    Windows,
}

const CODEX_NATIVE_HOOKS_MIN_VERSION: (u32, u32, u32) = (0, 147, 0);
const CODEX_NATIVE_NOTIFY_MIN_VERSION: (u32, u32, u32) = (0, 147, 0);
const CODEX_QUEUE_WAKE_MIN_VERSION: (u32, u32, u32) = (0, 149, 0);
/// agy 1.1.1 reloads hooks when the workspace is trusted mid-session and
/// 1.1.10 is the first release whose `Stop` hook fires at all.
const AGY_ACTIVITY_HOOKS_MIN_VERSION: (u32, u32, u32) = (1, 1, 10);
const CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliVersions {
    pub codex: Option<String>,
    pub claude: Option<String>,
    pub agy: Option<String>,
    pub codex_compaction_hooks_supported: bool,
    pub codex_notify_supported: bool,
    pub codex_queue_wake_supported: bool,
    pub agy_hooks_supported: bool,
}

static CLI_VERSIONS: LazyLock<CliVersions> = LazyLock::new(CliVersions::probe);

impl CliVersions {
    pub fn current() -> &'static Self {
        &CLI_VERSIONS
    }

    fn probe() -> Self {
        let codex = probe_cli_version("codex");
        let claude = probe_cli_version("claude");
        let agy = probe_cli_version("agy");
        let versions = Self::from_versions(codex, claude, agy);
        tracing::info!(
            codex = ?versions.codex,
            claude = ?versions.claude,
            agy = ?versions.agy,
            codex_compaction_hooks_supported = versions.codex_compaction_hooks_supported,
            codex_notify_supported = versions.codex_notify_supported,
            codex_queue_wake_supported = versions.codex_queue_wake_supported,
            agy_hooks_supported = versions.agy_hooks_supported,
            "CLI versions detected for native harness capability gates"
        );
        versions
    }

    pub fn codex_compaction_hooks_support(&self) -> Option<bool> {
        self.codex
            .as_ref()
            .map(|_| self.codex_compaction_hooks_supported)
    }

    /// `None` when the agy version could not be resolved at all, which is not
    /// proof of an unsupported CLI and must not uninstall a working hook.
    pub fn agy_hooks_support(&self) -> Option<bool> {
        self.agy.as_ref().map(|_| self.agy_hooks_supported)
    }

    #[cfg(test)]
    fn from_outputs(codex: Option<&str>, claude: Option<&str>, agy: Option<&str>) -> Self {
        Self::from_versions(
            codex.and_then(parse_cli_version),
            claude.and_then(parse_cli_version),
            agy.and_then(parse_cli_version),
        )
    }

    fn from_versions(
        codex: Option<((u32, u32, u32), String)>,
        claude: Option<((u32, u32, u32), String)>,
        agy: Option<((u32, u32, u32), String)>,
    ) -> Self {
        let codex_parsed = codex.as_ref().map(|(version, _)| *version);
        let agy_parsed = agy.as_ref().map(|(version, _)| *version);
        Self {
            codex: codex.map(|(_, normalized)| normalized),
            claude: claude.map(|(_, normalized)| normalized),
            agy: agy.map(|(_, normalized)| normalized),
            codex_compaction_hooks_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_NATIVE_HOOKS_MIN_VERSION),
            codex_notify_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_NATIVE_NOTIFY_MIN_VERSION),
            codex_queue_wake_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_QUEUE_WAKE_MIN_VERSION),
            agy_hooks_supported: agy_parsed
                .is_some_and(|version| version >= AGY_ACTIVITY_HOOKS_MIN_VERSION),
        }
    }
}

/// Version probes shell out to the user's real CLIs. No test lane may do that,
/// and an isolated lane can opt out with `TAURHAUS_SKIP_CLI_VERSION_PROBES`.
fn cli_version_probes_disabled() -> bool {
    cfg!(test) || std::env::var_os("TAURHAUS_SKIP_CLI_VERSION_PROBES").is_some()
}

fn probe_cli_version(program: &str) -> Option<((u32, u32, u32), String)> {
    if cli_version_probes_disabled() {
        return None;
    }
    let mut command = cli_version_command(program);
    let argv = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    tracing::info!(program, argv = ?argv, "Probing CLI version");
    let output = match crate::process_utils::run_command_with_timeout(
        &mut command,
        CLI_VERSION_TIMEOUT,
        &format!("{program} --version"),
    ) {
        Ok(output) => output,
        Err(error) => {
            tracing::info!(program, error = %error, "CLI version probe unavailable");
            return None;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    tracing::info!(
        program,
        status = ?output.status.code(),
        raw_output = %raw,
        "CLI version probe completed"
    );
    if !output.status.success() {
        return None;
    }
    parse_cli_version(&raw)
}

fn cli_version_command(program: &str) -> Command {
    let distro = crate::coordination::mesh_cli::resolve_wsl_distro_for_coordination(None);
    cli_version_command_for_platform(AppPlatform::current(), program, distro.as_deref())
}

fn cli_version_command_for_platform(
    platform: AppPlatform,
    program: &str,
    configured_distro: Option<&str>,
) -> Command {
    let script = format!("{program} --version");
    match platform {
        AppPlatform::Windows => {
            let interactive_script = format!(
                r#"user_shell="$(getent passwd "$(id -u)" | cut -d: -f7)"; exec "${{user_shell:-sh}}" -ilc '{script}'"#
            );
            let mut command = crate::daemon::launcher::wsl_command();
            if let Some(distro) = configured_distro {
                command.args(crate::daemon::launcher::wsl_shell_args(
                    distro,
                    "-lc",
                    &interactive_script,
                ));
            } else {
                command.args(["-e", "sh", "-lc", interactive_script.as_str()]);
            }
            command
        }
        AppPlatform::Linux | AppPlatform::Macos => {
            let shell = std::env::var_os("SHELL")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "sh".into());
            let mut command = Command::new(shell);
            command.args(["-ilc", script.as_str()]);
            command
        }
    }
}

fn parse_cli_version(raw: &str) -> Option<((u32, u32, u32), String)> {
    raw.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches('v');
        let mut parts = token.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next()?.parse::<u32>().ok()?;
        let patch = parts
            .next()?
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()?;
        let version = (major, minor, patch);
        Some((version, format!("{major}.{minor}.{patch}")))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntry {
    pub id: String,
    pub label: String,
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub deprecated: bool,
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_tier: Option<CapabilityTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_rank: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTier {
    Frontier,
    Strong,
    Efficient,
}

impl CapabilityTier {
    pub const ALL: [Self; 3] = [Self::Frontier, Self::Strong, Self::Efficient];

    // A new variant must join ALL: this match fails to compile when one is
    // added without extending the array the doc-pin test iterates.
    const fn all_covers(tier: Self) -> bool {
        match tier {
            Self::Frontier => true,
            Self::Strong => true,
            Self::Efficient => true,
        }
    }
}

const _: () = {
    assert!(CapabilityTier::ALL.len() == 3);
    assert!(CapabilityTier::all_covers(CapabilityTier::ALL[0]));
    assert!(CapabilityTier::all_covers(CapabilityTier::ALL[1]));
    assert!(CapabilityTier::all_covers(CapabilityTier::ALL[2]));
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub claude: Vec<ModelCatalogEntry>,
    pub codex: Vec<ModelCatalogEntry>,
    pub agy: Vec<ModelCatalogEntry>,
    pub grok: Vec<ModelCatalogEntry>,
}

static MODEL_CATALOG: LazyLock<ModelCatalog> = LazyLock::new(|| ModelCatalog {
    // Team decision 2026-08-28 (docs/design/model-steering-v4-plan.md): Claude
    // roles run Fable or Opus only. The `fable`/`opus` values are CLI aliases
    // that track each family's newest model (Fable 5.1 as of 2026-09-02) — the
    // labels name the current resolution. The retired ids stay so persisted
    // roles still resolve; they carry a hint at the model that replaces them.
    claude: vec![
        model_catalog_entry("opus", "Opus 5", CLAUDE_EFFORTS, None, false, None),
        model_catalog_entry("fable", "Fable 5.1", CLAUDE_EFFORTS, None, false, None),
        model_catalog_entry("sonnet", "Sonnet", CLAUDE_EFFORTS, None, true, Some("opus")),
        model_catalog_entry("haiku", "Haiku", CLAUDE_EFFORTS, None, true, Some("opus")),
        model_catalog_entry(
            "claude-opus-4-6",
            "Claude Opus 4.6",
            CLAUDE_EFFORTS,
            None,
            true,
            Some("opus"),
        ),
        model_catalog_entry(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            CLAUDE_EFFORTS,
            None,
            true,
            Some("opus"),
        ),
    ],
    // Same decision for Codex: gpt-5.6-sol, with gpt-5.6-luna for small work.
    // terra is not used, so every retired id points at sol rather than chaining
    // a hint through another deprecated model.
    codex: vec![
        model_catalog_entry(
            "gpt-5.6-sol",
            "GPT-5.6-Sol",
            CODEX_EFFORTS_WITH_ULTRA,
            Some("low"),
            false,
            None,
        ),
        model_catalog_entry(
            "gpt-5.6-terra",
            "GPT-5.6-Terra",
            CODEX_EFFORTS_WITH_ULTRA,
            Some("medium"),
            true,
            Some("gpt-5.6-sol"),
        ),
        model_catalog_entry(
            "gpt-5.6-luna",
            "GPT-5.6-Luna",
            CODEX_EFFORTS_WITH_MAX,
            Some("medium"),
            false,
            None,
        ),
        model_catalog_entry(
            "gpt-5.5",
            "GPT-5.5",
            CODEX_EFFORTS_THROUGH_XHIGH,
            Some("medium"),
            true,
            Some("gpt-5.6-sol"),
        ),
        model_catalog_entry(
            "gpt-5.4",
            "GPT-5.4",
            CODEX_EFFORTS_THROUGH_XHIGH,
            Some("medium"),
            true,
            Some("gpt-5.6-sol"),
        ),
        model_catalog_entry(
            "gpt-5.4-mini",
            "GPT-5.4-Mini",
            CODEX_EFFORTS_THROUGH_XHIGH,
            Some("medium"),
            true,
            Some("gpt-5.6-luna"),
        ),
    ],
    agy: vec![
        model_catalog_entry(
            "gemini-3.7-flash-high",
            "Gemini 3.7 Flash (High)",
            AGY_EFFORTS,
            Some("high"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.7-flash-medium",
            "Gemini 3.7 Flash (Medium)",
            AGY_EFFORTS,
            Some("medium"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.7-flash-low",
            "Gemini 3.7 Flash (Low)",
            AGY_EFFORTS,
            Some("low"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.6-flash-high",
            "Gemini 3.6 Flash (High)",
            AGY_EFFORTS,
            Some("high"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.6-flash-medium",
            "Gemini 3.6 Flash (Medium)",
            AGY_EFFORTS,
            Some("medium"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.6-flash-low",
            "Gemini 3.6 Flash (Low)",
            AGY_EFFORTS,
            Some("low"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.5-flash-high",
            "Gemini 3.5 Flash (High)",
            AGY_EFFORTS,
            Some("high"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.5-flash-medium",
            "Gemini 3.5 Flash (Medium)",
            AGY_EFFORTS,
            Some("medium"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.5-flash-low",
            "Gemini 3.5 Flash (Low)",
            AGY_EFFORTS,
            Some("low"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.1-pro-high",
            "Gemini 3.1 Pro (High)",
            AGY_EFFORTS,
            Some("high"),
            false,
            None,
        ),
        model_catalog_entry(
            "gemini-3.1-pro-low",
            "Gemini 3.1 Pro (Low)",
            AGY_EFFORTS,
            Some("low"),
            false,
            None,
        ),
        model_catalog_entry(
            "claude-sonnet-4-6",
            "Claude Sonnet 4.6 (Thinking)",
            AGY_EFFORTS,
            None,
            false,
            None,
        ),
        model_catalog_entry(
            "claude-opus-4-6-thinking",
            "Claude Opus 4.6 (Thinking)",
            AGY_EFFORTS,
            None,
            false,
            None,
        ),
        model_catalog_entry(
            "gpt-oss-120b-medium",
            "GPT-OSS 120B (Medium)",
            AGY_EFFORTS,
            Some("medium"),
            false,
            None,
        ),
    ],
    grok: vec![
        model_catalog_entry(
            "grok-4.6",
            "Grok 4.6",
            GROK_EFFORTS_THROUGH_XHIGH,
            Some("high"),
            false,
            None,
        ),
        model_catalog_entry(
            "grok-4.5",
            "Grok 4.5",
            GROK_EFFORTS_THROUGH_HIGH,
            Some("high"),
            false,
            None,
        ),
    ],
});

const CLAUDE_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS_WITH_ULTRA: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultra"];
const CODEX_EFFORTS_WITH_MAX: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS_THROUGH_XHIGH: &[&str] = &["low", "medium", "high", "xhigh"];
const AGY_EFFORTS: &[&str] = &["low", "medium", "high"];
/// `grok models` reports the effort menu per model: `grok-4.6` accepts all four
/// (default `high`), `grok-4.5` stops at `high`. Both run a 500k context window.
const GROK_EFFORTS_THROUGH_XHIGH: &[&str] = &["low", "medium", "high", "xhigh"];
const GROK_EFFORTS_THROUGH_HIGH: &[&str] = &["low", "medium", "high"];

fn model_catalog_entry(
    id: &str,
    label: &str,
    efforts: &[&str],
    default_effort: Option<&str>,
    deprecated: bool,
    replacement: Option<&str>,
) -> ModelCatalogEntry {
    let (capability_tier, tier_rank) = if deprecated {
        (None, None)
    } else {
        model_capability(id)
    };
    ModelCatalogEntry {
        id: id.to_string(),
        label: label.to_string(),
        efforts: efforts.iter().map(|effort| (*effort).to_string()).collect(),
        default_effort: default_effort.map(str::to_string),
        deprecated,
        replacement: replacement.map(str::to_string),
        capability_tier,
        tier_rank,
    }
}

fn model_capability(id: &str) -> (Option<CapabilityTier>, Option<u32>) {
    use CapabilityTier::{Efficient, Frontier, Strong};

    let assignment = match id {
        "fable" => (Frontier, 0),
        "gpt-5.6-sol" => (Strong, 0),
        "opus" => (Strong, 1),
        "claude-opus-4-6" | "claude-opus-4-6-thinking" => (Strong, 2),
        "gemini-3.1-pro-high" => (Strong, 3),
        "grok-4.6" => (Strong, 4),
        "gpt-5.5" => (Strong, 5),
        // Preferred for batch/volume work: speed and throughput over peak
        // intelligence (operator-signed Stage-0 catalog policy).
        "gpt-5.6-luna" => (Efficient, 0),
        "gpt-5.4" => (Efficient, 1),
        "gpt-5.4-mini" => (Efficient, 2),
        id if id.starts_with("gemini-3.") && id.contains("-flash-") => (Efficient, 3),
        "gemini-3.1-pro-low" => (Efficient, 4),
        "gpt-oss-120b-medium" => (Efficient, 5),
        "grok-4.5" => (Efficient, 6),
        _ => return (None, None),
    };
    (Some(assignment.0), Some(assignment.1))
}

impl Default for ModelCatalog {
    fn default() -> Self {
        MODEL_CATALOG.clone()
    }
}

impl ModelCatalog {
    pub fn entries_for(tool: CliTool) -> &'static [ModelCatalogEntry] {
        if !crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .catalog
        {
            return &[];
        }
        match tool {
            CliTool::Claude => &MODEL_CATALOG.claude,
            CliTool::Codex => &MODEL_CATALOG.codex,
            CliTool::Agy => &MODEL_CATALOG.agy,
            CliTool::Grok => &MODEL_CATALOG.grok,
            CliTool::Unknown => &[],
        }
    }

    pub fn default_from_entries(
        entries: &[ModelCatalogEntry],
        catalog_declared: bool,
    ) -> Option<&ModelCatalogEntry> {
        catalog_declared.then(|| entries.first()).flatten()
    }

    pub fn default_for(tool: CliTool) -> Option<&'static ModelCatalogEntry> {
        let catalog_declared = crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .catalog;
        Self::default_from_entries(Self::entries_for(tool), catalog_declared)
    }

    pub fn entry_for(tool: CliTool, model_id: &str) -> Option<&'static ModelCatalogEntry> {
        Self::entries_for(tool)
            .iter()
            .find(|entry| entry.id == model_id)
    }

    pub fn supports_effort(tool: CliTool, model_id: Option<&str>, effort: &str) -> bool {
        if !crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .catalog
        {
            return false;
        }
        match tool {
            CliTool::Claude => CLAUDE_EFFORTS.contains(&effort),
            // Known catalog entry: its own effort list. Unknown (user-added /
            // newer) model: the tool-wide vocabulary — Codex validates the pair
            // itself, and dropping a declared effort silently is the bug PR 4
            // fixed. The catalog is a suggestion list, not an allowlist.
            CliTool::Codex => match model_id.and_then(|model_id| Self::entry_for(tool, model_id)) {
                Some(entry) => entry.efforts.iter().any(|allowed| allowed == effort),
                None => CODEX_EFFORTS_WITH_ULTRA.contains(&effort),
            },
            CliTool::Agy => AGY_EFFORTS.contains(&effort),
            // grok rejects an unsupported effort eagerly and names the set, so
            // a known entry is validated per model and an unknown (user-added)
            // model falls back to the widest published vocabulary.
            CliTool::Grok => match model_id.and_then(|model_id| Self::entry_for(tool, model_id)) {
                Some(entry) => entry.efforts.iter().any(|allowed| allowed == effort),
                None => GROK_EFFORTS_THROUGH_XHIGH.contains(&effort),
            },
            CliTool::Unknown => false,
        }
    }

    pub fn contains_model_id(model_id: &str) -> bool {
        MODEL_CATALOG
            .claude
            .iter()
            .chain(&MODEL_CATALOG.codex)
            .chain(&MODEL_CATALOG.agy)
            .chain(&MODEL_CATALOG.grok)
            .any(|entry| entry.id == model_id)
    }
}

impl AppPlatform {
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPlatformContract {
    pub platform: AppPlatform,
    #[serde(alias = "default_emulator")]
    pub default_emulator: String,
    #[serde(alias = "supported_emulators")]
    pub supported_emulators: Vec<String>,
    #[serde(alias = "cli_command_defaults")]
    pub cli_command_defaults: CliCommandSettings,
    #[serde(alias = "model_catalog")]
    pub model_catalog: ModelCatalog,
    #[serde(alias = "cli_versions")]
    pub cli_versions: CliVersions,
    #[serde(default)]
    pub tools: Vec<crate::session_scanner::cli_tool::CliToolDescriptor>,
}

impl Default for TerminalPlatformContract {
    fn default() -> Self {
        Self::for_platform(AppPlatform::current())
    }
}

impl TerminalPlatformContract {
    pub fn for_platform(platform: AppPlatform) -> Self {
        let (default_emulator, supported_emulators) = match platform {
            AppPlatform::Linux => ("manual", vec!["manual"]),
            AppPlatform::Macos => (
                "iterm2",
                vec!["iterm2", "ghostty", "terminal_app", "custom"],
            ),
            AppPlatform::Windows => ("windows_terminal", vec!["windows_terminal", "custom"]),
        };

        Self {
            platform,
            default_emulator: default_emulator.to_string(),
            supported_emulators: supported_emulators
                .into_iter()
                .map(str::to_string)
                .collect(),
            cli_command_defaults: CliCommandSettings::default(),
            model_catalog: ModelCatalog::default(),
            cli_versions: CliVersions::default(),
            tools: crate::session_scanner::cli_tool::descriptors(),
        }
    }

    pub fn for_runtime_platform(platform: AppPlatform) -> Self {
        let mut contract = Self::for_platform(platform);
        contract.cli_versions = CliVersions::current().clone();
        contract
    }

    pub fn supports_emulator(&self, emulator: &str) -> bool {
        self.supported_emulators
            .iter()
            .any(|value| value == emulator)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSettings {
    /// Antigravity loads hooks only in a trusted workspace, so the sink is on
    /// by default and inert until the member answers the pane's trust prompt.
    #[serde(default = "default_true")]
    #[serde(alias = "agy_hooks")]
    pub agy_hooks: bool,
    /// grok's personal hook directory is always trusted, so its compaction
    /// bridge is on by default and can be switched off.
    #[serde(default = "default_true")]
    #[serde(alias = "grok_hooks")]
    pub grok_hooks: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for HarnessSettings {
    fn default() -> Self {
        Self {
            agy_hooks: true,
            grok_hooks: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettings {
    /// Terminal emulator to use:
    /// - Windows: "windows_terminal" (default), "custom"
    /// - macOS: "iterm2" (default), "terminal_app", "ghostty", "custom"
    /// - Linux: "manual" (default)
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
    /// Harness-native feature selection.
    #[serde(default)]
    pub harness: HarnessSettings,
    /// Per-tool global defaults. Missing keys use the provider's default dir.
    pub default_account_ids: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSettingsWire {
    emulator: String,
    #[serde(alias = "custom_command")]
    custom_command: String,
    #[serde(alias = "tmux_layout")]
    tmux_layout: String,
    #[serde(default, alias = "cli_commands")]
    cli_commands: CliCommandSettings,
    #[serde(default)]
    harness: HarnessSettings,
    #[serde(default, alias = "default_account_ids")]
    default_account_ids: HashMap<String, String>,
    #[serde(default, alias = "claude_default_account_id")]
    claude_default_account_id: Option<String>,
}

impl<'de> Deserialize<'de> for TerminalSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut wire = TerminalSettingsWire::deserialize(deserializer)?;
        if let Some(account_id) = wire.claude_default_account_id.take() {
            wire.default_account_ids
                .entry("claude".to_string())
                .or_insert(account_id);
        }
        Ok(Self {
            emulator: wire.emulator,
            custom_command: wire.custom_command,
            tmux_layout: wire.tmux_layout,
            cli_commands: wire.cli_commands,
            harness: wire.harness,
            default_account_ids: wire.default_account_ids,
        })
    }
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            emulator: TerminalPlatformContract::default().default_emulator,
            custom_command: String::new(),
            tmux_layout: "new_window".into(),
            cli_commands: CliCommandSettings::default(),
            harness: HarnessSettings::default(),
            default_account_ids: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
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
    #[serde(default)]
    #[serde(alias = "terminal_contract")]
    pub terminal_contract: TerminalPlatformContract,
}

impl Settings {
    pub fn with_runtime_terminal_contract(mut self) -> Self {
        let contract = TerminalPlatformContract::for_runtime_platform(AppPlatform::current());

        if self.terminal.emulator == "default"
            || !contract.supports_emulator(&self.terminal.emulator)
        {
            self.terminal.emulator = contract.default_emulator.clone();
        }
        self.terminal_contract = contract;
        self
    }
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
    /// "connected", "busy", "disconnected", "not_configured", "reconnecting"
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
    /// True when the installed daemon differs from the bundled one — in either
    /// direction. The app pins an exact daemon protocol, so a newer daemon is
    /// as unusable as an older one and has to be replaced from the bundle.
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

    #[test]
    fn terminal_settings_default_agy_hooks_on() {
        // Regression: commit 4e9e2c5 defaulted the Antigravity hooks off
        // because their trust-gated loading was unverified; agy 1.1.22 was
        // then observed firing PreInvocation and Stop under workspace trust.
        assert!(TerminalSettings::default().harness.agy_hooks);

        let legacy: TerminalSettings = serde_json::from_value(serde_json::json!({
            "emulator": "manual",
            "custom_command": "",
            "tmux_layout": "new_window"
        }))
        .expect("legacy settings");
        assert!(legacy.harness.agy_hooks);

        let opted_out: TerminalSettings = serde_json::from_value(serde_json::json!({
            "emulator": "manual",
            "custom_command": "",
            "tmux_layout": "new_window",
            "harness": {"agy_hooks": false}
        }))
        .expect("opted-out settings");
        assert!(!opted_out.harness.agy_hooks);
    }

    // Regression: 66ab7ec added the snake_case alias for `agy_hooks` only; the
    // frontend sends `harness.grok_hooks` in snake_case too, so the Settings
    // Grok toggle was silently dropped and grok hooks could never be turned off.
    #[test]
    fn terminal_settings_grok_hooks_opt_out_reaches_the_backend() {
        assert!(TerminalSettings::default().harness.grok_hooks);
        let opted_out: TerminalSettings = serde_json::from_value(serde_json::json!({
            "emulator": "manual",
            "custom_command": "",
            "tmux_layout": "new_window",
            "harness": {"grok_hooks": false}
        }))
        .expect("settings with a snake_case grok_hooks key parse");
        assert!(!opted_out.harness.grok_hooks);
    }
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
            account_memory: HashMap::from([(
                "claude".to_string(),
                AccountMemory {
                    account_id: "account-2".to_string(),
                    origin: AccountMemoryOrigin::Pinned,
                },
            )]),
        };
        let value = serde_json::to_value(summary).expect("serialize project summary");
        assert!(value.get("activityState").is_some());
        assert!(value.get("lastActivityAt").is_some());
        assert!(value.get("isDirty").is_some());
        assert_eq!(
            value["accountMemory"]["claude"]["accountId"].as_str(),
            Some("account-2")
        );
        assert!(value.get("claudeAccountId").is_none());
        assert!(value.get("activity_state").is_none());
    }

    #[test]
    fn settings_serialize_camel_case_nested_keys() {
        let settings = Settings::default();
        let value = serde_json::to_value(settings).expect("serialize settings");
        assert!(value.get("scanDirectories").is_some());
        assert!(value.get("darkMode").is_some());
        assert!(value.get("projectDialogLastPath").is_some());
        assert!(value.get("terminalContract").is_some());
        assert!(value.get("scan_directories").is_none());
    }

    #[test]
    fn terminal_settings_migrate_the_legacy_claude_default_without_reserializing_it() {
        // Regression: commit d6839a3 persisted one Claude-specific default;
        // the generic settings blob must retain its value under the tool key
        // and retire the old key on the next save.
        let settings: TerminalSettings = serde_json::from_value(serde_json::json!({
            "emulator": "manual",
            "customCommand": "",
            "tmuxLayout": "new_window",
            "claudeDefaultAccountId": "account-2"
        }))
        .unwrap();
        assert_eq!(
            settings
                .default_account_ids
                .get("claude")
                .map(String::as_str),
            Some("account-2")
        );

        let encoded = serde_json::to_value(settings).unwrap();
        assert!(encoded.get("defaultAccountIds").is_some());
        assert!(encoded.get("claudeDefaultAccountId").is_none());
        assert!(encoded.get("claude_default_account_id").is_none());
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
            account_memory: Default::default(),
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
            account_memory: Default::default(),
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
            account_memory: Default::default(),
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
        // Antigravity
        assert_eq!(
            cmds.agy.continue_cmd,
            "agy --dangerously-skip-permissions --continue"
        );
        assert_eq!(cmds.agy.fresh, "agy --dangerously-skip-permissions");
        assert_eq!(
            cmds.agy.resume,
            "agy --dangerously-skip-permissions --conversation {session_id}"
        );
    }

    #[test]
    fn cli_command_settings_serialization_roundtrip() {
        let cmds = CliCommandSettings::default();
        let json = serde_json::to_string(&cmds).unwrap();
        let back: CliCommandSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(cmds, back);
    }

    #[test]
    fn legacy_gemini_command_key_is_ignored_without_losing_other_tools() {
        // Regression: commit 9a66d1c persisted the retired Gemini command key;
        // renaming the fixed struct used to make the entire settings object
        // fall back and discard unrelated Claude and Codex custom commands.
        let value = serde_json::json!({
            "claude": {"continueCmd": "claude custom-c", "fresh": "claude custom", "resume": "claude custom-r"},
            "codex": {"continueCmd": "codex custom-c", "fresh": "codex custom", "resume": "codex custom-r"},
            "gemini": {"continueCmd": "retired old-c", "fresh": "retired old", "resume": "retired old-r"}
        });

        let loaded: CliCommandSettings = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.claude.fresh, "claude custom");
        assert_eq!(loaded.codex.fresh, "codex custom");
        assert_eq!(loaded.agy, CliCommandSettings::default().agy);
    }

    #[test]
    fn terminal_settings_default_includes_cli_commands() {
        let ts = TerminalSettings::default();
        assert_eq!(ts.cli_commands, CliCommandSettings::default());
    }

    #[test]
    fn terminal_platform_contract_linux_defaults_are_manual_only() {
        let contract = TerminalPlatformContract::for_platform(AppPlatform::Linux);
        assert_eq!(contract.default_emulator, "manual");
        assert_eq!(contract.supported_emulators, vec!["manual"]);
        assert_eq!(contract.cli_command_defaults, CliCommandSettings::default());
    }

    // Regression: 2cf41db centralized the terminal platform contract without
    // CLI versions, so 6fe0aa3 could install Codex hooks on unsupported CLIs.
    #[test]
    fn cli_versions_gate_codex_native_capabilities() {
        let before_hooks = CliVersions::from_outputs(
            Some("codex-cli 0.146.9"),
            Some("2.1.246 (Claude Code)"),
            None,
        );
        assert_eq!(before_hooks.codex.as_deref(), Some("0.146.9"));
        assert_eq!(before_hooks.claude.as_deref(), Some("2.1.246"));
        assert!(!before_hooks.codex_compaction_hooks_supported);
        assert!(!before_hooks.codex_notify_supported);
        assert!(!before_hooks.codex_queue_wake_supported);

        let hooks_and_notify =
            CliVersions::from_outputs(Some("codex-cli 0.147.0"), Some("claude 2.1.238"), None);
        assert!(hooks_and_notify.codex_compaction_hooks_supported);
        assert!(hooks_and_notify.codex_notify_supported);
        assert!(!hooks_and_notify.codex_queue_wake_supported);

        let queue = CliVersions::from_outputs(Some("codex-cli 0.149.0"), None, None);
        assert!(queue.codex_queue_wake_supported);
    }

    // Regression: 4e9e2c5 shipped the Antigravity hook sink with no CLI gate,
    // so a pre-1.1.10 agy that never reaches a Stop hook still got it installed.
    #[test]
    fn cli_versions_gate_agy_activity_hooks() {
        let before = CliVersions::from_outputs(None, None, Some("agy version 1.1.9"));
        assert_eq!(before.agy.as_deref(), Some("1.1.9"));
        assert!(!before.agy_hooks_supported);
        assert_eq!(before.agy_hooks_support(), Some(false));

        let supported = CliVersions::from_outputs(None, None, Some("1.1.10"));
        assert_eq!(supported.agy.as_deref(), Some("1.1.10"));
        assert!(supported.agy_hooks_supported);
        assert_eq!(supported.agy_hooks_support(), Some(true));

        let unknown = CliVersions::from_outputs(None, None, None);
        assert_eq!(unknown.agy, None);
        assert_eq!(unknown.agy_hooks_support(), None);
    }

    // Regression: c0aa59a used a non-interactive login shell for the version
    // probe, but managed panes source interactive rc files where nvm installs Codex.
    #[test]
    fn windows_cli_version_probe_uses_configured_distro_interactive_shell() {
        let command = cli_version_command_for_platform(
            AppPlatform::Windows,
            "codex",
            Some("Taurhaus-Distro"),
        );

        assert_eq!(command.get_program(), "wsl");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(&args[..5], ["-d", "Taurhaus-Distro", "-e", "sh", "-lc"]);
        assert!(args[5].contains(r#"exec "${user_shell:-sh}" -ilc"#));
        assert!(args[5].ends_with("'codex --version'"));
    }

    // Regression: c0aa59a switched the Unix probe to `-lc`, which still omits
    // interactive rc files and cannot resolve this host's nvm-installed Codex.
    #[test]
    fn unix_cli_version_probe_uses_interactive_login_shell() {
        let command = cli_version_command_for_platform(AppPlatform::Linux, "codex", None);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_ne!(command.get_program(), "codex");
        assert_eq!(args, vec!["-ilc", "codex --version"]);
    }

    // Regression: 61e9a24 made `Default` launch two blocking subprocesses,
    // making plain settings defaults host-dependent and potentially 10 s slow.
    #[test]
    fn cli_versions_default_is_inert() {
        assert_eq!(
            CliVersions::default(),
            CliVersions {
                codex: None,
                claude: None,
                agy: None,
                codex_compaction_hooks_supported: false,
                codex_notify_supported: false,
                codex_queue_wake_supported: false,
                agy_hooks_supported: false,
            }
        );
    }

    #[test]
    fn terminal_platform_contract_macos_defaults_include_supported_apps() {
        let contract = TerminalPlatformContract::for_platform(AppPlatform::Macos);
        assert_eq!(contract.default_emulator, "iterm2");
        assert_eq!(
            contract.supported_emulators,
            vec!["iterm2", "ghostty", "terminal_app", "custom"]
        );
    }

    #[test]
    fn terminal_platform_contract_windows_defaults_include_custom() {
        let contract = TerminalPlatformContract::for_platform(AppPlatform::Windows);
        assert_eq!(contract.default_emulator, "windows_terminal");
        assert_eq!(
            contract.supported_emulators,
            vec!["windows_terminal", "custom"]
        );
    }

    #[test]
    // Regression: PR 5a/5b review — `supports_effort` treated the catalog as an
    // allowlist, so a user-added Codex model (e.g. a newer slug not yet in the
    // static list) silently lost its declared reasoning effort.
    fn codex_effort_is_accepted_for_models_outside_the_catalog() {
        assert!(ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.7-nova"),
            "high"
        ));
        assert!(ModelCatalog::supports_effort(CliTool::Codex, None, "xhigh"));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.7-nova"),
            "turbo"
        ));
        // Known entries keep their own list (luna has no `ultra`).
        assert!(!ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.6-luna"),
            "ultra"
        ));
        assert!(ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.6-sol"),
            "ultra"
        ));
    }

    #[test]
    fn model_catalog_defaults_are_explicit_per_tool() {
        assert_eq!(
            ModelCatalog::default_for(CliTool::Claude)
                .expect("Claude catalog")
                .id,
            "opus"
        );
        assert_eq!(
            ModelCatalog::default_for(CliTool::Codex)
                .expect("Codex catalog")
                .id,
            "gpt-5.6-sol"
        );
        assert_eq!(
            ModelCatalog::default_for(CliTool::Agy)
                .expect("Antigravity catalog")
                .id,
            "gemini-3.7-flash-high"
        );
        assert_eq!(
            ModelCatalog::default_for(CliTool::Grok)
                .expect("Grok catalog")
                .id,
            "grok-4.6"
        );
    }

    #[test]
    fn claude_catalog_leads_with_opus_then_fable() {
        // Team decision 2026-08-28 (docs/design/model-steering-v4-plan.md):
        // Claude roles run Fable 5 or Opus 5 only. The retired ids stay in the
        // catalog so persisted roles keep resolving, but carry a hint.
        assert_eq!(
            ModelCatalog::entries_for(CliTool::Claude)
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            [
                "opus",
                "fable",
                "sonnet",
                "haiku",
                "claude-opus-4-6",
                "claude-sonnet-4-5",
            ]
        );

        let fable = ModelCatalog::entry_for(CliTool::Claude, "fable").expect("fable");
        assert_eq!(fable.label, "Fable 5.1");
        assert_eq!(fable.efforts, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(fable.default_effort, None);
        assert!(!fable.deprecated);
        assert!(
            !ModelCatalog::entry_for(CliTool::Claude, "opus")
                .expect("opus")
                .deprecated
        );

        for id in ["sonnet", "haiku", "claude-opus-4-6", "claude-sonnet-4-5"] {
            let entry =
                ModelCatalog::entry_for(CliTool::Claude, id).unwrap_or_else(|| panic!("{id}"));
            assert!(entry.deprecated, "{id} is deprecated");
            assert_eq!(
                entry.replacement.as_deref(),
                Some("opus"),
                "{id} replacement"
            );
        }
    }

    #[test]
    fn retired_codex_models_point_at_sol() {
        // Same decision: Codex roles run gpt-5.6-sol, with gpt-5.6-luna for
        // small work. terra is not used, so no hint may still point at it.
        for id in ["gpt-5.6-terra", "gpt-5.5", "gpt-5.4"] {
            let entry =
                ModelCatalog::entry_for(CliTool::Codex, id).unwrap_or_else(|| panic!("{id}"));
            assert!(entry.deprecated, "{id} is deprecated");
            assert_eq!(
                entry.replacement.as_deref(),
                Some("gpt-5.6-sol"),
                "{id} replacement"
            );
        }

        for id in ["gpt-5.6-sol", "gpt-5.6-luna"] {
            let entry =
                ModelCatalog::entry_for(CliTool::Codex, id).unwrap_or_else(|| panic!("{id}"));
            assert!(!entry.deprecated, "{id} is current");
            assert_eq!(entry.replacement, None, "{id} needs no replacement");
        }
    }

    #[test]
    fn grok_catalog_matches_the_verified_1_0_5_models() {
        // Regression: commit bfecae9 had no grok catalog, so `grok models`'
        // verified per-model effort menus could not gate a launch and an
        // unsupported effort would only fail inside the CLI.
        assert_eq!(ModelCatalog::entries_for(CliTool::Grok).len(), 2);
        let default = ModelCatalog::entry_for(CliTool::Grok, "grok-4.6").expect("grok-4.6");
        assert_eq!(default.default_effort.as_deref(), Some("high"));
        assert_eq!(default.efforts, ["low", "medium", "high", "xhigh"]);
        let previous = ModelCatalog::entry_for(CliTool::Grok, "grok-4.5").expect("grok-4.5");
        assert_eq!(previous.efforts, ["low", "medium", "high"]);

        assert!(ModelCatalog::supports_effort(
            CliTool::Grok,
            Some("grok-4.6"),
            "xhigh"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Grok,
            Some("grok-4.5"),
            "xhigh"
        ));
        // An id the static catalog does not know still renders a published
        // effort — the catalog is a suggestion list, not an allowlist.
        assert!(ModelCatalog::supports_effort(
            CliTool::Grok,
            Some("grok-5.0"),
            "xhigh"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Grok,
            Some("grok-4.6"),
            "ultra"
        ));
    }

    #[test]
    fn agy_catalog_matches_the_verified_1_1_22_models() {
        // Regression: commit 5680a7a retained Antigravity CLI's single stale model
        // after Google's supported harness changed to Antigravity CLI 1.1.22.
        assert_eq!(ModelCatalog::entries_for(CliTool::Agy).len(), 14);
        assert!(ModelCatalog::entry_for(CliTool::Agy, "claude-opus-4-6-thinking").is_some());
        assert!(ModelCatalog::entry_for(CliTool::Agy, "gpt-oss-120b-medium").is_some());
        assert!(ModelCatalog::supports_effort(
            CliTool::Agy,
            Some("gemini-3.7-flash-high"),
            "medium"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Agy,
            Some("gemini-3.7-flash-high"),
            "xhigh"
        ));
    }

    #[test]
    fn model_catalog_deprecated_entries_name_replacements() {
        let catalog = ModelCatalog::default();
        let deprecated = catalog
            .codex
            .iter()
            .find(|entry| entry.id == "gpt-5.4")
            .expect("gpt-5.4 catalog entry");

        assert!(deprecated.deprecated);
        assert_eq!(deprecated.replacement.as_deref(), Some("gpt-5.6-sol"));
    }

    #[test]
    fn deprecated_model_catalog_entries_are_untiered() {
        // Regression: commit 90625e7a assigned routing tiers to deprecated
        // catalog entries, making replacement-only models routable.
        for entry in ModelCatalog::default()
            .claude
            .into_iter()
            .chain(ModelCatalog::default().codex)
            .chain(ModelCatalog::default().agy)
            .chain(ModelCatalog::default().grok)
            .filter(|entry| entry.deprecated)
        {
            assert_eq!(entry.capability_tier, None, "{} tier", entry.id);
            assert_eq!(entry.tier_rank, None, "{} rank", entry.id);
        }
    }

    #[test]
    fn model_catalog_carries_signed_off_capability_tiers_and_ranks() {
        let expected = [
            (
                CliTool::Claude,
                "fable",
                Some(CapabilityTier::Frontier),
                Some(0),
            ),
            (
                CliTool::Codex,
                "gpt-5.6-sol",
                Some(CapabilityTier::Strong),
                Some(0),
            ),
            (
                CliTool::Claude,
                "opus",
                Some(CapabilityTier::Strong),
                Some(1),
            ),
            (CliTool::Claude, "claude-opus-4-6", None, None),
            (
                CliTool::Agy,
                "claude-opus-4-6-thinking",
                Some(CapabilityTier::Strong),
                Some(2),
            ),
            (
                CliTool::Agy,
                "gemini-3.1-pro-high",
                Some(CapabilityTier::Strong),
                Some(3),
            ),
            (
                CliTool::Grok,
                "grok-4.6",
                Some(CapabilityTier::Strong),
                Some(4),
            ),
            (CliTool::Codex, "gpt-5.5", None, None),
            (
                CliTool::Codex,
                "gpt-5.6-luna",
                Some(CapabilityTier::Efficient),
                Some(0),
            ),
            (CliTool::Codex, "gpt-5.4", None, None),
            (CliTool::Codex, "gpt-5.4-mini", None, None),
            (
                CliTool::Agy,
                "gemini-3.7-flash-high",
                Some(CapabilityTier::Efficient),
                Some(3),
            ),
            (
                CliTool::Agy,
                "gemini-3.6-flash-medium",
                Some(CapabilityTier::Efficient),
                Some(3),
            ),
            (
                CliTool::Agy,
                "gemini-3.5-flash-low",
                Some(CapabilityTier::Efficient),
                Some(3),
            ),
            (
                CliTool::Agy,
                "gemini-3.1-pro-low",
                Some(CapabilityTier::Efficient),
                Some(4),
            ),
            (
                CliTool::Agy,
                "gpt-oss-120b-medium",
                Some(CapabilityTier::Efficient),
                Some(5),
            ),
            (
                CliTool::Grok,
                "grok-4.5",
                Some(CapabilityTier::Efficient),
                Some(6),
            ),
            (CliTool::Codex, "gpt-5.6-terra", None, None),
            (CliTool::Claude, "sonnet", None, None),
            (CliTool::Claude, "haiku", None, None),
        ];

        for (tool, id, tier, rank) in expected {
            let entry = ModelCatalog::entry_for(tool, id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!(entry.capability_tier, tier, "{id} tier");
            assert_eq!(entry.tier_rank, rank, "{id} rank");
        }
    }

    #[test]
    fn routing_design_signed_off_tier_names_match_serialized_vocabulary() {
        // This path is compiled into the test: a docs sweep that moves the
        // design note must update this pin (the doc carries the same warning
        // beside its signed-off table).
        let design = include_str!("../../../docs/design/role-first-model-routing.md");
        let signed_off_table = design
            .split_once("the signed-off table:**")
            .expect("Stage-0 signed-off table marker")
            .1
            .split_once("Three rules the review produced:")
            .expect("end of Stage-0 signed-off table")
            .0;
        let documented = signed_off_table
            .lines()
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|line| line.split_once('`').map(|(tier, _)| tier))
            .collect::<Vec<_>>();
        let serialized = CapabilityTier::ALL
            .iter()
            .map(|tier| {
                serde_json::to_string(tier)
                    .expect("serialize capability tier")
                    .trim_matches('"')
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(documented, serialized);
    }

    #[test]
    fn routing_design_marks_stage_one_as_shipped_when_merged() {
        let design = include_str!("../../../docs/design/role-first-model-routing.md");
        let stage_one = design
            .split_once("### Stage 1 — telemetry: make cost-per-accepted-task measurable")
            .expect("Stage 1 roadmap row")
            .1
            .split_once("### Stage 2")
            .expect("end of Stage 1 roadmap row")
            .0;

        assert!(stage_one.contains("**Status: shipped when merged.**"));
    }

    #[test]
    fn model_catalog_validates_effort_per_tool_and_codex_model() {
        assert!(ModelCatalog::supports_effort(
            CliTool::Claude,
            Some("opus"),
            "max"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Claude,
            Some("opus"),
            "ultra"
        ));
        assert!(ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.6-sol"),
            "ultra"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.6-luna"),
            "ultra"
        ));
        assert!(ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.5"),
            "xhigh"
        ));
        assert!(!ModelCatalog::supports_effort(
            CliTool::Codex,
            Some("gpt-5.5"),
            "max"
        ));
        assert!(ModelCatalog::supports_effort(
            CliTool::Agy,
            Some("gemini-3.1-pro"),
            "high"
        ));
    }

    #[test]
    fn terminal_platform_contract_serializes_model_catalog_as_camel_case() {
        let contract = TerminalPlatformContract::for_platform(AppPlatform::Linux);
        let value = serde_json::to_value(contract).expect("serialize terminal contract");

        assert_eq!(value["modelCatalog"]["codex"][0]["id"], "gpt-5.6-sol");
        assert!(value["modelCatalog"]["codex"][0]
            .get("defaultEffort")
            .is_some());
        assert_eq!(
            value["modelCatalog"]["codex"][0]["capabilityTier"],
            "strong"
        );
        assert_eq!(value["modelCatalog"]["codex"][0]["tierRank"], 0);
        assert!(value["cliVersions"].get("codex").is_some());
        assert!(value["cliVersions"]
            .get("codexQueueWakeSupported")
            .is_some());
    }

    #[test]
    fn terminal_settings_deserializes_without_cli_commands() {
        // Backward compat: old settings JSON without cli_commands field
        let json = r#"{"emulator":"iterm2","customCommand":"","tmuxLayout":"new_window"}"#;
        let ts: TerminalSettings = serde_json::from_str(json).unwrap();
        assert_eq!(ts.cli_commands, CliCommandSettings::default());
    }

    #[test]
    fn settings_runtime_contract_migrates_legacy_linux_default_emulator() {
        let settings = Settings {
            terminal: TerminalSettings {
                emulator: "default".into(),
                custom_command: String::new(),
                tmux_layout: "new_window".into(),
                cli_commands: CliCommandSettings::default(),
                harness: HarnessSettings::default(),
                default_account_ids: HashMap::new(),
            },
            ..Settings::default()
        }
        .with_runtime_terminal_contract();

        let expected_default = TerminalPlatformContract::default().default_emulator;
        assert_eq!(settings.terminal.emulator, expected_default);
        assert_eq!(settings.terminal_contract.platform, AppPlatform::current());
        assert_eq!(
            settings.terminal_contract.cli_versions,
            CliVersions::current().clone()
        );
    }
}
