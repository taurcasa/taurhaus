use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::errors::AppError;
use crate::models::{
    Commit, CommitFile, DiffHunk, FileContent, FileTreeNode, GitRangeResult, GitStatus,
};

/// Abstraction over filesystem and git operations for a project.
///
/// Two implementations exist:
/// - `LocalProvider`: direct filesystem access (fast for Windows-local projects)
/// - `DaemonProvider`: TCP client to the WSL daemon (fast for WSL projects)
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
        commit_limit: Option<usize>,
    ) -> Result<GitRangeResult, AppError>;

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
    if crate::provider::path::is_wsl_path(project_path) {
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
