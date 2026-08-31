use super::*;
use crate::templates::adapters::{
    import_role as import_external_role, RoleExportFormat, RoleImportError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRoleImport {
    pub template: RoleTemplate,
    pub import_format: Option<RoleExportFormat>,
}

impl TemplateStore {
    pub fn list_roles(&self) -> Result<Vec<RoleTemplateRecord>, TemplateStoreError> {
        self.reconcile_builtins_if_needed()?;
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
        self.reconcile_builtins_if_needed()?;
        self.ensure_directories()?;

        if let Some(role_file) = self.load_role_file_by_id(&self.roles_dir(), role_id)? {
            role_file
                .template
                .validate()
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            return Ok(RoleTemplateRecord {
                template: role_file.template,
                source: TemplateSource::User,
                read_only: false,
            });
        }

        if let Some(role_file) =
            self.load_role_file_by_id(&self.builtins_dir.join(ROLES_DIRNAME), role_id)?
        {
            role_file
                .template
                .validate()
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            return Ok(RoleTemplateRecord {
                template: role_file.template,
                source: TemplateSource::BuiltIn,
                read_only: true,
            });
        }

        Err(TemplateStoreError::NotFound(format!(
            "role '{role_id}' not found"
        )))
    }

    pub fn create_role(
        &self,
        template: &RoleTemplate,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(&template.role_id, "role")?;
        let mut canonical = template.clone();
        canonical.normalize_model_fields();
        canonical
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
        let payload = serde_norway::to_string(&canonical).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to serialize role '{}': {err}",
                template.role_id
            ))
        })?;
        self.apply_single_template_mutation(
            TemplateFileMutation::write(relative_path, payload.into_bytes()),
            &format!("templates: create role {}", template.role_id),
        )
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
        let mut canonical = template.clone();
        canonical.normalize_model_fields();
        canonical
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
        let payload = serde_norway::to_string(&canonical).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize role '{role_id}': {err}"))
        })?;
        self.apply_single_template_mutation(
            TemplateFileMutation::write(relative_path, payload.into_bytes()),
            &format!("templates: update role {role_id}"),
        )
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

        self.apply_single_template_mutation(
            TemplateFileMutation::delete(relative_path),
            &format!("templates: delete role {role_id}"),
        )
    }

    pub fn prepare_role_import(
        &self,
        external_path: &Path,
    ) -> Result<PreparedRoleImport, TemplateStoreError> {
        let raw = fs::read_to_string(external_path)?;
        let import_format = infer_role_import_format(external_path, &raw)?;
        let mut template = match import_format {
            None => serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!(
                    "failed to parse external role {}: {err}",
                    external_path.display()
                ))
            })?,
            Some(format) => {
                import_external_role(format, &raw, Some(external_path.to_string_lossy().as_ref()))
                    .map(|imported| imported.template)
                    .map_err(map_role_import_error)?
            }
        };
        template.normalize_model_fields();
        template
            .validate()
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        Ok(PreparedRoleImport {
            template,
            import_format,
        })
    }

    pub fn save_prepared_role_import(
        &self,
        prepared: &PreparedRoleImport,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let mut template = prepared.template.clone();
        template.normalize_model_fields();
        let role_id = template.role_id.clone();
        validate_template_id(&role_id, "role")?;
        if let Ok(existing) = self.get_role(&role_id) {
            return Err(TemplateStoreError::Conflict(format!(
                "role '{}' already exists as a {:?} template; choose merge, replace, or skip",
                role_id, existing.source
            )));
        }

        let relative_path = self.role_file_path(&role_id);
        let payload = serde_norway::to_string(&template).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to serialize imported role '{}': {err}",
                role_id
            ))
        })?;
        let action = match prepared.import_format {
            Some(format) => format!(
                "templates: import role {} from {}",
                role_id,
                render_role_import_format(format)
            ),
            None => format!("templates: import role {role_id}"),
        };
        self.apply_single_template_mutation(
            TemplateFileMutation::write(relative_path, payload.into_bytes()),
            &action,
        )
    }

    pub fn import_role(
        &self,
        external_path: &Path,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let prepared = self.prepare_role_import(external_path)?;
        self.save_prepared_role_import(&prepared)
    }
}

fn infer_role_import_format(
    external_path: &Path,
    raw: &str,
) -> Result<Option<RoleExportFormat>, TemplateStoreError> {
    match external_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("yaml") | Some("yml") => Ok(None),
        Some("md") => Ok(Some(infer_markdown_import_format(external_path, raw))),
        Some(other) => Err(TemplateStoreError::Validation(format!(
            "unsupported role import extension '.{}' for {}",
            other,
            external_path.display()
        ))),
        None => Err(TemplateStoreError::Validation(format!(
            "cannot infer role import format for {}",
            external_path.display()
        ))),
    }
}

fn infer_markdown_import_format(external_path: &Path, raw: &str) -> RoleExportFormat {
    let normalized_path = external_path.to_string_lossy().replace('\\', "/");
    if normalized_path.contains("/.github/agents/") {
        return RoleExportFormat::CopilotAgent;
    }
    if normalized_path.contains("/.claude/agents/") {
        return RoleExportFormat::ClaudeAgent;
    }

    let frontmatter = raw
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .unwrap_or_default();
    if frontmatter
        .lines()
        .any(|line| line.trim_start().starts_with("description:"))
    {
        RoleExportFormat::CopilotAgent
    } else {
        RoleExportFormat::ClaudeAgent
    }
}

fn render_role_import_format(format: RoleExportFormat) -> &'static str {
    match format {
        RoleExportFormat::Yaml => "yaml",
        RoleExportFormat::ClaudeAgent => "claude_agent",
        RoleExportFormat::CopilotAgent => "copilot_agent",
        RoleExportFormat::AgentsMd => "agents_md",
        RoleExportFormat::GeminiMd => "gemini_md",
    }
}

fn map_role_import_error(err: RoleImportError) -> TemplateStoreError {
    match err {
        RoleImportError::UnsupportedFormat(_) => TemplateStoreError::Validation(err.to_string()),
        RoleImportError::InvalidFrontmatter(_) | RoleImportError::EmptyBody => {
            TemplateStoreError::Parse(err.to_string())
        }
    }
}
