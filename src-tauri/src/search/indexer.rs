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
}
