//! Mesh-bridged backend adapter.

use std::path::{Path, PathBuf};
#[cfg(feature = "mesh-bridged-backend")]
use std::process::Command;

#[cfg(feature = "mesh-bridged-backend")]
use std::sync::Arc;

use super::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::errors::CoordinationError;
#[cfg(feature = "mesh-bridged-backend")]
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::coordination::requests::{
    DeliveryMethod, DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult,
    OperatorNoticeDelivery, ProbeRequest, ProbeResult, TeardownRequest, TeardownResult,
};
#[cfg(feature = "mesh-bridged-backend")]
use crate::coordination::runtime::apply_background_command_settings;
use crate::coordination::stores::{MeshInboxMessage, MeshInboxStore};
use crate::session_scanner::cli_tool::CliTool;
use chrono::Utc;

#[cfg(not(feature = "mesh-bridged-backend"))]
const FEATURE_NAME: &str = "mesh-bridged-backend";
const NOTICE_SUMMARY: &str = "operator_notice";
pub const MESH_MISSING_ERROR: &str =
    "Mesh CLI not found. Install it to enable multi-agent collaboration.";
pub const TMUX_MISSING_ERROR: &str = "tmux is required for multi-agent sessions.";

/// Input shape for one agent's preflight tool requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightAgent {
    pub agent_name: String,
    pub cli_tool: String,
}

/// Warning emitted for one agent when its CLI binary is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPreflightWarning {
    pub agent_name: String,
    pub cli_tool: String,
    pub message: String,
}

/// Environment preflight report used before initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub blocking_errors: Vec<String>,
    pub agent_warnings: Vec<AgentPreflightWarning>,
}

impl PreflightReport {
    /// Returns true when initialization is safe to proceed.
    pub fn can_initialize(&self) -> bool {
        self.blocking_errors.is_empty()
    }
}

/// Baseline mesh feature availability used by UI gating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityReport {
    pub mesh_available: bool,
    pub tmux_available: bool,
    pub blocking_errors: Vec<String>,
}

impl AvailabilityReport {
    pub fn can_initialize(&self) -> bool {
        self.blocking_errors.is_empty()
    }
}

/// Abstraction for binary availability checks.
pub trait BinaryLookup: Send + Sync {
    fn is_available(&self, binary_name: &str) -> bool;
}

/// Build a command invocation to check whether a binary exists.
///
/// For `mesh`: checks the known install path directly (`test -x`).
/// For everything else: uses `which` (system PATH discovery).
#[cfg(feature = "mesh-bridged-backend")]
fn binary_lookup_invocation(binary_name: &str) -> CommandInvocation {
    // Mesh uses known-path check — same pattern as the daemon.
    if binary_name == "mesh" {
        if let Some(mesh_path) = mesh_cli::mesh_binary_path() {
            return if cfg!(target_os = "windows") {
                CommandInvocation {
                    program: "wsl".into(),
                    args: mesh_cli::wrap_wsl_args_for_coordination(
                        vec!["-e".into(), "test".into(), "-x".into(), mesh_path],
                        None,
                    ),
                }
            } else {
                CommandInvocation {
                    program: "test".into(),
                    args: vec!["-x".into(), mesh_path],
                }
            };
        }
    }

    // Fallback: PATH-based lookup for tmux and any registered harness binary
    // (claude, codex, agy, grok).
    if cfg!(target_os = "windows") {
        CommandInvocation {
            program: "wsl".into(),
            args: mesh_cli::wrap_wsl_args_for_coordination(
                vec!["-e".into(), "which".into(), binary_name.into()],
                None,
            ),
        }
    } else {
        CommandInvocation {
            program: "which".into(),
            args: vec![binary_name.into()],
        }
    }
}

/// Build a command invocation to run the mesh CLI.
///
/// Uses the known install path (`~/.local/bin/mesh`) rather than relying
/// on PATH discovery — matches the daemon execution pattern.
#[cfg(feature = "mesh-bridged-backend")]
fn mesh_command_invocation(args: &[&str], teams_dir: &Path) -> CommandInvocation {
    let team_name = command_flag_value(args, "--team");
    let member_name = command_flag_value(args, "--name");
    match (team_name, member_name) {
        (Some(team_name), Some(member_name)) => {
            crate::coordination::runtime::mesh_command_invocation_for_member_at(
                args,
                team_name,
                member_name,
                teams_dir,
            )
        }
        _ => mesh_cli::mesh_command_invocation(args),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn command_flag_value<'a>(args: &'a [&'a str], flag: &str) -> Option<&'a str> {
    args.windows(2).find_map(|window| {
        (window[0] == flag)
            .then_some(window[1])
            .filter(|value| !value.trim().is_empty())
    })
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_system_command(invocation: &CommandInvocation) -> std::io::Result<std::process::Output> {
    if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .output()
    } else {
        let mut cmd = Command::new(&invocation.program);
        apply_background_command_settings(&mut cmd);
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .output()
    }
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Default)]
struct SystemBinaryLookup;

#[cfg(feature = "mesh-bridged-backend")]
impl BinaryLookup for SystemBinaryLookup {
    fn is_available(&self, binary_name: &str) -> bool {
        let invocation = binary_lookup_invocation(binary_name);
        run_system_command(&invocation)
            .map(|out| out.status.success())
            .unwrap_or(false)
    }
}

#[cfg(not(feature = "mesh-bridged-backend"))]
#[derive(Debug, Default)]
struct SystemBinaryLookup;

#[cfg(not(feature = "mesh-bridged-backend"))]
impl BinaryLookup for SystemBinaryLookup {
    fn is_available(&self, _binary_name: &str) -> bool {
        false
    }
}

/// Run environment preflight checks using system binary lookup.
pub fn preflight_check(agents: &[PreflightAgent]) -> PreflightReport {
    preflight_check_with_lookup(agents, &SystemBinaryLookup)
}

/// Run baseline mesh/tmux availability checks for frontend gating.
pub fn availability_check() -> AvailabilityReport {
    availability_check_with_lookup(&SystemBinaryLookup)
}

/// Run baseline mesh/tmux availability checks with an injected lookup.
pub fn availability_check_with_lookup<L: BinaryLookup + ?Sized>(lookup: &L) -> AvailabilityReport {
    let mesh_available = lookup.is_available("mesh");
    let tmux_available = lookup.is_available("tmux");
    let mut blocking_errors = Vec::new();

    if !mesh_available {
        blocking_errors.push(MESH_MISSING_ERROR.to_string());
    }
    if !tmux_available {
        blocking_errors.push(TMUX_MISSING_ERROR.to_string());
    }

    AvailabilityReport {
        mesh_available,
        tmux_available,
        blocking_errors,
    }
}

/// Run environment preflight checks using an injected lookup (test-friendly).
pub fn preflight_check_with_lookup<L: BinaryLookup + ?Sized>(
    agents: &[PreflightAgent],
    lookup: &L,
) -> PreflightReport {
    let blocking_errors = availability_check_with_lookup(lookup).blocking_errors;
    let mut agent_warnings = Vec::new();

    for agent in agents {
        let normalized_tool = agent.cli_tool.trim().to_ascii_lowercase();
        let required_binary = required_binary_for_cli_tool(normalized_tool.as_str());
        match required_binary {
            Some(binary) => {
                if !lookup.is_available(binary) {
                    agent_warnings.push(AgentPreflightWarning {
                        agent_name: agent.agent_name.clone(),
                        cli_tool: agent.cli_tool.clone(),
                        message: format!(
                            "{} CLI not found for agent '{}'. Install it or choose a different tool.",
                            cli_tool_label(binary),
                            agent.agent_name
                        ),
                    });
                }
            }
            None => {
                agent_warnings.push(AgentPreflightWarning {
                    agent_name: agent.agent_name.clone(),
                    cli_tool: agent.cli_tool.clone(),
                    message: format!(
                        "Unsupported CLI tool '{}' for agent '{}'. Choose {}.",
                        agent.cli_tool,
                        agent.agent_name,
                        supported_cli_tool_list()
                    ),
                });
            }
        }
    }

    PreflightReport {
        blocking_errors,
        agent_warnings,
    }
}

fn required_binary_for_cli_tool(cli_tool: &str) -> Option<&'static str> {
    let parsed = CliTool::from_alias(cli_tool).ok()?;
    Some(crate::session_scanner::cli_tool::spec(parsed).name)
}

/// The tools a member may name, read from the registry so a new harness
/// reaches this message without another edit: "claude, codex, agy, or grok".
fn supported_cli_tool_list() -> String {
    let names: Vec<&'static str> = crate::session_scanner::cli_tool::all()
        .iter()
        .map(|spec| spec.name)
        .collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_string(),
        Some((last, head)) => format!("{}, or {last}", head.join(", ")),
    }
}

fn cli_tool_label(binary_name: &str) -> &'static str {
    CliTool::from_alias(binary_name)
        .ok()
        .map(|tool| crate::session_scanner::cli_tool::spec(tool).label)
        .unwrap_or("CLI tool")
}

/// Minimal command output abstraction used by the mesh runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshCommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Executes mesh CLI commands.
pub trait MeshCommandRunner: Send + Sync {
    fn run(&self, args: &[&str], teams_dir: &Path) -> Result<MeshCommandOutput, CoordinationError>;
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Default)]
struct SystemMeshCommandRunner;

#[cfg(feature = "mesh-bridged-backend")]
impl MeshCommandRunner for SystemMeshCommandRunner {
    fn run(&self, args: &[&str], teams_dir: &Path) -> Result<MeshCommandOutput, CoordinationError> {
        let invocation = mesh_command_invocation(args, teams_dir);
        let output = run_system_command(&invocation).map_err(CoordinationError::Io)?;

        Ok(MeshCommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Mesh-bridged backend implementation.
pub struct MeshBridgedBackend {
    #[cfg(feature = "mesh-bridged-backend")]
    runner: Arc<dyn MeshCommandRunner>,
    teams_dir: PathBuf,
}

impl Default for MeshBridgedBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MeshBridgedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshBridgedBackend")
            .field("teams_dir", &self.teams_dir)
            .finish()
    }
}

impl MeshBridgedBackend {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "mesh-bridged-backend")]
            runner: Arc::new(SystemMeshCommandRunner),
            teams_dir: crate::provider::platform_paths::PlatformPaths::teams_dir(),
        }
    }

    pub fn new_with_teams_dir(teams_dir: PathBuf) -> Self {
        Self {
            #[cfg(feature = "mesh-bridged-backend")]
            runner: Arc::new(SystemMeshCommandRunner),
            teams_dir,
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    pub fn with_runner(runner: Arc<dyn MeshCommandRunner>) -> Self {
        Self {
            runner,
            teams_dir: crate::provider::platform_paths::PlatformPaths::teams_dir(),
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    pub fn with_runner_and_teams_dir(
        runner: Arc<dyn MeshCommandRunner>,
        teams_dir: PathBuf,
    ) -> Self {
        Self { runner, teams_dir }
    }

    #[cfg(not(feature = "mesh-bridged-backend"))]
    fn mesh_disabled_error() -> CoordinationError {
        CoordinationError::Backend(format!(
            "MeshBridged backend is disabled; enable Cargo feature '{FEATURE_NAME}'"
        ))
    }

    #[cfg(feature = "mesh-bridged-backend")]
    fn run_mesh(&self, args: &[&str]) -> Result<MeshCommandOutput, CoordinationError> {
        let mut rooted_args = args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        if let Some(claude_dir) = self.teams_dir.parent() {
            rooted_args.push("--claude-dir".to_string());
            rooted_args
                .push(crate::coordination::runtime::mesh_cli_claude_dir_arg_from_path(claude_dir));
        }
        let rooted_arg_refs = rooted_args.iter().map(String::as_str).collect::<Vec<_>>();
        self.runner.run(&rooted_arg_refs, &self.teams_dir)
    }

    #[cfg(feature = "mesh-bridged-backend")]
    fn join_team(&self, team_name: &str, member_name: &str) -> Result<(), CoordinationError> {
        let out = self.run_mesh(&["join", "--team", team_name, "--name", member_name])?;
        if out.success {
            Ok(())
        } else {
            Err(CoordinationError::Backend(format!(
                "mesh join failed for '{member_name}' in '{team_name}': {}",
                out.stderr
            )))
        }
    }

    #[cfg(not(feature = "mesh-bridged-backend"))]
    fn join_team(&self, _team_name: &str, _member_name: &str) -> Result<(), CoordinationError> {
        Err(Self::mesh_disabled_error())
    }

    #[cfg(feature = "mesh-bridged-backend")]
    fn leave_team(&self, team_name: &str, member_name: &str) -> Result<(), CoordinationError> {
        let out = self.run_mesh(&["leave", "--team", team_name, "--name", member_name])?;
        if out.success {
            Ok(())
        } else {
            Err(CoordinationError::Backend(format!(
                "mesh leave failed for '{member_name}' in '{team_name}': {}",
                out.stderr
            )))
        }
    }

    #[cfg(not(feature = "mesh-bridged-backend"))]
    fn leave_team(&self, _team_name: &str, _member_name: &str) -> Result<(), CoordinationError> {
        Err(Self::mesh_disabled_error())
    }

    fn send_operator_notice(
        &self,
        payload: OperatorNoticeDelivery,
    ) -> Result<DeliveryResult, CoordinationError> {
        let message = MeshInboxMessage::operator_originated(
            &payload.member_name,
            payload.message,
            Some(NOTICE_SUMMARY.to_string()),
            Utc::now(),
            payload.sender_name.as_deref(),
        );
        MeshInboxStore::append(
            &self.teams_dir,
            &payload.team_name,
            &payload.member_name,
            &message,
        )?;
        Ok(DeliveryResult {
            delivered: true,
            method: DeliveryMethod::InboxFile,
            durable: true,
            wake: crate::coordination::requests::WakeDisposition::NotAttempted {
                reason: "wake not evaluated by backend".to_string(),
            },
            post_write_warnings: Vec::new(),
        })
    }
}

impl CoordinationBackend for MeshBridgedBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::MeshBridged
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::mesh_bridged()
    }

    fn launch(&self, req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        self.join_team(&req.team_name, &req.member.name)?;
        Ok(LaunchResult {
            pane_id: req
                .pane_target
                .unwrap_or_else(|| "mesh-bridged".to_string()),
            process_id: None,
        })
    }

    fn deliver(&self, req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        match req {
            DeliveryRequest::OperatorNotice(payload) => self.send_operator_notice(*payload),
            other => Err(CoordinationError::Validation(format!(
                "MeshBridged backend in C1 only supports operator_notice delivery, got: {other:?}"
            ))),
        }
    }

    fn probe(&self, req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        #[cfg(feature = "mesh-bridged-backend")]
        {
            let out = self.run_mesh(&[
                "read",
                "--unread",
                "--team",
                &req.team_name,
                "--name",
                &req.member_name,
            ])?;

            Ok(ProbeResult {
                alive: out.success,
                health: if out.success {
                    crate::coordination::domain::HealthState::Healthy
                } else {
                    crate::coordination::domain::HealthState::SessionDead
                },
                evidence: if out.success {
                    crate::coordination::requests::ProbeEvidence::WeakIo
                } else {
                    crate::coordination::requests::ProbeEvidence::None
                },
            })
        }

        #[cfg(not(feature = "mesh-bridged-backend"))]
        {
            let _ = req;
            Err(Self::mesh_disabled_error())
        }
    }

    fn teardown(&self, req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        self.leave_team(&req.team_name, &req.member_name)?;
        Ok(TeardownResult { success: true })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::collections::VecDeque;
    use std::path::PathBuf;
    #[cfg(feature = "mesh-bridged-backend")]
    use std::sync::Arc;
    use std::sync::Mutex;
    use tempfile::TempDir;

    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::requests::{
        DeliveryRequest, LaunchPermissions, OperatorNoticeDelivery, TeardownMode,
    };
    use crate::session_scanner::cli_tool::CliTool;

    use super::*;

    #[derive(Debug, Default)]
    struct MockBinaryLookup {
        available: HashSet<String>,
    }

    impl MockBinaryLookup {
        fn with_available(names: &[&str]) -> Self {
            Self {
                available: names.iter().map(|name| (*name).to_string()).collect(),
            }
        }
    }

    impl BinaryLookup for MockBinaryLookup {
        fn is_available(&self, binary_name: &str) -> bool {
            self.available.contains(binary_name)
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[derive(Debug, Default)]
    struct MockRunner {
        calls: Mutex<Vec<Vec<String>>>,
        outcomes: Mutex<VecDeque<MeshCommandOutput>>,
    }

    #[cfg(feature = "mesh-bridged-backend")]
    impl MockRunner {
        fn with_outcomes(outcomes: Vec<MeshCommandOutput>) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                outcomes: Mutex::new(VecDeque::from(outcomes)),
            })
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().expect("calls mutex poisoned").clone()
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    impl MeshCommandRunner for MockRunner {
        fn run(
            &self,
            args: &[&str],
            _teams_dir: &Path,
        ) -> Result<MeshCommandOutput, CoordinationError> {
            self.calls
                .lock()
                .expect("calls mutex poisoned")
                .push(args.iter().map(|arg| (*arg).to_string()).collect());

            Ok(self
                .outcomes
                .lock()
                .expect("outcomes mutex poisoned")
                .pop_front()
                .unwrap_or(MeshCommandOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                }))
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn parse_wsl_unix_path_from_stdout_handles_clean_output() {
        let stdout = b"/home/user\n";
        assert_eq!(
            mesh_cli::parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/user".to_string())
        );
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn parse_wsl_unix_path_from_stdout_ignores_banner_noise() {
        let stdout =
            b"Welcome to Ubuntu 22.04.5 LTS\nThis message is shown once a day.\n/home/user\n";
        assert_eq!(
            mesh_cli::parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/user".to_string())
        );
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn parse_wsl_unix_path_from_stdout_returns_none_without_path() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nNo path here\n";
        assert_eq!(mesh_cli::parse_wsl_unix_path_from_stdout(stdout), None);
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn binary_lookup_mesh_uses_known_path() {
        let invocation = binary_lookup_invocation("mesh");
        let expected_path = dirs::home_dir()
            .unwrap()
            .join(".local/bin/mesh")
            .to_string_lossy()
            .to_string();

        if cfg!(target_os = "windows") {
            assert_eq!(invocation.program, "wsl");
            assert!(invocation.args.contains(&"test".to_string()));
            assert!(invocation.args.contains(&"-x".to_string()));
        } else {
            // Known-path: `test -x ~/.local/bin/mesh`
            assert_eq!(invocation.program, "test");
            assert_eq!(invocation.args, vec!["-x", &expected_path]);
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn binary_lookup_other_uses_which() {
        let invocation = binary_lookup_invocation("tmux");

        if cfg!(target_os = "windows") {
            assert_eq!(invocation.program, "wsl");
            assert_eq!(invocation.args, vec!["-e", "which", "tmux"]);
        } else {
            assert_eq!(invocation.program, "which");
            assert_eq!(invocation.args, vec!["tmux"]);
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn mesh_command_invocation_uses_known_path() {
        let invocation = mesh_command_invocation(&["read", "--unread"], Path::new("/tmp/teams"));
        let expected_path = dirs::home_dir()
            .unwrap()
            .join(".local/bin/mesh")
            .to_string_lossy()
            .to_string();

        if cfg!(target_os = "windows") {
            assert_eq!(invocation.program, "wsl");
            assert_eq!(invocation.args[0], "-e");
            // Second arg should be the full mesh path, not just "mesh"
            assert!(invocation.args[1].ends_with("/.local/bin/mesh"));
            assert_eq!(invocation.args[2], "read");
            assert_eq!(invocation.args[3], "--unread");
        } else {
            assert_eq!(invocation.program, expected_path);
            assert_eq!(invocation.args, vec!["read", "--unread"]);
        }
    }

    fn sample_member(name: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            model: None,
            reasoning_effort: None,
            account_id: None,
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn launch_invokes_mesh_join() {
        let temp = TempDir::new().expect("tempdir");
        let teams_dir = temp.path().join("work").join("teams");
        let runner = MockRunner::with_outcomes(vec![MeshCommandOutput {
            success: true,
            stdout: "joined".to_string(),
            stderr: String::new(),
        }]);
        let backend =
            MeshBridgedBackend::with_runner_and_teams_dir(runner.clone(), teams_dir.clone());

        let result = backend
            .launch(LaunchRequest {
                member: sample_member("codex-reviewer"),
                team_name: "architecture-final".to_string(),
                pane_target: Some("%18".to_string()),
                permissions: LaunchPermissions::Standard,
            })
            .expect("launch should succeed");

        assert_eq!(result.pane_id, "%18");
        assert_eq!(result.process_id, None);
        assert_eq!(
            runner.calls(),
            vec![vec![
                "join".to_string(),
                "--team".to_string(),
                "architecture-final".to_string(),
                "--name".to_string(),
                "codex-reviewer".to_string(),
                "--claude-dir".to_string(),
                teams_dir
                    .parent()
                    .expect("Claude dir")
                    .to_string_lossy()
                    .to_string()
            ]]
        );
    }

    #[test]
    fn operator_notice_appends_directly_and_reports_inbox_file() {
        // Regression: the operator path reported tmux injection even though
        // durable delivery is the recipient's inbox file.
        let tmp = TempDir::new().expect("tempdir");
        let backend = MeshBridgedBackend::new_with_teams_dir(tmp.path().to_path_buf());

        let result = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: "codex-reviewer".to_string(),
                team_name: "architecture-final".to_string(),
                message: "check in".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect("delivery");

        assert_eq!(result.method, DeliveryMethod::InboxFile);
        let inbox = MeshInboxStore::load(tmp.path(), "architecture-final", "codex-reviewer")
            .expect("inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].text, "check in");
        assert_eq!(inbox[0].summary.as_deref(), Some(NOTICE_SUMMARY));
    }

    #[test]
    fn operator_notice_never_uses_recipient_as_sender() {
        // Regression: mesh-findings P1 delivery audit; the sender fallback
        // chain ended in a self-send, forging recipient activity and identity.
        let tmp = TempDir::new().expect("tempdir");
        let backend = MeshBridgedBackend::new_with_teams_dir(tmp.path().to_path_buf());

        backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: "codex-reviewer".to_string(),
                team_name: "architecture-final".to_string(),
                message: "check in".to_string(),
                sender_name: Some("codex-reviewer".to_string()),
                operational_context: None,
            }))
            .expect("delivery");

        let inbox = MeshInboxStore::load(tmp.path(), "architecture-final", "codex-reviewer")
            .expect("inbox");
        assert_eq!(
            inbox[0].from,
            crate::coordination::stores::OPERATOR_SENDER_NAME
        );
        assert_ne!(inbox[0].from, "codex-reviewer");
    }

    #[test]
    fn fake_backend_satisfies_trait_contract() {
        fn exercise_backend<B: CoordinationBackend>(backend: &B) {
            let launch = backend.launch(LaunchRequest {
                member: sample_member("fake-agent"),
                team_name: "architecture-final".to_string(),
                pane_target: None,
                permissions: LaunchPermissions::Standard,
            });
            assert!(launch.is_ok());

            let delivered =
                backend.deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                    member_name: "fake-agent".to_string(),
                    team_name: "architecture-final".to_string(),
                    message: "hello".to_string(),
                    sender_name: None,
                    operational_context: None,
                }));
            assert!(delivered.is_ok());

            let probe = backend.probe(ProbeRequest {
                member_name: "fake-agent".to_string(),
                team_name: "architecture-final".to_string(),
            });
            assert!(probe.is_ok());

            let teardown = backend.teardown(TeardownRequest {
                member_name: "fake-agent".to_string(),
                team_name: "architecture-final".to_string(),
                mode: TeardownMode::Graceful,
            });
            assert!(teardown.is_ok());
        }

        let fake = FakeBackend::default();
        exercise_backend(&fake);
        assert_eq!(fake.call_counts(), (1, 1, 1, 1));
    }

    #[test]
    fn preflight_all_tools_present_returns_clean_report() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex"]);
        let report = preflight_check_with_lookup(
            &[
                PreflightAgent {
                    agent_name: "team-lead".to_string(),
                    cli_tool: "claude".to_string(),
                },
                PreflightAgent {
                    agent_name: "frontend-dev".to_string(),
                    cli_tool: "codex".to_string(),
                },
            ],
            &lookup,
        );

        assert!(report.blocking_errors.is_empty());
        assert!(report.agent_warnings.is_empty());
        assert!(report.can_initialize());
    }

    #[test]
    fn preflight_mesh_missing_returns_blocking_error() {
        let lookup = MockBinaryLookup::with_available(&["tmux", "codex"]);
        let report = preflight_check_with_lookup(&[], &lookup);
        assert!(report
            .blocking_errors
            .contains(&MESH_MISSING_ERROR.to_string()));
        assert!(!report.can_initialize());
    }

    #[test]
    fn preflight_tmux_missing_returns_blocking_error() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "codex"]);
        let report = preflight_check_with_lookup(&[], &lookup);
        assert!(report
            .blocking_errors
            .contains(&TMUX_MISSING_ERROR.to_string()));
        assert!(!report.can_initialize());
    }

    #[test]
    fn availability_report_reflects_required_binaries() {
        let present = MockBinaryLookup::with_available(&["mesh", "tmux"]);
        let available = availability_check_with_lookup(&present);
        assert!(available.mesh_available);
        assert!(available.tmux_available);
        assert!(available.can_initialize());
        assert!(available.blocking_errors.is_empty());

        let missing = MockBinaryLookup::with_available(&["tmux"]);
        let unavailable = availability_check_with_lookup(&missing);
        assert!(!unavailable.mesh_available);
        assert!(unavailable.tmux_available);
        assert!(!unavailable.can_initialize());
        assert_eq!(
            unavailable.blocking_errors,
            vec![MESH_MISSING_ERROR.to_string()]
        );
    }

    #[test]
    fn preflight_agent_tool_missing_returns_warning() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude"]);
        let report = preflight_check_with_lookup(
            &[PreflightAgent {
                agent_name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
            }],
            &lookup,
        );

        assert!(report.blocking_errors.is_empty());
        assert_eq!(report.agent_warnings.len(), 1);
        assert_eq!(report.agent_warnings[0].agent_name, "frontend-dev");
        assert_eq!(report.agent_warnings[0].cli_tool, "codex");
        assert!(report.agent_warnings[0]
            .message
            .contains("Codex CLI not found"));
        assert!(report.can_initialize());
    }

    #[test]
    fn preflight_unknown_tool_returns_warning() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex"]);
        let report = preflight_check_with_lookup(
            &[PreflightAgent {
                agent_name: "qa".to_string(),
                cli_tool: "unknown-tool".to_string(),
            }],
            &lookup,
        );
        assert!(report.blocking_errors.is_empty());
        assert_eq!(report.agent_warnings.len(), 1);
        assert_eq!(report.agent_warnings[0].agent_name, "qa");
        assert!(report.agent_warnings[0]
            .message
            .contains("Unsupported CLI tool"));
    }

    // Regression: the guidance was hardcoded to "claude, codex, or agy" while
    // `required_binary_for_cli_tool` already accepted `grok`, so the error told
    // users to pick a tool that excluded a supported harness. The list is now
    // generated from the registry, so every registered harness is named.
    #[test]
    fn preflight_unknown_tool_names_every_registered_harness() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);
        let report = preflight_check_with_lookup(
            &[PreflightAgent {
                agent_name: "qa".to_string(),
                cli_tool: "unknown-tool".to_string(),
            }],
            &lookup,
        );
        let message = &report.agent_warnings[0].message;
        for spec in crate::session_scanner::cli_tool::all() {
            assert!(
                message.contains(spec.name),
                "unsupported-tool guidance must name {}: {message}",
                spec.name
            );
        }
    }

    #[test]
    fn preflight_multiple_issues_are_all_reported() {
        let lookup = MockBinaryLookup::with_available(&["tmux"]);
        let report = preflight_check_with_lookup(
            &[
                PreflightAgent {
                    agent_name: "team-lead".to_string(),
                    cli_tool: "claude".to_string(),
                },
                PreflightAgent {
                    agent_name: "frontend-dev".to_string(),
                    cli_tool: "codex".to_string(),
                },
            ],
            &lookup,
        );

        assert_eq!(report.blocking_errors.len(), 1);
        assert_eq!(report.blocking_errors[0], MESH_MISSING_ERROR);
        assert_eq!(report.agent_warnings.len(), 2);
        assert!(report
            .agent_warnings
            .iter()
            .any(|w| w.agent_name == "team-lead" && w.message.contains("Claude CLI not found")));
        assert!(report
            .agent_warnings
            .iter()
            .any(|w| w.agent_name == "frontend-dev" && w.message.contains("Codex CLI not found")));
        assert!(!report.can_initialize());
    }
}
