use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::session_scanner::cli_tool::CliTool;
use crate::templates::types::{ProjectBinding, RoleKind, RoleTemplate};

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
        import_fidelity: "Lossy unless Taurhaus-specific headings are re-imported",
    },
    RoleFieldMappingRow {
        taurhaus_field: "context_summary",
        claude_agent: "Compiled 'Context Summary' section in body",
        copilot_agent: "Compiled 'Context Summary' section in body",
        instruction_only: "Compiled 'Context Summary' section in body",
        export_mapping: "Rendered into prompt appendix section",
        import_fidelity: "Lossy unless Taurhaus-specific headings are re-imported",
    },
    RoleFieldMappingRow {
        taurhaus_field: "behavior_summary",
        claude_agent: "Compiled 'Behavior Summary' section in body",
        copilot_agent: "frontmatter.description + compiled body section",
        instruction_only: "Compiled 'Behavior Summary' section in body",
        export_mapping: "Rendered into prompt appendix section; Copilot may also mirror it into description",
        import_fidelity: "Lossy/partial",
    },
    RoleFieldMappingRow {
        taurhaus_field: "behavioral_contract",
        claude_agent: "Compiled 'Behavioral Contract' section in body",
        copilot_agent: "Compiled 'Behavioral Contract' section in body",
        instruction_only: "Compiled 'Behavioral Contract' section in body",
        export_mapping: "Rendered as grouped bullet lists in prompt appendix",
        import_fidelity: "Lossy unless Taurhaus-specific headings are re-imported",
    },
    RoleFieldMappingRow {
        taurhaus_field: "capabilities",
        claude_agent: "frontmatter.tools (partial)",
        copilot_agent: "No direct field",
        instruction_only: "Compiled 'Capabilities' section in body",
        export_mapping: "Mapped to Claude tools where possible; otherwise body section only",
        import_fidelity: "Partial for Claude, lossy elsewhere",
    },
    RoleFieldMappingRow {
        taurhaus_field: "constraints",
        claude_agent: "Compiled 'Constraints' section in body",
        copilot_agent: "Compiled 'Constraints' section in body",
        instruction_only: "Compiled 'Constraints' section in body",
        export_mapping: "Rendered into prompt appendix section only",
        import_fidelity: "Lossy",
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
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct CopilotAgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    model: Option<String>,
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
    cli_tool: CliTool,
) -> Result<ImportedRoleTemplate, RoleImportError> {
    let instructions = body.trim();
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
    let capabilities = match format {
        RoleExportFormat::ClaudeAgent => map_claude_tools_to_capabilities(&imported_tools),
        RoleExportFormat::CopilotAgent => Vec::new(),
        _ => Vec::new(),
    };
    let context_summary = imported_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let parsed_fields = RoleParsedFields {
        name: imported_name,
        model: imported_model.clone(),
        description: imported_description,
        tools: imported_tools,
        prompt_body: Some(instructions.to_string()),
    };
    let provenance = RoleProvenance {
        source_format: format,
        source_version: Some("1".to_string()),
        source_path: source_path.map(str::to_string),
        imported_at,
        non_roundtrippable_fields: synthesized_fields_for_import(format),
    };

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
                cli_tool,
                model: imported_model
                    .unwrap_or_else(|| default_model_for_tool(cli_tool).to_string()),
                default_name_pattern: format!("{role_id}-{{n}}"),
            },
            instructions: instructions.to_string(),
            focus_area: None,
            context_summary,
            behavior_summary: None,
            runtime_compact_summary: None,
            behavioral_contract: default_import_behavioral_contract(),
            capabilities,
            provenance: Some(provenance.clone()),
            constraints: default_import_constraints(),
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

    match format {
        RoleExportFormat::Yaml => {}
        RoleExportFormat::ClaudeAgent => {
            if !role.capabilities.is_empty() {
                lossy.push("capabilities".to_string());
            }
            push_compiled_section_losses(role, &mut lossy);
        }
        RoleExportFormat::CopilotAgent => {
            if !role.capabilities.is_empty() {
                lossy.push("capabilities".to_string());
            }
            push_compiled_section_losses(role, &mut lossy);
        }
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
    frontmatter.push("---".to_string());

    format!("{}\n\n{}", frontmatter.join("\n"), body)
}

fn render_copilot_agent(role: &RoleTemplate, body: &str) -> String {
    let description = role
        .behavior_summary
        .as_deref()
        .or(role.context_summary.as_deref())
        .unwrap_or("Exported from Taurhaus role template.");
    let frontmatter = [
        "---".to_string(),
        format!("name: {}", yaml_scalar(&role.name)),
        format!("description: {}", yaml_scalar(description)),
        format!("model: {}", yaml_scalar(&role.defaults.model)),
        "---".to_string(),
    ];

    format!("{}\n\n{}", frontmatter.join("\n"), body)
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

    if !role.capabilities.is_empty() {
        sections.push(PromptSection {
            heading: "Capabilities",
            body: role
                .capabilities
                .iter()
                .map(|item| format!("- {}", item.trim()))
                .collect::<Vec<_>>()
                .join("\n"),
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

fn default_model_for_tool(tool: CliTool) -> &'static str {
    match tool {
        CliTool::Claude => "claude-opus-4-6",
        CliTool::Codex => "gpt-5.4 high",
        CliTool::Gemini => "gemini-3.1-pro",
    }
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

fn synthesized_fields_for_import(format: RoleExportFormat) -> Vec<String> {
    let mut fields = vec![
        "behavioral_contract".to_string(),
        "constraints".to_string(),
        "focus_area".to_string(),
        "behavior_summary".to_string(),
    ];
    match format {
        RoleExportFormat::Yaml => {}
        RoleExportFormat::ClaudeAgent => {
            fields.push("context_summary".to_string());
        }
        RoleExportFormat::CopilotAgent => {
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
    if role.focus_area.is_some() {
        lossy.push("focus_area".to_string());
    }
    if role.context_summary.is_some() {
        lossy.push("context_summary".to_string());
    }
    if role.behavior_summary.is_some() {
        lossy.push("behavior_summary".to_string());
    }
    lossy.push("behavioral_contract".to_string());
    lossy.push("constraints".to_string());
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

fn yaml_scalar(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::cli_tool::CliTool;
    use crate::templates::types::{
        BehavioralContract, RoleConstraints, RoleDefaults, TemplateKind, TemplateSchema,
    };

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
                default_name_pattern: "worker-{n}".to_string(),
            },
            instructions: "Do the primary assignment first.".to_string(),
            focus_area: Some("Architecture review".to_string()),
            context_summary: Some("Remembers why the architecture looks this way.".to_string()),
            behavior_summary: Some("Escalates direction questions quickly.".to_string()),
            runtime_compact_summary: None,
            behavioral_contract: BehavioralContract {
                communication: vec!["Share interim findings.".to_string()],
                execution: vec!["Verify assumptions in code.".to_string()],
                escalation: vec!["Escalate blockers immediately.".to_string()],
            },
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
        assert!(body.contains("## Behavioral Contract"));
        assert!(body.contains("### Communication"));
        assert!(body.contains("### Execution"));
        assert!(body.contains("### Escalation"));
        assert!(body.contains("## Capabilities"));
        assert!(body.contains("## Constraints"));
        assert!(body.contains("- required lead tool: codex"));
    }

    #[test]
    fn export_role_to_claude_agent_maps_partial_tools_and_marks_lossy_fields() {
        let role = sample_role();
        let exported = export_role(&role, RoleExportFormat::ClaudeAgent);

        assert_eq!(exported.target_format, RoleExportFormat::ClaudeAgent);
        assert!(exported.file_content.contains("name: \"Sample Role\""));
        assert!(exported.file_content.contains("model: \"claude-opus-4-6\""));
        assert!(exported.file_content.contains("tools: [read, edit, bash]"));
        assert!(exported.lossy_fields.contains(&"capabilities".to_string()));
        assert!(exported
            .lossy_fields
            .contains(&"behavioral_contract".to_string()));
        assert!(exported.lossy_fields.contains(&"constraints".to_string()));
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
        assert_eq!(imported.template.defaults.model, "claude-opus-4-6");
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
        assert_eq!(imported.template.instructions, compile_prompt_body(&role));
        assert_eq!(
            imported
                .template
                .provenance
                .as_ref()
                .and_then(|provenance| provenance.source_path.as_deref()),
            Some(".claude/agents/sample-role.md")
        );
    }
}
