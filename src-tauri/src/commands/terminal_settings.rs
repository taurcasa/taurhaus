use crate::commands::projects::DbState;
use crate::coordination::compact_hook::{
    ensure_codex_compact_hook_installed_at, remove_codex_compact_hook_at,
};
use crate::coordination::errors::CoordinationError;
#[cfg(test)]
use crate::models::CliCommandSettings;
use crate::models::{CliVersions, CodexCompactionMode, TerminalSettings};
use crate::provider::platform_paths::PlatformPaths;

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
    apply_managed_codex_launch_inputs_with_support(
        cli_commands,
        has_managed_codex,
        codex_bypass_hook_trust,
        CliVersions::current().codex_notify_supported,
        &PlatformPaths::daemon_binary_path(),
    );
}

fn apply_managed_codex_launch_inputs_with_support(
    cli_commands: &mut crate::models::CliCommandSettings,
    has_managed_codex: bool,
    codex_bypass_hook_trust: bool,
    notify_supported: bool,
    daemon_executable: &std::path::Path,
) {
    cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
    cli_commands.codex_notify_executable =
        (has_managed_codex && notify_supported).then(|| daemon_executable.to_path_buf());
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
        CliVersions::current().codex_compaction_hooks_supported,
        taurhaus_exe,
    )
}

pub(crate) fn reconcile_codex_compaction_at_with_support(
    codex_home: &std::path::Path,
    mode: CodexCompactionMode,
    has_managed_codex: bool,
    hooks_supported: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    match mode {
        CodexCompactionMode::Hooks if has_managed_codex && hooks_supported => {
            ensure_codex_compact_hook_installed_at(codex_home, taurhaus_exe)
        }
        CodexCompactionMode::Hooks if has_managed_codex => remove_codex_compact_hook_at(codex_home),
        CodexCompactionMode::Hooks => Ok(false),
        CodexCompactionMode::Transcript => remove_codex_compact_hook_at(codex_home),
    }
}

pub(crate) fn reconcile_codex_compaction(
    mode: CodexCompactionMode,
    has_managed_codex: bool,
) -> Result<bool, CoordinationError> {
    let hooks_supported = CliVersions::current().codex_compaction_hooks_supported;
    let executable = compact_hook_executable()?;
    let changed = reconcile_codex_compaction_at(
        &PlatformPaths::codex_dir(),
        mode,
        has_managed_codex,
        &executable,
    )?;
    if mode == CodexCompactionMode::Hooks && has_managed_codex && !hooks_supported {
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
