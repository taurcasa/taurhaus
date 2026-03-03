use std::path::{Component, Path};

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

    // Reject only actual parent-directory traversal components.
    if Path::new(relative_path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
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
    let canonical = full_path
        .canonicalize()
        .map_err(|_| AppError::NotFound(format!("Cannot resolve path: {relative_path}")))?;
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| AppError::InvalidPath("Cannot resolve project root".to_string()))?;
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
///
/// Maps file extensions to Shiki-compatible language identifiers. For extensions
/// where the Shiki ID differs from the extension (e.g., .rs → "rust"), we map
/// explicitly. For everything else, we pass the raw extension — Shiki's full
/// bundle will load the grammar on demand if it exists, or the frontend falls
/// back to plaintext.
fn detect_language(path: &str) -> Option<String> {
    let lower = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?
        .to_lowercase();

    // Map extensions where the Shiki language ID differs from the extension
    let lang = match lower.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "htm" => "html",
        "yml" => "yaml",
        "md" => "markdown",
        "mdx" => "mdx",
        "py" | "pyw" => "python",
        "sh" | "bash" | "zsh" | "fish" => "shellscript",
        "patch" => "diff",
        "jsonc" | "json5" => "jsonc",
        "txt" | "text" | "log" => "plaintext",
        // For everything else, pass the extension as-is — Shiki knows its own catalog
        _ => return Some(lower),
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
    fn allows_filename_containing_double_dot() {
        let dir = setup();
        std::fs::write(dir.path().join("release..notes.md"), "safe").unwrap();

        let content = read_file(dir.path(), "release..notes.md").unwrap();
        assert_eq!(content.content, "safe");
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
    fn detect_language_passes_unknown_extension_through() {
        // Unknown extensions are passed as-is — Shiki will try to load them
        assert_eq!(detect_language("file.xyz"), Some("xyz".to_string()));
        assert_eq!(detect_language("scene.ron"), Some("ron".to_string()));
        assert_eq!(detect_language("shader.wgsl"), Some("wgsl".to_string()));
        assert_eq!(detect_language("shader.glsl"), Some("glsl".to_string()));
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
