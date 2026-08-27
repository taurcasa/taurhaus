//! CLI tool abstraction — detect and configure multiple AI coding tools.
//!
//! Supports Claude Code, Codex CLI, and Gemini CLI. Each tool has its
//! own process signature, session directory layout, and launch commands.

use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::models::{CliCommandSettings, ToolCommands};

/// Which CLI tool a session belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliTool {
    Claude,
    Codex,
    Gemini,
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

/// Capability declarations consumed by tool-agnostic call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliCapabilities {
    pub model_flag: Option<&'static str>,
    pub effort_flag: Option<EffortFlag>,
    pub display_name_flag: Option<&'static str>,
    pub team_flags: bool,
    pub native_inbox_poller: bool,
    pub authoritative_idle: bool,
    pub compaction_hook: bool,
    pub transcript_parser: bool,
    pub catalog: bool,
    pub config_dir_env: Option<&'static str>,
    pub usage_bridge: bool,
    pub notify_sink: bool,
    pub hook_trust: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopStrategy {
    SlashExit,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessActivitySignal {
    ReadChars,
    Tcp,
}

/// One registry record for a supported CLI harness.
pub struct CliToolSpec {
    pub tool: CliTool,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub argv_signatures: &'static [&'static str],
    pub default_commands: ToolCommands,
    pub label: &'static str,
    pub accent: &'static str,
    pub capabilities: CliCapabilities,
    pub stop_strategy: StopStrategy,
    pub process_activity_signal: ProcessActivitySignal,
    pub pane_binding: bool,
    pub display_name: &'static str,
    /// Base directory name under `$HOME` (e.g., ".claude", ".codex", ".gemini").
    pub base_dir_name: &'static str,
    /// Subdirectory within the base dir that contains project session data.
    pub projects_subdir: &'static str,
    /// File extension for session transcripts.
    pub session_extension: &'static str,
    /// Command to gracefully exit the CLI in a terminal.
    pub exit_command: &'static str,
}

static TOOL_SPECS: LazyLock<[CliToolSpec; 3]> = LazyLock::new(|| {
    [
        CliToolSpec {
            tool: CliTool::Claude,
            name: "claude",
            aliases: &["claude", "claude_native"],
            argv_signatures: &["claude", "@anthropic-ai/claude-code"],
            default_commands: ToolCommands {
                continue_cmd: "claude --dangerously-skip-permissions --continue".into(),
                fresh: "claude --dangerously-skip-permissions".into(),
                resume: "claude --dangerously-skip-permissions --resume".into(),
            },
            label: "Claude",
            accent: "emerald",
            capabilities: CliCapabilities {
                model_flag: Some("--model"),
                effort_flag: Some(EffortFlag::Argument { flag: "--effort" }),
                display_name_flag: Some("-n"),
                team_flags: true,
                native_inbox_poller: true,
                authoritative_idle: true,
                compaction_hook: true,
                transcript_parser: true,
                catalog: true,
                config_dir_env: Some("CLAUDE_CONFIG_DIR"),
                usage_bridge: true,
                notify_sink: false,
                hook_trust: false,
            },
            stop_strategy: StopStrategy::SlashExit,
            process_activity_signal: ProcessActivitySignal::ReadChars,
            pane_binding: false,
            display_name: "Claude Code",
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
            default_commands: ToolCommands {
                continue_cmd: "codex --yolo".into(),
                fresh: "codex --yolo".into(),
                resume: "codex resume --last --yolo".into(),
            },
            label: "Codex",
            accent: "sky",
            capabilities: CliCapabilities {
                model_flag: Some("-m"),
                effort_flag: Some(EffortFlag::Config {
                    flag: "-c",
                    key: "model_reasoning_effort",
                }),
                display_name_flag: None,
                team_flags: false,
                native_inbox_poller: false,
                authoritative_idle: true,
                compaction_hook: true,
                transcript_parser: true,
                catalog: true,
                config_dir_env: None,
                usage_bridge: false,
                notify_sink: true,
                hook_trust: true,
            },
            stop_strategy: StopStrategy::SlashExit,
            process_activity_signal: ProcessActivitySignal::ReadChars,
            pane_binding: true,
            display_name: "Codex CLI",
            base_dir_name: ".codex",
            projects_subdir: "sessions",
            session_extension: "jsonl",
            exit_command: "/exit",
        },
        CliToolSpec {
            tool: CliTool::Gemini,
            name: "gemini",
            aliases: &["gemini"],
            argv_signatures: &["gemini", "@google/gemini-cli"],
            default_commands: ToolCommands {
                continue_cmd: "gemini --yolo --resume".into(),
                fresh: "gemini --yolo".into(),
                resume: "gemini --yolo --resume".into(),
            },
            label: "Gemini",
            accent: "violet",
            capabilities: CliCapabilities {
                model_flag: Some("-m"),
                effort_flag: None,
                display_name_flag: None,
                team_flags: false,
                native_inbox_poller: false,
                authoritative_idle: false,
                compaction_hook: false,
                transcript_parser: false,
                catalog: true,
                config_dir_env: None,
                usage_bridge: false,
                notify_sink: false,
                hook_trust: false,
            },
            stop_strategy: StopStrategy::SlashExit,
            process_activity_signal: ProcessActivitySignal::Tcp,
            pane_binding: false,
            display_name: "Gemini CLI",
            base_dir_name: ".gemini",
            projects_subdir: "tmp",
            session_extension: "jsonl",
            exit_command: "/exit",
        },
    ]
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
    match tool {
        CliTool::Claude => &settings.claude,
        CliTool::Codex => &settings.codex,
        CliTool::Gemini => &settings.gemini,
    }
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
    pub display_name_flag: Option<String>,
    pub team_flags: bool,
    pub native_inbox_poller: bool,
    pub authoritative_idle: bool,
    pub compaction_hook: bool,
    pub transcript_parser: bool,
    pub catalog: bool,
    pub config_dir_env: Option<String>,
    pub usage_bridge: bool,
    pub notify_sink: bool,
    pub hook_trust: bool,
}

impl From<CliCapabilities> for CliCapabilityDescriptor {
    fn from(value: CliCapabilities) -> Self {
        Self {
            model_flag: value.model_flag.map(str::to_string),
            effort_flag: value.effort_flag.map(Into::into),
            display_name_flag: value.display_name_flag.map(str::to_string),
            team_flags: value.team_flags,
            native_inbox_poller: value.native_inbox_poller,
            authoritative_idle: value.authoritative_idle,
            compaction_hook: value.compaction_hook,
            transcript_parser: value.transcript_parser,
            catalog: value.catalog,
            config_dir_env: value.config_dir_env.map(str::to_string),
            usage_bridge: value.usage_bridge,
            notify_sink: value.notify_sink,
            hook_trust: value.hook_trust,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliToolDescriptor {
    pub id: CliTool,
    pub label: String,
    pub accent: String,
    pub aliases: Vec<String>,
    pub capabilities: CliCapabilityDescriptor,
}

impl From<&CliToolSpec> for CliToolDescriptor {
    fn from(value: &CliToolSpec) -> Self {
        Self {
            id: value.tool,
            label: value.label.to_string(),
            accent: value.accent.to_string(),
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
    pub fn session_source(&self) -> &'static dyn crate::session_scanner::idle::SessionSource {
        static CLAUDE: crate::session_scanner::idle::ClaudeRegistrySessionSource =
            crate::session_scanner::idle::ClaudeRegistrySessionSource;
        static CODEX: crate::session_scanner::idle::CodexSessionSource =
            crate::session_scanner::idle::CodexSessionSource;
        static NONE: crate::session_scanner::idle::NoSessionSource =
            crate::session_scanner::idle::NoSessionSource;

        match self.tool {
            CliTool::Claude => &CLAUDE,
            CliTool::Codex => &CODEX,
            CliTool::Gemini => &NONE,
        }
    }

    pub fn activity_source(&self) -> &'static dyn crate::session_scanner::idle::ActivitySource {
        static CLAUDE: crate::session_scanner::idle::ClaudeRegistryActivitySource =
            crate::session_scanner::idle::ClaudeRegistryActivitySource;
        static CODEX: crate::session_scanner::idle::CodexNotifyActivitySource =
            crate::session_scanner::idle::CodexNotifyActivitySource;
        static NONE: crate::session_scanner::idle::NoActivitySource =
            crate::session_scanner::idle::NoActivitySource;

        match self.tool {
            CliTool::Claude => &CLAUDE,
            CliTool::Codex => &CODEX,
            CliTool::Gemini => &NONE,
        }
    }

    pub(crate) fn session_resolver(
        &self,
    ) -> &'static dyn crate::session_scanner::idle::SessionResolver {
        use std::sync::OnceLock;

        static CLAUDE: OnceLock<crate::session_scanner::idle::ClaudeResolver> = OnceLock::new();
        static CODEX: OnceLock<crate::session_scanner::idle::CodexResolver> = OnceLock::new();
        static GEMINI: OnceLock<crate::session_scanner::idle::GeminiResolver> = OnceLock::new();

        match self.tool {
            CliTool::Claude => {
                CLAUDE.get_or_init(crate::session_scanner::idle::ClaudeResolver::new)
            }
            CliTool::Codex => CODEX.get_or_init(crate::session_scanner::idle::CodexResolver::new),
            CliTool::Gemini => {
                GEMINI.get_or_init(crate::session_scanner::idle::GeminiResolver::new)
            }
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
    fn cli_tool_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&CliTool::Claude).unwrap(),
            "\"claude\""
        );
        assert_eq!(serde_json::to_string(&CliTool::Codex).unwrap(), "\"codex\"");
        assert_eq!(
            serde_json::to_string(&CliTool::Gemini).unwrap(),
            "\"gemini\""
        );
    }

    #[test]
    fn cli_tool_deserializes() {
        let c: CliTool = serde_json::from_str("\"claude\"").unwrap();
        assert_eq!(c, CliTool::Claude);
        let x: CliTool = serde_json::from_str("\"codex\"").unwrap();
        assert_eq!(x, CliTool::Codex);
        let g: CliTool = serde_json::from_str("\"gemini\"").unwrap();
        assert_eq!(g, CliTool::Gemini);
    }

    #[test]
    fn config_for_returns_correct_config() {
        let claude = config_for(CliTool::Claude);
        assert_eq!(claude.base_dir_name, ".claude");
        assert_eq!(claude.display_name, "Claude Code");

        let codex = config_for(CliTool::Codex);
        assert_eq!(codex.base_dir_name, ".codex");

        let gemini = config_for(CliTool::Gemini);
        assert_eq!(gemini.base_dir_name, ".gemini");
    }

    #[test]
    fn all_tools_covers_every_variant() {
        let tools = all_tools();
        assert_eq!(tools.len(), 3);
        assert!(tools.iter().any(|c| c.tool == CliTool::Claude));
        assert!(tools.iter().any(|c| c.tool == CliTool::Codex));
        assert!(tools.iter().any(|c| c.tool == CliTool::Gemini));
    }

    #[test]
    fn cli_tool_from_str_is_case_insensitive() {
        assert_eq!("Claude".parse::<CliTool>().unwrap(), CliTool::Claude);
        assert_eq!("CODEX".parse::<CliTool>().unwrap(), CliTool::Codex);
        assert_eq!("gemini".parse::<CliTool>().unwrap(), CliTool::Gemini);
    }

    #[test]
    fn cli_tool_from_alias_maps_coordination_values() {
        assert_eq!(
            CliTool::from_alias("claude_native").unwrap(),
            CliTool::Claude
        );
        assert_eq!(CliTool::from_alias("mesh").unwrap(), CliTool::Codex);
        assert_eq!(CliTool::from_alias("mesh_bridged").unwrap(), CliTool::Codex);
    }

    #[test]
    fn cli_tool_from_alias_rejects_unknown_values() {
        let err = CliTool::from_alias("unknown").unwrap_err();
        assert_eq!(err.to_string(), "unsupported cli tool 'unknown'");
    }
}
