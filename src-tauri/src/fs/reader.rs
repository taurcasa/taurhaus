use std::path::Path;

use crate::errors::AppError;
use crate::models::FileContent;

/// Maximum file size we'll read (5 MB). Larger files get a friendly error.
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Read a file from a project directory. Returns raw text content.
///
/// Security: rejects path traversal (..), absolute paths, and symlink escapes
/// via canonicalization check.
pub fn read_file(project_root: &Path, relative_path: &str) -> Result<FileContent, AppError> {
    // Reject absolute paths
    if Path::new(relative_path).is_absolute() {
        return Err(AppError::InvalidPath(
            "Absolute paths not allowed".to_string(),
        ));
    }

    // Reject ".." path components
    if relative_path.contains("..") {
        return Err(AppError::InvalidPath(
            "Path traversal not allowed".to_string(),
        ));
    }

    let full_path = project_root.join(relative_path);
    if !full_path.exists() {
        return Err(AppError::NotFound(format!(
            "File not found: {relative_path}"
        )));
    }

    // Canonicalize both paths and verify the file is inside the project root.
    // This catches symlink escapes and other resolution tricks.
    let canonical = full_path.canonicalize().map_err(|_| {
        AppError::NotFound(format!("Cannot resolve path: {relative_path}"))
    })?;
    let canonical_root = project_root.canonicalize().map_err(|_| {
        AppError::InvalidPath("Cannot resolve project root".to_string())
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::InvalidPath(
            "Path resolves outside project directory".to_string(),
        ));
    }

    if !full_path.is_file() {
        return Err(AppError::InvalidPath(format!(
            "Not a file: {relative_path}"
        )));
    }

    // Check file size before reading
    let metadata = std::fs::metadata(&full_path)?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AppError::InvalidPath(
            "File too large to display (>5 MB)".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&full_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            AppError::InvalidPath("Binary file cannot be read as text".to_string())
        } else {
            AppError::Io(e)
        }
    })?;

    let language = detect_language(relative_path);

    Ok(FileContent {
        path: relative_path.to_string(),
        content,
        language,
    })
}

/// Detect programming language from file extension.
fn detect_language(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    let lang = match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "svelte" => "svelte",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "py" => "python",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "xml" => "xml",
        "txt" => "plaintext",
        _ => return None,
    };
    Some(lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# Hello\nWorld").unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub mod foo;").unwrap();
        dir
    }

    #[test]
    fn read_file_returns_content() {
        let dir = setup();
        let content = read_file(dir.path(), "readme.md").unwrap();
        assert_eq!(content.path, "readme.md");
        assert_eq!(content.content, "# Hello\nWorld");
        assert_eq!(content.language, Some("markdown".to_string()));
    }

    #[test]
    fn read_nested_file() {
        let dir = setup();
        let content = read_file(dir.path(), "src/lib.rs").unwrap();
        assert_eq!(content.path, "src/lib.rs");
        assert_eq!(content.language, Some("rust".to_string()));
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = setup();
        let result = read_file(dir.path(), "../etc/passwd");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath(msg) => assert!(msg.contains("traversal")),
            e => panic!("Expected InvalidPath, got: {e:?}"),
        }
    }

    #[test]
    fn file_not_found() {
        let dir = setup();
        let result = read_file(dir.path(), "nonexistent.txt");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NotFound(msg) => assert!(msg.contains("nonexistent.txt")),
            e => panic!("Expected NotFound, got: {e:?}"),
        }
    }

    #[test]
    fn directory_returns_error() {
        let dir = setup();
        let result = read_file(dir.path(), "src");
        assert!(result.is_err());
    }

    #[test]
    fn detect_language_rust() {
        assert_eq!(detect_language("main.rs"), Some("rust".to_string()));
    }

    #[test]
    fn detect_language_javascript() {
        assert_eq!(detect_language("index.js"), Some("javascript".to_string()));
    }

    #[test]
    fn detect_language_unknown() {
        assert_eq!(detect_language("file.xyz"), None);
    }

    #[test]
    fn detect_language_no_extension() {
        // "Makefile" → ext = "Makefile" which doesn't match
        assert_eq!(detect_language("Makefile"), None);
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = setup();
        let result = read_file(dir.path(), "/etc/passwd");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath(msg) => assert!(msg.contains("Absolute")),
            e => panic!("Expected InvalidPath, got: {e:?}"),
        }
    }

    #[test]
    fn rejects_file_outside_project_via_canonicalization() {
        let dir = setup();
        // Even if someone constructs a path that doesn't contain ".."
        // but resolves outside, canonicalization catches it.
        // This test creates a symlink to /tmp and verifies it's rejected.
        #[cfg(unix)]
        {
            let link_path = dir.path().join("escape");
            std::os::unix::fs::symlink("/tmp", &link_path).unwrap();
            // Try to read a file through the symlink
            // The symlink itself resolves outside project root
            let result = read_file(dir.path(), "escape");
            // This should either fail with InvalidPath (symlink resolves outside)
            // or NotFound. Either way, it should NOT succeed.
            assert!(result.is_err());
        }
    }
}
