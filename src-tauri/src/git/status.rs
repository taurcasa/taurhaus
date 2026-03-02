use std::path::Path;

use git2::Repository;

use crate::errors::AppError;
use crate::models::GitStatus;

/// Get the current git status of a repository.
pub fn get_status(repo_path: &Path) -> Result<GitStatus, AppError> {
    let repo = Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!(
            "Not a git repository: {}: {e}",
            repo_path.display()
        ))
    })?;

    let branch = get_branch_name(&repo);
    let is_dirty = check_dirty(&repo);

    Ok(GitStatus {
        branch,
        is_dirty,
        ahead: 0,
        behind: 0,
    })
}

/// Get the current branch name. Returns None for detached HEAD or empty repos.
fn get_branch_name(repo: &Repository) -> Option<String> {
    if repo.head_detached().unwrap_or(false) {
        // Detached HEAD — return short SHA
        return repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| format!("{:.8}", c.id()));
    }

    repo.head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
}

/// Check if the working tree has any modifications.
fn check_dirty(repo: &Repository) -> bool {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false);

    repo.statuses(Some(&mut opts))
        .map(|statuses| !statuses.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
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
    fn status_clean_repo() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        let status = get_status(dir.path()).unwrap();
        // Branch name depends on git/libgit2 defaults (main or master)
        assert!(status.branch.is_some());
        let branch = status.branch.unwrap();
        assert!(
            branch == "main" || branch == "master",
            "branch was: {branch}"
        );
        assert!(!status.is_dirty);
    }

    #[test]
    fn status_dirty_repo() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        // Create an untracked file
        std::fs::write(dir.path().join("new_file.txt"), "hello").unwrap();

        let status = get_status(dir.path()).unwrap();
        assert!(status.is_dirty);
    }

    #[test]
    fn status_modified_file() {
        let (dir, repo) = init_test_repo();

        // Create and commit a file
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "original").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.write().unwrap();

        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[])
            .unwrap();

        // Modify the file
        std::fs::write(&file_path, "modified").unwrap();

        let status = get_status(dir.path()).unwrap();
        assert!(status.is_dirty);
    }

    #[test]
    fn status_detached_head() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        // Detach HEAD
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        repo.set_head_detached(commit.id()).unwrap();

        let status = get_status(dir.path()).unwrap();
        // Branch should be the short SHA
        assert!(status.branch.is_some());
        let branch = status.branch.unwrap();
        assert_eq!(branch.len(), 8);
    }

    #[test]
    fn status_empty_repo() {
        let (dir, _repo) = init_test_repo();
        // No commits — HEAD doesn't exist yet
        let status = get_status(dir.path()).unwrap();
        assert_eq!(status.branch, None);
        assert!(!status.is_dirty);
    }

    #[test]
    fn status_not_a_repo() {
        let dir = TempDir::new().unwrap();
        let result = get_status(dir.path());
        assert!(result.is_err());
    }
}
