use crate::commands::projects::DbState;
use crate::models::{CliCommandSettings, TerminalSettings};

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

pub fn load_cli_commands(db: &DbState) -> CliCommandSettings {
    load_terminal_settings(db).cli_commands
}
