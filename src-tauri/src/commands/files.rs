use std::path::{Component, Path};

use base64::Engine as _;
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::queries;
use crate::errors::sanitize_error;
use crate::models::{FileContent, FileTreeNode};
use crate::ProviderState;

const PATH_TYPE_FILE: &str = "file";
const PATH_TYPE_DIRECTORY: &str = "directory";
const PATH_TYPE_NOT_FOUND: &str = "not_found";

/// Look up a project's path from the DB, releasing the lock immediately.
fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
}

fn fail_fast_if_foreground_daemon_lane_is_busy(
    providers: &ProviderState,
    project_path: &str,
    operation: &str,
) -> Result<(), String> {
    if crate::provider::path::is_wsl_path(project_path)
        && providers
            .daemon
            .as_ref()
            .is_some_and(|daemon| daemon.is_connected() && daemon.is_busy())
    {
        return Err(sanitize_error(&format!(
            "Daemon transport error: foreground {operation} skipped because the shared daemon connection is busy"
        )));
    }

    Ok(())
}

#[tauri::command]
pub fn get_file_tree(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<Vec<FileTreeNode>, String> {
    let span = IpcCommandSpan::start("get_file_tree");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        let provider = providers.resolve(&path);
        provider
            .file_tree(&path)
            .map_err(|e| sanitize_error(&e.to_string()))
    };
    span.finish_result(&result);
    result
}

fn classify_path_type(project_root: &Path, relative_path: &str) -> Result<&'static str, String> {
    let normalized = relative_path.replace('\\', "/");
    let trimmed = normalized.trim();

    // Empty path means "project root", which is always a directory.
    if trimmed.is_empty() || trimmed == "." {
        return Ok(PATH_TYPE_DIRECTORY);
    }

    let rel = Path::new(trimmed);

    if rel.is_absolute() {
        return Ok(PATH_TYPE_NOT_FOUND);
    }

    // Reject traversal/absolute-like components and report as not_found.
    if rel.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Ok(PATH_TYPE_NOT_FOUND);
    }

    let candidate = project_root.join(rel);
    if !candidate.exists() {
        return Ok(PATH_TYPE_NOT_FOUND);
    }

    let canonical_root = project_root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve project root: {e}"))?;

    let canonical_candidate = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => return Ok(PATH_TYPE_NOT_FOUND),
    };

    // Guard against symlink escapes.
    if !canonical_candidate.starts_with(&canonical_root) {
        return Ok(PATH_TYPE_NOT_FOUND);
    }

    let metadata = std::fs::metadata(&canonical_candidate).map_err(|e| e.to_string())?;
    if metadata.is_file() {
        Ok(PATH_TYPE_FILE)
    } else if metadata.is_dir() {
        Ok(PATH_TYPE_DIRECTORY)
    } else {
        Ok(PATH_TYPE_NOT_FOUND)
    }
}

#[tauri::command]
pub fn read_file(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    relative_path: String,
) -> Result<FileContent, String> {
    let span = IpcCommandSpan::start("read_file");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        // Normalize backslashes — search index on Windows may store paths with
        // backslashes (e.g. "tests\test_integration.py") that the Linux daemon
        // can't resolve. Belt-and-suspenders with the indexer normalization.
        let relative_path = relative_path.replace('\\', "/");
        let provider = providers.resolve(&path);
        provider
            .read_file(&path, &relative_path)
            .map_err(|e| sanitize_error(&e.to_string()))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn check_path_type(
    db: State<'_, DbState>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let span = IpcCommandSpan::start("check_path_type");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        classify_path_type(Path::new(&path), &relative_path)
            .map(|kind| kind.to_string())
            .map_err(|e| sanitize_error(&e))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_readme(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<Option<FileContent>, String> {
    let span = IpcCommandSpan::start("get_readme");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        fail_fast_if_foreground_daemon_lane_is_busy(&providers, &path, "README load")?;
        let is_wsl = crate::provider::path::is_wsl_path(&path);
        let has_daemon = providers.daemon.as_ref().is_some_and(|d| d.is_connected());
        let using_daemon = is_wsl && has_daemon;
        tracing::debug!(
            project_id,
            path,
            is_wsl,
            has_daemon,
            using_daemon,
            "get_readme: resolving provider"
        );
        let provider = providers.resolve(&path);
        let result = provider
            .read_readme(&path)
            .map_err(|e| sanitize_error(&e.to_string()))?;
        if let Some(ref content) = result {
            tracing::debug!(
                project_id,
                readme_path = content.path,
                content_len = content.content.len(),
                content_preview = &content.content[..content.content.len().min(80)],
                "get_readme: returning content"
            );
        } else {
            tracing::debug!(project_id, "get_readme: no README found");
        }
        Ok(result)
    };
    span.finish_result(&result);
    result
}

/// Read a binary file from a project directory and return it as a base64 data URI.
/// Used for rendering images embedded in markdown READMEs.
#[tauri::command]
pub fn read_project_asset(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let span = IpcCommandSpan::start("read_project_asset");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        let relative_path = relative_path.replace('\\', "/");
        let provider = providers.resolve(&path);
        let bytes = provider
            .read_asset(&path, &relative_path)
            .map_err(|e| sanitize_error(&e.to_string()))?;

        let mime = mime_from_extension(&relative_path);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{mime};base64,{b64}"))
    };
    span.finish_result(&result);
    result
}

fn mime_from_extension(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::daemon_client::DaemonProvider;
    use crate::provider::local::LocalProvider;
    use crate::ProviderState;
    use std::net::TcpListener;
    use tempfile::TempDir;

    #[test]
    fn classify_path_type_returns_file_for_file() {
        let dir = TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("README.md"), "# test").expect("write file");

        let kind = classify_path_type(dir.path(), "README.md").expect("classify");
        assert_eq!(kind, PATH_TYPE_FILE);
    }

    #[test]
    fn classify_path_type_returns_directory_for_directory() {
        let dir = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("docs")).expect("mkdir docs");

        let kind = classify_path_type(dir.path(), "docs").expect("classify");
        assert_eq!(kind, PATH_TYPE_DIRECTORY);
    }

    #[test]
    fn classify_path_type_returns_not_found_for_missing_path() {
        let dir = TempDir::new().expect("tempdir");
        let kind = classify_path_type(dir.path(), "missing.md").expect("classify");
        assert_eq!(kind, PATH_TYPE_NOT_FOUND);
    }

    #[test]
    fn classify_path_type_returns_not_found_for_parent_traversal() {
        let dir = TempDir::new().expect("tempdir");
        let kind = classify_path_type(dir.path(), "../outside.md").expect("classify");
        assert_eq!(kind, PATH_TYPE_NOT_FOUND);
    }

    #[test]
    fn classify_path_type_returns_not_found_for_absolute_path() {
        let dir = TempDir::new().expect("tempdir");
        let kind = classify_path_type(dir.path(), "/etc/passwd").expect("classify");
        assert_eq!(kind, PATH_TYPE_NOT_FOUND);
    }

    #[test]
    fn readme_foreground_read_fails_fast_when_daemon_lane_is_busy() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let accept_thread = std::thread::spawn(move || {
            let _stream = listener.accept().expect("accept client");
            std::thread::sleep(std::time::Duration::from_secs(2));
        });
        let providers = ProviderState {
            local: LocalProvider,
            daemon: Some(DaemonProvider::connect(&addr.to_string()).unwrap()),
            wsl_distro: Some("Ubuntu".to_string()),
        };

        std::thread::scope(|scope| {
            let provider = providers.daemon.as_ref().expect("daemon provider");
            let _busy_thread = scope.spawn(|| {
                let request = crate::daemon_api::protocol::DaemonRequest::ping("busy-readme");
                let _ = provider.send_status_request(&request);
            });
            std::thread::sleep(std::time::Duration::from_millis(100));

            let err = fail_fast_if_foreground_daemon_lane_is_busy(
                &providers,
                r"\\wsl.localhost\Ubuntu\home\mstie\projects\taurhaus",
                "README load",
            )
            .expect_err("busy WSL foreground README load should fail fast");
            assert!(err.to_lowercase().contains("daemon transport error"));
            assert!(err.to_lowercase().contains("busy"));
        });
        accept_thread.join().expect("accept thread joined");
    }

    #[test]
    fn classify_path_type_treats_empty_path_as_directory() {
        let dir = TempDir::new().expect("tempdir");
        let kind = classify_path_type(dir.path(), "").expect("classify");
        assert_eq!(kind, PATH_TYPE_DIRECTORY);
    }

    #[cfg(unix)]
    #[test]
    fn classify_path_type_returns_not_found_for_symlink_escape() {
        let dir = TempDir::new().expect("tempdir");
        std::os::unix::fs::symlink("/tmp", dir.path().join("escape")).expect("create symlink");

        let kind = classify_path_type(dir.path(), "escape").expect("classify");
        assert_eq!(kind, PATH_TYPE_NOT_FOUND);
    }
}
