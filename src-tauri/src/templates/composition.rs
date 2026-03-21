use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::session_scanner::cli_tool::CliTool;
use crate::templates::types::{
    validate_agent_slot_common, AgentSlot, BehavioralContract, ProjectBinding, RoleKind,
    RoleTemplate, RuntimeCompactSummary, SlotOverrides,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompositionOverrides {
    pub lead: Option<SlotOverrides>,
    #[serde(default)]
    pub instances: Vec<InstanceOverride>,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceOverride {
    pub slot_index: usize,
    pub member_index: u32,
    pub overrides: SlotOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMember {
    pub name: String,
    pub role_id: String,
    pub role_kind: RoleKind,
    pub cli_tool: CliTool,
    pub model: String,
    pub instructions: String,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub communication_style: Option<String>,
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    pub behavioral_contract: BehavioralContract,
    pub quality_gates: Option<Vec<String>>,
    pub definition_of_done: Option<Vec<String>>,
    pub phase_scope: Option<Vec<String>>,
    pub mode: Option<String>,
    pub inherits_from: Option<String>,
    pub required_artifacts: Option<Vec<String>>,
    pub capabilities: Vec<String>,
    pub project_binding: ProjectBinding,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompositionResult {
    pub roster: Vec<ResolvedMember>,
    pub warnings: Vec<String>,
    pub validation_errors: Vec<String>,
}

impl CompositionResult {
    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_empty()
    }
}

#[derive(Debug, Clone)]
struct ResolvedFields {
    model: String,
    name_pattern: String,
    instructions: String,
    focus_area: Option<String>,
    context_summary: Option<String>,
    behavior_summary: Option<String>,
    communication_style: Option<String>,
    runtime_compact_summary: Option<RuntimeCompactSummary>,
    behavioral_contract: BehavioralContract,
    quality_gates: Option<Vec<String>>,
    definition_of_done: Option<Vec<String>>,
    phase_scope: Option<Vec<String>>,
    mode: Option<String>,
    inherits_from: Option<String>,
    required_artifacts: Option<Vec<String>>,
    capabilities: Vec<String>,
}

pub fn compose_team(
    lead_role_id: &str,
    agent_slots: &[AgentSlot],
    role_catalog: &[RoleTemplate],
    overrides: &CompositionOverrides,
) -> CompositionResult {
    let mut result = CompositionResult::default();
    let mut role_map = HashMap::new();
    let mut seen_names = HashMap::new();
    let mut role_instance_counts: HashMap<&str, u32> = HashMap::new();
    let mut used_instance_overrides = HashSet::new();

    let project_name = overrides.project_name.as_deref().unwrap_or("project");

    for role in role_catalog {
        if let Err(err) = role.validate() {
            for entry in err.errors {
                result.validation_errors.push(format!(
                    "role '{}' validation failed: {entry}",
                    role.role_id
                ));
            }
        }
        if role_map.insert(role.role_id.as_str(), role).is_some() {
            result.validation_errors.push(format!(
                "duplicate role_id '{}' in role catalog",
                role.role_id
            ));
        }
    }

    let lead_role = match role_map.get(lead_role_id) {
        Some(role) if role.kind == RoleKind::Lead => Some(*role),
        Some(_) => {
            result.validation_errors.push(format!(
                "lead_role_id '{}' must reference a lead role",
                lead_role_id
            ));
            None
        }
        None => {
            result
                .validation_errors
                .push(format!("unknown lead_role_id '{}'", lead_role_id));
            None
        }
    };

    let instance_override_map = index_instance_overrides(overrides, &mut result);

    for (slot_index, slot) in agent_slots.iter().enumerate() {
        validate_agent_slot(slot, slot_index, &mut result);

        let Some(role) = role_map.get(slot.role_id.as_str()) else {
            result.validation_errors.push(format!(
                "agent_slots[{slot_index}] references unknown role_id '{}'",
                slot.role_id
            ));
            continue;
        };

        if role.kind != RoleKind::Agent {
            result.validation_errors.push(format!(
                "agent_slots[{slot_index}] role_id '{}' must reference an agent role",
                role.role_id
            ));
        }

        let role_count = role_instance_counts
            .entry(role.role_id.as_str())
            .or_insert(0);
        *role_count += slot.count;

        if let Some(required_tool) = role.constraints.requires_lead_tool {
            if let Some(lead) = lead_role {
                if lead.defaults.cli_tool != required_tool {
                    result.validation_errors.push(format!(
                        "role '{}' requires lead tool '{}', found '{}'",
                        role.role_id, required_tool, lead.defaults.cli_tool
                    ));
                }
            }
        }

        match role.constraints.allowed_project_binding {
            ProjectBinding::Any => {}
            ProjectBinding::LeadProject => {
                if slot.project_binding != ProjectBinding::LeadProject {
                    result.validation_errors.push(format!(
                        "role '{}' requires project_binding 'lead_project'",
                        role.role_id
                    ));
                }
            }
            ProjectBinding::ExplicitProject => {
                if slot.project_binding != ProjectBinding::ExplicitProject {
                    result.validation_errors.push(format!(
                        "role '{}' requires project_binding 'explicit_project'",
                        role.role_id
                    ));
                }
            }
        }
    }

    for (role_id, count) in role_instance_counts {
        if let Some(role) = role_map.get(role_id) {
            if count < role.constraints.min_instances {
                result.validation_errors.push(format!(
                    "role '{}' count {} is below min_instances {}",
                    role_id, count, role.constraints.min_instances
                ));
            }
            if count > role.constraints.max_instances {
                result.validation_errors.push(format!(
                    "role '{}' count {} exceeds max_instances {}",
                    role_id, count, role.constraints.max_instances
                ));
            }
        }
    }

    if let Some(lead) = lead_role {
        let fields = resolve_fields(lead, None, overrides.lead.as_ref());
        let lead_name = apply_name_pattern(&fields.name_pattern, 1, project_name);
        if lead_name.trim().is_empty() {
            result.validation_errors.push(format!(
                "lead role '{}' produced empty member name from pattern '{}'",
                lead.role_id, fields.name_pattern
            ));
        } else {
            let final_name = uniquify_name(lead_name, &mut seen_names, &mut result.warnings);
            validate_model_compatibility(
                lead.defaults.cli_tool,
                &fields.model,
                &lead.role_id,
                &mut result.validation_errors,
            );

            result.roster.push(ResolvedMember {
                name: final_name,
                role_id: lead.role_id.clone(),
                role_kind: lead.kind,
                cli_tool: lead.defaults.cli_tool,
                model: fields.model,
                instructions: fields.instructions,
                focus_area: fields.focus_area,
                context_summary: fields.context_summary,
                behavior_summary: fields.behavior_summary,
                communication_style: fields.communication_style,
                runtime_compact_summary: fields.runtime_compact_summary,
                behavioral_contract: fields.behavioral_contract,
                quality_gates: fields.quality_gates,
                definition_of_done: fields.definition_of_done,
                phase_scope: fields.phase_scope,
                mode: fields.mode,
                inherits_from: fields.inherits_from,
                required_artifacts: fields.required_artifacts,
                capabilities: fields.capabilities,
                project_binding: ProjectBinding::LeadProject,
                project_id: None,
            });
        }
    }

    for (slot_index, slot) in agent_slots.iter().enumerate() {
        let Some(role) = role_map.get(slot.role_id.as_str()) else {
            continue;
        };

        for member_index in 1..=slot.count {
            let instance_key = (slot_index, member_index);
            let instance_override = instance_override_map.get(&instance_key).copied();
            if instance_override.is_some() {
                used_instance_overrides.insert(instance_key);
            }

            let fields = resolve_fields(role, slot.overrides.as_ref(), instance_override);
            let member_name = apply_name_pattern(&fields.name_pattern, member_index, project_name);

            if member_name.trim().is_empty() {
                result.validation_errors.push(format!(
                    "role '{}' produced empty member name from pattern '{}'",
                    role.role_id, fields.name_pattern
                ));
                continue;
            }

            validate_model_compatibility(
                role.defaults.cli_tool,
                &fields.model,
                &role.role_id,
                &mut result.validation_errors,
            );

            let final_name = uniquify_name(member_name, &mut seen_names, &mut result.warnings);
            let project_id = slot.project_id.as_deref().and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

            result.roster.push(ResolvedMember {
                name: final_name,
                role_id: role.role_id.clone(),
                role_kind: role.kind,
                cli_tool: role.defaults.cli_tool,
                model: fields.model,
                instructions: fields.instructions,
                focus_area: fields.focus_area,
                context_summary: fields.context_summary,
                behavior_summary: fields.behavior_summary,
                communication_style: fields.communication_style,
                runtime_compact_summary: fields.runtime_compact_summary,
                behavioral_contract: fields.behavioral_contract,
                quality_gates: fields.quality_gates,
                definition_of_done: fields.definition_of_done,
                phase_scope: fields.phase_scope,
                mode: fields.mode,
                inherits_from: fields.inherits_from,
                required_artifacts: fields.required_artifacts,
                capabilities: fields.capabilities,
                project_binding: slot.project_binding,
                project_id,
            });
        }
    }

    for ((slot_index, member_index), _) in instance_override_map {
        if !used_instance_overrides.contains(&(slot_index, member_index)) {
            result.warnings.push(format!(
                "unused instance override for slot_index={slot_index}, member_index={member_index}"
            ));
        }
    }

    result
}

fn index_instance_overrides<'a>(
    overrides: &'a CompositionOverrides,
    result: &mut CompositionResult,
) -> HashMap<(usize, u32), &'a SlotOverrides> {
    let mut map = HashMap::new();
    for override_entry in &overrides.instances {
        if override_entry.member_index == 0 {
            result.validation_errors.push(format!(
                "instance override for slot_index={} must have member_index >= 1",
                override_entry.slot_index
            ));
            continue;
        }

        if map
            .insert(
                (override_entry.slot_index, override_entry.member_index),
                &override_entry.overrides,
            )
            .is_some()
        {
            result.warnings.push(format!(
                "duplicate instance override for slot_index={}, member_index={} (last one wins)",
                override_entry.slot_index, override_entry.member_index
            ));
        }
    }
    map
}

fn validate_agent_slot(slot: &AgentSlot, slot_index: usize, result: &mut CompositionResult) {
    validate_agent_slot_common(slot, slot_index, &mut result.validation_errors);
}

fn resolve_fields(
    role: &RoleTemplate,
    slot_override: Option<&SlotOverrides>,
    instance_override: Option<&SlotOverrides>,
) -> ResolvedFields {
    let mut resolved = ResolvedFields {
        model: role.defaults.model.clone(),
        name_pattern: role.defaults.default_name_pattern.clone(),
        instructions: role.instructions.clone(),
        focus_area: role.focus_area.clone(),
        context_summary: role.context_summary.clone(),
        behavior_summary: role.behavior_summary.clone(),
        communication_style: role.communication_style.clone(),
        runtime_compact_summary: role.runtime_compact_summary.clone(),
        behavioral_contract: role.behavioral_contract.clone(),
        quality_gates: role.quality_gates.clone(),
        definition_of_done: role.definition_of_done.clone(),
        phase_scope: role.phase_scope.clone(),
        mode: role.mode.clone(),
        inherits_from: role.inherits_from.clone(),
        required_artifacts: role.required_artifacts.clone(),
        capabilities: role.capabilities.clone(),
    };

    if let Some(slot) = slot_override {
        apply_overrides(&mut resolved, slot);
    }
    if let Some(instance) = instance_override {
        apply_overrides(&mut resolved, instance);
    }

    resolved
}

fn apply_overrides(resolved: &mut ResolvedFields, overrides: &SlotOverrides) {
    if let Some(model) = overrides.model.as_deref() {
        resolved.model = model.to_string();
    }
    if let Some(name_pattern) = overrides.name_pattern.as_deref() {
        resolved.name_pattern = name_pattern.to_string();
    }

    if let Some(instructions_replace) = overrides.instructions_replace.as_deref() {
        resolved.instructions = instructions_replace.to_string();
    }
    if let Some(instructions_append) = overrides.instructions_append.as_deref() {
        if !resolved.instructions.trim().is_empty() {
            resolved.instructions.push('\n');
        }
        resolved.instructions.push_str(instructions_append);
    }
    if let Some(focus_area) = overrides.focus_area.as_deref() {
        resolved.focus_area = Some(focus_area.to_string());
    }
    if let Some(context_summary) = overrides.context_summary.as_deref() {
        resolved.context_summary = Some(context_summary.to_string());
    }
    if let Some(behavior_summary) = overrides.behavior_summary.as_deref() {
        resolved.behavior_summary = Some(behavior_summary.to_string());
    }
    if let Some(runtime_compact_summary) = overrides.runtime_compact_summary.as_ref() {
        resolved.runtime_compact_summary = Some(runtime_compact_summary.clone());
    }

    if let Some(contract_append) = overrides.behavioral_contract_append.as_ref() {
        resolved
            .behavioral_contract
            .communication
            .extend(contract_append.communication.iter().cloned());
        resolved
            .behavioral_contract
            .execution
            .extend(contract_append.execution.iter().cloned());
        resolved
            .behavioral_contract
            .escalation
            .extend(contract_append.escalation.iter().cloned());
    }
}

fn apply_name_pattern(pattern: &str, n: u32, project_name: &str) -> String {
    pattern
        .replace("{n}", &n.to_string())
        .replace("{project}", project_name)
}

fn uniquify_name(
    base_name: String,
    seen_names: &mut HashMap<String, u32>,
    warnings: &mut Vec<String>,
) -> String {
    let counter = seen_names.entry(base_name.clone()).or_insert(0);
    if *counter == 0 {
        *counter = 1;
        base_name
    } else {
        let suffix = *counter;
        *counter += 1;
        let resolved = format!("{base_name}-{suffix}");
        warnings.push(format!(
            "resolved name collision for '{base_name}' as '{resolved}'"
        ));
        resolved
    }
}

fn validate_model_compatibility(
    cli_tool: CliTool,
    model: &str,
    role_id: &str,
    errors: &mut Vec<String>,
) {
    if model_is_compatible(cli_tool, model) {
        return;
    }

    errors.push(format!(
        "role '{role_id}' resolved model '{model}' is incompatible with cli_tool '{}'",
        cli_tool
    ));
}

fn model_is_compatible(cli_tool: CliTool, model: &str) -> bool {
    match cli_tool {
        CliTool::Claude => model.starts_with("claude-"),
        CliTool::Codex => model.starts_with("gpt-"),
        CliTool::Gemini => model.starts_with("gemini-"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::templates::types::{RoleTemplate, SlotOverrides, TeamPreset};

    fn templates_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("templates")
    }

    fn load_roles() -> Vec<RoleTemplate> {
        let mut files = fs::read_dir(templates_dir().join("roles"))
            .expect("read roles dir")
            .map(|entry| entry.expect("entry").path())
            .collect::<Vec<_>>();
        files.sort();

        files
            .into_iter()
            .map(|path| {
                let raw = fs::read_to_string(&path).expect("read role template");
                serde_norway::from_str::<RoleTemplate>(&raw).expect("parse role template")
            })
            .collect()
    }

    fn load_preset(id: &str) -> TeamPreset {
        let path = templates_dir().join("presets").join(format!("{id}.yaml"));
        let raw = fs::read_to_string(path).expect("read preset template");
        serde_norway::from_str::<TeamPreset>(&raw).expect("parse preset template")
    }

    #[test]
    fn compose_valid_dev_team() {
        let roles = load_roles();
        let preset = load_preset("dev-team");

        let result = compose_team(
            &preset.lead_role_id,
            &preset.agent_slots,
            &roles,
            &CompositionOverrides {
                project_name: Some("taurhaus".to_string()),
                ..CompositionOverrides::default()
            },
        );

        assert!(
            result.is_valid(),
            "expected valid composition, got errors: {:?}",
            result.validation_errors
        );
        assert_eq!(result.roster.len(), 3);
        assert!(result
            .roster
            .iter()
            .any(|member| member.name == "lead-taurhaus"));
        assert!(result.roster.iter().any(|member| member.name == "dev-1"));
        assert!(result.roster.iter().any(|member| member.name == "dev-2"));
    }

    #[test]
    fn compose_valid_pair_team() {
        let roles = load_roles();
        let preset = load_preset("pair");

        let result = compose_team(
            &preset.lead_role_id,
            &preset.agent_slots,
            &roles,
            &CompositionOverrides {
                project_name: Some("taurhaus".to_string()),
                ..CompositionOverrides::default()
            },
        );

        assert!(
            result.is_valid(),
            "expected valid composition, got errors: {:?}",
            result.validation_errors
        );
        assert_eq!(result.roster.len(), 2);
        assert!(result
            .roster
            .iter()
            .any(|member| member.role_id == "v3-lead-claude"));
        assert!(result
            .roster
            .iter()
            .any(|member| member.name == "lead-taurhaus"));
        assert!(result.roster.iter().any(|member| member.name == "quick-dev"));
    }

    #[test]
    fn compose_valid_full_team() {
        let roles = load_roles();
        let preset = load_preset("full-team");

        let result = compose_team(
            &preset.lead_role_id,
            &preset.agent_slots,
            &roles,
            &CompositionOverrides {
                project_name: Some("taurhaus".to_string()),
                ..CompositionOverrides::default()
            },
        );

        assert!(
            result.is_valid(),
            "expected valid composition, got errors: {:?}",
            result.validation_errors
        );
        assert!(result
            .roster
            .iter()
            .any(|member| member.role_id == "v3-lead-claude"));
        assert!(result
            .roster
            .iter()
            .any(|member| member.name == "architect"));
        assert!(result.roster.iter().any(|member| member.name == "dev-1"));
        assert!(result.roster.iter().any(|member| member.name == "dev-2"));
    }

    #[test]
    fn compose_enforces_single_lead_role() {
        let roles = load_roles();
        let slots = vec![AgentSlot {
            role_id: "codex-developer".to_string(),
            count: 1,
            project_binding: ProjectBinding::LeadProject,
            project_id: None,
            overrides: None,
        }];

        let result = compose_team(
            "codex-developer",
            &slots,
            &roles,
            &CompositionOverrides::default(),
        );

        assert!(!result.is_valid());
        assert!(result
            .validation_errors
            .iter()
            .any(|entry| entry.contains("must reference a lead role")));
    }

    #[test]
    fn compose_resolves_name_collisions_with_suffixes() {
        let roles = load_roles();
        let slots = vec![
            AgentSlot {
                role_id: "codex-developer".to_string(),
                count: 1,
                project_binding: ProjectBinding::LeadProject,
                project_id: None,
                overrides: Some(SlotOverrides {
                    model: None,
                    name_pattern: Some("worker".to_string()),
                    instructions_replace: None,
                    instructions_append: None,
                    focus_area: None,
                    context_summary: None,
                    behavior_summary: None,
                    runtime_compact_summary: None,
                    behavioral_contract_append: None,
                }),
            },
            AgentSlot {
                role_id: "codex-qa".to_string(),
                count: 1,
                project_binding: ProjectBinding::LeadProject,
                project_id: None,
                overrides: Some(SlotOverrides {
                    model: None,
                    name_pattern: Some("worker".to_string()),
                    instructions_replace: None,
                    instructions_append: None,
                    focus_area: None,
                    context_summary: None,
                    behavior_summary: None,
                    runtime_compact_summary: None,
                    behavioral_contract_append: None,
                }),
            },
        ];

        let result = compose_team(
            "claude-orchestrator",
            &slots,
            &roles,
            &CompositionOverrides {
                project_name: Some("taurhaus".to_string()),
                ..CompositionOverrides::default()
            },
        );

        assert!(result.is_valid());
        assert!(result.roster.iter().any(|member| member.name == "worker"));
        assert!(result.roster.iter().any(|member| member.name == "worker-1"));
        assert!(
            result
                .warnings
                .iter()
                .any(|entry| entry.contains("resolved name collision")),
            "expected collision warning, got: {:?}",
            result.warnings
        );
    }

    #[test]
    fn compose_applies_override_precedence() {
        let roles = load_roles();
        let slots = vec![AgentSlot {
            role_id: "codex-developer".to_string(),
            count: 2,
            project_binding: ProjectBinding::LeadProject,
            project_id: None,
            overrides: Some(SlotOverrides {
                model: Some("gpt-5-mini".to_string()),
                name_pattern: None,
                instructions_replace: None,
                instructions_append: Some("slot append".to_string()),
                focus_area: Some("Slot focus".to_string()),
                context_summary: Some("Slot context".to_string()),
                behavior_summary: Some("Slot behavior".to_string()),
                runtime_compact_summary: None,
                behavioral_contract_append: None,
            }),
        }];

        let result = compose_team(
            "claude-orchestrator",
            &slots,
            &roles,
            &CompositionOverrides {
                instances: vec![InstanceOverride {
                    slot_index: 0,
                    member_index: 1,
                    overrides: SlotOverrides {
                        model: Some("gpt-5.4 high".to_string()),
                        name_pattern: None,
                        instructions_replace: Some("instance replace".to_string()),
                        instructions_append: Some("instance append".to_string()),
                        focus_area: Some("Instance focus".to_string()),
                        context_summary: Some("Instance context".to_string()),
                        behavior_summary: Some("Instance behavior".to_string()),
                        runtime_compact_summary: None,
                        behavioral_contract_append: None,
                    },
                }],
                project_name: Some("taurhaus".to_string()),
                ..CompositionOverrides::default()
            },
        );

        assert!(result.is_valid(), "errors: {:?}", result.validation_errors);

        let dev1 = result
            .roster
            .iter()
            .find(|member| member.name == "dev-1")
            .expect("dev-1 exists");
        let dev2 = result
            .roster
            .iter()
            .find(|member| member.name == "dev-2")
            .expect("dev-2 exists");

        assert_eq!(dev1.model, "gpt-5.4 high");
        assert_eq!(dev2.model, "gpt-5-mini");

        assert_eq!(dev1.instructions, "instance replace\ninstance append");
        assert!(dev2.instructions.contains("slot append"));
        assert_eq!(dev1.focus_area.as_deref(), Some("Instance focus"));
        assert_eq!(dev1.context_summary.as_deref(), Some("Instance context"));
        assert_eq!(dev1.behavior_summary.as_deref(), Some("Instance behavior"));
        assert_eq!(dev2.focus_area.as_deref(), Some("Slot focus"));
        assert_eq!(dev2.context_summary.as_deref(), Some("Slot context"));
        assert_eq!(dev2.behavior_summary.as_deref(), Some("Slot behavior"));
    }

    #[test]
    fn compose_reports_role_constraint_violations() {
        let roles = load_roles();
        let slots = vec![AgentSlot {
            role_id: "codex-developer".to_string(),
            count: 9,
            project_binding: ProjectBinding::LeadProject,
            project_id: None,
            overrides: None,
        }];

        let result = compose_team(
            "claude-orchestrator",
            &slots,
            &roles,
            &CompositionOverrides::default(),
        );

        assert!(!result.is_valid());
        assert!(
            result
                .validation_errors
                .iter()
                .any(|entry| entry.contains("exceeds max_instances")),
            "expected constraint violation, got: {:?}",
            result.validation_errors
        );
    }
}
