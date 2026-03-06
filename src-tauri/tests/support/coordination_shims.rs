pub mod errors {
    pub use taurhaus_lib::errors::*;
}

pub mod models {
    pub use taurhaus_lib::models::*;
}

pub mod templates {
    pub mod types {
        pub use taurhaus_lib::templates::types::BehavioralContract;
    }
}

pub mod session_scanner {
    pub use taurhaus_lib::session_scanner::{scan_sessions, ActivityConfidence, SessionState};

    pub mod cli_tool {
        use serde::{Deserialize, Serialize};

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

        impl CliTool {
            pub fn from_alias(raw: &str) -> Result<Self, String> {
                match raw.trim().to_ascii_lowercase().as_str() {
                    "claude" | "claude_native" => Ok(Self::Claude),
                    "codex" | "mesh" | "mesh_bridged" => Ok(Self::Codex),
                    "gemini" => Ok(Self::Gemini),
                    _ => Err(format!("unsupported cli tool '{}'", raw.trim())),
                }
            }
        }
    }

    pub mod control {
        use crate::daemon::protocol::LaunchMode;
        use crate::models::CliCommandSettings;

        use super::cli_tool::CliTool;

        pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
            let first_token = cmd.split_whitespace().next().unwrap_or("");
            let base_name = first_token.rsplit('/').next().unwrap_or(first_token);
            const ALLOWED_TOOLS: &[&str] = &["claude", "codex", "gemini"];
            if !ALLOWED_TOOLS.contains(&base_name) {
                return Err(format!(
                    "Command override must start with claude/codex/gemini, got: {base_name}"
                ));
            }
            Ok(())
        }

        pub fn resolve_configured_tool_command(
            cmds: &CliCommandSettings,
            tool: CliTool,
            mode: LaunchMode,
        ) -> String {
            let tool_cmds = match tool {
                CliTool::Claude => &cmds.claude,
                CliTool::Codex => &cmds.codex,
                CliTool::Gemini => &cmds.gemini,
            };
            match mode {
                LaunchMode::Continue => tool_cmds.continue_cmd.clone(),
                LaunchMode::Fresh => tool_cmds.fresh.clone(),
                LaunchMode::Resume => tool_cmds.resume.clone(),
            }
        }

        pub fn build_team_launch_command(
            cmds: &CliCommandSettings,
            tool: CliTool,
            model: &str,
        ) -> String {
            match tool {
                CliTool::Claude => cmds.claude.fresh.clone(),
                CliTool::Gemini => cmds.gemini.fresh.clone(),
                CliTool::Codex => {
                    let base = cmds.codex.fresh.clone();
                    let model = model.trim();
                    if model.is_empty() || base.contains("-m ") || base.contains("--model") {
                        return base;
                    }
                    let model = if model.eq_ignore_ascii_case("gpt-5.4")
                        || model.eq_ignore_ascii_case("gpt-5.4-high")
                    {
                        "gpt-5.4 high".to_string()
                    } else if model.eq_ignore_ascii_case("gpt-5.3") {
                        "gpt-5.3-codex".to_string()
                    } else {
                        model.to_string()
                    };
                    format!("{base} -m '{model}'")
                }
            }
        }
    }
}

pub mod daemon {
    pub mod protocol {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum LaunchMode {
            Continue,
            Fresh,
            Resume,
        }
    }
}
