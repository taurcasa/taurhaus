//! Bounded, compare-before-write access to mesh-owned task records.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use crate::coordination::errors::CoordinationError;

const MAX_TASK_RECORD_BYTES: u64 = 1_048_576;

/// Change a task status only while its identity, owner, and previous status
/// still match the caller's observation.
pub(crate) fn commit_status_if_unchanged(
    teams_dir: &Path,
    team: &str,
    member: &str,
    task_id: &str,
    expected_status: &str,
    new_status: &str,
) -> Result<(), CoordinationError> {
    let path = task_path(teams_dir, team, task_id)?;
    let target_lock = super::lock::TargetFileLock::acquire_if_exists(&path)?.ok_or_else(|| {
        CoordinationError::NotFound(format!("mesh task '{task_id}' not found for team '{team}'"))
    })?;
    if fs::metadata(&path)?.len() > MAX_TASK_RECORD_BYTES {
        return Err(CoordinationError::Validation(format!(
            "mesh task '{task_id}' exceeds the 1 MiB record limit"
        )));
    }
    let raw = target_lock.read_contents()?;
    let mut task: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to parse mesh task '{task_id}' for team '{team}': {error}"
        ))
    })?;
    if task.get("id").and_then(serde_json::Value::as_str) != Some(task_id)
        || task.get("owner").and_then(serde_json::Value::as_str) != Some(member)
        || task.get("status").and_then(serde_json::Value::as_str) != Some(expected_status)
    {
        return Err(CoordinationError::Conflict(format!(
            "mesh task '{task_id}' changed before its status update committed"
        )));
    }
    task["status"] = serde_json::Value::String(new_status.to_string());

    let payload = serde_json::to_string_pretty(&task).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize mesh task '{task_id}': {error}"
        ))
    })?;
    let tmp_path = task_tmp_path(&path);
    write_file_synced(&tmp_path, &payload, Some(&path))?;
    if let Err(error) = fs::rename(&tmp_path, &path) {
        if super::lock::is_windows_unsupported_rename_error(&error) {
            super::lock::report_atomic_write_degraded(&path, "mesh_task", error.raw_os_error());
            target_lock
                .overwrite(payload.as_bytes())
                .map_err(CoordinationError::Io)?;
            let _ = fs::remove_file(&tmp_path);
        } else {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(error));
        }
    }

    Ok(())
}

/// Whether a task record still names this owner with `in_progress` status.
///
/// This probe is deliberately tolerant: every invalid or unavailable record
/// answers `false`; the write path provides the detailed error.
pub(crate) fn is_still_open(teams_dir: &Path, team: &str, member: &str, task_id: &str) -> bool {
    let Ok(path) = task_path(teams_dir, team, task_id) else {
        return false;
    };
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };
    if metadata.len() > MAX_TASK_RECORD_BYTES {
        return false;
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return false;
    };
    let Ok(task) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    task.get("id").and_then(serde_json::Value::as_str) == Some(task_id)
        && task.get("owner").and_then(serde_json::Value::as_str) == Some(member)
        && task.get("status").and_then(serde_json::Value::as_str) == Some("in_progress")
}

fn task_path(teams_dir: &Path, team: &str, task_id: &str) -> Result<PathBuf, CoordinationError> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(CoordinationError::Validation(format!(
            "invalid mesh task id '{task_id}'"
        )));
    }
    let tasks_dir =
        crate::coordination::pipelines::mesh_tasks_dir(teams_dir, team).ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "coordination teams root is not canonical: {}",
                teams_dir.display()
            ))
        })?;
    Ok(tasks_dir.join(format!("{task_id}.json")))
}

fn task_tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".status.tmp");
    PathBuf::from(tmp)
}

fn write_file_synced(
    path: &Path,
    payload: &str,
    permissions_from: Option<&Path>,
) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        let mode = permissions_from
            .and_then(|source| fs::metadata(source).ok())
            .map(|metadata| metadata.permissions().mode())
            .unwrap_or(0o600);
        file.set_permissions(fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = permissions_from;
    file.write_all(payload.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::coordination::errors::CoordinationError;

    fn task_path(teams_dir: &std::path::Path, team: &str, task_id: &str) -> std::path::PathBuf {
        teams_dir
            .parent()
            .expect("teams parent")
            .join("tasks")
            .join(team)
            .join(format!("{task_id}.json"))
    }

    fn write_task(
        teams_dir: &std::path::Path,
        task_id: &str,
        record_id: &str,
        owner: &str,
        status: &str,
    ) {
        let path = task_path(teams_dir, "deadline-team", task_id);
        std::fs::create_dir_all(path.parent().expect("task parent")).expect("create tasks");
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "id": record_id,
                "subject": "Run the migration",
                "owner": owner,
                "status": status,
                "metadata": { "deadline_minutes": 20 },
            }))
            .expect("serialize task"),
        )
        .expect("write task");
    }

    #[test]
    fn commit_status_preserves_the_record_and_changes_only_status() {
        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");
        write_task(&teams_dir, "42", "42", "builder", "in_progress");

        commit_status_if_unchanged(
            &teams_dir,
            "deadline-team",
            "builder",
            "42",
            "in_progress",
            "stale",
        )
        .expect("commit task status");

        let task: serde_json::Value = serde_json::from_slice(
            &std::fs::read(task_path(&teams_dir, "deadline-team", "42")).expect("read task"),
        )
        .expect("parse task");
        assert_eq!(task["status"], "stale");
        assert_eq!(task["metadata"]["deadline_minutes"], 20);
        assert_eq!(task["subject"], "Run the migration");
    }

    #[cfg(unix)]
    #[test]
    fn deadline_status_commit_preserves_mesh_task_permissions() {
        // Regression: 008536ec replaced mesh's 0600 record with a temp file
        // created under the process umask, widening the final mode to 0644.
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");
        write_task(&teams_dir, "42", "42", "builder", "in_progress");
        let path = task_path(&teams_dir, "deadline-team", "42");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("set mesh permissions");

        commit_status_if_unchanged(
            &teams_dir,
            "deadline-team",
            "builder",
            "42",
            "in_progress",
            "stale",
        )
        .expect("commit task status");

        assert_eq!(
            std::fs::metadata(path)
                .expect("task metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn commit_status_refuses_a_moved_foreign_or_mismatched_task() {
        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");

        for (task_id, record_id, owner, status) in [
            ("moved", "moved", "builder", "completed"),
            ("foreign", "foreign", "reviewer", "in_progress"),
            ("mismatch", "another-id", "builder", "in_progress"),
        ] {
            write_task(&teams_dir, task_id, record_id, owner, status);
            let error = commit_status_if_unchanged(
                &teams_dir,
                "deadline-team",
                "builder",
                task_id,
                "in_progress",
                "stale",
            )
            .expect_err("changed task must not commit");
            assert!(matches!(error, CoordinationError::Conflict(_)));
        }
    }

    #[test]
    fn commit_status_refuses_a_missing_task_without_creating_it() {
        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");
        let path = task_path(&teams_dir, "deadline-team", "missing");

        let error = commit_status_if_unchanged(
            &teams_dir,
            "deadline-team",
            "builder",
            "missing",
            "in_progress",
            "stale",
        )
        .expect_err("missing task must not commit");

        assert!(matches!(error, CoordinationError::NotFound(_)));
        assert!(!path.exists(), "locking must not create a missing task");
    }

    #[test]
    fn commit_status_refuses_an_oversized_task_record() {
        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");
        let path = task_path(&teams_dir, "deadline-team", "42");
        std::fs::create_dir_all(path.parent().expect("task parent")).expect("create tasks");
        std::fs::write(&path, vec![b' '; 1_048_577]).expect("write oversized task");

        let error = commit_status_if_unchanged(
            &teams_dir,
            "deadline-team",
            "builder",
            "42",
            "in_progress",
            "stale",
        )
        .expect_err("oversized task must not commit");

        assert!(matches!(error, CoordinationError::Validation(_)));
        assert_eq!(
            std::fs::metadata(path).expect("task metadata").len(),
            1_048_577
        );
    }

    #[test]
    fn is_still_open_tolerates_missing_malformed_oversized_and_moved_records() {
        let root = tempfile::tempdir().expect("root");
        let teams_dir = root.path().join("teams");
        write_task(&teams_dir, "42", "42", "builder", "in_progress");
        assert!(is_still_open(&teams_dir, "deadline-team", "builder", "42"));

        write_task(&teams_dir, "42", "42", "builder", "completed");
        assert!(!is_still_open(&teams_dir, "deadline-team", "builder", "42"));
        write_task(&teams_dir, "42", "42", "reviewer", "in_progress");
        assert!(!is_still_open(&teams_dir, "deadline-team", "builder", "42"));

        let path = task_path(&teams_dir, "deadline-team", "42");
        std::fs::write(&path, b"not-json").expect("write malformed task");
        assert!(!is_still_open(&teams_dir, "deadline-team", "builder", "42"));
        std::fs::write(&path, vec![b' '; 1_048_577]).expect("write oversized task");
        assert!(!is_still_open(&teams_dir, "deadline-team", "builder", "42"));
        std::fs::remove_file(path).expect("remove task");
        assert!(!is_still_open(&teams_dir, "deadline-team", "builder", "42"));
        assert!(!is_still_open(
            &teams_dir,
            "deadline-team",
            "builder",
            "../42"
        ));
    }
}
