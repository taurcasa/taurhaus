use crate::commands::projects::DbState;
use crate::coordination::compact_hook::{
    ensure_codex_compact_hook_installed_at, remove_codex_compact_hook_at,
};
use crate::coordination::errors::CoordinationError;
#[cfg(test)]
use crate::models::CliCommandSettings;
use crate::models::{CliVersions, CodexCompactionMode, TerminalSettings};
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::all;

pub fn load_terminal_settings(db: &DbState) -> TerminalSettings {
    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::warn!(error = %e, "Settings DB lock poisoned, using default terminal settings");
            return TerminalSettings::default();
        }
    };
    match crate::db::settings_queries::get_all_settings(&conn) {
        Ok(settings) => settings.terminal,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load settings, using default terminal settings");
            TerminalSettings::default()
        }
    }
}

pub(crate) fn apply_managed_codex_launch_inputs(
    cli_commands: &mut crate::models::CliCommandSettings,
    has_managed_codex: bool,
    codex_bypass_hook_trust: bool,
) {
    let notify_supported = CliVersions::current().codex_notify_supported;
    let daemon_executable = PlatformPaths::daemon_binary_path();
    let codex_config_path = PlatformPaths::codex_dir().join("config.toml");
    let user_notify_configured = match codex_config_has_notify(&codex_config_path) {
        Ok(configured) => configured,
        Err(error) => {
            tracing::warn!(
                path = %codex_config_path.display(),
                error,
                "Could not inspect Codex config for an existing notifier"
            );
            false
        }
    };
    apply_managed_codex_launch_inputs_with_support(
        cli_commands,
        has_managed_codex,
        codex_bypass_hook_trust,
        notify_supported,
        user_notify_configured,
        &daemon_executable,
    );
    apply_managed_account_selector(cli_commands, has_managed_codex, PlatformPaths::codex_dir());
    if has_managed_codex && notify_supported && user_notify_configured {
        tracing::info!(
            path = %codex_config_path.display(),
            "Codex native notify preserved the user's config.toml notifier"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "found".to_string(),
            serde_json::Value::String("config.toml notify".to_string()),
        );
        fields.insert(
            "path".to_string(),
            serde_json::Value::String(codex_config_path.display().to_string()),
        );
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "launch.notify.ignored",
            Some("Preserved the user-configured Codex notifier".to_string()),
            fields,
        );
    } else if has_managed_codex
        && notify_supported
        && !codex_notify_executable_available(&daemon_executable)
    {
        tracing::warn!(
            path = %daemon_executable.display(),
            "Codex native notify skipped because the daemon executable is missing"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "path".to_string(),
            serde_json::Value::String(daemon_executable.display().to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "codex.notify.executable_missing",
            Some("Managed Codex notify requires the installed taurhaus daemon".to_string()),
            fields,
        );
    }
}

fn apply_managed_account_selector(
    cli_commands: &mut crate::models::CliCommandSettings,
    enabled: bool,
    dir: std::path::PathBuf,
) {
    let Some(selector) = all()
        .iter()
        .find(|entry| entry.capabilities.managed_home)
        .and_then(|entry| entry.capabilities.account_selector)
    else {
        return;
    };
    if enabled {
        cli_commands
            .account_selector_dirs
            .insert(selector.to_string(), dir);
    } else {
        cli_commands.account_selector_dirs.remove(selector);
    }
}

fn apply_managed_codex_launch_inputs_with_support(
    cli_commands: &mut crate::models::CliCommandSettings,
    has_managed_codex: bool,
    codex_bypass_hook_trust: bool,
    notify_supported: bool,
    user_notify_configured: bool,
    daemon_executable: &std::path::Path,
) {
    cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
    cli_commands.codex_notify_executable = (has_managed_codex
        && notify_supported
        && !user_notify_configured
        && codex_notify_executable_available(daemon_executable))
    .then(|| daemon_executable.to_path_buf());
}

fn codex_config_has_notify(path: &std::path::Path) -> Result<bool, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to read Codex config '{}': {error}",
                path.display()
            ));
        }
    };
    let config = raw
        .parse::<toml::Table>()
        .map_err(|error| format!("failed to parse Codex config '{}': {error}", path.display()))?;
    Ok(config.contains_key("notify"))
}

#[cfg(not(target_os = "windows"))]
fn codex_notify_executable_available(path: &std::path::Path) -> bool {
    path.is_file()
}

#[cfg(target_os = "windows")]
fn codex_notify_executable_available(path: &std::path::Path) -> bool {
    let Some(distro) = crate::coordination::mesh_cli::resolve_wsl_distro_for_coordination(None)
    else {
        return false;
    };
    let Some(linux_path) = path.to_str() else {
        return false;
    };
    std::path::Path::new(&crate::provider::path::linux_to_wsl_unc(
        linux_path, &distro,
    ))
    .is_file()
}

#[cfg(test)]
pub fn load_cli_commands(db: &DbState) -> CliCommandSettings {
    load_terminal_settings(db).cli_commands
}

pub(crate) fn reconcile_codex_compaction_at(
    codex_home: &std::path::Path,
    mode: CodexCompactionMode,
    has_managed_codex: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    reconcile_codex_compaction_at_with_support(
        codex_home,
        mode,
        has_managed_codex,
        CliVersions::current().codex_compaction_hooks_support(),
        taurhaus_exe,
    )
}

pub(crate) fn reconcile_codex_compaction_at_with_support(
    codex_home: &std::path::Path,
    mode: CodexCompactionMode,
    has_managed_codex: bool,
    hooks_supported: Option<bool>,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    match mode {
        CodexCompactionMode::Hooks if has_managed_codex && hooks_supported == Some(true) => {
            ensure_codex_compact_hook_installed_at(codex_home, taurhaus_exe)
        }
        CodexCompactionMode::Hooks if has_managed_codex && hooks_supported == Some(false) => {
            remove_codex_compact_hook_at(codex_home)
        }
        CodexCompactionMode::Hooks => Ok(false),
        CodexCompactionMode::Transcript => remove_codex_compact_hook_at(codex_home),
    }
}

pub(crate) fn reconcile_codex_compaction(
    mode: CodexCompactionMode,
    has_managed_codex: bool,
) -> Result<bool, CoordinationError> {
    let hooks_support = CliVersions::current().codex_compaction_hooks_support();
    let executable = compact_hook_executable()?;
    let changed = reconcile_codex_compaction_at(
        &PlatformPaths::codex_dir(),
        mode,
        has_managed_codex,
        &executable,
    )?;
    if mode == CodexCompactionMode::Hooks && has_managed_codex && hooks_support == Some(false) {
        tracing::warn!(
            codex_version = ?CliVersions::current().codex,
            "Codex compact hook skipped because the installed CLI predates 0.147"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "version".to_string(),
            CliVersions::current()
                .codex
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        );
        fields.insert(
            "minimum_version".to_string(),
            serde_json::Value::String("0.147.0".to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "compaction.codex_hook.unsupported",
            Some("Codex compact hook requires CLI version 0.147.0 or newer".to_string()),
            fields,
        );
    } else if mode == CodexCompactionMode::Hooks && has_managed_codex && hooks_support.is_none() {
        tracing::warn!(
            "Codex compact hook reconciliation skipped because the CLI version could not be resolved"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "minimum_version".to_string(),
            serde_json::Value::String("0.147.0".to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "compaction.codex_hook.version_unknown",
            Some(
                "Left the Codex compact hook unchanged because the CLI version was unavailable"
                    .to_string(),
            ),
            fields,
        );
    }
    if changed {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "mode".to_string(),
            serde_json::Value::String(
                match mode {
                    CodexCompactionMode::Hooks => "hooks",
                    CodexCompactionMode::Transcript => "transcript",
                }
                .to_string(),
            ),
        );
        fields.insert("changed".to_string(), serde_json::Value::Bool(true));
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "compaction.codex_hook.reconciled",
            Some("Reconciled Codex compaction source".to_string()),
            fields,
        );
    }
    Ok(changed)
}

pub(crate) fn reconcile_agy_hooks(enabled: bool) -> Result<bool, CoordinationError> {
    let root = PlatformPaths::agy_dir();
    if enabled {
        crate::coordination::agy_hooks_installer::ensure_agy_hooks_installed_at(
            &root,
            &PlatformPaths::daemon_binary_path(),
        )
    } else {
        crate::coordination::agy_hooks_installer::remove_agy_hooks_at(&root)
    }
}

/// Reconcile the one global grok hook against the current roster.
///
/// grok registers hooks per home, not per session, so the hook has to appear as
/// soon as the first managed grok member exists and go away once the last one
/// does — every roster mutation calls this, not just startup and a Settings
/// save. A discovery failure is reported rather than answered with "no members":
/// an unreadable team directory is not proof the last grok member is gone, and
/// uninstalling on it would silently disable reinjection for a live session.
pub(crate) fn reconcile_grok_hooks_for_roster(
    teams_dir: &std::path::Path,
    enabled: bool,
) -> Result<bool, CoordinationError> {
    reconcile_grok_hooks_for_roster_at(
        teams_dir,
        &PlatformPaths::grok_dir(),
        enabled,
        &compact_hook_executable()?,
    )
}

pub(crate) fn reconcile_grok_hooks_for_roster_at(
    teams_dir: &std::path::Path,
    grok_home: &std::path::Path,
    enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let has_managed_grok = crate::coordination::compact_hook::any_managed_grok_member(teams_dir)?;
    reconcile_grok_hooks_at(grok_home, enabled, has_managed_grok, taurhaus_exe)
}

pub(crate) fn reconcile_grok_hooks_at(
    grok_home: &std::path::Path,
    enabled: bool,
    has_managed_grok: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    if enabled && has_managed_grok {
        crate::coordination::compact_hook::ensure_grok_compact_hook_installed_at(
            grok_home,
            taurhaus_exe,
        )
    } else {
        crate::coordination::compact_hook::remove_grok_compact_hook_at(grok_home)
    }
}

fn compact_hook_executable() -> Result<std::path::PathBuf, CoordinationError> {
    if cfg!(target_os = "windows") {
        return Ok(PlatformPaths::daemon_binary_path());
    }
    std::env::current_exe().map_err(|error| {
        CoordinationError::Backend(format!(
            "failed to resolve taurhaus executable for Codex compact hook: {error}"
        ))
    })
}

#[cfg(test)]
#[path = "terminal_settings/tests.rs"]
mod tests;
