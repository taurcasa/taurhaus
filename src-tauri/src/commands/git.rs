use tauri::State;

use crate::commands::projects::DbState;
use crate::db::queries;
use crate::errors::sanitize_error;
use crate::models::{Commit, GitStatus};
use crate::ProviderState;

/// Look up a project's path from the DB, releasing the lock immediately.
fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
    // conn (MutexGuard) drops here — lock released before any git work
}

fn get_remote_fetch_url(repo: &git2::Repository) -> Option<String> {
    if let Ok(origin) = repo.find_remote("origin") {
        if let Some(url) = origin.url() {
            return Some(url.to_string());
        }
    }

    let remotes = repo.remotes().ok()?;
    for name in remotes.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                return Some(url.to_string());
            }
        }
    }

    None
}

fn strip_trailing_dot_git(url: &str) -> String {
    let mut normalized = url.trim_end_matches('/').to_string();
    if let Some(stripped) = normalized.strip_suffix(".git") {
        normalized = stripped.to_string();
    }
    normalized
}

fn normalize_remote_url(raw_url: &str) -> Option<String> {
    let url = raw_url.trim();
    if url.is_empty()
        || url.starts_with("file://")
        || url.starts_with('/')
        || url.starts_with("./")
        || url.starts_with("../")
    {
        return None;
    }

    if url.starts_with("https://") || url.starts_with("http://") {
        return Some(strip_trailing_dot_git(url));
    }

    if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        let clean_path = path.trim_start_matches('/');
        if host.is_empty() || clean_path.is_empty() {
            return None;
        }
        return Some(strip_trailing_dot_git(&format!(
            "https://{host}/{clean_path}"
        )));
    }

    if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        let host_port = authority
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(authority);
        let host = host_port
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(host_port);
        let clean_path = path.trim_start_matches('/');
        if host.is_empty() || clean_path.is_empty() {
            return None;
        }
        return Some(strip_trailing_dot_git(&format!(
            "https://{host}/{clean_path}"
        )));
    }

    None
}

fn resolve_normalized_remote_url(repo: &git2::Repository) -> Option<String> {
    get_remote_fetch_url(repo).and_then(|raw| normalize_remote_url(&raw))
}

#[tauri::command]
pub fn get_recent_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .recent_commits(&path, limit.unwrap_or(10).min(500))
        .map_err(|e| sanitize_error(&e.to_string()))
}

#[tauri::command]
pub fn get_all_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .all_commits(&path, limit.unwrap_or(50).min(500), offset.unwrap_or(0))
        .map_err(|e| sanitize_error(&e.to_string()))
}

#[tauri::command]
pub fn get_git_status(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<GitStatus, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .git_status(&path)
        .map_err(|e| sanitize_error(&e.to_string()))
}

#[tauri::command]
pub fn get_remote_url(db: State<'_, DbState>, project_id: String) -> Result<Option<String>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let repo = git2::Repository::open(&path).map_err(|e| sanitize_error(&e.to_string()))?;
    Ok(resolve_normalized_remote_url(&repo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, git2::Repository) {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        (dir, repo)
    }

    #[test]
    fn normalize_remote_url_converts_scp_style_ssh_to_https() {
        assert_eq!(
            normalize_remote_url("git@github.com:user/repo.git"),
            Some("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn normalize_remote_url_converts_ssh_url_to_https() {
        assert_eq!(
            normalize_remote_url("ssh://git@github.com/user/repo.git"),
            Some("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn normalize_remote_url_strips_dot_git_from_https() {
        assert_eq!(
            normalize_remote_url("https://github.com/user/repo.git"),
            Some("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn normalize_remote_url_rejects_non_web_remote() {
        assert_eq!(normalize_remote_url("file:///tmp/repo"), None);
        assert_eq!(normalize_remote_url("/tmp/repo"), None);
    }

    #[test]
    fn resolve_normalized_remote_url_prefers_origin() {
        let (_dir, repo) = init_repo();
        repo.remote("upstream", "https://github.com/example/upstream.git")
            .unwrap();
        repo.remote("origin", "git@github.com:example/origin.git")
            .unwrap();

        assert_eq!(
            resolve_normalized_remote_url(&repo),
            Some("https://github.com/example/origin".to_string())
        );
    }

    #[test]
    fn resolve_normalized_remote_url_falls_back_to_first_fetch_remote() {
        let (_dir, repo) = init_repo();
        repo.remote("upstream", "https://gitlab.com/example/project.git")
            .unwrap();

        assert_eq!(
            resolve_normalized_remote_url(&repo),
            Some("https://gitlab.com/example/project".to_string())
        );
    }

    #[test]
    fn resolve_normalized_remote_url_returns_none_when_repo_has_no_remotes() {
        let (_dir, repo) = init_repo();
        assert_eq!(resolve_normalized_remote_url(&repo), None);
    }
}
