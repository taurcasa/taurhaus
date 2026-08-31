use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch::ModelSpec;
use crate::templates::adapters::RoleProvenance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    RoleTemplate,
    TeamPreset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSchema {
    pub kind: TemplateKind,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleKind {
    Lead,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectBinding {
    LeadProject,
    ExplicitProject,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDefaults {
    #[serde(alias = "cli_tool")]
    pub cli_tool: CliTool,
    pub model: String,
    #[serde(default, rename = "reasoning_effort", alias = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    #[serde(alias = "default_name_pattern")]
    pub default_name_pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BehavioralContract {
    #[serde(default)]
    pub communication: Vec<String>,
    #[serde(default)]
    pub execution: Vec<String>,
    #[serde(default)]
    pub escalation: Vec<String>,
}

impl BehavioralContract {
    fn is_empty(&self) -> bool {
        self.communication.is_empty() && self.execution.is_empty() && self.escalation.is_empty()
    }

    fn validate(&self, field_prefix: &str, errors: &mut Vec<String>) {
        validate_string_list(
            &format!("{field_prefix}.communication"),
            &self.communication,
            errors,
        );
        validate_string_list(
            &format!("{field_prefix}.execution"),
            &self.execution,
            errors,
        );
        validate_string_list(
            &format!("{field_prefix}.escalation"),
            &self.escalation,
            errors,
        );
    }
}

const RUNTIME_COMPACT_SUMMARY_MIN_WORDS: usize = 20;
const RUNTIME_COMPACT_SUMMARY_MAX_WORDS: usize = 2048;
const RUNTIME_COMPACT_ROLE_PURPOSE_MAX_WORDS: usize = 128;
const RUNTIME_COMPACT_MAX_BULLETS: usize = 64;
const RUNTIME_COMPACT_MAX_ITEM_WORDS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCompactSummary {
    #[serde(alias = "role_purpose")]
    pub role_purpose: String,
    #[serde(default, alias = "keep_doing")]
    pub keep_doing: Vec<String>,
    #[serde(default, alias = "workflow_sequence")]
    pub workflow_sequence: Vec<String>,
    #[serde(default)]
    pub avoid: Vec<String>,
    #[serde(default, alias = "escalate_when")]
    pub escalate_when: Vec<String>,
}

impl RuntimeCompactSummary {
    fn validate(&self, field_prefix: &str, errors: &mut Vec<String>) {
        validate_non_empty(
            &format!("{field_prefix}.role_purpose"),
            &self.role_purpose,
            errors,
        );
        validate_string_list(
            &format!("{field_prefix}.keep_doing"),
            &self.keep_doing,
            errors,
        );
        validate_string_list(
            &format!("{field_prefix}.workflow_sequence"),
            &self.workflow_sequence,
            errors,
        );
        validate_string_list(&format!("{field_prefix}.avoid"), &self.avoid, errors);
        validate_string_list(
            &format!("{field_prefix}.escalate_when"),
            &self.escalate_when,
            errors,
        );

        validate_summary_list_limit(
            &format!("{field_prefix}.keep_doing"),
            &self.keep_doing,
            errors,
        );
        validate_summary_list_limit(
            &format!("{field_prefix}.workflow_sequence"),
            &self.workflow_sequence,
            errors,
        );
        validate_summary_list_limit(&format!("{field_prefix}.avoid"), &self.avoid, errors);
        validate_summary_list_limit(
            &format!("{field_prefix}.escalate_when"),
            &self.escalate_when,
            errors,
        );

        let role_purpose_words = count_words(&self.role_purpose);
        if role_purpose_words > RUNTIME_COMPACT_ROLE_PURPOSE_MAX_WORDS {
            errors.push(format!(
                "{field_prefix}.role_purpose must be <= {RUNTIME_COMPACT_ROLE_PURPOSE_MAX_WORDS} words, found {role_purpose_words}"
            ));
        }

        let total_words = self.total_word_count();
        if !(RUNTIME_COMPACT_SUMMARY_MIN_WORDS..=RUNTIME_COMPACT_SUMMARY_MAX_WORDS)
            .contains(&total_words)
        {
            errors.push(format!(
                "{field_prefix} must be between {RUNTIME_COMPACT_SUMMARY_MIN_WORDS} and {RUNTIME_COMPACT_SUMMARY_MAX_WORDS} words, found {total_words}"
            ));
        }
    }

    pub fn total_word_count(&self) -> usize {
        count_words(&self.role_purpose)
            + self
                .keep_doing
                .iter()
                .map(|item| count_words(item))
                .sum::<usize>()
            + self
                .workflow_sequence
                .iter()
                .map(|item| count_words(item))
                .sum::<usize>()
            + self
                .avoid
                .iter()
                .map(|item| count_words(item))
                .sum::<usize>()
            + self
                .escalate_when
                .iter()
                .map(|item| count_words(item))
                .sum::<usize>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleConstraints {
    #[serde(default, alias = "min_instances")]
    pub min_instances: u32,
    #[serde(default = "default_max_instances", alias = "max_instances")]
    pub max_instances: u32,
    #[serde(default, alias = "requires_lead_tool")]
    pub requires_lead_tool: Option<CliTool>,
    #[serde(
        default = "default_allowed_project_binding",
        alias = "allowed_project_binding"
    )]
    pub allowed_project_binding: ProjectBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleTemplate {
    pub schema: TemplateSchema,
    #[serde(alias = "role_id")]
    pub role_id: String,
    pub name: String,
    pub version: String,
    pub kind: RoleKind,
    pub defaults: RoleDefaults,
    pub instructions: String,
    #[serde(default, alias = "focus_area")]
    pub focus_area: Option<String>,
    #[serde(default, alias = "context_summary")]
    pub context_summary: Option<String>,
    #[serde(default, alias = "behavior_summary")]
    pub behavior_summary: Option<String>,
    #[serde(
        default,
        alias = "communication_style",
        skip_serializing_if = "Option::is_none"
    )]
    pub communication_style: Option<String>,
    #[serde(
        default,
        alias = "runtime_compact_summary",
        skip_serializing_if = "Option::is_none"
    )]
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    #[serde(alias = "behavioral_contract")]
    pub behavioral_contract: BehavioralContract,
    #[serde(
        default,
        alias = "quality_gates",
        skip_serializing_if = "Option::is_none"
    )]
    pub quality_gates: Option<Vec<String>>,
    #[serde(
        default,
        alias = "handoff_expectations",
        skip_serializing_if = "Option::is_none"
    )]
    pub handoff_expectations: Option<Vec<String>>,
    #[serde(
        default,
        alias = "definition_of_done",
        skip_serializing_if = "Option::is_none"
    )]
    pub definition_of_done: Option<Vec<String>>,
    #[serde(
        default,
        alias = "phase_scope",
        skip_serializing_if = "Option::is_none"
    )]
    pub phase_scope: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(
        default,
        alias = "inherits_from",
        skip_serializing_if = "Option::is_none"
    )]
    pub inherits_from: Option<String>,
    #[serde(
        default,
        alias = "required_artifacts",
        skip_serializing_if = "Option::is_none"
    )]
    pub required_artifacts: Option<Vec<String>>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<RoleProvenance>,
    pub constraints: RoleConstraints,
}

impl RoleTemplate {
    pub fn normalize_model_fields(&mut self) {
        let parsed = ModelSpec::parse_legacy(&self.defaults.model);
        if let Some(model) = parsed.model {
            self.defaults.model = model;
        }
        if self.defaults.reasoning_effort.is_none() {
            self.defaults.reasoning_effort = parsed.reasoning_effort;
        }
    }

    pub fn validate(&self) -> Result<(), TemplateValidationError> {
        let mut normalized = self.clone();
        normalized.normalize_model_fields();
        normalized.validate_normalized()
    }

    fn validate_normalized(&self) -> Result<(), TemplateValidationError> {
        let mut errors = Vec::new();

        if self.schema.kind != TemplateKind::RoleTemplate {
            errors.push(format!(
                "role '{}' has schema.kind {:?}, expected role_template",
                self.role_id, self.schema.kind
            ));
        }
        if self.schema.version == 0 {
            errors.push(format!(
                "role '{}' has invalid schema.version 0",
                self.role_id
            ));
        }

        validate_non_empty("role_id", &self.role_id, &mut errors);
        validate_non_empty("name", &self.name, &mut errors);
        validate_non_empty("version", &self.version, &mut errors);
        validate_non_empty("defaults.model", &self.defaults.model, &mut errors);
        validate_non_empty(
            "defaults.default_name_pattern",
            &self.defaults.default_name_pattern,
            &mut errors,
        );
        validate_non_empty("instructions", &self.instructions, &mut errors);
        if let Some(focus_area) = self.focus_area.as_deref() {
            validate_non_empty("focus_area", focus_area, &mut errors);
        }
        if let Some(context_summary) = self.context_summary.as_deref() {
            validate_non_empty("context_summary", context_summary, &mut errors);
        }
        if let Some(behavior_summary) = self.behavior_summary.as_deref() {
            validate_non_empty("behavior_summary", behavior_summary, &mut errors);
        }
        if let Some(communication_style) = self.communication_style.as_deref() {
            validate_non_empty("communication_style", communication_style, &mut errors);
        }
        if let Some(runtime_compact_summary) = self.runtime_compact_summary.as_ref() {
            runtime_compact_summary.validate("runtime_compact_summary", &mut errors);
        }

        if self.behavioral_contract.is_empty() {
            errors.push(format!(
                "role '{}' behavioral_contract must include at least one bullet",
                self.role_id
            ));
        }
        self.behavioral_contract
            .validate("behavioral_contract", &mut errors);

        if let Some(quality_gates) = self.quality_gates.as_ref() {
            validate_string_list("quality_gates", quality_gates, &mut errors);
        }
        if let Some(handoff_expectations) = self.handoff_expectations.as_ref() {
            validate_string_list("handoff_expectations", handoff_expectations, &mut errors);
        }
        if let Some(definition_of_done) = self.definition_of_done.as_ref() {
            validate_string_list("definition_of_done", definition_of_done, &mut errors);
        }
        if let Some(phase_scope) = self.phase_scope.as_ref() {
            validate_string_list("phase_scope", phase_scope, &mut errors);
        }
        if let Some(mode) = self.mode.as_deref() {
            validate_non_empty("mode", mode, &mut errors);
        }
        if let Some(inherits_from) = self.inherits_from.as_deref() {
            validate_non_empty("inherits_from", inherits_from, &mut errors);
        }
        if let Some(required_artifacts) = self.required_artifacts.as_ref() {
            validate_string_list("required_artifacts", required_artifacts, &mut errors);
        }

        validate_string_list("capabilities", &self.capabilities, &mut errors);
        if let Some(provenance) = &self.provenance {
            validate_string_list(
                "provenance.non_roundtrippable_fields",
                &provenance.non_roundtrippable_fields,
                &mut errors,
            );
        }

        if self.constraints.max_instances < self.constraints.min_instances {
            errors.push(format!(
                "role '{}' constraints.max_instances ({}) must be >= min_instances ({})",
                self.role_id, self.constraints.max_instances, self.constraints.min_instances
            ));
        }
        if self.constraints.max_instances == 0 {
            errors.push(format!(
                "role '{}' constraints.max_instances must be > 0",
                self.role_id
            ));
        }

        match self.kind {
            RoleKind::Lead => {
                if self.constraints.min_instances < 1 {
                    errors.push(format!(
                        "lead role '{}' constraints.min_instances must be >= 1",
                        self.role_id
                    ));
                }
                if self.constraints.max_instances != 1 {
                    errors.push(format!(
                        "lead role '{}' constraints.max_instances must be 1",
                        self.role_id
                    ));
                }
                if self.constraints.requires_lead_tool.is_some() {
                    errors.push(format!(
                        "lead role '{}' must not set constraints.requires_lead_tool",
                        self.role_id
                    ));
                }
            }
            RoleKind::Agent => {}
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(TemplateValidationError { errors })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotOverrides {
    pub model: Option<String>,
    #[serde(default, rename = "reasoning_effort", alias = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    #[serde(alias = "name_pattern")]
    pub name_pattern: Option<String>,
    #[serde(alias = "instructions_replace")]
    pub instructions_replace: Option<String>,
    #[serde(alias = "instructions_append")]
    pub instructions_append: Option<String>,
    #[serde(default, alias = "focus_area")]
    pub focus_area: Option<String>,
    #[serde(default, alias = "context_summary")]
    pub context_summary: Option<String>,
    #[serde(default, alias = "behavior_summary")]
    pub behavior_summary: Option<String>,
    #[serde(default, alias = "runtime_compact_summary")]
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    #[serde(alias = "behavioral_contract_append")]
    pub behavioral_contract_append: Option<BehavioralContract>,
}

impl SlotOverrides {
    pub fn normalize_model_fields(&mut self) {
        let Some(model) = self.model.as_deref() else {
            return;
        };

        let parsed = ModelSpec::parse_legacy(model);
        if parsed.model.is_some() {
            self.model = parsed.model;
        }
        if self.reasoning_effort.is_none() {
            self.reasoning_effort = parsed.reasoning_effort;
        }
    }

    fn validate(&self, field_prefix: &str, errors: &mut Vec<String>) {
        if let Some(model) = self.model.as_deref() {
            validate_non_empty(&format!("{field_prefix}.model"), model, errors);
        }
        if let Some(name_pattern) = self.name_pattern.as_deref() {
            validate_non_empty(
                &format!("{field_prefix}.name_pattern"),
                name_pattern,
                errors,
            );
        }
        if let Some(instructions_replace) = self.instructions_replace.as_deref() {
            validate_non_empty(
                &format!("{field_prefix}.instructions_replace"),
                instructions_replace,
                errors,
            );
        }
        if let Some(instructions_append) = self.instructions_append.as_deref() {
            validate_non_empty(
                &format!("{field_prefix}.instructions_append"),
                instructions_append,
                errors,
            );
        }
        if let Some(focus_area) = self.focus_area.as_deref() {
            validate_non_empty(&format!("{field_prefix}.focus_area"), focus_area, errors);
        }
        if let Some(context_summary) = self.context_summary.as_deref() {
            validate_non_empty(
                &format!("{field_prefix}.context_summary"),
                context_summary,
                errors,
            );
        }
        if let Some(behavior_summary) = self.behavior_summary.as_deref() {
            validate_non_empty(
                &format!("{field_prefix}.behavior_summary"),
                behavior_summary,
                errors,
            );
        }
        if let Some(runtime_compact_summary) = self.runtime_compact_summary.as_ref() {
            runtime_compact_summary
                .validate(&format!("{field_prefix}.runtime_compact_summary"), errors);
        }
        if let Some(contract) = self.behavioral_contract_append.as_ref() {
            contract.validate(
                &format!("{field_prefix}.behavioral_contract_append"),
                errors,
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSlot {
    #[serde(alias = "role_id")]
    pub role_id: String,
    pub count: u32,
    #[serde(alias = "project_binding")]
    pub project_binding: ProjectBinding,
    #[serde(alias = "project_id")]
    pub project_id: Option<String>,
    pub overrides: Option<SlotOverrides>,
}

impl AgentSlot {
    fn validate(&self, slot_index: usize, errors: &mut Vec<String>) {
        let prefix = format!("agent_slots[{slot_index}]");
        validate_agent_slot_common(self, slot_index, errors);

        if let Some(overrides) = self.overrides.as_ref() {
            overrides.validate(&format!("{prefix}.overrides"), errors);
        }
    }
}

pub(crate) fn validate_agent_slot_common(
    slot: &AgentSlot,
    slot_index: usize,
    errors: &mut Vec<String>,
) {
    let prefix = format!("agent_slots[{slot_index}]");
    validate_non_empty(&format!("{prefix}.role_id"), &slot.role_id, errors);

    if slot.count == 0 {
        errors.push(format!("{prefix}.count must be >= 1"));
    }

    match slot.project_binding {
        ProjectBinding::ExplicitProject => {
            if slot
                .project_id
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                errors.push(format!(
                    "{prefix}.project_id is required when project_binding is explicit_project"
                ));
            }
        }
        ProjectBinding::LeadProject | ProjectBinding::Any => {
            if let Some(project_id) = slot.project_id.as_deref() {
                if !project_id.trim().is_empty() {
                    errors.push(format!(
                        "{prefix}.project_id must be omitted unless project_binding is explicit_project"
                    ));
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPresetDefaults {
    #[serde(alias = "team_name_pattern")]
    pub team_name_pattern: String,
    #[serde(alias = "tmux_layout")]
    pub tmux_layout: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPreset {
    pub schema: TemplateSchema,
    #[serde(alias = "preset_id")]
    pub preset_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(alias = "lead_role_id")]
    pub lead_role_id: String,
    /// What the preset pins for its lead on top of the lead role's own defaults.
    /// The advanced preset editor edits the lead's model/effort, and composition
    /// applies this as `CompositionOverrides::lead`.
    #[serde(default, alias = "lead_overrides")]
    pub lead_overrides: Option<SlotOverrides>,
    #[serde(default, alias = "agent_slots")]
    pub agent_slots: Vec<AgentSlot>,
    pub defaults: TeamPresetDefaults,
}

impl TeamPreset {
    pub fn normalize_model_fields(&mut self) {
        if let Some(overrides) = &mut self.lead_overrides {
            overrides.normalize_model_fields();
        }
        for slot in &mut self.agent_slots {
            if let Some(overrides) = &mut slot.overrides {
                overrides.normalize_model_fields();
            }
        }
    }

    pub fn validate(&self) -> Result<(), TemplateValidationError> {
        let mut normalized = self.clone();
        normalized.normalize_model_fields();
        normalized.validate_normalized()
    }

    fn validate_normalized(&self) -> Result<(), TemplateValidationError> {
        let mut errors = Vec::new();

        if self.schema.kind != TemplateKind::TeamPreset {
            errors.push(format!(
                "preset '{}' has schema.kind {:?}, expected team_preset",
                self.preset_id, self.schema.kind
            ));
        }
        if self.schema.version == 0 {
            errors.push(format!(
                "preset '{}' has invalid schema.version 0",
                self.preset_id
            ));
        }

        validate_non_empty("preset_id", &self.preset_id, &mut errors);
        validate_non_empty("name", &self.name, &mut errors);
        validate_non_empty("description", &self.description, &mut errors);
        validate_non_empty("version", &self.version, &mut errors);
        validate_non_empty("lead_role_id", &self.lead_role_id, &mut errors);
        validate_non_empty(
            "defaults.team_name_pattern",
            &self.defaults.team_name_pattern,
            &mut errors,
        );
        validate_non_empty(
            "defaults.tmux_layout",
            &self.defaults.tmux_layout,
            &mut errors,
        );

        if self.agent_slots.is_empty() {
            errors.push(format!(
                "preset '{}' must include at least one agent slot",
                self.preset_id
            ));
        }
        if let Some(overrides) = self.lead_overrides.as_ref() {
            overrides.validate("lead_overrides", &mut errors);
        }
        for (idx, slot) in self.agent_slots.iter().enumerate() {
            slot.validate(idx, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(TemplateValidationError { errors })
        }
    }

    pub fn validate_with_role_catalog(
        &self,
        role_catalog: &[RoleTemplate],
    ) -> Result<(), TemplateValidationError> {
        let mut errors = Vec::new();

        if let Err(err) = self.validate() {
            errors.extend(err.errors);
        }

        let mut role_map = HashMap::new();
        for role in role_catalog {
            if let Err(err) = role.validate() {
                errors.extend(err.errors);
            }
            if role_map.insert(role.role_id.as_str(), role).is_some() {
                errors.push(format!(
                    "duplicate role_id '{}' in role catalog",
                    role.role_id
                ));
            }
        }

        let lead = match role_map.get(self.lead_role_id.as_str()) {
            Some(role) => {
                if role.kind != RoleKind::Lead {
                    errors.push(format!(
                        "preset '{}' lead_role_id '{}' must reference a lead role",
                        self.preset_id, self.lead_role_id
                    ));
                }
                Some(*role)
            }
            None => {
                errors.push(format!(
                    "preset '{}' references unknown lead_role_id '{}'",
                    self.preset_id, self.lead_role_id
                ));
                None
            }
        };

        let mut role_instance_counts: HashMap<&str, u32> = HashMap::new();
        for slot in &self.agent_slots {
            let Some(role) = role_map.get(slot.role_id.as_str()) else {
                errors.push(format!(
                    "preset '{}' references unknown agent role_id '{}'",
                    self.preset_id, slot.role_id
                ));
                continue;
            };

            if role.kind != RoleKind::Agent {
                errors.push(format!(
                    "preset '{}' role_id '{}' must reference an agent role",
                    self.preset_id, slot.role_id
                ));
            }

            let entry = role_instance_counts
                .entry(role.role_id.as_str())
                .or_insert(0);
            *entry += slot.count;

            if let Some(required_lead_tool) = role.constraints.requires_lead_tool {
                if let Some(lead_role) = lead {
                    if required_lead_tool != lead_role.defaults.cli_tool {
                        errors.push(format!(
                            "preset '{}' role '{}' requires lead tool '{}', found '{}'",
                            self.preset_id,
                            role.role_id,
                            required_lead_tool,
                            lead_role.defaults.cli_tool
                        ));
                    }
                }
            }

            match role.constraints.allowed_project_binding {
                ProjectBinding::Any => {}
                ProjectBinding::LeadProject => {
                    if slot.project_binding != ProjectBinding::LeadProject {
                        errors.push(format!(
                            "preset '{}' role '{}' requires project_binding 'lead_project'",
                            self.preset_id, role.role_id
                        ));
                    }
                }
                ProjectBinding::ExplicitProject => {
                    if slot.project_binding != ProjectBinding::ExplicitProject {
                        errors.push(format!(
                            "preset '{}' role '{}' requires project_binding 'explicit_project'",
                            self.preset_id, role.role_id
                        ));
                    }
                }
            }
        }

        for (role_id, count) in role_instance_counts {
            if let Some(role) = role_map.get(role_id) {
                if count < role.constraints.min_instances {
                    errors.push(format!(
                        "preset '{}' role '{}' count {} is below min_instances {}",
                        self.preset_id, role_id, count, role.constraints.min_instances
                    ));
                }
                if count > role.constraints.max_instances {
                    errors.push(format!(
                        "preset '{}' role '{}' count {} exceeds max_instances {}",
                        self.preset_id, role_id, count, role.constraints.max_instances
                    ));
                }
            }
        }

        if let Err(err) = self.resolve_member_names(role_catalog, "project") {
            errors.extend(err.errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(TemplateValidationError { errors })
        }
    }

    pub fn resolve_member_names(
        &self,
        role_catalog: &[RoleTemplate],
        project_name: &str,
    ) -> Result<Vec<String>, TemplateValidationError> {
        let mut errors = Vec::new();
        let mut role_map = HashMap::new();
        for role in role_catalog {
            role_map.insert(role.role_id.as_str(), role);
        }

        let mut names = Vec::new();
        let mut seen: HashMap<String, u32> = HashMap::new();

        let Some(lead) = role_map.get(self.lead_role_id.as_str()) else {
            return Err(TemplateValidationError {
                errors: vec![format!(
                    "cannot resolve member names: unknown lead_role_id '{}'",
                    self.lead_role_id
                )],
            });
        };

        let lead_base = apply_name_pattern(&lead.defaults.default_name_pattern, 1, project_name);
        if lead_base.trim().is_empty() {
            errors.push(format!(
                "lead role '{}' produced empty name from pattern '{}'",
                lead.role_id, lead.defaults.default_name_pattern
            ));
        } else {
            names.push(make_unique_name(lead_base, &mut seen));
        }

        for slot in &self.agent_slots {
            let Some(role) = role_map.get(slot.role_id.as_str()) else {
                errors.push(format!(
                    "cannot resolve names for unknown role_id '{}'",
                    slot.role_id
                ));
                continue;
            };

            let pattern = slot
                .overrides
                .as_ref()
                .and_then(|overrides| overrides.name_pattern.as_deref())
                .unwrap_or(&role.defaults.default_name_pattern);

            for i in 1..=slot.count {
                let base = apply_name_pattern(pattern, i, project_name);
                if base.trim().is_empty() {
                    errors.push(format!(
                        "role '{}' produced empty name from pattern '{}'",
                        role.role_id, pattern
                    ));
                    continue;
                }
                names.push(make_unique_name(base, &mut seen));
            }
        }

        if errors.is_empty() {
            Ok(names)
        } else {
            Err(TemplateValidationError { errors })
        }
    }
}

fn make_unique_name(base: String, seen: &mut HashMap<String, u32>) -> String {
    let counter = seen.entry(base.clone()).or_insert(0);
    *counter += 1;
    if *counter == 1 {
        base
    } else {
        format!("{base}-{counter}")
    }
}

fn apply_name_pattern(pattern: &str, n: u32, project_name: &str) -> String {
    pattern
        .replace("{n}", &n.to_string())
        .replace("{project}", project_name)
}

fn validate_non_empty(field: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{field} must not be empty"));
    }
}

fn validate_summary_list_limit(field: &str, items: &[String], errors: &mut Vec<String>) {
    if items.len() > RUNTIME_COMPACT_MAX_BULLETS {
        errors.push(format!(
            "{field} must have at most {RUNTIME_COMPACT_MAX_BULLETS} bullets, found {}",
            items.len()
        ));
    }

    for (index, item) in items.iter().enumerate() {
        let words = count_words(item);
        if words > RUNTIME_COMPACT_MAX_ITEM_WORDS {
            errors.push(format!(
                "{field}[{index}] must be <= {RUNTIME_COMPACT_MAX_ITEM_WORDS} words, found {words}"
            ));
        }
    }
}

fn count_words(value: &str) -> usize {
    value.split_whitespace().count()
}

fn validate_string_list(field: &str, values: &[String], errors: &mut Vec<String>) {
    let mut seen = HashSet::new();
    for value in values {
        if value.trim().is_empty() {
            errors.push(format!("{field} must not include empty values"));
            continue;
        }
        if !seen.insert(value.as_str()) {
            errors.push(format!(
                "{field} must not include duplicate value '{value}'"
            ));
        }
    }
}

fn default_max_instances() -> u32 {
    1
}

fn default_allowed_project_binding() -> ProjectBinding {
    ProjectBinding::Any
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateValidationError {
    pub errors: Vec<String>,
}

impl fmt::Display for TemplateValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} validation error(s): {}",
            self.errors.len(),
            self.errors.join("; ")
        )
    }
}

impl Error for TemplateValidationError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::models::ModelCatalog;
    use crate::session_scanner::cli_tool;

    fn sample_role_template() -> RoleTemplate {
        RoleTemplate {
            schema: TemplateSchema {
                kind: TemplateKind::RoleTemplate,
                version: 1,
            },
            role_id: "sample-role".to_string(),
            name: "Sample Role".to_string(),
            version: "1.0.0".to_string(),
            kind: RoleKind::Agent,
            defaults: RoleDefaults {
                cli_tool: CliTool::Codex,
                model: "gpt-5.4 high".to_string(),
                reasoning_effort: None,
                default_name_pattern: "dev-{n}".to_string(),
            },
            instructions: "Execute scoped tasks.".to_string(),
            focus_area: Some("Implementation lane".to_string()),
            context_summary: Some(
                "Carries active implementation context for scoped code changes.".to_string(),
            ),
            behavior_summary: Some(
                "Implements assigned work and escalates structural uncertainty.".to_string(),
            ),
            communication_style: Some("Short, concrete progress updates.".to_string()),
            runtime_compact_summary: Some(RuntimeCompactSummary {
                role_purpose: "Implement scoped changes with validation-first discipline and explicit ownership boundaries.".to_string(),
                keep_doing: vec![
                    "Stay inside the assignment contract and preserve exact task scope.".to_string(),
                    "Use the named validation lane and keep regression coverage when behavior broke.".to_string(),
                ],
                workflow_sequence: vec![
                    "Confirm the active task, owned files, and validation expectation before editing.".to_string(),
                    "Implement the smallest scoped change that fixes the real issue.".to_string(),
                    "Run the promised validation and report exact outcomes plus residual risk.".to_string(),
                ],
                avoid: vec![
                    "Do not refactor adjacent systems or redesign another role's surface opportunistically.".to_string(),
                    "Do not stop at confidence when runtime verification is part of the assignment.".to_string(),
                ],
                escalate_when: vec![
                    "Escalate overlap, architecture drift, or blockers beyond the narrow override rule immediately.".to_string(),
                ],
            }),
            behavioral_contract: BehavioralContract {
                communication: vec!["post updates".to_string()],
                execution: vec!["deliver tests".to_string()],
                escalation: vec!["raise blockers".to_string()],
            },
            quality_gates: Some(vec![
                "Run the named validation lane.".to_string(),
                "Keep regression coverage for confirmed bugs.".to_string(),
            ]),
            handoff_expectations: None,
            definition_of_done: Some(vec![
                "Requested behavior matches the acceptance criteria.".to_string(),
                "Residual risks are called out explicitly.".to_string(),
            ]),
            phase_scope: Some(vec!["implementation".to_string(), "verification".to_string()]),
            mode: Some("execution".to_string()),
            inherits_from: Some("taurhaus-base-implementer".to_string()),
            required_artifacts: Some(vec![
                "code diff".to_string(),
                "verification summary".to_string(),
            ]),
            capabilities: vec!["implementation".to_string()],
            provenance: None,
            constraints: RoleConstraints {
                min_instances: 1,
                max_instances: 3,
                requires_lead_tool: Some(CliTool::Claude),
                allowed_project_binding: ProjectBinding::Any,
            },
        }
    }

    fn sample_team_preset() -> TeamPreset {
        TeamPreset {
            schema: TemplateSchema {
                kind: TemplateKind::TeamPreset,
                version: 1,
            },
            preset_id: "sample-preset".to_string(),
            name: "Sample Preset".to_string(),
            description: "Sample preset description".to_string(),
            version: "1.0.0".to_string(),
            lead_role_id: "lead-role".to_string(),
            lead_overrides: None,
            agent_slots: vec![AgentSlot {
                role_id: "sample-role".to_string(),
                count: 2,
                project_binding: ProjectBinding::ExplicitProject,
                project_id: Some("project-a".to_string()),
                overrides: Some(SlotOverrides {
                    model: Some("gpt-5.4 high".to_string()),
                    reasoning_effort: None,
                    name_pattern: Some("dev-{n}".to_string()),
                    instructions_replace: Some("Replace instructions".to_string()),
                    instructions_append: Some("Append instructions".to_string()),
                    focus_area: Some("Custom implementation lane".to_string()),
                    context_summary: Some("Custom context summary".to_string()),
                    behavior_summary: Some("Custom behavior summary".to_string()),
                    runtime_compact_summary: None,
                    behavioral_contract_append: Some(BehavioralContract {
                        communication: vec!["sync daily".to_string()],
                        execution: vec!["ship incrementally".to_string()],
                        escalation: vec!["escalate quickly".to_string()],
                    }),
                }),
            }],
            defaults: TeamPresetDefaults {
                team_name_pattern: "{project}-team".to_string(),
                tmux_layout: "tiled".to_string(),
            },
        }
    }

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn templates_dir() -> PathBuf {
        manifest_dir().join("resources").join("templates")
    }

    fn load_role_templates() -> Vec<RoleTemplate> {
        let roles_dir = templates_dir().join("roles");
        let mut paths = fs::read_dir(&roles_dir)
            .expect("read roles template dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        paths.sort();

        paths
            .into_iter()
            .map(|path| {
                let raw = fs::read_to_string(&path).expect("read role template");
                serde_norway::from_str::<RoleTemplate>(&raw)
                    .unwrap_or_else(|err| panic!("parse role template {}: {err}", path.display()))
            })
            .collect()
    }

    // Regression: a79d392 skipped legacy slug normalization whenever an explicit effort
    // was present, leaving `gpt-5.4 high` as an invalid model id on launch.
    #[test]
    fn explicit_effort_wins_while_legacy_model_slug_is_canonicalized() {
        let mut role = sample_role_template();
        role.defaults.reasoning_effort = Some("low".to_string());
        role.normalize_model_fields();
        assert_eq!(role.defaults.model, "gpt-5.4");
        assert_eq!(role.defaults.reasoning_effort.as_deref(), Some("low"));

        let mut overrides = SlotOverrides {
            model: Some("gpt-5.4 high".to_string()),
            reasoning_effort: Some("low".to_string()),
            name_pattern: None,
            instructions_replace: None,
            instructions_append: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            runtime_compact_summary: None,
            behavioral_contract_append: None,
        };
        overrides.normalize_model_fields();
        assert_eq!(overrides.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(overrides.reasoning_effort.as_deref(), Some("low"));
    }

    fn load_team_presets() -> Vec<TeamPreset> {
        let presets_dir = templates_dir().join("presets");
        let mut paths = fs::read_dir(&presets_dir)
            .expect("read presets template dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        paths.sort();

        paths
            .into_iter()
            .map(|path| {
                let raw = fs::read_to_string(&path).expect("read team preset");
                serde_norway::from_str::<TeamPreset>(&raw)
                    .unwrap_or_else(|err| panic!("parse team preset {}: {err}", path.display()))
            })
            .collect()
    }

    #[test]
    fn role_templates_deserialize_and_validate() {
        let roles = load_role_templates();
        assert!(
            !roles.is_empty(),
            "expected at least one built-in role template"
        );
        let unique_role_ids = roles
            .iter()
            .map(|role| role.role_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            unique_role_ids.len(),
            roles.len(),
            "built-in role templates should have unique role ids"
        );
        assert!(
            roles
                .iter()
                .any(|role| role.role_id == "codex-orchestrator"),
            "expected codex-orchestrator role template in built-ins"
        );
        assert!(
            roles
                .iter()
                .any(|role| role.role_id == "antigravity-orchestrator"),
            "expected antigravity-orchestrator role template in built-ins"
        );

        for role in &roles {
            role.validate()
                .unwrap_or_else(|err| panic!("role '{}' failed validation: {err}", role.role_id));
            assert!(
                role.focus_area
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()),
                "role '{}' should define focus_area",
                role.role_id
            );
            assert!(
                role.context_summary
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()),
                "role '{}' should define context_summary",
                role.role_id
            );
            assert!(
                role.behavior_summary
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()),
                "role '{}' should define behavior_summary",
                role.role_id
            );
        }
    }

    #[test]
    fn bundled_roles_are_the_canonical_shipped_playbook() {
        let roles = load_role_templates();
        let actual = roles
            .iter()
            .map(|role| role.role_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            "adversarial-reviewer-claude",
            "antigravity-orchestrator",
            "claude-design-lead",
            "claude-product-checker",
            "claude-researcher",
            "codex-orchestrator",
            "codex-qa",
            "docs-verifier-codex",
            "frontend-design-skill-developer",
            "quick-dev-codex",
            "v3-architect-codex",
            "v3-lead-claude",
            "v4-developer-agy",
            "v4-developer-claude",
            "v4-developer-codex",
            "v4-developer-grok",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            actual, expected,
            "bundled catalog should contain one canonical role per living lane and harness"
        );

        let harness_defaults = cli_tool::all()
            .iter()
            .map(|tool| (tool.name, tool.default_agent_role_id))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            harness_defaults,
            [
                ("agy", "v4-developer-agy"),
                ("claude", "v4-developer-claude"),
                ("codex", "v4-developer-codex"),
                ("grok", "v4-developer-grok"),
            ]
            .into_iter()
            .collect(),
            "every harness should default to its canonical implementation role"
        );

        for role in &roles {
            let communication = role.communication_style.as_deref().unwrap_or_default();
            for mark in [
                "objective",
                "exact deliverable",
                "concrete first action",
                "completion signal",
                "explicit response expectation",
                "ACTION REQUIRED:",
                "INFO ONLY:",
                "no response needed",
            ] {
                assert!(
                    communication.contains(mark),
                    "role '{}' communication_style is missing '{mark}'",
                    role.role_id
                );
            }

            let gates = role.quality_gates.as_deref().unwrap_or_default().join("\n");
            assert!(
                gates.contains("`just check-quick`"),
                "role '{}' should carry the per-task gate",
                role.role_id
            );
            assert!(
                gates.contains("Never run full `just check` as an agent"),
                "role '{}' should carry the serialized full-gate boundary",
                role.role_id
            );

            let done = role
                .definition_of_done
                .as_deref()
                .unwrap_or_default()
                .join("\n");
            assert!(
                done.contains("review-ready handoff"),
                "role '{}' should require a review-ready handoff",
                role.role_id
            );

            let catalog_entry =
                ModelCatalog::entry_for(role.defaults.cli_tool, role.defaults.model.as_str())
                    .unwrap_or_else(|| {
                        panic!(
                            "role '{}' pins unknown model '{}' for {:?}",
                            role.role_id, role.defaults.model, role.defaults.cli_tool
                        )
                    });
            assert!(
                !catalog_entry.deprecated,
                "role '{}' pins retired model '{}'",
                role.role_id, role.defaults.model
            );
            assert!(
                !role.instructions.contains("Sonnet") && !role.instructions.contains("sonnet"),
                "role '{}' should not recommend Sonnet",
                role.role_id
            );

            if role.kind == RoleKind::Agent {
                let contract = format!(
                    "{}\n{}\n{}",
                    role.instructions,
                    role.behavioral_contract.communication.join("\n"),
                    done
                );
                assert!(
                    contract.contains("RESULT <id>") && contract.contains("BLOCKED <id> <reason>"),
                    "member role '{}' should carry the managed-stage completion signals",
                    role.role_id
                );
            } else {
                assert!(
                    role.instructions
                        .contains("one active assignment per member")
                        && role.instructions.contains("uptake")
                        && role.instructions.contains("deadline"),
                    "lead role '{}' should carry the bounded stage contract",
                    role.role_id
                );
            }
        }
    }

    #[test]
    fn open_and_design_slots_name_their_candidate_models() {
        let roles = load_role_templates();
        let find = |role_id: &str| {
            roles
                .iter()
                .find(|role| role.role_id == role_id)
                .unwrap_or_else(|| panic!("missing role '{role_id}'"))
        };

        let architect = find("v3-architect-codex");
        assert_eq!(architect.defaults.cli_tool, CliTool::Claude);
        assert_eq!(architect.defaults.model, "fable");
        assert!(architect
            .instructions
            .contains("Candidates: Fable 5 (preferred) or GPT-5.6 Sol (fallback)"));

        let researcher = find("claude-researcher");
        assert_eq!(researcher.defaults.cli_tool, CliTool::Codex);
        assert_eq!(researcher.defaults.model, "gpt-5.6-sol");
        assert!(researcher
            .instructions
            .contains("Candidates: GPT-5.6 Sol (preferred) or Opus 5 High"));

        let reviewer = find("adversarial-reviewer-claude");
        assert_eq!(reviewer.defaults.cli_tool, CliTool::Claude);
        assert_eq!(reviewer.defaults.model, "opus");
        assert!(reviewer.instructions.contains("Default: Opus 5"));
        assert!(reviewer.instructions.contains(
            "Candidate variant: GPT-5.6 Sol recall pass followed by Opus 5 verification"
        ));

        let creative = find("claude-design-lead");
        assert_eq!(creative.defaults.cli_tool, CliTool::Claude);
        assert_eq!(creative.defaults.model, "fable");
        assert!(creative
            .instructions
            .contains("CREATIVE DIRECTION candidates: Fable 5 (preferred) or Gemini via agy"));
        assert!(creative.instructions.contains("human validation required"));

        let implementation = find("frontend-design-skill-developer");
        assert_eq!(implementation.defaults.cli_tool, CliTool::Codex);
        assert_eq!(implementation.defaults.model, "gpt-5.6-sol");
        assert!(implementation
            .instructions
            .contains("UI IMPLEMENTATION candidates: GPT-5.6 Sol (preferred) or Opus 5"));
        assert!(implementation
            .instructions
            .contains("human validation required"));
    }

    #[test]
    fn team_presets_deserialize_and_validate_against_roles() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        assert!(
            !presets.is_empty(),
            "expected at least one built-in team preset"
        );
        let unique_preset_ids = presets
            .iter()
            .map(|preset| preset.preset_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            unique_preset_ids.len(),
            presets.len(),
            "built-in team presets should have unique preset ids"
        );
        assert_eq!(
            presets.len(),
            5,
            "expected exactly five built-in team presets"
        );
        assert!(
            presets.iter().any(|preset| preset.preset_id == "pair"),
            "expected pair preset in built-ins"
        );
        assert!(
            presets.iter().any(|preset| preset.preset_id == "dev-team"),
            "expected dev-team preset in built-ins"
        );
        assert!(
            presets.iter().any(|preset| preset.preset_id == "full-team"),
            "expected full-team preset in built-ins"
        );
        assert!(
            presets
                .iter()
                .any(|preset| preset.preset_id == "research-team"),
            "expected research-team preset in built-ins"
        );
        // Regression: commit 6be3761 surfaced grok everywhere in the UI but
        // shipped no roster that could actually staff a grok member.
        assert!(
            presets.iter().any(|preset| preset.preset_id == "grok-pair"),
            "expected grok-pair preset in built-ins"
        );

        for preset in &presets {
            preset
                .validate_with_role_catalog(&roles)
                .unwrap_or_else(|err| {
                    panic!("preset '{}' failed validation: {err}", preset.preset_id)
                });
        }
    }

    // The bundled presets staff the v4 developer roles decided in
    // `docs/design/research/phase-c-v4-results.md`; the v3 roles stay in the
    // catalog for one release but must no longer be what a preset staffs.
    #[test]
    fn built_in_presets_staff_the_v4_developer_roles() {
        let roles = load_role_templates();
        let presets = load_team_presets();

        let expected: &[(&str, &str)] = &[
            ("dev-team", "v4-developer-codex"),
            ("full-team", "v4-developer-codex"),
            ("research-team", "v4-developer-codex"),
            ("grok-pair", "v4-developer-grok"),
        ];

        for (preset_id, role_id) in expected {
            let preset = presets
                .iter()
                .find(|preset| preset.preset_id == *preset_id)
                .unwrap_or_else(|| panic!("expected '{preset_id}' preset in built-ins"));
            assert!(
                preset
                    .agent_slots
                    .iter()
                    .any(|slot| slot.role_id == *role_id),
                "preset '{preset_id}' should staff its developer slot with '{role_id}'"
            );

            let role = roles
                .iter()
                .find(|role| role.role_id == *role_id)
                .unwrap_or_else(|| panic!("expected '{role_id}' role template in built-ins"));
            assert_eq!(
                role.defaults.reasoning_effort.as_deref(),
                Some("medium"),
                "'{role_id}' should default to medium effort, the level the presets inherit"
            );
        }

        for preset in &presets {
            for slot in &preset.agent_slots {
                assert!(
                    !matches!(
                        slot.role_id.as_str(),
                        "v3-developer-claude"
                            | "v3-developer-codex"
                            | "v3-developer-agy"
                            | "grok-developer"
                    ),
                    "preset '{}' still staffs superseded developer role '{}'",
                    preset.preset_id,
                    slot.role_id
                );
                assert!(
                    slot.overrides.as_ref().is_none_or(|overrides| {
                        overrides.model.is_none() && overrides.reasoning_effort.is_none()
                    }),
                    "preset '{}' should inherit model and effort from role '{}'",
                    preset.preset_id,
                    slot.role_id
                );
            }
        }
    }

    #[test]
    fn dev_team_preset_resolves_member_names() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        let preset = presets
            .iter()
            .find(|preset| preset.preset_id == "dev-team")
            .expect("dev-team preset exists");

        let names = preset
            .resolve_member_names(&roles, "taurhaus")
            .expect("member names should resolve");

        assert!(names.iter().any(|name| name == "lead-taurhaus"));
        assert!(names.iter().any(|name| name == "dev-1"));
        assert!(names.iter().any(|name| name == "dev-2"));

        let unique: HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "resolved names should be unique");
    }

    #[test]
    fn pair_preset_resolves_member_names() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        let preset = presets
            .iter()
            .find(|preset| preset.preset_id == "pair")
            .expect("pair preset exists");

        let names = preset
            .resolve_member_names(&roles, "taurhaus")
            .expect("member names should resolve");

        assert!(names.iter().any(|name| name == "lead-taurhaus"));
        assert!(names.iter().any(|name| name == "quick-dev"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn full_team_preset_resolves_member_names() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        let preset = presets
            .iter()
            .find(|preset| preset.preset_id == "full-team")
            .expect("full-team preset exists");

        let names = preset
            .resolve_member_names(&roles, "taurhaus")
            .expect("member names should resolve");

        assert!(names.iter().any(|name| name == "lead-taurhaus"));
        assert!(names.iter().any(|name| name == "architect"));
        assert!(names.iter().any(|name| name == "dev-1"));
        assert!(names.iter().any(|name| name == "dev-2"));
    }

    #[test]
    fn research_team_preset_resolves_member_names() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        let preset = presets
            .iter()
            .find(|preset| preset.preset_id == "research-team")
            .expect("research-team preset exists");

        let names = preset
            .resolve_member_names(&roles, "taurhaus")
            .expect("member names should resolve");

        assert!(names.iter().any(|name| name == "lead-taurhaus"));
        assert!(names.iter().any(|name| name == "researcher"));
        assert!(names.iter().any(|name| name == "dev-1"));
    }

    #[test]
    fn resolve_member_names_adds_suffix_on_collision() {
        let lead_role = RoleTemplate {
            schema: TemplateSchema {
                kind: TemplateKind::RoleTemplate,
                version: 1,
            },
            role_id: "lead".to_string(),
            name: "Lead".to_string(),
            version: "1.0.0".to_string(),
            kind: RoleKind::Lead,
            defaults: RoleDefaults {
                cli_tool: CliTool::Claude,
                model: "claude-opus-4-6".to_string(),
                reasoning_effort: None,
                default_name_pattern: "lead-{project}".to_string(),
            },
            instructions: "Lead".to_string(),
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            behavioral_contract: BehavioralContract {
                communication: vec!["a".to_string()],
                execution: vec!["b".to_string()],
                escalation: vec!["c".to_string()],
            },
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: vec!["planning".to_string()],
            provenance: None,
            constraints: RoleConstraints {
                min_instances: 1,
                max_instances: 1,
                requires_lead_tool: None,
                allowed_project_binding: ProjectBinding::LeadProject,
            },
        };
        let agent_role = RoleTemplate {
            schema: TemplateSchema {
                kind: TemplateKind::RoleTemplate,
                version: 1,
            },
            role_id: "agent".to_string(),
            name: "Agent".to_string(),
            version: "1.0.0".to_string(),
            kind: RoleKind::Agent,
            defaults: RoleDefaults {
                cli_tool: CliTool::Codex,
                model: "gpt-5.4 high".to_string(),
                reasoning_effort: None,
                default_name_pattern: "worker".to_string(),
            },
            instructions: "Agent".to_string(),
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            behavioral_contract: BehavioralContract {
                communication: vec!["a".to_string()],
                execution: vec!["b".to_string()],
                escalation: vec!["c".to_string()],
            },
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: vec!["implementation".to_string()],
            provenance: None,
            constraints: RoleConstraints {
                min_instances: 0,
                max_instances: 10,
                requires_lead_tool: Some(CliTool::Claude),
                allowed_project_binding: ProjectBinding::Any,
            },
        };

        let preset = TeamPreset {
            schema: TemplateSchema {
                kind: TemplateKind::TeamPreset,
                version: 1,
            },
            preset_id: "collision-test".to_string(),
            name: "Collision Test".to_string(),
            description: "Collision test".to_string(),
            version: "1.0.0".to_string(),
            lead_role_id: "lead".to_string(),
            lead_overrides: None,
            agent_slots: vec![
                AgentSlot {
                    role_id: "agent".to_string(),
                    count: 1,
                    project_binding: ProjectBinding::LeadProject,
                    project_id: None,
                    overrides: Some(SlotOverrides {
                        model: None,
                        reasoning_effort: None,
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
                    role_id: "agent".to_string(),
                    count: 1,
                    project_binding: ProjectBinding::LeadProject,
                    project_id: None,
                    overrides: Some(SlotOverrides {
                        model: None,
                        reasoning_effort: None,
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
            ],
            defaults: TeamPresetDefaults {
                team_name_pattern: "{project}-team".to_string(),
                tmux_layout: "tiled".to_string(),
            },
        };

        let names = preset
            .resolve_member_names(&[lead_role, agent_role], "demo")
            .expect("name collision should be resolved deterministically");
        assert!(names.iter().any(|name| name == "worker"));
        assert!(names.iter().any(|name| name == "worker-2"));
    }

    #[test]
    fn explicit_project_binding_requires_project_id() {
        let preset = TeamPreset {
            schema: TemplateSchema {
                kind: TemplateKind::TeamPreset,
                version: 1,
            },
            preset_id: "bad-preset".to_string(),
            name: "Bad Preset".to_string(),
            description: "missing project id".to_string(),
            version: "1.0.0".to_string(),
            lead_role_id: "lead".to_string(),
            lead_overrides: None,
            agent_slots: vec![AgentSlot {
                role_id: "agent".to_string(),
                count: 1,
                project_binding: ProjectBinding::ExplicitProject,
                project_id: None,
                overrides: None,
            }],
            defaults: TeamPresetDefaults {
                team_name_pattern: "{project}-team".to_string(),
                tmux_layout: "tiled".to_string(),
            },
        };

        let err = preset
            .validate()
            .expect_err("preset should fail validation");
        assert!(
            err.errors
                .iter()
                .any(|entry| entry.contains("project_id is required")),
            "expected explicit_project/project_id validation error, got: {err}"
        );
    }

    #[test]
    fn role_template_serializes_with_camel_case_keys() {
        let role = sample_role_template();
        let value = serde_json::to_value(&role).expect("serialize role template");
        let object = value.as_object().expect("role template object");

        assert!(object.contains_key("roleId"));
        assert!(!object.contains_key("role_id"));
        assert!(object.contains_key("behavioralContract"));
        assert!(!object.contains_key("behavioral_contract"));
        assert!(object.contains_key("focusArea"));
        assert!(!object.contains_key("focus_area"));
        assert!(object.contains_key("contextSummary"));
        assert!(!object.contains_key("context_summary"));
        assert!(object.contains_key("behaviorSummary"));
        assert!(!object.contains_key("behavior_summary"));
        assert!(object.contains_key("runtimeCompactSummary"));
        assert!(!object.contains_key("runtime_compact_summary"));

        let defaults = object
            .get("defaults")
            .and_then(serde_json::Value::as_object)
            .expect("defaults object");
        assert!(defaults.contains_key("cliTool"));
        assert!(!defaults.contains_key("cli_tool"));
        assert!(defaults.contains_key("defaultNamePattern"));
        assert!(!defaults.contains_key("default_name_pattern"));

        let constraints = object
            .get("constraints")
            .and_then(serde_json::Value::as_object)
            .expect("constraints object");
        assert!(constraints.contains_key("minInstances"));
        assert!(!constraints.contains_key("min_instances"));
        assert!(constraints.contains_key("maxInstances"));
        assert!(!constraints.contains_key("max_instances"));
        assert!(constraints.contains_key("requiresLeadTool"));
        assert!(!constraints.contains_key("requires_lead_tool"));
        assert!(constraints.contains_key("allowedProjectBinding"));
        assert!(!constraints.contains_key("allowed_project_binding"));
    }

    #[test]
    fn team_preset_serializes_with_camel_case_keys() {
        let preset = sample_team_preset();
        let value = serde_json::to_value(&preset).expect("serialize team preset");
        let object = value.as_object().expect("team preset object");

        assert!(object.contains_key("presetId"));
        assert!(!object.contains_key("preset_id"));
        assert!(object.contains_key("leadRoleId"));
        assert!(!object.contains_key("lead_role_id"));
        assert!(object.contains_key("agentSlots"));
        assert!(!object.contains_key("agent_slots"));

        let defaults = object
            .get("defaults")
            .and_then(serde_json::Value::as_object)
            .expect("defaults object");
        assert!(defaults.contains_key("teamNamePattern"));
        assert!(!defaults.contains_key("team_name_pattern"));

        let slots = object
            .get("agentSlots")
            .and_then(serde_json::Value::as_array)
            .expect("agentSlots array");
        let slot = slots
            .first()
            .and_then(serde_json::Value::as_object)
            .expect("first slot");
        assert!(slot.contains_key("roleId"));
        assert!(!slot.contains_key("role_id"));
        assert!(slot.contains_key("projectBinding"));
        assert!(!slot.contains_key("project_binding"));
        assert!(slot.contains_key("projectId"));
        assert!(!slot.contains_key("project_id"));

        let overrides = slot
            .get("overrides")
            .and_then(serde_json::Value::as_object)
            .expect("overrides object");
        assert!(overrides.contains_key("namePattern"));
        assert!(!overrides.contains_key("name_pattern"));
        assert!(overrides.contains_key("instructionsReplace"));
        assert!(!overrides.contains_key("instructions_replace"));
        assert!(overrides.contains_key("instructionsAppend"));
        assert!(!overrides.contains_key("instructions_append"));
        assert!(overrides.contains_key("focusArea"));
        assert!(!overrides.contains_key("focus_area"));
        assert!(overrides.contains_key("contextSummary"));
        assert!(!overrides.contains_key("context_summary"));
        assert!(overrides.contains_key("behaviorSummary"));
        assert!(!overrides.contains_key("behavior_summary"));
        assert!(overrides.contains_key("behavioralContractAppend"));
        assert!(!overrides.contains_key("behavioral_contract_append"));
    }

    #[test]
    fn team_preset_keeps_lead_overrides_in_both_spellings() {
        // The advanced preset editor pins the lead's model/effort, so the preset has
        // to carry that pin: camelCase over IPC, snake_case in the on-disk YAML.
        let camel: TeamPreset = serde_json::from_value(serde_json::json!({
            "schema": { "kind": "team_preset", "version": 1 },
            "presetId": "lead-pinned",
            "name": "Lead Pinned",
            "description": "Lead pinned to a model",
            "version": "1.0.0",
            "leadRoleId": "lead",
            "leadOverrides": { "model": "gpt-5.6-terra", "reasoningEffort": "xhigh" },
            "agentSlots": [{
                "roleId": "agent",
                "count": 1,
                "projectBinding": "lead_project",
                "projectId": null,
                "overrides": null
            }],
            "defaults": { "teamNamePattern": "{project}-team", "tmuxLayout": "tiled" }
        }))
        .expect("deserialize camelCase preset");
        let lead = camel
            .lead_overrides
            .as_ref()
            .expect("camelCase lead overrides");
        assert_eq!(lead.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(lead.reasoning_effort.as_deref(), Some("xhigh"));

        let snake: TeamPreset = serde_json::from_value(serde_json::json!({
            "schema": { "kind": "team_preset", "version": 1 },
            "preset_id": "lead-pinned",
            "name": "Lead Pinned",
            "description": "Lead pinned to a model",
            "version": "1.0.0",
            "lead_role_id": "lead",
            "lead_overrides": { "model": "gpt-5.4 high" },
            "agent_slots": [{
                "role_id": "agent",
                "count": 1,
                "project_binding": "lead_project",
                "project_id": null,
                "overrides": null
            }],
            "defaults": { "team_name_pattern": "{project}-team", "tmux_layout": "tiled" }
        }))
        .expect("deserialize snake_case preset");
        let mut normalized = snake.clone();
        normalized.normalize_model_fields();
        let lead = normalized
            .lead_overrides
            .as_ref()
            .expect("snake_case lead overrides");
        assert_eq!(lead.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(lead.reasoning_effort.as_deref(), Some("high"));

        snake.validate().expect("lead overrides stay valid");
    }

    #[test]
    fn team_preset_without_lead_overrides_deserializes() {
        let preset: TeamPreset = serde_json::from_value(serde_json::json!({
            "schema": { "kind": "team_preset", "version": 1 },
            "presetId": "plain",
            "name": "Plain",
            "description": "No lead pin",
            "version": "1.0.0",
            "leadRoleId": "lead",
            "agentSlots": [{
                "roleId": "agent",
                "count": 1,
                "projectBinding": "lead_project",
                "projectId": null,
                "overrides": null
            }],
            "defaults": { "teamNamePattern": "{project}-team", "tmuxLayout": "tiled" }
        }))
        .expect("deserialize preset without lead overrides");
        assert!(preset.lead_overrides.is_none());
    }

    #[test]
    fn role_template_allows_missing_or_empty_capabilities() {
        let mut role = sample_role_template();
        role.capabilities.clear();
        role.validate()
            .expect("empty capabilities should pass validation");

        let value = serde_json::json!({
            "schema": { "kind": "role_template", "version": 1 },
            "roleId": "minimal-role",
            "name": "Minimal Role",
            "version": "1.0.0",
            "kind": "agent",
            "defaults": {
                "cliTool": "codex",
                "model": "gpt-5.4 high",
                "defaultNamePattern": "dev-{n}"
            },
            "instructions": "Stay in lane.",
            "focusArea": "Implementation lane",
            "contextSummary": "Keeps active code-change context.",
            "behaviorSummary": "Escalates structural ambiguity.",
            "behavioralContract": {
                "communication": ["updates"],
                "execution": ["implement"],
                "escalation": ["escalate"]
            },
            "constraints": {
                "minInstances": 0,
                "maxInstances": 2,
                "allowedProjectBinding": "any"
            }
        });
        let deserialized: RoleTemplate =
            serde_json::from_value(value).expect("deserialize without capabilities");
        assert!(deserialized.capabilities.is_empty());
        deserialized
            .validate()
            .expect("missing capabilities should pass validation");
    }

    #[test]
    fn role_template_serializes_new_context_fields() {
        let role = sample_role_template();
        let round_trip = serde_json::from_value::<RoleTemplate>(
            serde_json::to_value(&role).expect("serialize role"),
        )
        .expect("deserialize role");
        assert_eq!(
            round_trip.focus_area.as_deref(),
            Some("Implementation lane")
        );
        assert_eq!(
            round_trip.context_summary.as_deref(),
            Some("Carries active implementation context for scoped code changes.")
        );
        assert_eq!(
            round_trip.behavior_summary.as_deref(),
            Some("Implements assigned work and escalates structural uncertainty.")
        );
        assert_eq!(
            round_trip
                .runtime_compact_summary
                .as_ref()
                .map(RuntimeCompactSummary::total_word_count),
            Some(100)
        );
    }

    #[test]
    fn role_template_serializes_optional_provenance() {
        let mut role = sample_role_template();
        role.provenance = Some(RoleProvenance {
            source_format: crate::templates::adapters::RoleExportFormat::ClaudeAgent,
            source_version: Some("1".to_string()),
            source_path: Some(".claude/agents/sample-role.md".to_string()),
            imported_at: chrono::Utc::now(),
            non_roundtrippable_fields: vec!["constraints".to_string()],
        });

        let value = serde_json::to_value(&role).expect("serialize role");
        assert!(value.get("provenance").is_some());

        let round_trip = serde_json::from_value::<RoleTemplate>(value).expect("deserialize role");
        assert_eq!(
            round_trip
                .provenance
                .as_ref()
                .and_then(|provenance| provenance.source_path.as_deref()),
            Some(".claude/agents/sample-role.md")
        );
    }

    #[test]
    fn runtime_compact_summary_validation_rejects_out_of_bounds_summary() {
        let mut role = sample_role_template();
        role.runtime_compact_summary = Some(RuntimeCompactSummary {
            role_purpose: "Too short.".to_string(),
            keep_doing: vec!["Keep going.".to_string()],
            workflow_sequence: vec!["Step.".to_string()],
            avoid: vec!["Avoid.".to_string()],
            escalate_when: vec!["Escalate.".to_string()],
        });

        let err = role
            .validate()
            .expect_err("summary under the size bound should fail validation");
        assert!(
            err.errors
                .iter()
                .any(|entry| entry.contains("runtime_compact_summary must be between")),
            "expected runtime_compact_summary size validation error, got: {err}"
        );
    }
}
