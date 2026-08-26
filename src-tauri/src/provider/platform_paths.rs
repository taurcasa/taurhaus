use std::path::{Path, PathBuf};

use crate::commands::logging::JSONL_LOG_FILE_NAME;
use crate::coordination::mesh_cli;
use crate::provider::path::linux_to_wsl_unc;
use crate::session_scanner::cli_tool::{config_for, CliTool};

const APP_BUNDLE_ID: &str = "com.taurhaus.dev";
const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const CODEX_HOME_ENV: &str = "CODEX_HOME";
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";
const HOOKS_DIRNAME: &str = "hooks";
const DAEMON_BINARY_NAME: &str = "taurhaus-daemon";

/// Central authority for platform-sensitive path resolution.
///
/// File-backed roots resolve to paths the current process can access directly.
/// On Windows, that means WSL-backed tool state resolves to UNC paths when
/// available. The daemon binary path is different: it resolves to the WSL Linux
/// path because launcher commands execute it inside WSL.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlatformPaths;

impl PlatformPaths {
    /// Active app data directory for the current run.
    pub fn app_data_root() -> PathBuf {
        env_path_override(DATA_DIR_OVERRIDE_ENV).unwrap_or_else(default_app_data_root)
    }

    /// Canonical structured JSONL log path.
    pub fn log_path() -> PathBuf {
        Self::app_data_root().join(JSONL_LOG_FILE_NAME)
    }

    /// Claude home directory (`~/.claude`).
    pub fn claude_dir() -> PathBuf {
        env_path_override(CLAUDE_DIR_OVERRIDE_ENV).unwrap_or_else(default_claude_dir)
    }

    /// Team state root (`~/.claude/teams`).
    pub fn teams_dir() -> PathBuf {
        Self::claude_dir().join("teams")
    }

    /// Codex home directory (`$CODEX_HOME` or `~/.codex`).
    pub fn codex_dir() -> PathBuf {
        env_path_override(CODEX_HOME_ENV).unwrap_or_else(default_codex_dir)
    }

    /// App-data root used by coordination template hydration.
    ///
    /// Production team state is Claude-owned while templates are taurhaus-owned.
    /// Explicit non-production roots keep the historical colocated layout used by
    /// tests and embedders.
    pub fn coordination_template_root(teams_dir: &Path) -> PathBuf {
        if teams_dir == Self::teams_dir() {
            return Self::app_data_root();
        }

        teams_dir
            .file_name()
            .filter(|name| *name == "teams")
            .and_then(|_| teams_dir.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| teams_dir.to_path_buf())
    }

    /// Per-tool session root.
    pub fn tool_session_root(tool: CliTool) -> PathBuf {
        match tool {
            CliTool::Claude => Self::claude_dir().join(config_for(tool).projects_subdir),
            _ => default_tool_session_root(tool),
        }
    }

    /// Daemon binary location.
    ///
    /// On Windows this resolves to the WSL Linux path because the daemon is
    /// executed inside WSL, not from the Windows filesystem.
    pub fn daemon_binary_path() -> PathBuf {
        if cfg!(target_os = "windows") {
            if let Some(home) = mesh_cli::resolve_wsl_home_for_coordination() {
                return PathBuf::from(format!("{home}/.local/bin/{DAEMON_BINARY_NAME}"));
            }
        }

        home_dir_or_temp()
            .join(".local")
            .join("bin")
            .join(DAEMON_BINARY_NAME)
    }

    /// Claude hook script directory (`~/.claude/hooks`).
    pub fn hook_script_dir() -> PathBuf {
        Self::claude_dir().join(HOOKS_DIRNAME)
    }

    /// Claude settings path (`~/.claude/settings.json`).
    pub fn hook_settings_path() -> PathBuf {
        Self::claude_dir().join(CLAUDE_SETTINGS_FILENAME)
    }
}

fn default_app_data_root() -> PathBuf {
    if let Some(data_dir) = dirs::data_dir() {
        return data_dir.join(APP_BUNDLE_ID);
    }

    let fallback = std::env::temp_dir().join(APP_BUNDLE_ID);
    tracing::warn!(
        fallback = %fallback.display(),
        "data directory unavailable; falling back to temp directory for app data root"
    );
    fallback
}

fn default_claude_dir() -> PathBuf {
    if let Some(path) = windows_unc_home_subdir(".claude") {
        return path;
    }

    home_dir_or_temp().join(".claude")
}

fn default_codex_dir() -> PathBuf {
    if let Some(path) = windows_unc_home_subdir(".codex") {
        return path;
    }

    home_dir_or_temp().join(".codex")
}

fn default_tool_session_root(tool: CliTool) -> PathBuf {
    let config = config_for(tool);

    if let Some(path) = windows_unc_home_subdir(&format!(
        "{}/{}",
        config.base_dir_name, config.projects_subdir
    )) {
        return path;
    }

    home_dir_or_temp()
        .join(config.base_dir_name)
        .join(config.projects_subdir)
}

fn windows_unc_home_subdir(subdir: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    let distro = mesh_cli::resolve_wsl_distro_for_coordination(None)?;
    let home = mesh_cli::resolve_wsl_home_for_coordination_in_distro(Some(&distro))?;
    let subdir = subdir.trim_start_matches('/');
    let linux_path = format!("{home}/{subdir}");
    Some(unc_path_for_linux_path(&distro, &linux_path))
}

fn unc_path_for_linux_path(distro: &str, linux_path: &str) -> PathBuf {
    PathBuf::from(linux_to_wsl_unc(linux_path, distro))
}

fn env_path_override(env_key: &str) -> Option<PathBuf> {
    let path = std::env::var_os(env_key)?;
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

fn home_dir_or_temp() -> PathBuf {
    if let Some(home_dir) = dirs::home_dir() {
        return home_dir;
    }

    let fallback = std::env::temp_dir().join("taurhaus-home");
    tracing::warn!(
        fallback = %fallback.display(),
        "home directory unavailable; falling back to temp directory"
    );
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs2::FileExt;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use tempfile::TempDir;

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvTestGuard {
        _in_process: MutexGuard<'static, ()>,
        lock_file: std::fs::File,
    }

    impl Drop for EnvTestGuard {
        fn drop(&mut self) {
            let _ = self.lock_file.unlock();
        }
    }

    fn acquire_env_test_guard() -> EnvTestGuard {
        let in_process = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let lock_path = std::env::temp_dir().join("taurhaus-platform-paths-env-tests.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap_or_else(|e| panic!("failed to open env test lock at {:?}: {e}", lock_path));
        lock_file
            .lock_exclusive()
            .unwrap_or_else(|e| panic!("failed to lock env test lock at {:?}: {e}", lock_path));
        EnvTestGuard {
            _in_process: in_process,
            lock_file,
        }
    }

    #[test]
    fn app_data_root_uses_override_when_set() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(DATA_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::app_data_root();

        std::env::remove_var(DATA_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path());
    }

    #[test]
    fn log_path_joins_jsonl_filename_under_app_data_root() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(DATA_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::log_path();

        std::env::remove_var(DATA_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path().join(JSONL_LOG_FILE_NAME));
    }

    #[test]
    fn claude_dir_uses_override_when_set() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::claude_dir();

        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path());
    }

    #[test]
    fn teams_dir_appends_teams_to_claude_dir() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::teams_dir();

        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path().join("teams"));
    }

    #[test]
    fn hook_paths_follow_claude_dir() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, temp.path());

        let script_dir = PlatformPaths::hook_script_dir();
        let settings_path = PlatformPaths::hook_settings_path();

        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);
        assert_eq!(script_dir, temp.path().join(HOOKS_DIRNAME));
        assert_eq!(settings_path, temp.path().join(CLAUDE_SETTINGS_FILENAME));
    }

    #[test]
    fn tool_session_root_uses_claude_projects_under_claude_dir() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::tool_session_root(CliTool::Claude);

        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path().join("projects"));
    }

    #[test]
    fn tool_session_root_uses_tool_specific_subdirs() {
        let _guard = acquire_env_test_guard();
        let home = TempDir::new().expect("tempdir");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let codex = PlatformPaths::tool_session_root(CliTool::Codex);
        let gemini = PlatformPaths::tool_session_root(CliTool::Gemini);

        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }

        assert_eq!(codex, home.path().join(".codex").join("sessions"));
        assert_eq!(gemini, home.path().join(".gemini").join("tmp"));
    }

    #[test]
    fn daemon_binary_path_uses_native_home_layout() {
        let _guard = acquire_env_test_guard();
        let home = TempDir::new().expect("tempdir");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let resolved = PlatformPaths::daemon_binary_path();

        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }

        if cfg!(target_os = "windows") {
            assert!(
                resolved
                    .to_string_lossy()
                    .ends_with("/.local/bin/taurhaus-daemon"),
                "windows daemon path should point at the WSL binary when available, got {}",
                resolved.display()
            );
        } else {
            assert_eq!(
                resolved,
                home.path()
                    .join(".local")
                    .join("bin")
                    .join(DAEMON_BINARY_NAME)
            );
        }
    }

    #[test]
    fn unc_path_for_linux_path_builds_wsl_unc_path() {
        let resolved = unc_path_for_linux_path("Ubuntu", "/home/user/.codex/sessions");
        assert_eq!(
            resolved.to_string_lossy(),
            r"\\wsl.localhost\Ubuntu\home\user\.codex\sessions"
        );
    }

    #[test]
    fn default_app_data_root_ends_with_bundle_id() {
        let _guard = acquire_env_test_guard();
        std::env::remove_var(DATA_DIR_OVERRIDE_ENV);
        let resolved = default_app_data_root();
        assert!(resolved.ends_with(APP_BUNDLE_ID));
    }
}
