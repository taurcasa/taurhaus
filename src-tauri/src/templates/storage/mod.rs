use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

use fs2::FileExt;
use git2::{Oid, Repository, Signature, Status};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::models::DiffHunk;

use super::types::{RoleTemplate, TeamPreset};

mod git;
mod presets;
mod roles;
mod state;

#[cfg(test)]
mod tests;

const TEMPLATES_DIRNAME: &str = "templates";
const ROLES_DIRNAME: &str = "roles";
const PRESETS_DIRNAME: &str = "presets";
const META_DIRNAME: &str = "_meta";
const GITIGNORE_FILENAME: &str = ".gitignore";
const LOCK_FILENAME: &str = ".lock";
const LOCK_FALLBACK_FILENAME: &str = ".lock.fallback";
const STATE_FILENAME: &str = "state.json";
const RECOVERY_COMMIT_MESSAGE: &str = "templates: recovery auto-commit";
const DEFAULT_DEBOUNCE_WINDOW_SECS: i64 = 30;
const FALLBACK_LOCK_RETRY_DELAY_MS: u64 = 20;
const FALLBACK_LOCK_RETRY_ATTEMPTS: usize = 250;
const TEMP_FILE_RANDOM_RETRY_ATTEMPTS: usize = 16;

const GITIGNORE_CONTENTS: &str = "_meta/state.json\n*.tmp*\n.lock\n.lock.fallback\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCatalog {
    pub roles: Vec<RoleTemplate>,
    pub presets: Vec<TeamPreset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    BuiltIn,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleTemplateRecord {
    pub template: RoleTemplate,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPresetRecord {
    pub template: TeamPreset,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommit {
    pub commit_id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommitPage {
    pub commits: Vec<TemplateCommit>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffFile {
    pub path: String,
    pub status: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiff {
    pub commit_id: String,
    pub files: Vec<TemplateDiffFile>,
    pub stats: TemplateDiffStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateMutationResult {
    pub commit_id: Option<String>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFileMutation {
    pub relative_path: PathBuf,
    pub contents: Option<Vec<u8>>,
}

impl TemplateFileMutation {
    pub fn write(relative_path: PathBuf, contents: Vec<u8>) -> Self {
        Self {
            relative_path,
            contents: Some(contents),
        }
    }

    pub fn delete(relative_path: PathBuf) -> Self {
        Self {
            relative_path,
            contents: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TemplateStoreState {
    #[serde(default)]
    pub pending_actions: Vec<PendingAction>,
    pub last_commit_at: Option<i64>,
    #[serde(default)]
    pub repo_initialized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct DebounceState {
    pending_actions: Vec<PendingAction>,
    window_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAction {
    pub action: String,
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub changed_paths: Vec<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid template path: {0}")]
    InvalidTemplatePath(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Read-only template: {0}")]
    ReadOnly(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Lock timeout: {0}")]
    LockTimeout(String),
}

#[derive(Debug, Clone)]
struct PathChange {
    path: PathBuf,
    deleted: bool,
}

#[derive(Debug, Clone)]
struct MutationDescriptor {
    action: String,
    kind: String,
    id: String,
}

#[derive(Debug)]
struct FallbackLockGuard {
    path: PathBuf,
    _file: File,
}

impl Drop for FallbackLockGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    lock_path = %self.path.display(),
                    error = %err,
                    "failed to remove template fallback lockfile"
                );
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TemplateStoreLockGuard {
    _advisory_file: Option<File>,
    _fallback_guard: Option<FallbackLockGuard>,
}

impl TemplateStoreLockGuard {
    fn advisory(file: File) -> Self {
        Self {
            _advisory_file: Some(file),
            _fallback_guard: None,
        }
    }

    fn fallback(guard: FallbackLockGuard) -> Self {
        Self {
            _advisory_file: None,
            _fallback_guard: Some(guard),
        }
    }
}

#[derive(Debug, Clone)]
struct RoleTemplateFile {
    template: RoleTemplate,
}

#[derive(Debug, Clone)]
struct TeamPresetFile {
    template: TeamPreset,
}

impl DebounceState {
    fn from_store(state: TemplateStoreState, window_secs: i64) -> Self {
        Self {
            pending_actions: state.pending_actions,
            window_secs,
        }
    }

    fn is_empty(&self) -> bool {
        self.pending_actions.is_empty()
    }

    fn oldest_first_seen(&self) -> Option<i64> {
        self.pending_actions.iter().map(|a| a.first_seen_at).min()
    }

    fn should_flush_lazy(&self, now_ts: i64) -> bool {
        self.oldest_first_seen()
            .map(|oldest| now_ts.saturating_sub(oldest) >= self.window_secs)
            .unwrap_or(false)
    }

    fn enqueue(&mut self, descriptor: MutationDescriptor, changed_paths: &[PathBuf], now_ts: i64) {
        if let Some(existing) = self
            .pending_actions
            .iter_mut()
            .find(|action| action.kind == descriptor.kind && action.id == descriptor.id)
        {
            existing.action = descriptor.action;
            existing.last_seen_at = now_ts;
            for path in changed_paths {
                let path_str = path.to_string_lossy().to_string();
                if !existing
                    .changed_paths
                    .iter()
                    .any(|existing| existing == &path_str)
                {
                    existing.changed_paths.push(path_str);
                }
            }
            return;
        }

        self.pending_actions.push(PendingAction {
            action: descriptor.action,
            kind: descriptor.kind,
            id: descriptor.id,
            changed_paths: changed_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            first_seen_at: now_ts,
            last_seen_at: now_ts,
        });
    }

    fn commit_message(&self) -> String {
        if self.pending_actions.len() == 1 {
            let action = &self.pending_actions[0];
            return format!("templates: {} {} {}", action.action, action.kind, action.id);
        }
        format!("templates: batch {} changes", self.pending_actions.len())
    }

    fn shutdown_message(&self) -> String {
        format!(
            "templates: shutdown flush {} changes",
            self.pending_actions.len()
        )
    }

    fn take_changed_paths(&self) -> Vec<PathBuf> {
        let mut unique = BTreeMap::<PathBuf, bool>::new();
        for action in &self.pending_actions {
            for raw in &action.changed_paths {
                let path = PathBuf::from(raw);
                unique.entry(path).or_insert(true);
            }
        }
        unique.into_keys().collect()
    }
}

#[derive(Debug, Clone)]
pub struct TemplateStore {
    templates_dir: PathBuf,
    builtins_dir: PathBuf,
    debounce_window_secs: i64,
}

impl TemplateStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir: default_builtins_dir(),
            debounce_window_secs: DEFAULT_DEBOUNCE_WINDOW_SECS,
        }
    }

    pub fn with_builtins_dir(app_data_dir: PathBuf, builtins_dir: PathBuf) -> Self {
        Self::with_builtins_and_debounce(app_data_dir, builtins_dir, DEFAULT_DEBOUNCE_WINDOW_SECS)
    }

    pub fn with_builtins_and_debounce(
        app_data_dir: PathBuf,
        builtins_dir: PathBuf,
        debounce_window_secs: i64,
    ) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir,
            debounce_window_secs,
        }
    }

    pub fn templates_dir(&self) -> &Path {
        &self.templates_dir
    }

    pub fn ensure_directories(&self) -> Result<(), TemplateStoreError> {
        fs::create_dir_all(self.roles_dir())?;
        fs::create_dir_all(self.presets_dir())?;
        fs::create_dir_all(self.meta_dir())?;
        Ok(())
    }

    pub fn load_catalog(&self) -> Result<TemplateCatalog, TemplateStoreError> {
        let roles = self.load_role_catalog()?;
        let presets = self.load_preset_catalog(&roles)?;

        Ok(TemplateCatalog { roles, presets })
    }

    fn load_role_catalog(&self) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut roles_by_id: BTreeMap<String, RoleTemplate> = BTreeMap::new();
        for role in self.load_role_templates_from_dir(&self.builtins_dir.join(ROLES_DIRNAME))? {
            roles_by_id.insert(role.role_id.clone(), role);
        }
        for role in self.load_role_templates_from_dir(&self.roles_dir())? {
            roles_by_id.insert(role.role_id.clone(), role);
        }

        let roles = roles_by_id.into_values().collect::<Vec<_>>();
        for role in &roles {
            role.validate()
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;
        }
        Ok(roles)
    }

    fn load_preset_catalog(
        &self,
        role_catalog: &[RoleTemplate],
    ) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut presets_by_id: BTreeMap<String, TeamPreset> = BTreeMap::new();
        for preset in self.load_presets_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }
        for preset in self.load_presets_from_dir(&self.presets_dir())? {
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }

        let presets = presets_by_id.into_values().collect::<Vec<_>>();
        for preset in &presets {
            preset
                .validate_with_role_catalog(role_catalog)
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;
        }

        Ok(presets)
    }

    fn roles_dir(&self) -> PathBuf {
        self.templates_dir.join(ROLES_DIRNAME)
    }

    fn presets_dir(&self) -> PathBuf {
        self.templates_dir.join(PRESETS_DIRNAME)
    }

    fn meta_dir(&self) -> PathBuf {
        self.templates_dir.join(META_DIRNAME)
    }

    fn git_dir(&self) -> PathBuf {
        self.templates_dir.join(".git")
    }

    fn state_path(&self) -> PathBuf {
        self.meta_dir().join(STATE_FILENAME)
    }

    fn gitignore_path(&self) -> PathBuf {
        self.templates_dir.join(GITIGNORE_FILENAME)
    }

    fn ensure_gitignore(&self) -> Result<(), TemplateStoreError> {
        let gitignore = self.gitignore_path();
        if gitignore.exists() {
            return Ok(());
        }
        write_atomic_file(&gitignore, GITIGNORE_CONTENTS.as_bytes())
    }

    fn seed_builtins_if_missing(&self) -> Result<(), TemplateStoreError> {
        self.copy_missing_from_dir(&self.builtins_dir.join(ROLES_DIRNAME), &self.roles_dir())?;
        self.copy_missing_from_dir(
            &self.builtins_dir.join(PRESETS_DIRNAME),
            &self.presets_dir(),
        )?;
        Ok(())
    }

    fn copy_missing_from_dir(
        &self,
        source_dir: &Path,
        target_dir: &Path,
    ) -> Result<(), TemplateStoreError> {
        if !source_dir.exists() {
            return Ok(());
        }

        fs::create_dir_all(target_dir)?;

        let mut entries = fs::read_dir(source_dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        entries.sort();

        for source_path in entries {
            if !source_path.is_file() {
                continue;
            }
            if !is_yaml_file(&source_path) {
                continue;
            }

            let Some(file_name) = source_path.file_name() else {
                continue;
            };
            let target_path = target_dir.join(file_name);
            if target_path.exists() {
                continue;
            }

            let bytes = fs::read(&source_path)?;
            write_atomic_file(&target_path, &bytes)?;
        }

        Ok(())
    }

    fn role_file_path(&self, role_id: &str) -> PathBuf {
        PathBuf::from(ROLES_DIRNAME).join(format!("{role_id}.yaml"))
    }

    fn preset_file_path(&self, preset_id: &str) -> PathBuf {
        PathBuf::from(PRESETS_DIRNAME).join(format!("{preset_id}.yaml"))
    }

    fn apply_single_template_mutation(
        &self,
        mutation: TemplateFileMutation,
        commit_message: &str,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let commit_id = self.mutate_and_commit(&[mutation], commit_message)?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    fn load_role_templates_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        Ok(self
            .load_role_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_role_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplateFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut roles = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let parsed = serde_yml::from_str::<RoleTemplate>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
            })?;
            roles.push(RoleTemplateFile { template: parsed });
        }

        Ok(roles)
    }

    fn load_role_file_by_id(
        &self,
        dir: &Path,
        role_id: &str,
    ) -> Result<Option<RoleTemplateFile>, TemplateStoreError> {
        let path = dir.join(format!("{role_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let parsed = serde_yml::from_str::<RoleTemplate>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
        })?;
        Ok(Some(RoleTemplateFile { template: parsed }))
    }

    fn load_presets_from_dir(&self, dir: &Path) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        Ok(self
            .load_preset_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_preset_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<TeamPresetFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut presets = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let parsed = serde_yml::from_str::<TeamPreset>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!(
                    "failed to parse preset {}: {err}",
                    path.display()
                ))
            })?;
            presets.push(TeamPresetFile { template: parsed });
        }

        Ok(presets)
    }

    fn load_preset_file_by_id(
        &self,
        dir: &Path,
        preset_id: &str,
    ) -> Result<Option<TeamPresetFile>, TemplateStoreError> {
        let path = dir.join(format!("{preset_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let parsed = serde_yml::from_str::<TeamPreset>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse preset {}: {err}", path.display()))
        })?;
        Ok(Some(TeamPresetFile { template: parsed }))
    }
}
fn resolve_signature(repo: &Repository) -> Result<Signature<'_>, TemplateStoreError> {
    match repo.signature() {
        Ok(sig) => Ok(sig),
        Err(_) => Signature::now("taurhaus", "templates@local").map_err(TemplateStoreError::Git),
    }
}

fn default_builtins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates")
}

fn current_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_mutation_descriptor(message: &str) -> Option<MutationDescriptor> {
    let payload = message.strip_prefix("templates: ")?;
    let mut parts = payload.split_whitespace();
    let action = parts.next()?.to_string();
    let kind = parts.next()?.to_string();
    let id = parts.collect::<Vec<_>>().join(" ");
    if id.trim().is_empty() {
        return None;
    }
    Some(MutationDescriptor { action, kind, id })
}

fn validate_template_id(id: &str, kind: &str) -> Result<(), TemplateStoreError> {
    if id.trim().is_empty() {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id must not be empty"
        )));
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id '{id}' must use only [a-zA-Z0-9_-]"
        )));
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, TemplateStoreError> {
    if path.is_absolute() {
        return Err(TemplateStoreError::InvalidTemplatePath(format!(
            "absolute paths are not allowed: {}",
            path.display()
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            _ => {
                return Err(TemplateStoreError::InvalidTemplatePath(format!(
                    "path contains invalid component: {}",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(TemplateStoreError::InvalidTemplatePath(
            "empty relative path".to_string(),
        ));
    }

    Ok(normalized)
}

fn is_managed_template_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Normal(first))
            if first == OsStr::new(ROLES_DIRNAME)
                || first == OsStr::new(PRESETS_DIRNAME)
                || first == OsStr::new(META_DIRNAME)
    )
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("yaml") | Some("yml")
    )
}

fn temp_path_for(path: &Path) -> PathBuf {
    let random_suffix = format!("{:016x}", rand::thread_rng().next_u64());
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) => path.with_extension(format!("{ext}.tmp.{random_suffix}")),
        None => path.with_extension(format!("tmp.{random_suffix}")),
    }
}

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

#[cfg(test)]
fn should_force_fallback_lock_for_tests() -> bool {
    std::env::var_os("TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK")
        .map(|value| value == "1")
        .unwrap_or(false)
}

#[cfg(not(test))]
fn should_force_fallback_lock_for_tests() -> bool {
    false
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn acquire_fallback_lock(
    fallback_lock_path: &Path,
) -> Result<FallbackLockGuard, TemplateStoreError> {
    let mut last_conflict = None;
    for _ in 0..FALLBACK_LOCK_RETRY_ATTEMPTS {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(fallback_lock_path)
        {
            Ok(mut file) => {
                let pid = std::process::id();
                let _ = writeln!(file, "{pid}");
                file.sync_all()?;
                return Ok(FallbackLockGuard {
                    path: fallback_lock_path.to_path_buf(),
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_conflict = Some(err);
                thread::sleep(Duration::from_millis(FALLBACK_LOCK_RETRY_DELAY_MS));
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let cause = last_conflict
        .map(|err| err.to_string())
        .unwrap_or_else(|| "unknown fallback lock contention".to_string());
    Err(TemplateStoreError::LockTimeout(format!(
        "timed out acquiring fallback lock {}: {}",
        fallback_lock_path.display(),
        cause
    )))
}

fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), TemplateStoreError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut tmp_open_error = None;
    let mut selected_tmp = None;
    let mut selected_file = None;
    for _ in 0..TEMP_FILE_RANDOM_RETRY_ATTEMPTS {
        let candidate = temp_path_for(target);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => {
                selected_tmp = Some(candidate);
                selected_file = Some(file);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                tmp_open_error = Some(err);
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let tmp = selected_tmp.ok_or_else(|| {
        let err = tmp_open_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "failed to allocate unique temp path after {} attempts for {}",
                    TEMP_FILE_RANDOM_RETRY_ATTEMPTS,
                    target.display()
                ),
            )
        });
        TemplateStoreError::Io(err)
    })?;

    let mut file = selected_file.ok_or_else(|| {
        TemplateStoreError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "internal temp-file selection mismatch while writing {}",
                target.display()
            ),
        ))
    })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    if let Err(err) = fs::rename(&tmp, target) {
        if is_windows_unsupported_rename_error(&err) {
            tracing::warn!(
                target = %target.display(),
                "atomic rename unsupported on this path; falling back to direct write"
            );
            fs::write(target, bytes)?;
            let _ = fs::remove_file(&tmp);
            return Ok(());
        }

        let _ = fs::remove_file(&tmp);
        return Err(TemplateStoreError::Io(err));
    }

    Ok(())
}

fn is_deleted_status(status: Status) -> bool {
    status.intersects(Status::INDEX_DELETED | Status::WT_DELETED | Status::CONFLICTED)
}
