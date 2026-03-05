//! Shared coordination app state with lazy orchestrator bootstrap.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::coordination::backend::{
    BackendKind, BackendSelector, ClaudeNativeBackend, CoordinationBackend, MeshBridgedBackend,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::session_scanner::cli_tool::CliTool;

type BackendFactory =
    dyn Fn(BackendKind) -> Result<Arc<dyn CoordinationBackend>, CoordinationError> + Send + Sync;
type RuntimeFactory = dyn Fn() -> Arc<dyn CoordinationRuntime> + Send + Sync;
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

/// App-managed coordination state that lazily initializes the orchestrator.
pub struct CoordinationState {
    teams_dir: PathBuf,
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
            default_teams_dir(),
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
        Self {
            teams_dir,
            backend_selector,
            backend_factory,
            runtime_factory,
            orchestrator: Mutex::new(None),
        }
    }

    pub fn teams_dir(&self) -> &PathBuf {
        &self.teams_dir
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

    fn build_orchestrator(&self) -> Result<CoordinationOrchestrator, CoordinationError> {
        let kind = self.backend_selector.select(CliTool::Codex);
        let backend = (self.backend_factory)(kind)?;
        let runtime = (self.runtime_factory)();
        let mut orchestrator =
            CoordinationOrchestrator::new_with_runtime(self.teams_dir.clone(), backend, runtime);
        if let Err(err) = orchestrator.reconcile_runtime_state_on_startup() {
            tracing::warn!(
                error = %err,
                teams_dir = %self.teams_dir.display(),
                "startup runtime reconciliation failed"
            );
        }
        Ok(orchestrator)
    }
}

fn default_backend_factory(
    kind: BackendKind,
) -> Result<Arc<dyn CoordinationBackend>, CoordinationError> {
    let backend: Arc<dyn CoordinationBackend> = match kind {
        BackendKind::MeshBridged => Arc::new(MeshBridgedBackend::default()),
        BackendKind::ClaudeNative => Arc::new(ClaudeNativeBackend),
    };
    Ok(backend)
}

fn default_runtime_factory() -> Arc<dyn CoordinationRuntime> {
    Arc::new(SystemCoordinationRuntime)
}

fn default_teams_dir() -> PathBuf {
    if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
        if !path.is_empty() {
            return PathBuf::from(path).join("teams");
        }
    }
    if let Some(path) = mesh_cli::resolve_windows_mesh_teams_dir() {
        return path;
    }
    let base = if let Some(home_dir) = dirs::home_dir() {
        home_dir
    } else {
        let fallback = std::env::temp_dir().join("taurhaus-home");
        tracing::warn!(
            fallback = %fallback.display(),
            "home directory unavailable; falling back to temp directory for coordination teams path"
        );
        fallback
    };
    base.join(".claude").join("teams")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::LazyLock;
    use std::sync::{Arc, Mutex};

    use tempfile::TempDir;

    use super::*;
    use crate::coordination::backend::fake::FakeBackend;

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn fake_factory_with_counter(counter: Arc<AtomicUsize>) -> Arc<BackendFactory> {
        Arc::new(move |_kind| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        })
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
    fn bootstrap_failure_from_factory_is_propagated() {
        let state = CoordinationState::with_components(
            PathBuf::from("/tmp/teams"),
            BackendSelector::m0(),
            Arc::new(|_kind| {
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
            Arc::new(move |_kind| {
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
    fn default_teams_dir_uses_claude_override_when_set() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let override_dir = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, override_dir.path());
        let resolved = default_teams_dir();
        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);

        assert_eq!(resolved, override_dir.path().join("teams"));
    }

    #[test]
    fn default_teams_dir_ignores_empty_override() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, "");
        let resolved = default_teams_dir();
        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);

        assert!(resolved.ends_with(PathBuf::from(".claude").join("teams")));
    }
}
