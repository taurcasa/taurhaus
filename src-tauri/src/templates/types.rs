use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::session_scanner::cli_tool::CliTool;

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
    #[serde(alias = "behavioral_contract")]
    pub behavioral_contract: BehavioralContract,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub constraints: RoleConstraints,
}

impl RoleTemplate {
    pub fn validate(&self) -> Result<(), TemplateValidationError> {
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

        if self.behavioral_contract.is_empty() {
            errors.push(format!(
                "role '{}' behavioral_contract must include at least one bullet",
                self.role_id
            ));
        }
        self.behavioral_contract
            .validate("behavioral_contract", &mut errors);

        validate_string_list("capabilities", &self.capabilities, &mut errors);
        if self.capabilities.is_empty() {
            errors.push(format!(
                "role '{}' capabilities must include at least one tag",
                self.role_id
            ));
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
    #[serde(alias = "name_pattern")]
    pub name_pattern: Option<String>,
    #[serde(alias = "instructions_replace")]
    pub instructions_replace: Option<String>,
    #[serde(alias = "instructions_append")]
    pub instructions_append: Option<String>,
    #[serde(alias = "behavioral_contract_append")]
    pub behavioral_contract_append: Option<BehavioralContract>,
    #[serde(default, alias = "capabilities_add")]
    pub capabilities_add: Vec<String>,
    #[serde(default, alias = "capabilities_remove")]
    pub capabilities_remove: Vec<String>,
}

impl SlotOverrides {
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
        if let Some(contract) = self.behavioral_contract_append.as_ref() {
            contract.validate(
                &format!("{field_prefix}.behavioral_contract_append"),
                errors,
            );
        }

        validate_string_list(
            &format!("{field_prefix}.capabilities_add"),
            &self.capabilities_add,
            errors,
        );
        validate_string_list(
            &format!("{field_prefix}.capabilities_remove"),
            &self.capabilities_remove,
            errors,
        );

        let add_set: HashSet<&str> = self.capabilities_add.iter().map(String::as_str).collect();
        let remove_set: HashSet<&str> = self
            .capabilities_remove
            .iter()
            .map(String::as_str)
            .collect();

        for overlap in add_set.intersection(&remove_set) {
            errors.push(format!(
                "{field_prefix}.capabilities_add and capabilities_remove both contain '{overlap}'"
            ));
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
    #[serde(default, alias = "agent_slots")]
    pub agent_slots: Vec<AgentSlot>,
    pub defaults: TeamPresetDefaults,
}

impl TeamPreset {
    pub fn validate(&self) -> Result<(), TemplateValidationError> {
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
                model: "gpt-5.3-codex".to_string(),
                default_name_pattern: "dev-{n}".to_string(),
            },
            instructions: "Execute scoped tasks.".to_string(),
            behavioral_contract: BehavioralContract {
                communication: vec!["post updates".to_string()],
                execution: vec!["deliver tests".to_string()],
                escalation: vec!["raise blockers".to_string()],
            },
            capabilities: vec!["implementation".to_string()],
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
            agent_slots: vec![AgentSlot {
                role_id: "sample-role".to_string(),
                count: 2,
                project_binding: ProjectBinding::ExplicitProject,
                project_id: Some("project-a".to_string()),
                overrides: Some(SlotOverrides {
                    model: Some("gpt-5.3-codex".to_string()),
                    name_pattern: Some("dev-{n}".to_string()),
                    instructions_replace: Some("Replace instructions".to_string()),
                    instructions_append: Some("Append instructions".to_string()),
                    behavioral_contract_append: Some(BehavioralContract {
                        communication: vec!["sync daily".to_string()],
                        execution: vec!["ship incrementally".to_string()],
                        escalation: vec!["escalate quickly".to_string()],
                    }),
                    capabilities_add: vec!["review".to_string()],
                    capabilities_remove: vec!["triage".to_string()],
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
                serde_yaml::from_str::<RoleTemplate>(&raw)
                    .unwrap_or_else(|err| panic!("parse role template {}: {err}", path.display()))
            })
            .collect()
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
                serde_yaml::from_str::<TeamPreset>(&raw)
                    .unwrap_or_else(|err| panic!("parse team preset {}: {err}", path.display()))
            })
            .collect()
    }

    #[test]
    fn role_templates_deserialize_and_validate() {
        let roles = load_role_templates();
        assert_eq!(roles.len(), 5, "expected five built-in role templates");

        for role in &roles {
            role.validate()
                .unwrap_or_else(|err| panic!("role '{}' failed validation: {err}", role.role_id));
        }
    }

    #[test]
    fn team_presets_deserialize_and_validate_against_roles() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        assert_eq!(presets.len(), 3, "expected three built-in team presets");

        for preset in &presets {
            preset
                .validate_with_role_catalog(&roles)
                .unwrap_or_else(|err| {
                    panic!("preset '{}' failed validation: {err}", preset.preset_id)
                });
        }
    }

    #[test]
    fn fullstack_preset_resolves_member_names() {
        let roles = load_role_templates();
        let presets = load_team_presets();
        let preset = presets
            .iter()
            .find(|preset| preset.preset_id == "fullstack-dev")
            .expect("fullstack-dev preset exists");

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
                default_name_pattern: "lead-{project}".to_string(),
            },
            instructions: "Lead".to_string(),
            behavioral_contract: BehavioralContract {
                communication: vec!["a".to_string()],
                execution: vec!["b".to_string()],
                escalation: vec!["c".to_string()],
            },
            capabilities: vec!["planning".to_string()],
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
                model: "gpt-5.3-codex".to_string(),
                default_name_pattern: "worker".to_string(),
            },
            instructions: "Agent".to_string(),
            behavioral_contract: BehavioralContract {
                communication: vec!["a".to_string()],
                execution: vec!["b".to_string()],
                escalation: vec!["c".to_string()],
            },
            capabilities: vec!["implementation".to_string()],
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
            agent_slots: vec![
                AgentSlot {
                    role_id: "agent".to_string(),
                    count: 1,
                    project_binding: ProjectBinding::LeadProject,
                    project_id: None,
                    overrides: Some(SlotOverrides {
                        model: None,
                        name_pattern: Some("worker".to_string()),
                        instructions_replace: None,
                        instructions_append: None,
                        behavioral_contract_append: None,
                        capabilities_add: Vec::new(),
                        capabilities_remove: Vec::new(),
                    }),
                },
                AgentSlot {
                    role_id: "agent".to_string(),
                    count: 1,
                    project_binding: ProjectBinding::LeadProject,
                    project_id: None,
                    overrides: Some(SlotOverrides {
                        model: None,
                        name_pattern: Some("worker".to_string()),
                        instructions_replace: None,
                        instructions_append: None,
                        behavioral_contract_append: None,
                        capabilities_add: Vec::new(),
                        capabilities_remove: Vec::new(),
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
        assert!(overrides.contains_key("behavioralContractAppend"));
        assert!(!overrides.contains_key("behavioral_contract_append"));
        assert!(overrides.contains_key("capabilitiesAdd"));
        assert!(!overrides.contains_key("capabilities_add"));
        assert!(overrides.contains_key("capabilitiesRemove"));
        assert!(!overrides.contains_key("capabilities_remove"));
    }
}
