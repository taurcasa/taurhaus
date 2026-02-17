use std::path::Path;

use tantivy::schema::{Field, Schema, STORED, STRING, TEXT};
use tantivy::{Index, IndexWriter, TantivyDocument};

use crate::errors::AppError;

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

        let dir = tantivy::directory::MmapDirectory::open(index_dir).map_err(|e| {
            AppError::SearchError(format!("Failed to open index directory: {e}"))
        })?;

        let index = if Index::exists(&dir).map_err(|e| {
            AppError::SearchError(format!("Failed to check index existence: {e}"))
        })? {
            Index::open(dir).map_err(|e| {
                AppError::SearchError(format!("Failed to open existing index: {e}"))
            })?
        } else {
            Index::create(dir, schema.clone(), Default::default()).map_err(|e| {
                AppError::SearchError(format!("Failed to create index: {e}"))
            })?
        };

        let writer = index.writer(WRITER_HEAP_SIZE).map_err(|e| {
            AppError::SearchError(format!("Failed to create index writer: {e}"))
        })?;

        Ok(Self {
            index,
            writer,
            fields,
            schema,
        })
    }

    /// Create an in-memory index (for testing).
    pub fn open_in_memory() -> Result<Self, AppError> {
        let (schema, fields) = build_schema();

        let index = Index::create_in_ram(schema.clone());

        let writer = index.writer(WRITER_HEAP_SIZE).map_err(|e| {
            AppError::SearchError(format!("Failed to create index writer: {e}"))
        })?;

        Ok(Self {
            index,
            writer,
            fields,
            schema,
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

        self.writer.add_document(doc).map_err(|e| {
            AppError::SearchError(format!("Failed to add document: {e}"))
        })?;
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
        self.writer.commit().map_err(|e| {
            AppError::SearchError(format!("Failed to commit index: {e}"))
        })?;
        Ok(())
    }

    /// Clear the entire index (delete all documents).
    pub fn clear(&mut self) -> Result<(), AppError> {
        self.writer.delete_all_documents().map_err(|e| {
            AppError::SearchError(format!("Failed to clear index: {e}"))
        })?;
        self.commit()
    }

    /// Return a reference to the underlying tantivy Index (for readers/searchers).
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Count the total number of documents in the index.
    pub fn doc_count(&self) -> Result<u64, AppError> {
        let reader = self.index.reader().map_err(|e| {
            AppError::SearchError(format!("Failed to create reader: {e}"))
        })?;
        let searcher = reader.searcher();
        let total: u64 = searcher
            .segment_readers()
            .iter()
            .map(|r| r.num_docs() as u64)
            .sum();
        Ok(total)
    }
}

// ---------------------------------------------------------------------------
// Bulk index builder
// ---------------------------------------------------------------------------

/// Max file size for indexing (1 MB — larger files are skipped).
const MAX_INDEX_FILE_SIZE: u64 = 1024 * 1024;

/// Extensions considered as text files for indexing.
const TEXT_EXTENSIONS: &[&str] = &[
    "md", "txt", "rs", "js", "ts", "svelte", "html", "css", "json", "toml",
    "yaml", "yml", "xml", "sql", "sh", "bash", "zsh", "py", "rb", "go",
    "java", "kt", "c", "cpp", "h", "hpp", "lua", "vim", "conf", "cfg",
    "ini", "env", "lock", "gitignore", "editorconfig",
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
    use ignore::WalkBuilder;

    let mut count = 0;

    let walker = WalkBuilder::new(project_root)
        .follow_links(false)
        .hidden(true) // skip hidden files
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
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
            .to_string();

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
pub fn index_project_sessions(
    index: &mut SearchIndex,
    project_id: &str,
    conn: &rusqlite::Connection,
) -> Result<usize, AppError> {
    use crate::db::session_queries;

    let sessions = session_queries::list_sessions(conn, project_id, 1000, 0)?;
    let mut count = 0;

    for summary in sessions {
        // Get the full session detail for next_steps / open_questions
        if let Ok(Some(detail)) = session_queries::get_session(conn, &summary.id) {
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
            let title_prefix = if detail.summary.len() > 60 {
                &detail.summary[..60]
            } else {
                &detail.summary
            };

            index.add_document(project_id, "session", &file_path, title_prefix, &content)?;
            count += 1;
        }
    }

    Ok(count)
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

    let mut count = 0;
    for commit in commits {
        let file_path = format!("commit:{}", commit.hash);
        index.add_document(
            project_id,
            "commit",
            &file_path,
            &commit.message,
            &format!("{} — {} ({})", commit.message, commit.author, commit.date),
        )?;
        count += 1;
    }

    Ok(count)
}

/// Build the full index for a single project (files + sessions + commits).
pub fn build_project_index(
    index: &mut SearchIndex,
    project_id: &str,
    project_root: &Path,
    conn: &rusqlite::Connection,
) -> Result<(usize, usize, usize), AppError> {
    // Remove existing entries for this project before re-indexing
    index.remove_by_project(project_id);

    let files = index_project_files(index, project_id, project_root)?;
    let sessions = index_project_sessions(index, project_id, conn)?;
    let commits = index_project_commits(index, project_id, project_root, 100)?;

    index.commit()?;

    Ok((files, sessions, commits))
}

/// Rebuild the entire index from scratch (all projects).
pub fn rebuild_all(
    index: &mut SearchIndex,
    conn: &rusqlite::Connection,
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

        let (files, sessions, commits) = build_project_index(index, &project.id, project_root, conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
            .add_document("p1", "session", "session-1.md", "Phase 5A", "Scaffolding complete")
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

        index.add_document("p1", "document", "a.md", "A", "Content A").unwrap();
        index.add_document("p2", "document", "b.md", "B", "Content B").unwrap();
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

        index.add_document("p1", "document", "README.md", "README", "docs").unwrap();
        index.add_document("p1", "session", "s1", "Phase 5A", "scaffold done").unwrap();
        index.add_document("p1", "commit", "abc123", "Add feature", "commit msg").unwrap();
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
        std::fs::write(dir.path().join("image.png"), &[0u8; 100]).unwrap();

        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_files(&mut index, "p1", dir.path()).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 2); // README.md + src/main.rs
        assert_eq!(index.doc_count().unwrap(), 2);
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
        let count = index_project_sessions(&mut index, "p1", &conn).unwrap();
        index.commit().unwrap();

        assert_eq!(count, 1);
        // Verify the session is searchable
        let results = index.search("scaffold", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].entity_type, "session");
    }

    #[test]
    fn index_project_commits_returns_zero_for_non_git_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut index = SearchIndex::open_in_memory().unwrap();
        let count = index_project_commits(&mut index, "p1", dir.path(), 10).unwrap();
        assert_eq!(count, 0);
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
}
