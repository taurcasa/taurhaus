use crate::coordination::errors::CoordinationError;

pub(crate) fn validate_team_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("team name", name)?;
    if has_path_separator(name) {
        return Err(CoordinationError::Validation(format!(
            "team name '{name}' must not contain path separators"
        )));
    }
    Ok(())
}

pub(crate) fn validate_member_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("member name", name)?;
    if has_path_separator(name) {
        return Err(CoordinationError::Validation(format!(
            "member name '{name}' must not contain path separators"
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
