//! Codex's owned-host launch slice; no shell or account discovery at execution.

use super::{words, LaunchSpec};
use crate::session_scanner::cli_tool::CliTool;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Runtime evidence verified the remote-resume primitive on `build`, which is the
/// MINIMUM admitted Codex build: harness version gates are floors, never exact
/// pins (operator ruling 2026-09-13 — harnesses ship too fast for pins). A newer
/// build is admitted; the live HTTP Upgrade + initialize handshake still refuses a
/// changed transport at launch, and an older or unparseable build is refused.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedDescriptor {
    pub build: String,
    pub transport: String,
    pub attached_tui: String,
}
impl HostedDescriptor {
    pub const HOST: &str = "taurhaus-daemon-owned-thread/1";
    pub const CONFIGURATION: &str = "strict-config/1";
    pub const TRUST: &str = "daemon-owned/1";

    pub fn codex() -> Self {
        Self {
            build: "0.153.4".into(),
            transport: "unix-websocket".into(),
            attached_tui: "verified on 0.153.4 (minimum)".into(),
        }
    }

    /// `installed` satisfies the floor when both are dotted numeric versions and
    /// `installed >= minimum`. Empty, placeholder or pre-release strings never do.
    pub fn build_satisfies(installed: &str, minimum: &str) -> bool {
        fn parse(version: &str) -> Option<Vec<u64>> {
            let parts: Vec<u64> = version
                .split('.')
                .map(|part| {
                    (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                        .then(|| part.parse().ok())
                        .flatten()
                })
                .collect::<Option<_>>()?;
            (!parts.is_empty()).then_some(parts)
        }
        match (parse(installed), parse(minimum)) {
            (Some(mut installed), Some(mut minimum)) => {
                let width = installed.len().max(minimum.len());
                installed.resize(width, 0);
                minimum.resize(width, 0);
                installed >= minimum
            }
            _ => false,
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
            &LaunchSpec {
                codex_notify_executable: None,
                ..self.clone()
            }
            .render()
            .command,
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
            "--strict-config".into(),
        ])
    }

    pub fn attach_command(&self, argv: &[String], home: &Path) -> String {
        let environment = self
            .environment
            .iter()
            .map(|(key, value)| {
                let value = if key == "CODEX_HOME" {
                    home.to_string_lossy().into_owned()
                } else {
                    value.clone()
                };
                super::shell_escape(&format!("{key}={value}"))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let command = argv
            .iter()
            .map(|s| super::shell_escape(s))
            .collect::<Vec<_>>()
            .join(" ");
        format!("env -u TMUX {environment} {command}")
    }

    /// Only auth is shared with the selected account. Never import its config,
    /// instructions, hooks, or skills into the attached client.
    #[cfg(target_os = "linux")]
    pub fn prepare_attach_home(
        &self,
        home: &Path,
        config: &serde_json::Value,
    ) -> Result<(), String> {
        use std::os::unix::fs::{symlink, DirBuilderExt};
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(home)
            .map_err(|e| e.to_string())?;
        let config = toml::to_string(config).map_err(|e| e.to_string())?;
        std::fs::write(home.join("config.toml"), config).map_err(|e| e.to_string())?;
        // A link keeps account rotation visible without copying or logging credentials.
        symlink(self.account_root.join("auth.json"), home.join("auth.json"))
            .map_err(|e| e.to_string())
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
    fn hosted_render_suppresses_managed_notify() {
        // Regression: 6f61f611 rendered the ordinary notify edge into hosted launches.
        let tmp = tempfile::tempdir().unwrap();
        let daemon = tmp.path().join("fake-daemon");
        let spec = LaunchSpec {
            tool: CliTool::Codex,
            mode: LaunchMode::Fresh,
            base: "codex",
            model: ModelSpec::default(),
            team: None,
            codex_bypass_hook_trust: false,
            codex_notify_executable: Some(&daemon),
            account_dir: Some(tmp.path()),
            selector: Some("CODEX_HOME"),
        };
        assert!(spec.render().command.contains("notify="));
        let hosted = spec.render_app_server(None).unwrap();
        assert!(!hosted.arguments.iter().any(|arg| arg.contains("notify=")));
    }

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

#[cfg(test)]
mod floor_tests {
    use super::HostedDescriptor;

    // Regression: dogfood finding 6 (2026-09-13) — the verified Codex build is a floor.
    #[test]
    fn build_satisfies_is_a_floor_over_dotted_numeric_versions() {
        for (installed, minimum, expected) in [
            ("0.153.4", "0.153.4", true),
            ("0.154.0", "0.153.4", true),
            ("0.153.5", "0.153.4", true),
            ("1.0", "0.153.4", true),
            ("0.153.3", "0.153.4", false),
            ("0.154.0-alpha.1", "0.153.4", false),
            ("", "0.153.4", false),
            ("wrong-build", "0.153.4", false),
            ("0.154.0", "*", false),
        ] {
            assert_eq!(
                HostedDescriptor::build_satisfies(installed, minimum),
                expected,
                "{installed} vs {minimum}"
            );
        }
    }
}
