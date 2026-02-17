use std::path::Path;

use crate::errors::AppError;
use crate::models::FileContent;

/// Common README filenames in priority order.
const README_NAMES: &[&str] = &[
    "README.md",
    "readme.md",
    "Readme.md",
    "README.txt",
    "readme.txt",
    "README",
    "readme",
];

/// Find and read the README file in a project directory.
/// Returns None if no README is found.
pub fn find_readme(project_root: &Path) -> Result<Option<FileContent>, AppError> {
    for name in README_NAMES {
        let path = project_root.join(name);
        if path.is_file() {
            let content = std::fs::read_to_string(&path)?;
            let language = if name.ends_with(".md") {
                Some("markdown".to_string())
            } else {
                Some("plaintext".to_string())
            };
            return Ok(Some(FileContent {
                path: name.to_string(),
                content,
                language,
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn finds_readme_md() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Project").unwrap();

        let result = find_readme(dir.path()).unwrap();
        assert!(result.is_some());
        let readme = result.unwrap();
        assert_eq!(readme.path, "README.md");
        assert_eq!(readme.content, "# Project");
        assert_eq!(readme.language, Some("markdown".to_string()));
    }

    #[test]
    fn finds_lowercase_readme() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# Hello").unwrap();

        let result = find_readme(dir.path()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, "readme.md");
    }

    #[test]
    fn finds_readme_txt() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.txt"), "Project info").unwrap();

        let result = find_readme(dir.path()).unwrap();
        assert!(result.is_some());
        let readme = result.unwrap();
        assert_eq!(readme.language, Some("plaintext".to_string()));
    }

    #[test]
    fn returns_none_when_no_readme() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let result = find_readme(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn prefers_readme_md_over_txt() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Markdown").unwrap();
        std::fs::write(dir.path().join("README.txt"), "Plain text").unwrap();

        let result = find_readme(dir.path()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, "README.md");
    }
}
