//! Mesh-bridged backend adapter.

#[cfg(feature = "mesh-bridged-backend")]
use std::process::Command;

#[cfg(feature = "mesh-bridged-backend")]
use std::sync::Arc;

use super::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult, OperatorNoticeDelivery,
    ProbeRequest, ProbeResult, TeardownRequest, TeardownResult,
};

#[cfg(not(feature = "mesh-bridged-backend"))]
const FEATURE_NAME: &str = "mesh-bridged-backend";
const COORDINATOR_AGENT_NAME: &str = "taurhaus-orchestrator";
const FALLBACK_OPERATOR_NAME: &str = "team-lead";
#[cfg(feature = "mesh-bridged-backend")]
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

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Default)]
struct SystemBinaryLookup;

#[cfg(feature = "mesh-bridged-backend")]
impl BinaryLookup for SystemBinaryLookup {
    fn is_available(&self, binary_name: &str) -> bool {
        Command::new("which")
            .arg(binary_name)
            .output()
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
        let (binary_name, cli_label) = match agent.cli_tool.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude_native" => ("claude", "Claude"),
            "codex" | "mesh" | "mesh_bridged" => ("codex", "Codex"),
            "gemini" => ("gemini", "Gemini"),
            _ => continue,
        };
        if !lookup.is_available(binary_name) {
            agent_warnings.push(AgentPreflightWarning {
                agent_name: agent.agent_name.clone(),
                cli_tool: binary_name.to_string(),
                message: format!(
                    "{cli_label} CLI not found - agent '{}' cannot be launched.",
                    agent.agent_name
                ),
            });
        }
    }

    PreflightReport {
        blocking_errors,
        agent_warnings,
    }
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
    fn run(&self, args: &[&str]) -> Result<MeshCommandOutput, CoordinationError>;
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Default)]
struct SystemMeshCommandRunner;

#[cfg(feature = "mesh-bridged-backend")]
impl MeshCommandRunner for SystemMeshCommandRunner {
    fn run(&self, args: &[&str]) -> Result<MeshCommandOutput, CoordinationError> {
        let output = Command::new("mesh")
            .args(args)
            .output()
            .map_err(CoordinationError::Io)?;

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
    coordinator_name: String,
}

impl Default for MeshBridgedBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MeshBridgedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshBridgedBackend")
            .field("coordinator_name", &self.coordinator_name)
            .finish()
    }
}

impl MeshBridgedBackend {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "mesh-bridged-backend")]
            runner: Arc::new(SystemMeshCommandRunner),
            coordinator_name: COORDINATOR_AGENT_NAME.to_string(),
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    pub fn with_runner(runner: Arc<dyn MeshCommandRunner>) -> Self {
        Self {
            runner,
            coordinator_name: COORDINATOR_AGENT_NAME.to_string(),
        }
    }

    #[cfg(not(feature = "mesh-bridged-backend"))]
    fn mesh_disabled_error() -> CoordinationError {
        CoordinationError::Backend(format!(
            "MeshBridged backend is disabled; enable Cargo feature '{FEATURE_NAME}'"
        ))
    }

    #[cfg(feature = "mesh-bridged-backend")]
    fn run_mesh(&self, args: &[&str]) -> Result<MeshCommandOutput, CoordinationError> {
        self.runner.run(args)
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

    #[cfg(feature = "mesh-bridged-backend")]
    fn send_operator_notice(
        &self,
        payload: OperatorNoticeDelivery,
    ) -> Result<DeliveryResult, CoordinationError> {
        let mut sender_candidates = vec![self.coordinator_name.as_str()];
        if self.coordinator_name != FALLBACK_OPERATOR_NAME {
            sender_candidates.push(FALLBACK_OPERATOR_NAME);
        }

        let mut last_stderr = String::new();
        for sender_name in sender_candidates {
            let out = self.run_mesh(&[
                "send",
                &payload.member_name,
                &payload.message,
                "--team",
                &payload.team_name,
                "--name",
                sender_name,
                "--summary",
                NOTICE_SUMMARY,
            ])?;
            if out.success {
                return Ok(DeliveryResult {
                    delivered: true,
                    method: crate::coordination::requests::DeliveryMethod::TmuxInjection,
                });
            }

            let stderr = out.stderr;
            let missing_sender = stderr.to_ascii_lowercase().contains(&format!(
                "agent '{}' not found",
                sender_name.to_ascii_lowercase()
            ));
            last_stderr = stderr;

            // Retry with a known in-team fallback sender only when the previous sender
            // specifically failed lookup.
            if !missing_sender {
                break;
            }
        }

        Err(CoordinationError::Backend(format!(
            "mesh send failed for '{}' in '{}': {}",
            payload.member_name, payload.team_name, last_stderr
        )))
    }

    #[cfg(not(feature = "mesh-bridged-backend"))]
    fn send_operator_notice(
        &self,
        _payload: OperatorNoticeDelivery,
    ) -> Result<DeliveryResult, CoordinationError> {
        Err(Self::mesh_disabled_error())
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
            DeliveryRequest::OperatorNotice(payload) => self.send_operator_notice(payload),
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
        fn run(&self, args: &[&str]) -> Result<MeshCommandOutput, CoordinationError> {
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

    fn sample_member(name: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            instructions: None,
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
        }
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn launch_invokes_mesh_join() {
        let runner = MockRunner::with_outcomes(vec![MeshCommandOutput {
            success: true,
            stdout: "joined".to_string(),
            stderr: String::new(),
        }]);
        let backend = MeshBridgedBackend::with_runner(runner.clone());

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
                "join",
                "--team",
                "architecture-final",
                "--name",
                "codex-reviewer"
            ]]
        );
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn operator_notice_invokes_mesh_send() {
        let runner = MockRunner::with_outcomes(vec![MeshCommandOutput {
            success: true,
            stdout: "sent".to_string(),
            stderr: String::new(),
        }]);
        let backend = MeshBridgedBackend::with_runner(runner.clone());

        let result = backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: "codex-reviewer".to_string(),
                team_name: "architecture-final".to_string(),
                message: "check in".to_string(),
            }))
            .expect("delivery should succeed");

        assert!(result.delivered);
        assert_eq!(
            result.method,
            crate::coordination::requests::DeliveryMethod::TmuxInjection
        );
        assert_eq!(
            runner.calls(),
            vec![vec![
                "send",
                "codex-reviewer",
                "check in",
                "--team",
                "architecture-final",
                "--name",
                COORDINATOR_AGENT_NAME,
                "--summary",
                NOTICE_SUMMARY
            ]]
        );
    }

    #[cfg(feature = "mesh-bridged-backend")]
    #[test]
    fn operator_notice_retries_with_fallback_sender_when_coordinator_missing() {
        let runner = MockRunner::with_outcomes(vec![
            MeshCommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "error: agent 'taurhaus-orchestrator' not found (no inbox)".to_string(),
            },
            MeshCommandOutput {
                success: true,
                stdout: "sent".to_string(),
                stderr: String::new(),
            },
        ]);
        let backend = MeshBridgedBackend::with_runner(runner.clone());

        let result = backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: "codex-reviewer".to_string(),
                team_name: "architecture-final".to_string(),
                message: "check in".to_string(),
            }))
            .expect("delivery should succeed after fallback retry");

        assert!(result.delivered);
        assert_eq!(
            result.method,
            crate::coordination::requests::DeliveryMethod::TmuxInjection
        );
        assert_eq!(
            runner.calls(),
            vec![
                vec![
                    "send",
                    "codex-reviewer",
                    "check in",
                    "--team",
                    "architecture-final",
                    "--name",
                    COORDINATOR_AGENT_NAME,
                    "--summary",
                    NOTICE_SUMMARY
                ],
                vec![
                    "send",
                    "codex-reviewer",
                    "check in",
                    "--team",
                    "architecture-final",
                    "--name",
                    FALLBACK_OPERATOR_NAME,
                    "--summary",
                    NOTICE_SUMMARY
                ]
            ]
        );
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
                backend.deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                    member_name: "fake-agent".to_string(),
                    team_name: "architecture-final".to_string(),
                    message: "hello".to_string(),
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
    fn preflight_agent_tool_missing_returns_warning_without_blocking() {
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
        assert_eq!(
            report.agent_warnings[0].message,
            "Codex CLI not found - agent 'frontend-dev' cannot be launched."
        );
        assert!(report.can_initialize());
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
        assert!(report.agent_warnings.iter().any(|warning| {
            warning.message == "Claude CLI not found - agent 'team-lead' cannot be launched."
        }));
        assert!(report.agent_warnings.iter().any(|warning| {
            warning.message == "Codex CLI not found - agent 'frontend-dev' cannot be launched."
        }));
        assert!(!report.can_initialize());
    }
}
