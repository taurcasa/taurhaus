pub mod daemon_client;
pub mod local;
pub mod path;

pub use crate::project_provider::{provider_for, ProjectProvider};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        Commit, CommitFile, DiffHunk, FileContent, FileTreeNode, GitRangeResult, GitStatus,
    };
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
            _: Option<usize>,
        ) -> Result<GitRangeResult, crate::errors::AppError> {
            Ok(GitRangeResult {
                commits: vec![],
                files: vec![],
                truncated: false,
                total_count: None,
            })
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
