//! Coordination runtime boundary for external side effects.
//!
//! This isolates host-level operations (tmux, mesh, process control) behind a
//! single interface so tests can run against a deterministic runtime double.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::MemberRuntimeRecord;
use crate::session_scanner::cli_tool::CliTool;

mod process;
mod recording;
mod system;
mod tmux;

pub(crate) use process::{apply_background_command_settings, mesh_command_invocation_for_member};
pub use recording::{RecordingCoordinationRuntime, RuntimeCall};
pub use system::SystemCoordinationRuntime;

const TMUX_TEXT_TO_ENTER_DELAY: Duration = Duration::from_millis(350);
const TMUX_POST_ENTER_DELAY: Duration = Duration::from_secs(1);
const FRESH_LAUNCH_RETRY_DELAYS: [Duration; 2] =
    [Duration::from_millis(150), Duration::from_millis(350)];
const SESSION_DETECT_ATTEMPTS: usize = 6;
const SESSION_DETECT_INTERVAL: Duration = Duration::from_millis(200);
const DAEMON_START_ATTEMPTS: usize = 30;
const DAEMON_START_INTERVAL: Duration = Duration::from_millis(100);
const TAURHAUS_TMUX_SESSION_NAME: &str = "taurhaus";
#[cfg(not(target_os = "windows"))]
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
pub(crate) const MESH_CONTROL_TOKEN_ENV: &str = "MESH_CONTROL_TOKEN";

pub trait CoordinationRuntime: Send + Sync {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError>;

    fn create_aitx_pane_and_launch(
        &self,
        project_id: &str,
        tmux_layout: &str,
        launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        let pane_id = self.create_aitx_pane(project_id, tmux_layout)?;
        let mut last_err = None;
        for retry_delay in FRESH_LAUNCH_RETRY_DELAYS
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
        {
            match self.send_tmux_keys_with_enter(&pane_id, launch_cmd) {
                Ok(()) => return Ok(pane_id),
                Err(err) => {
                    last_err = Some(err);
                    if let Some(delay) = retry_delay {
                        thread::sleep(delay);
                    }
                }
            }
        }

        let err = last_err.unwrap_or_else(|| {
            CoordinationError::Backend("tmux send-keys failed without error detail".to_string())
        });
        Err(CoordinationError::Backend(format!(
            "{err}; pane diagnostics: {}",
            pane_diagnostics_for_launch_failure(self, &pane_id)
        )))
    }

    fn create_aitx_pane_and_launch_in_target(
        &self,
        project_id: &str,
        target_pane: &str,
        launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        let _ = target_pane;
        self.create_aitx_pane_and_launch(project_id, "per_project", launch_cmd)
    }

    fn send_tmux_keys_with_enter(&self, pane_id: &str, keys: &str)
        -> Result<(), CoordinationError>;

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError>;

    fn detect_runtime_session(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<DetectedRuntimeSession, CoordinationError> {
        Ok(DetectedRuntimeSession {
            session_id: self.detect_session_id(pane_id, cli_tool)?,
            jsonl_path: None,
        })
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
    ) -> Result<(), CoordinationError>;

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError>;

    fn spawn_team_daemon(
        &self,
        _team_name: &str,
        _operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        Err(CoordinationError::Backend(
            "team daemon start not implemented".to_string(),
        ))
    }

    fn find_existing_mesh_daemon_pids(
        &self,
        _pane_id: &str,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        Ok(Vec::new())
    }

    fn find_existing_mesh_daemon_pid_by_member(
        &self,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<Option<u32>, CoordinationError> {
        Ok(None)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError>;
    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError>;
    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError>;
    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError>;
    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError>;

    fn mesh_daemon_uses_current_binary(&self, _pid: u32) -> Result<bool, CoordinationError> {
        Ok(true)
    }

    fn team_daemon_uses_current_binary(&self, _team_name: &str) -> Result<bool, CoordinationError> {
        Ok(true)
    }

    fn clear_mesh_daemon_pid_file(
        &self,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn stop_team_daemon(&self, _team_name: &str) -> Result<(), CoordinationError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneResolution {
    pub pane_id: String,
    pub reused_pane: bool,
    pub created_new_pane: bool,
}

fn pane_diagnostics_for_launch_failure<T: CoordinationRuntime + ?Sized>(
    runtime: &T,
    pane_id: &str,
) -> String {
    let exists = match runtime.pane_exists(pane_id) {
        Ok(value) => value.to_string(),
        Err(err) => format!("error({err})"),
    };
    let dead = match runtime.pane_is_dead(pane_id) {
        Ok(value) => value.to_string(),
        Err(err) => format!("error({err})"),
    };
    let shell = match runtime.pane_is_shell(pane_id) {
        Ok(value) => value.to_string(),
        Err(err) => format!("error({err})"),
    };
    let command = match runtime.pane_current_command(pane_id) {
        Ok(Some(value)) => value,
        Ok(None) => "none".to_string(),
        Err(err) => format!("error({err})"),
    };
    format!("pane={pane_id} exists={exists} dead={dead} shell={shell} command={command}")
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetectedRuntimeSession {
    pub session_id: Option<String>,
    pub jsonl_path: Option<PathBuf>,
}

/// Resolve a reusable pane for a member or create a fresh one.
///
/// Classification:
/// - Missing pane id -> create
/// - Lookup fail / missing pane target -> create
/// - Dead pane + ownership match -> kill + create
/// - Alive pane + ownership match -> reuse
/// - Ownership mismatch/check failure -> warn + create
pub fn resolve_or_create_pane_for_member(
    runtime: &dyn CoordinationRuntime,
    member: &Member,
    runtime_record: Option<&MemberRuntimeRecord>,
    tmux_layout: &str,
) -> Result<PaneResolution, CoordinationError> {
    let project_id = member.project_path.display().to_string();

    let create_new = || -> Result<PaneResolution, CoordinationError> {
        let pane_id = runtime.create_aitx_pane(&project_id, tmux_layout)?;
        Ok(PaneResolution {
            pane_id,
            reused_pane: false,
            created_new_pane: true,
        })
    };

    let Some(existing_pane_id) = runtime_record.and_then(|record| record.pane_id.as_deref()) else {
        return create_new();
    };

    if !runtime.pane_exists(existing_pane_id)? {
        return create_new();
    }

    let is_dead = runtime.pane_is_dead(existing_pane_id)?;
    if is_dead {
        match runtime.pane_belongs_to_project(existing_pane_id, &project_id) {
            Ok(true) => {
                if let Err(err) = runtime.kill_aitx_pane(existing_pane_id) {
                    tracing::warn!(
                        pane_id = %existing_pane_id,
                        member = %member.name,
                        team_project = %project_id,
                        error = %err,
                        "resume pane resolution: failed to kill dead pane before recreate"
                    );
                }
                return create_new();
            }
            Ok(false) => {
                tracing::warn!(
                    pane_id = %existing_pane_id,
                    member = %member.name,
                    team_project = %project_id,
                    "resume pane resolution: dead pane ownership mismatch, creating new pane"
                );
                return create_new();
            }
            Err(err) => {
                tracing::warn!(
                    pane_id = %existing_pane_id,
                    member = %member.name,
                    team_project = %project_id,
                    error = %err,
                    "resume pane resolution: failed ownership check for dead pane, creating new pane"
                );
                return create_new();
            }
        }
    }

    match runtime.pane_belongs_to_project(existing_pane_id, &project_id) {
        Ok(true) => Ok(PaneResolution {
            pane_id: existing_pane_id.to_string(),
            reused_pane: true,
            created_new_pane: false,
        }),
        Ok(false) => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                "resume pane resolution: pane ownership mismatch, creating new pane"
            );
            create_new()
        }
        Err(err) => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                error = %err,
                "resume pane resolution: ownership check failed, creating new pane"
            );
            create_new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::process::{
        command_matches_team_daemon, process_uses_current_mesh_binary, validate_unix_pid,
        wait_for_mesh_daemon_pid_file_with_retries, wait_for_team_daemon_pid_file_with_retries,
    };
    use super::tmux::{is_shell_command, tmux_target_for_pane};
    use super::*;
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::stores::MemberRuntimeRecord;
    use fs2::FileExt;
    #[cfg(not(target_os = "windows"))]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvTestGuard {
        _in_process: MutexGuard<'static, ()>,
        lock_file: std::fs::File,
    }

    impl Drop for EnvTestGuard {
        fn drop(&mut self) {
            let _ = self.lock_file.unlock();
        }
    }

    fn acquire_env_test_guard() -> EnvTestGuard {
        let in_process = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let lock_path = std::env::temp_dir().join("taurhaus-env-tests.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap_or_else(|e| panic!("failed to open env test lock at {:?}: {e}", lock_path));
        lock_file
            .lock_exclusive()
            .unwrap_or_else(|e| panic!("failed to lock env test lock at {:?}: {e}", lock_path));
        EnvTestGuard {
            _in_process: in_process,
            lock_file,
        }
    }

    fn sample_member(name: &str, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            runtime_compact_summary: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
        }
    }

    fn sample_runtime_with_pane(member_name: &str, pane_id: &str) -> MemberRuntimeRecord {
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: member_name.to_string(),
            cli_tool: None,
            project_path: None,
            pane_id: Some(pane_id.to_string()),
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn write_executable_script(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).expect("write script");
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod");
    }

    #[cfg(not(target_os = "windows"))]
    fn copy_executable(from: &std::path::Path, to: &std::path::Path) {
        use std::io::{copy, Seek, SeekFrom};

        let parent = to.parent().expect("copy target parent");
        let mut source = std::fs::File::open(from).expect("open source executable");
        let mut staged = tempfile::NamedTempFile::new_in(parent).expect("stage executable");
        copy(&mut source, staged.as_file_mut()).expect("copy executable");
        staged
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .expect("rewind staged executable");
        let mut permissions = staged
            .as_file()
            .metadata()
            .expect("staged metadata")
            .permissions();
        permissions.set_mode(0o755);
        staged
            .as_file()
            .set_permissions(permissions)
            .expect("chmod staged executable");
        staged.as_file().sync_all().expect("sync staged executable");
        staged.persist(to).expect("persist staged executable");
    }

    #[test]
    fn tmux_target_uses_pane_id_when_present() {
        assert_eq!(tmux_target_for_pane("%12"), "%12");
    }

    #[test]
    fn tmux_target_wraps_numeric_index() {
        assert_eq!(tmux_target_for_pane("3"), ":.3");
    }

    #[test]
    fn shell_command_detection_matches_supported_shells() {
        assert!(is_shell_command("bash"));
        assert!(is_shell_command("zsh"));
        assert!(is_shell_command("/usr/bin/fish"));
        assert!(is_shell_command("-sh"));
    }

    #[test]
    fn shell_command_detection_rejects_non_shell_commands() {
        assert!(!is_shell_command("codex"));
        assert!(!is_shell_command("claude"));
        assert!(!is_shell_command(""));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_accepts_normal_pid() {
        assert_eq!(validate_unix_pid(12345).unwrap(), "12345");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_zero() {
        let err = validate_unix_pid(0).expect_err("pid 0 should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_values_above_i32_max() {
        let err = validate_unix_pid(u32::MAX).expect_err("out-of-range pid should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn system_runtime_is_process_running_returns_false_for_out_of_range_pid() {
        let runtime = SystemCoordinationRuntime;
        assert!(!runtime.is_process_running_by_pid(u32::MAX).unwrap());
        assert!(!runtime.is_process_running_by_pid(0).unwrap());
    }

    #[test]
    fn command_matches_team_daemon_requires_expected_team() {
        let args = vec![
            "/home/mstie/.local/bin/mesh".to_string(),
            "team-daemon".to_string(),
            "start".to_string(),
            "--team".to_string(),
            "alpha".to_string(),
            "--name".to_string(),
            "operator".to_string(),
        ];
        assert!(command_matches_team_daemon(&args, "alpha"));
        assert!(!command_matches_team_daemon(&args, "beta"));
    }

    #[test]
    fn resolve_or_create_pane_missing_creates_new_pane() {
        let runtime = RecordingCoordinationRuntime::default();
        let member = sample_member("agent-a", "/tmp/project");

        let resolution = resolve_or_create_pane_for_member(&runtime, &member, None, "new_window")
            .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
    }

    #[test]
    fn resolve_or_create_pane_lookup_fail_creates_new_pane() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", false);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        assert!(runtime.calls().contains(&RuntimeCall::CheckPaneExists {
            pane_id: "%9".to_string()
        }));
    }

    #[test]
    fn resolve_or_create_pane_dead_and_owned_kills_then_recreates() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", true);
        runtime.set_pane_ownership("%9", true);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        let calls = runtime.calls();
        assert!(calls.contains(&RuntimeCall::KillPane {
            pane_id: "%9".to_string()
        }));
    }

    #[test]
    fn resolve_or_create_pane_alive_and_owned_reuses_existing() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_ownership("%9", true);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(!resolution.created_new_pane);
        assert!(resolution.reused_pane);
        assert_eq!(resolution.pane_id, "%9");
        let calls = runtime.calls();
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CreatePane { .. })));
    }

    #[test]
    fn resolve_or_create_pane_ownership_mismatch_creates_without_kill() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_ownership("%9", false);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        let calls = runtime.calls();
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CreatePane { .. })));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%9")));
    }

    #[test]
    fn recording_runtime_returns_deterministic_values_and_records_calls() {
        let runtime = RecordingCoordinationRuntime::default();

        let pane = runtime
            .create_aitx_pane("/tmp/project", "new_window")
            .expect("pane");
        let pid = runtime
            .spawn_mesh_daemon(&pane, "alpha", "agent-a")
            .expect("pid");
        runtime
            .send_tmux_keys_with_enter(&pane, "codex --yolo")
            .expect("keys");
        runtime
            .join_mesh("alpha", "agent-a", "/tmp/project")
            .expect("join");
        assert!(runtime
            .pane_belongs_to_project(&pane, "/tmp/project")
            .expect("ownership check"));
        runtime.terminate_process_by_pid(pid).expect("terminate");
        runtime.kill_aitx_pane(&pane).expect("kill pane");
        assert!(!runtime.is_process_running_by_pid(pid).expect("check pid"));

        assert_eq!(pane, "test-pane-1");
        assert_eq!(pid, 10000);
        assert_eq!(
            runtime.calls(),
            vec![
                RuntimeCall::CreatePane {
                    project_id: "/tmp/project".to_string()
                },
                RuntimeCall::SpawnDaemon {
                    pane_id: "test-pane-1".to_string(),
                    team_name: "alpha".to_string(),
                    member_name: "agent-a".to_string()
                },
                RuntimeCall::SendKeys {
                    pane_id: "test-pane-1".to_string(),
                    keys: "codex --yolo".to_string()
                },
                RuntimeCall::JoinMesh {
                    team_name: "alpha".to_string(),
                    member_name: "agent-a".to_string(),
                    project_id: "/tmp/project".to_string(),
                },
                RuntimeCall::CheckPaneOwnership {
                    pane_id: "test-pane-1".to_string(),
                    project_id: "/tmp/project".to_string()
                },
                RuntimeCall::TerminatePid { pid: 10000 },
                RuntimeCall::KillPane {
                    pane_id: "test-pane-1".to_string()
                },
                RuntimeCall::CheckPid { pid: 10000 },
            ]
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_mesh_daemon_pid_file_with_retries_rejects_stale_pid() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("agent.pid");
        std::fs::write(&pid_path, "0\n").expect("write pid");

        let err = wait_for_mesh_daemon_pid_file_with_retries(
            &pid_path,
            "%9",
            "alpha",
            "agent",
            1,
            Duration::ZERO,
        )
        .expect_err("stale pid should be rejected");

        assert!(err
            .to_string()
            .contains("timed out waiting for valid mesh daemon pid"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_team_daemon_pid_file_with_retries_rejects_non_matching_pid() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("team.pid");
        let script_path = dir.path().join("sleepy.sh");
        write_executable_script(&script_path, "#!/bin/sh\nsleep 5\n");

        let mut child = std::process::Command::new(&script_path)
            .spawn()
            .expect("spawn");
        std::fs::write(&pid_path, format!("{}\n", child.id())).expect("write pid");

        let err = wait_for_team_daemon_pid_file_with_retries(&pid_path, "alpha", 1, Duration::ZERO)
            .expect_err("non-matching pid should be rejected");

        assert!(err
            .to_string()
            .contains("timed out waiting for valid team daemon pid"));
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_team_daemon_pid_file_with_retries_accepts_matching_pid() {
        let _guard = acquire_env_test_guard();
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("team.pid");
        let install_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&install_dir).expect("install dir");
        let mesh_path = dir.path().join("mesh");
        std::os::unix::fs::symlink("/bin/sh", &mesh_path).expect("symlink mesh");
        std::os::unix::fs::symlink(&mesh_path, install_dir.join("mesh")).expect("install mesh");

        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", dir.path());

        let mut child = std::process::Command::new(&mesh_path)
            .args([
                "-c",
                "sleep 5",
                "team-daemon",
                "start",
                "--team",
                "alpha",
                "--name",
                "operator",
            ])
            .spawn()
            .expect("spawn");
        std::fs::write(&pid_path, format!("{}\n", child.id())).expect("write pid");

        let pid = wait_for_team_daemon_pid_file_with_retries(&pid_path, "alpha", 1, Duration::ZERO)
            .expect("matching pid should be accepted");

        assert_eq!(pid, child.id());
        let _ = child.kill();
        let _ = child.wait();
        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_mesh_daemon_pid_file_with_retries_rejects_non_matching_live_pid() {
        let _guard = acquire_env_test_guard();
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("agent.pid");
        let install_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&install_dir).expect("install dir");
        let mesh_path = dir.path().join("mesh");
        std::os::unix::fs::symlink("/bin/sh", &mesh_path).expect("symlink mesh");
        std::os::unix::fs::symlink(&mesh_path, install_dir.join("mesh")).expect("install mesh");

        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", dir.path());

        let mut child = std::process::Command::new(&mesh_path)
            .args([
                "-c", "sleep 5", "mesh", "daemon", "--pane", "%old", "--team", "alpha", "--name",
                "agent",
            ])
            .spawn()
            .expect("spawn");
        std::fs::write(&pid_path, format!("{}\n", child.id())).expect("write pid");

        let err = wait_for_mesh_daemon_pid_file_with_retries(
            &pid_path,
            "%new",
            "alpha",
            "agent",
            1,
            Duration::ZERO,
        )
        .expect_err("wrong pane daemon should be rejected");

        assert!(err
            .to_string()
            .contains("timed out waiting for valid mesh daemon pid"));
        let _ = child.kill();
        let _ = child.wait();
        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_mesh_daemon_pid_file_with_retries_accepts_matching_pid() {
        let _guard = acquire_env_test_guard();
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("agent.pid");
        let install_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&install_dir).expect("install dir");
        let mesh_path = dir.path().join("mesh");
        std::os::unix::fs::symlink("/bin/sh", &mesh_path).expect("symlink mesh");
        std::os::unix::fs::symlink(&mesh_path, install_dir.join("mesh")).expect("install mesh");

        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", dir.path());

        let mut child = std::process::Command::new(&mesh_path)
            .args([
                "-c", "sleep 5", "mesh", "daemon", "--pane", "%9", "--team", "alpha", "--name",
                "agent",
            ])
            .spawn()
            .expect("spawn");
        std::fs::write(&pid_path, format!("{}\n", child.id())).expect("write pid");

        let pid = wait_for_mesh_daemon_pid_file_with_retries(
            &pid_path,
            "%9",
            "alpha",
            "agent",
            1,
            Duration::ZERO,
        )
        .expect("matching mesh daemon should be accepted");

        assert_eq!(pid, child.id());
        let _ = child.kill();
        let _ = child.wait();
        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn process_uses_current_mesh_binary_detects_replaced_install() {
        let _guard = acquire_env_test_guard();
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let install_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&install_dir).expect("install dir");
        let installed_mesh = install_dir.join("mesh");
        let old_mesh = temp_home.path().join("mesh-old");
        let source_mesh = PathBuf::from("/bin/sh");
        copy_executable(&source_mesh, &old_mesh);
        copy_executable(&source_mesh, &installed_mesh);

        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp_home.path());

        let mut old_child = std::process::Command::new(&old_mesh)
            .args([
                "-c", "sleep 5", "mesh", "daemon", "--pane", "%9", "--team", "alpha", "--name",
                "agent",
            ])
            .spawn()
            .expect("spawn old mesh");
        assert!(
            !process_uses_current_mesh_binary(old_child.id()).expect("drift check"),
            "running process from replaced inode should be treated as drifted"
        );
        let _ = old_child.kill();
        let _ = old_child.wait();

        let mut current_child = std::process::Command::new(&installed_mesh)
            .args([
                "-c", "sleep 5", "mesh", "daemon", "--pane", "%9", "--team", "alpha", "--name",
                "agent",
            ])
            .spawn()
            .expect("spawn current mesh");
        assert!(
            process_uses_current_mesh_binary(current_child.id()).expect("current identity"),
            "running process from installed inode should be treated as current"
        );
        let _ = current_child.kill();
        let _ = current_child.wait();

        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
