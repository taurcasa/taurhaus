use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use git2::Repository;

use crate::errors::AppError;
use crate::models::{Commit, CommitFile, DiffHunk, DiffLine};

/// Upper bound for commit count returned by range-query IPC endpoints.
pub const DEFAULT_RANGE_QUERY_COMMIT_CAP: usize = 500;
const RANGE_QUERY_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RangeCacheKey {
    repo_path: String,
    after_ts: i64,
    before_ts: i64,
    commit_cap: Option<usize>,
}

#[derive(Debug, Clone)]
struct RangeCacheEntry {
    cached_at: Instant,
    result: crate::models::GitRangeResult,
}

static RANGE_QUERY_CACHE: OnceLock<Mutex<HashMap<RangeCacheKey, RangeCacheEntry>>> =
    OnceLock::new();

/// Extract subject (first line) and optional body (remaining lines after first blank line).
fn extract_subject_and_body(raw: &str) -> (String, Option<String>) {
    let mut lines = raw.lines();
    let subject = lines.next().unwrap_or("").to_string();
    let body_lines: Vec<&str> = lines.skip_while(|l| l.trim().is_empty()).collect();
    let body = if body_lines.is_empty() {
        None
    } else {
        Some(body_lines.join("\n"))
    };
    (subject, body)
}

fn open_git_repo(repo_path: &Path) -> Result<Repository, AppError> {
    Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!(
            "Not a git repository: {}: {e}",
            repo_path.display()
        ))
    })
}

fn head_revwalk(repo: &Repository) -> Result<Option<git2::Revwalk<'_>>, AppError> {
    let mut revwalk = repo.revwalk().map_err(git_err)?;
    if revwalk.push_head().is_err() {
        return Ok(None);
    }
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(git_err)?;
    Ok(Some(revwalk))
}

/// Get recent commits from a git repository, newest first.
pub fn get_recent_commits(repo_path: &Path, limit: usize) -> Result<Vec<Commit>, AppError> {
    let repo = open_git_repo(repo_path)?;
    let revwalk = match head_revwalk(&repo)? {
        Some(revwalk) => revwalk,
        // No HEAD means no commits yet
        None => return Ok(vec![]),
    };

    let now = Utc::now();
    let mut commits = Vec::with_capacity(limit);

    for oid_result in revwalk.take(limit) {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;

        let timestamp = commit.time().seconds();
        let date = format_relative_time(timestamp, now);
        let (message, body) = extract_subject_and_body(commit.message().unwrap_or(""));

        commits.push(Commit {
            hash: format!("{:.8}", oid),
            message,
            body,
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date,
            timestamp,
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
    let repo = open_git_repo(repo_path)?;
    let revwalk = match head_revwalk(&repo)? {
        Some(revwalk) => revwalk,
        None => return Ok(vec![]),
    };

    let now = Utc::now();
    let mut commits = Vec::with_capacity(limit);

    for oid_result in revwalk.skip(offset).take(limit) {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;

        let timestamp = commit.time().seconds();
        let date = format_relative_time(timestamp, now);
        let (message, body) = extract_subject_and_body(commit.message().unwrap_or(""));

        commits.push(Commit {
            hash: format!("{:.8}", oid),
            message,
            body,
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date,
            timestamp,
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

/// Get commits and changed files in a range using a single revwalk pass.
///
/// Results are memoized for a short TTL to avoid duplicate computation when UI
/// rerenders request the exact same range repeatedly.
pub fn get_commits_and_files_in_range(
    repo_path: &Path,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
    commit_cap: Option<usize>,
) -> Result<crate::models::GitRangeResult, AppError> {
    get_commits_and_files_in_range_with_policy(
        repo_path,
        after,
        before,
        commit_cap,
        RANGE_QUERY_CACHE_TTL,
        true,
    )
}

fn get_commits_and_files_in_range_with_policy(
    repo_path: &Path,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
    commit_cap: Option<usize>,
    cache_ttl: Duration,
    use_cache: bool,
) -> Result<crate::models::GitRangeResult, AppError> {
    let key = RangeCacheKey {
        repo_path: repo_path.to_string_lossy().to_string(),
        after_ts: after.timestamp(),
        before_ts: before.timestamp(),
        commit_cap,
    };

    if use_cache && !cache_ttl.is_zero() {
        let now = Instant::now();
        let cache = RANGE_QUERY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = guard.get(&key) {
            if now.duration_since(entry.cached_at) < cache_ttl {
                return Ok(entry.result.clone());
            }
        }
        guard.retain(|_, entry| now.duration_since(entry.cached_at) < cache_ttl);
    }

    let result = collect_range_single_pass(repo_path, after, before, commit_cap)?;

    if use_cache && !cache_ttl.is_zero() {
        let cache = RANGE_QUERY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(
            key,
            RangeCacheEntry {
                cached_at: Instant::now(),
                result: result.clone(),
            },
        );
    }

    Ok(result)
}

fn collect_range_single_pass(
    repo_path: &Path,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
    commit_cap: Option<usize>,
) -> Result<crate::models::GitRangeResult, AppError> {
    if before < after {
        return Ok(crate::models::GitRangeResult {
            commits: vec![],
            files: vec![],
            truncated: false,
            total_count: None,
        });
    }

    let repo = open_git_repo(repo_path)?;
    let revwalk = match head_revwalk(&repo)? {
        Some(revwalk) => revwalk,
        None => {
            return Ok(crate::models::GitRangeResult {
                commits: vec![],
                files: vec![],
                truncated: false,
                total_count: None,
            });
        }
    };

    let now = Utc::now();
    let after_ts = after.timestamp();
    let before_ts = before.timestamp();
    let mut commits = Vec::new();
    let mut files = BTreeSet::new();
    let mut total_count = 0usize;
    let mut truncated = false;

    for oid_result in revwalk {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;
        let ts = commit.time().seconds();

        if ts < after_ts {
            break;
        }

        if ts > before_ts {
            continue;
        }

        total_count += 1;

        let include_commit = commit_cap.is_none_or(|cap| commits.len() < cap);
        if !include_commit {
            truncated = true;
            continue;
        }

        let (message, body) = extract_subject_and_body(commit.message().unwrap_or(""));
        commits.push(Commit {
            hash: format!("{:.8}", oid),
            message,
            body,
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date: format_relative_time(ts, now),
            timestamp: ts,
        });

        let tree = commit.tree().map_err(git_err)?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
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

    Ok(crate::models::GitRangeResult {
        commits,
        files: files.into_iter().collect(),
        truncated,
        total_count: truncated.then_some(total_count),
    })
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
    let repo = open_git_repo(repo_path)?;
    let revwalk = match head_revwalk(&repo)? {
        Some(revwalk) => revwalk,
        None => return Ok(vec![]),
    };

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
            let (message, body) = extract_subject_and_body(commit.message().unwrap_or(""));

            commits.push(Commit {
                hash: format!("{:.8}", oid),
                message,
                body,
                author: commit.author().name().unwrap_or("unknown").to_string(),
                date: format_relative_time(ts, now),
                timestamp: ts,
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
    let repo = open_git_repo(repo_path)?;
    let revwalk = match head_revwalk(&repo)? {
        Some(revwalk) => revwalk,
        None => return Ok(vec![]),
    };

    let after_ts = after.timestamp();
    let before_ts = before.timestamp();
    let mut files = BTreeSet::new();

    for oid_result in revwalk {
        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;
        let ts = commit.time().seconds();

        if ts < after_ts {
            break;
        }

        if ts <= before_ts {
            let tree = commit.tree().map_err(git_err)?;
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

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
        AppError::InvalidPath(format!(
            "Not a git repository: {}: {e}",
            repo_path.display()
        ))
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

/// Get the diff hunks for a specific file in a specific commit.
///
/// Returns a list of hunks with line-level detail. For binary files or
/// non-existent paths, returns an empty vec.
pub fn get_commit_diff(
    repo_path: &Path,
    commit_hash: &str,
    file_path: &str,
) -> Result<Vec<DiffHunk>, AppError> {
    let repo = Repository::open(repo_path).map_err(|e| {
        AppError::InvalidPath(format!(
            "Not a git repository: {}: {e}",
            repo_path.display()
        ))
    })?;

    let oid = repo
        .revparse_single(commit_hash)
        .map_err(|_| AppError::NotFound(format!("Commit not found: {commit_hash}")))?
        .peel_to_commit()
        .map_err(|_| AppError::NotFound(format!("Not a commit: {commit_hash}")))?
        .id();

    let commit = repo.find_commit(oid).map_err(git_err)?;
    let tree = commit.tree().map_err(git_err)?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(file_path);
    opts.context_lines(3);

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))
        .map_err(git_err)?;

    let mut hunks: Vec<DiffHunk> = Vec::new();

    diff.print(git2::DiffFormat::Patch, |_delta, hunk, line| {
        match line.origin() {
            'H' | 'F' => {
                // Hunk header or file header — push a new hunk if we have hunk info
                if let Some(h) = hunk {
                    hunks.push(DiffHunk {
                        old_start: h.old_start(),
                        old_lines: h.old_lines(),
                        new_start: h.new_start(),
                        new_lines: h.new_lines(),
                        lines: Vec::new(),
                    });
                }
            }
            '+' | '-' | ' ' => {
                let content = std::str::from_utf8(line.content())
                    .unwrap_or("")
                    .trim_end_matches('\n')
                    .to_string();
                let diff_line = DiffLine {
                    origin: line.origin(),
                    content,
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                };
                if let Some(last_hunk) = hunks.last_mut() {
                    last_hunk.lines.push(diff_line);
                }
            }
            _ => {}
        }
        true
    })
    .map_err(git_err)?;

    Ok(hunks)
}

fn git_err(e: git2::Error) -> AppError {
    AppError::Git(e)
}

#[cfg(test)]
fn clear_range_query_cache() {
    if let Some(cache) = RANGE_QUERY_CACHE.get() {
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.clear();
    }
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

    fn create_commit_at(repo: &Repository, message: &str, timestamp: DateTime<Utc>) {
        let git_time = git2::Time::new(timestamp.timestamp(), 0);
        let sig = Signature::new("Test User", "test@example.com", &git_time).unwrap();
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
    fn head_revwalk_is_none_without_head_commit() {
        let (_dir, repo) = init_test_repo();
        let revwalk = head_revwalk(&repo).unwrap();
        assert!(revwalk.is_none());
    }

    #[test]
    fn head_revwalk_is_available_after_first_commit() {
        let (_dir, repo) = init_test_repo();
        create_commit(&repo, "Initial commit");
        let revwalk = head_revwalk(&repo).unwrap();
        assert!(revwalk.is_some());
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
        let commits = get_recent_commits(dir.path(), 10).unwrap();
        assert!(commits.is_empty());
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
    fn multiline_message_extracts_body() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Subject\n\nBody paragraph\nMore details");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits[0].message, "Subject");
        assert_eq!(
            commits[0].body,
            Some("Body paragraph\nMore details".to_string())
        );
    }

    #[test]
    fn single_line_message_has_no_body() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Single line");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits[0].message, "Single line");
        assert_eq!(commits[0].body, None);
    }

    #[test]
    fn extract_subject_and_body_helper() {
        let (s, b) = extract_subject_and_body("Subject\n\nBody line 1\nBody line 2");
        assert_eq!(s, "Subject");
        assert_eq!(b, Some("Body line 1\nBody line 2".to_string()));

        let (s, b) = extract_subject_and_body("Just subject");
        assert_eq!(s, "Just subject");
        assert_eq!(b, None);

        let (s, b) = extract_subject_and_body("Subject\n\n");
        assert_eq!(s, "Subject");
        assert_eq!(b, None);

        let (s, b) = extract_subject_and_body("");
        assert_eq!(s, "");
        assert_eq!(b, None);
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
        index.add_path(Path::new(filename)).unwrap();
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
        let now = Utc::now();
        create_commit_at(&repo, "Old commit", now - chrono::Duration::seconds(10));
        create_commit_at(&repo, "In-range commit", now - chrono::Duration::seconds(5));

        let after = now - chrono::Duration::seconds(6);
        let before = now - chrono::Duration::seconds(4);

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
        repo.commit(Some("HEAD"), &sig, &sig, "Initial with files", &tree, &[])
            .unwrap();

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let files = get_commit_files(dir.path(), &commits[0].hash).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.status == "added"));
    }

    // --- get_commit_diff tests ---

    #[test]
    fn get_commit_diff_added_file() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "hello.txt", "Add hello");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let hunks = get_commit_diff(dir.path(), &commits[0].hash, "hello.txt").unwrap();
        assert!(!hunks.is_empty());
        // All lines should be additions
        for hunk in &hunks {
            for line in &hunk.lines {
                assert_eq!(line.origin, '+');
            }
        }
    }

    #[test]
    fn get_commit_diff_modified_file() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "file.txt", "Initial");
        // Modify
        std::fs::write(dir.path().join("file.txt"), "line1\nline2\nline3").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Update file");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let hunks = get_commit_diff(dir.path(), &commits[0].hash, "file.txt").unwrap();
        assert!(!hunks.is_empty());
        // Should have mixed origins (context, add, delete)
        let origins: Vec<char> = hunks
            .iter()
            .flat_map(|h| h.lines.iter().map(|l| l.origin))
            .collect();
        assert!(origins.contains(&'+') || origins.contains(&'-'));
    }

    #[test]
    fn get_commit_diff_deleted_file() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "gone.txt", "Initial");
        std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("gone.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, "Delete file");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let hunks = get_commit_diff(dir.path(), &commits[0].hash, "gone.txt").unwrap();
        assert!(!hunks.is_empty());
        for hunk in &hunks {
            for line in &hunk.lines {
                assert_eq!(line.origin, '-');
            }
        }
    }

    #[test]
    fn get_commit_diff_invalid_hash() {
        let (dir, repo) = init_test_repo();
        create_commit(&repo, "Initial");
        let result = get_commit_diff(dir.path(), "deadbeef99999999", "file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn get_commit_diff_nonexistent_file_path() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "real.txt", "Add file");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let hunks = get_commit_diff(dir.path(), &commits[0].hash, "nonexistent.txt").unwrap();
        assert!(hunks.is_empty());
    }

    #[test]
    fn get_commit_diff_has_line_numbers() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "nums.txt", "Add nums");

        let commits = get_recent_commits(dir.path(), 1).unwrap();
        let hunks = get_commit_diff(dir.path(), &commits[0].hash, "nums.txt").unwrap();
        assert!(!hunks.is_empty());
        let first_line = &hunks[0].lines[0];
        // Added file: old_lineno is None, new_lineno is Some
        assert!(first_line.old_lineno.is_none());
        assert!(first_line.new_lineno.is_some());
    }

    #[test]
    fn single_pass_range_matches_dual_pass_output() {
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "a.txt", "Add a");
        create_commit_with_file(&repo, dir.path(), "b.txt", "Add b");
        create_commit_with_file(&repo, dir.path(), "a.txt", "Update a");

        let after = Utc::now() - chrono::Duration::hours(1);
        let before = Utc::now() + chrono::Duration::hours(1);

        let dual_pass_commits = get_commits_in_range(dir.path(), after, before).unwrap();
        let dual_pass_files = get_files_changed_in_range(dir.path(), after, before).unwrap();
        let single_pass = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            None,
            std::time::Duration::from_secs(5),
            false,
        )
        .unwrap();

        assert_eq!(single_pass.commits, dual_pass_commits);
        assert_eq!(single_pass.files, dual_pass_files);
        assert!(!single_pass.truncated);
        assert_eq!(single_pass.total_count, None);
    }

    #[test]
    fn range_query_memoization_hits_within_ttl() {
        clear_range_query_cache();
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "before.txt", "Before");

        let after = Utc::now() - chrono::Duration::hours(1);
        let before = Utc::now() + chrono::Duration::hours(1);

        let first = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            Some(500),
            std::time::Duration::from_secs(5),
            true,
        )
        .unwrap();

        // This commit should be ignored by the second call if cache is hit.
        create_commit_with_file(&repo, dir.path(), "after.txt", "After");

        let second = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            Some(500),
            std::time::Duration::from_secs(5),
            true,
        )
        .unwrap();

        assert_eq!(second.commits, first.commits);
        assert_eq!(second.files, first.files);
        assert_eq!(second.total_count, first.total_count);
    }

    #[test]
    fn range_query_memoization_expires_after_ttl() {
        clear_range_query_cache();
        let (dir, repo) = init_test_repo();
        create_commit_with_file(&repo, dir.path(), "before.txt", "Before");

        let after = Utc::now() - chrono::Duration::hours(1);
        let before = Utc::now() + chrono::Duration::hours(1);
        let ttl = std::time::Duration::from_millis(20);

        let first = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            Some(500),
            ttl,
            true,
        )
        .unwrap();

        create_commit_with_file(&repo, dir.path(), "after.txt", "After");
        std::thread::sleep(std::time::Duration::from_millis(30));

        let second = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            Some(500),
            ttl,
            true,
        )
        .unwrap();

        assert!(second.commits.len() > first.commits.len());
        assert!(second.files.len() > first.files.len());
    }

    #[test]
    fn range_query_truncates_when_commit_cap_exceeded() {
        clear_range_query_cache();
        let (dir, repo) = init_test_repo();
        for i in 0..6 {
            create_commit_with_file(&repo, dir.path(), &format!("file-{i}.txt"), "add");
        }

        let after = Utc::now() - chrono::Duration::hours(1);
        let before = Utc::now() + chrono::Duration::hours(1);

        let result = get_commits_and_files_in_range_with_policy(
            dir.path(),
            after,
            before,
            Some(3),
            std::time::Duration::from_secs(5),
            false,
        )
        .unwrap();

        assert_eq!(result.commits.len(), 3);
        assert!(result.truncated);
        assert_eq!(result.total_count, Some(6));
    }
}
