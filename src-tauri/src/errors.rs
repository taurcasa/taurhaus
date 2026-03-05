use serde::{Deserialize, Serialize};

/// Application-level error type used across all service operations.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Daemon transport error: {0}")]
    DaemonTransport(String),

    #[error("Daemon protocol error: {0}")]
    DaemonProtocol(String),

    #[error("Search error: {0}")]
    SearchError(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    ValidationError,
    NotFound,
    Conflict,
    Unavailable,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl IpcError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::ValidationError,
            message: sanitize_error(&message.into()),
            retryable: false,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::NotFound,
            message: sanitize_error(&message.into()),
            retryable: false,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::Conflict,
            message: sanitize_error(&message.into()),
            retryable: false,
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::Unavailable,
            message: sanitize_error(&message.into()),
            retryable: true,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::InternalError,
            message: sanitize_error(&message.into()),
            retryable: false,
        }
    }
}

pub type IpcResult<T> = Result<T, IpcError>;

pub fn map_command_error<E: std::fmt::Display>(err: E) -> IpcError {
    let message = sanitize_error(&err.to_string());
    let lower = message.to_ascii_lowercase();

    if lower.contains("not found") {
        return IpcError::not_found(message);
    }
    if lower.contains("already exists") || lower.contains("duplicate") || lower.contains("conflict")
    {
        return IpcError::conflict(message);
    }
    if lower.contains("invalid")
        || lower.contains("validation")
        || lower.contains("must not be empty")
        || lower.contains("unsupported")
    {
        return IpcError::validation(message);
    }
    if lower.contains("unavailable")
        || lower.contains("timeout")
        || lower.contains("temporar")
        || lower.contains("daemon unreachable")
    {
        return IpcError::unavailable(message);
    }

    IpcError::internal(message)
}

pub trait CommandResultExt<T> {
    fn ipc(self) -> IpcResult<T>;
}

impl<T, E: std::fmt::Display> CommandResultExt<T> for Result<T, E> {
    fn ipc(self) -> IpcResult<T> {
        self.map_err(map_command_error)
    }
}

impl From<String> for IpcError {
    fn from(value: String) -> Self {
        map_command_error(value)
    }
}

impl From<&str> for IpcError {
    fn from(value: &str) -> Self {
        map_command_error(value)
    }
}

impl From<AppError> for IpcError {
    fn from(value: AppError) -> Self {
        match value {
            AppError::NotFound(msg) => Self::not_found(msg),
            AppError::AlreadyExists(msg) => Self::conflict(msg),
            AppError::InvalidPath(msg) | AppError::ParseError(msg) => Self::validation(msg),
            AppError::DaemonTransport(msg)
            | AppError::DaemonProtocol(msg)
            | AppError::SearchError(msg) => Self::unavailable(msg),
            AppError::Database(err) => Self::internal(err.to_string()),
            AppError::Git(err) => Self::internal(err.to_string()),
            AppError::Io(err) => Self::internal(err.to_string()),
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
impl From<crate::coordination::errors::CoordinationError> for IpcError {
    fn from(value: crate::coordination::errors::CoordinationError) -> Self {
        use crate::coordination::errors::CoordinationError;
        match value {
            CoordinationError::Validation(msg) => Self::validation(msg),
            CoordinationError::NotFound(msg) => Self::not_found(msg),
            CoordinationError::Conflict(msg) => Self::conflict(msg),
            CoordinationError::Backend(msg) => Self::unavailable(msg),
            CoordinationError::StoreError(msg) => Self::internal(msg),
            CoordinationError::Io(err) => Self::internal(err.to_string()),
        }
    }
}

/// Replace the user's home directory path with `~` in error messages
/// to avoid leaking filesystem structure to the frontend.
pub fn sanitize_error(msg: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        msg.replace(home.to_string_lossy().as_ref(), "~")
    } else {
        msg.to_string()
    }
}

/// Extension trait for `Result` to sanitize error messages for IPC responses.
/// Replaces absolute home directory paths with `~` before returning to frontend.
pub trait SanitizeErr<T> {
    fn sanitize_err(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> SanitizeErr<T> for Result<T, E> {
    fn sanitize_err(self) -> Result<T, String> {
        self.map_err(|e| sanitize_error(&e.to_string()))
    }
}

/// Serializable form for IPC responses.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_error_replaces_home_dir() {
        let home = dirs::home_dir().expect("home dir must exist for test");
        let home_str = home.to_string_lossy();
        let msg = format!("Failed to read {home_str}/projects/secret/file.rs");
        let sanitized = sanitize_error(&msg);
        assert!(sanitized.starts_with("Failed to read ~/projects/secret/file.rs"));
        assert!(!sanitized.contains(&*home_str));
    }

    #[test]
    fn sanitize_error_passes_through_without_home() {
        let msg = "Some error without any path";
        assert_eq!(sanitize_error(msg), msg);
    }

    #[test]
    fn ipc_error_serializes_with_envelope_shape() {
        let err = IpcError::validation("bad input");
        let value = serde_json::to_value(err).expect("serialize ipc error");
        assert_eq!(value["code"], "VALIDATION_ERROR");
        assert_eq!(value["message"], "bad input");
        assert_eq!(value["retryable"], serde_json::Value::Bool(false));
    }

    #[test]
    fn command_result_ext_maps_to_internal_error() {
        let result: IpcResult<()> = Err("boom").ipc();
        let err = result.expect_err("expected mapped error");
        assert_eq!(err.code, IpcErrorCode::InternalError);
        assert!(!err.retryable);
        assert_eq!(err.message, "boom");
    }

    #[test]
    fn app_error_maps_to_typed_ipc_codes() {
        let validation = IpcError::from(AppError::InvalidPath("bad path".to_string()));
        assert_eq!(validation.code, IpcErrorCode::ValidationError);

        let not_found = IpcError::from(AppError::NotFound("missing".to_string()));
        assert_eq!(not_found.code, IpcErrorCode::NotFound);

        let conflict = IpcError::from(AppError::AlreadyExists("dup".to_string()));
        assert_eq!(conflict.code, IpcErrorCode::Conflict);

        let daemon_transport =
            IpcError::from(AppError::DaemonTransport("daemon offline".to_string()));
        assert_eq!(daemon_transport.code, IpcErrorCode::Unavailable);
        assert!(daemon_transport.retryable);
    }
}
