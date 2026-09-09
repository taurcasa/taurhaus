//! Codex's owned-host launch slice; no shell or account discovery at execution.

use super::{words, LaunchSpec};
use crate::session_scanner::cli_tool::CliTool;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Runtime evidence pins transport and TUI together; a build bump needs a fresh
/// HTTP Upgrade + initialize probe before changing this allowlist.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HostedDescriptor {
    pub build: String,
    pub transport: String,
    pub attached_tui: String,
    pub transport_probe_required_on_bump: bool,
}
impl HostedDescriptor {
    pub fn codex() -> Self {
        Self {
            build: "0.153.4".into(),
            transport: "unix-websocket".into(),
            attached_tui: "verified on 0.153.4".into(),
            transport_probe_required_on_bump: true,
        }
    }
}

#[derive(Clone)]
pub struct HostedLaunch {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub account_root: PathBuf,
    pub applied_effort: Option<String>,
    pub account: crate::session_scanner::launch_base::LaunchAccountResult,
}

impl LaunchSpec<'_> {
    pub fn render_app_server(&self, session: Option<&str>) -> Result<HostedLaunch, String> {
        if self.tool != CliTool::Codex {
            return Err("this harness has no owned app-server launch capability".into());
        }
        HostedLaunch::from_rendered(
            &self.render().command,
            self.account_dir
                .ok_or("hosted launch requires an explicit account root")?,
            session,
        )
    }
}

impl HostedLaunch {
    pub fn attach_argv(&self, socket: &Path, thread: &str) -> Result<Vec<String>, String> {
        if !socket.is_absolute() || thread.is_empty() || thread.starts_with('-') {
            return Err("attached TUI requires an exact owned thread and absolute socket".into());
        }
        Ok(vec![
            self.program.to_string_lossy().into_owned(),
            "--remote".into(),
            format!("unix://{}", socket.display()),
            "resume".into(),
            thread.into(),
            "--no-alt-screen".into(),
        ])
    }

    pub fn attach_command(&self, argv: &[String]) -> String {
        let environment = self
            .environment
            .iter()
            .map(|(key, value)| super::shell_escape(&format!("{key}={value}")))
            .collect::<Vec<_>>()
            .join(" ");
        let command = argv
            .iter()
            .map(|s| super::shell_escape(s))
            .collect::<Vec<_>>()
            .join(" ");
        format!("env -u TMUX {environment} {command}")
    }

    pub fn supports(tool: CliTool) -> bool {
        tool == CliTool::Codex
    }

    /// Consume the existing resolved/rendered launch, without evaluating shell
    /// syntax. A named resume selects the thread via RPC, never a CLI picker.
    pub fn from_rendered(
        command: &str,
        account_root: &Path,
        session: Option<&str>,
    ) -> Result<Self, String> {
        if !account_root.is_absolute()
            || command.contains(['$', '`', ';', '|', '&', '<', '>', '\n', '\r'])
        {
            return Err(
                "hosted launch requires resolved literal arguments and an absolute account root"
                    .into(),
            );
        }
        let tokens = words(command);
        let mut cursor = 0;
        let mut environment = BTreeMap::new();
        while let Some(word) = tokens.get(cursor) {
            if let Some(name) = word.assignment_name() {
                environment.insert(
                    name.to_string(),
                    word.text.split_once('=').unwrap().1.to_string(),
                );
            } else if word.text != "env" || word.quoted {
                break;
            }
            cursor += 1;
        }
        if environment.get("CODEX_HOME").map(Path::new) != Some(account_root) {
            return Err("hosted account selector is absent or mismatched".into());
        }
        let program = PathBuf::from(&tokens.get(cursor).ok_or("missing executable")?.text);
        if !matches!(
            program.file_name().and_then(|s| s.to_str()),
            Some("codex" | "codex.exe")
        ) {
            return Err("opaque executable cannot establish hosted launch policy".into());
        }
        cursor += 1;
        let mut arguments = Vec::new();
        let mut applied_effort = None;
        while let Some(word) = tokens.get(cursor) {
            cursor += 1;
            if word.text == "resume" {
                let named = tokens
                    .get(cursor)
                    .ok_or("hosted resume requires a named conversation")?;
                if session != Some(named.text.as_str()) || named.text.starts_with('-') {
                    return Err("hosted resume conversation mismatch".into());
                }
                cursor += 1;
                continue;
            }
            if matches!(
                word.text.as_str(),
                "--yolo" | "--dangerously-bypass-approvals-and-sandbox"
            ) {
                arguments.extend(
                    [
                        "-c",
                        "approval_policy=\"never\"",
                        "-c",
                        "sandbox_mode=\"danger-full-access\"",
                    ]
                    .map(str::to_string),
                );
                continue;
            }
            let key = match word.text.as_str() {
                "-c" | "--config" => None,
                "-m" | "--model" => Some("model"),
                "-s" | "--sandbox" => Some("sandbox_mode"),
                "-a" | "--ask-for-approval" => Some("approval_policy"),
                _ => return Err(format!("unsupported hosted argument: {}", word.text)),
            };
            let value = &tokens
                .get(cursor)
                .ok_or("missing hosted launch argument value")?
                .text;
            cursor += 1;
            let config = match key {
                Some(key) => format!("{key}={}", serde_json::json!(value)),
                None => value.clone(),
            };
            if let Some(("model_reasoning_effort", value)) = config.split_once('=') {
                applied_effort = Some(value.trim().trim_matches('"').to_string());
            }
            arguments.extend(["-c".to_string(), config]);
        }
        arguments.push("app-server".into());
        Ok(Self {
            program,
            arguments,
            environment,
            account_root: account_root.into(),
            applied_effort,
            account: crate::session_scanner::launch_base::LaunchAccountResult {
                account_applied: Some(true),
                ..Default::default()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::protocol::LaunchMode;
    use crate::session_scanner::launch::ModelSpec;

    #[test]
    fn hosted_render_preserves_account_model_effort_and_explicit_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let base = "CODEX_HOME=/wrong codex --model gpt-5.6-luna -c 'model_reasoning_effort=\"low\"' --sandbox read-only --ask-for-approval never";
        let launch = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base,
            model: ModelSpec::parse_legacy("gpt-5.6-sol-high"),
            team: None,
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: Some(tmp.path()),
            selector: Some("CODEX_HOME"),
        }
        .render_app_server(None)
        .unwrap();
        assert_eq!(
            launch.environment["CODEX_HOME"],
            tmp.path().to_str().unwrap()
        );
        assert_eq!(launch.applied_effort.as_deref(), Some("low"));
        assert_eq!(
            launch.arguments.join(" "),
            r#"-c model="gpt-5.6-luna" -c model_reasoning_effort="low" -c sandbox_mode="read-only" -c approval_policy="never" app-server"#
        );
    }

    #[test]
    fn hosted_render_refuses_opaque_shell_missing_account_and_unnamed_resume() {
        let tmp = tempfile::tempdir().unwrap();
        // Regression: 76fa63c4 hid unsupported flags behind an opaque refusal.
        let hook = "--dangerously-bypass-hook-trust";
        for (command, reason) in [
            ("wrapper codex", "opaque executable"),
            ("codex && echo x", "resolved literal arguments"),
            ("codex resume --last", "conversation mismatch"),
            ("codex --unknown", "--unknown"),
            ("codex --dangerously-bypass-hook-trust", hook),
        ] {
            let command = format!("CODEX_HOME='{}' {command}", tmp.path().display());
            let error = HostedLaunch::from_rendered(&command, tmp.path(), None)
                .err()
                .unwrap();
            assert!(error.contains(reason), "{error}");
        }
        for root in [tmp.path(), Path::new("relative")] {
            assert!(HostedLaunch::from_rendered("codex", root, None).is_err());
        }
        let command = format!(
            "CODEX_HOME='{}' codex resume owned-id --yolo",
            tmp.path().display()
        );
        let launch = HostedLaunch::from_rendered(&command, tmp.path(), Some("owned-id")).unwrap();
        assert_eq!(
            launch.arguments.join(" "),
            r#"-c approval_policy="never" -c sandbox_mode="danger-full-access" app-server"#
        );
        assert!(HostedLaunch::from_rendered(&command, tmp.path(), Some("another-id")).is_err());
    }
}
