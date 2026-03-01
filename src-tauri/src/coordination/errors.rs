//! Canonical coordination-layer errors.

use crate::errors::AppError;

/// Error categories exposed by coordination backends and orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Store error: {0}")]
    StoreError(String),
}

impl From<std::io::Error> for CoordinationError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<AppError> for CoordinationError {
    fn from(value: AppError) -> Self {
        match value {
            AppError::NotFound(message) => Self::NotFound(message),
            AppError::AlreadyExists(message) => Self::Conflict(message),
            AppError::InvalidPath(message) | AppError::ParseError(message) => {
                Self::Validation(message)
            }
            AppError::Database(err) => Self::StoreError(err.to_string()),
            AppError::Io(err) => Self::Io(err),
            AppError::Git(err) => Self::Backend(err.to_string()),
            AppError::SearchError(message) => Self::Backend(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_maps_to_canonical_variant() {
        let mapped = CoordinationError::from(std::io::Error::other("disk full"));
        match mapped {
            CoordinationError::Io(err) => assert_eq!(err.kind(), std::io::ErrorKind::Other),
            other => panic!("expected IO variant, got {other:?}"),
        }
    }

    #[test]
    fn app_error_maps_to_conflict_variant() {
        let mapped = CoordinationError::from(AppError::AlreadyExists("team exists".to_string()));
        match mapped {
            CoordinationError::Conflict(message) => assert_eq!(message, "team exists"),
            other => panic!("expected conflict variant, got {other:?}"),
        }
    }

    #[test]
    fn app_database_error_maps_to_store_error_variant() {
        let db_err = rusqlite::Error::InvalidPath("bad.db".into());
        let mapped = CoordinationError::from(AppError::Database(db_err));
        match mapped {
            CoordinationError::StoreError(message) => {
                assert!(message.contains("bad.db"));
            }
            other => panic!("expected store error variant, got {other:?}"),
        }
    }

    #[test]
    fn all_variants_have_expected_display_strings() {
        let validation = CoordinationError::Validation("bad input".to_string());
        let io = CoordinationError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        let backend = CoordinationError::Backend("mesh down".to_string());
        let not_found = CoordinationError::NotFound("missing".to_string());
        let conflict = CoordinationError::Conflict("duplicate".to_string());
        let store = CoordinationError::StoreError("db".to_string());

        assert_eq!(validation.to_string(), "Validation error: bad input");
        assert!(io.to_string().starts_with("IO error: "));
        assert_eq!(backend.to_string(), "Backend error: mesh down");
        assert_eq!(not_found.to_string(), "Not found: missing");
        assert_eq!(conflict.to_string(), "Conflict: duplicate");
        assert_eq!(store.to_string(), "Store error: db");
    }

    #[test]
    fn app_error_mapping_covers_all_variants() {
        match CoordinationError::from(AppError::NotFound("x".to_string())) {
            CoordinationError::NotFound(msg) => assert_eq!(msg, "x"),
            other => panic!("unexpected mapping: {other:?}"),
        }
        match CoordinationError::from(AppError::InvalidPath("bad".to_string())) {
            CoordinationError::Validation(msg) => assert_eq!(msg, "bad"),
            other => panic!("unexpected mapping: {other:?}"),
        }
        match CoordinationError::from(AppError::ParseError("bad".to_string())) {
            CoordinationError::Validation(msg) => assert_eq!(msg, "bad"),
            other => panic!("unexpected mapping: {other:?}"),
        }
        match CoordinationError::from(AppError::Io(std::io::Error::other("boom"))) {
            CoordinationError::Io(err) => assert_eq!(err.kind(), std::io::ErrorKind::Other),
            other => panic!("unexpected mapping: {other:?}"),
        }
        match CoordinationError::from(AppError::Git(git2::Error::from_str("git failed"))) {
            CoordinationError::Backend(msg) => assert!(msg.contains("git failed")),
            other => panic!("unexpected mapping: {other:?}"),
        }
        match CoordinationError::from(AppError::SearchError("search failed".to_string())) {
            CoordinationError::Backend(msg) => assert_eq!(msg, "search failed"),
            other => panic!("unexpected mapping: {other:?}"),
        }
    }
}
