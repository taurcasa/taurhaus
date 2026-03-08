pub mod errors {
    pub use taurhaus_lib::errors::*;
}

pub mod models {
    pub use taurhaus_lib::models::*;
}

pub mod templates {
    pub mod composition {
        pub use taurhaus_lib::templates::composition::{
            compose_team, CompositionOverrides, ResolvedMember,
        };
    }

    pub mod storage {
        pub use taurhaus_lib::templates::storage::{TemplateStore, TemplateStoreError};
    }

    pub mod types {
        pub use taurhaus_lib::templates::types::{BehavioralContract, RoleTemplate};
    }
}

pub mod provider {
    pub mod path {
        pub use taurhaus_lib::provider::path::*;
    }
}

pub mod session_scanner {
    pub use taurhaus_lib::session_scanner::{
        scan_sessions_for_display, scan_sessions_for_runtime, ActivityAttribution,
        ActivityConfidence, DisplaySession, RuntimeSession, SessionGroupKind, SessionState,
    };

    pub mod cli_tool {
        pub use taurhaus_lib::session_scanner::cli_tool::CliTool;
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

        fn codex_command_has_model_arg(command: &str) -> bool {
            let mut tokens = command.split_whitespace();
            while let Some(token) = tokens.next() {
                if token == "-m" {
                    return true;
                }
                if token.starts_with("--model") {
                    return true;
                }
                if token == "--model" {
                    let _ = tokens.next();
                    return true;
                }
            }
            false
        }

        fn normalize_codex_model(model: &str) -> String {
            let trimmed = model.trim();
            let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            if compact.eq_ignore_ascii_case("gpt-5.4")
                || compact.eq_ignore_ascii_case("gpt-5.4 high")
                || compact.eq_ignore_ascii_case("gpt-5.4 medium")
                || compact.eq_ignore_ascii_case("gpt-5.4 low")
                || trimmed.eq_ignore_ascii_case("gpt-5.4-high")
                || trimmed.eq_ignore_ascii_case("gpt-5.4-medium")
                || trimmed.eq_ignore_ascii_case("gpt-5.4-low")
            {
                return "gpt-5.4".to_string();
            }
            if trimmed.eq_ignore_ascii_case("gpt-5.3") {
                return "gpt-5.3-codex".to_string();
            }
            compact
        }

        fn shell_escape(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
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
                    if model.is_empty() || codex_command_has_model_arg(&base) {
                        return base;
                    }
                    let normalized_model = normalize_codex_model(model);
                    format!("{base} -m {}", shell_escape(&normalized_model))
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
