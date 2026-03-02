use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::errors::AppError;
use crate::fs::{reader, readme, tree};
use crate::git::{commits, status};
use crate::models::{Commit, CommitFile, DiffHunk, FileContent, FileTreeNode, GitStatus};

use super::ProjectProvider;

/// Provider that performs all operations via direct local filesystem access.
///
/// Used for Windows-local projects and as the fallback for WSL projects when
/// the daemon is unavailable.
pub struct LocalProvider;

impl ProjectProvider for LocalProvider {
    fn git_status(&self, project_path: &str) -> Result<GitStatus, AppError> {
        status::get_status(Path::new(project_path))
    }

    fn recent_commits(&self, project_path: &str, limit: usize) -> Result<Vec<Commit>, AppError> {
        commits::get_recent_commits(Path::new(project_path), limit)
    }

    fn all_commits(
        &self,
        project_path: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Commit>, AppError> {
        commits::get_all_commits(Path::new(project_path), limit, offset)
    }

    fn latest_commit_time(&self, project_path: &str) -> Result<Option<DateTime<Utc>>, AppError> {
        Ok(commits::get_latest_commit_time(Path::new(project_path)))
    }

    fn commits_in_range(
        &self,
        project_path: &str,
        after: &str,
        before: &str,
    ) -> Result<(Vec<Commit>, Vec<String>), AppError> {
        let path = Path::new(project_path);
        let after_dt = chrono::DateTime::parse_from_rfc3339(after)
            .map_err(|e| AppError::InvalidPath(format!("Bad 'after' timestamp: {e}")))?
            .with_timezone(&chrono::Utc);
        let before_dt = chrono::DateTime::parse_from_rfc3339(before)
            .map_err(|e| AppError::InvalidPath(format!("Bad 'before' timestamp: {e}")))?
            .with_timezone(&chrono::Utc);
        let range_commits = commits::get_commits_in_range(path, after_dt, before_dt)?;
        let files = commits::get_files_changed_in_range(path, after_dt, before_dt)?;
        Ok((range_commits, files))
    }

    fn commit_files(&self, project_path: &str, hash: &str) -> Result<Vec<CommitFile>, AppError> {
        commits::get_commit_files(Path::new(project_path), hash)
    }

    fn commit_diff(
        &self,
        project_path: &str,
        hash: &str,
        file_path: &str,
    ) -> Result<Vec<DiffHunk>, AppError> {
        commits::get_commit_diff(Path::new(project_path), hash, file_path)
    }

    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>, AppError> {
        tree::build_file_tree(Path::new(project_path))
    }

    fn read_file(&self, project_path: &str, relative_path: &str) -> Result<FileContent, AppError> {
        reader::read_file(Path::new(project_path), relative_path)
    }

    fn read_readme(&self, project_path: &str) -> Result<Option<FileContent>, AppError> {
        readme::find_readme(Path::new(project_path))
    }

    fn read_asset(&self, project_path: &str, relative_path: &str) -> Result<Vec<u8>, AppError> {
        let root = Path::new(project_path);
        let full_path = root.join(relative_path);

        // Security: ensure resolved path is within the project directory
        let canonical_root = root
            .canonicalize()
            .map_err(|e| AppError::InvalidPath(format!("Cannot resolve project root: {e}")))?;
        let canonical_file = full_path
            .canonicalize()
            .map_err(|e| AppError::NotFound(format!("Asset not found: {relative_path} ({e})")))?;
        if !canonical_file.starts_with(&canonical_root) {
            return Err(AppError::InvalidPath(
                "Access denied: path traversal detected".to_string(),
            ));
        }

        std::fs::read(&canonical_file).map_err(AppError::Io)
    }

    fn scan_session_files(&self, project_path: &str) -> Result<Vec<PathBuf>, AppError> {
        let handoffs_dir = Path::new(project_path).join(".claude").join("handoffs");
        if !handoffs_dir.is_dir() {
            return Ok(vec![]);
        }

        let mut files = Vec::new();
        for entry in std::fs::read_dir(&handoffs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        (dir, repo)
    }

    fn create_commit(repo: &Repository, message: &str) {
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }

    #[test]
    fn local_git_status_works() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let status = provider.git_status(path).unwrap();
        assert!(status.branch.is_some());
        assert!(!status.is_dirty);
    }

    #[test]
    fn local_recent_commits_works() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First");
        create_commit(&repo, "Second");

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let commits = provider.recent_commits(path, 10).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Second");
    }

    #[test]
    fn local_all_commits_with_offset() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First");
        create_commit(&repo, "Second");
        create_commit(&repo, "Third");

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let commits = provider.all_commits(path, 2, 1).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Second");
    }

    #[test]
    fn local_latest_commit_time_works() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let time = provider.latest_commit_time(path).unwrap();
        assert!(time.is_some());
    }

    #[test]
    fn local_file_tree_works() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "world").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/nested.rs"), "code").unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let tree = provider.file_tree(path).unwrap();
        assert!(!tree.is_empty());
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"sub"));
    }

    #[test]
    fn local_read_file_works() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let content = provider.read_file(path, "test.rs").unwrap();
        assert_eq!(content.content, "fn main() {}");
        assert_eq!(content.language, Some("rust".to_string()));
    }

    #[test]
    fn local_read_readme_works() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let readme = provider.read_readme(path).unwrap();
        assert!(readme.is_some());
        assert_eq!(readme.unwrap().content, "# Hello");
    }

    #[test]
    fn local_read_readme_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let readme = provider.read_readme(path).unwrap();
        assert!(readme.is_none());
    }

    #[test]
    fn local_read_asset_works() {
        let dir = TempDir::new().unwrap();
        let data = vec![0x89, 0x50, 0x4e, 0x47]; // PNG magic bytes
        std::fs::write(dir.path().join("icon.png"), &data).unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let bytes = provider.read_asset(path, "icon.png").unwrap();
        assert_eq!(bytes, data);
    }

    #[test]
    fn local_read_asset_rejects_traversal() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("ok.txt"), "safe").unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        // ".." is caught by canonicalization check
        let result = provider.read_asset(path, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn local_scan_session_files_works() {
        let dir = TempDir::new().unwrap();
        let handoffs = dir.path().join(".claude").join("handoffs");
        std::fs::create_dir_all(&handoffs).unwrap();
        std::fs::write(handoffs.join("2025-01-15-session.md"), "# Session").unwrap();
        std::fs::write(handoffs.join("2025-01-16-session.md"), "# Session 2").unwrap();
        std::fs::write(handoffs.join("metadata.json"), "{}").unwrap(); // not .md

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let files = provider.scan_session_files(path).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn local_scan_session_files_empty_when_no_dir() {
        let dir = TempDir::new().unwrap();

        let provider = LocalProvider;
        let path = dir.path().to_str().unwrap();
        let files = provider.scan_session_files(path).unwrap();
        assert!(files.is_empty());
    }
}
