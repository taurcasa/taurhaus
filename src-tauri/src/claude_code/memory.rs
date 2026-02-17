//! Parse Claude Code auto-memory files (v1.1 UI).
//!
//! Memory files live at `~/.claude/projects/<slug>/memory/`.
//! - `MEMORY.md` is the main memory file (always loaded into system prompt)
//! - `*.md` are topic-specific files (debugging.md, patterns.md, etc.)

use std::path::{Path, PathBuf};

use crate::errors::AppError;

/// A single Claude Code memory file.
#[derive(Debug, Clone)]
pub struct MemoryFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// File content.
    pub content: String,
    /// Whether this is the main MEMORY.md file.
    pub is_main: bool,
}

/// Read all memory files from a Claude Code project memory directory.
///
/// Returns an empty vec if the directory doesn't exist or is empty.
pub fn read_memory_files(memory_dir: &Path) -> Result<Vec<MemoryFile>, AppError> {
    if !memory_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

    for entry in std::fs::read_dir(memory_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .md files
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_main = filename == "MEMORY.md";

        files.push(MemoryFile {
            path,
            content,
            is_main,
        });
    }

    // Sort: MEMORY.md first, then alphabetical
    files.sort_by(|a, b| {
        match (a.is_main, b.is_main) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.path.cmp(&b.path),
        }
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_empty_dir() {
        let dir = TempDir::new().unwrap();
        let files = read_memory_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn read_nonexistent_dir() {
        let files = read_memory_files(Path::new("/nonexistent/memory")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn read_memory_with_main_and_topics() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("MEMORY.md"), "# Main memory").unwrap();
        std::fs::write(dir.path().join("debugging.md"), "# Debugging notes").unwrap();
        std::fs::write(dir.path().join("patterns.md"), "# Patterns").unwrap();
        // Non-md file should be ignored
        std::fs::write(dir.path().join("notes.txt"), "text file").unwrap();

        let files = read_memory_files(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].is_main);
        assert_eq!(files[0].content, "# Main memory");
        assert!(!files[1].is_main);
        assert!(!files[2].is_main);
    }

    #[test]
    fn main_memory_sorted_first() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("zebra.md"), "z").unwrap();
        std::fs::write(dir.path().join("MEMORY.md"), "main").unwrap();
        std::fs::write(dir.path().join("alpha.md"), "a").unwrap();

        let files = read_memory_files(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].is_main);
        assert_eq!(files[0].content, "main");
    }

    #[test]
    fn only_main_memory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("MEMORY.md"), "# Project memory").unwrap();

        let files = read_memory_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].is_main);
    }
}
