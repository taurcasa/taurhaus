//! Coordination runtime boundary for external side effects.
//!
//! This isolates host-level operations (tmux, mesh, process control) behind a
//! single interface so tests can run against a deterministic runtime double.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use crate::coordination::domain::{HealthState, Member};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::process::detect_cli_tool;

mod process;
mod recording;
mod system;
mod tmux;

pub(crate) use process::{
    apply_background_command_settings, mesh_cli_claude_dir_arg_from_path,
    mesh_command_invocation_for_member_at,
};
pub use recording::{RecordingCoordinationRuntime, RuntimeCall};
/// Test seam used by the scanner tests; the integration-test shim crates
/// compile this module too and do not use it.
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use system::RealRuntimeScan;
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
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError>;

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError>;

    fn spawn_mesh_daemon_at_root(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
        _teams_dir: &std::path::Path,
    ) -> Result<u32, CoordinationError> {
        self.spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        _team_name: &str,
        _operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        Err(CoordinationError::Backend(
            "team daemon start not implemented".to_string(),
        ))
    }

    fn spawn_team_daemon_at_root(
        &self,
        team_name: &str,
        operator_name: &str,
        _teams_dir: &std::path::Path,
    ) -> Result<u32, CoordinationError> {
        self.spawn_team_daemon(team_name, operator_name)
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

    fn find_existing_mesh_daemon_pid_by_member_at_root(
        &self,
        team_name: &str,
        member_name: &str,
        _teams_dir: &Path,
    ) -> Result<Option<u32>, CoordinationError> {
        self.find_existing_mesh_daemon_pid_by_member(team_name, member_name)
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
    fn live_pane(&self, pane_id: &str) -> Result<Option<LivePane>, CoordinationError> {
        if !self.pane_exists(pane_id)? {
            return Ok(None);
        }
        Ok(Some(LivePane {
            pane_id: pane_id.to_string(),
            pane_pid: None,
            pane_start_time: None,
            current_command: self.pane_current_command(pane_id)?,
            current_path: None,
            is_dead: self.pane_is_dead(pane_id)?,
        }))
    }
    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError>;
    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError>;
    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError>;

    fn mesh_daemon_uses_current_binary(&self, _pid: u32) -> Result<bool, CoordinationError> {
        Ok(true)
    }

    fn team_daemon_uses_current_binary(&self, _team_name: &str) -> Result<bool, CoordinationError> {
        Ok(true)
    }

    fn team_daemon_uses_current_binary_at_root(
        &self,
        team_name: &str,
        _teams_dir: &Path,
    ) -> Result<bool, CoordinationError> {
        self.team_daemon_uses_current_binary(team_name)
    }

    fn clear_mesh_daemon_pid_file(
        &self,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn clear_mesh_daemon_pid_file_at_root(
        &self,
        team_name: &str,
        member_name: &str,
        _teams_dir: &Path,
    ) -> Result<(), CoordinationError> {
        self.clear_mesh_daemon_pid_file(team_name, member_name)
    }

    fn stop_team_daemon(&self, _team_name: &str) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn stop_team_daemon_at_root(
        &self,
        team_name: &str,
        _teams_dir: &Path,
    ) -> Result<(), CoordinationError> {
        self.stop_team_daemon(team_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneResolution {
    pub pane_id: String,
    pub reused_pane: bool,
    pub created_new_pane: bool,
    pub foreign_pane_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivePane {
    pub pane_id: String,
    pub pane_pid: Option<u32>,
    pub pane_start_time: Option<u64>,
    pub current_command: Option<String>,
    pub current_path: Option<PathBuf>,
    pub is_dead: bool,
}

impl LivePane {
    pub fn is_shell(&self) -> bool {
        self.current_command
            .as_deref()
            .is_some_and(tmux::is_shell_command)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneOwnership {
    Owned,
    Foreign { reason: String },
}

/// Verify that a live tmux pane is still the pane recorded for a member.
///
/// New records use the pane PID and, on Linux, the process start ticks as the
/// decisive identity. Legacy records fall back to the configured tool and
/// project path. Unknown commands such as `cat` cannot disprove ownership and
/// remain eligible for the universal tmux floor.
pub fn pane_belongs_to_member(record: &MemberRuntimeRecord, live_pane: &LivePane) -> PaneOwnership {
    if record.pane_id.as_deref() != Some(live_pane.pane_id.as_str()) {
        return PaneOwnership::Foreign {
            reason: "pane_id_mismatch".to_string(),
        };
    }

    if let Some(expected) = record.pane_pid {
        if live_pane.pane_pid != Some(expected) {
            return PaneOwnership::Foreign {
                reason: "pane_pid_mismatch".to_string(),
            };
        }
    }
    if let Some(expected) = record.pane_start_time {
        if live_pane.pane_start_time != Some(expected) {
            return PaneOwnership::Foreign {
                reason: "pane_start_time_mismatch".to_string(),
            };
        }
    }
    if record.pane_pid.is_some() || record.pane_start_time.is_some() {
        return PaneOwnership::Owned;
    }

    if let (Some(expected_tool), Some(found_tool)) = (
        record.cli_tool,
        live_pane
            .current_command
            .as_deref()
            .and_then(detect_cli_tool),
    ) {
        if expected_tool != found_tool {
            return PaneOwnership::Foreign {
                reason: format!("cli_tool_mismatch: expected={expected_tool} found={found_tool}"),
            };
        }
    }

    if let (Some(expected), Some(found)) = (
        record.project_path.as_deref(),
        live_pane.current_path.as_deref(),
    ) {
        if crate::provider::path::normalize_project_path(&expected.display().to_string())
            != crate::provider::path::normalize_project_path(&found.display().to_string())
        {
            return PaneOwnership::Foreign {
                reason: "project_path_mismatch".to_string(),
            };
        }
    }

    PaneOwnership::Owned
}

/// Atomically quarantine a member only if its persisted pane binding still
/// matches the record and live pane used to reach the foreign verdict.
pub fn quarantine_foreign_member(
    teams_dir: &Path,
    runtime: &dyn CoordinationRuntime,
    team_name: &str,
    member_name: &str,
    observed_record: &MemberRuntimeRecord,
    live_pane: &LivePane,
    reason: &str,
) -> Result<bool, CoordinationError> {
    let mut applied = false;
    let mut daemon_pid = None;
    let observed_pane_id = observed_record.pane_id.as_deref();

    MemberRuntimeStore::update(teams_dir, team_name, member_name, |record| {
        let same_observation = observed_pane_id == Some(live_pane.pane_id.as_str())
            && record.pane_id == observed_record.pane_id
            && record.pane_pid == observed_record.pane_pid
            && record.pane_start_time == observed_record.pane_start_time
            && record.attached_at == observed_record.attached_at;
        if !same_observation {
            return;
        }

        applied = true;
        daemon_pid = record.daemon_pid;
        if record.cli_tool.is_none() {
            record.cli_tool = observed_record.cli_tool;
        }
        if record.project_path.is_none() {
            record.project_path = observed_record.project_path.clone();
        }
        record.health = HealthState::SessionDead;
        record.pane_id = None;
        record.pane_pid = None;
        record.pane_start_time = None;
        record.session_id = None;
        record.jsonl_path = None;
        record.daemon_pid = None;
    })?;

    if !applied {
        tracing::debug!(
            team = %team_name,
            member = %member_name,
            pane_id = %live_pane.pane_id,
            "ignored stale foreign-pane observation after runtime binding changed"
        );
        return Ok(false);
    }

    if let Some(pid) = daemon_pid {
        match runtime.is_process_running_by_pid(pid) {
            Ok(true) => {
                if let Err(error) = runtime.terminate_process_by_pid(pid) {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid,
                        error = %error,
                        "failed to terminate foreign-pane member daemon"
                    );
                }
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pid,
                    error = %error,
                    "failed to verify foreign-pane member daemon pid"
                );
            }
        }
    }
    if let Err(error) =
        runtime.clear_mesh_daemon_pid_file_at_root(team_name, member_name, teams_dir)
    {
        tracing::warn!(
            team = %team_name,
            member = %member_name,
            error = %error,
            "failed to clear foreign-pane daemon pid file"
        );
    }

    if let Err(error) =
        TeamConfigStore::clear_member_pane_binding(teams_dir, team_name, member_name)
    {
        tracing::warn!(
            team = %team_name,
            member = %member_name,
            error = %error,
            "failed to clear foreign pane metadata from team config"
        );
    }

    // Clearing the binding under the runtime lock makes this transition
    // idempotent: later observations cannot apply again, even when the record
    // was already SessionDead before pane reuse was discovered.
    emit_foreign_pane_event(team_name, member_name, &live_pane.pane_id, reason);

    Ok(true)
}

pub fn emit_foreign_pane_event(team_name: &str, member_name: &str, pane_id: &str, reason: &str) {
    tracing::warn!(
        team = %team_name,
        member = %member_name,
        pane_id = %pane_id,
        reason,
        "recorded tmux pane belongs to a foreign member"
    );
    let mut fields = Map::new();
    fields.insert("team".to_string(), Value::String(team_name.to_string()));
    fields.insert("member".to_string(), Value::String(member_name.to_string()));
    fields.insert("pane_id".to_string(), Value::String(pane_id.to_string()));
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    emit_global(
        "warn",
        "coordination",
        "coordination.pane.foreign",
        Some("Recorded tmux pane belongs to a foreign member".to_string()),
        fields,
    );
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
            foreign_pane_reason: None,
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

    let mut ownership_record = runtime_record
        .cloned()
        .expect("existing pane id requires a runtime record");
    ownership_record.cli_tool.get_or_insert(member.cli_tool);
    ownership_record
        .project_path
        .get_or_insert_with(|| member.project_path.clone());
    let live_pane = match runtime.live_pane(existing_pane_id) {
        Ok(Some(live_pane)) => live_pane,
        Ok(None) => return create_new(),
        Err(err) => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                error = %err,
                "resume pane resolution: pane identity check failed, creating new pane"
            );
            return create_new();
        }
    };
    match pane_belongs_to_member(&ownership_record, &live_pane) {
        PaneOwnership::Owned => Ok(PaneResolution {
            pane_id: existing_pane_id.to_string(),
            reused_pane: true,
            created_new_pane: false,
            foreign_pane_reason: None,
        }),
        PaneOwnership::Foreign { reason } => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                reason,
                "resume pane resolution: pane ownership mismatch, creating new pane"
            );
            let mut resolution = create_new()?;
            resolution.foreign_pane_reason = Some(reason);
            Ok(resolution)
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
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    fn sample_runtime_with_pane(member_name: &str, pane_id: &str) -> MemberRuntimeRecord {
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: member_name.to_string(),
            cli_tool: None,
            project_path: None,
            pane_id: Some(pane_id.to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
            applied_effort: None,
            effort_resume_failure: None,
            launch_account: Default::default(),
            extra: Default::default(),
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

    #[test]
    fn pane_pid_mismatch_is_foreign() {
        // Regression: mesh-findings P3, tmux reused pane ids; daemons for
        // taurrust/gotaurus/espn pointed at claude panes.
        let mut record = sample_runtime_with_pane("agent-a", "%9");
        record.cli_tool = Some(CliTool::Codex);
        record.project_path = Some(PathBuf::from("/tmp/project"));
        record.pane_pid = Some(1200);
        record.pane_start_time = Some(1_755_000_000);
        let live_pane = LivePane {
            pane_id: "%9".to_string(),
            pane_pid: Some(9900),
            pane_start_time: Some(1_755_000_000),
            current_command: Some("codex".to_string()),
            current_path: Some(PathBuf::from("/tmp/project")),
            is_dead: false,
        };

        assert!(matches!(
            pane_belongs_to_member(&record, &live_pane),
            PaneOwnership::Foreign { ref reason } if reason == "pane_pid_mismatch"
        ));
    }

    #[test]
    fn matching_pane_pid_and_start_time_is_owned() {
        // Regression: mesh-findings P3, tmux reused pane ids; daemons for
        // taurrust/gotaurus/espn pointed at claude panes.
        let mut record = sample_runtime_with_pane("agent-a", "%9");
        record.cli_tool = Some(CliTool::Codex);
        record.project_path = Some(PathBuf::from("/tmp/project"));
        record.pane_pid = Some(1200);
        record.pane_start_time = Some(1_755_000_000);
        let live_pane = LivePane {
            pane_id: "%9".to_string(),
            pane_pid: Some(1200),
            pane_start_time: Some(1_755_000_000),
            current_command: Some("codex".to_string()),
            current_path: Some(PathBuf::from("/tmp/project")),
            is_dead: false,
        };

        assert_eq!(
            pane_belongs_to_member(&record, &live_pane),
            PaneOwnership::Owned
        );
    }

    #[test]
    fn matching_primary_identity_ignores_foreground_cwd_changes() {
        // Regression: aecc8ac checked pane_current_path before the recorded
        // PID identity, so `cd` into a project subdirectory killed a healthy daemon.
        let mut record = sample_runtime_with_pane("agent-a", "%9");
        record.cli_tool = Some(CliTool::Codex);
        record.project_path = Some(PathBuf::from("/tmp/project"));
        record.pane_pid = Some(1200);
        record.pane_start_time = Some(1_755_000_000);
        let live_pane = LivePane {
            pane_id: "%9".to_string(),
            pane_pid: Some(1200),
            pane_start_time: Some(1_755_000_000),
            current_command: Some("codex".to_string()),
            current_path: Some(PathBuf::from("/tmp/project/subdirectory")),
            is_dead: false,
        };

        assert_eq!(
            pane_belongs_to_member(&record, &live_pane),
            PaneOwnership::Owned
        );
    }

    #[test]
    fn matching_pane_identity_with_foreign_cli_is_foreign() {
        // Regression: mesh-findings P3, tmux reused pane ids; daemons for
        // taurrust/gotaurus/espn pointed at claude panes.
        let mut record = sample_runtime_with_pane("agent-a", "%9");
        record.cli_tool = Some(CliTool::Codex);
        let live_pane = LivePane {
            pane_id: "%9".to_string(),
            pane_pid: None,
            pane_start_time: None,
            current_command: Some("claude".to_string()),
            current_path: None,
            is_dead: false,
        };

        assert!(matches!(
            pane_belongs_to_member(&record, &live_pane),
            PaneOwnership::Foreign { ref reason } if reason.contains("expected=codex found=claude")
        ));
    }

    #[test]
    fn already_dead_record_emits_foreign_event_once_when_binding_is_cleared() {
        // Regression: aecc8ac suppressed the first foreign-pane event when an
        // earlier missing-pane pass had already marked the stale record dead.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let log_path = tmp.path().join("pane-foreign-events.jsonl");
        let log_state =
            taurhaus_lib::logging::LogFileState::new(log_path.clone()).expect("log state");
        taurhaus_lib::logging::install_global_sink(&log_state);
        let runtime = RecordingCoordinationRuntime::default();
        let record = sample_runtime_with_pane("agent-a", "%9");
        MemberRuntimeStore::save(tmp.path(), "team-a", "agent-a", &record).expect("save runtime");
        let live_pane = LivePane {
            pane_id: "%9".to_string(),
            pane_pid: None,
            pane_start_time: None,
            current_command: Some("claude".to_string()),
            current_path: None,
            is_dead: false,
        };

        assert!(quarantine_foreign_member(
            tmp.path(),
            &runtime,
            "team-a",
            "agent-a",
            &record,
            &live_pane,
            "cli_tool_mismatch",
        )
        .expect("first quarantine"));
        assert!(!quarantine_foreign_member(
            tmp.path(),
            &runtime,
            "team-a",
            "agent-a",
            &record,
            &live_pane,
            "cli_tool_mismatch",
        )
        .expect("repeat quarantine"));

        log_state.flush_for_test().expect("flush log");
        let event_count = std::fs::read_to_string(log_path)
            .expect("read log")
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|event| {
                event["event"] == "coordination.pane.foreign"
                    && event["team"] == "team-a"
                    && event["member"] == "agent-a"
            })
            .count();
        assert_eq!(event_count, 1);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[ignore = "requires tmux on the host"]
    fn tmux_pane_running_cat_still_passes_member_ownership_guard() {
        // Regression: mesh-findings P3, tmux reused pane ids; daemons for
        // taurrust/gotaurus/espn pointed at claude panes.
        struct TmuxSessionGuard(String);
        impl Drop for TmuxSessionGuard {
            fn drop(&mut self) {
                let _ = std::process::Command::new("tmux")
                    .args(["kill-session", "-t", &self.0])
                    .status();
            }
        }

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let session_name = format!("taurhaus-pane-guard-{}", std::process::id());
        let _guard = TmuxSessionGuard(session_name.clone());
        let status = std::process::Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &session_name,
                "-c",
                tmp.path().to_string_lossy().as_ref(),
                "cat",
            ])
            .status()
            .expect("run tmux");
        assert!(status.success(), "create tmux cat pane");
        let output = std::process::Command::new("tmux")
            .args(["list-panes", "-t", &session_name, "-F", "#{pane_id}"])
            .output()
            .expect("list panes");
        assert!(output.status.success(), "list tmux cat pane");
        let pane_id = String::from_utf8(output.stdout)
            .expect("utf8 pane id")
            .trim()
            .to_string();
        let runtime = SystemCoordinationRuntime;
        let live_pane = runtime
            .live_pane(&pane_id)
            .expect("probe pane")
            .expect("live pane");
        let mut record = sample_runtime_with_pane("agent-a", &pane_id);
        record.cli_tool = Some(CliTool::Codex);
        record.project_path = Some(tmp.path().to_path_buf());
        record.pane_pid = live_pane.pane_pid;
        record.pane_start_time = live_pane.pane_start_time;

        assert!(record.pane_pid.is_some(), "tmux should report the pane pid");
        #[cfg(target_os = "linux")]
        assert!(
            record.pane_start_time.is_some(),
            "the Linux process inventory should report the pane process start time"
        );

        assert_eq!(
            pane_belongs_to_member(&record, &live_pane),
            PaneOwnership::Owned
        );
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
            "/home/user/.local/bin/mesh".to_string(),
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
    fn resolve_or_create_pane_identity_probe_failure_creates_new_pane() {
        // Regression: aecc8ac propagated a live-pane probe error even though
        // resume had historically failed soft by creating a fresh pane.
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_live_pane_failure("%9", "transient tmux failure");
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("probe failure should fall back to a new pane");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
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
            .join_mesh(
                "alpha",
                "agent-a",
                "/tmp/project",
                "general-purpose",
                "gpt-5.6-sol",
                "/tmp/claude",
            )
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
                    member_type: "general-purpose".to_string(),
                    model: "gpt-5.6-sol".to_string(),
                    claude_dir: "/tmp/claude".to_string(),
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
