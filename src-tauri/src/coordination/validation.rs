use crate::coordination::errors::CoordinationError;

/// Validate the effective seat before any create/resume side effects.
/// Missing declarations use the existing hydration authority; explicit invalid
/// values and incoherent role/tool seats still require an actionable rejection.
pub(crate) fn validate_member_configuration(
    member: &crate::coordination::domain::Member,
    template_root: &std::path::Path,
) -> Result<(), CoordinationError> {
    let invalid = |field: &str, reason: String| {
        CoordinationError::Validation(format!(
            "member '{}' field '{field}': {reason}",
            member.name
        ))
    };
    if member
        .extra
        .get("adapter_mode")
        .and_then(serde_json::Value::as_str)
        == Some("app_server")
    {
        if !taurhaus_lib::session_scanner::launch::HostedLaunch::supports(member.cli_tool) {
            return Err(invalid(
                "delivery",
                format!(
                    "app_server_unsupported_harness: {} cannot host an app-server seat",
                    member.cli_tool
                ),
            ));
        }
        if !cfg!(target_os = "linux") {
            return Err(invalid(
                "delivery",
                "app_server_unsupported_platform: Owned hosting is Linux/WSL only".into(),
            ));
        }
    }
    if !member.project_path.is_dir() {
        return Err(invalid(
            "cwd",
            format!(
                "'{}' is not an existing directory; choose an existing checkout",
                member.project_path.display()
            ),
        ));
    }
    let role = member.role_id.as_deref().and_then(|role_id| {
        crate::coordination::member_activation::load_role_for_member_hydration(
            template_root,
            role_id,
            &member.name,
            "validation",
        )
    });
    if let Some(role) = role.as_ref() {
        if role.defaults.cli_tool != member.cli_tool {
            return Err(invalid(
                "cli_tool",
                format!(
                    "{} disagrees with role '{}' tool {}; select a matching role or tool",
                    member.cli_tool, role.role_id, role.defaults.cli_tool
                ),
            ));
        }
    }
    let mut resolved = member.clone();
    crate::coordination::member_activation::hydrate_member_model_fields(
        &mut resolved,
        role.as_ref(),
    );
    if !resolved.model.as_deref().is_some_and(|model| {
        !model.eq_ignore_ascii_case("external")
            && !model.trim().is_empty()
            && crate::coordination::member_activation::model_is_valid_for(member.cli_tool, model)
    }) {
        return Err(invalid(
            "model",
            format!(
                "choose a resolvable model for {}; 'external' is not a managed model",
                member.cli_tool,
            ),
        ));
    }
    Ok(())
}

/// Runtime additions must use the target config, never the builder's mode.
pub(crate) fn validate_member_configuration_for_team(
    member: &crate::coordination::domain::Member,
    template_root: &std::path::Path,
    config: &crate::coordination::stores::TeamConfig,
) -> Result<(), CoordinationError> {
    validate_member_configuration(member, template_root)?;
    if member.extra.get("adapter_mode").and_then(serde_json::Value::as_str) == Some("app_server")
        && config.extra.get("messaging_format") != Some(&serde_json::json!(2))
    {
        return Err(CoordinationError::Validation(format!(
            "member '{}' field 'delivery': app_server_requires_canonical_messaging: target team must use messaging_format 2",
            member.name
        )));
    }
    Ok(())
}

pub(crate) fn validate_team_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("team name", name)?;
    if has_path_separator(name) || is_reserved_path_component(name) {
        return Err(CoordinationError::Validation(format!(
            "team name '{name}' must not contain path separators or reserved path components"
        )));
    }
    Ok(())
}

pub(crate) fn validate_member_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("member name", name)?;
    if has_path_separator(name) || is_reserved_path_component(name) {
        return Err(CoordinationError::Validation(format!(
            "member name '{name}' must not contain path separators or reserved path components"
        )));
    }
    Ok(())
}

pub(crate) fn validate_non_empty(field: &str, value: &str) -> Result<(), CoordinationError> {
    if value.trim().is_empty() {
        return Err(CoordinationError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

pub(crate) fn has_path_separator(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

pub(crate) fn is_reserved_path_component(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "." || trimmed == ".."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_name_rejects_parent_component() {
        let err = validate_team_name("..").expect_err(".. should be rejected");
        assert!(format!("{err}").contains("reserved path components"));
    }

    #[test]
    fn member_name_rejects_current_component() {
        let err = validate_member_name(".").expect_err(". should be rejected");
        assert!(format!("{err}").contains("reserved path components"));
    }

    #[test]
    fn dotted_names_are_allowed() {
        validate_team_name("ledger.team").expect("dotted name should be valid");
        validate_member_name("codex.reviewer_1").expect("dotted member should be valid");
    }
}
