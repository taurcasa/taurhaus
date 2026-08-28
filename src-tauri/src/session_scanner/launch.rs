use crate::coordination::domain::MemberRole;
use crate::daemon::protocol::LaunchMode;
use crate::models::{CliCommandSettings, ModelCatalog};
use crate::session_scanner::cli_tool::{spec, CliCapabilities, CliTool, EffortFlag};

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

        // Canonical catalog ids win over the legacy `model-effort` migration
        // spelling. Several Antigravity providers publish real ids ending in
        // an effort word, so prefix heuristics are not sufficient here.
        if ModelCatalog::contains_model_id(trimmed) {
            return ModelSpec {
                model: Some(trimmed.to_string()),
                reasoning_effort: None,
            };
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
    /// Managed Codex launches opt into the taurhaus-written user hook without
    /// prompting for hook trust. Unmanaged launches always leave this false.
    pub codex_bypass_hook_trust: bool,
    /// Managed Codex launches with native notify support receive the daemon
    /// executable that will persist turn-complete edges. Unmanaged or
    /// unsupported launches leave this unset.
    pub codex_notify_executable: Option<&'a std::path::Path>,
    /// Account directory this launch runs on. Unset means the tool default.
    pub account_dir: Option<&'a std::path::Path>,
    /// Registry-declared environment selector for `account_dir`.
    pub selector: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchNote {
    /// A requested value has no declared flag-shaped launch capability.
    CapabilityMissing {
        capability: LaunchCapability,
        found: String,
    },
    /// The base contains a flag removed by the current CLI.
    DeprecatedFlag { flag: String },
    /// The base already selected a model, so it wins over `ModelSpec`.
    ModelIgnored { found: String },
    /// The base already selected a Codex notifier, so it wins over the managed
    /// taurhaus turn-complete sink.
    NotifyIgnored { found: String },
    /// The selected catalog model is deprecated and has a preferred replacement.
    ModelDeprecated {
        found: String,
        replacement: Option<String>,
    },
    /// The requested effort was ignored because the base overrides it or the
    /// selected tool/model does not support it.
    EffortIgnored {
        found: String,
        reason: EffortIgnoreReason,
    },
    /// The base already selected an account dir, so it wins over taurhaus.
    SelectorIgnored { found: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchCapability {
    Model,
    Effort,
    DisplayName,
    Selector,
}

impl LaunchCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Effort => "effort",
            Self::DisplayName => "displayName",
            Self::Selector => "selector",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortIgnoreReason {
    BaseOverride,
    Invalid,
}

impl LaunchNote {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::CapabilityMissing { .. } => "launch.capability_missing",
            Self::DeprecatedFlag { .. } => "launch.flag.deprecated",
            Self::ModelIgnored { .. } => "launch.model.ignored",
            Self::NotifyIgnored { .. } => "launch.notify.ignored",
            Self::ModelDeprecated { .. } => "launch.model.deprecated",
            Self::EffortIgnored {
                reason: EffortIgnoreReason::BaseOverride,
                ..
            } => "launch.effort.ignored",
            Self::EffortIgnored {
                reason: EffortIgnoreReason::Invalid,
                ..
            } => "launch.effort.invalid",
            Self::SelectorIgnored { .. } => "launch.selector.ignored",
        }
    }
}

pub struct RenderedLaunch {
    pub command: String,
    pub notes: Vec<LaunchNote>,
}

impl LaunchSpec<'_> {
    pub fn render(&self) -> RenderedLaunch {
        self.render_with_capabilities(spec(self.tool).capabilities)
    }

    /// Render against explicit capability data for registry conformance tests.
    pub fn render_with_capabilities(&self, capabilities: CliCapabilities) -> RenderedLaunch {
        let mut command = self.base.trim_end().to_string();
        let mut notes = Vec::new();
        let mut requested_model = self.model.model.as_deref();
        let mut requested_effort = self.model.reasoning_effort.as_deref();

        if !capabilities.catalog {
            if let Some(model) = requested_model.take() {
                notes.push(LaunchNote::CapabilityMissing {
                    capability: LaunchCapability::Model,
                    found: model.to_string(),
                });
            }
            if let Some(effort) = requested_effort.take() {
                notes.push(LaunchNote::CapabilityMissing {
                    capability: LaunchCapability::Effort,
                    found: effort.to_string(),
                });
            }
        }

        match self.tool {
            CliTool::Codex => {
                if capabilities.hook_trust
                    && self.codex_bypass_hook_trust
                    && !command_contains_flag(&command, "--dangerously-bypass-hook-trust")
                {
                    command.push_str(" --dangerously-bypass-hook-trust");
                }
                if let Some(executable) = self
                    .codex_notify_executable
                    .filter(|_| capabilities.notify_sink)
                {
                    if command_contains_codex_config(self.base, "notify") {
                        notes.push(LaunchNote::NotifyIgnored {
                            found: "notify".to_string(),
                        });
                    } else {
                        let notify = serde_json::to_string(&[
                            executable.to_string_lossy().as_ref(),
                            "codex-notify",
                        ])
                        .expect("string-only Codex notify command serializes");
                        append_flag(&mut command, "-c", &format!("notify={notify}"));
                    }
                }
                if command_contains_flag(self.base, "--full-auto") {
                    notes.push(LaunchNote::DeprecatedFlag {
                        flag: "--full-auto".to_string(),
                    });
                }

                let base_model = capabilities.model_flag.and_then(|model_flag| {
                    first_present_flag_value(self.base, &[model_flag, "--model"])
                });
                if let Some(model) = requested_model {
                    if let Some(model_flag) = capabilities.model_flag {
                        if let Some(found) = first_present_flag(self.base, &[model_flag, "--model"])
                        {
                            notes.push(LaunchNote::ModelIgnored {
                                found: found.to_string(),
                            });
                        } else {
                            if let Some(entry) = ModelCatalog::entry_for(self.tool, model)
                                .filter(|entry| entry.deprecated)
                            {
                                notes.push(LaunchNote::ModelDeprecated {
                                    found: model.to_string(),
                                    replacement: entry.replacement.clone(),
                                });
                            }
                            append_flag(&mut command, model_flag, model);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Model,
                            found: model.to_string(),
                        });
                    }
                }

                if let Some(effort) = requested_effort {
                    let effective_model = base_model.as_deref().or(requested_model);
                    if !ModelCatalog::supports_effort(self.tool, effective_model, effort) {
                        notes.push(LaunchNote::EffortIgnored {
                            found: effort.to_string(),
                            reason: EffortIgnoreReason::Invalid,
                        });
                    } else if let Some(EffortFlag::Config { flag, key }) = capabilities.effort_flag
                    {
                        if command_contains_flag(self.base, key) {
                            notes.push(LaunchNote::EffortIgnored {
                                found: key.to_string(),
                                reason: EffortIgnoreReason::BaseOverride,
                            });
                        } else {
                            append_flag(&mut command, flag, &format!("{key}=\"{effort}\""));
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Effort,
                            found: effort.to_string(),
                        });
                    }
                }
            }
            CliTool::Claude => {
                if let Some(model) = requested_model {
                    if let Some(model_flag) = capabilities.model_flag {
                        if command_contains_flag(self.base, model_flag) {
                            notes.push(LaunchNote::ModelIgnored {
                                found: model_flag.to_string(),
                            });
                        } else {
                            append_flag(&mut command, model_flag, model);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Model,
                            found: model.to_string(),
                        });
                    }
                }

                if let Some(effort) = requested_effort {
                    if !ModelCatalog::supports_effort(self.tool, requested_model, effort) {
                        notes.push(LaunchNote::EffortIgnored {
                            found: effort.to_string(),
                            reason: EffortIgnoreReason::Invalid,
                        });
                    } else if let Some(EffortFlag::Argument { flag }) = capabilities.effort_flag {
                        if command_contains_flag(self.base, flag) {
                            notes.push(LaunchNote::EffortIgnored {
                                found: flag.to_string(),
                                reason: EffortIgnoreReason::BaseOverride,
                            });
                        } else {
                            append_flag(&mut command, flag, effort);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Effort,
                            found: effort.to_string(),
                        });
                    }
                }

                if let Some(team) = self.team.as_ref().filter(|_| capabilities.team_flags) {
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
                    if let Some(display_name_flag) = capabilities.display_name_flag {
                        if first_present_flag(self.base, &[display_name_flag, "--name"]).is_none() {
                            append_flag(&mut command, display_name_flag, team.agent_name);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::DisplayName,
                            found: team.agent_name.to_string(),
                        });
                    }
                }
            }
            CliTool::Agy => {
                if let Some(model) = requested_model {
                    if let Some(model_flag) = capabilities.model_flag {
                        if let Some(found) = first_present_flag(self.base, &[model_flag, "--model"])
                        {
                            notes.push(LaunchNote::ModelIgnored {
                                found: found.to_string(),
                            });
                        } else {
                            append_flag(&mut command, model_flag, model);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Model,
                            found: model.to_string(),
                        });
                    }
                }

                if let Some(effort) = requested_effort {
                    if !ModelCatalog::supports_effort(self.tool, requested_model, effort) {
                        notes.push(LaunchNote::EffortIgnored {
                            found: effort.to_string(),
                            reason: EffortIgnoreReason::Invalid,
                        });
                    } else if let Some(EffortFlag::Argument { flag }) = capabilities.effort_flag {
                        if command_contains_flag(self.base, flag) {
                            notes.push(LaunchNote::EffortIgnored {
                                found: flag.to_string(),
                                reason: EffortIgnoreReason::BaseOverride,
                            });
                        } else {
                            append_flag(&mut command, flag, effort);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Effort,
                            found: effort.to_string(),
                        });
                    }
                }
            }
            CliTool::Grok => {
                if let Some(model) = requested_model {
                    if let Some(model_flag) = capabilities.model_flag {
                        if let Some(found) =
                            first_present_flag(self.base, &[model_flag, "--model", "-m"])
                        {
                            notes.push(LaunchNote::ModelIgnored {
                                found: found.to_string(),
                            });
                        } else {
                            append_flag(&mut command, model_flag, model);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Model,
                            found: model.to_string(),
                        });
                    }
                }

                if let Some(effort) = requested_effort {
                    let effective_model = capabilities
                        .model_flag
                        .and_then(|model_flag| {
                            first_present_flag_value(self.base, &[model_flag, "--model", "-m"])
                        })
                        .or_else(|| requested_model.map(str::to_string));
                    if !ModelCatalog::supports_effort(self.tool, effective_model.as_deref(), effort)
                    {
                        notes.push(LaunchNote::EffortIgnored {
                            found: effort.to_string(),
                            reason: EffortIgnoreReason::Invalid,
                        });
                    } else if let Some(EffortFlag::Argument { flag }) = capabilities.effort_flag {
                        if let Some(found) =
                            first_present_flag(self.base, &[flag, "--reasoning-effort"])
                        {
                            notes.push(LaunchNote::EffortIgnored {
                                found: found.to_string(),
                                reason: EffortIgnoreReason::BaseOverride,
                            });
                        } else {
                            append_flag(&mut command, flag, effort);
                        }
                    } else {
                        notes.push(LaunchNote::CapabilityMissing {
                            capability: LaunchCapability::Effort,
                            found: effort.to_string(),
                        });
                    }
                }
            }
            CliTool::Unknown => {}
        }

        // Last, so the selector lands in front of any tool-specific team
        // environment the match above may have prepended.
        if let Some(account_dir) = self.account_dir {
            if let Some(selector) = self.selector {
                if crate::session_scanner::accounts::command_contains_env(self.base, selector) {
                    notes.push(LaunchNote::SelectorIgnored {
                        found: selector.to_string(),
                    });
                } else {
                    let assignment = shell_escape(&account_dir.to_string_lossy());
                    command = format!("{selector}={assignment} {command}");
                }
            } else {
                notes.push(LaunchNote::CapabilityMissing {
                    capability: LaunchCapability::Selector,
                    found: account_dir.to_string_lossy().into_owned(),
                });
            }
        }

        RenderedLaunch { command, notes }
    }
}

/// Return the configured base command without normalizing or rewriting it.
pub fn base_command(commands: &CliCommandSettings, tool: CliTool, mode: LaunchMode) -> &str {
    let tool_commands = commands.get(tool);

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

fn command_contains_codex_config(command: &str, key: &str) -> bool {
    command.match_indices(key).any(|(index, _)| {
        let before = command[..index].chars().next_back();
        let after = command[index + key.len()..].trim_start();
        let starts_config_key = before.is_none_or(|character| {
            character.is_whitespace() || matches!(character, '\'' | '"' | '=')
        });
        starts_config_key && after.starts_with('=')
    })
}

pub(crate) fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Redact secret-like shell assignments before persisting a rendered command.
/// The executable command remains untouched; this is only for structured logs.
pub fn redact_command_for_logging(command: &str) -> String {
    let mut redacted = String::with_capacity(command.len());
    let mut cursor = 0;

    while cursor < command.len() {
        let character = command[cursor..]
            .chars()
            .next()
            .expect("cursor remains on a character boundary");
        if character.is_whitespace() {
            redacted.push(character);
            cursor += character.len_utf8();
            continue;
        }

        let word_end = shell_word_end(command, cursor);
        let word = &command[cursor..word_end];
        if let Some((name, _)) = word.split_once('=') {
            if is_secret_assignment_name(name) {
                redacted.push_str(name);
                redacted.push_str("=[REDACTED]");
                cursor = word_end;
                continue;
            }
        }

        redacted.push_str(word);
        cursor = word_end;
    }

    redacted
}

fn shell_word_end(command: &str, start: usize) -> usize {
    let mut quote = None;
    let mut escaped = false;

    for (offset, character) in command[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match quote {
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                }
            }
            Some('"') => match character {
                '"' => quote = None,
                '\\' => escaped = true,
                _ => {}
            },
            Some(_) => unreachable!("only shell quote characters are stored"),
            None => match character {
                '\'' | '"' => quote = Some(character),
                '\\' => escaped = true,
                _ if character.is_whitespace() => return start + offset,
                _ => {}
            },
        }
    }

    command.len()
}

fn is_secret_assignment_name(name: &str) -> bool {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return false;
    }

    let uppercase = name.to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "PASSWORD"]
        .iter()
        .any(|marker| uppercase.contains(marker))
}

fn first_present_flag<'a>(command: &str, flags: &'a [&str]) -> Option<&'a str> {
    flags
        .iter()
        .copied()
        .find(|flag| command_contains_flag(command, flag))
}

fn first_present_flag_value(command: &str, flags: &[&str]) -> Option<String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    for flag in flags {
        for (index, token) in tokens.iter().enumerate() {
            let token = token.trim_start_matches(['\'', '"']);
            if token == *flag {
                return tokens
                    .get(index + 1)
                    .map(|value| value.trim_matches(['\'', '"']).to_string());
            }
            if let Some(value) = token
                .strip_prefix(flag)
                .and_then(|suffix| suffix.strip_prefix('='))
            {
                return Some(value.trim_matches(['\'', '"']).to_string());
            }
        }
    }
    None
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
        // Regression: commit 5576838 only protected Google-prefixed Antigravity
        // ids, truncating the catalog's GPT-OSS id into a nonexistent model.
        for raw in [
            "claude-opus-4-6",
            "gpt-5.6-terra",
            "gemini-3.7-flash-high",
            "gemini-3.1-pro-low",
            "gpt-oss-120b-medium",
        ] {
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
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "codex --yolo -m 'gpt-5.4' -c 'model_reasoning_effort=\"high\"'"
        );
    }

    // Regression: 0b87699 had no Codex hook trust bridge, so managed launches
    // left taurhaus's user-level compaction hook disabled.
    #[test]
    fn managed_codex_launch_bypasses_trust_only_when_requested() {
        let trusted = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::default(),
            team: None,
            codex_bypass_hook_trust: true,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
        }
        .render();
        assert!(trusted.command.contains("--dangerously-bypass-hook-trust"));

        let unmanaged = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::default(),
            team: None,
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
        }
        .render();
        assert!(!unmanaged
            .command
            .contains("--dangerously-bypass-hook-trust"));
    }

    // Regression: 791f6be centralized managed launch rendering without Codex's
    // native turn-complete notify, so completed turns stayed active until the
    // fd/rchar heuristic and display hysteresis settled.
    #[test]
    fn managed_codex_launch_renders_turn_complete_notify() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::default(),
            team: None,
            codex_bypass_hook_trust: false,
            codex_notify_executable: Some(std::path::Path::new(
                "/home/test/.local/bin/taurhaus-daemon",
            )),
            account_dir: None,
            selector: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            concat!(
                "codex --yolo -c '",
                "notify=[\"/home/test/.local/bin/taurhaus-daemon\",\"codex-notify\"]'"
            )
        );
        assert!(rendered.notes.is_empty());
    }

    #[test]
    fn managed_codex_launch_preserves_user_notify_and_notes_it() {
        for base in [
            "codex --yolo -c 'notify=[\"custom-notifier\"]'",
            "codex --yolo -c 'notify = [\"custom-notifier\"]'",
            "codex --yolo --config=notify=[\"custom-notifier\"]",
        ] {
            let rendered = LaunchSpec {
                tool: CliTool::Codex,
                mode: LaunchMode::Fresh,
                base,
                model: ModelSpec::default(),
                team: None,
                codex_bypass_hook_trust: false,
                codex_notify_executable: Some(std::path::Path::new(
                    "/home/test/.local/bin/taurhaus-daemon",
                )),
                account_dir: None,
                selector: None,
            }
            .render();

            assert_eq!(rendered.command, base);
            assert_eq!(
                rendered.notes,
                vec![LaunchNote::NotifyIgnored {
                    found: "notify".to_string(),
                }]
            );
        }
    }

    #[test]
    fn codex_render_does_not_alias_gpt_5_3() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::parse_legacy("gpt-5.3"),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
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
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
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
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
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

    // Regression: a79d392 added catalog deprecation metadata for gpt-5.4,
    // but launches using that model emitted no actionable replacement note.
    #[test]
    fn codex_render_notes_deprecated_catalog_model() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: model_spec("gpt-5.4", None),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert!(rendered.notes.iter().any(|note| matches!(
            note,
            LaunchNote::ModelDeprecated { found, replacement }
                if found == "gpt-5.4"
                    && replacement.as_deref() == Some("gpt-5.6-sol")
                    && note.event_name() == "launch.model.deprecated"
        )));
    }

    #[test]
    fn codex_render_never_adds_sandbox_flags() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: model_spec("gpt-5.4", Some("high")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert!(!rendered.command.contains("--sandbox"));
        assert!(!rendered.command.contains("--ask-for-approval"));
    }

    // Regression: ff40911 discarded a role's explicit effort instead of
    // validating it, allowing the user's global effort to win silently.
    #[test]
    fn codex_render_notes_and_drops_effort_invalid_for_model() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: model_spec("gpt-5.4", Some("ultra")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert!(!rendered.command.contains("model_reasoning_effort"));
        assert!(rendered.notes.iter().any(
            |note| matches!(note, LaunchNote::EffortIgnored { found, .. } if found == "ultra")
        ));
    }

    // Regression: a79d392 validated effort against the declared model even
    // when the free-form base pinned a different effective Codex model.
    #[test]
    fn codex_effort_is_validated_against_base_model() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo -m gpt-5.5",
            model: model_spec("gpt-5.6-sol", Some("ultra")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "codex --yolo -m gpt-5.5");
        assert_eq!(
            rendered.notes,
            vec![
                LaunchNote::ModelIgnored {
                    found: "-m".to_string(),
                },
                LaunchNote::EffortIgnored {
                    found: "ultra".to_string(),
                    reason: EffortIgnoreReason::Invalid,
                },
            ]
        );
    }

    // Regression: a79d392 validated model-less effort against the catalog
    // default, and the review fix then treated the catalog as an allowlist;
    // a declared effort must render whenever it is in Codex's vocabulary,
    // even when the effective model is unknown to the static catalog —
    // Codex validates the pair itself.
    #[test]
    fn codex_effort_without_an_effective_model_renders_when_in_vocabulary() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec {
                model: None,
                reasoning_effort: Some("max".to_string()),
            },
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "codex --yolo -c 'model_reasoning_effort=\"max\"'"
        );
        assert!(rendered.notes.is_empty(), "notes: {:?}", rendered.notes);

        let invalid = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec {
                model: Some("gpt-5.7-nova".to_string()),
                reasoning_effort: Some("turbo".to_string()),
            },
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();
        assert_eq!(invalid.command, "codex --yolo -m 'gpt-5.7-nova'");
        assert_eq!(
            invalid.notes,
            vec![LaunchNote::EffortIgnored {
                found: "turbo".to_string(),
                reason: EffortIgnoreReason::Invalid,
            }]
        );
    }

    #[test]
    fn claude_render_emits_fable_with_its_effort() {
        // Fable 5 is the Claude catalog's leading model (models/mod.rs) and the
        // Claude CLI accepts `--model fable`; the renderer must pass the id
        // through with the declared effort and add nothing else.
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: model_spec("fable", Some("high")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "claude --dangerously-skip-permissions --model 'fable' --effort 'high'"
        );
        assert_eq!(rendered.notes, vec![]);
    }

    #[test]
    fn claude_render_notes_and_drops_unknown_effort() {
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: model_spec("opus", Some("ultra")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert!(!rendered.command.contains("--effort"));
        assert!(matches!(
            rendered.notes.as_slice(),
            [LaunchNote::EffortIgnored { found, .. }] if found == "ultra"
        ));
    }

    #[test]
    fn launch_note_events_use_entity_verb_shape() {
        assert_eq!(
            LaunchNote::DeprecatedFlag {
                flag: "--full-auto".to_string()
            }
            .event_name(),
            "launch.flag.deprecated"
        );
        assert_eq!(
            LaunchNote::ModelIgnored {
                found: "--model".to_string()
            }
            .event_name(),
            "launch.model.ignored"
        );
        assert_eq!(
            LaunchNote::NotifyIgnored {
                found: "notify".to_string()
            }
            .event_name(),
            "launch.notify.ignored"
        );
        assert_eq!(
            LaunchNote::ModelDeprecated {
                found: "gpt-5.4".to_string(),
                replacement: Some("gpt-5.6-terra".to_string()),
            }
            .event_name(),
            "launch.model.deprecated"
        );
        assert_eq!(
            LaunchNote::EffortIgnored {
                found: "--effort".to_string(),
                reason: EffortIgnoreReason::BaseOverride,
            }
            .event_name(),
            "launch.effort.ignored"
        );
        assert_eq!(
            LaunchNote::EffortIgnored {
                found: "ultra".to_string(),
                reason: EffortIgnoreReason::Invalid,
            }
            .event_name(),
            "launch.effort.invalid"
        );
    }

    #[test]
    fn claude_render_adds_model_effort_and_display_name_once() {
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions --team-name 'ledger-team'",
            model: model_spec("claude-opus-4-6", Some("high")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
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

    // Regression: 791f6be replaced the captured renderer parity case with a
    // no-op assertion whose input was already the expected output.
    #[test]
    fn claude_team_context_renders_exact_command() {
        let expected = concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions ",
            "--team-name 'ledger-team' --agent-name 'team-lead' ",
            "--agent-id 'team-lead@ledger-team' --agent-type 'orchestrator' ",
            "-n 'team-lead'"
        );
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: Some(TeamContext {
                team_name: "ledger-team",
                agent_name: "team-lead",
                role: MemberRole::Lead,
            }),
        }
        .render();

        assert_eq!(rendered.command, expected);
    }

    #[test]
    fn claude_render_is_idempotent_when_flags_already_present() {
        let base = concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions ",
            "--team-name 'ledger-team' --agent-name 'team-lead' ",
            "--agent-id 'team-lead@ledger-team' --agent-type 'orchestrator' ",
            "-n 'team-lead'"
        );
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base,
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: Some(TeamContext {
                team_name: "ledger-team",
                agent_name: "team-lead",
                role: MemberRole::Lead,
            }),
        }
        .render();

        assert_eq!(rendered.command, base);
    }

    #[test]
    fn base_is_preserved_verbatim() {
        let base = "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions";
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base,
            model: model_spec("claude-opus-4-6", None),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert!(rendered.command.starts_with(base));
    }

    #[test]
    fn agy_render_adds_model_and_effort() {
        // Regression: commit 4cd067a registered agy with its verified flags but
        // left the retired Google harness renderer dropping effort.
        let rendered = LaunchSpec {
            tool: CliTool::Agy,
            mode: LaunchMode::Fresh,
            base: "agy",
            model: model_spec("gemini-3.1-pro", Some("high")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "agy --model 'gemini-3.1-pro' --effort 'high'"
        );
        assert!(rendered.notes.is_empty());
    }

    #[test]
    fn agy_free_form_base_can_remove_dangerous_permission_bypass() {
        // Regression: commit efcd7d2 force-injected the high-risk flag after
        // Settings, so an interactive agy user could not opt back into prompts.
        let base = "agy --sandbox";
        let rendered = LaunchSpec {
            tool: CliTool::Agy,
            mode: LaunchMode::Fresh,
            base,
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.command, base);
    }

    #[test]
    fn agy_render_rejects_unknown_effort() {
        // Regression: commit 4cd067a had no agy effort renderer, so every
        // requested value was discarded without exercising the vocabulary.
        let rendered = LaunchSpec {
            tool: CliTool::Agy,
            mode: LaunchMode::Fresh,
            base: "agy",
            model: model_spec("gemini-3.7-flash-high", Some("xhigh")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.notes[0].event_name(), "launch.effort.invalid");
    }

    // Regression: 791f6be checked only the third harness's short model flag, so a
    // free-form base using --model received a second model selection.
    #[test]
    fn agy_render_respects_model_flag_and_notes_it() {
        let rendered = LaunchSpec {
            tool: CliTool::Agy,
            mode: LaunchMode::Fresh,
            base: "agy --model gemini-3.1-pro-low",
            model: model_spec("gemini-3.1-pro", None),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "agy --model gemini-3.1-pro-low");
        assert_eq!(
            rendered.notes,
            vec![LaunchNote::ModelIgnored {
                found: "--model".to_string()
            }]
        );
    }

    #[test]
    fn grok_render_adds_model_and_effort() {
        // Regression: commit bfecae9 had no grok launch arm, so a registered
        // model and reasoning effort were dropped from every grok launch.
        let rendered = LaunchSpec {
            tool: CliTool::Grok,
            mode: LaunchMode::Fresh,
            base: "grok --always-approve",
            model: model_spec("grok-4.6", Some("xhigh")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "grok --always-approve --model 'grok-4.6' --effort 'xhigh'"
        );
        assert!(rendered.notes.is_empty());
    }

    #[test]
    fn grok_render_validates_effort_against_the_selected_model() {
        // Regression: commit bfecae9 had no grok catalog, so `xhigh` — which
        // only grok-4.6 accepts — would have been rendered for grok-4.5 and
        // rejected eagerly by the CLI instead of noted here.
        let rendered = LaunchSpec {
            tool: CliTool::Grok,
            mode: LaunchMode::Fresh,
            base: "grok --always-approve",
            model: model_spec("grok-4.5", Some("xhigh")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "grok --always-approve --model 'grok-4.5'");
        assert_eq!(
            rendered.notes,
            vec![LaunchNote::EffortIgnored {
                found: "xhigh".to_string(),
                reason: EffortIgnoreReason::Invalid,
            }]
        );
    }

    #[test]
    fn grok_render_respects_a_base_that_already_selects_model_or_effort() {
        // Regression: commit bfecae9 knew none of grok's short and long flag
        // spellings, so a free-form base using `-m` or `--reasoning-effort`
        // would have received a second, conflicting selection.
        let rendered = LaunchSpec {
            tool: CliTool::Grok,
            mode: LaunchMode::Fresh,
            base: "grok -m grok-4.5 --reasoning-effort low",
            model: model_spec("grok-4.6", Some("high")),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: None,
            selector: None,
            team: None,
        }
        .render();

        assert_eq!(rendered.command, "grok -m grok-4.5 --reasoning-effort low");
        assert_eq!(
            rendered.notes,
            vec![
                LaunchNote::ModelIgnored {
                    found: "-m".to_string(),
                },
                LaunchNote::EffortIgnored {
                    found: "--reasoning-effort".to_string(),
                    reason: EffortIgnoreReason::BaseOverride,
                },
            ]
        );
    }

    // Regression: 791f6be rendered the Claude arm without any notion of a
    // config dir, so every launch ran on whichever subscription `~/.claude`
    // happened to hold.
    #[test]
    fn claude_render_prefixes_the_selected_config_dir() {
        let config_dir = std::path::Path::new("/home/user/.claude-account2");
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: Some(config_dir),
            selector: Some("CLAUDE_CONFIG_DIR"),
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "CLAUDE_CONFIG_DIR='/home/user/.claude-account2' claude --dangerously-skip-permissions"
        );
        assert!(rendered.notes.is_empty());
    }

    // Regression: 791f6be prepends the team environment itself, so a config dir
    // appended after it would land inside the command instead of in front of
    // the assignments the shell has to read first.
    #[test]
    fn claude_render_puts_the_config_dir_in_front_of_the_team_environment() {
        let config_dir = std::path::Path::new("/home/user/.claude-account2");
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: Some(config_dir),
            selector: Some("CLAUDE_CONFIG_DIR"),
            team: Some(TeamContext {
                team_name: "ledger-team",
                agent_name: "team-lead",
                role: MemberRole::Lead,
            }),
        }
        .render();

        assert!(
            rendered.command.starts_with(
                "CLAUDE_CONFIG_DIR='/home/user/.claude-account2' CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 claude"
            ),
            "{}",
            rendered.command
        );
        assert_eq!(count_flag(&rendered.command, "CLAUDE_CONFIG_DIR"), 1);
    }

    #[test]
    fn a_base_that_already_selects_a_config_dir_wins_and_is_noted() {
        let base = "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions";
        let rendered = LaunchSpec {
            tool: CliTool::Claude,
            mode: LaunchMode::Fresh,
            base,
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: Some(std::path::Path::new("/home/user/.claude")),
            selector: Some("CLAUDE_CONFIG_DIR"),
            team: None,
        }
        .render();

        assert_eq!(rendered.command, base);
        assert_eq!(
            rendered.notes,
            vec![LaunchNote::SelectorIgnored {
                found: "CLAUDE_CONFIG_DIR".to_string()
            }]
        );
        assert_eq!(
            LaunchNote::SelectorIgnored {
                found: "CLAUDE_CONFIG_DIR".to_string()
            }
            .event_name(),
            "launch.selector.ignored"
        );
    }

    // Regression: d6839a3 rendered account selection only inside the Claude
    // arm, so a registry-declared selector on another tool was ignored.
    #[test]
    fn an_account_dir_uses_the_selected_tools_registry_selector() {
        let rendered = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex --yolo",
            model: ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: Some(std::path::Path::new("/home/user/.codex-account2")),
            selector: Some("CODEX_HOME"),
            team: None,
        }
        .render();

        assert_eq!(
            rendered.command,
            "CODEX_HOME='/home/user/.codex-account2' codex --yolo"
        );
        assert!(rendered.notes.is_empty());
    }

    // Regression: 791f6be logged the free-form base verbatim, persisting API
    // keys and other secret-like environment assignments in the JSONL sink.
    #[test]
    fn logged_command_redacts_secret_environment_assignments() {
        assert_eq!(
            redact_command_for_logging(
                "OPENAI_API_KEY=sk-secret ACCOUNT_TOKEN='two words' SAFE=value codex --yolo"
            ),
            "OPENAI_API_KEY=[REDACTED] ACCOUNT_TOKEN=[REDACTED] SAFE=value codex --yolo"
        );
    }
}
