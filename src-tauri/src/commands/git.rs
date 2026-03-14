use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
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

fn reject_wsl_unc_remote_url_lookup(path: &str) -> Result<(), String> {
    if crate::provider::path::requires_daemon_git_trust(path) {
        return Err(sanitize_error(
            "Remote URL lookup for WSL UNC repositories requires daemon-backed git access",
        ));
    }

    Ok(())
}

#[tauri::command]
pub fn get_recent_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let span = IpcCommandSpan::start("get_recent_commits");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        fail_fast_if_foreground_daemon_lane_is_busy(&providers, &path, "recent commits load")?;
        let provider = providers.resolve(&path);
        provider
            .recent_commits(&path, limit.unwrap_or(10).min(500))
            .map_err(|e| sanitize_error(&e.to_string()))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_all_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let span = IpcCommandSpan::start("get_all_commits");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        let provider = providers.resolve(&path);
        provider
            .all_commits(&path, limit.unwrap_or(50).min(500), offset.unwrap_or(0))
            .map_err(|e| sanitize_error(&e.to_string()))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_git_status(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<GitStatus, String> {
    let span = IpcCommandSpan::start("get_git_status");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        let provider = providers.resolve(&path);
        provider
            .git_status(&path)
            .map_err(|e| sanitize_error(&e.to_string()))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_remote_url(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Option<String>, String> {
    let span = IpcCommandSpan::start("get_remote_url");
    let result = {
        let path = resolve_project_path(&db, &project_id)?;
        reject_wsl_unc_remote_url_lookup(&path)?;
        let repo = git2::Repository::open(&path).map_err(|e| sanitize_error(&e.to_string()))?;
        Ok(resolve_normalized_remote_url(&repo))
    };
    span.finish_result(&result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::daemon_client::DaemonProvider;
    use crate::provider::local::LocalProvider;
    use crate::ProviderState;
    use pretty_assertions::assert_eq;
    use std::net::TcpListener;
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

    #[test]
    fn reject_wsl_unc_remote_url_lookup_requires_daemon_trust_path() {
        let error = reject_wsl_unc_remote_url_lookup(r"\\wsl.localhost\Ubuntu\home\user\repo")
            .expect_err("WSL UNC remote URL lookup should require daemon-backed git access");
        assert!(error.contains("daemon-backed git access"));
    }

    #[test]
    fn recent_commits_foreground_read_fails_fast_when_daemon_lane_is_busy() {
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
                let request = crate::daemon_api::protocol::DaemonRequest::ping("busy-git");
                let _ = provider.send_status_request(&request);
            });
            std::thread::sleep(std::time::Duration::from_millis(100));

            let err = fail_fast_if_foreground_daemon_lane_is_busy(
                &providers,
                r"\\wsl.localhost\Ubuntu\home\mstie\projects\taurhaus",
                "recent commits load",
            )
            .expect_err("busy WSL foreground load should fail fast");
            assert!(err.to_lowercase().contains("daemon transport error"));
            assert!(err.to_lowercase().contains("busy"));
        });
        accept_thread.join().expect("accept thread joined");
    }
}
