pub mod daemon_client;
pub mod local;
pub mod path;

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::errors::AppError;
use crate::models::{Commit, CommitFile, DiffHunk, FileContent, FileTreeNode, GitStatus};

/// Abstraction over filesystem and git operations for a project.
///
/// Two implementations exist:
/// - `LocalProvider`: direct filesystem access (fast for Windows-local projects)
/// - `DaemonProvider`: TCP client to the WSL daemon (fast for WSL projects)
///
/// The IPC command layer resolves the correct provider per-project based on
/// `path::is_wsl_path()`.
pub trait ProjectProvider: Send + Sync {
    // -- Git --

    fn git_status(&self, project_path: &str) -> Result<GitStatus, AppError>;

    fn recent_commits(&self, project_path: &str, limit: usize) -> Result<Vec<Commit>, AppError>;

    fn all_commits(
        &self,
        project_path: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Commit>, AppError>;

    fn latest_commit_time(&self, project_path: &str) -> Result<Option<DateTime<Utc>>, AppError>;

    /// Get commits and files changed within a time range (RFC 3339 strings).
    fn commits_in_range(
        &self,
        project_path: &str,
        after: &str,
        before: &str,
    ) -> Result<(Vec<Commit>, Vec<String>), AppError>;

    /// Get files changed by a specific commit (identified by hash prefix).
    fn commit_files(&self, project_path: &str, hash: &str) -> Result<Vec<CommitFile>, AppError>;

    /// Get diff hunks for a specific file in a specific commit.
    fn commit_diff(
        &self,
        project_path: &str,
        hash: &str,
        file_path: &str,
    ) -> Result<Vec<DiffHunk>, AppError>;

    // -- Files --

    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>, AppError>;

    fn read_file(&self, project_path: &str, relative_path: &str) -> Result<FileContent, AppError>;

    fn read_readme(&self, project_path: &str) -> Result<Option<FileContent>, AppError>;

    fn read_asset(&self, project_path: &str, relative_path: &str) -> Result<Vec<u8>, AppError>;

    // -- Session file discovery --

    fn scan_session_files(&self, project_path: &str) -> Result<Vec<PathBuf>, AppError>;
}

/// Resolve the correct provider for a given project path.
///
/// WSL paths (`\\wsl$\...`, `\\wsl.localhost\...`) route through the daemon
/// if it's connected. Everything else (and WSL paths when daemon is down) use
/// the local provider.
pub fn provider_for<'a>(
    project_path: &str,
    local: &'a dyn ProjectProvider,
    daemon: Option<&'a dyn ProjectProvider>,
) -> &'a dyn ProjectProvider {
    if path::is_wsl_path(project_path) {
        if let Some(d) = daemon {
            return d;
        }
        tracing::warn!(
            path = project_path,
            "WSL path but no daemon — falling back to local I/O (slow)"
        );
    }
    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Commit, FileContent, FileTreeNode, GitStatus};
    use chrono::{DateTime, Utc};
    use std::path::PathBuf;

    /// Stub provider that returns a fixed branch name so we can identify which was used.
    struct StubProvider {
        name: &'static str,
    }

    impl ProjectProvider for StubProvider {
        fn git_status(&self, _: &str) -> Result<GitStatus, crate::errors::AppError> {
            Ok(GitStatus {
                branch: Some(self.name.to_string()),
                is_dirty: false,
                ahead: 0,
                behind: 0,
            })
        }
        fn recent_commits(
            &self,
            _: &str,
            _: usize,
        ) -> Result<Vec<Commit>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn all_commits(
            &self,
            _: &str,
            _: usize,
            _: usize,
        ) -> Result<Vec<Commit>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn latest_commit_time(
            &self,
            _: &str,
        ) -> Result<Option<DateTime<Utc>>, crate::errors::AppError> {
            Ok(None)
        }
        fn commits_in_range(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<(Vec<Commit>, Vec<String>), crate::errors::AppError> {
            Ok((vec![], vec![]))
        }
        fn commit_files(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<CommitFile>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn commit_diff(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<Vec<DiffHunk>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn file_tree(&self, _: &str) -> Result<Vec<FileTreeNode>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn read_file(&self, _: &str, _: &str) -> Result<FileContent, crate::errors::AppError> {
            Ok(FileContent {
                path: String::new(),
                content: String::new(),
                language: None,
            })
        }
        fn read_readme(&self, _: &str) -> Result<Option<FileContent>, crate::errors::AppError> {
            Ok(None)
        }
        fn read_asset(&self, _: &str, _: &str) -> Result<Vec<u8>, crate::errors::AppError> {
            Ok(vec![])
        }
        fn scan_session_files(&self, _: &str) -> Result<Vec<PathBuf>, crate::errors::AppError> {
            Ok(vec![])
        }
    }

    #[test]
    fn routes_windows_path_to_local() {
        let local = StubProvider { name: "local" };
        let daemon = StubProvider { name: "daemon" };
        let provider = provider_for(r"C:\Users\me\projects\app", &local, Some(&daemon));
        let status = provider.git_status("").unwrap();
        assert_eq!(status.branch.as_deref(), Some("local"));
    }

    #[test]
    fn routes_linux_path_to_local() {
        let local = StubProvider { name: "local" };
        let daemon = StubProvider { name: "daemon" };
        let provider = provider_for("/home/user/projects/app", &local, Some(&daemon));
        let status = provider.git_status("").unwrap();
        assert_eq!(status.branch.as_deref(), Some("local"));
    }

    #[test]
    fn routes_wsl_path_to_daemon() {
        let local = StubProvider { name: "local" };
        let daemon = StubProvider { name: "daemon" };
        let provider = provider_for(r"\\wsl$\Ubuntu\home\user\app", &local, Some(&daemon));
        let status = provider.git_status("").unwrap();
        assert_eq!(status.branch.as_deref(), Some("daemon"));
    }

    #[test]
    fn routes_wsl_localhost_path_to_daemon() {
        let local = StubProvider { name: "local" };
        let daemon = StubProvider { name: "daemon" };
        let provider = provider_for(
            r"\\wsl.localhost\Ubuntu\home\user\app",
            &local,
            Some(&daemon),
        );
        let status = provider.git_status("").unwrap();
        assert_eq!(status.branch.as_deref(), Some("daemon"));
    }

    #[test]
    fn wsl_path_falls_back_to_local_without_daemon() {
        let local = StubProvider { name: "local" };
        let provider = provider_for(r"\\wsl$\Ubuntu\home\user\app", &local, None);
        let status = provider.git_status("").unwrap();
        assert_eq!(status.branch.as_deref(), Some("local"));
    }
}
