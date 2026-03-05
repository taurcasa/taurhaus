use super::*;

impl TemplateStore {
    pub fn ensure_repo_for_mutation(&self) -> Result<Option<Repository>, TemplateStoreError> {
        self.ensure_directories()?;
        self.seed_builtins_if_missing()?;

        if self.git_dir().exists() {
            match Repository::open(self.templates_dir()) {
                Ok(repo) => {
                    self.ensure_gitignore()?;
                    return Ok(Some(repo));
                }
                Err(err) => {
                    tracing::warn!(
                        templates_dir = %self.templates_dir.display(),
                        error = %err,
                        "template git repo exists but failed to open; falling back to plain filesystem"
                    );
                    return Ok(None);
                }
            }
        }

        match Repository::init(self.templates_dir()) {
            Ok(repo) => {
                self.ensure_gitignore()?;
                Ok(Some(repo))
            }
            Err(err) => {
                tracing::warn!(
                    templates_dir = %self.templates_dir.display(),
                    error = %err,
                    "template git init failed; continuing in plain filesystem mode"
                );
                Ok(None)
            }
        }
    }

    pub fn recover_dirty_tree(&self) -> Result<Option<String>, TemplateStoreError> {
        self.ensure_directories()?;

        if !self.git_dir().exists() {
            return Ok(None);
        }

        let _lock = self.acquire_lock()?;
        let repo = match Repository::open(self.templates_dir()) {
            Ok(repo) => repo,
            Err(err) => {
                tracing::warn!(
                    templates_dir = %self.templates_dir.display(),
                    error = %err,
                    "template recovery skipped because repository could not be opened"
                );
                return Ok(None);
            }
        };

        let changes = self.collect_managed_changes(&repo)?;
        if changes.is_empty() {
            return Ok(None);
        }

        // Ensure current tree remains schema-valid before auto-commit.
        let _ = self.load_catalog()?;

        let result = self.commit_with_repo(&repo, &changes, RECOVERY_COMMIT_MESSAGE);
        match result {
            Ok(Some(oid)) => Ok(Some(oid.to_string())),
            Ok(None) => Ok(None),
            Err(err) => {
                tracing::warn!(
                    templates_dir = %self.templates_dir.display(),
                    error = %err,
                    "template recovery commit failed; keeping filesystem state"
                );
                Ok(None)
            }
        }
    }

    pub fn write_template_file(
        &self,
        relative_path: &Path,
        contents: &[u8],
    ) -> Result<(), TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;

        let relative = normalize_relative_path(relative_path)?;
        if !is_managed_template_path(&relative) {
            return Err(TemplateStoreError::InvalidTemplatePath(format!(
                "{} is outside managed template scope",
                relative.display()
            )));
        }

        let target = self.templates_dir().join(relative);
        write_atomic_file(&target, contents)
    }

    pub fn commit_paths(
        &self,
        changed_paths: &[PathBuf],
        message: &str,
    ) -> Result<Option<String>, TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;
        self.commit_paths_unlocked(changed_paths, message)
    }

    pub fn mutate_and_commit(
        &self,
        mutations: &[TemplateFileMutation],
        message: &str,
    ) -> Result<Option<String>, TemplateStoreError> {
        self.ensure_directories()?;
        let _lock = self.acquire_lock()?;

        if mutations.is_empty() {
            return Ok(None);
        }

        let mut changed_paths = Vec::with_capacity(mutations.len());
        for mutation in mutations {
            let relative = normalize_relative_path(&mutation.relative_path)?;
            if !is_managed_template_path(&relative) {
                return Err(TemplateStoreError::InvalidTemplatePath(format!(
                    "{} is outside managed template scope",
                    relative.display()
                )));
            }

            let target = self.templates_dir().join(&relative);
            match mutation.contents.as_ref() {
                Some(contents) => write_atomic_file(&target, contents)?,
                None => {
                    if target.exists() {
                        fs::remove_file(&target)?;
                    }
                }
            }
            changed_paths.push(relative);
        }

        self.commit_paths_unlocked(&changed_paths, message)
    }

    fn commit_paths_unlocked(
        &self,
        changed_paths: &[PathBuf],
        message: &str,
    ) -> Result<Option<String>, TemplateStoreError> {
        let Some(repo) = self.ensure_repo_for_mutation()? else {
            return Ok(None);
        };
        let mut normalized_paths = Vec::new();
        for path in changed_paths {
            let relative = normalize_relative_path(path)?;
            if !is_managed_template_path(&relative) {
                return Err(TemplateStoreError::InvalidTemplatePath(format!(
                    "{} is outside managed template scope",
                    relative.display()
                )));
            }
            normalized_paths.push(relative);
        }

        if normalized_paths.is_empty() {
            return Ok(None);
        }

        let now_ts = current_timestamp();
        let mut persisted_state = self.load_state_unlocked()?;
        let mut state =
            DebounceState::from_store(persisted_state.clone(), self.debounce_window_secs);

        let stale_commit =
            self.flush_debounce_if_needed_with_repo(&repo, &mut state, false, false, now_ts)?;

        let descriptor = parse_mutation_descriptor(message).unwrap_or_else(|| MutationDescriptor {
            action: "update".to_string(),
            kind: "template".to_string(),
            id: "unknown".to_string(),
        });
        state.enqueue(descriptor, &normalized_paths, now_ts);

        let followup_commit =
            self.flush_debounce_if_needed_with_repo(&repo, &mut state, false, false, now_ts)?;

        persisted_state.pending_actions = state.pending_actions;
        if stale_commit.is_some() || followup_commit.is_some() {
            persisted_state.last_commit_at = Some(now_ts);
        }
        persisted_state.repo_initialized = true;
        self.save_state_unlocked(&persisted_state)?;
        Ok(followup_commit.or(stale_commit))
    }

    pub(super) fn flush_debounce_if_needed_with_repo(
        &self,
        repo: &Repository,
        debounce: &mut DebounceState,
        force: bool,
        shutdown_mode: bool,
        now_ts: i64,
    ) -> Result<Option<String>, TemplateStoreError> {
        if debounce.is_empty() {
            return Ok(None);
        }

        if !force && !debounce.should_flush_lazy(now_ts) {
            return Ok(None);
        }

        if let Err(err) = self.load_catalog() {
            tracing::warn!(
                templates_dir = %self.templates_dir.display(),
                error = %err,
                "template pending commit skipped due to pre-commit schema validation failure"
            );
            return Ok(None);
        }

        let mut changes = Vec::new();
        for path in debounce.take_changed_paths() {
            let relative = normalize_relative_path(&path)?;
            let absolute = self.templates_dir().join(&relative);
            changes.push(PathChange {
                path: relative,
                deleted: !absolute.exists(),
            });
        }

        if changes.is_empty() {
            debounce.pending_actions.clear();
            return Ok(None);
        }

        let message = if shutdown_mode {
            debounce.shutdown_message()
        } else {
            debounce.commit_message()
        };

        match self.commit_with_repo(repo, &changes, &message) {
            Ok(Some(oid)) => {
                debounce.pending_actions.clear();
                Ok(Some(oid.to_string()))
            }
            Ok(None) => Ok(None),
            Err(err) => {
                tracing::warn!(
                    templates_dir = %self.templates_dir.display(),
                    error = %err,
                    "template commit skipped; persisted YAML remains source of truth"
                );
                Ok(None)
            }
        }
    }

    pub(super) fn acquire_lock(&self) -> Result<TemplateStoreLockGuard, TemplateStoreError> {
        let lock_path = self.templates_dir().join(LOCK_FILENAME);
        let fallback_lock_path = self.templates_dir().join(LOCK_FALLBACK_FILENAME);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;

        if should_force_fallback_lock_for_tests() {
            tracing::warn!(
                lock_path = %lock_path.display(),
                fallback_lock_path = %fallback_lock_path.display(),
                "forcing template fallback lock in test mode"
            );
            return acquire_fallback_lock(&fallback_lock_path)
                .map(TemplateStoreLockGuard::fallback);
        }

        match file.lock_exclusive() {
            Ok(()) => Ok(TemplateStoreLockGuard::advisory(file)),
            Err(err) if is_windows_unsupported_lock_error(&err) => {
                tracing::warn!(
                    lock_path = %lock_path.display(),
                    fallback_lock_path = %fallback_lock_path.display(),
                    "advisory file locks unsupported at template path; switching to fallback lockfile"
                );
                acquire_fallback_lock(&fallback_lock_path).map(TemplateStoreLockGuard::fallback)
            }
            Err(err) => Err(TemplateStoreError::Io(err)),
        }
    }

    fn collect_managed_changes(
        &self,
        repo: &Repository,
    ) -> Result<Vec<PathChange>, TemplateStoreError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        let statuses = repo.statuses(Some(&mut opts))?;
        let mut by_path: BTreeMap<PathBuf, PathChange> = BTreeMap::new();

        for entry in statuses.iter() {
            let Some(path_str) = entry.path() else {
                continue;
            };
            let rel = PathBuf::from(path_str);
            if !is_managed_template_path(&rel) {
                continue;
            }

            let status = entry.status();
            let deleted = is_deleted_status(status);
            by_path.insert(rel.clone(), PathChange { path: rel, deleted });
        }

        Ok(by_path.into_values().collect())
    }

    fn commit_with_repo(
        &self,
        repo: &Repository,
        changes: &[PathChange],
        message: &str,
    ) -> Result<Option<Oid>, TemplateStoreError> {
        if changes.is_empty() {
            return Ok(None);
        }

        let mut index = repo.index()?;
        for change in changes {
            if change.deleted {
                let _ = index.remove_path(&change.path);
            } else {
                index.add_path(&change.path)?;
            }
        }

        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let sig = resolve_signature(repo)?;
        let mut parents = Vec::new();
        if let Ok(head) = repo.head() {
            if let Ok(parent) = head.peel_to_commit() {
                parents.push(parent);
            }
        }
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;
        Ok(Some(oid))
    }
}
