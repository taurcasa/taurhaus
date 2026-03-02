use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::snippet::SnippetGenerator;
use tantivy::TantivyDocument;

use super::indexer::SearchIndex;
use crate::errors::AppError;

/// A single search result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub project_id: String,
    pub entity_type: String,
    pub file_path: String,
    pub title: String,
    pub snippet: String,
    pub relevance_score: f32,
}

impl SearchIndex {
    /// Execute a full-text search query and return ranked results.
    ///
    /// Searches across title and content fields using BM25 ranking.
    /// Returns up to `limit` results with snippet extraction.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, AppError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let reader = self
            .index()
            .reader()
            .map_err(|e| AppError::SearchError(format!("Failed to create reader: {e}")))?;
        let searcher = reader.searcher();

        let query_parser =
            QueryParser::for_index(self.index(), vec![self.fields.title, self.fields.content]);

        let parsed_query = query_parser
            .parse_query(trimmed)
            .map_err(|e| AppError::SearchError(format!("Failed to parse query: {e}")))?;

        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .map_err(|e| AppError::SearchError(format!("Search failed: {e}")))?;

        // Set up snippet generator for the content field
        let snippet_generator =
            SnippetGenerator::create(&searcher, &parsed_query, self.fields.content)
                .map_err(|e| AppError::SearchError(format!("Snippet generation failed: {e}")))?;

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| AppError::SearchError(format!("Failed to retrieve document: {e}")))?;

            let project_id = field_text(&doc, self.fields.project_id);
            let entity_type = field_text(&doc, self.fields.entity_type);
            let file_path = field_text(&doc, self.fields.file_path);
            let title = field_text(&doc, self.fields.title);

            let snippet_obj = snippet_generator.snippet_from_doc(&doc);
            // Use fragment() for plain text (no HTML tags) — safe for direct rendering
            let snippet = snippet_obj.fragment().to_string();
            // If no snippet generated, use title as fallback
            let snippet = if snippet.trim().is_empty() {
                title.clone()
            } else {
                snippet
            };

            results.push(SearchResult {
                project_id,
                entity_type,
                file_path,
                title,
                snippet,
                relevance_score: score,
            });
        }

        Ok(results)
    }
}

/// Extract the first text value from a field in a tantivy document.
fn field_text(doc: &TantivyDocument, field: tantivy::schema::Field) -> String {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_with_docs() -> SearchIndex {
        let mut idx = SearchIndex::open_in_memory().unwrap();
        idx.add_document(
            "p1",
            "document",
            "README.md",
            "README",
            "This project manages AI sessions and provides a dashboard for tracking progress.",
        )
        .unwrap();
        idx.add_document(
            "p1",
            "session",
            "session-1.md",
            "Phase 5A Complete",
            "Completed scaffolding with SQLite database and project CRUD operations.",
        )
        .unwrap();
        idx.add_document(
            "p1",
            "commit",
            "abc123",
            "Add tantivy search",
            "Implement full-text search using tantivy with BM25 ranking.",
        )
        .unwrap();
        idx.add_document(
            "p2",
            "document",
            "src/lib.rs",
            "lib.rs",
            "Rust library implementing the core business logic for the application.",
        )
        .unwrap();
        idx.commit().unwrap();
        idx
    }

    #[test]
    fn search_finds_matching_document() {
        let idx = index_with_docs();
        let results = idx.search("dashboard", 10).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "README.md");
        assert_eq!(results[0].entity_type, "document");
    }

    #[test]
    fn search_finds_session() {
        let idx = index_with_docs();
        let results = idx.search("scaffolding SQLite", 10).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].entity_type, "session");
        assert_eq!(results[0].file_path, "session-1.md");
    }

    #[test]
    fn search_finds_commit() {
        let idx = index_with_docs();
        let results = idx.search("tantivy BM25", 10).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].entity_type, "commit");
    }

    #[test]
    fn search_returns_ranked_results() {
        let idx = index_with_docs();
        let results = idx.search("project", 10).unwrap();

        // Multiple matches expected — scores should be descending
        assert!(results.len() >= 2);
        for window in results.windows(2) {
            assert!(window[0].relevance_score >= window[1].relevance_score);
        }
    }

    #[test]
    fn search_respects_limit() {
        let idx = index_with_docs();
        let results = idx.search("project", 1).unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_snippet_contains_match_term() {
        let idx = index_with_docs();
        let results = idx.search("dashboard", 10).unwrap();

        assert!(!results.is_empty());
        // Snippet should contain the match (possibly with HTML highlight tags)
        let snippet_lower = results[0].snippet.to_lowercase();
        assert!(
            snippet_lower.contains("dashboard"),
            "Snippet should contain 'dashboard', got: {}",
            results[0].snippet
        );
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let idx = index_with_docs();
        let results = idx.search("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_whitespace_query_returns_empty() {
        let idx = index_with_docs();
        let results = idx.search("   ", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_no_matches_returns_empty() {
        let idx = index_with_docs();
        let results = idx.search("xyzzynonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_zero_limit_returns_empty() {
        let idx = index_with_docs();
        let results = idx.search("dashboard", 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_result_has_all_fields() {
        let idx = index_with_docs();
        let results = idx.search("dashboard", 10).unwrap();

        assert!(!results.is_empty());
        let r = &results[0];
        assert!(!r.project_id.is_empty());
        assert!(!r.entity_type.is_empty());
        assert!(!r.file_path.is_empty());
        assert!(!r.title.is_empty());
        assert!(!r.snippet.is_empty());
        assert!(r.relevance_score > 0.0);
    }

    #[test]
    fn search_across_projects() {
        let idx = index_with_docs();
        let results = idx.search("Rust library", 10).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].project_id, "p2");
    }
}
