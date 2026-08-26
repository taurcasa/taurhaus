use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use crate::session_scanner::cli_tool::CliTool;

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

/// Per-tool launch command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliCommandSettings {
    pub claude: ToolCommands,
    pub codex: ToolCommands,
    pub gemini: ToolCommands,
    /// Runtime-only launch input. The coordination command resolves managed hook
    /// trust before pipeline rendering; this is never persisted or sent to the UI.
    #[serde(skip)]
    pub codex_bypass_hook_trust: bool,
    /// Runtime-only managed-launch input for Codex's per-turn notify command.
    /// User-authored bases remain untouched and unmanaged launches leave this
    /// unset.
    #[serde(skip)]
    pub codex_notify_executable: Option<std::path::PathBuf>,
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
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
        }
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
const CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliVersions {
    pub codex: Option<String>,
    pub claude: Option<String>,
    pub codex_compaction_hooks_supported: bool,
    pub codex_notify_supported: bool,
    pub codex_queue_wake_supported: bool,
}

static CLI_VERSIONS: LazyLock<CliVersions> = LazyLock::new(CliVersions::probe);

impl CliVersions {
    pub fn current() -> &'static Self {
        &CLI_VERSIONS
    }

    fn probe() -> Self {
        let codex = probe_cli_version("codex");
        let claude = probe_cli_version("claude");
        let versions = Self::from_versions(codex, claude);
        tracing::info!(
            codex = ?versions.codex,
            claude = ?versions.claude,
            codex_compaction_hooks_supported = versions.codex_compaction_hooks_supported,
            codex_notify_supported = versions.codex_notify_supported,
            codex_queue_wake_supported = versions.codex_queue_wake_supported,
            "CLI versions detected for native harness capability gates"
        );
        versions
    }

    pub fn codex_compaction_hooks_support(&self) -> Option<bool> {
        self.codex
            .as_ref()
            .map(|_| self.codex_compaction_hooks_supported)
    }

    #[cfg(test)]
    fn from_outputs(codex: Option<&str>, claude: Option<&str>) -> Self {
        Self::from_versions(
            codex.and_then(parse_cli_version),
            claude.and_then(parse_cli_version),
        )
    }

    fn from_versions(
        codex: Option<((u32, u32, u32), String)>,
        claude: Option<((u32, u32, u32), String)>,
    ) -> Self {
        let codex_parsed = codex.as_ref().map(|(version, _)| *version);
        Self {
            codex: codex.map(|(_, normalized)| normalized),
            claude: claude.map(|(_, normalized)| normalized),
            codex_compaction_hooks_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_NATIVE_HOOKS_MIN_VERSION),
            codex_notify_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_NATIVE_NOTIFY_MIN_VERSION),
            codex_queue_wake_supported: codex_parsed
                .is_some_and(|version| version >= CODEX_QUEUE_WAKE_MIN_VERSION),
        }
    }
}

fn probe_cli_version(program: &str) -> Option<((u32, u32, u32), String)> {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub claude: Vec<ModelCatalogEntry>,
    pub codex: Vec<ModelCatalogEntry>,
    pub gemini: Vec<ModelCatalogEntry>,
}

static MODEL_CATALOG: LazyLock<ModelCatalog> = LazyLock::new(|| ModelCatalog {
    claude: vec![
        model_catalog_entry("opus", "Opus 5", CLAUDE_EFFORTS, None, false, None),
        model_catalog_entry("sonnet", "Sonnet", CLAUDE_EFFORTS, None, false, None),
        model_catalog_entry("haiku", "Haiku", CLAUDE_EFFORTS, None, false, None),
        model_catalog_entry(
            "claude-opus-4-6",
            "Claude Opus 4.6",
            CLAUDE_EFFORTS,
            None,
            false,
            None,
        ),
        model_catalog_entry(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            CLAUDE_EFFORTS,
            None,
            false,
            None,
        ),
    ],
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
            false,
            None,
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
            false,
            None,
        ),
        model_catalog_entry(
            "gpt-5.4",
            "GPT-5.4",
            CODEX_EFFORTS_THROUGH_XHIGH,
            Some("medium"),
            true,
            Some("gpt-5.6-terra"),
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
    gemini: vec![model_catalog_entry(
        "gemini-3.1-pro",
        "Gemini 3.1 Pro",
        &[],
        None,
        false,
        None,
    )],
});

const CLAUDE_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS_WITH_ULTRA: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultra"];
const CODEX_EFFORTS_WITH_MAX: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS_THROUGH_XHIGH: &[&str] = &["low", "medium", "high", "xhigh"];

fn model_catalog_entry(
    id: &str,
    label: &str,
    efforts: &[&str],
    default_effort: Option<&str>,
    deprecated: bool,
    replacement: Option<&str>,
) -> ModelCatalogEntry {
    ModelCatalogEntry {
        id: id.to_string(),
        label: label.to_string(),
        efforts: efforts.iter().map(|effort| (*effort).to_string()).collect(),
        default_effort: default_effort.map(str::to_string),
        deprecated,
        replacement: replacement.map(str::to_string),
    }
}

impl Default for ModelCatalog {
    fn default() -> Self {
        MODEL_CATALOG.clone()
    }
}

impl ModelCatalog {
    pub fn default_for(tool: CliTool) -> &'static ModelCatalogEntry {
        match tool {
            CliTool::Claude => &MODEL_CATALOG.claude[0],
            CliTool::Codex => &MODEL_CATALOG.codex[0],
            CliTool::Gemini => &MODEL_CATALOG.gemini[0],
        }
    }

    pub fn entry_for(tool: CliTool, model_id: &str) -> Option<&'static ModelCatalogEntry> {
        let entries = match tool {
            CliTool::Claude => &MODEL_CATALOG.claude,
            CliTool::Codex => &MODEL_CATALOG.codex,
            CliTool::Gemini => &MODEL_CATALOG.gemini,
        };
        entries.iter().find(|entry| entry.id == model_id)
    }

    pub fn supports_effort(tool: CliTool, model_id: Option<&str>, effort: &str) -> bool {
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
            CliTool::Gemini => false,
        }
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
    #[serde(default)]
    #[serde(alias = "codex_compaction")]
    pub codex_compaction: CodexCompactionMode,
}

impl Default for HarnessSettings {
    fn default() -> Self {
        Self {
            codex_compaction: CodexCompactionMode::Transcript,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodexCompactionMode {
    Hooks,
    #[default]
    Transcript,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Harness-native feature selection. Codex hooks are the verified default;
    /// transcript parsing remains the explicit compatibility fallback.
    #[serde(default)]
    pub harness: HarnessSettings,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            emulator: TerminalPlatformContract::default().default_emulator,
            custom_command: String::new(),
            tmux_layout: "new_window".into(),
            cli_commands: CliCommandSettings::default(),
            harness: HarnessSettings::default(),
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

    // Regression: 0b87699 offered no Codex compaction source setting, leaving
    // the unstable transcript tailer as the only path.
    #[test]
    fn terminal_settings_default_codex_compaction_to_transcript() {
        let settings = TerminalSettings::default();
        assert_eq!(
            settings.harness.codex_compaction,
            CodexCompactionMode::Transcript
        );

        let legacy: TerminalSettings = serde_json::from_value(serde_json::json!({
            "emulator": "manual",
            "custom_command": "",
            "tmux_layout": "new_window"
        }))
        .expect("legacy settings");
        assert_eq!(
            legacy.harness.codex_compaction,
            CodexCompactionMode::Transcript
        );
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
        assert!(value.get("terminalContract").is_some());
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
        let before_hooks =
            CliVersions::from_outputs(Some("codex-cli 0.146.9"), Some("2.1.246 (Claude Code)"));
        assert_eq!(before_hooks.codex.as_deref(), Some("0.146.9"));
        assert_eq!(before_hooks.claude.as_deref(), Some("2.1.246"));
        assert!(!before_hooks.codex_compaction_hooks_supported);
        assert!(!before_hooks.codex_notify_supported);
        assert!(!before_hooks.codex_queue_wake_supported);

        let hooks_and_notify =
            CliVersions::from_outputs(Some("codex-cli 0.147.0"), Some("claude 2.1.238"));
        assert!(hooks_and_notify.codex_compaction_hooks_supported);
        assert!(hooks_and_notify.codex_notify_supported);
        assert!(!hooks_and_notify.codex_queue_wake_supported);

        let queue = CliVersions::from_outputs(Some("codex-cli 0.149.0"), None);
        assert!(queue.codex_queue_wake_supported);
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
                codex_compaction_hooks_supported: false,
                codex_notify_supported: false,
                codex_queue_wake_supported: false,
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
        assert_eq!(ModelCatalog::default_for(CliTool::Claude).id, "opus");
        assert_eq!(ModelCatalog::default_for(CliTool::Codex).id, "gpt-5.6-sol");
        assert_eq!(
            ModelCatalog::default_for(CliTool::Gemini).id,
            "gemini-3.1-pro"
        );
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
        assert_eq!(deprecated.replacement.as_deref(), Some("gpt-5.6-terra"));
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
        assert!(!ModelCatalog::supports_effort(
            CliTool::Gemini,
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
