use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::ModelCatalog;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch::ModelSpec;
use crate::templates::types::{
    BehavioralContract, CapabilityPolicy, ProjectBinding, RoleConstraints, RoleKind, RoleTemplate,
};

/// Canonical field mapping for Taurhaus role import/export adapters.
///
/// This table is the contract for stream-2 adapter work:
/// - `claude_agent` means `.claude/agents/*.md` with YAML frontmatter + Markdown body.
/// - `copilot_agent` means `.github/agents/*.md` with YAML frontmatter + Markdown body.
/// - `instruction_only` covers `AGENTS.md`, `GEMINI.md`, Cursor rules, and Windsurf rules.
/// - `export_mapping` describes how Taurhaus writes the field out.
/// - `import_fidelity` describes whether Taurhaus can round-trip that field back without loss.
pub const ROLE_FIELD_MAPPINGS: &[RoleFieldMappingRow] = &[
    RoleFieldMappingRow {
        taurhaus_field: "name",
        claude_agent: "frontmatter.name",
        copilot_agent: "frontmatter.name",
        instruction_only: "H1/title line in Markdown body",
        export_mapping: "Direct field when schema exists; title heading for instruction-only exports",
        import_fidelity: "Lossless for Claude/Copilot, lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "defaults.model",
        claude_agent: "frontmatter.model",
        copilot_agent: "frontmatter.model",
        instruction_only: "Optional 'Model' section in Markdown body",
        export_mapping: "Direct field when schema exists; rendered as body metadata for instruction-only exports",
        import_fidelity: "Lossless for Claude/Copilot, lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "defaults.reasoning_effort",
        claude_agent: "Not represented",
        copilot_agent: "Not represented",
        instruction_only: "Not represented",
        export_mapping: "Not exported; external agent frontmatter has no effort field",
        import_fidelity: "Lossy for all non-YAML formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "capability_policy",
        claude_agent: "frontmatter.capability_policy",
        copilot_agent: "frontmatter.capability_policy",
        instruction_only: "Not represented",
        export_mapping: "Namespaced policy data in agent frontmatter; omitted when absent",
        import_fidelity: "Lossless for YAML and Claude/Copilot; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "instructions",
        claude_agent: "Markdown body",
        copilot_agent: "Markdown body",
        instruction_only: "Markdown body",
        export_mapping: "Direct body content in all formats",
        import_fidelity: "Lossless for all formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "focus_area",
        claude_agent: "Compiled 'Focus Area' section in body",
        copilot_agent: "Compiled 'Focus Area' section in body",
        instruction_only: "Compiled 'Focus Area' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "context_summary",
        claude_agent: "Compiled 'Context Summary' section in body",
        copilot_agent: "Compiled 'Context Summary' section in body",
        instruction_only: "Compiled 'Context Summary' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "behavior_summary",
        claude_agent: "Compiled 'Behavior Summary' section in body",
        copilot_agent: "frontmatter.description + compiled body section",
        instruction_only: "Compiled 'Behavior Summary' section in body",
        export_mapping: "Rendered into prompt appendix section; Copilot may also mirror it into description",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy/partial for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "communication_style",
        claude_agent: "Compiled 'Communication Style' section in body",
        copilot_agent: "Compiled 'Communication Style' section in body",
        instruction_only: "Compiled 'Communication Style' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "behavioral_contract",
        claude_agent: "Compiled 'Behavioral Contract' section in body",
        copilot_agent: "Compiled 'Behavioral Contract' section in body",
        instruction_only: "Compiled 'Behavioral Contract' section in body",
        export_mapping: "Rendered as grouped bullet lists in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "quality_gates",
        claude_agent: "Compiled 'Quality Gates' section in body",
        copilot_agent: "Compiled 'Quality Gates' section in body",
        instruction_only: "Compiled 'Quality Gates' section in body",
        export_mapping: "Rendered as bullet list section in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "handoff_expectations",
        claude_agent: "Compiled 'Handoff Expectations' section in body",
        copilot_agent: "Compiled 'Handoff Expectations' section in body",
        instruction_only: "Compiled 'Handoff Expectations' section in Markdown body",
        export_mapping: "Rendered as bullet list section in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "definition_of_done",
        claude_agent: "Compiled 'Definition of Done' section in body",
        copilot_agent: "Compiled 'Definition of Done' section in body",
        instruction_only: "Compiled 'Definition of Done' section in body",
        export_mapping: "Rendered as bullet list section in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "phase_scope",
        claude_agent: "Compiled 'Phase Scope' section in body",
        copilot_agent: "Compiled 'Phase Scope' section in body",
        instruction_only: "Compiled 'Phase Scope' section in body",
        export_mapping: "Rendered as bullet list section in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "mode",
        claude_agent: "Compiled 'Mode' section in body",
        copilot_agent: "Compiled 'Mode' section in body",
        instruction_only: "Compiled 'Mode' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "inherits_from",
        claude_agent: "Compiled 'Inherits From' section in body",
        copilot_agent: "Compiled 'Inherits From' section in body",
        instruction_only: "Compiled 'Inherits From' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "required_artifacts",
        claude_agent: "Compiled 'Required Artifacts' section in body",
        copilot_agent: "Compiled 'Required Artifacts' section in body",
        instruction_only: "Compiled 'Required Artifacts' section in body",
        export_mapping: "Rendered as bullet list section in prompt appendix",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "capabilities",
        claude_agent: "frontmatter.tools (partial)",
        copilot_agent: "No direct field",
        instruction_only: "Compiled 'Capabilities' section in body",
        export_mapping: "Mapped to Claude tools where possible; otherwise body section only",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; partial for arbitrary Claude imports, lossy for instruction-only formats",
    },
    RoleFieldMappingRow {
        taurhaus_field: "constraints",
        claude_agent: "Compiled 'Constraints' section in body",
        copilot_agent: "Compiled 'Constraints' section in body",
        instruction_only: "Compiled 'Constraints' section in body",
        export_mapping: "Rendered into prompt appendix section only",
        import_fidelity: "Lossless for Taurhaus-generated Claude/Copilot exports; lossy for instruction-only formats",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleFieldMappingRow {
    pub taurhaus_field: &'static str,
    pub claude_agent: &'static str,
    pub copilot_agent: &'static str,
    pub instruction_only: &'static str,
    pub export_mapping: &'static str,
    pub import_fidelity: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleExportFormat {
    Yaml,
    ClaudeAgent,
    CopilotAgent,
    AgentsMd,
    GeminiMd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleExportResult {
    pub target_format: RoleExportFormat,
    pub file_content: String,
    #[serde(default)]
    pub lossy_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoleParsedFields {
    pub name: Option<String>,
    pub model: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    pub prompt_body: Option<String>,
    #[serde(default)]
    pub capability_policy: Option<CapabilityPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleImportSource {
    pub source_format: RoleExportFormat,
    pub parsed_fields: RoleParsedFields,
    pub provenance: RoleProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedRoleTemplate {
    pub template: RoleTemplate,
    pub import_source: RoleImportSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleProvenance {
    pub source_format: RoleExportFormat,
    pub source_version: Option<String>,
    pub source_path: Option<String>,
    pub imported_at: DateTime<Utc>,
    #[serde(default)]
    pub non_roundtrippable_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptSection {
    heading: &'static str,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ParsedCompiledPromptBody {
    instructions: String,
    focus_area: Option<String>,
    context_summary: Option<String>,
    behavior_summary: Option<String>,
    communication_style: Option<String>,
    behavioral_contract: Option<BehavioralContract>,
    quality_gates: Option<Vec<String>>,
    handoff_expectations: Option<Vec<String>>,
    definition_of_done: Option<Vec<String>>,
    phase_scope: Option<Vec<String>>,
    mode: Option<String>,
    inherits_from: Option<String>,
    required_artifacts: Option<Vec<String>>,
    capabilities: Option<Vec<String>>,
    constraints: Option<RoleConstraints>,
    default_cli_tool: Option<CliTool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownPromptHeading {
    FocusArea,
    ContextSummary,
    BehaviorSummary,
    CommunicationStyle,
    BehavioralContract,
    QualityGates,
    HandoffExpectations,
    DefinitionOfDone,
    PhaseScope,
    Mode,
    InheritsFrom,
    RequiredArtifacts,
    Capabilities,
    Constraints,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoleImportError {
    #[error("unsupported import format '{0:?}'")]
    UnsupportedFormat(RoleExportFormat),
    #[error("invalid yaml frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error("imported role body is empty")]
    EmptyBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct ClaudeAgentFrontmatter {
    name: Option<String>,
    model: Option<String>,
    #[serde(default)]
    tools: Option<StringListOrScalar>,
    #[serde(default, alias = "capabilityPolicy")]
    capability_policy: Option<CapabilityPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct CopilotAgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    model: Option<String>,
    #[serde(default, alias = "capabilityPolicy")]
    capability_policy: Option<CapabilityPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum StringListOrScalar {
    List(Vec<String>),
    Scalar(String),
}

impl Default for StringListOrScalar {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

impl StringListOrScalar {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::List(items) => items,
            Self::Scalar(item) => vec![item],
        }
    }
}

pub fn export_role(role: &RoleTemplate, format: RoleExportFormat) -> RoleExportResult {
    let lossy_fields = combined_lossy_fields_for_export(role, format);
    let file_content = match format {
        RoleExportFormat::Yaml => serde_norway::to_string(role).expect("serialize role yaml"),
        RoleExportFormat::ClaudeAgent => render_claude_agent(role, &compile_prompt_body(role)),
        RoleExportFormat::CopilotAgent => render_copilot_agent(role, &compile_prompt_body(role)),
        RoleExportFormat::AgentsMd => render_agents_md(role, &lossy_fields),
        RoleExportFormat::GeminiMd => render_gemini_md(role, &lossy_fields),
    };

    RoleExportResult {
        target_format: format,
        file_content,
        lossy_fields,
    }
}

pub fn import_role(
    format: RoleExportFormat,
    raw: &str,
    source_path: Option<&str>,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    import_role_at(format, raw, source_path, Utc::now())
}

pub fn import_role_at(
    format: RoleExportFormat,
    raw: &str,
    source_path: Option<&str>,
    imported_at: DateTime<Utc>,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    match format {
        RoleExportFormat::Yaml => Err(RoleImportError::UnsupportedFormat(format)),
        RoleExportFormat::ClaudeAgent => import_claude_agent_at(raw, source_path, imported_at),
        RoleExportFormat::CopilotAgent => import_copilot_agent_at(raw, source_path, imported_at),
        other => Err(RoleImportError::UnsupportedFormat(other)),
    }
}

pub fn compile_prompt_body(role: &RoleTemplate) -> String {
    let mut blocks = vec![role.instructions.trim().to_string()];
    for section in compiled_prompt_sections(role) {
        blocks.push(format!("## {}\n{}", section.heading, section.body));
    }
    blocks
        .into_iter()
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn import_claude_agent_at(
    raw: &str,
    source_path: Option<&str>,
    imported_at: DateTime<Utc>,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    let (frontmatter_raw, body) = split_frontmatter_and_body(raw)?;
    let frontmatter = match frontmatter_raw {
        Some(frontmatter) => serde_norway::from_str::<ClaudeAgentFrontmatter>(&frontmatter)
            .map_err(|err| RoleImportError::InvalidFrontmatter(err.to_string()))?,
        None => ClaudeAgentFrontmatter::default(),
    };

    build_imported_role(
        RoleExportFormat::ClaudeAgent,
        body,
        source_path,
        imported_at,
        frontmatter.name,
        frontmatter.model,
        None,
        frontmatter.tools.unwrap_or_default().into_vec(),
        frontmatter.capability_policy,
        CliTool::Claude,
    )
}

fn import_copilot_agent_at(
    raw: &str,
    source_path: Option<&str>,
    imported_at: DateTime<Utc>,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    let (frontmatter_raw, body) = split_frontmatter_and_body(raw)?;
    let frontmatter = match frontmatter_raw {
        Some(frontmatter) => serde_norway::from_str::<CopilotAgentFrontmatter>(&frontmatter)
            .map_err(|err| RoleImportError::InvalidFrontmatter(err.to_string()))?,
        None => CopilotAgentFrontmatter::default(),
    };

    build_imported_role(
        RoleExportFormat::CopilotAgent,
        body,
        source_path,
        imported_at,
        frontmatter.name,
        frontmatter.model,
        frontmatter.description,
        Vec::new(),
        frontmatter.capability_policy,
        CliTool::Codex,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_imported_role(
    format: RoleExportFormat,
    body: &str,
    source_path: Option<&str>,
    imported_at: DateTime<Utc>,
    imported_name: Option<String>,
    imported_model: Option<String>,
    imported_description: Option<String>,
    imported_tools: Vec<String>,
    capability_policy: Option<CapabilityPolicy>,
    cli_tool: CliTool,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    let parsed_body = parse_compiled_prompt_body(body);
    let instructions = parsed_body.instructions.trim();
    if instructions.is_empty() {
        return Err(RoleImportError::EmptyBody);
    }

    let fallback_name = source_path
        .and_then(stem_from_source_path)
        .unwrap_or("imported-role");
    let name = imported_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    let role_id = slugify_identifier(&name);
    let capabilities = parsed_body
        .capabilities
        .clone()
        .unwrap_or_else(|| match format {
            RoleExportFormat::ClaudeAgent => map_claude_tools_to_capabilities(&imported_tools),
            RoleExportFormat::CopilotAgent => Vec::new(),
            _ => Vec::new(),
        });
    let context_summary = parsed_body.context_summary.clone().or_else(|| {
        imported_description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });
    let parsed_fields = RoleParsedFields {
        name: imported_name,
        model: imported_model.clone(),
        description: imported_description,
        tools: imported_tools,
        prompt_body: Some(instructions.to_string()),
        capability_policy: capability_policy.clone(),
    };
    let provenance = RoleProvenance {
        source_format: format,
        source_version: Some("1".to_string()),
        source_path: source_path.map(str::to_string),
        imported_at,
        non_roundtrippable_fields: synthesized_fields_for_import(format, &parsed_body),
    };

    let default_cli_tool = parsed_body.default_cli_tool.unwrap_or(cli_tool);
    let default_model = imported_model
        .clone()
        .or_else(|| ModelCatalog::default_for(default_cli_tool).map(|entry| entry.id.clone()))
        .unwrap_or_default();
    let model = ModelSpec::parse_legacy(&default_model);

    Ok(ImportedRoleTemplate {
        template: RoleTemplate {
            schema: crate::templates::types::TemplateSchema {
                kind: crate::templates::types::TemplateKind::RoleTemplate,
                version: 1,
            },
            role_id: role_id.clone(),
            name,
            version: "imported-1".to_string(),
            kind: RoleKind::Agent,
            defaults: crate::templates::types::RoleDefaults {
                cli_tool: default_cli_tool,
                model: model.model.unwrap_or(default_model),
                reasoning_effort: model.reasoning_effort,
                default_name_pattern: format!("{role_id}-{{n}}"),
            },
            capability_policy,
            instructions: instructions.to_string(),
            focus_area: parsed_body.focus_area,
            context_summary,
            behavior_summary: parsed_body.behavior_summary,
            communication_style: parsed_body.communication_style,
            runtime_compact_summary: None,
            behavioral_contract: parsed_body
                .behavioral_contract
                .unwrap_or_else(default_import_behavioral_contract),
            quality_gates: parsed_body.quality_gates,
            handoff_expectations: parsed_body.handoff_expectations,
            definition_of_done: parsed_body.definition_of_done,
            phase_scope: parsed_body.phase_scope,
            mode: parsed_body.mode,
            inherits_from: parsed_body.inherits_from,
            required_artifacts: parsed_body.required_artifacts,
            capabilities,
            provenance: Some(provenance.clone()),
            constraints: parsed_body
                .constraints
                .unwrap_or_else(default_import_constraints),
        },
        import_source: RoleImportSource {
            source_format: format,
            parsed_fields,
            provenance,
        },
    })
}

pub fn lossy_fields_for_export(role: &RoleTemplate, format: RoleExportFormat) -> Vec<String> {
    let mut lossy = Vec::new();

    if format != RoleExportFormat::Yaml && role.defaults.reasoning_effort.is_some() {
        lossy.push("defaults.reasoning_effort".to_string());
    }

    match format {
        RoleExportFormat::Yaml => {}
        RoleExportFormat::ClaudeAgent => {}
        RoleExportFormat::CopilotAgent => {}
        RoleExportFormat::AgentsMd | RoleExportFormat::GeminiMd => {
            lossy.push("name".to_string());
            lossy.push("defaults.model".to_string());
            if !role.capabilities.is_empty() {
                lossy.push("capabilities".to_string());
            }
            push_compiled_section_losses(role, &mut lossy);
        }
    }

    lossy
}

fn combined_lossy_fields_for_export(role: &RoleTemplate, format: RoleExportFormat) -> Vec<String> {
    let mut lossy = lossy_fields_for_export(role, format);
    if let Some(provenance) = &role.provenance {
        lossy.extend(provenance.non_roundtrippable_fields.iter().cloned());
    }
    dedupe_preserving_order(lossy)
}

fn render_claude_agent(role: &RoleTemplate, body: &str) -> String {
    let tools = map_capabilities_to_claude_tools(&role.capabilities);
    let mut frontmatter = vec![
        "---".to_string(),
        format!("name: {}", yaml_scalar(&role.name)),
        format!("model: {}", yaml_scalar(&role.defaults.model)),
    ];
    if !tools.is_empty() {
        frontmatter.push(format!("tools: [{}]", tools.join(", ")));
    }
    push_capability_policy_frontmatter(&mut frontmatter, role.capability_policy.as_ref());
    frontmatter.push("---".to_string());

    format!("{}\n\n{}", frontmatter.join("\n"), body)
}

fn render_copilot_agent(role: &RoleTemplate, body: &str) -> String {
    let description = role
        .behavior_summary
        .as_deref()
        .or(role.context_summary.as_deref())
        .unwrap_or("Exported from Taurhaus role template.");
    let mut frontmatter = vec![
        "---".to_string(),
        format!("name: {}", yaml_scalar(&role.name)),
        format!("description: {}", yaml_scalar(description)),
        format!("model: {}", yaml_scalar(&role.defaults.model)),
    ];
    push_capability_policy_frontmatter(&mut frontmatter, role.capability_policy.as_ref());
    frontmatter.push("---".to_string());

    format!("{}\n\n{}", frontmatter.join("\n"), body)
}

fn push_capability_policy_frontmatter(
    frontmatter: &mut Vec<String>,
    policy: Option<&CapabilityPolicy>,
) {
    let Some(policy) = policy else {
        return;
    };
    frontmatter.push("capability_policy:".to_string());
    let serialized = serde_norway::to_string(policy).expect("serialize capability policy");
    frontmatter.extend(serialized.lines().map(|line| format!("  {line}")));
}

fn render_agents_md(role: &RoleTemplate, lossy_fields: &[String]) -> String {
    render_instruction_only_document("AGENTS.md", role, lossy_fields)
}

fn render_gemini_md(role: &RoleTemplate, lossy_fields: &[String]) -> String {
    render_instruction_only_document("GEMINI.md", role, lossy_fields)
}

fn render_instruction_only_document(
    format_label: &str,
    role: &RoleTemplate,
    lossy_fields: &[String],
) -> String {
    let mut blocks = vec![
        format!("# {}", role.name.trim()),
        format!(
            "_Instruction-only export for {}. This format is intentionally lossy._",
            format_label
        ),
        "## Metadata".to_string(),
        [
            format!("- role id: {}", role.role_id.trim()),
            format!("- role kind: {}", render_role_kind(role.kind)),
            format!("- cli tool: {}", role.defaults.cli_tool),
            format!("- default model: {}", role.defaults.model.trim()),
            format!(
                "- default name pattern: {}",
                role.defaults.default_name_pattern.trim()
            ),
        ]
        .join("\n"),
        "## Core Instructions".to_string(),
        role.instructions.trim().to_string(),
    ];

    for section in compiled_prompt_sections(role) {
        blocks.push(format!("## {}\n{}", section.heading, section.body));
    }

    blocks.push(render_round_trip_notes(lossy_fields));
    blocks.join("\n\n")
}

fn render_round_trip_notes(lossy_fields: &[String]) -> String {
    let mut lines = vec![
        "## Round-Trip Notes".to_string(),
        "This export does not preserve the full Taurhaus role schema.".to_string(),
    ];

    if lossy_fields.is_empty() {
        lines.push("- no known lossy fields".to_string());
    } else {
        lines.push("- non-round-trippable or downgraded fields:".to_string());
        lines.extend(
            lossy_fields
                .iter()
                .map(|field| format!("  - {}", field.trim())),
        );
    }

    lines.join("\n")
}

fn compiled_prompt_sections(role: &RoleTemplate) -> Vec<PromptSection> {
    let mut sections = Vec::new();

    if let Some(value) = non_empty(role.focus_area.as_deref()) {
        sections.push(PromptSection {
            heading: "Focus Area",
            body: value.to_string(),
        });
    }

    if let Some(value) = non_empty(role.context_summary.as_deref()) {
        sections.push(PromptSection {
            heading: "Context Summary",
            body: value.to_string(),
        });
    }

    if let Some(value) = non_empty(role.behavior_summary.as_deref()) {
        sections.push(PromptSection {
            heading: "Behavior Summary",
            body: value.to_string(),
        });
    }

    if let Some(value) = non_empty(role.communication_style.as_deref()) {
        sections.push(PromptSection {
            heading: "Communication Style",
            body: value.to_string(),
        });
    }

    if !role.behavioral_contract.communication.is_empty()
        || !role.behavioral_contract.execution.is_empty()
        || !role.behavioral_contract.escalation.is_empty()
    {
        let mut lines = Vec::new();
        push_bullets(
            &mut lines,
            "Communication",
            &role.behavioral_contract.communication,
        );
        push_bullets(&mut lines, "Execution", &role.behavioral_contract.execution);
        push_bullets(
            &mut lines,
            "Escalation",
            &role.behavioral_contract.escalation,
        );
        sections.push(PromptSection {
            heading: "Behavioral Contract",
            body: lines.join("\n"),
        });
    }

    if let Some(quality_gates) = role
        .quality_gates
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        sections.push(PromptSection {
            heading: "Quality Gates",
            body: render_bulleted_list(quality_gates),
        });
    }

    if let Some(handoff_expectations) = role
        .handoff_expectations
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        sections.push(PromptSection {
            heading: "Handoff Expectations",
            body: render_bulleted_list(handoff_expectations),
        });
    }

    if let Some(definition_of_done) = role
        .definition_of_done
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        sections.push(PromptSection {
            heading: "Definition of Done",
            body: render_bulleted_list(definition_of_done),
        });
    }

    if let Some(phase_scope) = role.phase_scope.as_ref().filter(|items| !items.is_empty()) {
        sections.push(PromptSection {
            heading: "Phase Scope",
            body: render_bulleted_list(phase_scope),
        });
    }

    if let Some(value) = non_empty(role.mode.as_deref()) {
        sections.push(PromptSection {
            heading: "Mode",
            body: value.to_string(),
        });
    }

    if let Some(value) = non_empty(role.inherits_from.as_deref()) {
        sections.push(PromptSection {
            heading: "Inherits From",
            body: value.to_string(),
        });
    }

    if let Some(required_artifacts) = role
        .required_artifacts
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        sections.push(PromptSection {
            heading: "Required Artifacts",
            body: render_bulleted_list(required_artifacts),
        });
    }

    if !role.capabilities.is_empty() {
        sections.push(PromptSection {
            heading: "Capabilities",
            body: render_bulleted_list(&role.capabilities),
        });
    }

    sections.push(PromptSection {
        heading: "Constraints",
        body: render_constraints(role),
    });

    sections
}

fn render_constraints(role: &RoleTemplate) -> String {
    let requires_lead_tool = role
        .constraints
        .requires_lead_tool
        .map(|tool| tool.to_string())
        .unwrap_or_else(|| "none".to_string());

    [
        format!("- role kind: {}", render_role_kind(role.kind)),
        format!("- min instances: {}", role.constraints.min_instances),
        format!("- max instances: {}", role.constraints.max_instances),
        format!(
            "- allowed project binding: {}",
            render_project_binding(role.constraints.allowed_project_binding)
        ),
        format!("- required lead tool: {requires_lead_tool}"),
        format!("- default cli tool: {}", role.defaults.cli_tool),
    ]
    .join("\n")
}

fn parse_compiled_prompt_body(body: &str) -> ParsedCompiledPromptBody {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return ParsedCompiledPromptBody::default();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut parsed = ParsedCompiledPromptBody::default();
    let mut intro = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        if prompt_heading_from_line(lines[index]).is_some() {
            break;
        }
        intro.push(lines[index]);
        index += 1;
    }

    parsed.instructions = intro.join("\n").trim().to_string();

    while index < lines.len() {
        let Some(heading) = prompt_heading_from_line(lines[index]) else {
            let tail = lines[index..].join("\n").trim().to_string();
            if !tail.is_empty() {
                if parsed.instructions.is_empty() {
                    parsed.instructions = tail;
                } else {
                    parsed.instructions.push_str("\n\n");
                    parsed.instructions.push_str(&tail);
                }
            }
            break;
        };

        index += 1;
        let body_start = index;
        while index < lines.len() {
            if prompt_heading_from_line(lines[index]).is_some() {
                break;
            }
            index += 1;
        }
        let section_body = lines[body_start..index].join("\n").trim().to_string();
        apply_parsed_prompt_section(&mut parsed, heading, &section_body);
    }

    parsed.instructions = parsed.instructions.trim().to_string();
    parsed
}

fn prompt_heading_from_line(line: &str) -> Option<KnownPromptHeading> {
    match line.trim() {
        "## Focus Area" => Some(KnownPromptHeading::FocusArea),
        "## Context Summary" => Some(KnownPromptHeading::ContextSummary),
        "## Behavior Summary" => Some(KnownPromptHeading::BehaviorSummary),
        "## Communication Style" => Some(KnownPromptHeading::CommunicationStyle),
        "## Behavioral Contract" => Some(KnownPromptHeading::BehavioralContract),
        "## Quality Gates" => Some(KnownPromptHeading::QualityGates),
        "## Handoff Expectations" => Some(KnownPromptHeading::HandoffExpectations),
        "## Definition of Done" => Some(KnownPromptHeading::DefinitionOfDone),
        "## Phase Scope" => Some(KnownPromptHeading::PhaseScope),
        "## Mode" => Some(KnownPromptHeading::Mode),
        "## Inherits From" => Some(KnownPromptHeading::InheritsFrom),
        "## Required Artifacts" => Some(KnownPromptHeading::RequiredArtifacts),
        "## Capabilities" => Some(KnownPromptHeading::Capabilities),
        "## Constraints" => Some(KnownPromptHeading::Constraints),
        _ => None,
    }
}

fn apply_parsed_prompt_section(
    parsed: &mut ParsedCompiledPromptBody,
    heading: KnownPromptHeading,
    body: &str,
) {
    match heading {
        KnownPromptHeading::FocusArea => {
            parsed.focus_area = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::ContextSummary => {
            parsed.context_summary = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::BehaviorSummary => {
            parsed.behavior_summary = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::CommunicationStyle => {
            parsed.communication_style = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::BehavioralContract => {
            parsed.behavioral_contract = parse_behavioral_contract(body);
        }
        KnownPromptHeading::QualityGates => {
            parsed.quality_gates = parse_bulleted_list(body);
        }
        KnownPromptHeading::HandoffExpectations => {
            parsed.handoff_expectations = parse_bulleted_list(body);
        }
        KnownPromptHeading::DefinitionOfDone => {
            parsed.definition_of_done = parse_bulleted_list(body);
        }
        KnownPromptHeading::PhaseScope => {
            parsed.phase_scope = parse_bulleted_list(body);
        }
        KnownPromptHeading::Mode => {
            parsed.mode = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::InheritsFrom => {
            parsed.inherits_from = non_empty(Some(body)).map(str::to_string);
        }
        KnownPromptHeading::RequiredArtifacts => {
            parsed.required_artifacts = parse_bulleted_list(body);
        }
        KnownPromptHeading::Capabilities => {
            parsed.capabilities = parse_bulleted_list(body);
        }
        KnownPromptHeading::Constraints => {
            let (constraints, cli_tool) = parse_constraints_section(body);
            parsed.constraints = constraints;
            parsed.default_cli_tool = cli_tool;
        }
    }
}

fn parse_bulleted_list(body: &str) -> Option<Vec<String>> {
    let items = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- ").map(|item| item.trim()))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn parse_behavioral_contract(body: &str) -> Option<BehavioralContract> {
    let mut current_section = None::<&str>;
    let mut communication = Vec::new();
    let mut execution = Vec::new();
    let mut escalation = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        match trimmed {
            "### Communication" => current_section = Some("communication"),
            "### Execution" => current_section = Some("execution"),
            "### Escalation" => current_section = Some("escalation"),
            _ => {
                let Some(item) = trimmed.strip_prefix("- ").map(str::trim) else {
                    continue;
                };
                if item.is_empty() {
                    continue;
                }
                match current_section {
                    Some("communication") => communication.push(item.to_string()),
                    Some("execution") => execution.push(item.to_string()),
                    Some("escalation") => escalation.push(item.to_string()),
                    _ => {}
                }
            }
        }
    }

    if communication.is_empty() && execution.is_empty() && escalation.is_empty() {
        None
    } else {
        Some(BehavioralContract {
            communication,
            execution,
            escalation,
        })
    }
}

fn parse_constraints_section(body: &str) -> (Option<RoleConstraints>, Option<CliTool>) {
    let mut constraints = default_import_constraints();
    let mut saw_field = false;
    let mut default_cli_tool = None;

    for line in body.lines() {
        let Some(item) = line.trim().strip_prefix("- ").map(str::trim) else {
            continue;
        };
        let Some((label, value)) = item.split_once(':') else {
            continue;
        };
        let label = label.trim();
        let value = value.trim();
        match label {
            "role kind" => saw_field = true,
            "min instances" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    constraints.min_instances = parsed;
                    saw_field = true;
                }
            }
            "max instances" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    constraints.max_instances = parsed;
                    saw_field = true;
                }
            }
            "allowed project binding" => {
                constraints.allowed_project_binding = match value {
                    "lead_project" => ProjectBinding::LeadProject,
                    "explicit_project" => ProjectBinding::ExplicitProject,
                    _ => ProjectBinding::Any,
                };
                saw_field = true;
            }
            // Canonical harness names only: the exporter
            // writes those, and internal aliases such as `mesh` must not start
            // parsing from hand-authored markdown.
            "required lead tool" => {
                constraints.requires_lead_tool = value.parse::<CliTool>().ok();
                saw_field = true;
            }
            "default cli tool" => {
                default_cli_tool = value.parse::<CliTool>().ok();
            }
            _ => {}
        }
    }

    (saw_field.then_some(constraints), default_cli_tool)
}

fn split_frontmatter_and_body(raw: &str) -> Result<(Option<String>, &str), RoleImportError> {
    let trimmed_start = raw.trim_start_matches('\u{feff}');
    if !trimmed_start.starts_with("---") {
        return Ok((None, trimmed_start));
    }

    let mut lines = trimmed_start.lines();
    let Some(first_line) = lines.next() else {
        return Ok((None, trimmed_start));
    };
    if first_line.trim() != "---" {
        return Ok((None, trimmed_start));
    }

    let mut frontmatter_lines = Vec::new();
    let mut body_start = first_line.len() + 1;
    let mut found_closing = false;

    for line in lines {
        body_start += line.len() + 1;
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        frontmatter_lines.push(line);
    }

    if !found_closing {
        return Err(RoleImportError::InvalidFrontmatter(
            "missing closing '---' delimiter".to_string(),
        ));
    }

    let frontmatter = frontmatter_lines.join("\n");
    let body = trimmed_start.get(body_start..).unwrap_or_default();
    Ok((Some(frontmatter), body))
}

fn default_import_behavioral_contract() -> crate::templates::types::BehavioralContract {
    crate::templates::types::BehavioralContract {
        communication: Vec::new(),
        execution: vec!["Follow the imported instructions faithfully.".to_string()],
        escalation: Vec::new(),
    }
}

fn default_import_constraints() -> crate::templates::types::RoleConstraints {
    crate::templates::types::RoleConstraints {
        min_instances: 0,
        max_instances: 4,
        requires_lead_tool: None,
        allowed_project_binding: ProjectBinding::Any,
    }
}

fn synthesized_fields_for_import(
    format: RoleExportFormat,
    parsed_body: &ParsedCompiledPromptBody,
) -> Vec<String> {
    let mut fields = Vec::new();
    push_missing_synthesized_field(
        &mut fields,
        "behavioral_contract",
        parsed_body.behavioral_contract.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "constraints",
        parsed_body.constraints.is_none(),
    );
    push_missing_synthesized_field(&mut fields, "focus_area", parsed_body.focus_area.is_none());
    push_missing_synthesized_field(
        &mut fields,
        "behavior_summary",
        parsed_body.behavior_summary.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "communication_style",
        parsed_body.communication_style.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "quality_gates",
        parsed_body.quality_gates.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "handoff_expectations",
        parsed_body.handoff_expectations.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "definition_of_done",
        parsed_body.definition_of_done.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "phase_scope",
        parsed_body.phase_scope.is_none(),
    );
    push_missing_synthesized_field(&mut fields, "mode", parsed_body.mode.is_none());
    push_missing_synthesized_field(
        &mut fields,
        "inherits_from",
        parsed_body.inherits_from.is_none(),
    );
    push_missing_synthesized_field(
        &mut fields,
        "required_artifacts",
        parsed_body.required_artifacts.is_none(),
    );
    match format {
        RoleExportFormat::Yaml => {}
        RoleExportFormat::ClaudeAgent => {
            if parsed_body.context_summary.is_none() {
                fields.push("context_summary".to_string());
            }
        }
        RoleExportFormat::CopilotAgent if parsed_body.capabilities.is_none() => {
            fields.push("capabilities".to_string());
        }
        _ => {}
    }
    fields
}

fn map_claude_tools_to_capabilities(tools: &[String]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| {
            let normalized = tool.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "read" => Some("read"),
                "edit" => Some("write"),
                "grep" => Some("search"),
                "bash" => Some("shell"),
                "git" => Some("git"),
                _ => None,
            }
        })
        .fold(Vec::<String>::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == item) {
                acc.push(item.to_string());
            }
            acc
        })
}

fn stem_from_source_path(path: &str) -> Option<&str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let file_name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let file_name = file_name.rsplit('\\').next().unwrap_or(file_name);
    file_name.strip_suffix(".md").or(Some(file_name))
}

fn dedupe_preserving_order(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .fold(Vec::<String>::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        })
}

fn slugify_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "imported-role".to_string()
    } else {
        trimmed.to_string()
    }
}

fn push_compiled_section_losses(role: &RoleTemplate, lossy: &mut Vec<String>) {
    if role.capability_policy.is_some() {
        lossy.push("capability_policy".to_string());
    }
    if role.focus_area.is_some() {
        lossy.push("focus_area".to_string());
    }
    if role.context_summary.is_some() {
        lossy.push("context_summary".to_string());
    }
    if role.behavior_summary.is_some() {
        lossy.push("behavior_summary".to_string());
    }
    if role.communication_style.is_some() {
        lossy.push("communication_style".to_string());
    }
    lossy.push("behavioral_contract".to_string());
    if role.quality_gates.is_some() {
        lossy.push("quality_gates".to_string());
    }
    if role.handoff_expectations.is_some() {
        lossy.push("handoff_expectations".to_string());
    }
    if role.definition_of_done.is_some() {
        lossy.push("definition_of_done".to_string());
    }
    if role.phase_scope.is_some() {
        lossy.push("phase_scope".to_string());
    }
    if role.mode.is_some() {
        lossy.push("mode".to_string());
    }
    if role.inherits_from.is_some() {
        lossy.push("inherits_from".to_string());
    }
    if role.required_artifacts.is_some() {
        lossy.push("required_artifacts".to_string());
    }
    lossy.push("constraints".to_string());
}

fn push_missing_synthesized_field(lossy: &mut Vec<String>, field: &str, missing: bool) {
    if missing {
        lossy.push(field.to_string());
    }
}

fn render_bulleted_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {}", item.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn map_capabilities_to_claude_tools(capabilities: &[String]) -> Vec<String> {
    capabilities
        .iter()
        .filter_map(|capability| {
            let normalized = capability.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "read" | "fs-read" | "code-reading" => Some("read"),
                "write" | "fs-write" | "code-writing" => Some("edit"),
                "search" | "grep" | "ripgrep" => Some("grep"),
                "shell" | "terminal" | "command-line" => Some("bash"),
                "git" => Some("git"),
                _ => None,
            }
        })
        .fold(Vec::<String>::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == item) {
                acc.push(item.to_string());
            }
            acc
        })
}

fn push_bullets(lines: &mut Vec<String>, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }

    lines.push(format!("### {label}"));
    lines.extend(items.iter().map(|item| format!("- {}", item.trim())));
}

fn render_role_kind(kind: RoleKind) -> &'static str {
    match kind {
        RoleKind::Lead => "lead",
        RoleKind::Agent => "agent",
    }
}

fn render_project_binding(binding: ProjectBinding) -> &'static str {
    match binding {
        ProjectBinding::LeadProject => "lead_project",
        ProjectBinding::ExplicitProject => "explicit_project",
        ProjectBinding::Any => "any",
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

/// One double-quoted YAML scalar. Role text is free-form, so every character
/// that would otherwise end the scalar or break block indentation is escaped.
pub(crate) fn yaml_scalar(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CapabilityTier;
    use crate::session_scanner::cli_tool::CliTool;
    use crate::templates::types::{
        BehavioralContract, ModelSelection, RoleConstraints, RoleDefaults, TemplateKind,
        TemplateSchema,
    };

    // Regression: e17f3eb (PR 15) parsed the constraint tools through
    // `CliTool::from_alias`, so hand-authored `mesh`/`Claude` values silently
    // became real constraints where they had parsed to nothing before.
    #[test]
    fn constraint_tools_parse_canonical_names_only() {
        let (constraints, default_cli_tool) =
            parse_constraints_section("- required lead tool: mesh\n- default cli tool: codex\n");
        assert_eq!(constraints.unwrap().requires_lead_tool, None);
        assert_eq!(default_cli_tool, Some(CliTool::Codex));

        let (constraints, default_cli_tool) = parse_constraints_section(
            "- required lead tool: codex\n- default cli tool: claude_native\n",
        );
        assert_eq!(
            constraints.unwrap().requires_lead_tool,
            Some(CliTool::Codex)
        );
        assert_eq!(default_cli_tool, None);
    }

    fn sample_role() -> RoleTemplate {
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
                cli_tool: CliTool::Claude,
                model: "claude-opus-4-6".to_string(),
                reasoning_effort: None,
                default_name_pattern: "worker-{n}".to_string(),
            },
            capability_policy: None,
            instructions: "Do the primary assignment first.".to_string(),
            focus_area: Some("Architecture review".to_string()),
            context_summary: Some("Remembers why the architecture looks this way.".to_string()),
            behavior_summary: Some("Escalates direction questions quickly.".to_string()),
            communication_style: Some("Calm, decisive check-ins.".to_string()),
            runtime_compact_summary: None,
            behavioral_contract: BehavioralContract {
                communication: vec!["Share interim findings.".to_string()],
                execution: vec!["Verify assumptions in code.".to_string()],
                escalation: vec!["Escalate blockers immediately.".to_string()],
            },
            quality_gates: Some(vec![
                "Run the named verification lane.".to_string(),
                "Keep regression coverage intact.".to_string(),
            ]),
            handoff_expectations: None,
            definition_of_done: Some(vec![
                "The requested outcome is visible in code or docs.".to_string(),
                "Residual risk is called out.".to_string(),
            ]),
            phase_scope: Some(vec![
                "implementation".to_string(),
                "verification".to_string(),
            ]),
            mode: Some("execution".to_string()),
            inherits_from: Some("taurhaus-base-worker".to_string()),
            required_artifacts: Some(vec![
                "diff summary".to_string(),
                "validation summary".to_string(),
            ]),
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

    #[test]
    fn role_export_format_serializes_with_snake_case() {
        assert_eq!(
            serde_json::to_string(&RoleExportFormat::Yaml).unwrap(),
            "\"yaml\""
        );
        assert_eq!(
            serde_json::to_string(&RoleExportFormat::ClaudeAgent).unwrap(),
            "\"claude_agent\""
        );
        assert_eq!(
            serde_json::to_string(&RoleExportFormat::CopilotAgent).unwrap(),
            "\"copilot_agent\""
        );
        assert_eq!(
            serde_json::to_string(&RoleExportFormat::AgentsMd).unwrap(),
            "\"agents_md\""
        );
        assert_eq!(
            serde_json::to_string(&RoleExportFormat::GeminiMd).unwrap(),
            "\"gemini_md\""
        );
    }

    #[test]
    fn compile_prompt_body_includes_taurhaus_only_sections() {
        let role = sample_role();
        let body = compile_prompt_body(&role);

        assert!(body.contains("Do the primary assignment first."));
        assert!(body.contains("## Focus Area"));
        assert!(body.contains("Architecture review"));
        assert!(body.contains("## Context Summary"));
        assert!(body.contains("## Behavior Summary"));
        assert!(body.contains("## Communication Style"));
        assert!(body.contains("## Behavioral Contract"));
        assert!(body.contains("## Quality Gates"));
        assert!(body.contains("## Definition of Done"));
        assert!(body.contains("## Phase Scope"));
        assert!(body.contains("## Mode"));
        assert!(body.contains("## Inherits From"));
        assert!(body.contains("## Required Artifacts"));
        assert!(body.contains("### Communication"));
        assert!(body.contains("### Execution"));
        assert!(body.contains("### Escalation"));
        assert!(body.contains("## Capabilities"));
        assert!(body.contains("## Constraints"));
        assert!(body.contains("- required lead tool: codex"));
    }

    #[test]
    fn export_role_to_claude_agent_maps_partial_tools_and_marks_lossy_fields() {
        let mut role = sample_role();
        role.defaults.reasoning_effort = Some("high".to_string());
        let exported = export_role(&role, RoleExportFormat::ClaudeAgent);

        assert_eq!(exported.target_format, RoleExportFormat::ClaudeAgent);
        assert!(exported.file_content.contains("name: \"Sample Role\""));
        assert!(exported.file_content.contains("model: \"claude-opus-4-6\""));
        assert!(exported.file_content.contains("tools: [read, edit, bash]"));
        assert_eq!(
            exported.lossy_fields,
            vec!["defaults.reasoning_effort".to_string()]
        );
    }

    #[test]
    fn export_role_to_instruction_only_marks_name_and_model_lossy() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::AgentsMd);

        assert!(exported.file_content.starts_with("# Sample Role"));
        assert!(exported
            .file_content
            .contains("Instruction-only export for AGENTS.md"));
        assert!(exported.file_content.contains("## Metadata"));
        assert!(exported
            .file_content
            .contains("- default model: claude-opus-4-6"));
        assert!(exported.file_content.contains("## Round-Trip Notes"));
        assert!(exported.lossy_fields.contains(&"name".to_string()));
        assert!(exported
            .lossy_fields
            .contains(&"defaults.model".to_string()));
    }

    #[test]
    fn export_role_to_yaml_round_trips_without_lossy_fields() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::Yaml);

        assert_eq!(exported.target_format, RoleExportFormat::Yaml);
        assert!(exported.lossy_fields.is_empty());

        let parsed = serde_norway::from_str::<RoleTemplate>(&exported.file_content)
            .expect("parse exported role yaml");
        assert_eq!(parsed.role_id, role.role_id);
        assert_eq!(parsed.name, role.name);
        assert_eq!(parsed.defaults.model, role.defaults.model);
    }

    #[test]
    fn export_role_to_copilot_agent_preserves_compiled_fields_without_known_loss() {
        let mut role = sample_role();
        role.defaults.reasoning_effort = Some("high".to_string());
        let exported = export_role(&role, RoleExportFormat::CopilotAgent);

        assert_eq!(exported.target_format, RoleExportFormat::CopilotAgent);
        assert!(exported.file_content.contains("name: \"Sample Role\""));
        assert!(exported
            .file_content
            .contains("description: \"Escalates direction questions quickly.\""));
        assert_eq!(
            exported.lossy_fields,
            vec!["defaults.reasoning_effort".to_string()]
        );

        let imported = import_role_at(
            RoleExportFormat::CopilotAgent,
            &exported.file_content,
            Some(".github/agents/sample-role.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 21, 22, 35, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("copilot import should succeed");

        assert_eq!(imported.template.instructions, role.instructions);
        assert_eq!(
            imported.template.communication_style,
            role.communication_style
        );
        assert_eq!(imported.template.quality_gates, role.quality_gates);
        assert_eq!(
            imported.template.definition_of_done,
            role.definition_of_done
        );
        assert_eq!(imported.template.phase_scope, role.phase_scope);
        assert_eq!(imported.template.mode, role.mode);
        assert_eq!(imported.template.inherits_from, role.inherits_from);
        assert_eq!(
            imported.template.required_artifacts,
            role.required_artifacts
        );
        assert_eq!(imported.template.capabilities, role.capabilities);
        assert_eq!(imported.template.constraints, role.constraints);
    }

    #[test]
    fn export_role_to_gemini_md_uses_gemini_specific_heading_and_loss_notes() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::GeminiMd);

        assert_eq!(exported.target_format, RoleExportFormat::GeminiMd);
        assert!(exported
            .file_content
            .contains("Instruction-only export for GEMINI.md"));
        assert!(exported.file_content.contains("## Core Instructions"));
        assert!(exported.file_content.contains("## Focus Area"));
        assert!(exported
            .file_content
            .contains("- non-round-trippable or downgraded fields:"));
        assert!(exported.lossy_fields.contains(&"constraints".to_string()));
    }

    #[test]
    fn instruction_only_exports_document_round_trip_loss_in_markdown() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::AgentsMd);

        assert!(exported
            .file_content
            .contains("This export does not preserve the full Taurhaus role schema."));
        assert!(exported.file_content.contains("  - name"));
        assert!(exported.file_content.contains("  - defaults.model"));
        assert!(exported.file_content.contains("  - capabilities"));
    }

    #[test]
    fn map_capabilities_to_claude_tools_deduplicates_and_filters_unknown_values() {
        let tools = map_capabilities_to_claude_tools(&[
            "read".to_string(),
            "fs-read".to_string(),
            "write".to_string(),
            "shell".to_string(),
            "shell".to_string(),
            "unknown".to_string(),
        ]);

        assert_eq!(tools, vec!["read", "edit", "bash"]);
    }

    #[test]
    fn provenance_round_trips_with_import_source() {
        let imported_at = Utc::now();
        let provenance = RoleProvenance {
            source_format: RoleExportFormat::CopilotAgent,
            source_version: Some("1".to_string()),
            source_path: Some(".github/agents/reviewer.md".to_string()),
            imported_at,
            non_roundtrippable_fields: vec!["constraints".to_string()],
        };
        let source = RoleImportSource {
            source_format: RoleExportFormat::CopilotAgent,
            parsed_fields: RoleParsedFields {
                name: Some("Reviewer".to_string()),
                model: Some("gpt-5".to_string()),
                description: Some("Reviews code".to_string()),
                tools: Vec::new(),
                prompt_body: Some("Review aggressively.".to_string()),
                capability_policy: None,
            },
            provenance,
        };

        let json = serde_json::to_string(&source).expect("serialize source");
        let round_trip: RoleImportSource = serde_json::from_str(&json).expect("deserialize source");
        assert_eq!(round_trip, source);
    }

    #[test]
    fn import_claude_agent_maps_frontmatter_body_and_provenance() {
        let raw = r#"---
name: Architecture Reviewer
model: claude-sonnet-4-6
tools:
  - read
  - bash
---
Review architecture changes and report structural risks.
"#;

        let imported = import_role_at(
            RoleExportFormat::ClaudeAgent,
            raw,
            Some(".claude/agents/architecture-reviewer.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 0, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("import should succeed");

        assert_eq!(imported.template.role_id, "architecture-reviewer");
        assert_eq!(imported.template.name, "Architecture Reviewer");
        assert_eq!(imported.template.defaults.cli_tool, CliTool::Claude);
        assert_eq!(imported.template.defaults.model, "claude-sonnet-4-6");
        assert_eq!(
            imported.template.instructions,
            "Review architecture changes and report structural risks."
        );
        assert_eq!(imported.template.capabilities, vec!["read", "shell"]);
        assert_eq!(
            imported.import_source.provenance.source_path.as_deref(),
            Some(".claude/agents/architecture-reviewer.md")
        );
        assert_eq!(
            imported.import_source.provenance.source_format,
            RoleExportFormat::ClaudeAgent
        );
    }

    #[test]
    fn import_claude_agent_parses_compiled_taurhaus_sections() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::ClaudeAgent);

        let imported = import_role_at(
            RoleExportFormat::ClaudeAgent,
            &exported.file_content,
            Some(".claude/agents/sample-role.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 21, 22, 30, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("import should succeed");

        assert_eq!(imported.template.instructions, role.instructions);
        assert_eq!(imported.template.focus_area, role.focus_area);
        assert_eq!(imported.template.context_summary, role.context_summary);
        assert_eq!(imported.template.behavior_summary, role.behavior_summary);
        assert_eq!(
            imported.template.communication_style,
            role.communication_style
        );
        assert_eq!(
            imported.template.behavioral_contract,
            role.behavioral_contract
        );
        assert_eq!(imported.template.quality_gates, role.quality_gates);
        assert_eq!(
            imported.template.definition_of_done,
            role.definition_of_done
        );
        assert_eq!(imported.template.phase_scope, role.phase_scope);
        assert_eq!(imported.template.mode, role.mode);
        assert_eq!(imported.template.inherits_from, role.inherits_from);
        assert_eq!(
            imported.template.required_artifacts,
            role.required_artifacts
        );
        assert_eq!(imported.template.capabilities, role.capabilities);
        assert_eq!(imported.template.constraints, role.constraints);
        assert_eq!(imported.template.defaults.cli_tool, role.defaults.cli_tool);
        assert_eq!(
            imported.import_source.provenance.non_roundtrippable_fields,
            vec!["handoff_expectations".to_string()]
        );
    }

    #[test]
    fn import_copilot_agent_maps_description_to_context_summary() {
        let raw = r#"---
name: Copilot Researcher
description: Gathers design context before implementation.
model: gpt-5
unknown_key: ignore-me
---
Investigate the relevant files before proposing a change.
"#;

        let imported = import_role_at(
            RoleExportFormat::CopilotAgent,
            raw,
            Some(".github/agents/copilot-researcher.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 5, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("import should succeed");

        assert_eq!(imported.template.name, "Copilot Researcher");
        assert_eq!(imported.template.role_id, "copilot-researcher");
        assert_eq!(imported.template.defaults.cli_tool, CliTool::Codex);
        assert_eq!(imported.template.defaults.model, "gpt-5");
        assert_eq!(
            imported.template.context_summary.as_deref(),
            Some("Gathers design context before implementation.")
        );
        assert_eq!(
            imported.template.instructions,
            "Investigate the relevant files before proposing a change."
        );
        assert!(imported.template.capabilities.is_empty());
        assert!(imported
            .import_source
            .provenance
            .non_roundtrippable_fields
            .contains(&"capabilities".to_string()));
        assert_eq!(
            imported
                .template
                .provenance
                .as_ref()
                .map(|provenance| provenance.source_format),
            Some(RoleExportFormat::CopilotAgent)
        );
    }

    #[test]
    fn import_without_frontmatter_uses_source_path_and_default_model() {
        let raw = "Follow the existing repository conventions exactly.";

        let imported = import_role_at(
            RoleExportFormat::ClaudeAgent,
            raw,
            Some(".claude/agents/no-frontmatter.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 10, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("import should succeed");

        assert_eq!(imported.template.role_id, "no-frontmatter");
        assert_eq!(imported.template.name, "no-frontmatter");
        assert_eq!(imported.template.defaults.model, "opus");
        assert_eq!(
            imported.template.instructions,
            "Follow the existing repository conventions exactly."
        );
    }

    #[test]
    fn import_rejects_empty_body_gracefully() {
        let raw = r#"---
name: Empty Agent
model: claude-opus-4-6
---
"#;

        let err = import_role_at(
            RoleExportFormat::ClaudeAgent,
            raw,
            Some(".claude/agents/empty-agent.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 15, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect_err("empty body should fail");

        assert_eq!(err, RoleImportError::EmptyBody);
    }

    #[test]
    fn import_rejects_unclosed_frontmatter_gracefully() {
        let raw = r#"---
name: Broken Agent
model: claude-opus-4-6
Review carefully."#;

        let err = import_role_at(
            RoleExportFormat::ClaudeAgent,
            raw,
            Some(".claude/agents/broken-agent.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 20, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect_err("unclosed frontmatter should fail");

        assert!(matches!(err, RoleImportError::InvalidFrontmatter(_)));
    }

    #[test]
    fn reexport_of_imported_role_surfaces_provenance_lossy_fields() {
        let raw = r#"---
name: Imported Claude Agent
model: claude-opus-4-6
---
Review carefully and summarize the tradeoffs.
"#;

        let imported = import_role_at(
            RoleExportFormat::ClaudeAgent,
            raw,
            Some(".claude/agents/imported-claude-agent.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 30, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("import should succeed");

        let exported = export_role(&imported.template, RoleExportFormat::CopilotAgent);
        assert!(exported
            .lossy_fields
            .contains(&"context_summary".to_string()));
        assert!(exported
            .lossy_fields
            .contains(&"behavioral_contract".to_string()));
    }

    #[test]
    fn export_then_import_round_trip_preserves_core_fields_and_tracks_provenance() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::ClaudeAgent);

        let imported = import_role_at(
            RoleExportFormat::ClaudeAgent,
            &exported.file_content,
            Some(".claude/agents/sample-role.md"),
            chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 8, 7, 35, 0)
                .single()
                .expect("valid timestamp"),
        )
        .expect("round-trip import should succeed");

        assert_eq!(imported.template.name, role.name);
        assert_eq!(imported.template.role_id, role.role_id);
        assert_eq!(imported.template.defaults.model, role.defaults.model);
        assert_eq!(imported.template.instructions, role.instructions);
        assert_eq!(
            imported.template.communication_style,
            role.communication_style
        );
        assert_eq!(imported.template.quality_gates, role.quality_gates);
        assert_eq!(
            imported.template.definition_of_done,
            role.definition_of_done
        );
        assert_eq!(imported.template.phase_scope, role.phase_scope);
        assert_eq!(imported.template.mode, role.mode);
        assert_eq!(imported.template.inherits_from, role.inherits_from);
        assert_eq!(
            imported.template.required_artifacts,
            role.required_artifacts
        );
        assert_eq!(
            imported
                .template
                .provenance
                .as_ref()
                .and_then(|provenance| provenance.source_path.as_deref()),
            Some(".claude/agents/sample-role.md")
        );
    }

    #[test]
    fn agent_adapters_round_trip_capability_policy_losslessly() {
        let mut role = sample_role();
        role.capability_policy = Some(CapabilityPolicy {
            model_selection: ModelSelection::Adaptive,
            minimum_capability: Some(CapabilityTier::Strong),
            allowed_models: vec!["gpt-5.6-sol".to_string(), "opus".to_string()],
            effort_band: vec!["medium".to_string(), "high".to_string()],
        });

        for format in [
            RoleExportFormat::ClaudeAgent,
            RoleExportFormat::CopilotAgent,
        ] {
            let exported = export_role(&role, format);
            assert!(!exported
                .lossy_fields
                .contains(&"capability_policy".to_string()));

            let imported = import_role_at(
                format,
                &exported.file_content,
                Some("agent.md"),
                chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 9, 4, 12, 0, 0)
                    .single()
                    .expect("valid timestamp"),
            )
            .expect("round-trip import should succeed");
            assert_eq!(imported.template.capability_policy, role.capability_policy);
            assert_eq!(
                imported.import_source.parsed_fields.capability_policy,
                role.capability_policy
            );
        }

        let instruction_only = export_role(&role, RoleExportFormat::AgentsMd);
        assert!(instruction_only
            .lossy_fields
            .contains(&"capability_policy".to_string()));
    }
}
