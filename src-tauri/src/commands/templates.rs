use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use git2::{Repository, Sort, StatusOptions};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::errors::sanitize_error;
use crate::git::commits::{get_commit_diff, get_commit_files};
use crate::models::DiffHunk;
use crate::templates::composition::{compose_team, CompositionOverrides, CompositionResult};
use crate::templates::storage::{
    PendingAction, RoleTemplateRecord, TeamPresetRecord, TemplateFileMutation, TemplateSource,
    TemplateStore, TemplateStoreError,
};
use crate::templates::types::{
    AgentSlot, ProjectBinding, RoleKind, RoleTemplate, SlotOverrides, TeamPreset,
};

#[cfg(feature = "mesh-bridged-backend")]
use crate::commands::coordination;
#[cfg(feature = "mesh-bridged-backend")]
use crate::commands::coordination_types::{InitializeReport, InitializeTeamRequest};
#[cfg(feature = "mesh-bridged-backend")]
use crate::commands::projects::DbState;
#[cfg(feature = "mesh-bridged-backend")]
use crate::coordination::state::CoordinationState;
#[cfg(feature = "mesh-bridged-backend")]
use tauri::AppHandle;

pub struct TemplateStoreState(pub TemplateStore);

impl TemplateStoreState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self(TemplateStore::new(app_data_dir))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStorageMode {
    Git,
    PlainFilesystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleTemplateSummary {
    pub role_id: String,
    pub name: String,
    pub version: String,
    pub kind: RoleKind,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPresetSummary {
    pub preset_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleTemplateFull {
    #[serde(flatten)]
    pub template: RoleTemplate,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPresetFull {
    #[serde(flatten)]
    pub template: TeamPreset,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesUpsertRoleRequest {
    pub template: RoleTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesUpsertPresetRequest {
    pub preset: TeamPreset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesComposeAgentSlotRequest {
    #[serde(alias = "role_id")]
    pub role_id: String,
    pub count: u32,
    #[serde(alias = "project_binding")]
    pub project_binding: ProjectBinding,
    #[serde(default, alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default)]
    pub overrides: Option<SlotOverrides>,
}

impl From<TemplatesComposeAgentSlotRequest> for AgentSlot {
    fn from(value: TemplatesComposeAgentSlotRequest) -> Self {
        Self {
            role_id: value.role_id,
            count: value.count,
            project_binding: value.project_binding,
            project_id: value.project_id,
            overrides: value.overrides,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesComposeTeamRequest {
    #[serde(alias = "lead_role_id")]
    pub lead_role_id: String,
    #[serde(default, alias = "agent_slots")]
    pub agent_slots: Vec<TemplatesComposeAgentSlotRequest>,
    #[serde(default)]
    pub overrides: CompositionOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateStorageStatus {
    pub mode: TemplateStorageMode,
    pub repo_initialized: bool,
    pub dirty: bool,
    pub pending_actions: Vec<PendingAction>,
    pub last_commit: Option<i64>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRevertRequest {
    pub id: String,
    pub commit_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFlushResult {
    pub committed: bool,
    pub commit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TemplateImportResult {
    Role { template: RoleTemplate },
    Preset { preset: TeamPreset },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesApplyCompositionRequest {
    pub initialize_request: InitializeTeamRequest,
}

#[tauri::command]
pub fn templates_list_roles(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<RoleTemplateSummary>, String> {
    let store = &state.0;
    store
        .list_roles()
        .map(|roles| roles.into_iter().map(map_role_summary).collect())
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_list_roles_full(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<RoleTemplateFull>, String> {
    let store = &state.0;
    store
        .list_roles()
        .map(|roles| roles.into_iter().map(map_role_full).collect())
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_get_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> Result<RoleTemplate, String> {
    let store = &state.0;
    store
        .get_role(&role_id)
        .map(|record| record.template)
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_upsert_role(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertRoleRequest,
) -> Result<RoleTemplate, String> {
    let store = &state.0;
    let role_id = request.template.role_id.clone();

    match store.get_role(&role_id) {
        Ok(_) => store.update_role(&role_id, &request.template),
        Err(TemplateStoreError::NotFound(_)) => store.create_role(&request.template),
        Err(err) => Err(err),
    }
    .map_err(map_template_error)?;

    store
        .get_role(&role_id)
        .map(|record| record.template)
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_delete_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> Result<(), String> {
    let store = &state.0;
    store.delete_role(&role_id).map_err(map_template_error)?;
    Ok(())
}

#[tauri::command]
pub fn templates_list_presets(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<TeamPresetSummary>, String> {
    let store = &state.0;
    store
        .list_presets()
        .map(|presets| presets.into_iter().map(map_preset_summary).collect())
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_list_presets_full(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<TeamPresetFull>, String> {
    let store = &state.0;
    store
        .list_presets()
        .map(|presets| presets.into_iter().map(map_preset_full).collect())
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_get_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> Result<TeamPreset, String> {
    let store = &state.0;
    store
        .get_preset(&preset_id)
        .map(|record| record.template)
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_upsert_preset(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertPresetRequest,
) -> Result<TeamPreset, String> {
    let store = &state.0;
    let preset_id = request.preset.preset_id.clone();

    match store.get_preset(&preset_id) {
        Ok(_) => store.update_preset(&preset_id, &request.preset),
        Err(TemplateStoreError::NotFound(_)) => store.create_preset(&request.preset),
        Err(err) => Err(err),
    }
    .map_err(map_template_error)?;

    store
        .get_preset(&preset_id)
        .map(|record| record.template)
        .map_err(map_template_error)
}

#[tauri::command]
pub fn templates_delete_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> Result<(), String> {
    let store = &state.0;
    store
        .delete_preset(&preset_id)
        .map_err(map_template_error)?;
    Ok(())
}

#[tauri::command]
pub fn templates_compose_team(
    state: State<'_, TemplateStoreState>,
    request: TemplatesComposeTeamRequest,
) -> Result<CompositionResult, String> {
    let store = &state.0;
    let catalog = store.load_catalog().map_err(map_template_error)?;
    let agent_slots: Vec<AgentSlot> = request.agent_slots.into_iter().map(Into::into).collect();
    Ok(compose_team(
        &request.lead_role_id,
        &agent_slots,
        &catalog.roles,
        &request.overrides,
    ))
}

#[tauri::command]
pub fn templates_validate_composition(
    state: State<'_, TemplateStoreState>,
    request: TemplatesComposeTeamRequest,
) -> Result<CompositionResult, String> {
    templates_compose_team(state, request)
}

#[cfg(feature = "mesh-bridged-backend")]
#[tauri::command]
pub fn templates_apply_composition(
    app: AppHandle,
    db: State<'_, DbState>,
    coordination_state: State<'_, CoordinationState>,
    request: TemplatesApplyCompositionRequest,
) -> Result<InitializeReport, String> {
    coordination::coordination_initialize_team(
        app,
        db,
        coordination_state,
        request.initialize_request,
    )
    .map_err(|err| err.message)
}

#[tauri::command]
pub fn templates_get_storage_status(
    state: State<'_, TemplateStoreState>,
) -> Result<TemplateStorageStatus, String> {
    let store = &state.0;
    store.ensure_directories().map_err(map_template_error)?;
    let persisted = store.load_state().map_err(map_template_error)?;

    let git_dir = store.templates_dir().join(".git");
    let mode = if git_dir.exists() {
        TemplateStorageMode::Git
    } else {
        TemplateStorageMode::PlainFilesystem
    };

    let dirty = if git_dir.exists() {
        match Repository::open(store.templates_dir()) {
            Ok(repo) => has_managed_dirty_status(&repo).unwrap_or(false),
            Err(_) => false,
        }
    } else {
        false
    };

    Ok(TemplateStorageStatus {
        mode,
        repo_initialized: persisted.repo_initialized || git_dir.exists(),
        dirty,
        pending_actions: persisted.pending_actions,
        last_commit: persisted.last_commit_at,
    })
}

#[tauri::command]
pub fn templates_get_history(
    state: State<'_, TemplateStoreState>,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<TemplateCommitPage, String> {
    let store = &state.0;
    store.ensure_directories().map_err(map_template_error)?;

    let git_dir = store.templates_dir().join(".git");
    if !git_dir.exists() {
        return Ok(TemplateCommitPage {
            commits: Vec::new(),
            next_cursor: None,
        });
    }

    let repo =
        Repository::open(store.templates_dir()).map_err(|err| sanitize_error(&err.to_string()))?;
    let mut revwalk = repo
        .revwalk()
        .map_err(|err| sanitize_error(&err.to_string()))?;
    if revwalk.push_head().is_err() {
        return Ok(TemplateCommitPage {
            commits: Vec::new(),
            next_cursor: None,
        });
    }
    revwalk
        .set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|err| sanitize_error(&err.to_string()))?;

    let max = limit.unwrap_or(50).clamp(1, 200);
    let mut commits = Vec::new();
    let mut can_collect = cursor.is_none();

    for oid_result in revwalk {
        let oid = oid_result.map_err(|err| sanitize_error(&err.to_string()))?;
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

        let commit = repo
            .find_commit(oid)
            .map_err(|err| sanitize_error(&err.to_string()))?;
        let changed_paths = commit_changed_template_paths(&repo, &commit)
            .map_err(|err| sanitize_error(&err.to_string()))?;
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

#[tauri::command]
pub fn templates_get_diff(
    state: State<'_, TemplateStoreState>,
    commit_id: String,
) -> Result<TemplateDiff, String> {
    let store = &state.0;
    store.ensure_directories().map_err(map_template_error)?;

    let files = get_commit_files(store.templates_dir(), &commit_id)
        .map_err(|err| sanitize_error(&err.to_string()))?;

    let mut out_files = Vec::new();
    let mut insertions = 0u32;
    let mut deletions = 0u32;

    for file in files {
        if !is_managed_template_path(file.path.as_str()) {
            continue;
        }

        let hunks = get_commit_diff(store.templates_dir(), &commit_id, &file.path)
            .map_err(|err| sanitize_error(&err.to_string()))?;

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
        commit_id,
        files: out_files,
        stats,
    })
}

#[tauri::command]
pub fn templates_revert(
    state: State<'_, TemplateStoreState>,
    request: TemplateRevertRequest,
) -> Result<(), String> {
    if !is_valid_template_id(&request.id) {
        return Err("invalid template id".to_string());
    }

    let store = &state.0;
    store.ensure_directories().map_err(map_template_error)?;

    let repo =
        Repository::open(store.templates_dir()).map_err(|err| sanitize_error(&err.to_string()))?;
    let object = repo
        .revparse_single(&request.commit_hash)
        .map_err(|err| sanitize_error(&err.to_string()))?;
    let commit = object
        .peel_to_commit()
        .map_err(|err| sanitize_error(&err.to_string()))?;
    let tree = commit
        .tree()
        .map_err(|err| sanitize_error(&err.to_string()))?;

    let candidates = [
        format!("roles/{}.yaml", request.id),
        format!("presets/{}.yaml", request.id),
    ];

    let mut touched = Vec::new();
    let mut mutations = Vec::new();
    for rel in candidates {
        let rel_path = PathBuf::from(&rel);
        if let Ok(entry) = tree.get_path(Path::new(&rel)) {
            let obj = entry
                .to_object(&repo)
                .map_err(|err| sanitize_error(&err.to_string()))?;
            if let Some(blob) = obj.as_blob() {
                mutations.push(TemplateFileMutation::write(
                    rel_path.clone(),
                    blob.content().to_vec(),
                ));
                touched.push(rel_path);
            }
            continue;
        }

        let abs = store.templates_dir().join(&rel_path);
        if abs.exists() {
            mutations.push(TemplateFileMutation::delete(rel_path.clone()));
            touched.push(rel_path);
        }
    }

    if touched.is_empty() {
        return Err(format!("template '{}' not found in commit", request.id));
    }

    let short = format!("{:.8}", commit.id());
    let _ = store
        .mutate_and_commit(
            &mutations,
            &format!("templates: revert template {} to {}", request.id, short),
        )
        .map_err(map_template_error)?;
    let _ = store.flush_pending_commits().map_err(map_template_error)?;
    Ok(())
}

#[tauri::command]
pub fn templates_flush_pending(
    state: State<'_, TemplateStoreState>,
) -> Result<TemplateFlushResult, String> {
    let store = &state.0;
    let commit_id = store.flush_pending_commits().map_err(map_template_error)?;
    Ok(TemplateFlushResult {
        committed: commit_id.is_some(),
        commit_id,
    })
}

#[tauri::command]
pub fn templates_import(
    state: State<'_, TemplateStoreState>,
    path: String,
) -> Result<TemplateImportResult, String> {
    let store = &state.0;
    let source = PathBuf::from(path);
    let raw = fs::read_to_string(&source).map_err(|err| sanitize_error(&err.to_string()))?;

    let role_error = match serde_yaml::from_str::<RoleTemplate>(&raw) {
        Ok(role) => match role.validate() {
            Ok(()) => {
                store.import_role(&source).map_err(map_template_error)?;
                let imported = store
                    .get_role(&role.role_id)
                    .map_err(map_template_error)?
                    .template;
                return Ok(TemplateImportResult::Role { template: imported });
            }
            Err(err) => format!("role validation failed: {err}"),
        },
        Err(err) => format!("role parse failed: {err}"),
    };

    let preset_error = match serde_yaml::from_str::<TeamPreset>(&raw) {
        Ok(preset) => {
            let role_catalog = store.load_catalog().map_err(map_template_error)?.roles;
            match preset.validate_with_role_catalog(&role_catalog) {
                Ok(()) => {
                    store.import_preset(&source).map_err(map_template_error)?;
                    let imported = store
                        .get_preset(&preset.preset_id)
                        .map_err(map_template_error)?
                        .template;
                    return Ok(TemplateImportResult::Preset { preset: imported });
                }
                Err(err) => format!("preset validation failed: {err}"),
            }
        }
        Err(err) => format!("preset parse failed: {err}"),
    };

    Err(sanitize_error(&format!(
        "file is neither a valid role template nor team preset ({}; {})",
        role_error, preset_error
    )))
}

fn map_preset_summary(record: TeamPresetRecord) -> TeamPresetSummary {
    TeamPresetSummary {
        preset_id: record.template.preset_id,
        name: record.template.name,
        description: record.template.description,
        version: record.template.version,
        source: record.source,
        read_only: record.read_only,
    }
}

fn map_role_summary(record: RoleTemplateRecord) -> RoleTemplateSummary {
    RoleTemplateSummary {
        role_id: record.template.role_id,
        name: record.template.name,
        version: record.template.version,
        kind: record.template.kind,
        source: record.source,
        read_only: record.read_only,
    }
}

fn map_role_full(record: RoleTemplateRecord) -> RoleTemplateFull {
    RoleTemplateFull {
        template: record.template,
        source: record.source,
        read_only: record.read_only,
    }
}

fn map_preset_full(record: TeamPresetRecord) -> TeamPresetFull {
    TeamPresetFull {
        template: record.template,
        source: record.source,
        read_only: record.read_only,
    }
}

fn map_template_error(err: TemplateStoreError) -> String {
    sanitize_error(&err.to_string())
}

fn is_managed_template_path(path: &str) -> bool {
    path.starts_with("roles/") || path.starts_with("presets/") || path.starts_with("_meta/")
}

fn is_valid_template_id(id: &str) -> bool {
    !id.trim().is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn has_managed_dirty_status(repo: &Repository) -> Result<bool, git2::Error> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(statuses
        .iter()
        .any(|entry| entry.path().map(is_managed_template_path).unwrap_or(false)))
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
                if is_managed_template_path(path) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::storage::TemplateSource;
    use crate::templates::types::{TemplateKind, TemplateSchema};

    #[test]
    fn managed_template_path_filter_matches_expected_prefixes() {
        assert!(is_managed_template_path("roles/dev.yaml"));
        assert!(is_managed_template_path("presets/fullstack.yaml"));
        assert!(is_managed_template_path("_meta/state.json"));
        assert!(!is_managed_template_path("README.md"));
    }

    #[test]
    fn template_id_validation_rejects_path_traversal() {
        assert!(is_valid_template_id("qa_reviewer-1"));
        assert!(!is_valid_template_id("../etc/passwd"));
        assert!(!is_valid_template_id("role with spaces"));
    }

    #[test]
    fn preset_summary_maps_source_metadata() {
        let record = TeamPresetRecord {
            template: TeamPreset {
                schema: TemplateSchema {
                    kind: TemplateKind::TeamPreset,
                    version: 1,
                },
                preset_id: "preset".to_string(),
                name: "Preset".to_string(),
                description: "desc".to_string(),
                version: "1.0.0".to_string(),
                lead_role_id: "lead".to_string(),
                agent_slots: Vec::new(),
                defaults: crate::templates::types::TeamPresetDefaults {
                    team_name_pattern: "{project}".to_string(),
                    tmux_layout: "tiled".to_string(),
                },
            },
            source: TemplateSource::BuiltIn,
            read_only: true,
        };

        let summary = map_preset_summary(record);
        assert_eq!(summary.source, TemplateSource::BuiltIn);
        assert!(summary.read_only);
    }

    #[test]
    fn compose_request_accepts_camel_case_agent_slot_fields() {
        let value = serde_json::json!({
            "leadRoleId": "claude-orchestrator",
            "agentSlots": [
                {
                    "roleId": "codex-developer",
                    "count": 2,
                    "projectBinding": "lead_project",
                    "projectId": null
                }
            ]
        });

        let request: TemplatesComposeTeamRequest =
            serde_json::from_value(value).expect("request should deserialize");
        assert_eq!(request.agent_slots.len(), 1);
        assert_eq!(request.agent_slots[0].role_id, "codex-developer");
        assert_eq!(
            request.agent_slots[0].project_binding,
            ProjectBinding::LeadProject
        );
    }

    #[test]
    fn compose_request_accepts_snake_case_agent_slot_aliases() {
        let value = serde_json::json!({
            "lead_role_id": "claude-orchestrator",
            "agent_slots": [
                {
                    "role_id": "codex-developer",
                    "count": 1,
                    "project_binding": "lead_project",
                    "project_id": null
                }
            ]
        });

        let request: TemplatesComposeTeamRequest =
            serde_json::from_value(value).expect("request should deserialize");
        assert_eq!(request.lead_role_id, "claude-orchestrator");
        assert_eq!(request.agent_slots[0].role_id, "codex-developer");
    }
}
