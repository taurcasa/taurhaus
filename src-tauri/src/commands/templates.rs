use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::errors::sanitize_error;
use crate::templates::composition::{compose_team, CompositionOverrides, CompositionResult};
use crate::templates::storage::{
    PendingAction, RoleTemplateRecord, TeamPresetRecord, TemplateCommitPage, TemplateDiff,
    TemplateSource, TemplateStore, TemplateStoreError,
};
use crate::templates::types::{AgentSlot, ProjectBinding, RoleTemplate, SlotOverrides, TeamPreset};

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

#[tauri::command]
pub fn templates_list_roles_full(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<RoleTemplateFull>, String> {
    let span = IpcCommandSpan::start("templates_list_roles_full");
    let result = (|| {
        let store = &state.0;
        store
            .list_roles()
            .map(|roles| roles.into_iter().map(map_role_full).collect())
            .map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> Result<RoleTemplate, String> {
    let span = IpcCommandSpan::start("templates_get_role");
    let result = (|| {
        let store = &state.0;
        store
            .get_role(&role_id)
            .map(|record| record.template)
            .map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_upsert_role(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertRoleRequest,
) -> Result<RoleTemplate, String> {
    let span = IpcCommandSpan::start("templates_upsert_role");
    let result = (|| {
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
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_delete_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("templates_delete_role");
    let result = (|| {
        let store = &state.0;
        store.delete_role(&role_id).map_err(map_template_error)?;
        Ok(())
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_list_presets_full(
    state: State<'_, TemplateStoreState>,
) -> Result<Vec<TeamPresetFull>, String> {
    let span = IpcCommandSpan::start("templates_list_presets_full");
    let result = (|| {
        let store = &state.0;
        store
            .list_presets()
            .map(|presets| presets.into_iter().map(map_preset_full).collect())
            .map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> Result<TeamPreset, String> {
    let span = IpcCommandSpan::start("templates_get_preset");
    let result = (|| {
        let store = &state.0;
        store
            .get_preset(&preset_id)
            .map(|record| record.template)
            .map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_upsert_preset(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertPresetRequest,
) -> Result<TeamPreset, String> {
    let span = IpcCommandSpan::start("templates_upsert_preset");
    let result = (|| {
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
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_delete_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("templates_delete_preset");
    let result = (|| {
        let store = &state.0;
        store
            .delete_preset(&preset_id)
            .map_err(map_template_error)?;
        Ok(())
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_compose_team(
    state: State<'_, TemplateStoreState>,
    request: TemplatesComposeTeamRequest,
) -> Result<CompositionResult, String> {
    let span = IpcCommandSpan::start("templates_compose_team");
    let result = (|| {
        let store = &state.0;
        let catalog = store.load_catalog().map_err(map_template_error)?;
        let agent_slots: Vec<AgentSlot> = request.agent_slots.into_iter().map(Into::into).collect();
        Ok(compose_team(
            &request.lead_role_id,
            &agent_slots,
            &catalog.roles,
            &request.overrides,
        ))
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_storage_status(
    state: State<'_, TemplateStoreState>,
) -> Result<TemplateStorageStatus, String> {
    let span = IpcCommandSpan::start("templates_get_storage_status");
    let result = (|| {
        let store = &state.0;
        store.ensure_directories().map_err(map_template_error)?;
        let persisted = store.load_state().map_err(map_template_error)?;

        let git_dir = store.templates_dir().join(".git");
        let mode = if git_dir.exists() {
            TemplateStorageMode::Git
        } else {
            TemplateStorageMode::PlainFilesystem
        };

        let dirty = store.managed_dirty_status().map_err(map_template_error)?;

        Ok(TemplateStorageStatus {
            mode,
            repo_initialized: persisted.repo_initialized || git_dir.exists(),
            dirty,
            pending_actions: persisted.pending_actions,
            last_commit: persisted.last_commit_at,
        })
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_history(
    state: State<'_, TemplateStoreState>,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<TemplateCommitPage, String> {
    let span = IpcCommandSpan::start("templates_get_history");
    let result = (|| {
        let store = &state.0;
        store.get_history(limit, cursor).map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_diff(
    state: State<'_, TemplateStoreState>,
    commit_id: String,
) -> Result<TemplateDiff, String> {
    let span = IpcCommandSpan::start("templates_get_diff");
    let result = (|| {
        let store = &state.0;
        store.get_diff(&commit_id).map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_revert(
    state: State<'_, TemplateStoreState>,
    request: TemplateRevertRequest,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("templates_revert");
    let result = (|| {
        let store = &state.0;
        store
            .revert_template(&request.id, &request.commit_hash)
            .map_err(map_template_error)
    })();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_flush_pending(
    state: State<'_, TemplateStoreState>,
) -> Result<TemplateFlushResult, String> {
    let span = IpcCommandSpan::start("templates_flush_pending");
    let result = (|| {
        let store = &state.0;
        let commit_id = store.flush_pending_commits().map_err(map_template_error)?;
        Ok(TemplateFlushResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    })();
    span.finish_result(&result);
    result
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

#[cfg(test)]
mod tests {
    use super::*;

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
