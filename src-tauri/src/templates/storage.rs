use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use git2::{Oid, Repository, Signature, Status, StatusOptions};
use serde::{Deserialize, Serialize};

use super::types::{RoleTemplate, TeamPreset};

const TEMPLATES_DIRNAME: &str = "templates";
const ROLES_DIRNAME: &str = "roles";
const PRESETS_DIRNAME: &str = "presets";
const META_DIRNAME: &str = "_meta";
const GITIGNORE_FILENAME: &str = ".gitignore";
const LOCK_FILENAME: &str = ".lock";
const STATE_FILENAME: &str = "state.json";
const RECOVERY_COMMIT_MESSAGE: &str = "templates: recovery auto-commit";
const DEFAULT_DEBOUNCE_WINDOW_SECS: i64 = 30;

const GITIGNORE_CONTENTS: &str = "_meta/state.json\n*.tmp\n.lock\n";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateMutationResult {
    pub commit_id: Option<String>,
    pub committed: bool,
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
                if !existing.changed_paths.iter().any(|existing| existing == &path_str) {
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
        format!("templates: shutdown flush {} changes", self.pending_actions.len())
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

    pub fn load_catalog(&self) -> Result<TemplateCatalog, TemplateStoreError> {
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
                .validate_with_role_catalog(&roles)
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;
        }

        Ok(TemplateCatalog { roles, presets })
    }

    pub fn list_roles(&self) -> Result<Vec<RoleTemplateRecord>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut merged: BTreeMap<String, RoleTemplateRecord> = BTreeMap::new();
        for role_file in self.load_role_files_from_dir(&self.builtins_dir.join(ROLES_DIRNAME))? {
            role_file
                .template
                .validate()
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                role_file.template.role_id.clone(),
                RoleTemplateRecord {
                    template: role_file.template,
                    source: TemplateSource::BuiltIn,
                    read_only: true,
                },
            );
        }

        for role_file in self.load_role_files_from_dir(&self.roles_dir())? {
            role_file
                .template
                .validate()
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                role_file.template.role_id.clone(),
                RoleTemplateRecord {
                    template: role_file.template,
                    source: TemplateSource::User,
                    read_only: false,
                },
            );
        }

        Ok(merged.into_values().collect())
    }

    pub fn get_role(&self, role_id: &str) -> Result<RoleTemplateRecord, TemplateStoreError> {
        let roles = self.list_roles()?;
        roles
            .into_iter()
            .find(|record| record.template.role_id == role_id)
            .ok_or_else(|| TemplateStoreError::NotFound(format!("role '{role_id}' not found")))
    }

    pub fn create_role(
        &self,
        template: &RoleTemplate,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(&template.role_id, "role")?;
        template
            .validate()
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_role(&template.role_id) {
            Ok(record) => {
                return match record.source {
                    TemplateSource::BuiltIn => Err(TemplateStoreError::ReadOnly(format!(
                        "role '{}' is built-in and cannot be created over",
                        template.role_id
                    ))),
                    TemplateSource::User => Err(TemplateStoreError::AlreadyExists(format!(
                        "role '{}' already exists",
                        template.role_id
                    ))),
                };
            }
            Err(TemplateStoreError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }

        let relative_path = self.role_file_path(&template.role_id);
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize role '{}': {err}", template.role_id))
        })?;
        self.write_template_file(&relative_path, payload.as_bytes())?;

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: create role {}", template.role_id),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn update_role(
        &self,
        role_id: &str,
        template: &RoleTemplate,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        if template.role_id != role_id {
            return Err(TemplateStoreError::Validation(format!(
                "role_id mismatch: expected '{role_id}', got '{}'",
                template.role_id
            )));
        }
        validate_template_id(role_id, "role")?;
        template
            .validate()
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_role(role_id) {
            Ok(_) => {}
            Err(TemplateStoreError::NotFound(_)) => {
                return Err(TemplateStoreError::NotFound(format!(
                    "role '{role_id}' not found"
                )));
            }
            Err(err) => return Err(err),
        }

        // Updating built-ins creates/updates user override.
        let relative_path = self.role_file_path(role_id);
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize role '{role_id}': {err}"))
        })?;
        self.write_template_file(&relative_path, payload.as_bytes())?;

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: update role {role_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn delete_role(&self, role_id: &str) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(role_id, "role")?;
        self.ensure_directories()?;

        let record = self.get_role(role_id)?;
        if record.read_only {
            return Err(TemplateStoreError::ReadOnly(format!(
                "role '{role_id}' is built-in and cannot be deleted"
            )));
        }

        let catalog = self.load_catalog()?;
        if catalog.presets.iter().any(|preset| {
            preset.lead_role_id == role_id
                || preset
                    .agent_slots
                    .iter()
                    .any(|slot| slot.role_id == role_id)
        }) {
            return Err(TemplateStoreError::Conflict(format!(
                "role '{role_id}' is referenced by one or more presets"
            )));
        }

        let relative_path = self.role_file_path(role_id);
        let absolute_path = self.templates_dir().join(&relative_path);
        if !absolute_path.exists() {
            return Err(TemplateStoreError::NotFound(format!(
                "role file missing for '{role_id}'"
            )));
        }

        let _lock = self.acquire_lock()?;
        fs::remove_file(&absolute_path)?;
        drop(_lock);

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: delete role {role_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn import_role(
        &self,
        external_path: &Path,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let raw = fs::read_to_string(external_path)?;
        let template = serde_yaml::from_str::<RoleTemplate>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to parse external role {}: {err}",
                external_path.display()
            ))
        })?;
        template
            .validate()
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        let role_id = template.role_id.clone();
        validate_template_id(&role_id, "role")?;
        let action = match self.get_role(&role_id) {
            Ok(_) => "update",
            Err(TemplateStoreError::NotFound(_)) => "create",
            Err(err) => return Err(err),
        };

        let relative_path = self.role_file_path(&role_id);
        self.write_template_file(&relative_path, raw.as_bytes())?;
        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: {action} role {role_id}"),
        )?;

        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn list_presets(&self) -> Result<Vec<TeamPresetRecord>, TemplateStoreError> {
        self.ensure_directories()?;
        let role_catalog = self.load_catalog()?.roles;

        let mut merged: BTreeMap<String, TeamPresetRecord> = BTreeMap::new();
        for preset_file in self.load_preset_files_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                preset_file.template.preset_id.clone(),
                TeamPresetRecord {
                    template: preset_file.template,
                    source: TemplateSource::BuiltIn,
                    read_only: true,
                },
            );
        }

        for preset_file in self.load_preset_files_from_dir(&self.presets_dir())? {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                preset_file.template.preset_id.clone(),
                TeamPresetRecord {
                    template: preset_file.template,
                    source: TemplateSource::User,
                    read_only: false,
                },
            );
        }

        Ok(merged.into_values().collect())
    }

    pub fn get_preset(&self, preset_id: &str) -> Result<TeamPresetRecord, TemplateStoreError> {
        self.list_presets()?
            .into_iter()
            .find(|record| record.template.preset_id == preset_id)
            .ok_or_else(|| TemplateStoreError::NotFound(format!("preset '{preset_id}' not found")))
    }

    pub fn create_preset(
        &self,
        template: &TeamPreset,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(&template.preset_id, "preset")?;
        let roles = self.load_catalog()?.roles;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_preset(&template.preset_id) {
            Ok(record) => {
                return match record.source {
                    TemplateSource::BuiltIn => Err(TemplateStoreError::ReadOnly(format!(
                        "preset '{}' is built-in and cannot be created over",
                        template.preset_id
                    ))),
                    TemplateSource::User => Err(TemplateStoreError::AlreadyExists(format!(
                        "preset '{}' already exists",
                        template.preset_id
                    ))),
                };
            }
            Err(TemplateStoreError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }

        let relative_path = self.preset_file_path(&template.preset_id);
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to serialize preset '{}': {err}",
                template.preset_id
            ))
        })?;
        self.write_template_file(&relative_path, payload.as_bytes())?;

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: create preset {}", template.preset_id),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn update_preset(
        &self,
        preset_id: &str,
        template: &TeamPreset,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        if template.preset_id != preset_id {
            return Err(TemplateStoreError::Validation(format!(
                "preset_id mismatch: expected '{preset_id}', got '{}'",
                template.preset_id
            )));
        }
        validate_template_id(preset_id, "preset")?;

        let roles = self.load_catalog()?.roles;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_preset(preset_id) {
            Ok(_) => {}
            Err(TemplateStoreError::NotFound(_)) => {
                return Err(TemplateStoreError::NotFound(format!(
                    "preset '{preset_id}' not found"
                )));
            }
            Err(err) => return Err(err),
        }

        // Updating built-ins creates/updates user override.
        let relative_path = self.preset_file_path(preset_id);
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize preset '{preset_id}': {err}"))
        })?;
        self.write_template_file(&relative_path, payload.as_bytes())?;

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: update preset {preset_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn delete_preset(
        &self,
        preset_id: &str,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(preset_id, "preset")?;
        self.ensure_directories()?;

        let record = self.get_preset(preset_id)?;
        if record.read_only {
            return Err(TemplateStoreError::ReadOnly(format!(
                "preset '{preset_id}' is built-in and cannot be deleted"
            )));
        }

        let relative_path = self.preset_file_path(preset_id);
        let absolute_path = self.templates_dir().join(&relative_path);
        if !absolute_path.exists() {
            return Err(TemplateStoreError::NotFound(format!(
                "preset file missing for '{preset_id}'"
            )));
        }

        let _lock = self.acquire_lock()?;
        fs::remove_file(&absolute_path)?;
        drop(_lock);

        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: delete preset {preset_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn import_preset(
        &self,
        external_path: &Path,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let raw = fs::read_to_string(external_path)?;
        let template = serde_yaml::from_str::<TeamPreset>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to parse external preset {}: {err}",
                external_path.display()
            ))
        })?;

        validate_template_id(&template.preset_id, "preset")?;
        let roles = self.load_catalog()?.roles;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        let preset_id = template.preset_id.clone();
        let action = match self.get_preset(&preset_id) {
            Ok(_) => "update",
            Err(TemplateStoreError::NotFound(_)) => "create",
            Err(err) => return Err(err),
        };

        let relative_path = self.preset_file_path(&preset_id);
        self.write_template_file(&relative_path, raw.as_bytes())?;
        let commit_id = self.commit_paths(
            std::slice::from_ref(&relative_path),
            &format!("templates: {action} preset {preset_id}"),
        )?;

        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
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
        let mut state = DebounceState::from_store(persisted_state.clone(), self.debounce_window_secs);

        let stale_commit = self.flush_debounce_if_needed_with_repo(
            &repo,
            &mut state,
            false,
            false,
            now_ts,
        )?;

        let descriptor = parse_mutation_descriptor(message).unwrap_or_else(|| MutationDescriptor {
            action: "update".to_string(),
            kind: "template".to_string(),
            id: "unknown".to_string(),
        });
        state.enqueue(descriptor, &normalized_paths, now_ts);

        let followup_commit = self.flush_debounce_if_needed_with_repo(
            &repo,
            &mut state,
            false,
            false,
            now_ts,
        )?;

        persisted_state.pending_actions = state.pending_actions;
        if stale_commit.is_some() || followup_commit.is_some() {
            persisted_state.last_commit_at = Some(now_ts);
        }
        persisted_state.repo_initialized = true;
        self.save_state_unlocked(&persisted_state)?;
        Ok(followup_commit.or(stale_commit))
    }

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

    fn load_state_unlocked(&self) -> Result<TemplateStoreState, TemplateStoreError> {
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

    fn save_state_unlocked(&self, state: &TemplateStoreState) -> Result<(), TemplateStoreError> {
        let payload = serde_json::to_vec_pretty(state)
            .map_err(|err| TemplateStoreError::Parse(format!("failed to serialize state: {err}")))?;
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

    fn flush_debounce_if_needed_with_repo(
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

    fn copy_missing_from_dir(&self, source_dir: &Path, target_dir: &Path) -> Result<(), TemplateStoreError> {
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

    fn load_role_templates_from_dir(&self, dir: &Path) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        Ok(self
            .load_role_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_role_files_from_dir(&self, dir: &Path) -> Result<Vec<RoleTemplateFile>, TemplateStoreError> {
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
            let parsed = serde_yaml::from_str::<RoleTemplate>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
            })?;
            roles.push(RoleTemplateFile { template: parsed });
        }

        Ok(roles)
    }

    fn load_presets_from_dir(&self, dir: &Path) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        Ok(self
            .load_preset_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_preset_files_from_dir(&self, dir: &Path) -> Result<Vec<TeamPresetFile>, TemplateStoreError> {
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
            let parsed = serde_yaml::from_str::<TeamPreset>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!("failed to parse preset {}: {err}", path.display()))
            })?;
            presets.push(TeamPresetFile { template: parsed });
        }

        Ok(presets)
    }

    fn acquire_lock(&self) -> Result<File, TemplateStoreError> {
        let lock_path = self.templates_dir().join(LOCK_FILENAME);
        let file = File::create(&lock_path)?;
        match file.lock_exclusive() {
            Ok(()) => {}
            Err(err) if is_windows_unsupported_lock_error(&err) => {
                tracing::warn!(
                    lock_path = %lock_path.display(),
                    "advisory file locks unsupported at template path; continuing unlocked"
                );
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
        Ok(file)
    }

    fn collect_managed_changes(&self, repo: &Repository) -> Result<Vec<PathChange>, TemplateStoreError> {
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
            by_path.insert(
                rel.clone(),
                PathChange {
                    path: rel,
                    deleted,
                },
            );
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
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) => path.with_extension(format!("{ext}.tmp")),
        None => path.with_extension("tmp"),
    }
}

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), TemplateStoreError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = temp_path_for(target);
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

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
    status.intersects(
        Status::INDEX_DELETED | Status::WT_DELETED | Status::CONFLICTED,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use git2::Repository;
    use tempfile::TempDir;

    use super::*;

    fn setup_dirs() -> (TempDir, PathBuf, PathBuf) {
        let root = TempDir::new().expect("tempdir");
        let app_data = root.path().join("app-data");
        let builtins = root.path().join("builtins");

        fs::create_dir_all(builtins.join("roles")).expect("create builtins roles");
        fs::create_dir_all(builtins.join("presets")).expect("create builtins presets");

        (root, app_data, builtins)
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write file");
    }

    fn lead_role_yaml(role_id: &str, instructions: &str) -> String {
        format!(
            "schema:\n  kind: role_template\n  version: 1\nrole_id: {role_id}\nname: Lead\nversion: \"1.0.0\"\nkind: lead\ndefaults:\n  cli_tool: claude\n  model: claude-opus-4-6\n  default_name_pattern: lead-{{project}}\ninstructions: \"{instructions}\"\nbehavioral_contract:\n  communication:\n    - sync\n  execution:\n    - plan\n  escalation:\n    - escalate\ncapabilities:\n  - planning\nconstraints:\n  min_instances: 1\n  max_instances: 1\n  allowed_project_binding: lead_project\n"
        )
    }

    fn agent_role_yaml(role_id: &str, instructions: &str) -> String {
        format!(
            "schema:\n  kind: role_template\n  version: 1\nrole_id: {role_id}\nname: Dev\nversion: \"1.0.0\"\nkind: agent\ndefaults:\n  cli_tool: codex\n  model: gpt-5.3-codex\n  default_name_pattern: dev-{{n}}\ninstructions: \"{instructions}\"\nbehavioral_contract:\n  communication:\n    - updates\n  execution:\n    - implement\n  escalation:\n    - escalate\ncapabilities:\n  - implementation\nconstraints:\n  min_instances: 0\n  max_instances: 8\n  allowed_project_binding: any\n"
        )
    }

    fn preset_yaml(preset_id: &str) -> String {
        preset_yaml_with_agent(preset_id, "dev")
    }

    fn preset_yaml_with_agent(preset_id: &str, agent_role_id: &str) -> String {
        format!(
            "schema:\n  kind: team_preset\n  version: 1\npreset_id: {preset_id}\nname: Base Team\ndescription: Base preset\nversion: \"1.0.0\"\nlead_role_id: lead\nagent_slots:\n  - role_id: {agent_role_id}\n    count: 1\n    project_binding: lead_project\ndefaults:\n  team_name_pattern: \"{{project}}-team\"\n  tmux_layout: tiled\n"
        )
    }

    fn seed_valid_catalog(builtins_dir: &Path) {
        write(
            &builtins_dir.join("roles").join("lead.yaml"),
            &lead_role_yaml("lead", "lead built-in"),
        );
        write(
            &builtins_dir.join("roles").join("dev.yaml"),
            &agent_role_yaml("dev", "dev built-in"),
        );
        write(
            &builtins_dir.join("presets").join("base.yaml"),
            &preset_yaml("base"),
        );
    }

    fn parse_role(yaml: &str) -> RoleTemplate {
        serde_yaml::from_str::<RoleTemplate>(yaml).expect("parse role yaml")
    }

    fn parse_preset(yaml: &str) -> TeamPreset {
        serde_yaml::from_str::<TeamPreset>(yaml).expect("parse preset yaml")
    }

    fn age_pending_actions(store: &TemplateStore, seconds: i64) {
        let mut state = store.load_state().expect("load state");
        for action in &mut state.pending_actions {
            action.first_seen_at -= seconds;
            action.last_seen_at -= seconds;
        }
        store.save_state(&state).expect("save state");
    }

    fn latest_commit_message(repo_path: &Path) -> String {
        let repo = Repository::open(repo_path).expect("open repo");
        let head = repo.head().expect("head");
        let commit = head.peel_to_commit().expect("head commit");
        commit.message().unwrap_or("").trim().to_string()
    }

    #[test]
    fn ensure_directories_creates_expected_structure() {
        let (_root, app_data, builtins) = setup_dirs();
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

        store.ensure_directories().expect("ensure directories");

        let templates = app_data.join("templates");
        assert!(templates.join("roles").is_dir());
        assert!(templates.join("presets").is_dir());
        assert!(templates.join("_meta").is_dir());
    }

    #[test]
    fn ensure_repo_for_mutation_initializes_repo_copies_builtins_and_writes_gitignore() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);

        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins.clone());
        let repo = store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo should initialize");

        assert!(repo.path().exists());
        assert!(app_data
            .join("templates")
            .join("roles")
            .join("lead.yaml")
            .exists());
        assert!(app_data
            .join("templates")
            .join("presets")
            .join("base.yaml")
            .exists());

        let gitignore = fs::read_to_string(app_data.join("templates").join(".gitignore"))
            .expect("read gitignore");
        assert!(gitignore.contains("_meta/state.json"));
    }

    #[test]
    fn ensure_repo_for_mutation_falls_back_when_existing_git_dir_is_invalid() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);

        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");
        fs::create_dir_all(app_data.join("templates").join(".git")).expect("create fake git dir");

        let repo = store.ensure_repo_for_mutation().expect("ensure repo");
        assert!(repo.is_none(), "invalid git dir should trigger plain filesystem fallback");
    }

    #[test]
    fn load_catalog_merges_builtins_with_user_overrides() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);

        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins.clone());
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("roles").join("dev.yaml"),
            &agent_role_yaml("dev", "dev user override"),
        );

        let catalog = store.load_catalog().expect("load catalog");
        let dev = catalog
            .roles
            .iter()
            .find(|role| role.role_id == "dev")
            .expect("dev role exists");
        assert_eq!(dev.instructions, "dev user override");
        assert!(catalog.presets.iter().any(|preset| preset.preset_id == "base"));
    }

    #[test]
    fn write_template_file_is_atomic_and_writes_content() {
        let (_root, app_data, builtins) = setup_dirs();
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

        let rel = Path::new("roles/new-role.yaml");
        store
            .write_template_file(rel, b"content-v1")
            .expect("write file");

        let path = app_data.join("templates").join(rel);
        let tmp = path.with_extension("yaml.tmp");
        assert_eq!(fs::read_to_string(path).expect("read file"), "content-v1");
        assert!(!tmp.exists(), "tmp file should be cleaned up");
    }

    #[test]
    fn recover_dirty_tree_auto_commits_changes() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);

        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("init repo")
            .expect("repo exists");

        store
            .write_template_file(
                Path::new("roles/lead.yaml"),
                lead_role_yaml("lead", "lead v1").as_bytes(),
            )
            .expect("write lead");
        store
            .write_template_file(
                Path::new("roles/dev.yaml"),
                agent_role_yaml("dev", "dev v1").as_bytes(),
            )
            .expect("write dev");
        store
            .write_template_file(
                Path::new("presets/base.yaml"),
                preset_yaml("base").as_bytes(),
            )
            .expect("write preset");

        let initial_commit = store
            .commit_paths(
                &[
                    PathBuf::from("roles/lead.yaml"),
                    PathBuf::from("roles/dev.yaml"),
                    PathBuf::from("presets/base.yaml"),
                ],
                "templates: seed baseline",
            )
            .expect("initial commit");
        assert!(initial_commit.is_none(), "baseline should be debounced");
        let flushed = store
            .flush_pending_commits()
            .expect("flush baseline pending commit");
        assert!(flushed.is_some(), "baseline should commit when flushed");

        store
            .write_template_file(
                Path::new("roles/dev.yaml"),
                agent_role_yaml("dev", "dev v2").as_bytes(),
            )
            .expect("modify role");

        let recovery_commit = store.recover_dirty_tree().expect("recovery run");
        assert!(recovery_commit.is_some(), "dirty tree should auto-commit on recovery");

        let repo = Repository::open(app_data.join("templates")).expect("open repo");
        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        let statuses = repo.statuses(Some(&mut opts)).expect("statuses");
        let managed_dirty = statuses.iter().any(|entry| {
            entry
                .path()
                .map(|path| is_managed_template_path(Path::new(path)))
                .unwrap_or(false)
        });
        assert!(
            !managed_dirty,
            "managed template files should be clean after recovery commit"
        );
    }

    #[test]
    fn state_round_trip_persists_pending_actions() {
        let (_root, app_data, builtins) = setup_dirs();
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let state = TemplateStoreState {
            pending_actions: vec![PendingAction {
                action: "update".to_string(),
                kind: "role".to_string(),
                id: "dev".to_string(),
                changed_paths: vec!["roles/dev.yaml".to_string()],
                first_seen_at: 1,
                last_seen_at: 2,
            }],
            last_commit_at: Some(99),
            repo_initialized: true,
        };

        store.save_state(&state).expect("save state");
        let loaded = store.load_state().expect("load state");

        assert_eq!(loaded, state);
    }

    #[test]
    fn list_roles_merges_sources_and_marks_read_only() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("roles").join("dev.yaml"),
            &agent_role_yaml("dev", "user override"),
        );

        let roles = store.list_roles().expect("list roles");
        let lead = roles
            .iter()
            .find(|role| role.template.role_id == "lead")
            .expect("lead role");
        assert_eq!(lead.source, TemplateSource::BuiltIn);
        assert!(lead.read_only);

        let dev = roles
            .iter()
            .find(|role| role.template.role_id == "dev")
            .expect("dev role");
        assert_eq!(dev.source, TemplateSource::User);
        assert!(!dev.read_only);
        assert_eq!(dev.template.instructions, "user override");
    }

    #[test]
    fn get_role_prefers_user_override() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("roles").join("dev.yaml"),
            &agent_role_yaml("dev", "user override"),
        );

        let role = store.get_role("dev").expect("get role");
        assert_eq!(role.source, TemplateSource::User);
        assert_eq!(role.template.instructions, "user override");
    }

    #[test]
    fn create_role_validates_writes_and_commits() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let template = parse_role(&agent_role_yaml("qa", "qa role"));
        let result = store.create_role(&template).expect("create role");

        assert!(!result.committed);
        assert!(result.commit_id.is_none());
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");
        assert!(app_data
            .join("templates")
            .join("roles")
            .join("qa.yaml")
            .exists());
    }

    #[test]
    fn create_role_blocks_built_in_collision() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let template = parse_role(&lead_role_yaml("lead", "override"));
        let err = store.create_role(&template).expect_err("should fail");
        assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
    }

    #[test]
    fn update_role_creates_user_override_for_built_in() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let template = parse_role(&lead_role_yaml("lead", "lead override"));
        let result = store
            .update_role("lead", &template)
            .expect("update built-in via override");

        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");
        let role = store.get_role("lead").expect("get role");
        assert_eq!(role.source, TemplateSource::User);
        assert_eq!(role.template.instructions, "lead override");
        assert!(app_data
            .join("templates")
            .join("roles")
            .join("lead.yaml")
            .exists());
    }

    #[test]
    fn update_role_fails_when_missing() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let template = parse_role(&agent_role_yaml("does-not-exist", "new"));
        let err = store
            .update_role("does-not-exist", &template)
            .expect_err("update should fail");
        assert!(matches!(err, TemplateStoreError::NotFound(_)));
    }

    #[test]
    fn delete_role_blocks_when_referenced_by_preset() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        store.create_role(&qa).expect("create qa");
        write(
            &app_data.join("templates").join("presets").join("qa.yaml"),
            &preset_yaml_with_agent("qa-preset", "qa"),
        );

        let err = store.delete_role("qa").expect_err("delete should fail");
        assert!(matches!(err, TemplateStoreError::Conflict(_)));
    }

    #[test]
    fn delete_role_removes_user_template_and_commits() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        store.create_role(&qa).expect("create qa");

        let result = store.delete_role("qa").expect("delete qa");
        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");
        assert!(!app_data
            .join("templates")
            .join("roles")
            .join("qa.yaml")
            .exists());
    }

    #[test]
    fn import_role_validates_and_writes_to_user_directory() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let external = app_data.join("external-role.yaml");
        write(&external, &agent_role_yaml("researcher", "research role"));

        let result = store.import_role(&external).expect("import role");
        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");

        let role = store.get_role("researcher").expect("get imported role");
        assert_eq!(role.source, TemplateSource::User);
        assert_eq!(role.template.instructions, "research role");
    }

    #[test]
    fn list_roles_picks_up_external_files_added_to_roles_directory() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("roles").join("ext.yaml"),
            &agent_role_yaml("ext", "external file"),
        );

        let roles = store.list_roles().expect("list roles");
        let ext = roles
            .iter()
            .find(|role| role.template.role_id == "ext")
            .expect("external role present");
        assert_eq!(ext.source, TemplateSource::User);
        assert_eq!(ext.template.instructions, "external file");
    }

    #[test]
    fn list_presets_merges_sources_and_marks_read_only() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("presets").join("base.yaml"),
            &preset_yaml("base"),
        );

        let presets = store.list_presets().expect("list presets");
        let base = presets
            .iter()
            .find(|preset| preset.template.preset_id == "base")
            .expect("base preset");
        assert_eq!(base.source, TemplateSource::User);
        assert!(!base.read_only);
    }

    #[test]
    fn get_preset_prefers_user_override() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");
        write(
            &app_data.join("templates").join("presets").join("base.yaml"),
            &preset_yaml("base"),
        );

        let preset = store.get_preset("base").expect("get preset");
        assert_eq!(preset.source, TemplateSource::User);
    }

    #[test]
    fn create_preset_validates_writes_and_commits() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let preset = parse_preset(&preset_yaml_with_agent("qa-team", "dev"));
        let result = store.create_preset(&preset).expect("create preset");

        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");
        assert!(app_data
            .join("templates")
            .join("presets")
            .join("qa-team.yaml")
            .exists());
    }

    #[test]
    fn create_preset_rejects_unknown_role_reference() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let preset = parse_preset(&preset_yaml_with_agent("bad", "missing-role"));
        let err = store.create_preset(&preset).expect_err("must fail");
        assert!(matches!(err, TemplateStoreError::Validation(_)));
    }

    #[test]
    fn create_preset_blocks_built_in_collision() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let preset = parse_preset(&preset_yaml("base"));
        let err = store.create_preset(&preset).expect_err("must fail");
        assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
    }

    #[test]
    fn update_preset_creates_user_override_for_built_in() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let preset = parse_preset(&preset_yaml("base"));
        let result = store.update_preset("base", &preset).expect("update base");
        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");

        let loaded = store.get_preset("base").expect("get base");
        assert_eq!(loaded.source, TemplateSource::User);
        assert!(app_data
            .join("templates")
            .join("presets")
            .join("base.yaml")
            .exists());
    }

    #[test]
    fn update_preset_fails_when_missing() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let preset = parse_preset(&preset_yaml_with_agent("missing", "dev"));
        let err = store
            .update_preset("missing", &preset)
            .expect_err("missing preset");
        assert!(matches!(err, TemplateStoreError::NotFound(_)));
    }

    #[test]
    fn delete_preset_removes_user_preset_and_commits() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let preset = parse_preset(&preset_yaml_with_agent("tmp", "dev"));
        store.create_preset(&preset).expect("create preset");

        let result = store.delete_preset("tmp").expect("delete preset");
        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");
        assert!(!app_data
            .join("templates")
            .join("presets")
            .join("tmp.yaml")
            .exists());
    }

    #[test]
    fn delete_preset_blocks_built_in_delete() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data, builtins);

        let err = store.delete_preset("base").expect_err("built-in delete blocked");
        assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
    }

    #[test]
    fn import_preset_validates_and_writes_to_user_directory() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let external = app_data.join("external-preset.yaml");
        write(&external, &preset_yaml_with_agent("external", "dev"));

        let result = store.import_preset(&external).expect("import preset");
        assert!(!result.committed);
        let flushed = store.flush_pending_commits().expect("flush pending");
        assert!(flushed.is_some(), "flush should create commit");

        let preset = store.get_preset("external").expect("get imported");
        assert_eq!(preset.source, TemplateSource::User);
    }

    #[test]
    fn list_presets_picks_up_external_files_added_to_presets_directory() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
        store.ensure_directories().expect("ensure dirs");

        write(
            &app_data.join("templates").join("presets").join("ext.yaml"),
            &preset_yaml_with_agent("ext", "dev"),
        );

        let presets = store.list_presets().expect("list presets");
        let ext = presets
            .iter()
            .find(|preset| preset.template.preset_id == "ext")
            .expect("external preset present");
        assert_eq!(ext.source, TemplateSource::User);
    }

    #[test]
    fn debounce_coalesces_repeated_role_updates_into_single_commit() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa_v1 = parse_role(&agent_role_yaml("qa", "qa v1"));
        let qa_v2 = parse_role(&agent_role_yaml("qa", "qa v2"));
        assert!(!store.create_role(&qa_v1).expect("create").committed);
        assert!(!store.update_role("qa", &qa_v2).expect("update").committed);

        let state = store.load_state().expect("load state");
        assert_eq!(state.pending_actions.len(), 1, "same role should coalesce");
        assert_eq!(state.pending_actions[0].action, "update");
        assert_eq!(state.pending_actions[0].id, "qa");

        assert!(store
            .maybe_flush_pending_commits()
            .expect("maybe flush before debounce")
            .is_none());
        age_pending_actions(&store, 31);
        let commit_id = store
            .maybe_flush_pending_commits()
            .expect("flush after debounce");
        assert!(commit_id.is_some());
        assert_eq!(
            latest_commit_message(&app_data.join("templates")),
            "templates: update role qa"
        );
    }

    #[test]
    fn debounce_uses_batch_message_for_multiple_pending_actions() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        let preset = parse_preset(&preset_yaml_with_agent("qa-team", "qa"));
        assert!(!store.create_role(&qa).expect("create role").committed);
        assert!(!store.create_preset(&preset).expect("create preset").committed);

        let state = store.load_state().expect("load state");
        assert_eq!(state.pending_actions.len(), 2);

        age_pending_actions(&store, 31);
        let commit_id = store
            .maybe_flush_pending_commits()
            .expect("flush pending batch");
        assert!(commit_id.is_some());
        assert_eq!(
            latest_commit_message(&app_data.join("templates")),
            "templates: batch 2 changes"
        );
    }

    #[test]
    fn shutdown_flush_uses_shutdown_message() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        assert!(!store.create_role(&qa).expect("create role").committed);

        let commit_id = store
            .flush_pending_commits_on_shutdown()
            .expect("shutdown flush");
        assert!(commit_id.is_some());
        assert_eq!(
            latest_commit_message(&app_data.join("templates")),
            "templates: shutdown flush 1 changes"
        );
    }

    #[test]
    fn stale_pending_actions_flush_before_enqueueing_new_mutation() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        assert!(!store.create_role(&qa).expect("create qa").committed);
        age_pending_actions(&store, 31);

        let qb = parse_role(&agent_role_yaml("qb", "qb role"));
        let second = store.create_role(&qb).expect("create qb");
        assert!(
            second.committed,
            "creating qb should flush stale qa action before enqueueing qb"
        );

        let state = store.load_state().expect("load state");
        assert_eq!(state.pending_actions.len(), 1);
        assert_eq!(state.pending_actions[0].id, "qb");
        assert_eq!(
            latest_commit_message(&app_data.join("templates")),
            "templates: create role qa"
        );
    }

    #[test]
    fn precommit_validation_failure_preserves_pending_actions() {
        let (_root, app_data, builtins) = setup_dirs();
        seed_valid_catalog(&builtins);
        let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
        store
            .ensure_repo_for_mutation()
            .expect("ensure repo")
            .expect("repo");

        let qa = parse_role(&agent_role_yaml("qa", "qa role"));
        assert!(!store.create_role(&qa).expect("create role").committed);

        write(
            &app_data.join("templates").join("presets").join("invalid.yaml"),
            "not: valid: yaml",
        );
        age_pending_actions(&store, 31);

        let flush_result = store
            .maybe_flush_pending_commits()
            .expect("flush should not error");
        assert!(flush_result.is_none(), "invalid schema should skip commit");

        let state = store.load_state().expect("load state");
        assert!(
            !state.pending_actions.is_empty(),
            "pending actions should remain for later retry"
        );
    }
}
