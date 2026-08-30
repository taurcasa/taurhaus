//! Shared coordination app state with lazy orchestrator bootstrap.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::coordination::backend::{
    BackendKind, BackendSelector, ClaudeNativeBackend, CoordinationBackend, MeshBridgedBackend,
};
use crate::coordination::compact_hook::{
    ensure_compact_hook_installed, team_has_managed_claude_member,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::{CoordinationOrchestrator, TeamSelfHealResult};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::TeamConfigStore;
use crate::models::CliCommandSettings;
use crate::provider::platform_paths::PlatformPaths;
#[cfg(test)]
use crate::session_scanner::cli_tool::CliTool;

type BackendFactory = dyn Fn(BackendKind, &Path) -> Result<Arc<dyn CoordinationBackend>, CoordinationError>
    + Send
    + Sync;
type RuntimeFactory = dyn Fn() -> Arc<dyn CoordinationRuntime> + Send + Sync;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundSelfHealPassResult {
    pub teams_scanned: usize,
    pub teams_skipped: usize,
    pub teams_reconciled: usize,
    pub team_daemons_ensured: usize,
    pub team_errors: usize,
    /// Members relaunched to reach the effort their assignment carries.
    pub members_effort_resumed: usize,
}

/// Layout the background pass relaunches a member into when the caller has no
/// configured one: the same default every operator-driven resume uses.
#[cfg(test)]
const DEFAULT_TMUX_LAYOUT: &str = "new_window";

/// App-managed coordination state that lazily initializes the orchestrator.
pub struct CoordinationState {
    teams_dir: PathBuf,
    app_started_at: DateTime<Utc>,
    backend_selector: BackendSelector,
    backend_factory: Arc<BackendFactory>,
    runtime_factory: Arc<RuntimeFactory>,
    orchestrator: Mutex<Option<CoordinationOrchestrator>>,
}

impl std::fmt::Debug for CoordinationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let initialized = self
            .orchestrator
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false);
        f.debug_struct("CoordinationState")
            .field("teams_dir", &self.teams_dir)
            .field("initialized", &initialized)
            .finish()
    }
}

impl CoordinationState {
    /// Build default app state without performing backend checks at startup.
    pub fn for_app_startup() -> Self {
        Self::with_components_and_runtime(
            PlatformPaths::teams_dir(),
            BackendSelector::m0(),
            Arc::new(default_backend_factory),
            Arc::new(default_runtime_factory),
        )
    }

    /// Build state with explicit dependencies (used by tests).
    pub fn with_components(
        teams_dir: PathBuf,
        backend_selector: BackendSelector,
        backend_factory: Arc<BackendFactory>,
    ) -> Self {
        Self::with_components_and_runtime(
            teams_dir,
            backend_selector,
            backend_factory,
            Arc::new(default_runtime_factory),
        )
    }

    /// Build state with explicit backend + runtime dependencies (used by tests).
    pub fn with_components_and_runtime(
        teams_dir: PathBuf,
        backend_selector: BackendSelector,
        backend_factory: Arc<BackendFactory>,
        runtime_factory: Arc<RuntimeFactory>,
    ) -> Self {
        Self::with_components_runtime_and_started_at(
            teams_dir,
            backend_selector,
            backend_factory,
            runtime_factory,
            Utc::now(),
        )
    }

    /// Build state with explicit backend + runtime dependencies and a fixed app start time.
    pub fn with_components_runtime_and_started_at(
        teams_dir: PathBuf,
        backend_selector: BackendSelector,
        backend_factory: Arc<BackendFactory>,
        runtime_factory: Arc<RuntimeFactory>,
        app_started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            teams_dir,
            app_started_at,
            backend_selector,
            backend_factory,
            runtime_factory,
            orchestrator: Mutex::new(None),
        }
    }

    pub fn teams_dir(&self) -> &PathBuf {
        &self.teams_dir
    }

    pub fn app_started_at(&self) -> DateTime<Utc> {
        self.app_started_at
    }

    /// Lazily initialize and reuse a single orchestrator instance.
    pub fn with_orchestrator<R, F>(&self, op: F) -> Result<R, CoordinationError>
    where
        F: FnOnce(&mut CoordinationOrchestrator) -> Result<R, CoordinationError>,
    {
        let mut guard = self.orchestrator.lock().map_err(|_| {
            CoordinationError::StoreError("coordination state mutex poisoned".to_string())
        })?;
        if guard.is_none() {
            *guard = Some(self.build_orchestrator()?);
        }
        let orchestrator = guard.as_mut().ok_or_else(|| {
            CoordinationError::StoreError(
                "coordination orchestrator missing after initialization".to_string(),
            )
        })?;
        op(orchestrator)
    }

    pub fn trigger_team_self_heal(
        &self,
        team_name: &str,
    ) -> Result<TeamSelfHealResult, CoordinationError> {
        self.with_orchestrator(|orchestrator| orchestrator.trigger_team_self_heal(team_name))
    }

    /// One background pass over every team.
    ///
    /// `cli_commands` and `tmux_layout` are the operator's own launch settings,
    /// resolved by the caller at the command boundary the way every other
    /// managed launch resolves them: a relaunch this pass performs must land on
    /// the same account and the same configured command as the launch it
    /// replaces.
    pub fn run_background_self_heal_pass(
        &self,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<BackgroundSelfHealPassResult, CoordinationError> {
        let team_names = TeamConfigStore::list(&self.teams_dir)?;
        let mut summary = BackgroundSelfHealPassResult::default();
        let mut orchestrator = self.build_background_orchestrator()?;

        for team_name in team_names {
            summary.teams_scanned += 1;
            match orchestrator.trigger_team_self_heal(&team_name) {
                Ok(result) => apply_self_heal_result(&mut summary, &result),
                Err(err) => {
                    summary.team_errors += 1;
                    tracing::warn!(
                        team = %team_name,
                        error = %err,
                        "background coordination self-heal failed"
                    );
                }
            }

            // Retries only. A Codex effort switch is started by the task event
            // that made the assignment visible (`apply_task_effort_for_project`);
            // this sweep exists so one that failed there — a pane that would
            // not come down, a launch that did not land — is picked up again
            // rather than left pending until the next assignment.
            match orchestrator.apply_pending_task_effort(
                &team_name,
                cli_commands,
                tmux_layout,
                crate::coordination::task_effort::EffortPassScope::RetryPending,
            ) {
                Ok(members) => summary.members_effort_resumed += members.len(),
                Err(err) => {
                    summary.team_errors += 1;
                    tracing::warn!(
                        team = %team_name,
                        error = %err,
                        "background task-effort pass failed"
                    );
                }
            }
        }

        Ok(summary)
    }

    /// Put a pending assignment effort into force for every member working in
    /// `project_path`, and report how many were switched.
    ///
    /// Called from the task scan that just persisted the project's tasks and
    /// rewrote the operational snapshots from them — the moment an assignment
    /// mesh wrote becomes visible to taurhaus, and the earliest a Codex member
    /// can be moved to the level it carries. `cli_commands` and `tmux_layout`
    /// are the operator's own launch settings, resolved by the caller at the
    /// command boundary the way every other managed launch resolves them: a
    /// relaunch must land on the same account and the same configured command
    /// as the launch it replaces.
    pub fn apply_task_effort_for_project(
        &self,
        project_path: &str,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<usize, CoordinationError> {
        let teams = self.teams_working_in_project(project_path)?;
        if teams.is_empty() {
            return Ok(0);
        }
        self.with_orchestrator(|orchestrator| {
            let mut switched = 0;
            for team_name in teams {
                match orchestrator.apply_pending_task_effort(
                    &team_name,
                    cli_commands,
                    tmux_layout,
                    crate::coordination::task_effort::EffortPassScope::TaskChanged,
                ) {
                    Ok(members) => switched += members.len(),
                    Err(err) => tracing::warn!(
                        team = %team_name,
                        error = %err,
                        "task-arrival effort pass failed"
                    ),
                }
            }
            Ok(switched)
        })
    }

    /// Teams with at least one member whose project is `project_path`.
    ///
    /// The task scan runs per project, so this is what keeps a change in one
    /// project from sweeping every team on the host.
    pub fn teams_working_in_project(
        &self,
        project_path: &str,
    ) -> Result<Vec<String>, CoordinationError> {
        let wanted = crate::provider::path::normalize_project_path(project_path);
        let mut teams = Vec::new();
        for team_name in TeamConfigStore::list(&self.teams_dir)? {
            let Ok(config) = TeamConfigStore::load(&self.teams_dir, &team_name) else {
                continue;
            };
            if config.members.iter().any(|member| {
                crate::provider::path::normalize_project_path(
                    &member.project_path.to_string_lossy(),
                ) == wanted
            }) {
                teams.push(team_name);
            }
        }
        Ok(teams)
    }

    fn build_orchestrator(&self) -> Result<CoordinationOrchestrator, CoordinationError> {
        let kind = self.backend_selector.select_floor();
        let backend = (self.backend_factory)(kind, &self.teams_dir)?;
        let runtime = (self.runtime_factory)();
        let mut orchestrator =
            CoordinationOrchestrator::new_with_runtime(self.teams_dir.clone(), backend, runtime);
        // Set dedicated Claude backend for per-member routing: Claude agents get
        // inbox file delivery instead of mesh send, fixing auth failures on
        // Claude-only teams.
        orchestrator.claude_backend =
            Some(Arc::new(ClaudeNativeBackend::new(self.teams_dir.clone())));
        if let Err(err) = orchestrator.reconcile_runtime_state_on_startup() {
            tracing::warn!(
                error = %err,
                teams_dir = %self.teams_dir.display(),
                "startup runtime reconciliation failed"
            );
        }
        match ensure_startup_claude_compact_hook(&self.teams_dir) {
            Ok(true) => {
                tracing::info!(
                    teams_dir = %self.teams_dir.display(),
                    "installed Claude compact hook during startup self-heal"
                );
            }
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    teams_dir = %self.teams_dir.display(),
                    "startup Claude compact hook ensure failed"
                );
            }
        }
        Ok(orchestrator)
    }

    fn build_background_orchestrator(&self) -> Result<CoordinationOrchestrator, CoordinationError> {
        let kind = self.backend_selector.select_floor();
        let backend = (self.backend_factory)(kind, &self.teams_dir)?;
        let runtime = (self.runtime_factory)();
        let mut orchestrator =
            CoordinationOrchestrator::new_with_runtime(self.teams_dir.clone(), backend, runtime);
        orchestrator.claude_backend =
            Some(Arc::new(ClaudeNativeBackend::new(self.teams_dir.clone())));
        Ok(orchestrator)
    }
}

fn ensure_startup_claude_compact_hook(
    teams_dir: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let has_managed_claude =
        TeamConfigStore::list(teams_dir)?
            .into_iter()
            .try_fold(false, |found, team_name| {
                if found {
                    return Ok(true);
                }
                team_has_managed_claude_member(teams_dir, &team_name)
            })?;
    if !has_managed_claude {
        return Ok(false);
    }

    let current_exe = std::env::current_exe().map_err(|err| {
        CoordinationError::Backend(format!(
            "failed to resolve taurhaus executable for startup Claude hook install: {err}"
        ))
    })?;
    ensure_compact_hook_installed(teams_dir, &current_exe)
}

fn default_backend_factory(
    kind: BackendKind,
    teams_dir: &Path,
) -> Result<Arc<dyn CoordinationBackend>, CoordinationError> {
    let backend: Arc<dyn CoordinationBackend> = match kind {
        BackendKind::MeshBridged => Arc::new(MeshBridgedBackend::new_with_teams_dir(
            teams_dir.to_path_buf(),
        )),
        BackendKind::ClaudeNative => Arc::new(ClaudeNativeBackend::new(teams_dir.to_path_buf())),
    };
    Ok(backend)
}

fn default_runtime_factory() -> Arc<dyn CoordinationRuntime> {
    Arc::new(SystemCoordinationRuntime)
}

fn apply_self_heal_result(summary: &mut BackgroundSelfHealPassResult, result: &TeamSelfHealResult) {
    if !result.runtime_candidate_found {
        summary.teams_skipped += 1;
        return;
    }

    if result.member_liveness_reconciled {
        summary.teams_reconciled += 1;
    }
    if result.team_daemon_ensured {
        summary.team_daemons_ensured += 1;
    }
}

#[cfg(test)]
mod tests {
    use fs2::FileExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

    use tempfile::TempDir;

    use super::*;
    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
    use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
    use crate::coordination::stores::{MemberRuntimeStore, TeamConfig, TeamConfigStore};

    const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
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

    fn fake_factory_with_counter(counter: Arc<AtomicUsize>) -> Arc<BackendFactory> {
        Arc::new(move |_kind, _teams_dir| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        })
    }

    fn sample_member(name: &str, role: MemberRole, tool: CliTool, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role,
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
            project_path: PathBuf::from(project_path),
            cli_tool: tool,
            extra: Default::default(),
        }
    }

    fn save_team_fixture(teams_dir: &std::path::Path, team_name: &str, members: Vec<Member>) {
        TeamConfigStore::save(
            teams_dir,
            team_name,
            &TeamConfig {
                schema_version: 1,
                name: team_name.to_string(),
                description: None,
                created_at: chrono::DateTime::parse_from_rfc3339("2026-03-08T16:30:00Z")
                    .expect("timestamp")
                    .with_timezone(&chrono::Utc),
                members,
                extra: Default::default(),
            },
        )
        .expect("team fixture saved");
    }

    fn write_lead_credential(teams_dir: &std::path::Path, team_name: &str) {
        let mut config = TeamConfigStore::load(teams_dir, team_name).expect("team config");
        let lead = config
            .members
            .iter_mut()
            .find(|member| member.role == MemberRole::Lead)
            .expect("lead member");
        lead.extra.insert(
            "controlAuthTokenHash".to_string(),
            serde_json::Value::String("sha256:test-token".to_string()),
        );
        lead.extra
            .insert("isActive".to_string(), serde_json::Value::Bool(true));
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save lead auth hash");
        let dir = teams_dir.join(team_name).join("state").join("control_auth");
        std::fs::create_dir_all(&dir).expect("credential dir");
        std::fs::write(
            dir.join("team-lead.json"),
            r#"{"name":"team-lead","token":"test-token"}"#,
        )
        .expect("lead credential");
    }

    #[derive(Debug, Default)]
    struct FlakyTeamDaemonRuntime {
        inner: RecordingCoordinationRuntime,
        fail_spawn_team_daemon_count: AtomicUsize,
    }

    impl FlakyTeamDaemonRuntime {
        fn new(failures: usize) -> Self {
            Self {
                inner: RecordingCoordinationRuntime::default(),
                fail_spawn_team_daemon_count: AtomicUsize::new(failures),
            }
        }

        fn calls(&self) -> Vec<RuntimeCall> {
            self.inner.calls()
        }

        fn set_pane_exists(&self, pane_id: &str, exists: bool) {
            self.inner.set_pane_exists(pane_id, exists);
        }

        fn set_pane_dead(&self, pane_id: &str, dead: bool) {
            self.inner.set_pane_dead(pane_id, dead);
        }

        fn set_pane_shell(&self, pane_id: &str, shell: bool) {
            self.inner.set_pane_shell(pane_id, shell);
        }

        fn set_pid_running(&self, pid: u32, running: bool) {
            self.inner.set_pid_running(pid, running);
        }

        fn set_pid_current_mesh_binary(&self, pid: u32, current: bool) {
            self.inner.set_pid_current_mesh_binary(pid, current);
        }

        fn set_team_daemon_current_mesh_binary(&self, team_name: &str, current: bool) {
            self.inner
                .set_team_daemon_current_mesh_binary(team_name, current);
        }
    }

    impl CoordinationRuntime for FlakyTeamDaemonRuntime {
        fn create_aitx_pane(
            &self,
            project_id: &str,
            tmux_layout: &str,
        ) -> Result<String, CoordinationError> {
            self.inner.create_aitx_pane(project_id, tmux_layout)
        }

        fn send_tmux_keys_with_enter(
            &self,
            pane_id: &str,
            keys: &str,
        ) -> Result<(), CoordinationError> {
            self.inner.send_tmux_keys_with_enter(pane_id, keys)
        }

        fn detect_session_id(
            &self,
            pane_id: &str,
            cli_tool: CliTool,
        ) -> Result<Option<String>, CoordinationError> {
            self.inner.detect_session_id(pane_id, cli_tool)
        }

        fn join_mesh(
            &self,
            team_name: &str,
            member_name: &str,
            project_id: &str,
            member_type: &str,
            model: &str,
            claude_dir: &str,
        ) -> Result<(), CoordinationError> {
            self.inner.join_mesh(
                team_name,
                member_name,
                project_id,
                member_type,
                model,
                claude_dir,
            )
        }

        fn spawn_mesh_daemon(
            &self,
            pane_id: &str,
            team_name: &str,
            member_name: &str,
        ) -> Result<u32, CoordinationError> {
            self.inner
                .spawn_mesh_daemon(pane_id, team_name, member_name)
        }

        fn spawn_team_daemon(
            &self,
            team_name: &str,
            operator_name: &str,
        ) -> Result<u32, CoordinationError> {
            if self.fail_spawn_team_daemon_count.load(Ordering::SeqCst) > 0 {
                self.inner.spawn_team_daemon(team_name, operator_name)?;
                self.fail_spawn_team_daemon_count
                    .fetch_sub(1, Ordering::SeqCst);
                return Err(CoordinationError::Backend(
                    "simulated team daemon restart failure".to_string(),
                ));
            }
            self.inner.spawn_team_daemon(team_name, operator_name)
        }

        fn find_existing_mesh_daemon_pids(
            &self,
            pane_id: &str,
            team_name: &str,
            member_name: &str,
        ) -> Result<Vec<u32>, CoordinationError> {
            self.inner
                .find_existing_mesh_daemon_pids(pane_id, team_name, member_name)
        }

        fn pane_belongs_to_project(
            &self,
            pane_id: &str,
            project_id: &str,
        ) -> Result<bool, CoordinationError> {
            self.inner.pane_belongs_to_project(pane_id, project_id)
        }

        fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
            self.inner.pane_exists(pane_id)
        }

        fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
            self.inner.pane_is_dead(pane_id)
        }

        fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
            self.inner.pane_is_shell(pane_id)
        }

        fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError> {
            self.inner.pane_current_command(pane_id)
        }

        fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
            self.inner.kill_aitx_pane(pane_id)
        }

        fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
            self.inner.terminate_process_by_pid(pid)
        }

        fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
            self.inner.is_process_running_by_pid(pid)
        }

        fn mesh_daemon_uses_current_binary(&self, pid: u32) -> Result<bool, CoordinationError> {
            self.inner.mesh_daemon_uses_current_binary(pid)
        }

        fn team_daemon_uses_current_binary(
            &self,
            team_name: &str,
        ) -> Result<bool, CoordinationError> {
            self.inner.team_daemon_uses_current_binary(team_name)
        }

        fn clear_mesh_daemon_pid_file(
            &self,
            team_name: &str,
            member_name: &str,
        ) -> Result<(), CoordinationError> {
            self.inner
                .clear_mesh_daemon_pid_file(team_name, member_name)
        }

        fn stop_team_daemon(&self, team_name: &str) -> Result<(), CoordinationError> {
            self.inner.stop_team_daemon(team_name)
        }
    }

    #[test]
    fn bootstrap_success_creates_orchestrator_on_first_use() {
        let tmp = TempDir::new().expect("tempdir");
        let counter = Arc::new(AtomicUsize::new(0));
        let state = CoordinationState::with_components(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            fake_factory_with_counter(counter.clone()),
        );

        let teams = state
            .with_orchestrator(|orch| {
                orch.create_team("architecture-final", None)?;
                orch.list_teams()
            })
            .expect("orchestrator operation should succeed");

        assert_eq!(teams, vec!["architecture-final".to_string()]);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn default_backend_delivery_uses_the_state_teams_dir() {
        // Regression: 694b130 gave MeshBridgedBackend its own PlatformPaths root,
        // so state-scoped delivery silently appended outside the owning store.
        let _guard = acquire_env_test_guard();
        let tmp = TempDir::new().expect("tempdir");
        let platform_claude_dir = tmp.path().join("platform-claude");
        let teams_dir = tmp.path().join("state-teams");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, &platform_claude_dir);
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = CoordinationState::with_components_and_runtime(
            teams_dir.clone(),
            BackendSelector::m0(),
            Arc::new(default_backend_factory),
            Arc::new(move || runtime.clone()),
        );

        let result = state.with_orchestrator(|orchestrator| {
            orchestrator.create_team("root-authority", None)?;
            orchestrator.add_member(
                "root-authority",
                sample_member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/project"),
            )?;
            orchestrator.deliver_message(DeliveryRequest::operator_notice(
                OperatorNoticeDelivery {
                    team_name: "root-authority".to_string(),
                    member_name: "builder".to_string(),
                    message: "status?".to_string(),
                    sender_name: None,
                    operational_context: None,
                },
            ))?;
            Ok(())
        });
        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);
        result.expect("delivery succeeds");

        assert!(
            teams_dir
                .join("root-authority")
                .join("inboxes")
                .join("builder.json")
                .is_file(),
            "delivery must append beneath CoordinationState::teams_dir"
        );
        assert!(
            !platform_claude_dir
                .join("teams")
                .join("root-authority")
                .join("inboxes")
                .join("builder.json")
                .exists(),
            "delivery must not resolve an independent platform root"
        );
    }

    #[test]
    fn bootstrap_failure_from_factory_is_propagated() {
        let state = CoordinationState::with_components(
            PathBuf::from("/tmp/teams"),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Err(CoordinationError::Backend(
                    "simulated backend factory failure".to_string(),
                ))
            }),
        );

        let err = state
            .with_orchestrator(|_| Ok(()))
            .expect_err("bootstrap should fail");
        match err {
            CoordinationError::Backend(message) => assert!(message.contains("simulated")),
            other => panic!("expected backend error, got {other:?}"),
        }
    }

    #[test]
    fn first_use_initializes_once_and_reuses_orchestrator() {
        let tmp = TempDir::new().expect("tempdir");
        let counter = Arc::new(AtomicUsize::new(0));
        let state = CoordinationState::with_components(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            fake_factory_with_counter(counter.clone()),
        );

        let first_ptr = state
            .with_orchestrator(|orch| Ok((orch as *mut CoordinationOrchestrator) as usize))
            .expect("first access");
        let second_ptr = state
            .with_orchestrator(|orch| Ok((orch as *mut CoordinationOrchestrator) as usize))
            .expect("second access");

        assert_eq!(
            first_ptr, second_ptr,
            "orchestrator instance should be reused"
        );
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "backend factory should run only once"
        );
    }

    #[test]
    fn startup_state_creation_is_non_blocking_even_if_backend_unavailable() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let state = CoordinationState::with_components(
            PathBuf::from("/tmp/teams"),
            BackendSelector::m0(),
            Arc::new(move |_kind, _teams_dir| {
                calls_clone.fetch_add(1, Ordering::SeqCst);
                Err(CoordinationError::Backend(
                    "mesh unavailable until first command".to_string(),
                ))
            }),
        );

        // State creation should not invoke backend checks.
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let err = state
            .with_orchestrator(|_| Ok(()))
            .expect_err("first command should surface backend unavailability");
        match err {
            CoordinationError::Backend(message) => assert!(message.contains("mesh unavailable")),
            other => panic!("expected backend error, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn startup_with_existing_claude_team_installs_compact_hook() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        save_team_fixture(
            &teams_dir,
            "architecture-final",
            vec![sample_member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead",
            )],
        );

        let state = CoordinationState::with_components(
            teams_dir.clone(),
            BackendSelector::m0(),
            fake_factory_with_counter(Arc::new(AtomicUsize::new(0))),
        );

        state
            .with_orchestrator(|_| Ok(()))
            .expect("bootstrap succeeds");

        let settings_raw =
            std::fs::read_to_string(tmp.path().join("settings.json")).expect("settings exists");
        assert!(
            settings_raw.contains("taurhaus-session-start-compact"),
            "startup should install the Claude compact hook for existing managed Claude teams"
        );
    }

    #[test]
    fn startup_with_existing_compact_hook_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        save_team_fixture(
            &teams_dir,
            "architecture-final",
            vec![sample_member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead",
            )],
        );
        let current_exe = std::env::current_exe().expect("current exe");
        let first_install =
            ensure_compact_hook_installed(&teams_dir, &current_exe).expect("first install");
        assert!(first_install);
        let before =
            std::fs::read_to_string(tmp.path().join("settings.json")).expect("settings before");

        let state = CoordinationState::with_components(
            teams_dir.clone(),
            BackendSelector::m0(),
            fake_factory_with_counter(Arc::new(AtomicUsize::new(0))),
        );

        state
            .with_orchestrator(|_| Ok(()))
            .expect("bootstrap succeeds");

        let after =
            std::fs::read_to_string(tmp.path().join("settings.json")).expect("settings after");
        assert_eq!(after, before, "startup hook ensure should be idempotent");
    }

    #[test]
    fn startup_with_no_managed_claude_teams_leaves_compact_hook_uninstalled() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        save_team_fixture(
            &teams_dir,
            "architecture-final",
            vec![sample_member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/app",
            )],
        );

        let state = CoordinationState::with_components(
            teams_dir.clone(),
            BackendSelector::m0(),
            fake_factory_with_counter(Arc::new(AtomicUsize::new(0))),
        );

        state
            .with_orchestrator(|_| Ok(()))
            .expect("bootstrap succeeds");

        assert!(
            !tmp.path().join("settings.json").exists(),
            "startup should not touch Claude hook settings when no managed Claude teams exist"
        );
    }

    #[test]
    fn platform_paths_teams_dir_uses_claude_override_when_set() {
        let _guard = acquire_env_test_guard();
        let override_dir = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, override_dir.path());
        let resolved = PlatformPaths::teams_dir();
        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);

        assert_eq!(resolved, override_dir.path().join("teams"));
    }

    #[test]
    fn platform_paths_teams_dir_ignores_empty_override() {
        let _guard = acquire_env_test_guard();
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, "");
        let resolved = PlatformPaths::teams_dir();
        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);

        assert!(resolved.ends_with(PathBuf::from(".claude").join("teams")));
    }

    #[test]
    fn trigger_team_self_heal_repairs_active_team() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("architecture-final", None)?;
                orch.add_member(
                    "architecture-final",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orch.add_member(
                    "architecture-final",
                    sample_member(
                        "existing-dev",
                        MemberRole::Agent,
                        CliTool::Codex,
                        "/tmp/app",
                    ),
                )?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "architecture-final");

        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "existing-dev")
                .expect("load runtime");
        runtime_record.pane_id = Some("%9".to_string());
        runtime_record.health = HealthState::SessionDead;
        runtime_record.daemon_pid = None;
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "existing-dev",
            &runtime_record,
        )
        .expect("save runtime");

        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_shell("%9", false);

        let result = state
            .trigger_team_self_heal("architecture-final")
            .expect("self-heal succeeds");

        assert!(result.runtime_candidate_found);
        assert!(result.member_liveness_reconciled);
        assert!(result.team_daemon_ensured);
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnDaemon {
                member_name,
                ..
            } if member_name == "existing-dev"
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnTeamDaemon {
                team_name,
                operator_name,
            } if team_name == "architecture-final" && operator_name == "team-lead"
        )));
    }

    /// Put a member on an active task carrying `level`, the way the
    /// operational snapshot sync does once mesh has written the assignment
    /// onto the task record.
    fn assign_task(teams_dir: &std::path::Path, team_name: &str, member_name: &str, level: &str) {
        use crate::coordination::stores::{
            OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
            OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
            OperationalWorkingSetSnapshot,
        };

        OperationalContextSnapshotStore::save(
            teams_dir,
            &OperationalContextSnapshot {
                version: 1,
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                updated_at: Utc::now(),
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Run the migration".to_string(),
                    status: "in_progress".to_string(),
                    ..Default::default()
                },
                assignment_footer: OperationalAssignmentFooterSnapshot {
                    task_effort: level.to_string(),
                    task_effort_why: "the migration is irreversible".to_string(),
                    ..Default::default()
                },
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "/tmp/app".to_string(),
                    focal_files: vec![],
                },
            },
        )
        .expect("write operational snapshot");
    }

    #[test]
    fn a_task_change_puts_a_pending_assignment_effort_into_force() {
        // The lead's per-assignment effort reaches a Claude, Antigravity or
        // Grok member through mesh's own `/effort`, before the notice. Codex
        // has no such command, so taurhaus resumes it — from the task scan that
        // made the assignment visible, which is the earliest taurhaus can act.
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_detected_runtime_session(
            "%31",
            CliTool::Codex,
            Some("session-effort"),
            Some("/tmp/effort.jsonl"),
        );
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("effort-team", None)?;
                orch.add_member(
                    "effort-team",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                let mut builder =
                    sample_member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app");
                builder.reasoning_effort = Some("low".to_string());
                orch.add_member("effort-team", builder)?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "effort-team");

        crate::coordination::stores::MemberRuntimeStore::update(
            tmp.path(),
            "effort-team",
            "builder",
            |record| {
                record.pane_id = Some("%31".to_string());
                record.health = HealthState::Healthy;
                record.applied_effort = Some("low".to_string());
                record.session_id = Some("session-effort".to_string());
            },
        )
        .expect("seed runtime");
        runtime.set_pane_exists("%31", true);
        runtime.set_pane_dead("%31", false);

        assign_task(tmp.path(), "effort-team", "builder", "high");

        state.orchestrator.lock().expect("state mutex").take();

        let resumed = state
            .apply_task_effort_for_project(
                "/tmp/app",
                &CliCommandSettings::default(),
                DEFAULT_TMUX_LAYOUT,
            )
            .expect("task-arrival pass succeeds");

        assert_eq!(resumed, 1);
        let record = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "effort-team",
            "builder",
        )
        .expect("runtime record");
        assert_eq!(record.applied_effort.as_deref(), Some("high"));
    }

    // Regression: 2529309 started every effort switch from the 30 s self-heal
    // pass, so a Codex member could read a whole assignment at its previous
    // level before the timer came round. The switch belongs on the task event
    // that made the assignment visible; the timer only retries one that failed.
    #[test]
    fn the_background_pass_starts_no_switch_of_its_own() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_detected_runtime_session(
            "%31",
            CliTool::Codex,
            Some("session-effort"),
            Some("/tmp/effort.jsonl"),
        );
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("effort-team", None)?;
                orch.add_member(
                    "effort-team",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                let mut builder =
                    sample_member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app");
                builder.reasoning_effort = Some("low".to_string());
                orch.add_member("effort-team", builder)?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "effort-team");

        crate::coordination::stores::MemberRuntimeStore::update(
            tmp.path(),
            "effort-team",
            "builder",
            |record| {
                record.pane_id = Some("%31".to_string());
                record.health = HealthState::Healthy;
                record.applied_effort = Some("low".to_string());
                record.session_id = Some("session-effort".to_string());
            },
        )
        .expect("seed runtime");
        runtime.set_pane_exists("%31", true);
        runtime.set_pane_dead("%31", false);

        assign_task(tmp.path(), "effort-team", "builder", "high");

        state.orchestrator.lock().expect("state mutex").take();

        let summary = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("background pass succeeds");

        assert_eq!(
            summary.members_effort_resumed, 0,
            "a switch nothing has attempted yet is the task event's to start"
        );
        let record = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "effort-team",
            "builder",
        )
        .expect("runtime record");
        assert_eq!(record.applied_effort.as_deref(), Some("low"));
    }

    // Regression: 2529309 ran the effort relaunch with
    // `CliCommandSettings::default()`, so a member launched on a selected
    // account came back on the tool's default one, under the stock command.
    #[test]
    fn an_effort_relaunch_uses_the_operators_own_launch_settings() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = TempDir::new().expect("codex home");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_detected_runtime_session(
            "%31",
            CliTool::Codex,
            Some("session-effort"),
            Some("/tmp/effort.jsonl"),
        );
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("effort-team", None)?;
                orch.add_member(
                    "effort-team",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                let mut builder =
                    sample_member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app");
                builder.reasoning_effort = Some("low".to_string());
                orch.add_member("effort-team", builder)?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "effort-team");

        crate::coordination::stores::MemberRuntimeStore::update(
            tmp.path(),
            "effort-team",
            "builder",
            |record| {
                record.pane_id = Some("%31".to_string());
                record.health = HealthState::Healthy;
                record.applied_effort = Some("low".to_string());
                record.session_id = Some("session-effort".to_string());
            },
        )
        .expect("seed runtime");
        runtime.set_pane_exists("%31", true);
        runtime.set_pane_dead("%31", false);

        assign_task(tmp.path(), "effort-team", "builder", "high");

        state.orchestrator.lock().expect("state mutex").take();

        let mut cli_commands = CliCommandSettings::default();
        cli_commands
            .account_selector_dirs
            .insert("CODEX_HOME".to_string(), codex_home.path().to_path_buf());
        let resumed = state
            .apply_task_effort_for_project("/tmp/app", &cli_commands, DEFAULT_TMUX_LAYOUT)
            .expect("task-arrival pass succeeds");

        assert_eq!(resumed, 1);
        let launch = runtime
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys),
                _ => None,
            })
            .rfind(|keys| keys.contains("codex"))
            .expect("a codex launch was sent to the pane");
        assert!(
            launch.contains("CODEX_HOME=")
                && launch.contains(&codex_home.path().display().to_string()),
            "the effort relaunch must keep the operator's account, got: {launch}"
        );
    }

    #[test]
    fn background_self_heal_pass_skips_inactive_teams() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("idle-team", None)?;
                orch.add_member(
                    "idle-team",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orch.add_member(
                    "idle-team",
                    sample_member(
                        "existing-dev",
                        MemberRole::Agent,
                        CliTool::Codex,
                        "/tmp/app",
                    ),
                )?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "idle-team");

        state.orchestrator.lock().expect("state mutex").take();

        let summary = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("background pass succeeds");

        assert_eq!(summary.teams_scanned, 1);
        assert_eq!(summary.teams_skipped, 1);
        assert_eq!(summary.teams_reconciled, 0);
        assert_eq!(summary.team_daemons_ensured, 0);
        assert_eq!(summary.team_errors, 0);
        assert!(
            runtime.calls().iter().all(|call| matches!(
                call,
                RuntimeCall::CheckTeamDaemonCurrentMeshBinary { team_name }
                    if team_name == "idle-team"
            )),
            "inactive team should only perform the cheap team-daemon identity probe"
        );
        assert!(
            state.orchestrator.lock().expect("state mutex").is_none(),
            "background self-heal should not repopulate the shared orchestrator"
        );
    }

    #[test]
    fn background_self_heal_pass_cycles_stale_team_daemon_for_active_team() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("architecture-final", None)?;
                orch.add_member(
                    "architecture-final",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orch.add_member(
                    "architecture-final",
                    sample_member(
                        "existing-dev",
                        MemberRole::Agent,
                        CliTool::Codex,
                        "/tmp/app",
                    ),
                )?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "architecture-final");

        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "existing-dev")
                .expect("load runtime");
        runtime_record.pane_id = Some("%9".to_string());
        runtime_record.health = HealthState::Healthy;
        runtime_record.session_id = Some("session-123".to_string());
        runtime_record.daemon_pid = Some(4242);
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "existing-dev",
            &runtime_record,
        )
        .expect("save runtime");

        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_shell("%9", false);
        runtime.set_pid_running(4242, true);
        runtime.set_pid_current_mesh_binary(4242, false);
        runtime.set_team_daemon_current_mesh_binary("architecture-final", false);

        let summary = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("background pass succeeds");

        assert_eq!(summary.teams_scanned, 1);
        assert_eq!(summary.teams_skipped, 0);
        assert_eq!(summary.teams_reconciled, 1);
        assert_eq!(summary.team_daemons_ensured, 1);
        assert_eq!(summary.team_errors, 0);
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::CheckTeamDaemonCurrentMeshBinary { team_name }
                if team_name == "architecture-final"
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::StopTeamDaemon { team_name } if team_name == "architecture-final"
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnTeamDaemon {
                team_name,
                operator_name,
            } if team_name == "architecture-final" && operator_name == "team-lead"
        )));
    }

    #[test]
    fn background_self_heal_upgrade_cycle_restores_delivery_after_daemon_rotation() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let fake = FakeBackend::default();
        let fake_for_factory = fake.clone();
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(move |_kind, _teams_dir| {
                Ok(Arc::new(fake_for_factory.clone()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("architecture-final", None)?;
                orch.add_member(
                    "architecture-final",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orch.add_member(
                    "architecture-final",
                    sample_member(
                        "existing-dev",
                        MemberRole::Agent,
                        CliTool::Codex,
                        "/tmp/app",
                    ),
                )?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "architecture-final");

        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "existing-dev")
                .expect("load runtime");
        runtime_record.pane_id = Some("%9".to_string());
        runtime_record.health = HealthState::Healthy;
        runtime_record.session_id = Some("session-123".to_string());
        runtime_record.daemon_pid = Some(4242);
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "existing-dev",
            &runtime_record,
        )
        .expect("save runtime");

        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_shell("%9", false);
        runtime.set_pid_running(4242, true);
        runtime.set_pid_current_mesh_binary(4242, false);
        runtime.set_team_daemon_current_mesh_binary("architecture-final", false);

        let summary = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("self-heal succeeds");
        assert_eq!(summary.teams_reconciled, 1);
        assert_eq!(summary.team_daemons_ensured, 1);

        state
            .with_orchestrator(|orch| {
                orch.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                    team_name: "architecture-final".to_string(),
                    member_name: "existing-dev".to_string(),
                    message: "post-upgrade ping".to_string(),
                    sender_name: Some("team-lead".to_string()),
                    operational_context: None,
                }))
            })
            .expect("delivery after upgrade cycle");

        let delivered = fake.delivered_requests();
        assert_eq!(delivered.len(), 1);
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::TerminatePid { pid } if *pid == 4242
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnDaemon { member_name, .. } if member_name == "existing-dev"
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::StopTeamDaemon { team_name } if team_name == "architecture-final"
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnTeamDaemon { team_name, .. } if team_name == "architecture-final"
        )));
    }

    #[test]
    fn background_self_heal_retries_team_daemon_recovery_after_previous_restart_failure() {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(FlakyTeamDaemonRuntime::new(1));
        let state = CoordinationState::with_components_and_runtime(
            tmp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );

        state
            .with_orchestrator(|orch| {
                orch.create_team("architecture-final", None)?;
                orch.add_member(
                    "architecture-final",
                    sample_member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orch.add_member(
                    "architecture-final",
                    sample_member(
                        "existing-dev",
                        MemberRole::Agent,
                        CliTool::Codex,
                        "/tmp/app",
                    ),
                )?;
                Ok(())
            })
            .expect("seed team");
        write_lead_credential(tmp.path(), "architecture-final");

        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "existing-dev")
                .expect("load runtime");
        runtime_record.pane_id = Some("%9".to_string());
        runtime_record.health = HealthState::Healthy;
        runtime_record.session_id = Some("session-123".to_string());
        runtime_record.daemon_pid = Some(4242);
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "existing-dev",
            &runtime_record,
        )
        .expect("save runtime");

        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_shell("%9", false);
        runtime.set_pid_running(4242, true);
        runtime.set_pid_current_mesh_binary(4242, false);
        runtime.set_team_daemon_current_mesh_binary("architecture-final", false);

        let first = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("first self-heal pass");
        let second = state
            .run_background_self_heal_pass(&CliCommandSettings::default(), DEFAULT_TMUX_LAYOUT)
            .expect("second self-heal pass");

        assert_eq!(first.teams_scanned, 1);
        assert_eq!(second.teams_scanned, 1);
        assert!(
            runtime.calls().iter().filter(|call| matches!(
                call,
                RuntimeCall::SpawnTeamDaemon { team_name, .. } if team_name == "architecture-final"
            )).count() >= 2,
            "recovery should attempt team-daemon spawn again on the next self-heal pass"
        );
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::StopTeamDaemon { team_name } if team_name == "architecture-final"
        )));
    }
}
