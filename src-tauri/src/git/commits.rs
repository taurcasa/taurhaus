use std::path::Path;

use chrono::{DateTime, Utc};
use git2::Repository;

use crate::errors::AppError;
use crate::models::Commit;

/// Get recent commits from a git repository, newest first.
pub fn get_recent_commits(repo_path: &Path, limit: usize) -> Result<Vec<Commit>, AppError> {
    let repo = Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!("Not a git repository: {}: {e}", repo_path.display()))
    })?;

    let mut revwalk = repo.revwalk().map_err(git_err)?;
    revwalk.push_head().map_err(|_| {
        // No HEAD means no commits yet
        AppError::NotFound("No commits".into())
    })?;
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(git_err)?;

    let now = Utc::now();
    let mut commits = Vec::with_capacity(limit);

    for oid_result in revwalk.take(limit) {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;

        let timestamp = commit.time().seconds();
        let date = format_relative_time(timestamp, now);
        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        commits.push(Commit {
            hash: format!("{:.8}", oid),
            message,
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date,
        });
    }

    Ok(commits)
}

/// Get all commits with pagination (offset + limit), newest first.
pub fn get_all_commits(
    repo_path: &Path,
    limit: usize,
    offset: usize,
) -> Result<Vec<Commit>, AppError> {
    let repo = Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!("Not a git repository: {}: {e}", repo_path.display()))
    })?;

    let mut revwalk = repo.revwalk().map_err(git_err)?;
    if revwalk.push_head().is_err() {
        return Ok(vec![]);
    }
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(git_err)?;

    let now = Utc::now();
    let mut commits = Vec::with_capacity(limit);

    for oid_result in revwalk.skip(offset).take(limit) {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;

        let timestamp = commit.time().seconds();
        let date = format_relative_time(timestamp, now);
        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        commits.push(Commit {
            hash: format!("{:.8}", oid),
            message,
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date,
        });
    }

    Ok(commits)
}

/// Format a Unix timestamp as a relative time string ("2h", "3d", "2w", "3mo").
fn format_relative_time(timestamp: i64, now: DateTime<Utc>) -> String {
    let commit_time = DateTime::from_timestamp(timestamp, 0).unwrap_or(now);
    let duration = now.signed_duration_since(commit_time);

    let seconds = duration.num_seconds();
    if seconds < 60 {
        return "now".to_string();
    }

    let minutes = duration.num_minutes();
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{hours}h");
    }

    let days = duration.num_days();
    if days < 7 {
        return format!("{days}d");
    }

    let weeks = days / 7;
    if weeks < 5 {
        return format!("{weeks}w");
    }

    let months = days / 30;
    if months < 12 {
        return format!("{months}mo");
    }

    let years = days / 365;
    format!("{years}y")
}

fn git_err(e: git2::Error) -> AppError {
    AppError::Git(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user for commits
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
    fn get_recent_commits_returns_commits() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First commit");
        create_commit(&repo, "Second commit");
        create_commit(&repo, "Third commit");

        let commits = get_recent_commits(dir.path(), 10).unwrap();
        assert_eq!(commits.len(), 3);
        // Newest first
        assert_eq!(commits[0].message, "Third commit");
        assert_eq!(commits[1].message, "Second commit");
        assert_eq!(commits[2].message, "First commit");
    }

    #[test]
    fn get_recent_commits_respects_limit() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First");
        create_commit(&repo, "Second");
        create_commit(&repo, "Third");

        let commits = get_recent_commits(dir.path(), 2).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Third");
        assert_eq!(commits[1].message, "Second");
    }

    #[test]
    fn get_recent_commits_empty_repo() {
        let (dir, _repo) = init_test_repo();
        // No commits — push_head fails, should return NotFound
        let result = get_recent_commits(dir.path(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn get_all_commits_with_offset() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First");
        create_commit(&repo, "Second");
        create_commit(&repo, "Third");

        let commits = get_all_commits(dir.path(), 2, 1).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Second");
        assert_eq!(commits[1].message, "First");
    }

    #[test]
    fn commit_hash_is_8_chars() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Test");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits[0].hash.len(), 8);
    }

    #[test]
    fn commit_author_is_correct() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Test");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits[0].author, "Test User");
    }

    #[test]
    fn multiline_message_uses_first_line() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "First line\n\nBody paragraph\nMore details");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits[0].message, "First line");
    }

    #[test]
    fn not_a_git_repo_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = get_recent_commits(dir.path(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn format_relative_time_minutes() {
        let now = Utc::now();
        let ts = (now - chrono::Duration::minutes(30)).timestamp();
        assert_eq!(format_relative_time(ts, now), "30m");
    }

    #[test]
    fn format_relative_time_hours() {
        let now = Utc::now();
        let ts = (now - chrono::Duration::hours(5)).timestamp();
        assert_eq!(format_relative_time(ts, now), "5h");
    }

    #[test]
    fn format_relative_time_days() {
        let now = Utc::now();
        let ts = (now - chrono::Duration::days(3)).timestamp();
        assert_eq!(format_relative_time(ts, now), "3d");
    }

    #[test]
    fn format_relative_time_weeks() {
        let now = Utc::now();
        let ts = (now - chrono::Duration::weeks(2)).timestamp();
        assert_eq!(format_relative_time(ts, now), "2w");
    }
}
