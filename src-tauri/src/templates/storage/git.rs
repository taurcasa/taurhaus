use super::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use git2::{Repository, Sort, StatusOptions};

use crate::git::commits::{get_commit_diff, get_commit_files};

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

    pub fn managed_dirty_status(&self) -> Result<bool, TemplateStoreError> {
        self.ensure_directories()?;
        if !self.git_dir().exists() {
            return Ok(false);
        }

        let repo = match Repository::open(self.templates_dir()) {
            Ok(repo) => repo,
            Err(_) => return Ok(false),
        };

        match has_managed_dirty_status(&repo) {
            Ok(dirty) => Ok(dirty),
            Err(_) => Ok(false),
        }
    }

    pub fn get_history(
        &self,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<TemplateCommitPage, TemplateStoreError> {
        self.ensure_directories()?;
        if !self.git_dir().exists() {
            return Ok(TemplateCommitPage {
                commits: Vec::new(),
                next_cursor: None,
            });
        }

        let repo = Repository::open(self.templates_dir())?;
        let mut revwalk = repo.revwalk()?;
        if revwalk.push_head().is_err() {
            return Ok(TemplateCommitPage {
                commits: Vec::new(),
                next_cursor: None,
            });
        }
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

        let max = limit.unwrap_or(50).clamp(1, 200);
        let mut commits = Vec::new();
        let mut can_collect = cursor.is_none();

        for oid_result in revwalk {
            let oid = oid_result?;
            let full = oid.to_string();

            if !can_collect {
                if let Some(target) = cursor.as_deref() {
                    if full.starts_with(target) {
                        can_collect = true;
                    }
                }
                continue;
            }

            if commits.len() >= max {
                break;
            }

            let commit = repo.find_commit(oid)?;
            let changed_paths = commit_changed_template_paths(&repo, &commit)?;
            if changed_paths.is_empty() {
                continue;
            }

            commits.push(TemplateCommit {
                short_id: format!("{oid:.8}"),
                commit_id: full,
                message: commit.summary().unwrap_or("").to_string(),
                author: commit.author().name().unwrap_or("unknown").to_string(),
                timestamp: commit.time().seconds(),
                changed_paths,
            });
        }

        let next_cursor = if commits.len() == max {
            commits.last().map(|commit| commit.commit_id.clone())
        } else {
            None
        };

        Ok(TemplateCommitPage {
            commits,
            next_cursor,
        })
    }

    pub fn get_diff(&self, commit_id: &str) -> Result<TemplateDiff, TemplateStoreError> {
        self.ensure_directories()?;

        let files = get_commit_files(self.templates_dir(), commit_id)
            .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;

        let mut out_files = Vec::new();
        let mut insertions = 0u32;
        let mut deletions = 0u32;

        for file in files {
            if !is_managed_template_str_path(file.path.as_str()) {
                continue;
            }

            let hunks = get_commit_diff(self.templates_dir(), commit_id, &file.path)
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;

            for hunk in &hunks {
                for line in &hunk.lines {
                    if line.origin == '+' {
                        insertions = insertions.saturating_add(1);
                    } else if line.origin == '-' {
                        deletions = deletions.saturating_add(1);
                    }
                }
            }

            out_files.push(TemplateDiffFile {
                path: file.path,
                status: file.status,
                hunks,
            });
        }

        let stats = TemplateDiffStats {
            files_changed: out_files.len() as u32,
            insertions,
            deletions,
        };

        Ok(TemplateDiff {
            commit_id: commit_id.to_string(),
            files: out_files,
            stats,
        })
    }

    pub fn revert_template(&self, id: &str, commit_hash: &str) -> Result<(), TemplateStoreError> {
        if !is_valid_template_id(id) {
            return Err(TemplateStoreError::Validation(
                "invalid template id".to_string(),
            ));
        }

        self.ensure_directories()?;

        let repo = Repository::open(self.templates_dir())?;
        let object = repo.revparse_single(commit_hash)?;
        let commit = object.peel_to_commit()?;
        let tree = commit.tree()?;

        let candidates = [format!("roles/{id}.yaml"), format!("presets/{id}.yaml")];

        let mut touched = Vec::new();
        let mut mutations = Vec::new();
        for rel in candidates {
            let rel_path = PathBuf::from(&rel);
            if let Ok(entry) = tree.get_path(Path::new(&rel)) {
                let obj = entry.to_object(&repo)?;
                if let Some(blob) = obj.as_blob() {
                    mutations.push(TemplateFileMutation::write(
                        rel_path.clone(),
                        blob.content().to_vec(),
                    ));
                    touched.push(rel_path);
                }
                continue;
            }

            let abs = self.templates_dir().join(&rel_path);
            if abs.exists() {
                mutations.push(TemplateFileMutation::delete(rel_path.clone()));
                touched.push(rel_path);
            }
        }

        if touched.is_empty() {
            return Err(TemplateStoreError::NotFound(format!(
                "template '{id}' not found in commit"
            )));
        }

        let short = format!("{:.8}", commit.id());
        let _ = self.mutate_and_commit(
            &mutations,
            &format!("templates: revert template {id} to {short}"),
        )?;
        let _ = self.flush_pending_commits()?;
        Ok(())
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

fn is_valid_template_id(id: &str) -> bool {
    !id.trim().is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_managed_template_str_path(path: &str) -> bool {
    is_managed_template_path(Path::new(path))
}

fn has_managed_dirty_status(repo: &Repository) -> Result<bool, git2::Error> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(statuses.iter().any(|entry| {
        entry
            .path()
            .map(is_managed_template_str_path)
            .unwrap_or(false)
    }))
}

fn commit_changed_template_paths(
    repo: &Repository,
    commit: &git2::Commit<'_>,
) -> Result<Vec<String>, git2::Error> {
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

    let mut paths = BTreeSet::new();
    diff.foreach(
        &mut |delta, _| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(|path| path.to_str());
            if let Some(path) = path {
                if is_managed_template_str_path(path) {
                    paths.insert(path.to_string());
                }
            }
            true
        },
        None,
        None,
        None,
    )?;

    Ok(paths.into_iter().collect())
}
