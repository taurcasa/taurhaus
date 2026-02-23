use std::path::Path;

use chrono::{DateTime, Utc};
use git2::Repository;

use crate::errors::AppError;
use crate::models::{Commit, CommitFile};

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

/// Get the timestamp of the most recent commit, or `None` if the repo has no commits.
pub fn get_latest_commit_time(repo_path: &Path) -> Option<DateTime<Utc>> {
    let repo = Repository::open(repo_path).ok()?;
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    DateTime::from_timestamp(commit.time().seconds(), 0)
}

/// Get commits in a time range (inclusive), newest first.
///
/// Returns commits whose author date falls within `[after, before]`.
/// Used to find commits made during a specific CLI session.
pub fn get_commits_in_range(
    repo_path: &Path,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
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
    let after_ts = after.timestamp();
    let before_ts = before.timestamp();
    let mut commits = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;
        let ts = commit.time().seconds();

        // Stop early — commits are newest-first, so once we're before the range, we're done
        if ts < after_ts {
            break;
        }

        if ts <= before_ts {
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
                date: format_relative_time(ts, now),
            });
        }
    }

    Ok(commits)
}

/// Get deduplicated list of files changed across a set of commits.
///
/// Walks each commit's diff against its parent to collect changed file paths.
pub fn get_files_changed_in_range(
    repo_path: &Path,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
) -> Result<Vec<String>, AppError> {
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

    let after_ts = after.timestamp();
    let before_ts = before.timestamp();
    let mut files = std::collections::BTreeSet::new();

    for oid_result in revwalk {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;
        let ts = commit.time().seconds();

        if ts < after_ts {
            break;
        }

        if ts <= before_ts {
            let tree = commit.tree().map_err(git_err)?;
            let parent_tree = commit
                .parent(0)
                .ok()
                .and_then(|p| p.tree().ok());

            let diff = repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
                .map_err(git_err)?;

            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path().and_then(|p| p.to_str()) {
                        files.insert(path.to_string());
                    }
                    true
                },
                None,
                None,
                None,
            )
            .map_err(git_err)?;
        }
    }

    Ok(files.into_iter().collect())
}

/// Get the list of files changed by a specific commit.
///
/// Diffs the commit's tree against its parent to find changed files.
/// For the initial commit (no parent), diffs against an empty tree.
pub fn get_commit_files(repo_path: &Path, commit_hash: &str) -> Result<Vec<CommitFile>, AppError> {
    let repo = Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!("Not a git repository: {}: {e}", repo_path.display()))
    })?;

    // Resolve partial hash to full OID
    let oid = repo
        .revparse_single(commit_hash)
        .map_err(|_| AppError::NotFound(format!("Commit not found: {commit_hash}")))?
        .peel_to_commit()
        .map_err(|_| AppError::NotFound(format!("Not a commit: {commit_hash}")))?
        .id();

    let commit = repo.find_commit(oid).map_err(git_err)?;
    let tree = commit.tree().map_err(git_err)?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(git_err)?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            let status = match delta.status() {
                git2::Delta::Added => "added",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Renamed => "renamed",
                _ => "modified",
            };

            files.push(CommitFile {
                path,
                status: status.to_string(),
            });
            true
        },
        None,
        None,
        None,
    )
    .map_err(git_err)?;

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
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

    fn create_commit_with_file(repo: &Repository, dir: &Path, filename: &str, message: &str) {
        let file_path = dir.join(filename);
        std::fs::write(&file_path, format!("content of {filename}")).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new(filename))
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }

    #[test]
    fn get_commits_in_range_filters_by_time() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Old commit");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let after = Utc::now();
        create_commit(&repo, "In-range commit");
        let before = Utc::now();

        let commits = get_commits_in_range(dir.path(), after, before).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "In-range commit");
    }

    #[test]
    fn get_commits_in_range_empty_when_no_match() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Old commit");

        let after = Utc::now() + chrono::Duration::hours(1);
        let before = after + chrono::Duration::hours(1);

        let commits = get_commits_in_range(dir.path(), after, before).unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn get_commits_in_range_empty_repo() {
        let (dir, _repo) = init_test_repo();
        let now = Utc::now();
        let commits = get_commits_in_range(dir.path(), now, now).unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn get_files_changed_in_range_returns_files() {
        let (dir, repo) = init_test_repo();
        // Create an initial commit with a file (outside the range)
        create_commit_with_file(&repo, dir.path(), "old.txt", "Old file");
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let after = Utc::now();
        create_commit_with_file(&repo, dir.path(), "new.txt", "New file");
        create_commit_with_file(&repo, dir.path(), "another.txt", "Another file");
        let before = Utc::now();

        let files = get_files_changed_in_range(dir.path(), after, before).unwrap();
        assert!(files.contains(&"new.txt".to_string()));
        assert!(files.contains(&"another.txt".to_string()));
        assert!(!files.contains(&"old.txt".to_string()));
    }

    #[test]
    fn get_files_changed_deduplicates() {
        let (dir, repo) = init_test_repo();
        let after = Utc::now() - chrono::Duration::hours(1);
        // Two commits touching the same file
        create_commit_with_file(&repo, dir.path(), "shared.txt", "First version");
        std::fs::write(dir.path().join("shared.txt"), "Updated content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("shared.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Update shared.txt");
        let before = Utc::now();

        let files = get_files_changed_in_range(dir.path(), after, before).unwrap();
        // Should appear only once despite two commits
        assert_eq!(files.iter().filter(|f| *f == "shared.txt").count(), 1);
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

    #[test]
    fn get_commit_files_added() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "hello.txt", "Add hello");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "hello.txt");
        assert_eq!(files[0].status, "added");
    }

    #[test]
    fn get_commit_files_modified() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "file.txt", "Initial");
        // Modify the file
        std::fs::write(dir.path().join("file.txt"), "Modified content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Update file");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "file.txt");
        assert_eq!(files[0].status, "modified");
    }

    #[test]
    fn get_commit_files_deleted() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "gone.txt", "Initial");
        // Delete the file
        std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("gone.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Delete file");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "gone.txt");
        assert_eq!(files[0].status, "deleted");
    }

    #[test]
    fn get_commit_files_multiple() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "existing.txt", "Initial");
        // Create a commit that adds one file and modifies another
        std::fs::write(dir.path().join("existing.txt"), "Changed").unwrap();
        std::fs::write(dir.path().join("new.txt"), "Brand new").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("existing.txt")).unwrap();
        index.add_path(Path::new("new.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Multi-change");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 2);
        // Sorted by path
        assert_eq!(files[0].path, "existing.txt");
        assert_eq!(files[0].status, "modified");
        assert_eq!(files[1].path, "new.txt");
        assert_eq!(files[1].status, "added");
    }

    #[test]
    fn get_commit_files_invalid_hash() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");

        let result = get_commit_files(dir.path(), "deadbeef99999999");
        assert!(result.is_err());
    }

    #[test]
    fn get_commit_files_initial_commit_all_added() {
        let (dir, repo) = init_test_repo();
        // First commit with a file: everything is "added"
        std::fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        std::fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.txt")).unwrap();
        index.add_path(Path::new("b.txt")).unwrap();
        index.write().unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial with files", &tree, &[]).unwrap();

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.status == "added"));
    }
}
