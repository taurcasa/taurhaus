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
#[cfg(feature = "mesh-bridged-backend")]
const NOTICE_SUMMARY: &str = "operator_notice";

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
        let out = self.run_mesh(&[
            "send",
            &payload.member_name,
            &payload.message,
            "--team",
            &payload.team_name,
            "--name",
            &self.coordinator_name,
            "--summary",
            NOTICE_SUMMARY,
        ])?;
        if out.success {
            Ok(DeliveryResult {
                delivered: true,
                method: crate::coordination::requests::DeliveryMethod::TmuxInjection,
            })
        } else {
            Err(CoordinationError::Backend(format!(
                "mesh send failed for '{}' in '{}': {}",
                payload.member_name, payload.team_name, out.stderr
            )))
        }
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
            pane_id: req.pane_target.unwrap_or_else(|| "mesh-bridged".to_string()),
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

            let delivered = backend.deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
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
}
