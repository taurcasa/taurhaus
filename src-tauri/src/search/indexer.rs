use std::path::Path;

use tantivy::schema::{Field, Schema, STORED, STRING, TEXT};
use tantivy::{Index, IndexWriter, TantivyDocument};

use crate::errors::AppError;
use crate::services::scan_policy::ScanIndexPolicy;

/// Heap size for the tantivy index writer (50 MB).
const WRITER_HEAP_SIZE: usize = 50 * 1024 * 1024;

/// Named fields in the tantivy schema.
pub struct SearchFields {
    pub project_id: Field,
    pub entity_type: Field,
    pub file_path: Field,
    pub title: Field,
    pub content: Field,
}

/// Wrapper around a tantivy index providing add/remove/commit operations.
pub struct SearchIndex {
    index: Index,
    writer: IndexWriter,
    pub fields: SearchFields,
    pub schema: Schema,
    #[cfg(test)]
    commit_calls: usize,
}

/// Build the tantivy schema used for all indexed content.
fn build_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();

    let project_id = builder.add_text_field("project_id", STRING | STORED);
    let entity_type = builder.add_text_field("entity_type", STRING | STORED);
    let file_path = builder.add_text_field("file_path", STRING | STORED);
    let title = builder.add_text_field("title", TEXT | STORED);
    let content = builder.add_text_field("content", TEXT | STORED);

    let schema = builder.build();
    let fields = SearchFields {
        project_id,
        entity_type,
        file_path,
        title,
        content,
    };

    (schema, fields)
}

impl SearchIndex {
    /// Open or create a persistent tantivy index at the given directory.
    pub fn open(index_dir: &Path) -> Result<Self, AppError> {
        std::fs::create_dir_all(index_dir)?;

        let (schema, fields) = build_schema();

        let dir = tantivy::directory::MmapDirectory::open(index_dir)
            .map_err(|e| AppError::SearchError(format!("Failed to open index directory: {e}")))?;

        let index = if Index::exists(&dir)
            .map_err(|e| AppError::SearchError(format!("Failed to check index existence: {e}")))?
        {
            Index::open(dir)
                .map_err(|e| AppError::SearchError(format!("Failed to open existing index: {e}")))?
        } else {
            Index::create(dir, schema.clone(), Default::default())
                .map_err(|e| AppError::SearchError(format!("Failed to create index: {e}")))?
        };

        let writer = index
            .writer(WRITER_HEAP_SIZE)
            .map_err(|e| AppError::SearchError(format!("Failed to create index writer: {e}")))?;

        Ok(Self {
            index,
            writer,
            fields,
            schema,
            #[cfg(test)]
            commit_calls: 0,
        })
    }

    /// Create an in-memory index (for testing).
    pub fn open_in_memory() -> Result<Self, AppError> {
        let (schema, fields) = build_schema();

        let index = Index::create_in_ram(schema.clone());

        let writer = index
            .writer(WRITER_HEAP_SIZE)
            .map_err(|e| AppError::SearchError(format!("Failed to create index writer: {e}")))?;

        Ok(Self {
            index,
            writer,
            fields,
            schema,
            #[cfg(test)]
            commit_calls: 0,
        })
    }

    /// Add a document to the index (buffered, call `commit` to persist).
    pub fn add_document(
        &mut self,
        project_id: &str,
        entity_type: &str,
        file_path: &str,
        title: &str,
        content: &str,
    ) -> Result<(), AppError> {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.fields.project_id, project_id);
        doc.add_text(self.fields.entity_type, entity_type);
        doc.add_text(self.fields.file_path, file_path);
        doc.add_text(self.fields.title, title);
        doc.add_text(self.fields.content, content);

        self.writer
            .add_document(doc)
            .map_err(|e| AppError::SearchError(format!("Failed to add document: {e}")))?;
        Ok(())
    }

    /// Remove all documents matching a file_path (exact match).
    pub fn remove_by_file_path(&mut self, file_path: &str) {
        let term = tantivy::Term::from_field_text(self.fields.file_path, file_path);
        self.writer.delete_term(term);
    }

    /// Remove all documents for a given project.
    pub fn remove_by_project(&mut self, project_id: &str) {
        let term = tantivy::Term::from_field_text(self.fields.project_id, project_id);
        self.writer.delete_term(term);
    }

    /// Commit all pending changes to the index.
    pub fn commit(&mut self) -> Result<(), AppError> {
        #[cfg(test)]
        {
            self.commit_calls = self.commit_calls.saturating_add(1);
        }
        self.writer
            .commit()
            .map_err(|e| AppError::SearchError(format!("Failed to commit index: {e}")))?;
        Ok(())
    }

    #[cfg(test)]
    fn commit_calls(&self) -> usize {
        self.commit_calls
    }

    /// Clear the entire index (delete all documents).
    pub fn clear(&mut self) -> Result<(), AppError> {
        self.writer
            .delete_all_documents()
            .map_err(|e| AppError::SearchError(format!("Failed to clear index: {e}")))?;
        self.commit()
    }

    /// Return a reference to the underlying tantivy Index (for readers/searchers).
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Count the total number of documents in the index.
    pub fn doc_count(&self) -> Result<u64, AppError> {
        let reader = self
            .index
            .reader()
            .map_err(|e| AppError::SearchError(format!("Failed to create reader: {e}")))?;
        let searcher = reader.searcher();
        let total: u64 = searcher
            .segment_readers()
            .iter()
            .map(|r| r.num_docs() as u64)
            .sum();
        Ok(total)
    }
}

/// Truncate a string at the nearest char boundary at or before `max_bytes`.
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk backwards from max_bytes to find a valid char boundary
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ---------------------------------------------------------------------------
// Bulk index builder
// ---------------------------------------------------------------------------

/// Max file size for indexing (1 MB — larger files are skipped).
const MAX_INDEX_FILE_SIZE: u64 = 1024 * 1024;

/// Extensions considered as text files for indexing.
const TEXT_EXTENSIONS: &[&str] = &[
    "md",
    "txt",
    "rs",
    "js",
    "ts",
    "svelte",
    "html",
    "css",
    "json",
    "toml",
    "yaml",
    "yml",
    "xml",
    "sql",
    "sh",
    "bash",
    "zsh",
    "py",
    "rb",
    "go",
    "java",
    "kt",
    "c",
    "cpp",
    "h",
    "hpp",
    "lua",
    "vim",
    "conf",
    "cfg",
    "ini",
    "env",
    "lock",
    "gitignore",
    "editorconfig",
];

/// Check whether a file should be indexed based on extension.
fn is_indexable_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| TEXT_EXTENSIONS.contains(&ext))
}

/// Index all text files in a project directory (respects .gitignore).
pub fn index_project_files(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
) -> Result<usize, AppError> {
    index_project_files_with_policy(index, project_id, project_root, &ScanIndexPolicy::default())
}

pub fn index_project_files_with_policy(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    policy: &ScanIndexPolicy,
) -> Result<usize, AppError> {
    use ignore::WalkBuilder;

    let mut count = 0;
    let matcher = policy.matcher_for_root(project_root);

    let walker = WalkBuilder::new(project_root)
        .follow_links(false)
        .hidden(true) // skip hidden files
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        let is_dir = entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        if matcher.ignores_path(path, is_dir) {
            continue;
        }

        if !path.is_file() || !is_indexable_file(path) {
            continue;
        }

        // Skip oversized files
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > MAX_INDEX_FILE_SIZE {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary/unreadable
        };

        let relative = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        index.add_document(project_id, "document", &relative, &title, &content)?;
        count += 1;
    }

    Ok(count)
}

/// Index sessions from the database for a given project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIndexStats {
    pub indexed: usize,
    pub skipped: usize,
}

fn index_session_summaries(
    index: &mut SearchIndex,
    project_id: &str,
    conn: &rusqlite::Connection,
    summaries: impl IntoIterator<Item = crate::models::SessionSummary>,
) -> Result<SessionIndexStats, AppError> {
    use crate::db::session_queries;

    let mut indexed = 0;
    let mut skipped = 0;

    for summary in summaries {
        // Get the full session detail for next_steps / open_questions.
        let detail = match session_queries::get_session(conn, &summary.id) {
            Ok(Some(detail)) => detail,
            Ok(None) => {
                skipped += 1;
                tracing::debug!(
                    project_id,
                    session_id = %summary.id,
                    "search index: skipping missing session detail"
                );
                continue;
            }
            Err(err) => {
                skipped += 1;
                tracing::debug!(
                    project_id,
                    session_id = %summary.id,
                    error = %err,
                    "search index: skipping session detail due to load error"
                );
                continue;
            }
        };

        let mut content = detail.summary.clone();
        for step in &detail.next_steps {
            content.push_str("\n- ");
            content.push_str(step);
        }
        for q in &detail.open_questions {
            content.push_str("\n? ");
            content.push_str(q);
        }

        let file_path = format!("session:{}", detail.id);
        let title_prefix = truncate_at_char_boundary(&detail.summary, 60);

        index.add_document(project_id, "session", &file_path, title_prefix, &content)?;
        indexed += 1;
    }

    if skipped > 0 {
        tracing::debug!(
            project_id,
            indexed,
            skipped,
            "search index: skipped sessions during indexing"
        );
    }

    Ok(SessionIndexStats { indexed, skipped })
}

pub fn index_project_sessions(
    index: &mut SearchIndex,
    project_id: &str,
    conn: &rusqlite::Connection,
) -> Result<SessionIndexStats, AppError> {
    use crate::db::session_queries;

    let sessions = session_queries::list_sessions(conn, project_id, 1000, 0)?;
    index_session_summaries(index, project_id, conn, sessions)
}

fn index_commit_documents(
    index: &mut SearchIndex,
    project_id: &str,
    commits: &[crate::models::Commit],
    replace_existing: bool,
) -> Result<usize, AppError> {
    for commit in commits {
        let file_path = format!("commit:{}", commit.hash);
        if replace_existing {
            index.remove_by_file_path(&file_path);
        }
        index.add_document(
            project_id,
            "commit",
            &file_path,
            &commit.message,
            &format!("{} — {} ({})", commit.message, commit.author, commit.date),
        )?;
    }

    Ok(commits.len())
}

/// Index recent commits from git for a given project.
pub fn index_project_commits(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    limit: usize,
) -> Result<usize, AppError> {
    use crate::git::commits::get_recent_commits;

    let commits = match get_recent_commits(project_root, limit) {
        Ok(c) => c,
        Err(_) => return Ok(0), // no git repo or no commits
    };

    index_commit_documents(index, project_id, &commits, false)
}

/// Build the full index for a single project (files + sessions + commits).
pub fn build_project_index(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    conn: &rusqlite::Connection,
) -> Result<(usize, usize, usize), AppError> {
    let policy = ScanIndexPolicy::load(conn)?;
    build_project_index_with_policy(index, project_id, project_root, conn, &policy)
}

pub fn build_project_index_with_policy(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    conn: &rusqlite::Connection,
    policy: &ScanIndexPolicy,
) -> Result<(usize, usize, usize), AppError> {
    // Remove existing entries for this project before re-indexing
    index.remove_by_project(project_id);

    let files = index_project_files_with_policy(index, project_id, project_root, policy)?;
    let session_stats = index_project_sessions(index, project_id, conn)?;
    let sessions = session_stats.indexed;
    let commits = index_project_commits(index, project_id, project_root, 100)?;

    index.commit()?;

    Ok((files, sessions, commits))
}

/// Rebuild the entire index from scratch (all projects).
pub fn rebuild_all(
    index: &mut SearchIndex,
    conn: &rusqlite::Connection,
) -> Result<usize, AppError> {
    let policy = ScanIndexPolicy::load(conn)?;
    rebuild_all_with_policy(index, conn, &policy)
}

pub fn rebuild_all_with_policy(
    index: &mut SearchIndex,
    conn: &rusqlite::Connection,
    policy: &ScanIndexPolicy,
) -> Result<usize, AppError> {
    use crate::db::queries;

    index.clear()?;

    let projects = queries::list_projects(conn)?;
    let mut total = 0;

    for project in projects {
        let project_root = std::path::Path::new(&project.path);
        if !project_root.exists() {
            continue;
        }

        let (files, sessions, commits) =
            build_project_index_with_policy(index, &project.id, project_root, conn, policy)?;
        total += files + sessions + commits;
        tracing::info!(
            project = project.name,
            files,
            sessions,
            commits,
            "indexed project"
        );
    }

    Ok(total)
}

// ---------------------------------------------------------------------------
// Incremental index updates (called from file watcher)
// ---------------------------------------------------------------------------

/// Update the search index for a single file change.
///
/// If the file exists and is indexable, upserts its content.
/// If the file doesn't exist (deleted), removes it from the index.
/// Returns true if the index was modified.
pub fn update_file(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    absolute_path: &Path,
) -> Result<bool, AppError> {
    update_file_with_scan_policy(
        index,
        project_id,
        project_root,
        absolute_path,
        &ScanIndexPolicy::default(),
    )
}

pub fn update_file_with_scan_policy(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    absolute_path: &Path,
    policy: &ScanIndexPolicy,
) -> Result<bool, AppError> {
    update_file_impl(index, project_id, project_root, absolute_path, true, policy)
}

/// Batch-mode variant of `update_file` that defers commit until caller flushes.
pub fn update_file_batched(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    absolute_path: &Path,
) -> Result<bool, AppError> {
    update_file_batched_with_scan_policy(
        index,
        project_id,
        project_root,
        absolute_path,
        &ScanIndexPolicy::default(),
    )
}

pub fn update_file_batched_with_scan_policy(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    absolute_path: &Path,
    policy: &ScanIndexPolicy,
) -> Result<bool, AppError> {
    update_file_impl(
        index,
        project_id,
        project_root,
        absolute_path,
        false,
        policy,
    )
}

/// Flush buffered updates after a batch of `update_file_batched` calls.
pub fn commit_batch(index: &mut SearchIndex) -> Result<(), AppError> {
    index.commit()
}

fn update_file_impl(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    absolute_path: &Path,
    auto_commit: bool,
    policy: &ScanIndexPolicy,
) -> Result<bool, AppError> {
    fn remove_path(
        index: &mut SearchIndex,
        relative: &str,
        auto_commit: bool,
    ) -> Result<bool, AppError> {
        index.remove_by_file_path(relative);
        if auto_commit {
            index.commit()?;
        }
        Ok(true)
    }

    let relative = match absolute_path.strip_prefix(project_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => return Ok(false),
    };

    // Enforce project boundary on resolved paths to prevent symlink escapes.
    let canonical_root = match project_root.canonicalize() {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    let canonical_file = match absolute_path.canonicalize() {
        Ok(path) => path,
        Err(_) => return remove_path(index, &relative, auto_commit),
    };
    if !canonical_file.starts_with(&canonical_root) {
        return remove_path(index, &relative, auto_commit);
    }

    let matcher = policy.matcher_for_root(project_root);
    if matcher.ignores_path(absolute_path, false) {
        return remove_path(index, &relative, auto_commit);
    }

    // If file was deleted or isn't indexable, remove from index
    if !absolute_path.is_file() || !is_indexable_file(absolute_path) {
        return remove_path(index, &relative, auto_commit);
    }

    // Skip oversized files
    if let Ok(meta) = std::fs::metadata(absolute_path) {
        if meta.len() > MAX_INDEX_FILE_SIZE {
            return remove_path(index, &relative, auto_commit);
        }
    }

    // Read and index the file
    let content = match std::fs::read_to_string(absolute_path) {
        Ok(c) => c,
        Err(_) => {
            // File exists but is no longer readable as text (e.g. binary/encoding/permissions):
            // remove stale index content for this path.
            return remove_path(index, &relative, auto_commit);
        }
    };

    let title = absolute_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // Remove old entry and add updated one
    index.remove_by_file_path(&relative);
    index.add_document(project_id, "document", &relative, &title, &content)?;
    if auto_commit {
        index.commit()?;
    }

    Ok(true)
}

/// Index a single session by ID (called after session import).
pub fn index_session(
    index: &mut SearchIndex,
    project_id: &str,
    session_id: &str,
    conn: &rusqlite::Connection,
) -> Result<bool, AppError> {
    use crate::db::session_queries;

    let detail = match session_queries::get_session(conn, session_id)? {
        Some(d) => d,
        None => return Ok(false),
    };

    let mut content = detail.summary.clone();
    for step in &detail.next_steps {
        content.push_str("\n- ");
        content.push_str(step);
    }
    for q in &detail.open_questions {
        content.push_str("\n? ");
        content.push_str(q);
    }

    let file_path = format!("session:{}", detail.id);
    let title = if detail.summary.len() > 60 {
        &detail.summary[..60]
    } else {
        &detail.summary
    };

    // Remove any existing entry for this session, then add fresh
    index.remove_by_file_path(&file_path);
    index.add_document(project_id, "session", &file_path, title, &content)?;
    index.commit()?;

    Ok(true)
}

/// Re-index recent commits for a project (called on git changes).
pub fn reindex_commits(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    limit: usize,
) -> Result<usize, AppError> {
    use crate::git::commits::get_recent_commits;

    // Remove all existing commit entries for this project
    // We use a convention: commit file_paths start with "commit:"
    // Since we can't query tantivy for "starts with", remove by project and re-add all.
    // But that would also remove files/sessions. Instead, get all existing commit hashes
    // and remove them individually. Actually, just re-index the commits — the old ones
    // will naturally be replaced since we're deleting by file_path.
    let commits = match get_recent_commits(project_root, limit) {
        Ok(c) => c,
        Err(_) => return Ok(0),
    };

    let count = index_commit_documents(index, project_id, &commits, true)?;

    if !commits.is_empty() {
        index.commit()?;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;

    #[test]
    fn create_in_memory_index() {
        let index = SearchIndex::open_in_memory().unwrap();
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn create_persistent_index() {
        let dir = tempfile::TempDir::new().unwrap();
        let index = SearchIndex::open(dir.path()).unwrap();
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn persistent_index_survives_reopen() {
        let dir = tempfile::TempDir::new().unwrap();

        // Create and add a document
        {
            let mut index = SearchIndex::open(dir.path()).unwrap();
            index
                .add_document("p1", "document", "src/main.rs", "main.rs", "fn main() {}")
                .unwrap();
            index.commit().unwrap();
        }

        // Reopen and verify
        let index = SearchIndex::open(dir.path()).unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);
    }

    #[test]
    fn add_document_and_count() {
        let mut index = SearchIndex::open_in_memory().unwrap();

        index
            .add_document("p1", "document", "README.md", "README", "# Hello World")
            .unwrap();
        index
            .add_document(
                "p1",
                "session",
                "session-1.md",
                "Phase 5A",
                "Scaffolding complete",
            )
            .unwrap();
        index.commit().unwrap();

        assert_eq!(index.doc_count().unwrap(), 2);
    }

    #[test]
    fn remove_by_file_path() {
        let mut index = SearchIndex::open_in_memory().unwrap();

        index
            .add_document("p1", "document", "README.md", "README", "# Hello")
            .unwrap();
        index
            .add_document("p1", "document", "src/main.rs", "main.rs", "fn main() {}")
            .unwrap();
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 2);

        index.remove_by_file_path("README.md");
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);
    }

    #[test]
    fn remove_by_project() {
        let mut index = SearchIndex::open_in_memory().unwrap();

        index
            .add_document("p1", "document", "a.md", "A", "Project 1")
            .unwrap();
        index
            .add_document("p2", "document", "b.md", "B", "Project 2")
            .unwrap();
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 2);

        index.remove_by_project("p1");
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);
    }

    #[test]
    fn clear_removes_all() {
        let mut index = SearchIndex::open_in_memory().unwrap();

        index
            .add_document("p1", "document", "a.md", "A", "Content A")
            .unwrap();
        index
            .add_document("p2", "document", "b.md", "B", "Content B")
            .unwrap();
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 2);

        index.clear().unwrap();
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn schema_has_expected_fields() {
        let index = SearchIndex::open_in_memory().unwrap();
        let schema = &index.schema;

        assert!(schema.get_field("project_id").is_ok());
        assert!(schema.get_field("entity_type").is_ok());
        assert!(schema.get_field("file_path").is_ok());
        assert!(schema.get_field("title").is_ok());
        assert!(schema.get_field("content").is_ok());
    }

    #[test]
    fn add_all_entity_types() {
        let mut index = SearchIndex::open_in_memory().unwrap();

        index
            .add_document("p1", "document", "README.md", "README", "docs")
            .unwrap();
        index
            .add_document("p1", "session", "s1", "Phase 5A", "scaffold done")
            .unwrap();
        index
            .add_document("p1", "commit", "abc123", "Add feature", "commit msg")
            .unwrap();
        index.commit().unwrap();

        assert_eq!(index.doc_count().unwrap(), 3);
    }

    // --- Builder tests ---

    #[test]
    fn is_indexable_detects_text_files() {
        assert!(is_indexable_file(Path::new("README.md")));
        assert!(is_indexable_file(Path::new("src/main.rs")));
        assert!(is_indexable_file(Path::new("package.json")));
        assert!(is_indexable_file(Path::new("style.css")));
        assert!(!is_indexable_file(Path::new("image.png")));
        assert!(!is_indexable_file(Path::new("binary.exe")));
        assert!(!is_indexable_file(Path::new("noextension")));
    }

    #[test]
    fn index_project_files_indexes_text_files() {
        let dir = tempfile::TempDir::new().unwrap();
        // Create some text files
        std::fs::write(dir.path().join("README.md"), "# Hello World").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        // Create a binary file (should be skipped)
        std::fs::write(dir.path().join("image.png"), [0u8; 100]).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_files(&mut index, "p1", dir.path()).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 2); // README.md + src/main.rs
        assert_eq!(index.doc_count().unwrap(), 2);
    }

    #[test]
    fn index_project_files_stores_forward_slash_paths() {
        // Regression: on Windows, strip_prefix produces backslash paths (src\main.rs).
        // The daemon runs on Linux where backslashes are literal chars, not separators.
        // Paths must be stored with forward slashes so they work cross-platform.
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        index_project_files(&mut index, "p1", dir.path()).unwrap();
        index.commit().unwrap();

        let results = index.search("main", 10).unwrap();
        assert_eq!(results.len(), 1);
        // Path must use forward slashes, not backslashes
        assert_eq!(results[0].file_path, "src/main.rs");
        assert!(!results[0].file_path.contains('\\'));
    }

    #[test]
    fn index_project_files_skips_oversized() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("small.md"), "Small file").unwrap();
        // Create a file larger than 1MB
        let big = vec![b'x'; (MAX_INDEX_FILE_SIZE + 1) as usize];
        std::fs::write(dir.path().join("big.md"), big).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_files(&mut index, "p1", dir.path()).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 1); // only small.md
    }

    #[test]
    fn index_project_files_respects_saved_ignore_patterns() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# keep me").unwrap();
        std::fs::create_dir_all(dir.path().join("generated")).unwrap();
        std::fs::write(dir.path().join("generated/skip.md"), "skip me").unwrap();

        let policy = ScanIndexPolicy::from_settings(&Settings {
            ignore_patterns: vec!["generated".into()],
            ..Settings::default()
        });

        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_files_with_policy(&mut index, "p1", dir.path(), &policy).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 1);
        assert_eq!(index.doc_count().unwrap(), 1);
        assert!(index.search("keep", 10).unwrap().len() == 1);
        assert!(index.search("skip", 10).unwrap().is_empty());
    }

    #[test]
    fn index_project_sessions_from_db() {
        use crate::db;
        use crate::models::SessionDetail;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();

        // Insert a project
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES ('p1', 'test', '/test', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        // Insert sessions
        let session = SessionDetail {
            id: "s1".into(),
            project_id: "p1".into(),
            date: "2026-02-15".into(),
            summary: "Completed Phase 5A scaffold".into(),
            next_steps: vec!["Implement git module".into()],
            open_questions: vec!["Tantivy config?".into()],
            metadata: serde_json::json!({}),
            file_path: "/test/sessions/s1.md".into(),
            created_at: "2026-02-15T00:00:00Z".into(),
        };
        db::session_queries::insert_session(&conn, &session).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let stats = index_project_sessions(&mut index, "p1", &conn).unwrap();
        index.commit().unwrap();

        assert_eq!(stats.indexed, 1);
        assert_eq!(stats.skipped, 0);
        // Verify the session is searchable
        let results = index.search("scaffold", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].entity_type, "session");
    }

    #[test]
    fn index_session_summaries_tracks_skipped_missing_details() {
        use crate::db;
        use crate::models::{SessionDetail, SessionSummary};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES ('p1', 'test', '/test', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();

        let session = SessionDetail {
            id: "s1".into(),
            project_id: "p1".into(),
            date: "2026-02-15".into(),
            summary: "Session present".into(),
            next_steps: vec![],
            open_questions: vec![],
            metadata: serde_json::json!({}),
            file_path: "/test/sessions/s1.md".into(),
            created_at: "2026-02-15T00:00:00Z".into(),
        };
        db::session_queries::insert_session(&conn, &session).unwrap();

        let summaries = vec![
            SessionSummary {
                id: "s1".into(),
                project_id: "p1".into(),
                date: "2026-02-15".into(),
                summary: "Session present".into(),
            },
            SessionSummary {
                id: "missing".into(),
                project_id: "p1".into(),
                date: "2026-02-14".into(),
                summary: "Session missing".into(),
            },
        ];

        let mut index = SearchIndex::open_in_memory().unwrap();
        let stats = index_session_summaries(&mut index, "p1", &conn, summaries).unwrap();

        assert_eq!(
            stats,
            SessionIndexStats {
                indexed: 1,
                skipped: 1
            }
        );
    }

    #[test]
    fn index_session_summaries_tracks_skipped_load_errors() {
        use crate::db;
        use crate::models::SessionSummary;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();

        conn.execute("DROP TABLE sessions", []).unwrap();
        let summaries = vec![SessionSummary {
            id: "s1".into(),
            project_id: "p1".into(),
            date: "2026-02-15".into(),
            summary: "Session stale".into(),
        }];

        let mut index = SearchIndex::open_in_memory().unwrap();
        let stats = index_session_summaries(&mut index, "p1", &conn, summaries).unwrap();

        assert_eq!(
            stats,
            SessionIndexStats {
                indexed: 0,
                skipped: 1
            }
        );
    }

    #[test]
    fn index_project_commits_returns_zero_for_non_git_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_commits(&mut index, "p1", dir.path(), 10).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn index_commit_documents_inserts_commit_documents() {
        let mut index = SearchIndex::open_in_memory().unwrap();
        let commits = vec![
            crate::models::Commit {
                hash: "abc12345".to_string(),
                message: "Add feature".to_string(),
                body: None,
                author: "Alice".to_string(),
                date: "1m".to_string(),
                timestamp: 1,
            },
            crate::models::Commit {
                hash: "def67890".to_string(),
                message: "Fix bug".to_string(),
                body: None,
                author: "Bob".to_string(),
                date: "2m".to_string(),
                timestamp: 2,
            },
        ];

        let count = index_commit_documents(&mut index, "p1", &commits, false).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 2);
        assert_eq!(index.doc_count().unwrap(), 2);
        assert_eq!(index.search("feature", 10).unwrap().len(), 1);
        assert_eq!(index.search("bug", 10).unwrap().len(), 1);
    }

    #[test]
    fn index_commit_documents_replaces_existing_hash_when_requested() {
        let mut index = SearchIndex::open_in_memory().unwrap();
        let initial = vec![crate::models::Commit {
            hash: "abc12345".to_string(),
            message: "Old message".to_string(),
            body: None,
            author: "Alice".to_string(),
            date: "1m".to_string(),
            timestamp: 1,
        }];
        let updated = vec![crate::models::Commit {
            hash: "abc12345".to_string(),
            message: "Updated message".to_string(),
            body: None,
            author: "Alice".to_string(),
            date: "1m".to_string(),
            timestamp: 1,
        }];

        index_commit_documents(&mut index, "p1", &initial, false).unwrap();
        index.commit().unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);
        assert_eq!(index.search("Old message", 10).unwrap().len(), 1);

        index_commit_documents(&mut index, "p1", &updated, true).unwrap();
        index.commit().unwrap();

        assert_eq!(index.doc_count().unwrap(), 1);
        assert_eq!(index.search("Updated message", 10).unwrap().len(), 1);
    }

    #[test]
    fn build_project_index_replaces_existing_entries() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.md"), "Original content").unwrap();

        use crate::db;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES ('p1', 'test', '/test', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();

        // First build
        let (files, _, _) = build_project_index(&mut index, "p1", dir.path(), &conn).unwrap();
        assert_eq!(files, 1);
        let count1 = index.doc_count().unwrap();

        // Add another file and rebuild
        std::fs::write(dir.path().join("another.md"), "Second file").unwrap();
        let (files2, _, _) = build_project_index(&mut index, "p1", dir.path(), &conn).unwrap();
        assert_eq!(files2, 2);
        let count2 = index.doc_count().unwrap();

        // rebuild should have replaced, not appended
        assert_eq!(count2, 2);
        assert!(count2 >= count1); // at least as many (replaces old, adds new)
    }

    // --- Incremental update tests ---

    #[test]
    fn update_file_indexes_new_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("hello.md");
        std::fs::write(&file_path, "Hello incremental world").unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 1);

        let results = index.search("incremental", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "hello.md");
    }

    #[test]
    fn update_file_upserts_existing() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("notes.md");

        // Initial content
        std::fs::write(&file_path, "Original text").unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        update_file(&mut index, "p1", dir.path(), &file_path).unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);

        // Update content
        std::fs::write(&file_path, "Updated text with new keywords").unwrap();
        update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        // Should still be 1 doc (upserted, not appended)
        assert_eq!(index.doc_count().unwrap(), 1);

        // New content should be searchable
        let results = index.search("keywords", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn update_file_batch_commits_once_for_multiple_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_a = dir.path().join("a.md");
        let file_b = dir.path().join("b.md");
        let file_c = dir.path().join("c.md");
        std::fs::write(&file_a, "alpha").unwrap();
        std::fs::write(&file_b, "beta").unwrap();
        std::fs::write(&file_c, "gamma").unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        assert!(update_file_batched(&mut index, "p1", dir.path(), &file_a).unwrap());
        assert!(update_file_batched(&mut index, "p1", dir.path(), &file_b).unwrap());
        assert!(update_file_batched(&mut index, "p1", dir.path(), &file_c).unwrap());
        commit_batch(&mut index).unwrap();

        assert_eq!(index.commit_calls(), 1);
        assert_eq!(index.doc_count().unwrap(), 3);
    }

    #[test]
    fn update_file_removes_deleted() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("temp.md");

        // Index the file
        std::fs::write(&file_path, "Temporary content").unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        update_file(&mut index, "p1", dir.path(), &file_path).unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);

        // Delete the file and update
        std::fs::remove_file(&file_path).unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn update_file_skips_non_indexable() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("image.png");
        std::fs::write(&file_path, [0u8; 100]).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        // Should return true (removed from index if it was there) but doc count stays 0
        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn update_file_removes_or_skips_paths_blocked_by_policy() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("generated")).unwrap();
        let file_path = dir.path().join("generated/output.md");
        std::fs::write(&file_path, "should not be searchable").unwrap();

        let policy = ScanIndexPolicy::from_settings(&Settings {
            ignore_patterns: vec!["generated".into()],
            ..Settings::default()
        });

        let mut index = SearchIndex::open_in_memory().unwrap();
        let modified =
            update_file_batched_with_scan_policy(&mut index, "p1", dir.path(), &file_path, &policy)
                .unwrap();
        commit_batch(&mut index).unwrap();

        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 0);
        assert!(index.search("searchable", 10).unwrap().is_empty());
    }

    #[test]
    fn update_file_removes_stale_doc_when_file_becomes_unreadable() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("notes.md");

        std::fs::write(&file_path, "Readable markdown content").unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        update_file(&mut index, "p1", dir.path(), &file_path).unwrap();
        assert_eq!(index.doc_count().unwrap(), 1);
        assert_eq!(index.search("Readable", 10).unwrap().len(), 1);

        // Keep the same indexable extension, but make content unreadable as UTF-8.
        std::fs::write(&file_path, [0xff, 0xfe, 0xfd]).unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 0);
        assert!(index.search("Readable", 10).unwrap().is_empty());
    }

    #[test]
    fn update_file_skips_outside_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let file_path = outside.path().join("rogue.md");
        std::fs::write(&file_path, "Outside content").unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &file_path).unwrap();

        assert!(!modified);
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn update_file_rejects_symlink_target_outside_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let outside_file = outside.path().join("secret.md");
        std::fs::write(&outside_file, "outside secret").unwrap();

        let symlink_path = dir.path().join("notes.md");
        std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let modified = update_file(&mut index, "p1", dir.path(), &symlink_path).unwrap();

        assert!(modified);
        assert_eq!(index.doc_count().unwrap(), 0);
        assert!(index.search("secret", 10).unwrap().is_empty());
    }

    #[test]
    fn index_session_by_id() {
        use crate::db;
        use crate::models::SessionDetail;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();

        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES ('p1', 'test', '/test', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        let session = SessionDetail {
            id: "s-incr-1".into(),
            project_id: "p1".into(),
            date: "2026-02-17".into(),
            summary: "Incremental session indexing test".into(),
            next_steps: vec!["Verify it works".into()],
            open_questions: vec![],
            metadata: serde_json::json!({}),
            file_path: "/test/sessions/s-incr-1.md".into(),
            created_at: "2026-02-17T00:00:00Z".into(),
        };
        db::session_queries::insert_session(&conn, &session).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let indexed = index_session(&mut index, "p1", "s-incr-1", &conn).unwrap();

        assert!(indexed);
        assert_eq!(index.doc_count().unwrap(), 1);

        let results = index.search("incremental session", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].entity_type, "session");
    }

    #[test]
    fn index_session_nonexistent_returns_false() {
        use crate::db;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = db::init_db(tmp.path()).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let indexed = index_session(&mut index, "p1", "nonexistent", &conn).unwrap();

        assert!(!indexed);
        assert_eq!(index.doc_count().unwrap(), 0);
    }

    #[test]
    fn reindex_commits_returns_zero_for_non_git() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = reindex_commits(&mut index, "p1", dir.path(), 50).unwrap();
        assert_eq!(count, 0);
    }
}
