use crate::coordination::domain::MemberRole;
use crate::daemon::protocol::LaunchMode;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

/// Model + effort as the role/member declared them. Parsed from the legacy single string
/// ("gpt-5.4 high", "gpt-5.4-high", "gpt-5.4", "claude-opus-4-6", "") until PR 5a splits the schema.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelSpec {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
}

impl ModelSpec {
    /// Split a legacy spelling into independent model and reasoning-effort fields.
    ///
    /// Trims the input, then recognizes a whole trailing `low`, `medium`,
    /// `high`, `xhigh`, `max`, or `ultra` token separated by whitespace or a
    /// hyphen. The remaining model slug is otherwise preserved without aliases.
    pub fn parse_legacy(raw: &str) -> ModelSpec {
        const EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return ModelSpec::default();
        }

        let split = trimmed
            .char_indices()
            .rev()
            .find(|(_, character)| character.is_whitespace())
            .map(|(index, _)| (&trimmed[..index], &trimmed[index..]))
            .or_else(|| trimmed.rsplit_once('-'));

        if let Some((model, effort)) = split {
            let model = model.trim_end();
            let effort = effort.trim().to_ascii_lowercase();
            if !model.is_empty() && EFFORTS.contains(&effort.as_str()) {
                return ModelSpec {
                    model: Some(model.to_string()),
                    reasoning_effort: Some(effort),
                };
            }
        }

        ModelSpec {
            model: Some(trimmed.to_string()),
            reasoning_effort: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.model.is_none() && self.reasoning_effort.is_none()
    }
}

pub struct TeamContext<'a> {
    pub team_name: &'a str,
    pub agent_name: &'a str,
    pub role: MemberRole,
}

pub struct LaunchSpec<'a> {
    pub tool: CliTool,
    pub mode: LaunchMode,
    /// Loaded user configuration for this tool and mode. Free-form shell syntax
    /// is preserved; only trailing whitespace is removed during rendering.
    pub base: &'a str,
    pub model: ModelSpec,
    pub team: Option<TeamContext<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchNote {
    /// The base contains a flag removed by the current CLI.
    DeprecatedFlag { flag: String },
    /// The base already selected a model, so it wins over `ModelSpec`.
    ModelIgnored { found: String },
    /// The base already selected an effort, so it wins over `ModelSpec`.
    EffortIgnored { found: String },
}

pub struct RenderedLaunch {
    pub command: String,
    pub notes: Vec<LaunchNote>,
}

impl LaunchSpec<'_> {
    pub fn render(&self) -> RenderedLaunch {
        let mut command = self.base.trim_end().to_string();
        let mut notes = Vec::new();

        match self.tool {
            CliTool::Codex => {
                if command_contains_flag(self.base, "--full-auto") {
                    notes.push(LaunchNote::DeprecatedFlag {
                        flag: "--full-auto".to_string(),
                    });
                }

                if let Some(model) = self.model.model.as_deref() {
                    if let Some(found) = first_present_flag(self.base, &["-m", "--model"]) {
                        notes.push(LaunchNote::ModelIgnored {
                            found: found.to_string(),
                        });
                    } else {
                        append_flag(&mut command, "-m", model);
                    }
                }

                if let Some(effort) = self.model.reasoning_effort.as_deref() {
                    if command_contains_flag(self.base, "model_reasoning_effort") {
                        notes.push(LaunchNote::EffortIgnored {
                            found: "model_reasoning_effort".to_string(),
                        });
                    } else {
                        append_flag(
                            &mut command,
                            "-c",
                            &format!("model_reasoning_effort=\"{effort}\""),
                        );
                    }
                }
            }
            CliTool::Claude => {
                if let Some(model) = self.model.model.as_deref() {
                    if command_contains_flag(self.base, "--model") {
                        notes.push(LaunchNote::ModelIgnored {
                            found: "--model".to_string(),
                        });
                    } else {
                        append_flag(&mut command, "--model", model);
                    }
                }

                if let Some(effort) = self.model.reasoning_effort.as_deref() {
                    if command_contains_flag(self.base, "--effort") {
                        notes.push(LaunchNote::EffortIgnored {
                            found: "--effort".to_string(),
                        });
                    } else {
                        append_flag(&mut command, "--effort", effort);
                    }
                }

                if let Some(team) = self.team.as_ref() {
                    if !self.base.contains("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=") {
                        command = format!(
                            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 {command}"
                        );
                    }

                    append_flag_unless_present(
                        &mut command,
                        self.base,
                        "--team-name",
                        team.team_name,
                    );
                    append_flag_unless_present(
                        &mut command,
                        self.base,
                        "--agent-name",
                        team.agent_name,
                    );
                    append_flag_unless_present(
                        &mut command,
                        self.base,
                        "--agent-id",
                        &format!("{}@{}", team.agent_name, team.team_name),
                    );
                    let agent_type = if team.role == MemberRole::Lead {
                        "orchestrator"
                    } else {
                        "general-purpose"
                    };
                    append_flag_unless_present(&mut command, self.base, "--agent-type", agent_type);
                    if first_present_flag(self.base, &["-n", "--name"]).is_none() {
                        append_flag(&mut command, "-n", team.agent_name);
                    }
                }
            }
            CliTool::Gemini => {
                // unverified (S12): Gemini is not installed on the audit host.
                if let Some(model) = self.model.model.as_deref() {
                    if command_contains_flag(self.base, "-m") {
                        notes.push(LaunchNote::ModelIgnored {
                            found: "-m".to_string(),
                        });
                    } else {
                        append_flag(&mut command, "-m", model);
                    }
                }
            }
        }

        RenderedLaunch { command, notes }
    }
}

/// Return the configured base command without normalizing or rewriting it.
pub fn base_command(commands: &CliCommandSettings, tool: CliTool, mode: LaunchMode) -> &str {
    let tool_commands = match tool {
        CliTool::Claude => &commands.claude,
        CliTool::Codex => &commands.codex,
        CliTool::Gemini => &commands.gemini,
    };

    match mode {
        LaunchMode::Continue => &tool_commands.continue_cmd,
        LaunchMode::Fresh => &tool_commands.fresh,
        LaunchMode::Resume => &tool_commands.resume,
    }
}

pub(crate) fn command_contains_flag(command: &str, flag: &str) -> bool {
    command.split_whitespace().any(|token| {
        let token = token.trim_start_matches(['\'', '"']);
        token == flag
            || token
                .strip_prefix(flag)
                .is_some_and(|suffix| suffix.starts_with('='))
    })
}

pub(crate) fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn first_present_flag<'a>(command: &str, flags: &'a [&str]) -> Option<&'a str> {
    flags
        .iter()
        .copied()
        .find(|flag| command_contains_flag(command, flag))
}

fn append_flag(command: &mut String, flag: &str, value: &str) {
    command.push(' ');
    command.push_str(flag);
    command.push(' ');
    command.push_str(&shell_escape(value));
}

fn append_flag_unless_present(command: &mut String, base: &str, flag: &str, value: &str) {
    if !command_contains_flag(base, flag) {
        append_flag(command, flag, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_spec(model: &str, reasoning_effort: Option<&str>) -> ModelSpec {
        ModelSpec {
            model: Some(model.to_string()),
            reasoning_effort: reasoning_effort.map(str::to_string),
        }
    }

    fn count_flag(command: &str, flag: &str) -> usize {
        command
            .split_whitespace()
            .filter(|token| *token == flag || token.starts_with(&format!("{flag}=")))
            .count()
    }

    #[test]
    fn parse_legacy_splits_space_and_dash_suffix() {
        for raw in ["gpt-5.4 high", "gpt-5.4-high", "  gpt-5.4 high  "] {
            assert_eq!(
                ModelSpec::parse_legacy(raw),
                model_spec("gpt-5.4", Some("high")),
                "legacy spelling {raw:?}"
            );
        }
    }

    #[test]
    fn parse_legacy_keeps_model_only() {
        for raw in ["claude-opus-4-6", "gpt-5.6-terra"] {
            assert_eq!(ModelSpec::parse_legacy(raw), model_spec(raw, None));
        }
    }

    #[test]
    fn parse_legacy_empty() {
        assert!(ModelSpec::parse_legacy("  ").is_empty());
    }

    // Regression: ff40911 stripped the suffix and 5d2ce27 aliased gpt-5.3;
    // roles declaring "gpt-5.4 high" ran at the user's global xhigh.
    #[test]
    fn codex_render_emits_reasoning_effort_for_legacy_model() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::parse_legacy("gpt-5.4 high"),
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "codex --yolo -m 'gpt-5.4' -c 'model_reasoning_effort=\"high\"'"
        );
    }

    #[test]
    fn codex_render_does_not_alias_gpt_5_3() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::parse_legacy("gpt-5.3"),
            team: None,
        }
        .render();

        assert!(rendered.command.contains("-m 'gpt-5.3'"));
        assert!(!rendered.command.contains("gpt-5.3-codex"));
    }

    #[test]
    fn codex_render_respects_base_model_flag_and_notes_it() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo --model gpt-6",
            model: model_spec("gpt-5.4", None),
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "codex --yolo --model gpt-6");
        assert_eq!(
            rendered.notes,
            vec![LaunchNote::ModelIgnored {
                found: "--model".to_string()
            }]
        );
    }

    #[test]
    fn codex_render_notes_deprecated_full_auto() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --full-auto",
            model: ModelSpec::default(),
            team: None,
        }
        .render();

        assert_eq!(
            rendered.notes,
            vec![LaunchNote::DeprecatedFlag {
                flag: "--full-auto".to_string()
            }]
        );
    }

    #[test]
    fn codex_render_never_adds_sandbox_flags() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: model_spec("gpt-5.4", Some("high")),
            team: None,
        }
        .render();

        assert!(!rendered.command.contains("--sandbox"));
        assert!(!rendered.command.contains("--ask-for-approval"));
    }

    #[test]
    fn claude_render_adds_model_effort_and_display_name_once() {
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions --team-name 'ledger-team'",
            model: model_spec("claude-opus-4-6", Some("high")),
            team: Some(TeamContext {
                team_name: "ledger-team",
                agent_name: "team-lead",
                role: MemberRole::Lead,
            }),
        }
        .render();

        assert_eq!(count_flag(&rendered.command, "--model"), 1);
        assert_eq!(count_flag(&rendered.command, "--effort"), 1);
        assert_eq!(count_flag(&rendered.command, "--team-name"), 1);
        assert_eq!(count_flag(&rendered.command, "--agent-name"), 1);
        assert_eq!(count_flag(&rendered.command, "--agent-id"), 1);
        assert_eq!(count_flag(&rendered.command, "--agent-type"), 1);
        assert_eq!(count_flag(&rendered.command, "-n"), 1);
        assert!(rendered.command.contains("--model 'claude-opus-4-6'"));
        assert!(rendered.command.contains("--effort 'high'"));
        assert!(rendered.command.contains("-n 'team-lead'"));
    }

    #[test]
    fn claude_team_context_matches_previous_with_claude_team_context_output() {
        let previous_output = concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions ",
            "--team-name 'ledger-team' --agent-name 'team-lead' ",
            "--agent-id 'team-lead@ledger-team' --agent-type 'orchestrator' ",
            "-n 'team-lead'"
        );
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: previous_output,
            model: ModelSpec::default(),
            team: Some(TeamContext {
                team_name: "ledger-team",
                agent_name: "team-lead",
                role: MemberRole::Lead,
            }),
        }
        .render();

        assert_eq!(rendered.command, previous_output);
    }

    #[test]
    fn base_is_preserved_verbatim() {
        let base = "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions";
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base,
            model: model_spec("claude-opus-4-6", None),
            team: None,
        }
        .render();

        assert!(rendered.command.starts_with(base));
    }

    #[test]
    fn gemini_render_adds_model_only() {
        let rendered = LaunchSpec {
            tool: CliTool::Gemini,
            mode: LaunchMode::Fresh,
            base: "gemini --yolo",
            model: model_spec("gemini-3.1-pro", Some("high")),
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "gemini --yolo -m 'gemini-3.1-pro'");
        assert!(rendered.notes.is_empty());
    }
}
