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
}
