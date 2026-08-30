use std::path::{Path, PathBuf};

use crate::commands::logging::JSONL_LOG_FILE_NAME;
use crate::coordination::mesh_cli;
use crate::provider::path::linux_to_wsl_unc;
use crate::session_scanner::cli_tool::{config_for, CliTool};

const APP_BUNDLE_ID: &str = "com.taurhaus.dev";
const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const CODEX_HOME_ENV: &str = "CODEX_HOME";
const GROK_HOME_ENV: &str = "GROK_HOME";
const AGY_DIR_OVERRIDE_ENV: &str = "TAURHAUS_AGY_DIR";
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";
const HOOKS_DIRNAME: &str = "hooks";
const DAEMON_BINARY_NAME: &str = "taurhaus-daemon";
const DAEMON_BINARY_OVERRIDE_ENV: &str = "TAURHAUS_DAEMON_BINARY";
pub(crate) const DAEMON_TOKEN_FILENAME: &str = "daemon.token";

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

    /// Whether the active app-data root is the ordinary platform default.
    pub(crate) fn app_data_root_is_default() -> bool {
        Self::app_data_root() == default_app_data_root()
    }

    /// Daemon authentication token under the active app data root.
    pub fn daemon_token_path() -> PathBuf {
        Self::app_data_root().join(DAEMON_TOKEN_FILENAME)
    }

    /// Canonical structured JSONL log path.
    pub fn log_path() -> PathBuf {
        Self::app_data_root().join(JSONL_LOG_FILE_NAME)
    }

    /// Native Codex turn-complete edge sink.
    pub fn codex_notify_path() -> PathBuf {
        Self::app_data_root().join(crate::daemon::codex_notify::CODEX_NOTIFY_FILENAME)
    }

    /// Native Antigravity hook activity sink.
    pub fn agy_hooks_path() -> PathBuf {
        Self::app_data_root().join(crate::daemon::agy_hooks::AGY_HOOKS_FILENAME)
    }

    /// Claude home directory (`~/.claude`).
    pub fn claude_dir() -> PathBuf {
        env_path_override(CLAUDE_DIR_OVERRIDE_ENV).unwrap_or_else(default_claude_dir)
    }

    /// The Claude root only when it was moved (`TAURHAUS_CLAUDE_DIR`).
    ///
    /// Claude Code itself knows nothing about that variable: with
    /// `CLAUDE_CONFIG_DIR` unset it reads `~/.claude` whatever taurhaus was
    /// pointed at. Anything that launches Claude has to say the root out loud,
    /// and anything that scans for accounts has to stay inside it.
    pub fn claude_dir_override() -> Option<PathBuf> {
        env_path_override(CLAUDE_DIR_OVERRIDE_ENV)
    }

    /// Team state root (`~/.claude/teams`).
    pub fn teams_dir() -> PathBuf {
        Self::claude_dir().join("teams")
    }

    /// Codex home directory (`$CODEX_HOME` or `~/.codex`).
    pub fn codex_dir() -> PathBuf {
        env_path_override(CODEX_HOME_ENV).unwrap_or_else(default_codex_dir)
    }

    /// Grok home directory (`$GROK_HOME` or `~/.grok`).
    pub fn grok_dir() -> PathBuf {
        env_path_override(GROK_HOME_ENV).unwrap_or_else(default_grok_dir)
    }

    /// Antigravity's shared Google tooling root.
    ///
    /// `TAURHAUS_AGY_DIR` is a taurhaus-only isolation override. Antigravity
    /// has no supported process-level home selector, so managed launches still
    /// use its real `~/.gemini` root.
    pub fn agy_dir() -> PathBuf {
        if let Some(path) = Self::agy_dir_override() {
            return path;
        }
        if let Some(path) = windows_unc_home_subdir(".gemini") {
            return path;
        }
        home_dir_or_temp().join(".gemini")
    }

    /// The taurhaus-only Antigravity root override, when configured.
    pub fn agy_dir_override() -> Option<PathBuf> {
        env_path_override(AGY_DIR_OVERRIDE_ENV)
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

    /// Per-tool home root in the path form this process can access.
    pub fn tool_home(tool: CliTool) -> PathBuf {
        let tool_config = config_for(tool);
        match tool_config.capabilities.session_root {
            crate::session_scanner::cli_tool::SessionRoot::AppManagedClaudeDir => {
                Self::claude_dir()
            }
            crate::session_scanner::cli_tool::SessionRoot::ToolHome => {
                // Registry-driven: a tool whose spec names a session-home
                // override env has its session root follow that env. No tool
                // identity is consulted here.
                if let Some(home) = tool_config.home_override_env.and_then(env_path_override) {
                    return home;
                }
                default_tool_home(tool)
            }
        }
    }

    /// Per-tool session root.
    pub fn tool_session_root(tool: CliTool) -> PathBuf {
        config_for(tool)
            .projects_subdir
            .split('/')
            .filter(|segment| !segment.is_empty())
            .fold(Self::tool_home(tool), |root, segment| root.join(segment))
    }

    /// Daemon binary location.
    ///
    /// On Windows this resolves to the WSL Linux path because the daemon is
    /// executed inside WSL, not from the Windows filesystem.
    pub fn daemon_binary_path() -> PathBuf {
        if let Some(path) = env_path_override(DAEMON_BINARY_OVERRIDE_ENV) {
            return path;
        }
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

fn default_grok_dir() -> PathBuf {
    if let Some(path) = windows_unc_home_subdir(".grok") {
        return path;
    }

    home_dir_or_temp().join(".grok")
}

fn default_tool_home(tool: CliTool) -> PathBuf {
    let config = config_for(tool);

    if let Some(path) = windows_unc_home_subdir(config.base_dir_name) {
        return path;
    }

    home_dir_or_temp().join(config.base_dir_name)
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
    use tempfile::TempDir;

    struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvRestore {
        fn apply(values: &[(&'static str, Option<&Path>)]) -> Self {
            let previous = values
                .iter()
                .map(|(key, value)| {
                    let previous = std::env::var_os(key);
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
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

    fn acquire_env_test_guard() -> crate::test_support::EnvTestGuard {
        crate::test_support::acquire_env_test_guard()
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
    fn codex_notify_path_joins_jsonl_filename_under_app_data_root() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var(DATA_DIR_OVERRIDE_ENV, temp.path());

        let resolved = PlatformPaths::codex_notify_path();

        std::env::remove_var(DATA_DIR_OVERRIDE_ENV);
        assert_eq!(resolved, temp.path().join("codex-notify.jsonl"));
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

    // Regression: commit 4e9e2c54 centralized Antigravity's root but left it
    // fixed at the operator's `~/.gemini`, so E2E startup installed hooks into
    // the real profile even when every other tool root was isolated.
    #[test]
    fn agy_dir_and_sessions_use_the_taurhaus_override() {
        let _guard = acquire_env_test_guard();
        let temp = TempDir::new().expect("tempdir");
        std::env::set_var("TAURHAUS_AGY_DIR", temp.path());

        let root = PlatformPaths::agy_dir();
        let sessions = PlatformPaths::tool_session_root(CliTool::Agy);

        std::env::remove_var("TAURHAUS_AGY_DIR");
        assert_eq!(root, temp.path());
        assert_eq!(sessions, temp.path().join("antigravity-cli/conversations"));
    }

    // Regression: commit 0dcb7ba5 made account-home selectors change Codex and
    // Grok session scanning, despite the root-isolation spec preserving their
    // existing session-root behavior.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn codex_and_grok_session_roots_ignore_account_home_selectors() {
        let _guard = acquire_env_test_guard();
        let home = TempDir::new().expect("home");
        let account_home = TempDir::new().expect("account home");
        let _env = EnvRestore::apply(&[
            ("HOME", Some(home.path())),
            (CODEX_HOME_ENV, Some(account_home.path())),
            (GROK_HOME_ENV, Some(account_home.path())),
        ]);

        let codex_sessions = PlatformPaths::tool_session_root(CliTool::Codex);
        let grok_sessions = PlatformPaths::tool_session_root(CliTool::Grok);

        assert_eq!(codex_sessions, home.path().join(".codex").join("sessions"));
        assert_eq!(grok_sessions, home.path().join(".grok").join("sessions"));
    }

    // Regression: the ToolHome arm compared `tool == CliTool::Agy` — tool
    // identity outside the registry — while the registry field meant to carry
    // the rule sat unread (Opus review of 3a-i, remaining minor).
    #[test]
    fn tool_session_root_follows_the_registry_home_override() {
        let _guard = acquire_env_test_guard();
        let home = TempDir::new().expect("home");
        let agy_home = TempDir::new().expect("agy home");
        let _env = EnvRestore::apply(&[
            ("HOME", Some(home.path())),
            (AGY_DIR_OVERRIDE_ENV, Some(agy_home.path())),
        ]);

        assert_eq!(
            PlatformPaths::tool_session_root(CliTool::Agy),
            agy_home.path().join("antigravity-cli/conversations"),
            "the registry names the override env; the resolution reads it"
        );
        let spec = crate::session_scanner::cli_tool::spec(CliTool::Agy);
        assert_eq!(spec.home_override_env, Some(AGY_DIR_OVERRIDE_ENV));
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
        let _env = EnvRestore::apply(&[
            ("HOME", Some(home.path())),
            (CODEX_HOME_ENV, None),
            (GROK_HOME_ENV, None),
            (AGY_DIR_OVERRIDE_ENV, None),
        ]);

        let codex = PlatformPaths::tool_session_root(CliTool::Codex);
        let agy = PlatformPaths::tool_session_root(CliTool::Agy);

        assert_eq!(codex, home.path().join(".codex").join("sessions"));
        assert_eq!(
            agy,
            home.path()
                .join(".gemini")
                .join("antigravity-cli/conversations")
        );
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

    // Regression: commit 7908cbf4 isolated the E2E daemon port but forced the
    // app to launch the operator-installed daemon, which can predate the
    // worker's isolated auth-root behavior when installation is opted out.
    #[test]
    fn daemon_binary_path_uses_worker_override_when_set() {
        let _guard = acquire_env_test_guard();
        let root = TempDir::new().expect("tempdir");
        let daemon = root.path().join("taurhaus-daemon");
        let _env = EnvRestore::apply(&[("TAURHAUS_DAEMON_BINARY", Some(daemon.as_path()))]);

        assert_eq!(PlatformPaths::daemon_binary_path(), daemon);
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
