use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::coordination::errors::CoordinationError;

/// The shared user-level hooks file agy 1.0.8 moved to and the TUI edits.
const HOOKS_FILE: &str = "config/hooks.json";
/// The pre-1.0.8 app-data file. agy still loads it as a second source, and its
/// one-shot migration replaces it with a symlink onto the shared file.
const LEGACY_HOOKS_FILE: &str = "antigravity-cli/hooks.json";
const TAURHAUS_HOOK: &str = "taurhaus";

fn hook_executable_probe_path(agy_root: &Path, daemon_executable: &Path) -> PathBuf {
    let executable = daemon_executable.to_string_lossy();
    if executable.starts_with('/') {
        crate::provider::path::wsl_distro_from_path(&agy_root.to_string_lossy())
            .map(|distro| {
                PathBuf::from(crate::provider::path::linux_to_wsl_unc(
                    &executable,
                    &distro,
                ))
            })
            .unwrap_or_else(|| daemon_executable.to_path_buf())
    } else {
        daemon_executable.to_path_buf()
    }
}

pub fn ensure_agy_hooks_installed_at(
    agy_root: &Path,
    daemon_executable: &Path,
) -> Result<bool, CoordinationError> {
    let probe_path = hook_executable_probe_path(agy_root, daemon_executable);
    if !probe_path.is_file() {
        let changed = remove_agy_hooks_at(agy_root)?;
        let mut fields = Map::new();
        fields.insert(
            "executable".to_string(),
            Value::String(daemon_executable.to_string_lossy().into_owned()),
        );
        fields.insert(
            "probe_path".to_string(),
            Value::String(probe_path.to_string_lossy().into_owned()),
        );
        taurhaus_lib::logging::emit_global(
            "warn",
            "coordination",
            "agy.hooks.degraded",
            Some("Antigravity hook executable is unavailable; removed the hook".to_string()),
            fields,
        );
        return Ok(changed);
    }
    let path = hooks_path(agy_root);
    let mut root = load_hooks(&path)?;
    let original = root.clone();
    root.as_object_mut()
        .expect("hook root was validated")
        .insert(TAURHAUS_HOOK.to_string(), taurhaus_entry(daemon_executable));
    let changed = root != original;
    if changed {
        write_hooks(&path, &root)?;
    }
    Ok(prune_legacy_hooks(agy_root)? || changed)
}

pub fn remove_agy_hooks_at(agy_root: &Path) -> Result<bool, CoordinationError> {
    let changed = remove_taurhaus_entry(&hooks_path(agy_root))?;
    Ok(prune_legacy_hooks(agy_root)? || changed)
}

/// Drop any taurhaus entry left in the pre-1.0.8 file. agy reads both paths as
/// distinct sources, so an entry left behind would install our hook twice —
/// unless its own migration already symlinked the legacy path onto the shared
/// file, in which case pruning would delete the entry we just wrote.
fn prune_legacy_hooks(agy_root: &Path) -> Result<bool, CoordinationError> {
    let legacy = legacy_hooks_path(agy_root);
    if !legacy.exists() || is_same_file(&legacy, &hooks_path(agy_root)) {
        return Ok(false);
    }
    remove_taurhaus_entry(&legacy)
}

fn remove_taurhaus_entry(path: &Path) -> Result<bool, CoordinationError> {
    let mut root = load_hooks(path)?;
    let changed = root
        .as_object_mut()
        .expect("hook root was validated")
        .remove(TAURHAUS_HOOK)
        .is_some();
    if changed {
        write_hooks(path, &root)?;
    }
    Ok(changed)
}

fn is_same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Read the installed hooks back from disk. `agy -p /hooks` is not a health
/// check: print mode loads no customizations at all, trusted or not.
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

fn legacy_hooks_path(agy_root: &Path) -> PathBuf {
    agy_root.join(LEGACY_HOOKS_FILE)
}

/// Follow a symlinked hooks file to the file agy actually reads. Renaming a
/// tempfile onto the link itself would replace it with a private regular file.
fn resolve_symlink_target(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            break;
        };
        if !metadata.file_type().is_symlink() {
            break;
        }
        let Ok(target) = fs::read_link(&current) else {
            break;
        };
        current = match current.parent() {
            Some(parent) if target.is_relative() => parent.join(target),
            _ => target,
        };
    }
    current
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
    let path = &resolve_symlink_target(path);
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
        let shared_dir = root.join("config");
        std::fs::create_dir_all(&shared_dir).unwrap();
        std::fs::write(
            shared_dir.join("hooks.json"),
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
            serde_json::from_slice(&std::fs::read(shared_dir.join("hooks.json")).unwrap()).unwrap();
        assert!(value.get("foreign").is_some());
        assert!(value.get("taurhaus").is_none());
    }

    #[test]
    fn installer_writes_the_shared_hooks_file_and_keeps_foreign_entries() {
        // Regression: commit 4e9e2c5 wrote the legacy `antigravity-cli` file
        // that agy 1.0.8 replaced with the shared `config/hooks.json`.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(
            root.join(HOOKS_FILE),
            r#"{"foreign":{"Stop":[{"type":"command","command":"foreign"}]}}"#,
        )
        .unwrap();
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());
        assert!(agy_hooks_installed_at(&root));
        let value: Value =
            serde_json::from_slice(&std::fs::read(root.join(HOOKS_FILE)).unwrap()).unwrap();
        assert!(value.get("foreign").is_some());
        assert!(value.get(TAURHAUS_HOOK).is_some());
        assert!(!root.join(LEGACY_HOOKS_FILE).exists());
    }

    #[test]
    fn installer_clears_taurhaus_entries_from_the_legacy_hooks_file() {
        // Regression: commit 4e9e2c5 owned the legacy path, so moving to the
        // shared file would leave a second taurhaus entry loading behind it.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        std::fs::create_dir_all(root.join("antigravity-cli")).unwrap();
        std::fs::write(
            root.join(LEGACY_HOOKS_FILE),
            serde_json::to_vec_pretty(&json!({
                "foreign": {"Stop": [{"type": "command", "command": "foreign"}]},
                TAURHAUS_HOOK: taurhaus_entry(Path::new("/stale/taurhaus-daemon")),
            }))
            .unwrap(),
        )
        .unwrap();
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());

        let legacy: Value =
            serde_json::from_slice(&std::fs::read(root.join(LEGACY_HOOKS_FILE)).unwrap()).unwrap();
        assert!(legacy.get("foreign").is_some());
        assert!(legacy.get(TAURHAUS_HOOK).is_none());
        assert!(agy_hooks_installed_at(&root));
    }

    #[cfg(unix)]
    #[test]
    fn installer_writes_through_a_symlinked_shared_hooks_file() {
        // Regression: commit 4e9e2c5 renamed a tempfile onto the target, which
        // replaces agy's migration symlink with a private regular file.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        std::fs::create_dir_all(root.join("config")).unwrap();
        let target = temp.path().join("shared-hooks.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, root.join(HOOKS_FILE)).unwrap();
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());

        assert!(std::fs::symlink_metadata(root.join(HOOKS_FILE))
            .unwrap()
            .file_type()
            .is_symlink());
        let value: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
        assert!(value.get(TAURHAUS_HOOK).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn migrated_legacy_symlink_is_never_pruned_into_the_shared_file() {
        // Regression: agy's own migration symlinks the legacy path onto the
        // shared file, so pruning it would delete the entry just installed.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("antigravity-cli")).unwrap();
        std::fs::write(root.join(HOOKS_FILE), "{}").unwrap();
        std::os::unix::fs::symlink(root.join(HOOKS_FILE), root.join(LEGACY_HOOKS_FILE)).unwrap();
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());
        assert!(agy_hooks_installed_at(&root));
    }

    #[test]
    fn windows_agy_root_maps_linux_hook_executable_for_host_probe() {
        // Regression: commit 4e9e2c5 probed a WSL Linux executable path directly
        // from the native Windows process, so enabling agy hooks always failed.
        let agy_root = Path::new(r"\\wsl.localhost\Ubuntu\home\user\.gemini");
        let executable = Path::new("/home/user/.local/bin/taurhaus-daemon");

        assert_eq!(
            hook_executable_probe_path(agy_root, executable),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\user\.local\bin\taurhaus-daemon")
        );
    }

    #[test]
    fn missing_hook_executable_degrades_by_removing_the_entry() {
        // Regression: commit 4e9e2c5 returned an error for a missing executable,
        // leaving an already-installed hook pointing at a dead command.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".gemini");
        let executable = temp.path().join("taurhaus-daemon");
        std::fs::write(&executable, "fixture").unwrap();
        ensure_agy_hooks_installed_at(&root, &executable).unwrap();
        std::fs::remove_file(&executable).unwrap();

        assert!(ensure_agy_hooks_installed_at(&root, &executable).unwrap());
        assert!(!agy_hooks_installed_at(&root));
    }
}
