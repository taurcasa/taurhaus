//! CLI tool abstraction — detect and configure multiple AI coding tools.
//!
//! Supports Claude Code, Codex CLI, and Gemini CLI. Each tool has its
//! own process signature, session directory layout, and launch commands.

use serde::{Deserialize, Serialize};

/// Which CLI tool a session belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliTool {
    Claude,
    Codex,
    Gemini,
}

impl std::fmt::Display for CliTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliTool::Claude => write!(f, "claude"),
            CliTool::Codex => write!(f, "codex"),
            CliTool::Gemini => write!(f, "gemini"),
        }
    }
}

/// Static configuration for a CLI tool — directory layout, commands, etc.
pub struct CliToolConfig {
    pub tool: CliTool,
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

static TOOL_CONFIGS: &[CliToolConfig] = &[
    CliToolConfig {
        tool: CliTool::Claude,
        display_name: "Claude Code",
        base_dir_name: ".claude",
        projects_subdir: "projects",
        session_extension: "jsonl",
        exit_command: "/exit",
    },
    CliToolConfig {
        tool: CliTool::Codex,
        display_name: "Codex CLI",
        base_dir_name: ".codex",
        projects_subdir: "sessions",
        session_extension: "jsonl",
        exit_command: "/exit",
    },
    CliToolConfig {
        tool: CliTool::Gemini,
        display_name: "Gemini CLI",
        base_dir_name: ".gemini",
        projects_subdir: "tmp",
        session_extension: "jsonl",
        exit_command: "/exit",
    },
];

/// Get all registered tool configurations.
pub fn all_tools() -> &'static [CliToolConfig] {
    TOOL_CONFIGS
}

/// Get the configuration for a specific tool.
pub fn config_for(tool: CliTool) -> &'static CliToolConfig {
    TOOL_CONFIGS
        .iter()
        .find(|c| c.tool == tool)
        .expect("every CliTool variant has a config entry")
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
}
