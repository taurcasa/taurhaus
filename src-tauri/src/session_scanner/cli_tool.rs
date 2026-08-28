//! CLI tool abstraction — detect and configure multiple AI coding tools.
//!
//! Supports Claude Code, Codex CLI, Antigravity CLI, and Grok CLI. Each tool
//! has its own process signature, session directory layout, and launch
//! commands.

use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::models::{CliCommandSettings, ToolCommands};

/// Which CLI tool a session belongs to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliTool {
    #[default]
    Claude,
    Codex,
    Agy,
    Grok,
    /// A persisted tool value that this build no longer supports.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCliToolError {
    raw: String,
}

impl ParseCliToolError {
    fn new(raw: &str) -> Self {
        Self {
            raw: raw.trim().to_string(),
        }
    }
}

impl std::fmt::Display for ParseCliToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported cli tool '{}'", self.raw)
    }
}

impl std::error::Error for ParseCliToolError {}

impl FromStr for CliTool {
    type Err = ParseCliToolError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.trim().to_ascii_lowercase();
        all()
            .iter()
            .find(|entry| entry.name == normalized)
            .map(|entry| entry.tool)
            .ok_or_else(|| ParseCliToolError::new(raw))
    }
}

impl CliTool {
    /// Parse a CLI tool string with coordination aliases.
    pub fn from_alias(raw: &str) -> Result<Self, ParseCliToolError> {
        let normalized = raw.trim().to_ascii_lowercase();
        all()
            .iter()
            .find(|entry| entry.aliases.contains(&normalized.as_str()))
            .map(|entry| entry.tool)
            .ok_or_else(|| ParseCliToolError::new(raw))
    }

    /// Decode storage/database text without turning a retired provider into a
    /// different supported harness.
    pub fn from_persisted(raw: &str) -> Self {
        raw.parse().unwrap_or(Self::Unknown)
    }
}

impl std::fmt::Display for CliTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(spec(*self).name)
    }
}

/// How a reasoning-effort value is expressed by a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortFlag {
    Argument {
        flag: &'static str,
    },
    Config {
        flag: &'static str,
        key: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionRoot {
    ToolHome,
    AppManagedClaudeDir,
}

/// Capability declarations consumed by tool-agnostic call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliCapabilities {
    pub model_flag: Option<&'static str>,
    pub effort_flag: Option<EffortFlag>,
    pub auto_approve_flag: Option<&'static str>,
    pub display_name_flag: Option<&'static str>,
    pub team_flags: bool,
    pub native_inbox_poller: bool,
    pub session_source: bool,
    pub runtime_session_capture: bool,
    pub authoritative_idle: bool,
    pub compaction_hook: bool,
    /// The harness also loads another vendor's hook registrations, so one
    /// compaction can invoke the bridge more than once.
    pub compaction_hook_compat_import: bool,
    pub transcript_parser: bool,
    pub transcript_compaction_signals: bool,
    pub catalog: bool,
    pub session_root: SessionRoot,
    pub account_selector: Option<&'static str>,
    pub account_selection: bool,
    pub team_config_namespace: bool,
    pub usage: bool,
    /// Why this harness reports no usage windows, shown where a meter would be.
    pub usage_note: Option<&'static str>,
    pub notify_sink: bool,
    pub hook_trust: bool,
    /// The app supplies a managed account home for team launches.
    pub managed_home: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopStrategy {
    SlashExit,
    Interrupt,
}

/// One registry record for a supported CLI harness.
pub struct CliToolSpec {
    pub tool: CliTool,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub argv_signatures: &'static [&'static str],
    /// Flags that identify a non-interactive invocation of this executable.
    pub non_session_flags: &'static [&'static str],
    /// First positional commands that are utilities rather than sessions.
    pub non_session_subcommands: &'static [&'static str],
    /// Global flags whose following argv token is a value, not a subcommand.
    pub argv_value_flags: &'static [&'static str],
    pub model_prefixes: &'static [&'static str],
    pub model_markers: &'static [&'static str],
    pub default_commands: ToolCommands,
    pub label: &'static str,
    pub accent: &'static str,
    pub medallion_accent: &'static str,
    pub default_agent_role_id: &'static str,
    pub capabilities: CliCapabilities,
    pub stop_strategy: StopStrategy,
    /// Presence directory next to the transcript directory, when a released
    /// flock is the harness's clean-stop confirmation.
    pub stop_presence_dir: Option<&'static str>,
    /// The harness clears its own live-session registry on a clean exit, so the
    /// session source answering "no session" confirms the stop.
    pub stop_registry_release: bool,
    /// Add the harness-specific exit and mesh inbox hints to onboarding text.
    pub onboarding_exit_hint: bool,
    /// One extra onboarding sentence about how this harness receives messages.
    pub onboarding_delivery_hint: Option<&'static str>,
    pub pane_binding: bool,
    pub display_name: &'static str,
    pub settings_label: &'static str,
    /// Base directory name under `$HOME` (e.g., ".claude", ".codex", ".gemini").
    pub base_dir_name: &'static str,
    /// Subdirectory within the base dir that contains project session data.
    pub projects_subdir: &'static str,
    /// File extension for session transcripts.
    pub session_extension: &'static str,
    /// Command to gracefully exit the CLI in a terminal.
    pub exit_command: &'static str,
}

static TOOL_SPECS: LazyLock<[CliToolSpec; 4]> = LazyLock::new(|| {
    [
        CliToolSpec {
            tool: CliTool::Claude,
            name: "claude",
            aliases: &["claude", "claude_native"],
            argv_signatures: &["claude", "@anthropic-ai/claude-code"],
            non_session_flags: &[],
            non_session_subcommands: &[],
            argv_value_flags: &[],
            model_prefixes: &["claude-"],
            model_markers: &["claude"],
            default_commands: ToolCommands {
                continue_cmd: "claude --dangerously-skip-permissions --continue".into(),
                fresh: "claude --dangerously-skip-permissions".into(),
                resume: "claude --dangerously-skip-permissions --resume".into(),
            },
            label: "Claude",
            accent: "emerald",
            medallion_accent: "amber",
            default_agent_role_id: "claude-reviewer",
            capabilities: CliCapabilities {
                model_flag: Some("--model"),
                effort_flag: Some(EffortFlag::Argument { flag: "--effort" }),
                auto_approve_flag: Some("--dangerously-skip-permissions"),
                display_name_flag: Some("-n"),
                team_flags: true,
                native_inbox_poller: true,
                session_source: true,
                runtime_session_capture: true,
                authoritative_idle: true,
                compaction_hook: true,
                compaction_hook_compat_import: false,
                transcript_parser: true,
                transcript_compaction_signals: false,
                catalog: true,
                session_root: SessionRoot::AppManagedClaudeDir,
                account_selector: Some("CLAUDE_CONFIG_DIR"),
                account_selection: true,
                team_config_namespace: true,
                usage: true,
                usage_note: None,
                notify_sink: false,
                hook_trust: false,
                managed_home: false,
            },
            stop_strategy: StopStrategy::SlashExit,
            stop_presence_dir: None,
            stop_registry_release: false,
            onboarding_exit_hint: false,
            onboarding_delivery_hint: None,
            pane_binding: false,
            display_name: "Claude Code",
            settings_label: "Claude Code",
            base_dir_name: ".claude",
            projects_subdir: "projects",
            session_extension: "jsonl",
            exit_command: "/exit",
        },
        CliToolSpec {
            tool: CliTool::Codex,
            name: "codex",
            aliases: &["codex", "mesh", "mesh_bridged"],
            argv_signatures: &["codex", "@openai/codex"],
            non_session_flags: &[],
            non_session_subcommands: &[],
            argv_value_flags: &[],
            model_prefixes: &["gpt-"],
            model_markers: &[],
            default_commands: ToolCommands {
                continue_cmd: "codex --yolo".into(),
                fresh: "codex --yolo".into(),
                resume: "codex resume --last --yolo".into(),
            },
            label: "Codex",
            accent: "sky",
            medallion_accent: "emerald",
            default_agent_role_id: "codex-developer",
            capabilities: CliCapabilities {
                model_flag: Some("-m"),
                effort_flag: Some(EffortFlag::Config {
                    flag: "-c",
                    key: "model_reasoning_effort",
                }),
                auto_approve_flag: Some("--yolo"),
                display_name_flag: None,
                team_flags: false,
                native_inbox_poller: false,
                session_source: true,
                runtime_session_capture: true,
                authoritative_idle: true,
                compaction_hook: true,
                compaction_hook_compat_import: false,
                transcript_parser: true,
                transcript_compaction_signals: true,
                catalog: true,
                session_root: SessionRoot::ToolHome,
                account_selector: Some("CODEX_HOME"),
                account_selection: true,
                team_config_namespace: false,
                usage: true,
                usage_note: None,
                notify_sink: true,
                hook_trust: true,
                managed_home: true,
            },
            stop_strategy: StopStrategy::Interrupt,
            stop_presence_dir: None,
            stop_registry_release: false,
            onboarding_exit_hint: false,
            onboarding_delivery_hint: None,
            pane_binding: true,
            display_name: "Codex CLI",
            settings_label: "Codex",
            base_dir_name: ".codex",
            projects_subdir: "sessions",
            session_extension: "jsonl",
            exit_command: "/exit",
        },
        CliToolSpec {
            tool: CliTool::Agy,
            name: "agy",
            aliases: &["agy", "antigravity"],
            argv_signatures: &["agy"],
            non_session_flags: &["-p", "--print", "--prompt", "--input-format"],
            non_session_subcommands: &[
                "agent",
                "agents",
                "changelog",
                "help",
                "install",
                "mcp",
                "mic-serve",
                "models",
                "plugin",
                "plugins",
                "update",
            ],
            argv_value_flags: &[
                "--add-dir",
                "--agent",
                "--conversation",
                "--effort",
                "-i",
                "--prompt-interactive",
                "--json-schema",
                "--log-file",
                "--mode",
                "--model",
                "--output-format",
                "--print-timeout",
                "--project",
            ],
            model_prefixes: &[],
            model_markers: &[],
            default_commands: ToolCommands {
                continue_cmd: "agy --dangerously-skip-permissions --continue".into(),
                fresh: "agy --dangerously-skip-permissions".into(),
                resume: "agy --dangerously-skip-permissions --conversation {session_id}".into(),
            },
            label: "Antigravity",
            accent: "google-blue",
            medallion_accent: "google-blue",
            default_agent_role_id: "antigravity-ui-specialist",
            capabilities: CliCapabilities {
                model_flag: Some("--model"),
                effort_flag: Some(EffortFlag::Argument { flag: "--effort" }),
                auto_approve_flag: Some("--dangerously-skip-permissions"),
                display_name_flag: None,
                team_flags: false,
                native_inbox_poller: false,
                session_source: true,
                runtime_session_capture: false,
                authoritative_idle: false,
                compaction_hook: false,
                compaction_hook_compat_import: false,
                transcript_parser: false,
                transcript_compaction_signals: false,
                catalog: true,
                session_root: SessionRoot::ToolHome,
                account_selector: None,
                account_selection: false,
                team_config_namespace: false,
                usage: true,
                usage_note: None,
                notify_sink: false,
                hook_trust: false,
                managed_home: false,
            },
            stop_strategy: StopStrategy::SlashExit,
            stop_presence_dir: Some("presence"),
            stop_registry_release: false,
            onboarding_exit_hint: true,
            onboarding_delivery_hint: None,
            pane_binding: false,
            display_name: "Antigravity CLI",
            settings_label: "Antigravity CLI",
            base_dir_name: ".gemini",
            projects_subdir: "antigravity-cli/conversations",
            session_extension: "db",
            exit_command: "/exit",
        },
        CliToolSpec {
            tool: CliTool::Grok,
            name: "grok",
            aliases: &["grok"],
            argv_signatures: &["grok"],
            // A single-prompt source is the only thing that makes grok headless;
            // a bare positional PROMPT is still the first turn of the TUI.
            non_session_flags: &["-p", "--single", "--prompt-file", "--prompt-json"],
            non_session_subcommands: &[
                "agent",
                "completions",
                "dashboard",
                "doctor",
                "du",
                "export",
                "inspect",
                "leader",
                "login",
                "logout",
                "mcp",
                "memory",
                "models",
                "plugin",
                "sessions",
                "setup",
                "trace",
                "update",
                "version",
                "worktree",
                "wrap",
            ],
            argv_value_flags: &[
                "--agent",
                "--agent-profile",
                "--agents",
                "--cwd",
                "--debug-file",
                "--effort",
                "--json-schema",
                "--leader-socket",
                "--log-file",
                "-m",
                "--max-turns",
                "--model",
                "--output-format",
                "--permission-mode",
                "-r",
                "--ref",
                "--reasoning-effort",
                "--resume",
                "-s",
                "--session-id",
                "--tools",
                "-w",
                "--worktree",
                "--worktree-ref",
            ],
            model_prefixes: &["grok-"],
            model_markers: &["grok"],
            default_commands: ToolCommands {
                continue_cmd: "grok --always-approve --continue".into(),
                fresh: "grok --always-approve".into(),
                resume: "grok --always-approve --resume {session_id}".into(),
            },
            label: "Grok",
            accent: "grok",
            medallion_accent: "grok",
            default_agent_role_id: "grok-developer",
            capabilities: CliCapabilities {
                model_flag: Some("--model"),
                effort_flag: Some(EffortFlag::Argument { flag: "--effort" }),
                auto_approve_flag: Some("--always-approve"),
                display_name_flag: None,
                team_flags: false,
                native_inbox_poller: false,
                session_source: false,
                runtime_session_capture: false,
                authoritative_idle: false,
                compaction_hook: false,
                // grok reads `~/.claude/settings.json` hooks by default, so one
                // compaction can reach the bridge through two registrations.
                compaction_hook_compat_import: true,
                transcript_parser: false,
                transcript_compaction_signals: false,
                catalog: true,
                session_root: SessionRoot::ToolHome,
                account_selector: None,
                account_selection: false,
                team_config_namespace: false,
                usage: false,
                usage_note: Some("Grok shows credits in its own /usage"),
                notify_sink: false,
                hook_trust: false,
                managed_home: false,
            },
            stop_strategy: StopStrategy::SlashExit,
            stop_presence_dir: None,
            stop_registry_release: false,
            onboarding_exit_hint: true,
            onboarding_delivery_hint: Some(
                "Plain Enter queues a message until the running turn ends; Ctrl+Enter interjects immediately.",
            ),
            pane_binding: false,
            display_name: "Grok CLI",
            settings_label: "Grok CLI",
            base_dir_name: ".grok",
            projects_subdir: "sessions",
            session_extension: "jsonl",
            exit_command: "/quit",
        },
    ]
});

static UNKNOWN_TOOL_SPEC: LazyLock<CliToolSpec> = LazyLock::new(|| CliToolSpec {
    tool: CliTool::Unknown,
    name: "unknown",
    aliases: &[],
    argv_signatures: &[],
    non_session_flags: &[],
    non_session_subcommands: &[],
    argv_value_flags: &[],
    model_prefixes: &[],
    model_markers: &[],
    default_commands: ToolCommands {
        continue_cmd: String::new(),
        fresh: String::new(),
        resume: String::new(),
    },
    label: "Unknown tool",
    accent: "zinc",
    medallion_accent: "zinc",
    default_agent_role_id: "",
    capabilities: CliCapabilities {
        model_flag: None,
        effort_flag: None,
        auto_approve_flag: None,
        display_name_flag: None,
        team_flags: false,
        native_inbox_poller: false,
        session_source: false,
        runtime_session_capture: false,
        authoritative_idle: false,
        compaction_hook: false,
        compaction_hook_compat_import: false,
        transcript_parser: false,
        transcript_compaction_signals: false,
        catalog: false,
        session_root: SessionRoot::ToolHome,
        account_selector: None,
        account_selection: false,
        team_config_namespace: false,
        usage: false,
        usage_note: None,
        notify_sink: false,
        hook_trust: false,
        managed_home: false,
    },
    stop_strategy: StopStrategy::Interrupt,
    stop_presence_dir: None,
    stop_registry_release: false,
    onboarding_exit_hint: false,
    onboarding_delivery_hint: None,
    pane_binding: false,
    display_name: "Unknown tool",
    settings_label: "Unknown tool",
    base_dir_name: "",
    projects_subdir: "",
    session_extension: "",
    exit_command: "/exit",
});

/// Get every registered CLI harness.
pub fn all() -> &'static [CliToolSpec] {
    TOOL_SPECS.as_slice()
}

/// Get all registered tool configurations.
pub fn all_tools() -> &'static [CliToolSpec] {
    all()
}

/// Get the configuration for a specific tool.
pub fn spec(tool: CliTool) -> &'static CliToolSpec {
    if tool == CliTool::Unknown {
        return &UNKNOWN_TOOL_SPEC;
    }
    all()
        .iter()
        .find(|c| c.tool == tool)
        .expect("every CliTool variant has a config entry")
}

/// Backwards-compatible name for callers that consume filesystem layout.
pub fn config_for(tool: CliTool) -> &'static CliToolSpec {
    spec(tool)
}

pub fn command_settings_for(settings: &CliCommandSettings, tool: CliTool) -> &ToolCommands {
    static EMPTY: LazyLock<ToolCommands> = LazyLock::new(|| ToolCommands {
        continue_cmd: String::new(),
        fresh: String::new(),
        resume: String::new(),
    });
    match tool {
        CliTool::Claude => &settings.claude,
        CliTool::Codex => &settings.codex,
        CliTool::Agy => &settings.agy,
        CliTool::Grok => &settings.grok,
        CliTool::Unknown => &EMPTY,
    }
}

pub fn bridged_default() -> CliTool {
    all()
        .iter()
        .find(|entry| entry.pane_binding)
        .map(|entry| entry.tool)
        .unwrap_or_default()
}

pub fn model_is_compatible(tool: CliTool, model: &str) -> bool {
    spec(tool)
        .model_prefixes
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

pub fn infer_from_model(model: &str) -> CliTool {
    let normalized = model.to_ascii_lowercase();
    all()
        .iter()
        .find(|entry| {
            entry
                .model_markers
                .iter()
                .any(|marker| normalized.contains(marker))
        })
        .map(|entry| entry.tool)
        .unwrap_or_else(bridged_default)
}

pub fn native_inbox_tool() -> Option<CliTool> {
    all()
        .iter()
        .find(|entry| entry.capabilities.native_inbox_poller)
        .map(|entry| entry.tool)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EffortFlagDescriptor {
    Argument { flag: String },
    Config { flag: String, key: String },
}

impl From<EffortFlag> for EffortFlagDescriptor {
    fn from(value: EffortFlag) -> Self {
        match value {
            EffortFlag::Argument { flag } => Self::Argument { flag: flag.into() },
            EffortFlag::Config { flag, key } => Self::Config {
                flag: flag.into(),
                key: key.into(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliCapabilityDescriptor {
    pub model_flag: Option<String>,
    pub effort_flag: Option<EffortFlagDescriptor>,
    pub auto_approve_flag: Option<String>,
    pub display_name_flag: Option<String>,
    pub team_flags: bool,
    pub native_inbox_poller: bool,
    pub session_source: bool,
    pub runtime_session_capture: bool,
    pub authoritative_idle: bool,
    pub compaction_hook: bool,
    pub compaction_hook_compat_import: bool,
    pub transcript_parser: bool,
    pub transcript_compaction_signals: bool,
    pub catalog: bool,
    pub session_root: SessionRoot,
    pub account_selector: Option<String>,
    pub account_selection: bool,
    pub team_config_namespace: bool,
    pub usage: bool,
    pub usage_note: Option<String>,
    pub notify_sink: bool,
    pub hook_trust: bool,
    pub managed_home: bool,
}

impl From<CliCapabilities> for CliCapabilityDescriptor {
    fn from(value: CliCapabilities) -> Self {
        Self {
            model_flag: value.model_flag.map(str::to_string),
            effort_flag: value.effort_flag.map(Into::into),
            auto_approve_flag: value.auto_approve_flag.map(str::to_string),
            display_name_flag: value.display_name_flag.map(str::to_string),
            team_flags: value.team_flags,
            native_inbox_poller: value.native_inbox_poller,
            session_source: value.session_source,
            runtime_session_capture: value.runtime_session_capture,
            authoritative_idle: value.authoritative_idle,
            compaction_hook: value.compaction_hook,
            compaction_hook_compat_import: value.compaction_hook_compat_import,
            transcript_parser: value.transcript_parser,
            transcript_compaction_signals: value.transcript_compaction_signals,
            catalog: value.catalog,
            session_root: value.session_root,
            account_selector: value.account_selector.map(str::to_string),
            account_selection: value.account_selection,
            team_config_namespace: value.team_config_namespace,
            usage: value.usage,
            usage_note: value.usage_note.map(str::to_string),
            notify_sink: value.notify_sink,
            hook_trust: value.hook_trust,
            managed_home: value.managed_home,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliToolDescriptor {
    pub id: CliTool,
    pub label: String,
    pub display_name: String,
    pub accent: String,
    pub medallion_accent: String,
    pub default_agent_role_id: String,
    pub aliases: Vec<String>,
    pub capabilities: CliCapabilityDescriptor,
}

impl From<&CliToolSpec> for CliToolDescriptor {
    fn from(value: &CliToolSpec) -> Self {
        Self {
            id: value.tool,
            label: value.label.to_string(),
            display_name: value.settings_label.to_string(),
            accent: value.accent.to_string(),
            medallion_accent: value.medallion_accent.to_string(),
            default_agent_role_id: value.default_agent_role_id.to_string(),
            aliases: value
                .aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect(),
            capabilities: value.capabilities.into(),
        }
    }
}

impl CliToolSpec {
    /// Account provider for this tool. Provider rollout follows selector
    /// declaration, so a declared selector may temporarily use the floor.
    pub fn account_provider(
        &self,
    ) -> Option<&'static dyn crate::session_scanner::accounts::AccountProvider> {
        static CLAUDE: crate::session_scanner::accounts::claude::ClaudeAccountProvider =
            crate::session_scanner::accounts::claude::ClaudeAccountProvider;
        static CODEX: crate::session_scanner::accounts::codex::CodexAccountProvider =
            crate::session_scanner::accounts::codex::CodexAccountProvider;
        static AGY: crate::session_scanner::accounts::agy::AgyAccountProvider =
            crate::session_scanner::accounts::agy::AgyAccountProvider;

        match self.tool {
            CliTool::Claude => Some(&CLAUDE),
            CliTool::Codex => Some(&CODEX),
            CliTool::Agy => Some(&AGY),
            CliTool::Grok => None,
            CliTool::Unknown => None,
        }
    }

    /// Usage provider for this tool. Implementations are added per provider.
    pub fn usage_provider(
        &self,
    ) -> Option<&'static dyn crate::session_scanner::accounts::UsageProvider> {
        static CLAUDE: crate::session_scanner::accounts::claude::ClaudeUsageProvider =
            crate::session_scanner::accounts::claude::ClaudeUsageProvider;
        static CODEX: crate::session_scanner::accounts::codex::CodexUsageProvider =
            crate::session_scanner::accounts::codex::CodexUsageProvider;
        static AGY: crate::session_scanner::accounts::agy::AgyUsageProvider =
            crate::session_scanner::accounts::agy::AgyUsageProvider;

        match self.tool {
            CliTool::Claude => Some(&CLAUDE),
            CliTool::Codex => Some(&CODEX),
            CliTool::Agy => Some(&AGY),
            CliTool::Grok => None,
            CliTool::Unknown => None,
        }
    }

    pub fn matches_argv_token(&self, token: &str) -> bool {
        self.argv_signatures.iter().any(|signature| {
            if signature.starts_with('@') {
                token.contains(signature)
            } else {
                token == *signature
                    || token
                        .rsplit_once('/')
                        .is_some_and(|(_, file_name)| file_name == *signature)
            }
        })
    }

    /// Whether a matching executable invocation represents an interactive
    /// terminal session rather than a one-shot driver or utility subcommand.
    pub fn argv_is_session(&self, args: &str) -> bool {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.iter().any(|token| {
            self.non_session_flags.iter().any(|flag| {
                token == flag
                    || token
                        .strip_prefix(flag)
                        .is_some_and(|rest| rest.starts_with('='))
            })
        }) {
            return false;
        }

        let Some(executable_index) = tokens
            .iter()
            .position(|token| self.matches_argv_token(token))
        else {
            return false;
        };
        let mut index = executable_index + 1;
        while let Some(token) = tokens.get(index) {
            if token.starts_with('-') {
                let takes_separate_value =
                    !token.contains('=') && self.argv_value_flags.iter().any(|flag| token == flag);
                index += if takes_separate_value { 2 } else { 1 };
                continue;
            }
            return !self.non_session_subcommands.contains(token);
        }
        true
    }

    pub fn session_source(&self) -> &'static dyn crate::session_scanner::idle::SessionSource {
        static CLAUDE: crate::session_scanner::idle::ClaudeRegistrySessionSource =
            crate::session_scanner::idle::ClaudeRegistrySessionSource;
        static CODEX: crate::session_scanner::idle::CodexSessionSource =
            crate::session_scanner::idle::CodexSessionSource;
        static AGY: std::sync::OnceLock<crate::session_scanner::idle::AgyResolver> =
            std::sync::OnceLock::new();
        static NONE: crate::session_scanner::idle::NoSessionSource =
            crate::session_scanner::idle::NoSessionSource;

        if !self.capabilities.session_source {
            return &NONE;
        }

        match self.tool {
            CliTool::Claude => &CLAUDE,
            CliTool::Codex => &CODEX,
            CliTool::Agy => AGY.get_or_init(crate::session_scanner::idle::AgyResolver::new),
            CliTool::Grok => &NONE,
            CliTool::Unknown => &NONE,
        }
    }

    pub fn activity_source(&self) -> &'static dyn crate::session_scanner::idle::ActivitySource {
        static CLAUDE: crate::session_scanner::idle::ClaudeRegistryActivitySource =
            crate::session_scanner::idle::ClaudeRegistryActivitySource;
        static CODEX: crate::session_scanner::idle::CodexNotifyActivitySource =
            crate::session_scanner::idle::CodexNotifyActivitySource;
        static AGY: crate::session_scanner::idle::AgyHooksActivitySource =
            crate::session_scanner::idle::AgyHooksActivitySource;
        static NONE: crate::session_scanner::idle::NoActivitySource =
            crate::session_scanner::idle::NoActivitySource;

        match self.tool {
            CliTool::Claude => &CLAUDE,
            CliTool::Codex => &CODEX,
            CliTool::Agy => &AGY,
            CliTool::Grok => &NONE,
            CliTool::Unknown => &NONE,
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    pub fn compaction_signal_source(
        &self,
    ) -> Option<&'static dyn crate::coordination::compact_hook::CompactionSignalSource> {
        static CLAUDE: crate::coordination::compact_hook::ClaudeCompactionSignalSource =
            crate::coordination::compact_hook::ClaudeCompactionSignalSource;
        static CODEX: crate::coordination::compact_hook::CodexCompactionSignalSource =
            crate::coordination::compact_hook::CodexCompactionSignalSource;

        if !self.capabilities.compaction_hook {
            if self.tool == CliTool::Agy {
                static LOGGED: std::sync::Once = std::sync::Once::new();
                LOGGED.call_once(|| {
                    let mut fields = serde_json::Map::new();
                    fields.insert("tool".to_string(), serde_json::Value::String("agy".into()));
                    taurhaus_lib::logging::emit_global(
                        "info",
                        "coordination",
                        "compaction.unsupported",
                        Some("Harness does not expose a compaction signal".to_string()),
                        fields,
                    );
                });
            }
            return None;
        }

        match self.tool {
            CliTool::Claude => Some(&CLAUDE),
            CliTool::Codex => Some(&CODEX),
            CliTool::Agy => None,
            CliTool::Grok => None,
            CliTool::Unknown => None,
        }
    }

    pub fn transcript_parser(&self) -> Option<&'static dyn crate::task_scanner::TranscriptParser> {
        static CLAUDE: crate::task_scanner::claude::ClaudeTranscriptParser =
            crate::task_scanner::claude::ClaudeTranscriptParser;
        static CODEX: crate::task_scanner::codex::CodexTranscriptParser =
            crate::task_scanner::codex::CodexTranscriptParser;

        match self.tool {
            CliTool::Claude => Some(&CLAUDE),
            CliTool::Codex => Some(&CODEX),
            CliTool::Agy => None,
            CliTool::Grok => None,
            CliTool::Unknown => None,
        }
    }

    pub(crate) fn session_resolver(
        &self,
    ) -> &'static dyn crate::session_scanner::idle::SessionResolver {
        use std::sync::OnceLock;

        static CLAUDE: OnceLock<crate::session_scanner::idle::ClaudeResolver> = OnceLock::new();
        static CODEX: OnceLock<crate::session_scanner::idle::CodexResolver> = OnceLock::new();
        static AGY: OnceLock<crate::session_scanner::idle::AgyResolver> = OnceLock::new();
        static NONE: crate::session_scanner::idle::NoSessionSource =
            crate::session_scanner::idle::NoSessionSource;

        match self.tool {
            CliTool::Claude => {
                CLAUDE.get_or_init(crate::session_scanner::idle::ClaudeResolver::new)
            }
            CliTool::Codex => CODEX.get_or_init(crate::session_scanner::idle::CodexResolver::new),
            CliTool::Agy => AGY.get_or_init(crate::session_scanner::idle::AgyResolver::new),
            CliTool::Grok => &NONE,
            CliTool::Unknown => &NONE,
        }
    }
}

pub fn descriptors() -> Vec<CliToolDescriptor> {
    all().iter().map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_replaces_gemini_with_antigravity_capabilities() {
        // Regression: commit 9a66d1c made Antigravity CLI the third fixed harness;
        // Google now refuses that client for individuals, so the registry must
        // expose agy without accepting the incompatible persisted tool value.
        let agy = all()
            .iter()
            .find(|entry| entry.name == "agy")
            .expect("Antigravity registry entry");
        assert_eq!(agy.aliases, ["agy", "antigravity"]);
        assert_eq!(
            agy.default_commands.fresh,
            "agy --dangerously-skip-permissions"
        );
        assert_eq!(
            agy.default_commands.continue_cmd,
            "agy --dangerously-skip-permissions --continue"
        );
        assert_eq!(
            agy.default_commands.resume,
            "agy --dangerously-skip-permissions --conversation {session_id}"
        );
        assert_eq!("agy".parse::<CliTool>(), Ok(CliTool::Agy));
        assert_eq!(
            serde_json::from_str::<CliTool>("\"gemini\"").unwrap(),
            CliTool::Unknown
        );

        let descriptor = serde_json::to_value(CliToolDescriptor::from(agy)).unwrap();
        assert_eq!(
            descriptor["capabilities"]["autoApproveFlag"],
            "--dangerously-skip-permissions"
        );
        assert_eq!(descriptor["capabilities"]["managedHome"], false);
    }

    #[test]
    fn agy_default_commands_keep_unattended_permission_bypass() {
        // Regression: commit 0e35895 stopped force-injecting agy's permission
        // bypass but left every registry default bare, so unattended team
        // members stalled or silently soft-denied their first tool call.
        let commands = &spec(CliTool::Agy).default_commands;
        assert_eq!(commands.fresh, "agy --dangerously-skip-permissions");
        assert_eq!(
            commands.continue_cmd,
            "agy --dangerously-skip-permissions --continue"
        );
        assert_eq!(
            commands.resume,
            "agy --dangerously-skip-permissions --conversation {session_id}"
        );
    }

    #[test]
    fn managed_home_is_an_explicit_registry_capability() {
        // Regression: commit 2c49132 selected Codex's managed CODEX_HOME by
        // coupling two unrelated native capabilities (hook trust + notify).
        let managed = descriptors()
            .into_iter()
            .filter(|entry| {
                serde_json::to_value(entry).unwrap()["capabilities"]["managedHome"] == true
            })
            .map(|entry| entry.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(managed, ["codex"]);
    }

    #[test]
    fn grok_registry_entry_declares_its_verified_launch_surface() {
        // Regression: commit bfecae9 fixed the harness set at three CLIs, so a
        // fourth could only be added by branching outside the capability
        // slices. Grok's verified argv, flags and defaults are registry data.
        let grok = all()
            .iter()
            .find(|entry| entry.name == "grok")
            .expect("Grok registry entry");
        assert_eq!(grok.aliases, ["grok"]);
        assert_eq!(grok.default_commands.fresh, "grok --always-approve");
        assert_eq!(
            grok.default_commands.continue_cmd,
            "grok --always-approve --continue"
        );
        assert_eq!(
            grok.default_commands.resume,
            "grok --always-approve --resume {session_id}"
        );
        assert_eq!("grok".parse::<CliTool>(), Ok(CliTool::Grok));
        assert_eq!(CliTool::from_alias("grok"), Ok(CliTool::Grok));
        assert_eq!(grok.exit_command, "/quit");
        assert_eq!(grok.base_dir_name, ".grok");
        assert_eq!(grok.projects_subdir, "sessions");

        let descriptor = serde_json::to_value(CliToolDescriptor::from(grok)).unwrap();
        assert_eq!(descriptor["id"], "grok");
        assert_eq!(
            descriptor["capabilities"]["autoApproveFlag"],
            "--always-approve"
        );
        assert_eq!(descriptor["capabilities"]["modelFlag"], "--model");
        assert_eq!(
            descriptor["capabilities"]["effortFlag"],
            serde_json::json!({ "kind": "argument", "flag": "--effort" })
        );
        assert_eq!(descriptor["capabilities"]["usage"], false);
        assert_eq!(
            descriptor["capabilities"]["usageNote"],
            "Grok shows credits in its own /usage"
        );
        assert_eq!(
            descriptor["capabilities"]["compactionHookCompatImport"],
            true
        );
    }

    #[test]
    fn grok_argv_classification_separates_the_tui_from_headless_and_services() {
        // Regression: commit bfecae9 left argv classification with no entry for
        // grok, whose headless drivers, agent services and management commands
        // would otherwise all register as interactive sessions.
        let grok = spec(CliTool::Grok);
        for interactive in [
            "grok",
            "/home/user/.local/bin/grok --model grok-4.6 --effort low",
            "grok --continue",
            "grok --always-approve",
            "grok --resume 01a04585-2d53-7123-8000-000000000000",
            "grok 'reply with the single word OK'",
            "grok --resume models",
            "grok --model grok-4.6 'explain this repo'",
        ] {
            assert!(grok.argv_is_session(interactive), "{interactive}");
        }

        for non_session in [
            "grok -p 'summarise'",
            "grok --single 'summarise'",
            "grok --prompt-file prompt.txt",
            "grok --prompt-json '[]'",
            "grok agent stdio",
            "grok agent leader",
            "grok models",
            "grok sessions list",
            "grok update",
            "grok --model grok-4.6 inspect",
        ] {
            assert!(!grok.argv_is_session(non_session), "{non_session}");
        }
    }

    #[test]
    fn cli_tool_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&CliTool::Claude).unwrap(),
            "\"claude\""
        );
        assert_eq!(serde_json::to_string(&CliTool::Codex).unwrap(), "\"codex\"");
        assert_eq!(serde_json::to_string(&CliTool::Agy).unwrap(), "\"agy\"");
        assert_eq!(serde_json::to_string(&CliTool::Grok).unwrap(), "\"grok\"");
    }

    #[test]
    fn cli_tool_deserializes() {
        let c: CliTool = serde_json::from_str("\"claude\"").unwrap();
        assert_eq!(c, CliTool::Claude);
        let x: CliTool = serde_json::from_str("\"codex\"").unwrap();
        assert_eq!(x, CliTool::Codex);
        let a: CliTool = serde_json::from_str("\"agy\"").unwrap();
        assert_eq!(a, CliTool::Agy);
        let g: CliTool = serde_json::from_str("\"grok\"").unwrap();
        assert_eq!(g, CliTool::Grok);
        assert_eq!(
            serde_json::from_str::<CliTool>("\"gemini\"").unwrap(),
            CliTool::Unknown
        );
    }

    #[test]
    fn config_for_returns_correct_config() {
        let claude = config_for(CliTool::Claude);
        assert_eq!(claude.base_dir_name, ".claude");
        assert_eq!(claude.display_name, "Claude Code");

        let codex = config_for(CliTool::Codex);
        assert_eq!(codex.base_dir_name, ".codex");

        let agy = config_for(CliTool::Agy);
        assert_eq!(agy.base_dir_name, ".gemini");

        let grok = config_for(CliTool::Grok);
        assert_eq!(grok.base_dir_name, ".grok");
    }

    #[test]
    fn all_tools_covers_every_variant() {
        let tools = all_tools();
        assert_eq!(tools.len(), 4);
        assert!(tools.iter().any(|c| c.tool == CliTool::Claude));
        assert!(tools.iter().any(|c| c.tool == CliTool::Codex));
        assert!(tools.iter().any(|c| c.tool == CliTool::Agy));
        assert!(tools.iter().any(|c| c.tool == CliTool::Grok));
    }

    #[test]
    fn cli_tool_from_str_is_case_insensitive() {
        assert_eq!("Claude".parse::<CliTool>().unwrap(), CliTool::Claude);
        assert_eq!("CODEX".parse::<CliTool>().unwrap(), CliTool::Codex);
        assert_eq!("AGY".parse::<CliTool>().unwrap(), CliTool::Agy);
        assert_eq!("Grok".parse::<CliTool>().unwrap(), CliTool::Grok);
    }

    #[test]
    fn cli_tool_from_alias_maps_coordination_values() {
        assert_eq!(
            CliTool::from_alias("claude_native").unwrap(),
            CliTool::Claude
        );
        assert_eq!(CliTool::from_alias("mesh").unwrap(), CliTool::Codex);
        assert_eq!(CliTool::from_alias("mesh_bridged").unwrap(), CliTool::Codex);
        assert_eq!(CliTool::from_alias("antigravity").unwrap(), CliTool::Agy);
        assert_eq!(CliTool::from_alias("agy").unwrap(), CliTool::Agy);
    }

    #[test]
    fn cli_tool_from_alias_rejects_unknown_values() {
        let err = CliTool::from_alias("unknown").unwrap_err();
        assert_eq!(err.to_string(), "unsupported cli tool 'unknown'");
    }
}
