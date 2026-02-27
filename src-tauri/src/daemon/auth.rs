//! Daemon authentication — shared token between daemon and app.
//!
//! On startup the daemon generates a random 32-byte token, writes it to a
//! well-known file with 0600 permissions, and validates it on every request.
//! The app reads the token file when connecting.

use std::fs;
use std::io;
use std::path::PathBuf;

use rand::Rng;

/// Resolve the platform-specific path for the daemon auth token.
///
/// - Linux:  `~/.local/share/taurhaus/daemon.token`
/// - macOS:  `~/Library/Application Support/taurhaus/daemon.token`
/// - Windows: `{FOLDERID_LocalAppData}/taurhaus/daemon.token`
pub fn token_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("taurhaus").join("daemon.token"))
}

/// Generate a random 32-byte hex token, write it to `path` with 0600 permissions.
pub fn generate_and_write_token(path: &std::path::Path) -> io::Result<String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    let token = hex::encode(bytes);

    fs::write(path, &token)?;

    // Set 0600 permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

/// Read a token from a file (trims whitespace).
pub fn read_token(path: &std::path::Path) -> io::Result<String> {
    let content = fs::read_to_string(path)?;
    Ok(content.trim().to_string())
}

/// Validate a provided token against the expected value.
///
/// Uses constant-time comparison to prevent timing attacks.
pub fn validate_token(expected: &str, provided: Option<&str>) -> Result<(), String> {
    let provided = provided.ok_or_else(|| "Missing auth token".to_string())?;

    if expected.len() != provided.len() {
        return Err("Invalid auth token".to_string());
    }

    // Constant-time comparison
    let mismatch = expected
        .as_bytes()
        .iter()
        .zip(provided.as_bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));

    if mismatch == 0 {
        Ok(())
    } else {
        Err("Invalid auth token".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_path_returns_some() {
        // On any CI or dev machine, data_dir should exist
        assert!(token_path().is_some());
    }

    #[test]
    fn token_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("daemon.token");

        let token = generate_and_write_token(&path).unwrap();
        assert_eq!(token.len(), 64); // 32 bytes hex-encoded

        let read_back = read_token(&path).unwrap();
        assert_eq!(token, read_back);
    }

    #[cfg(unix)]
    #[test]
    fn token_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("daemon.token");
        generate_and_write_token(&path).unwrap();

        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn validate_correct_token() {
        assert!(validate_token("abc123", Some("abc123")).is_ok());
    }

    #[test]
    fn validate_wrong_token() {
        assert!(validate_token("abc123", Some("wrong!")).is_err());
    }

    #[test]
    fn validate_missing_token() {
        assert!(validate_token("abc123", None).is_err());
    }

    #[test]
    fn validate_different_length() {
        assert!(validate_token("abc123", Some("abc")).is_err());
    }
}
