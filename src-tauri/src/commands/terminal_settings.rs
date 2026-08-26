use crate::commands::projects::DbState;
use crate::coordination::compact_hook::{
    ensure_codex_compact_hook_installed_at, remove_codex_compact_hook_at,
};
use crate::coordination::errors::CoordinationError;
#[cfg(test)]
use crate::models::CliCommandSettings;
use crate::models::{CodexCompactionMode, TerminalSettings};
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
    match mode {
        CodexCompactionMode::Hooks if has_managed_codex => {
            ensure_codex_compact_hook_installed_at(codex_home, taurhaus_exe)
        }
        CodexCompactionMode::Hooks => Ok(false),
        CodexCompactionMode::Transcript => remove_codex_compact_hook_at(codex_home),
    }
}

pub(crate) fn reconcile_codex_compaction(
    mode: CodexCompactionMode,
    has_managed_codex: bool,
) -> Result<bool, CoordinationError> {
    let executable = compact_hook_executable()?;
    let changed = reconcile_codex_compaction_at(
        &PlatformPaths::codex_dir(),
        mode,
        has_managed_codex,
        &executable,
    )?;
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
