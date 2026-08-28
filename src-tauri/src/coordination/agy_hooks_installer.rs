use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::coordination::errors::CoordinationError;

const HOOKS_FILE: &str = "antigravity-cli/hooks.json";
const TAURHAUS_HOOK: &str = "taurhaus";

pub fn ensure_agy_hooks_installed_at(
    agy_root: &Path,
    daemon_executable: &Path,
) -> Result<bool, CoordinationError> {
    if !daemon_executable.is_file() {
        return Err(CoordinationError::StoreError(format!(
            "Antigravity hook executable '{}' does not exist",
            daemon_executable.display()
        )));
    }
    let path = hooks_path(agy_root);
    let mut root = load_hooks(&path)?;
    let original = root.clone();
    root.as_object_mut()
        .expect("hook root was validated")
        .insert(TAURHAUS_HOOK.to_string(), taurhaus_entry(daemon_executable));
    if root == original {
        return Ok(false);
    }
    write_hooks(&path, &root)?;
    Ok(true)
}

pub fn remove_agy_hooks_at(agy_root: &Path) -> Result<bool, CoordinationError> {
    let path = hooks_path(agy_root);
    let mut root = load_hooks(&path)?;
    let changed = root
        .as_object_mut()
        .expect("hook root was validated")
        .remove(TAURHAUS_HOOK)
        .is_some();
    if changed {
        write_hooks(&path, &root)?;
    }
    Ok(changed)
}

pub fn agy_hooks_installed_at(agy_root: &Path) -> bool {
    load_hooks(&hooks_path(agy_root))
        .ok()
        .and_then(|root| root.get(TAURHAUS_HOOK).cloned())
        .is_some_and(|entry| {
            entry.get("enabled").and_then(Value::as_bool) != Some(false)
                && has_mode(&entry, "PreInvocation", "busy")
                && has_mode(&entry, "Stop", "idle")
        })
}

fn hooks_path(agy_root: &Path) -> PathBuf {
    agy_root.join(HOOKS_FILE)
}

fn taurhaus_entry(daemon_executable: &Path) -> Value {
    let executable = shell_quote(&daemon_executable.to_string_lossy());
    json!({
        "PreInvocation": [{
            "type": "command",
            "command": format!("{executable} agy-hook busy"),
            "timeout": 5
        }],
        "Stop": [{
            "type": "command",
            "command": format!("{executable} agy-hook idle"),
            "timeout": 5
        }]
    })
}

fn has_mode(entry: &Value, event: &str, mode: &str) -> bool {
    entry
        .get(event)
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("type").and_then(Value::as_str) == Some("command")
                    && hook
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.ends_with(&format!(" agy-hook {mode}")))
            })
        })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn load_hooks(path: &Path) -> Result<Value, CoordinationError> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Value::Object(Map::new()));
        }
        Err(error) => return Err(CoordinationError::Io(error)),
    };
    let value = serde_json::from_str::<Value>(&raw).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to parse Antigravity hooks '{}': {error}",
            path.display()
        ))
    })?;
    if !value.is_object() {
        return Err(CoordinationError::StoreError(format!(
            "Antigravity hooks '{}' are not a JSON object",
            path.display()
        )));
    }
    Ok(value)
}

fn write_hooks(path: &Path, value: &Value) -> Result<(), CoordinationError> {
    let parent = path.parent().ok_or_else(|| {
        CoordinationError::StoreError(format!("hook path '{}' has no parent", path.display()))
    })?;
    fs::create_dir_all(parent)?;
    let payload = serde_json::to_vec_pretty(value).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize Antigravity hooks '{}': {error}",
            path.display()
        ))
    })?;
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, payload)?;
    fs::rename(&staged, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_hooks_installer_is_opt_in_idempotent_and_removable() {
        // Regression: commit 6fe0aa3 only reconciled Codex hooks; agy's
        // unverified trust-gated hooks must be explicit and preserve neighbors.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        let app_dir = root.join("antigravity-cli");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(
            app_dir.join("hooks.json"),
            r#"{"foreign":{"Stop":[{"type":"command","command":"foreign"}]}}"#,
        )
        .unwrap();
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());
        assert!(!ensure_agy_hooks_installed_at(&root, &executable).unwrap());
        assert!(agy_hooks_installed_at(&root));
        assert!(remove_agy_hooks_at(&root).unwrap());
        assert!(!remove_agy_hooks_at(&root).unwrap());
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(app_dir.join("hooks.json")).unwrap()).unwrap();
        assert!(value.get("foreign").is_some());
        assert!(value.get("taurhaus").is_none());
    }
}
