use super::*;

impl TemplateStore {
    pub fn list_presets(&self) -> Result<Vec<TeamPresetRecord>, TemplateStoreError> {
        self.ensure_directories()?;
        let role_catalog = self.load_role_catalog()?;

        let mut merged: BTreeMap<String, TeamPresetRecord> = BTreeMap::new();
        for preset_file in
            self.load_preset_files_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))?
        {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                preset_file.template.preset_id.clone(),
                TeamPresetRecord {
                    template: preset_file.template,
                    source: TemplateSource::BuiltIn,
                    read_only: true,
                },
            );
        }

        for preset_file in self.load_preset_files_from_dir(&self.presets_dir())? {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            merged.insert(
                preset_file.template.preset_id.clone(),
                TeamPresetRecord {
                    template: preset_file.template,
                    source: TemplateSource::User,
                    read_only: false,
                },
            );
        }

        Ok(merged.into_values().collect())
    }

    pub fn get_preset(&self, preset_id: &str) -> Result<TeamPresetRecord, TemplateStoreError> {
        self.ensure_directories()?;
        let role_catalog = self.load_role_catalog()?;

        if let Some(preset_file) = self.load_preset_file_by_id(&self.presets_dir(), preset_id)? {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            return Ok(TeamPresetRecord {
                template: preset_file.template,
                source: TemplateSource::User,
                read_only: false,
            });
        }

        if let Some(preset_file) =
            self.load_preset_file_by_id(&self.builtins_dir.join(PRESETS_DIRNAME), preset_id)?
        {
            preset_file
                .template
                .validate_with_role_catalog(&role_catalog)
                .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;
            return Ok(TeamPresetRecord {
                template: preset_file.template,
                source: TemplateSource::BuiltIn,
                read_only: true,
            });
        }

        Err(TemplateStoreError::NotFound(format!(
            "preset '{preset_id}' not found"
        )))
    }

    pub fn create_preset(
        &self,
        template: &TeamPreset,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(&template.preset_id, "preset")?;
        let roles = self.load_role_catalog()?;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_preset(&template.preset_id) {
            Ok(record) => {
                return match record.source {
                    TemplateSource::BuiltIn => Err(TemplateStoreError::ReadOnly(format!(
                        "preset '{}' is built-in and cannot be created over",
                        template.preset_id
                    ))),
                    TemplateSource::User => Err(TemplateStoreError::AlreadyExists(format!(
                        "preset '{}' already exists",
                        template.preset_id
                    ))),
                };
            }
            Err(TemplateStoreError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }

        let relative_path = self.preset_file_path(&template.preset_id);
        let payload = serde_yml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to serialize preset '{}': {err}",
                template.preset_id
            ))
        })?;
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(
                relative_path,
                payload.into_bytes(),
            )],
            &format!("templates: create preset {}", template.preset_id),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn update_preset(
        &self,
        preset_id: &str,
        template: &TeamPreset,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        if template.preset_id != preset_id {
            return Err(TemplateStoreError::Validation(format!(
                "preset_id mismatch: expected '{preset_id}', got '{}'",
                template.preset_id
            )));
        }
        validate_template_id(preset_id, "preset")?;

        let roles = self.load_role_catalog()?;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        match self.get_preset(preset_id) {
            Ok(_) => {}
            Err(TemplateStoreError::NotFound(_)) => {
                return Err(TemplateStoreError::NotFound(format!(
                    "preset '{preset_id}' not found"
                )));
            }
            Err(err) => return Err(err),
        }

        // Updating built-ins creates/updates user override.
        let relative_path = self.preset_file_path(preset_id);
        let payload = serde_yml::to_string(template).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to serialize preset '{preset_id}': {err}"))
        })?;
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(
                relative_path,
                payload.into_bytes(),
            )],
            &format!("templates: update preset {preset_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn delete_preset(
        &self,
        preset_id: &str,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        validate_template_id(preset_id, "preset")?;
        self.ensure_directories()?;

        let record = self.get_preset(preset_id)?;
        if record.read_only {
            return Err(TemplateStoreError::ReadOnly(format!(
                "preset '{preset_id}' is built-in and cannot be deleted"
            )));
        }

        let relative_path = self.preset_file_path(preset_id);
        let absolute_path = self.templates_dir().join(&relative_path);
        if !absolute_path.exists() {
            return Err(TemplateStoreError::NotFound(format!(
                "preset file missing for '{preset_id}'"
            )));
        }

        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::delete(relative_path)],
            &format!("templates: delete preset {preset_id}"),
        )?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    pub fn import_preset(
        &self,
        external_path: &Path,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let raw = fs::read_to_string(external_path)?;
        let template = serde_yml::from_str::<TeamPreset>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!(
                "failed to parse external preset {}: {err}",
                external_path.display()
            ))
        })?;

        validate_template_id(&template.preset_id, "preset")?;
        let roles = self.load_role_catalog()?;
        template
            .validate_with_role_catalog(&roles)
            .map_err(|err| TemplateStoreError::Validation(err.to_string()))?;

        let preset_id = template.preset_id.clone();
        let action = match self.get_preset(&preset_id) {
            Ok(_) => "update",
            Err(TemplateStoreError::NotFound(_)) => "create",
            Err(err) => return Err(err),
        };

        let relative_path = self.preset_file_path(&preset_id);
        let commit_id = self.mutate_and_commit(
            &[TemplateFileMutation::write(relative_path, raw.into_bytes())],
            &format!("templates: {action} preset {preset_id}"),
        )?;

        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }
}
