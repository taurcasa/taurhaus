use super::*;

impl TemplateStore {
    pub fn maybe_flush_pending_commits(&self) -> Result<Option<String>, TemplateStoreError> {
        self.flush_pending_commits_internal(false, false)
    }

    pub fn flush_pending_commits_on_shutdown(&self) -> Result<Option<String>, TemplateStoreError> {
        self.flush_pending_commits_internal(true, true)
    }

    pub fn flush_pending_commits(&self) -> Result<Option<String>, TemplateStoreError> {
        self.flush_pending_commits_internal(true, false)
    }

    pub fn load_state(&self) -> Result<TemplateStoreState, TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;
        self.load_state_unlocked()
    }

    pub fn save_state(&self, state: &TemplateStoreState) -> Result<(), TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;
        self.save_state_unlocked(state)
    }

    pub(super) fn load_state_unlocked(&self) -> Result<TemplateStoreState, TemplateStoreError> {
        let path = self.state_path();
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(TemplateStoreState::default());
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        };

        serde_json::from_str::<TemplateStoreState>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse {}: {err}", path.display()))
        })
    }

    pub(super) fn save_state_unlocked(
        &self,
        state: &TemplateStoreState,
    ) -> Result<(), TemplateStoreError> {
        let payload = serde_json::to_vec_pretty(state).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize state: {err}"))
        })?;
        write_atomic_file(&self.state_path(), &payload)
    }

    fn flush_pending_commits_internal(
        &self,
        force: bool,
        shutdown_mode: bool,
    ) -> Result<Option<String>, TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;

        if !self.git_dir().exists() {
            return Ok(None);
        }

        let repo = match Repository::open(self.templates_dir()) {
            Ok(repo) => repo,
            Err(err) => {
                tracing::warn!(
                    templates_dir = %self.templates_dir.display(),
                    error = %err,
                    "skipping pending template flush because repository could not be opened"
                );
                return Ok(None);
            }
        };

        let mut persisted = self.load_state_unlocked()?;
        let mut debounce = DebounceState::from_store(persisted.clone(), self.debounce_window_secs);
        let commit_id = self.flush_debounce_if_needed_with_repo(
            &repo,
            &mut debounce,
            force,
            shutdown_mode,
            current_timestamp(),
        )?;

        persisted.pending_actions = debounce.pending_actions;
        if commit_id.is_some() {
            persisted.last_commit_at = Some(current_timestamp());
        }
        persisted.repo_initialized = true;
        self.save_state_unlocked(&persisted)?;
        Ok(commit_id)
    }
}
