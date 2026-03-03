use crate::coordination::errors::CoordinationError;

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
