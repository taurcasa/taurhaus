//! Daemon authentication — shared token between daemon and app.
//!
//! On startup the daemon generates a random 32-byte token, writes it to a
//! well-known file with 0600 permissions, and validates it on every request.
//! The app reads the token file when connecting.

use std::fs;
use std::io;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::time::Duration;

use rand::Rng;

#[cfg(target_os = "windows")]
const WSL_AUTH_TOKEN_TIMEOUT: Duration = Duration::from_millis(350);

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

/// Read the daemon's auth token, with cross-platform fallback.
///
/// On the same OS (macOS, Linux), `token_path()` points to the same file the
/// daemon wrote. On Windows with a WSL daemon, the native path doesn't exist —
/// fall back to reading via `wsl.exe` from the Linux filesystem.
pub fn read_auth_token() -> Option<String> {
    read_auth_token_for_distro(None)
}

/// Read the daemon auth token for a specific runtime context.
///
/// On Windows, `wsl_distro` ensures we probe the same distro the daemon was
/// started in rather than whichever distro is currently the default.
pub fn read_auth_token_for_distro(wsl_distro: Option<&str>) -> Option<String> {
    #[cfg(not(target_os = "windows"))]
    let _ = wsl_distro;

    // Try native path first (works when daemon runs on same OS)
    if let Some(path) = token_path() {
        if let Ok(token) = read_token(&path) {
            return Some(token);
        }
    }

    // Fallback: read from WSL filesystem (Windows app + WSL daemon)
    #[cfg(target_os = "windows")]
    {
        if let Some(token) = read_token_via_wsl(wsl_distro) {
            return Some(token);
        }
    }

    None
}

/// Read the daemon token from inside WSL via `wsl.exe`.
///
/// Runs: `wsl.exe -e sh -c 'cat "$HOME/.local/share/taurhaus/daemon.token"'`
/// Uses `wsl_command()` from launcher to suppress console window flash.
#[cfg(target_os = "windows")]
fn read_token_via_wsl(wsl_distro: Option<&str>) -> Option<String> {
    let mut command = crate::daemon::launcher::wsl_command();
    if let Some(distro) = wsl_distro {
        crate::daemon::launcher::validate_wsl_distro(distro).ok()?;
        command.arg("-d").arg(distro);
    }
    command
        .args([
            "-e",
            "sh",
            "-c",
            "cat \"$HOME/.local/share/taurhaus/daemon.token\" 2>/dev/null",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        WSL_AUTH_TOKEN_TIMEOUT,
        "wsl read daemon auth token",
    )
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if token.is_empty() {
        return None;
    }

    tracing::debug!("Read daemon auth token via WSL fallback");
    Some(token)
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
