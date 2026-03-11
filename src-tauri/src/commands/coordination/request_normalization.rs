use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::commands::coordination_types::{
    AddAgentRequest, AgentSetupConfig, InitializeTeamRequest,
};
use crate::commands::projects::DbState;
use crate::coordination::state::CoordinationState;
use crate::errors::sanitize_error;
use crate::templates::composition::{compose_team, CompositionOverrides, ResolvedMember};
use crate::templates::storage::{TemplateStore, TemplateStoreError};
use crate::templates::types::RoleTemplate;

pub(super) fn validate_and_collect_preflight_agents(
    request: InitializeTeamRequest,
) -> Result<Vec<crate::coordination::backend::bridged::PreflightAgent>, String> {
    super::validate_non_empty("team_name", &request.team_name)?;
    super::validate_non_empty("lead.name", &request.lead.name)?;
    super::validate_non_empty("lead.cli_tool", &request.lead.cli_tool)?;
    for (idx, agent) in request.agents.iter().enumerate() {
        super::validate_non_empty(&format!("agents[{idx}].name"), &agent.name)?;
        super::validate_non_empty(&format!("agents[{idx}].cli_tool"), &agent.cli_tool)?;
    }

    let mut preflight_agents = Vec::with_capacity(1 + request.agents.len());
    preflight_agents.push(crate::coordination::backend::bridged::PreflightAgent {
        agent_name: request.lead.name,
        cli_tool: request.lead.cli_tool,
    });
    for agent in request.agents {
        preflight_agents.push(crate::coordination::backend::bridged::PreflightAgent {
            agent_name: agent.name,
            cli_tool: agent.cli_tool,
        });
    }
    Ok(preflight_agents)
}

pub(super) fn normalize_initialize_request_paths(
    db: &DbState,
    mut request: InitializeTeamRequest,
) -> Result<InitializeTeamRequest, String> {
    request.lead.project_id = resolve_project_reference(db, &request.lead.project_id)?;
    for agent in &mut request.agents {
        agent.project_id = resolve_project_reference(db, &agent.project_id)?;
    }
    Ok(request)
}

pub(super) fn normalize_add_agent_request_path(
    db: &DbState,
    mut request: AddAgentRequest,
) -> Result<AddAgentRequest, String> {
    request.agent.project_id = resolve_project_reference(db, &request.agent.project_id)?;
    Ok(request)
}

pub(super) fn hydrate_initialize_request_role_metadata(
    state: &CoordinationState,
    mut request: InitializeTeamRequest,
) -> Result<InitializeTeamRequest, String> {
    if let Some(preset_id) = request
        .preset_id
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        hydrate_initialize_request_from_preset(state, &mut request, &preset_id)?;
        return Ok(request);
    }

    hydrate_agent_setup_from_role_template(state, &mut request.lead)?;
    for agent in &mut request.agents {
        hydrate_agent_setup_from_role_template(state, agent)?;
    }
    Ok(request)
}

pub(super) fn hydrate_add_agent_request_role_metadata(
    state: &CoordinationState,
    mut request: AddAgentRequest,
) -> Result<AddAgentRequest, String> {
    hydrate_agent_setup_from_role_template(state, &mut request.agent)?;
    Ok(request)
}

fn hydrate_initialize_request_from_preset(
    state: &CoordinationState,
    request: &mut InitializeTeamRequest,
    preset_id: &str,
) -> Result<(), String> {
    let store = TemplateStore::new(coordination_app_data_dir(state));
    let catalog = store.load_catalog().map_err(map_template_store_error)?;
    let preset = catalog
        .presets
        .iter()
        .find(|entry| entry.preset_id == preset_id)
        .ok_or_else(|| sanitize_error(&format!("unknown preset_id '{preset_id}'")))?;
    let role_names: HashMap<&str, &str> = catalog
        .roles
        .iter()
        .map(|role| (role.role_id.as_str(), role.name.as_str()))
        .collect();
    let composition = compose_team(
        &preset.lead_role_id,
        &preset.agent_slots,
        &catalog.roles,
        &CompositionOverrides::default(),
    );

    if !composition.validation_errors.is_empty() {
        return Err(sanitize_error(&format!(
            "preset '{}' could not be resolved: {}",
            preset_id,
            composition.validation_errors.join("; ")
        )));
    }

    let Some(resolved_lead) = composition.roster.first() else {
        return Err(sanitize_error(&format!(
            "preset '{}' resolved no lead member",
            preset_id
        )));
    };

    if composition.roster.len() != request.agents.len() + 1 {
        return Err(sanitize_error(&format!(
            "preset '{}' expected {} agents but initialize request provided {}",
            preset_id,
            composition.roster.len().saturating_sub(1),
            request.agents.len()
        )));
    }

    apply_resolved_member_defaults(
        &mut request.lead,
        resolved_lead,
        role_names.get(resolved_lead.role_id.as_str()).copied(),
    );
    for (agent, resolved) in request
        .agents
        .iter_mut()
        .zip(composition.roster.iter().skip(1))
    {
        apply_resolved_member_defaults(
            agent,
            resolved,
            role_names.get(resolved.role_id.as_str()).copied(),
        );
    }

    Ok(())
}

fn hydrate_agent_setup_from_role_template(
    state: &CoordinationState,
    agent: &mut AgentSetupConfig,
) -> Result<(), String> {
    let Some(role_id) = agent.role_id.as_deref() else {
        return Ok(());
    };
    if !agent_role_metadata_missing(agent) {
        return Ok(());
    }

    let store = TemplateStore::new(coordination_app_data_dir(state));
    let role = match store.get_role(role_id) {
        Ok(record) => record.template,
        Err(TemplateStoreError::NotFound(_)) => return Ok(()),
        Err(err) => return Err(map_template_store_error(err)),
    };
    apply_role_template_defaults(agent, &role);
    Ok(())
}

fn apply_resolved_member_defaults(
    agent: &mut AgentSetupConfig,
    member: &ResolvedMember,
    role_name: Option<&str>,
) {
    if agent.cli_tool.trim().is_empty() {
        agent.cli_tool = member.cli_tool.to_string();
    }
    if agent.model.trim().is_empty() {
        agent.model = member.model.clone();
    }
    if agent
        .role_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.role_id = Some(member.role_id.clone());
    }
    if agent
        .role_name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.role_name = role_name.map(str::to_string);
    }
    if agent
        .focus_area
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.focus_area = member.focus_area.clone();
    }
    if agent
        .context_summary
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.context_summary = member.context_summary.clone();
    }
    if agent
        .behavior_summary
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.behavior_summary = member.behavior_summary.clone();
    }
    if agent.runtime_compact_summary.is_none() {
        agent.runtime_compact_summary = member.runtime_compact_summary.clone();
    }
    if agent
        .instructions
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.instructions = Some(member.instructions.clone());
    }
    if agent.behavioral_contract.is_none() {
        agent.behavioral_contract = Some(member.behavioral_contract.clone());
    }
    if agent.capabilities.is_none() {
        agent.capabilities = Some(member.capabilities.clone());
    }
}

fn agent_role_metadata_missing(agent: &AgentSetupConfig) -> bool {
    agent
        .role_name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
        || agent
            .focus_area
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        || agent
            .context_summary
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        || agent
            .behavior_summary
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        || agent.runtime_compact_summary.is_none()
        || agent
            .instructions
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        || agent.behavioral_contract.is_none()
        || agent.capabilities.is_none()
}

fn apply_role_template_defaults(agent: &mut AgentSetupConfig, role: &RoleTemplate) {
    if agent
        .role_name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.role_name = Some(role.name.clone());
    }
    if agent
        .focus_area
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.focus_area = role.focus_area.clone();
    }
    if agent
        .context_summary
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.context_summary = role.context_summary.clone();
    }
    if agent
        .behavior_summary
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.behavior_summary = role.behavior_summary.clone();
    }
    if agent.runtime_compact_summary.is_none() {
        agent.runtime_compact_summary = role.runtime_compact_summary.clone();
    }
    if agent
        .instructions
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        agent.instructions = Some(role.instructions.clone());
    }
    if agent.behavioral_contract.is_none() {
        agent.behavioral_contract = Some(role.behavioral_contract.clone());
    }
    if agent.capabilities.is_none() {
        agent.capabilities = Some(role.capabilities.clone());
    }
}

fn coordination_app_data_dir(state: &CoordinationState) -> PathBuf {
    state
        .teams_dir()
        .file_name()
        .filter(|name| *name == "teams")
        .and_then(|_| state.teams_dir().parent().map(Path::to_path_buf))
        .unwrap_or_else(|| state.teams_dir().clone())
}

fn map_template_store_error(err: TemplateStoreError) -> String {
    sanitize_error(&err.to_string())
}

#[cfg(not(test))]
fn resolve_project_reference(db: &DbState, project_ref: &str) -> Result<String, String> {
    super::validate_non_empty("project_id", project_ref)?;
    let trimmed = project_ref.trim();

    let project_path = {
        let conn = db.0.lock().map_err(|err| format!("{err}"))?;
        match crate::db::queries::get_project(&conn, trimmed).map_err(|err| format!("{err}"))? {
            Some(project) => project.path,
            None => trimmed.to_string(),
        }
    };

    Ok(crate::provider::path::to_linux(&project_path).unwrap_or(project_path))
}

#[cfg(test)]
fn resolve_project_reference(_db: &DbState, project_ref: &str) -> Result<String, String> {
    super::validate_non_empty("project_id", project_ref)?;
    Ok(project_ref.trim().to_string())
}
