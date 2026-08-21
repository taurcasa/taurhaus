use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::errors::{sanitize_error, CommandResultExt, IpcResult};
use crate::templates::adapters::{export_role, RoleExportFormat, RoleExportResult};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRoleToFileRequest {
    pub role_id: String,
    pub target_format: RoleExportFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRoleFromFileRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRoleFromFileResult {
    pub success: bool,
    pub role: RoleTemplate,
    pub conflict: Option<RoleTemplateFull>,
}

#[tauri::command]
pub fn templates_list_roles_full(
    state: State<'_, TemplateStoreState>,
) -> IpcResult<Vec<RoleTemplateFull>> {
    let span = IpcCommandSpan::start("templates_list_roles_full");
    let result: Result<Vec<RoleTemplateFull>, String> = {
        let store = &state.0;
        store
            .list_roles()
            .map(|roles| roles.into_iter().map(map_role_full).collect())
            .map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_list_roles_full");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> IpcResult<RoleTemplate> {
    let span = IpcCommandSpan::start("templates_get_role");
    let result: Result<RoleTemplate, String> = {
        let store = &state.0;
        store
            .get_role(&role_id)
            .map(|record| record.template)
            .map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_get_role");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_upsert_role(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertRoleRequest,
) -> IpcResult<RoleTemplate> {
    let span = IpcCommandSpan::start("templates_upsert_role");
    let result: Result<RoleTemplate, String> = {
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
    };
    let result = result.ipc_cmd("templates_upsert_role");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_delete_role(
    state: State<'_, TemplateStoreState>,
    role_id: String,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("templates_delete_role");
    let result: Result<(), String> = {
        let store = &state.0;
        store.delete_role(&role_id).map_err(map_template_error)?;
        Ok(())
    };
    let result = result.ipc_cmd("templates_delete_role");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn import_role_from_file(
    state: State<'_, TemplateStoreState>,
    request: ImportRoleFromFileRequest,
) -> IpcResult<ImportRoleFromFileResult> {
    let span = IpcCommandSpan::start("import_role_from_file");
    let result = import_role_from_file_internal(&state.0, request).ipc_cmd("import_role_from_file");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_list_presets_full(
    state: State<'_, TemplateStoreState>,
) -> IpcResult<Vec<TeamPresetFull>> {
    let span = IpcCommandSpan::start("templates_list_presets_full");
    let result: Result<Vec<TeamPresetFull>, String> = {
        let store = &state.0;
        store
            .list_presets()
            .map(|presets| presets.into_iter().map(map_preset_full).collect())
            .map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_list_presets_full");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> IpcResult<TeamPreset> {
    let span = IpcCommandSpan::start("templates_get_preset");
    let result: Result<TeamPreset, String> = {
        let store = &state.0;
        store
            .get_preset(&preset_id)
            .map(|record| record.template)
            .map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_get_preset");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_upsert_preset(
    state: State<'_, TemplateStoreState>,
    request: TemplatesUpsertPresetRequest,
) -> IpcResult<TeamPreset> {
    let span = IpcCommandSpan::start("templates_upsert_preset");
    let result: Result<TeamPreset, String> = {
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
    };
    let result = result.ipc_cmd("templates_upsert_preset");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_delete_preset(
    state: State<'_, TemplateStoreState>,
    preset_id: String,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("templates_delete_preset");
    let result: Result<(), String> = {
        let store = &state.0;
        store
            .delete_preset(&preset_id)
            .map_err(map_template_error)?;
        Ok(())
    };
    let result = result.ipc_cmd("templates_delete_preset");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_compose_team(
    state: State<'_, TemplateStoreState>,
    request: TemplatesComposeTeamRequest,
) -> IpcResult<CompositionResult> {
    let span = IpcCommandSpan::start("templates_compose_team");
    let result: Result<CompositionResult, String> = {
        let store = &state.0;
        let catalog = store.load_catalog().map_err(map_template_error)?;
        let agent_slots: Vec<AgentSlot> = request.agent_slots.into_iter().map(Into::into).collect();
        Ok(compose_team(
            &request.lead_role_id,
            &agent_slots,
            &catalog.roles,
            &request.overrides,
        ))
    };
    let result = result.ipc_cmd("templates_compose_team");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_storage_status(
    state: State<'_, TemplateStoreState>,
) -> IpcResult<TemplateStorageStatus> {
    let span = IpcCommandSpan::start("templates_get_storage_status");
    let result: Result<TemplateStorageStatus, String> = {
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
    };
    let result = result.ipc_cmd("templates_get_storage_status");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_history(
    state: State<'_, TemplateStoreState>,
    limit: Option<usize>,
    cursor: Option<String>,
) -> IpcResult<TemplateCommitPage> {
    let span = IpcCommandSpan::start("templates_get_history");
    let result: Result<TemplateCommitPage, String> = {
        let store = &state.0;
        store.get_history(limit, cursor).map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_get_history");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_get_diff(
    state: State<'_, TemplateStoreState>,
    commit_id: String,
) -> IpcResult<TemplateDiff> {
    let span = IpcCommandSpan::start("templates_get_diff");
    let result: Result<TemplateDiff, String> = {
        let store = &state.0;
        store.get_diff(&commit_id).map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_get_diff");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_revert(
    state: State<'_, TemplateStoreState>,
    request: TemplateRevertRequest,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("templates_revert");
    let result: Result<(), String> = {
        let store = &state.0;
        store
            .revert_template(&request.id, &request.commit_hash)
            .map_err(map_template_error)
    };
    let result = result.ipc_cmd("templates_revert");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn templates_flush_pending(
    state: State<'_, TemplateStoreState>,
) -> IpcResult<TemplateFlushResult> {
    let span = IpcCommandSpan::start("templates_flush_pending");
    let result: Result<TemplateFlushResult, String> = {
        let store = &state.0;
        let commit_id = store.flush_pending_commits().map_err(map_template_error)?;
        Ok(TemplateFlushResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    };
    let result = result.ipc_cmd("templates_flush_pending");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn export_role_to_file(
    state: State<'_, TemplateStoreState>,
    request: ExportRoleToFileRequest,
) -> IpcResult<RoleExportResult> {
    let span = IpcCommandSpan::start("export_role_to_file");
    let result = export_role_to_file_internal(&state.0, request).ipc_cmd("export_role_to_file");
    span.finish_result(&result);
    result
}

fn export_role_to_file_internal(
    store: &TemplateStore,
    request: ExportRoleToFileRequest,
) -> Result<RoleExportResult, String> {
    store
        .get_role(&request.role_id)
        .map(|record| export_role(&record.template, request.target_format))
        .map_err(map_template_error)
}

fn import_role_from_file_internal(
    store: &TemplateStore,
    request: ImportRoleFromFileRequest,
) -> Result<ImportRoleFromFileResult, String> {
    let external_path = Path::new(&request.file_path);
    let prepared = store
        .prepare_role_import(external_path)
        .map_err(map_template_error)?;
    let role_id = prepared.template.role_id.clone();

    if let Ok(existing) = store.get_role(&role_id) {
        return Ok(ImportRoleFromFileResult {
            success: false,
            role: prepared.template,
            conflict: Some(map_role_full(existing)),
        });
    }

    store
        .save_prepared_role_import(&prepared)
        .map_err(map_template_error)?;

    store
        .get_role(&role_id)
        .map(|record| ImportRoleFromFileResult {
            success: true,
            role: record.template,
            conflict: None,
        })
        .map_err(map_template_error)
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
    use crate::session_scanner::cli_tool::CliTool;
    use crate::templates::types::{
        BehavioralContract, RoleConstraints, RoleDefaults, RoleKind, TemplateKind, TemplateSchema,
    };
    use tempfile::TempDir;

    #[derive(Debug, Deserialize)]
    struct ClaudeAgentFrontmatter {
        name: String,
        model: String,
        #[serde(default)]
        tools: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct CopilotAgentFrontmatter {
        name: String,
        description: String,
        model: String,
    }

    fn sample_role() -> RoleTemplate {
        RoleTemplate {
            schema: TemplateSchema {
                kind: TemplateKind::RoleTemplate,
                version: 1,
            },
            role_id: "sample-export-role".to_string(),
            name: "Sample Export Role".to_string(),
            version: "1.0.0".to_string(),
            kind: RoleKind::Agent,
            defaults: RoleDefaults {
                cli_tool: CliTool::Claude,
                model: "claude-opus-4-6".to_string(),
                reasoning_effort: None,
                default_name_pattern: "export-{n}".to_string(),
            },
            instructions: "Ship the requested change with tests first.".to_string(),
            focus_area: Some("Backend export pipelines".to_string()),
            context_summary: Some(
                "Keeps role portability constraints in working memory.".to_string(),
            ),
            behavior_summary: Some("Writes clean exports and flags lossy conversions.".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            behavioral_contract: BehavioralContract {
                communication: vec!["Post concise progress updates.".to_string()],
                execution: vec!["Validate generated output.".to_string()],
                escalation: vec!["Escalate unsupported mappings.".to_string()],
            },
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: vec![
                "read".to_string(),
                "write".to_string(),
                "shell".to_string(),
                "unknown".to_string(),
            ],
            provenance: None,
            constraints: RoleConstraints {
                min_instances: 0,
                max_instances: 2,
                requires_lead_tool: Some(CliTool::Codex),
                allowed_project_binding: ProjectBinding::LeadProject,
            },
        }
    }

    fn extract_frontmatter(content: &str) -> (String, String) {
        let opening = content
            .strip_prefix("---\n")
            .expect("content should start with YAML frontmatter");
        let close = opening
            .find("\n---\n")
            .expect("content should include closing YAML frontmatter delimiter");
        let frontmatter = opening[..close].to_string();
        let body = opening[close + 5..].trim().to_string();
        (frontmatter, body)
    }

    fn store_state() -> (TempDir, TemplateStoreState) {
        let root = TempDir::new().expect("tempdir");
        let app_data = root.path().join("app-data");
        let builtins = root.path().join("builtins");
        std::fs::create_dir_all(builtins.join("roles")).expect("create builtins roles dir");
        std::fs::create_dir_all(builtins.join("presets")).expect("create builtins presets dir");

        (
            root,
            TemplateStoreState(TemplateStore::with_builtins_dir(app_data, builtins)),
        )
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

    #[test]
    fn export_role_to_file_returns_claude_agent_markdown_with_parseable_frontmatter() {
        let tmp = TempDir::new().expect("tempdir");
        let state = TemplateStoreState::new(tmp.path().to_path_buf());
        state.0.create_role(&sample_role()).expect("create role");

        let exported = export_role_to_file_internal(
            &state.0,
            ExportRoleToFileRequest {
                role_id: "sample-export-role".to_string(),
                target_format: RoleExportFormat::ClaudeAgent,
            },
        )
        .expect("export role");

        assert_eq!(exported.target_format, RoleExportFormat::ClaudeAgent);
        let (frontmatter, body) = extract_frontmatter(&exported.file_content);
        let parsed: ClaudeAgentFrontmatter =
            serde_norway::from_str(&frontmatter).expect("parse YAML frontmatter");

        assert_eq!(parsed.name, "Sample Export Role");
        assert_eq!(parsed.model, "claude-opus-4-6");
        assert_eq!(parsed.tools, vec!["read", "edit", "bash"]);
        assert!(body.contains("Ship the requested change with tests first."));
        assert!(body.contains("## Focus Area"));
        assert!(body.contains("## Behavioral Contract"));
        assert!(exported.lossy_fields.is_empty());
    }

    #[test]
    fn export_role_to_file_returns_copilot_agent_markdown_with_parseable_frontmatter() {
        let tmp = TempDir::new().expect("tempdir");
        let state = TemplateStoreState::new(tmp.path().to_path_buf());
        state.0.create_role(&sample_role()).expect("create role");

        let exported = export_role_to_file_internal(
            &state.0,
            ExportRoleToFileRequest {
                role_id: "sample-export-role".to_string(),
                target_format: RoleExportFormat::CopilotAgent,
            },
        )
        .expect("export role");

        assert_eq!(exported.target_format, RoleExportFormat::CopilotAgent);
        let (frontmatter, body) = extract_frontmatter(&exported.file_content);
        let parsed: CopilotAgentFrontmatter =
            serde_norway::from_str(&frontmatter).expect("parse YAML frontmatter");

        assert_eq!(parsed.name, "Sample Export Role");
        assert_eq!(
            parsed.description,
            "Writes clean exports and flags lossy conversions."
        );
        assert_eq!(parsed.model, "claude-opus-4-6");
        assert!(body.contains("## Context Summary"));
        assert!(body.contains("## Constraints"));
        assert!(exported.lossy_fields.is_empty());
    }

    #[test]
    fn export_role_to_file_returns_canonical_yaml_without_lossy_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let state = TemplateStoreState::new(tmp.path().to_path_buf());
        state.0.create_role(&sample_role()).expect("create role");

        let exported = export_role_to_file_internal(
            &state.0,
            ExportRoleToFileRequest {
                role_id: "sample-export-role".to_string(),
                target_format: RoleExportFormat::Yaml,
            },
        )
        .expect("export role");

        assert_eq!(exported.target_format, RoleExportFormat::Yaml);
        assert!(exported.lossy_fields.is_empty());

        let parsed = serde_norway::from_str::<RoleTemplate>(&exported.file_content)
            .expect("parse exported role yaml");
        assert_eq!(parsed.role_id, "sample-export-role");
        assert_eq!(parsed.name, "Sample Export Role");
        assert_eq!(parsed.defaults.model, "claude-opus-4-6");
    }

    #[test]
    fn export_role_to_file_returns_not_found_for_missing_role() {
        let tmp = TempDir::new().expect("tempdir");
        let state = TemplateStoreState::new(tmp.path().to_path_buf());

        let err = export_role_to_file_internal(
            &state.0,
            ExportRoleToFileRequest {
                role_id: "missing-role".to_string(),
                target_format: RoleExportFormat::ClaudeAgent,
            },
        )
        .expect_err("missing role should fail");

        assert!(err.contains("role 'missing-role' not found"));
    }

    #[test]
    fn import_role_from_file_returns_conflict_details_without_overwriting() {
        let (_root, state) = store_state();
        state.0.create_role(&sample_role()).expect("create role");

        let import_path = state.0.templates_dir().join("incoming.md");
        std::fs::write(
            &import_path,
            r#"---
name: Sample Export Role
model: claude-opus-4-6
---
Updated imported instructions.
"#,
        )
        .expect("write import file");

        let result = import_role_from_file_internal(
            &state.0,
            ImportRoleFromFileRequest {
                file_path: import_path.to_string_lossy().to_string(),
            },
        )
        .expect("import conflict result");

        assert!(!result.success);
        assert_eq!(result.role.role_id, "sample-export-role");
        assert_eq!(
            result
                .conflict
                .as_ref()
                .map(|role| role.template.role_id.as_str()),
            Some("sample-export-role")
        );
        assert_eq!(
            state
                .0
                .get_role("sample-export-role")
                .expect("existing role")
                .template
                .instructions,
            "Ship the requested change with tests first."
        );
    }

    #[test]
    fn import_role_from_file_saves_markdown_role_with_provenance() {
        let (root, state) = store_state();
        let import_path = root.path().join("architect.md");
        std::fs::write(
            &import_path,
            r#"---
name: Architecture Reviewer
model: claude-opus-4-6
---
Review structural changes and call out long-term tradeoffs.
"#,
        )
        .expect("write import file");

        let result = import_role_from_file_internal(
            &state.0,
            ImportRoleFromFileRequest {
                file_path: import_path.to_string_lossy().to_string(),
            },
        )
        .expect("import role");

        assert!(result.success);
        assert!(result.conflict.is_none());
        assert_eq!(result.role.role_id, "architecture-reviewer");
        assert_eq!(
            result
                .role
                .provenance
                .as_ref()
                .and_then(|value| value.source_path.as_deref()),
            Some(import_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            state
                .0
                .get_role("architecture-reviewer")
                .expect("stored role")
                .template
                .instructions,
            "Review structural changes and call out long-term tradeoffs."
        );
    }
}
