use super::*;

impl TemplateStore {
    pub fn list_roles(&self) -> Result<Vec<RoleTemplateRecord>, TemplateStoreError> {
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
        template
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
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to serialize role '{}': {err}",
                template.role_id
            ))
        })?;
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(
                relative_path,
                payload.into_bytes(),
            )],
            &format!("templates: create role {}", template.role_id),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
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
        template
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
        let payload = serde_yaml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize role '{role_id}': {err}"))
        })?;
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(
                relative_path,
                payload.into_bytes(),
            )],
            &format!("templates: update role {role_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
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

        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::delete(relative_path)],
            &format!("templates: delete role {role_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn import_role(
        &self,
        external_path: &Path,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let raw = fs::read_to_string(external_path)?;
        let template = serde_yaml::from_str::<RoleTemplate>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to parse external role {}: {err}",
                external_path.display()
            ))
        })?;
        template
            .validate()
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        let role_id = template.role_id.clone();
        validate_template_id(&role_id, "role")?;
        let action = match self.get_role(&role_id) {
            Ok(_) => "update",
            Err(TemplateStoreError::NotFound(_)) => "create",
            Err(err) => return Err(err),
        };

        let relative_path = self.role_file_path(&role_id);
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(relative_path, raw.into_bytes())],
            &format!("templates: {action} role {role_id}"),
        )?;

        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }
}
