//! Daemon authentication — shared token between daemon and app.
//!
//! On startup the daemon generates a random 32-byte token, writes it to a
//! well-known file with 0600 permissions, and validates it on every request.
//! The app reads the token file when connecting.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::time::Duration;

use rand::Rng;

#[cfg(target_os = "windows")]
const WSL_AUTH_TOKEN_TIMEOUT: Duration = Duration::from_millis(350);

/// Resolve the daemon auth token under the active app data root.
pub fn token_path() -> Option<PathBuf> {
    Some(crate::provider::platform_paths::PlatformPaths::daemon_token_path())
}

/// The pre-0.8.5 token location. This remains a read-only compatibility path.
fn legacy_token_path() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("taurhaus").join("daemon.token"))
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
    let data_dir = crate::provider::platform_paths::PlatformPaths::app_data_root();
    read_auth_token_for_distro_at(wsl_distro, &data_dir)
}

fn read_auth_token_for_distro_at(wsl_distro: Option<&str>, data_dir: &Path) -> Option<String> {
    #[cfg(not(target_os = "windows"))]
    let _ = wsl_distro;

    // The app reads the root it passed to the daemon, never a reconstructed one.
    if let Ok(token) = read_token(&data_dir.join("daemon.token")) {
        return Some(token);
    }

    // Keep an already-running pre-migration daemon reachable until a new daemon
    // starts and writes the canonical token. Nothing writes through this path.
    if let Some(token) = legacy_token_path().and_then(|path| read_token(&path).ok()) {
        return Some(token);
    }

    // Fallback: read from WSL filesystem (Windows app + WSL daemon)
    #[cfg(target_os = "windows")]
    {
        let raw_data_dir = data_dir.to_string_lossy();
        let daemon_data_dir = crate::provider::path::to_linux(&raw_data_dir)
            .unwrap_or_else(|| raw_data_dir.to_string());
        if let Some(token) = read_token_via_wsl(wsl_distro, Path::new(&daemon_data_dir)) {
            return Some(token);
        }
    }

    None
}

/// Read the daemon token from inside WSL via `wsl.exe`.
///
/// Reads `<daemon_data_dir>/daemon.token`, then the legacy token as a fallback.
/// Uses `wsl_command()` from launcher to suppress console window flash.
#[cfg(target_os = "windows")]
fn read_token_via_wsl(wsl_distro: Option<&str>, daemon_data_dir: &Path) -> Option<String> {
    let mut command = crate::daemon::launcher::wsl_command();
    if let Some(distro) = wsl_distro {
        crate::daemon::launcher::validate_wsl_distro(distro).ok()?;
        command.arg("-d").arg(distro);
    }
    let script = wsl_token_read_script(daemon_data_dir);
    command
        .args(["-e", "sh", "-c"])
        .arg(script)
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

#[cfg(any(target_os = "windows", test))]
fn wsl_token_read_script(daemon_data_dir: &Path) -> String {
    let canonical = daemon_data_dir.join("daemon.token");
    let canonical = canonical.to_string_lossy().replace('\'', "'\"'\"'");
    format!(
        "cat '{canonical}' 2>/dev/null || cat \"$HOME/.local/share/taurhaus/daemon.token\" 2>/dev/null"
    )
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

    struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvRestore {
        fn set(values: &[(&'static str, &std::path::Path)]) -> Self {
            let previous = values
                .iter()
                .map(|(key, value)| {
                    let previous = std::env::var_os(key);
                    std::env::set_var(key, value);
                    (*key, previous)
                })
                .collect();
            Self(previous)
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn token_path_returns_some() {
        // On any CI or dev machine, data_dir should exist
        assert!(token_path().is_some());
    }

    // Regression: commit 06e8f740 introduced the daemon token under a second,
    // non-overridable `taurhaus` data root, so an isolated E2E daemon could
    // overwrite the operator daemon's credential.
    #[test]
    fn token_path_honours_the_app_data_override() {
        let _guard = crate::test_support::acquire_env_test_guard();
        let active_root = tempfile::tempdir().expect("active root");
        let legacy_data_root = tempfile::tempdir().expect("legacy data root");
        let _env = EnvRestore::set(&[
            ("TAURHAUS_DATA_DIR", active_root.path()),
            ("XDG_DATA_HOME", legacy_data_root.path()),
        ]);

        assert_eq!(token_path(), Some(active_root.path().join("daemon.token")));
    }

    // Regression: commit 06e8f740 made the old `taurhaus/daemon.token` path
    // the only reader. Moving the authority must retain that path as a
    // read-only fallback while an already-running older daemon still uses it.
    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_token_remains_readable() {
        let _guard = crate::test_support::acquire_env_test_guard();
        let active_root = tempfile::tempdir().expect("active root");
        let legacy_data_root = tempfile::tempdir().expect("legacy data root");
        let _env = EnvRestore::set(&[
            ("TAURHAUS_DATA_DIR", active_root.path()),
            ("XDG_DATA_HOME", legacy_data_root.path()),
        ]);
        let legacy_path = legacy_data_root.path().join("taurhaus/daemon.token");
        std::fs::create_dir_all(legacy_path.parent().expect("legacy parent")).unwrap();
        std::fs::write(&legacy_path, "legacy-token\n").unwrap();

        assert_eq!(read_auth_token(), Some("legacy-token".to_string()));
    }

    // Regression: commit 06e8f740 made the legacy path writable. Once a run
    // supplies an override, generation must stay wholly inside that root.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_override_never_writes_the_legacy_token_path() {
        let _guard = crate::test_support::acquire_env_test_guard();
        let active_root = tempfile::tempdir().expect("active root");
        let legacy_data_root = tempfile::tempdir().expect("legacy data root");
        let _env = EnvRestore::set(&[
            ("TAURHAUS_DATA_DIR", active_root.path()),
            ("XDG_DATA_HOME", legacy_data_root.path()),
        ]);
        let active_path = token_path().expect("active token path");
        let legacy_path = legacy_data_root.path().join("taurhaus/daemon.token");

        generate_and_write_token(&active_path).expect("generate token");

        assert!(active_root.path().join("daemon.token").is_file());
        assert!(!legacy_path.exists());
    }

    // Regression: commit c8ccdc16 added the Windows/WSL fallback with a
    // hard-coded `$HOME/.local/share/taurhaus` path, so the app guessed a root
    // different from the one its launcher passed to an isolated daemon.
    #[test]
    fn wsl_fallback_reads_the_passed_data_dir_before_the_legacy_path() {
        let script = wsl_token_read_script(Path::new("/mnt/c/e2e root/it's isolated"));
        let canonical = script
            .find("/mnt/c/e2e root/it")
            .expect("passed data dir in script");
        let legacy = script
            .find("$HOME/.local/share/taurhaus/daemon.token")
            .expect("legacy fallback in script");

        assert!(script.contains("it'\"'\"'s isolated/daemon.token"));
        assert!(canonical < legacy, "canonical root must be attempted first");
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
