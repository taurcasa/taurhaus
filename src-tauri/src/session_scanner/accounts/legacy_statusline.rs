//! One-shot removal of the status-line bridge shipped in taurhaus 0.6.8.

use std::fs;
use std::path::Path;
use std::sync::Once;

use serde_json::{Map, Value};

const SETTINGS: &str = "settings.json";
const HOOKS: &str = "hooks";
const RECORD: &str = "taurhaus-statusline.json";
const SHELL_SCRIPT: &str = "taurhaus-statusline.sh";
const POWERSHELL_SCRIPT: &str = "taurhaus-statusline.ps1";
static RETIRE: Once = Once::new();

#[derive(serde::Deserialize)]
struct Record {
    command: Option<String>,
    wrapped: Option<Value>,
}

/// Restore a status line owned by taurhaus and remove its bridge files.
pub fn uninstall(config_dir: &Path) -> Result<bool, String> {
    let hooks = config_dir.join(HOOKS);
    let record_path = hooks.join(RECORD);
    let record = match fs::read_to_string(&record_path) {
        Ok(raw) => serde_json::from_str::<Record>(&raw)
            .map_err(|error| format!("failed to parse '{}': {error}", record_path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to read '{}': {error}",
                record_path.display()
            ))
        }
    };
    let Some(owned_command) = record.command else {
        return Ok(false);
    };

    let settings_path = config_dir.join(SETTINGS);
    let mut settings = load_settings(&settings_path)?;
    let current_command = settings
        .get("statusLine")
        .and_then(Value::as_object)
        .and_then(|row| row.get("command"))
        .and_then(Value::as_str);
    if current_command != Some(owned_command.as_str()) {
        // An edited or wrapped invocation is not ours to restore or delete.
        return Ok(false);
    }

    match record.wrapped {
        Some(wrapped) if !wrapped.is_null() => {
            settings.insert("statusLine".to_string(), wrapped);
        }
        _ => {
            settings.remove("statusLine");
        }
    }
    let encoded = serde_json::to_vec_pretty(&Value::Object(settings))
        .map_err(|error| format!("failed to serialize '{}': {error}", settings_path.display()))?;
    fs::write(&settings_path, encoded)
        .map_err(|error| format!("failed to write '{}': {error}", settings_path.display()))?;

    for path in [
        hooks.join(SHELL_SCRIPT),
        hooks.join(POWERSHELL_SCRIPT),
        record_path,
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to remove '{}': {error}", path.display())),
        }
    }
    Ok(true)
}

fn load_settings(path: &Path) -> Result<Map<String, Value>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(settings)) => Ok(settings),
        Ok(_) => Err(format!("'{}' is not a JSON object", path.display())),
        Err(error) => Err(format!("failed to parse '{}': {error}", path.display())),
    }
}

/// Retire every detected 0.6.8 bridge once in this process.
pub fn retire_once() {
    RETIRE.call_once(|| {
        for config_dir in super::transcript_dirs(crate::session_scanner::cli_tool::CliTool::Claude)
        {
            match uninstall(&config_dir) {
                Ok(true) => {
                    let mut fields = Map::new();
                    fields.insert(
                        "config_dir".to_string(),
                        Value::String(config_dir.display().to_string()),
                    );
                    crate::commands::logging::emit_global(
                        "info",
                        "accounts",
                        "claude.usage.legacy_bridge.removed",
                        Some("Removed legacy Claude status-line bridge".to_string()),
                        fields,
                    );
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(config_dir = %config_dir.display(), error = %error, "Legacy Claude status-line bridge removal failed"),
            }
        }

        let app_data = crate::provider::platform_paths::PlatformPaths::app_data_root();
        for filename in ["claude-usage.jsonl", "claude-usage.jsonl.lock"] {
            let path = app_data.join(filename);
            if let Err(error) = fs::remove_file(&path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(path = %path.display(), error = %error, "Legacy Claude usage sink removal failed");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{json, Value};

    use super::*;

    fn write_install(config_dir: &Path, command: &str, wrapped: Value) {
        let hooks = config_dir.join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(
            config_dir.join("settings.json"),
            serde_json::to_vec_pretty(&json!({
                "theme": "dark",
                "statusLine": { "type": "command", "command": command }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(hooks.join("taurhaus-statusline.sh"), "#!/bin/sh\n").unwrap();
        fs::write(
            hooks.join("taurhaus-statusline.json"),
            serde_json::to_vec_pretty(&json!({
                "executable": "/usr/bin/taurhaus-daemon",
                "sink": "/tmp/claude-usage.jsonl",
                "command": command,
                "wrapped": wrapped
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn settings(config_dir: &Path) -> Value {
        serde_json::from_slice(&fs::read(config_dir.join("settings.json")).unwrap()).unwrap()
    }

    #[test]
    fn wrapped_row_is_restored_byte_identically() {
        // Regression: 79be608 installed the 0.6.8 status-line bridge; retiring
        // it must return the user's exact command and options.
        let temp = tempfile::tempdir().unwrap();
        let original = json!({
            "type": "command",
            "command": "printf '%s' \"$payload\" | jq -r '.model.display_name'",
            "padding": 0
        });
        write_install(
            temp.path(),
            "bash '/tmp/taurhaus-statusline.sh'",
            original.clone(),
        );

        assert!(uninstall(temp.path()).unwrap());
        assert_eq!(settings(temp.path())["statusLine"], original);
    }

    #[test]
    fn taurhaus_only_row_is_removed() {
        // Regression: 79be608 took an empty status-line seat; uninstalling it
        // must remove the key instead of leaving a dead command behind.
        let temp = tempfile::tempdir().unwrap();
        write_install(
            temp.path(),
            "bash '/tmp/taurhaus-statusline.sh'",
            Value::Null,
        );

        assert!(uninstall(temp.path()).unwrap());
        assert!(settings(temp.path()).get("statusLine").is_none());
        assert_eq!(settings(temp.path())["theme"], "dark");
    }

    #[test]
    fn foreign_row_that_references_the_script_is_untouched() {
        // Regression: 79be608 created the script, but exact command ownership
        // is required before removing a row or a file another command uses.
        let temp = tempfile::tempdir().unwrap();
        let owned = "bash '/tmp/taurhaus-statusline.sh'";
        write_install(temp.path(), owned, Value::Null);
        let foreign = format!("env DEBUG=1 {owned}");
        fs::write(
            temp.path().join("settings.json"),
            serde_json::to_vec_pretty(&json!({
                "theme": "dark",
                "statusLine": { "type": "command", "command": foreign }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(!uninstall(temp.path()).unwrap());
        assert_eq!(settings(temp.path())["statusLine"]["command"], foreign);
        assert!(temp.path().join("hooks/taurhaus-statusline.sh").exists());
        assert!(temp.path().join("hooks/taurhaus-statusline.json").exists());
    }
}
