use serde::Serialize;

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

    #[error("Search error: {0}")]
    SearchError(String),
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
}
