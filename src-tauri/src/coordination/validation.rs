use crate::coordination::errors::CoordinationError;

/// Validate persisted seat identity before any create/resume side effects.
/// Do not hydrate invalid values: F2/F13 require an actionable rejection.
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
    if !member.project_path.is_dir() {
        return Err(invalid(
            "cwd",
            format!(
                "'{}' is not an existing directory; choose an existing checkout",
                member.project_path.display()
            ),
        ));
    }
    let declared = crate::session_scanner::launch::ModelSpec::parse_legacy(
        member.model.as_deref().unwrap_or_default(),
    );
    if !declared.model.as_deref().is_some_and(|model| {
        crate::models::ModelCatalog::entry_for(member.cli_tool, model).is_some()
    }) {
        return Err(invalid(
            "model",
            format!(
                "choose a non-empty model from the {} model catalog",
                member.cli_tool
            ),
        ));
    }
    if let Some(role) = member.role_id.as_deref().and_then(|role_id| {
        crate::coordination::member_activation::load_role_for_member_hydration(
            template_root,
            role_id,
            &member.name,
            "validation",
        )
    }) {
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
