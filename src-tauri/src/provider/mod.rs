pub mod daemon_client;
pub mod local;
pub mod path;

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::errors::AppError;
use crate::models::{Commit, FileContent, FileTreeNode, GitStatus};

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

    fn recent_commits(
        &self,
        project_path: &str,
        limit: usize,
    ) -> Result<Vec<Commit>, AppError>;

    fn all_commits(
        &self,
        project_path: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Commit>, AppError>;

    fn latest_commit_time(
        &self,
        project_path: &str,
    ) -> Result<Option<DateTime<Utc>>, AppError>;

    // -- Files --

    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>, AppError>;

    fn read_file(
        &self,
        project_path: &str,
        relative_path: &str,
    ) -> Result<FileContent, AppError>;

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
    _daemon: Option<&'a dyn ProjectProvider>,
) -> &'a dyn ProjectProvider {
    if path::is_wsl_path(project_path) {
        if let Some(daemon) = _daemon {
            return daemon;
        }
        // Fallback: direct I/O (slow but works)
    }
    local
}
