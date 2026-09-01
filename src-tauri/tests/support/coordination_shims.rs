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
        pub use taurhaus_lib::templates::types::{
            BehavioralContract, RoleKind, RoleTemplate, RuntimeCompactSummary,
        };
    }
}

pub mod provider {
    pub mod daemon_client {
        pub use taurhaus_lib::provider::daemon_client::*;
    }

    pub mod path {
        pub use taurhaus_lib::provider::path::*;
    }

    pub mod platform_paths {
        pub use taurhaus_lib::provider::platform_paths::*;
    }
}

pub mod tmux_layout {
    pub use taurhaus_lib::tmux_layout::*;
}

pub mod session_scanner {
    pub use taurhaus_lib::session_scanner::{
        scan_sessions_for_display, scan_sessions_for_runtime, ActivityAttribution,
        ActivityConfidence, DisplaySession, RuntimeSession, SessionGroupKind, SessionState,
    };

    pub mod cli_tool {
        pub use taurhaus_lib::session_scanner::cli_tool::*;
    }

    pub mod accounts {
        pub use taurhaus_lib::session_scanner::accounts::{
            configured_default_dir, to_launch_namespace,
        };
    }

    pub mod process {
        pub use taurhaus_lib::session_scanner::process::detect_cli_tool;
    }

    pub mod transcript_boundary {
        pub use taurhaus_lib::session_scanner::transcript_boundary::*;
    }

    pub mod launch {
        use crate::coordination::domain::MemberRole;
        use crate::daemon::protocol::LaunchMode;
        use crate::session_scanner::cli_tool::CliTool;

        pub use taurhaus_lib::session_scanner::launch::{
            base_command, command_contains_flag, redact_command_for_logging, shell_escape,
            LaunchNote, ModelSpec, RenderedLaunch,
        };

        pub struct TeamContext<'a> {
            pub team_name: &'a str,
            pub agent_name: &'a str,
            pub role: MemberRole,
        }

        pub struct LaunchSpec<'a> {
            pub tool: CliTool,
            pub mode: LaunchMode,
            pub base: &'a str,
            pub model: ModelSpec,
            pub team: Option<TeamContext<'a>>,
            pub codex_bypass_hook_trust: bool,
            pub codex_notify_executable: Option<&'a std::path::Path>,
            pub account_dir: Option<&'a std::path::Path>,
            pub selector: Option<&'static str>,
        }

        impl LaunchSpec<'_> {
            pub fn render(&self) -> RenderedLaunch {
                taurhaus_lib::session_scanner::launch::LaunchSpec {
                    tool: self.tool,
                    mode: self.mode,
                    base: self.base,
                    model: self.model.clone(),
                    codex_bypass_hook_trust: self.codex_bypass_hook_trust,
                    codex_notify_executable: self.codex_notify_executable,
                    account_dir: self.account_dir,
                    selector: self.selector,
                    team: self.team.as_ref().map(|team| {
                        taurhaus_lib::session_scanner::launch::TeamContext {
                            team_name: team.team_name,
                            agent_name: team.agent_name,
                            role: match team.role {
                                MemberRole::Lead => {
                                    taurhaus_lib::coordination::domain::MemberRole::Lead
                                }
                                MemberRole::Agent => {
                                    taurhaus_lib::coordination::domain::MemberRole::Agent
                                }
                            },
                        }
                    }),
                }
                .render()
            }
        }
    }

    pub mod control {
        pub use taurhaus_lib::session_scanner::control::{
            launch_command_in_tmux_with_layout, split_command_in_tmux_target_pane,
            TMUX_SESSION_NAME,
        };

        // Mirrors `session_scanner::control::validate_command_override`:
        // commands are free-form; only empty/multi-line input is rejected.
        pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
            if cmd.trim().is_empty() {
                return Err("Command override is empty".to_string());
            }
            if let Some(c) = cmd.chars().find(|c| matches!(c, '\n' | '\r' | '\0')) {
                return Err(format!(
                    "Command override must be a single line without control characters, found: {c:?}"
                ));
            }
            Ok(())
        }
    }
}

pub mod daemon {
    pub mod initialize_runs {
        use crate::coordination::requests::{InitializeReport, StepProgress};
        use crate::coordination::state::CoordinationState;
        use crate::models::CliCommandSettings;

        pub(crate) fn execute_initialize_pipeline(
            state: &CoordinationState,
            request: &crate::coordination::requests::InitializeTeamRequest,
            cli_commands: &CliCommandSettings,
            tmux_layout: &str,
            mut emit: Option<&mut dyn FnMut(StepProgress)>,
        ) -> Result<InitializeReport, crate::coordination::errors::CoordinationError> {
            state.with_orchestrator(|orchestrator| {
                orchestrator.initialize_team_with_cli_commands_and_layout_and_progress(
                    request,
                    cli_commands,
                    tmux_layout,
                    Some(&mut |step, status, message| {
                        if let Some(emit) = emit.as_deref_mut() {
                            emit(StepProgress {
                                step: step.to_string(),
                                status,
                                message,
                            });
                        }
                    }),
                )
            })
        }
    }

    pub mod protocol {
        use serde::{Deserialize, Serialize};

        pub use taurhaus_lib::daemon_api::protocol::{DaemonRequest, LaunchMode};

        pub mod method {
            pub const COORDINATION_INITIALIZE_TEAM: &str = "coordination.initialize_team";
            pub const COORDINATION_INITIALIZE_STATUS: &str = "coordination.initialize_status";
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeParams {
            pub request: crate::coordination::requests::InitializeTeamRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
            #[serde(default)]
            pub operational_snapshots: Vec<crate::coordination::stores::OperationalContextSnapshot>,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeStatusParams {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationInitializeOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::InitializeReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeStatus {
            pub run_id: String,
            pub steps: Vec<crate::coordination::requests::StepProgress>,
            pub outcome: CoordinationInitializeOutcome,
        }
    }
}

pub mod workflow_runs {
    pub use taurhaus_lib::workflow_runs::{activity_for_transcript, WorkflowActivity};
}
