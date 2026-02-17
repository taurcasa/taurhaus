use std::path::Path;

use ignore::WalkBuilder;

use crate::errors::AppError;
use crate::models::FileTreeNode;

/// Build a file tree for a project directory, respecting .gitignore.
/// Returns top-level entries (not the root dir itself).
pub fn build_file_tree(project_root: &Path) -> Result<Vec<FileTreeNode>, AppError> {
    if !project_root.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Not a directory: {}",
            project_root.display()
        )));
    }

    let mut root_children = Vec::new();

    // Use ignore crate's WalkBuilder for .gitignore-aware traversal
    let walker = WalkBuilder::new(project_root)
        .hidden(false) // Don't skip hidden files (we filter .git manually)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .max_depth(Some(1))
        .build();

    for entry in walker {
        let entry = entry.map_err(|e| AppError::Io(std::io::Error::other(e)))?;
        let path = entry.path();

        // Skip the root directory itself
        if path == project_root {
            continue;
        }

        // Skip .git directory
        if let Some(name) = path.file_name() {
            if name == ".git" {
                continue;
            }
        }

        let node = build_node(project_root, path)?;
        root_children.push(node);
    }

    sort_tree(&mut root_children);
    Ok(root_children)
}

/// Recursively build a file tree node.
fn build_node(project_root: &Path, path: &Path) -> Result<FileTreeNode, AppError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let relative = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let is_dir = path.is_dir();

    let children = if is_dir {
        let mut kids = Vec::new();
        let walker = WalkBuilder::new(path)
            .hidden(false)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(true)
            .max_depth(Some(1))
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| {
                AppError::Io(std::io::Error::other(e))
            })?;
            let child_path = entry.path();
            if child_path == path {
                continue;
            }
            if let Some(n) = child_path.file_name() {
                if n == ".git" {
                    continue;
                }
            }
            kids.push(build_node(project_root, child_path)?);
        }
        sort_tree(&mut kids);
        kids
    } else {
        Vec::new()
    };

    Ok(FileTreeNode {
        name,
        path: relative,
        is_dir,
        children,
    })
}

/// Sort: directories first, then files, both alphabetical (case-insensitive).
fn sort_tree(nodes: &mut [FileTreeNode]) {
    nodes.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create directory structure
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub mod foo;").unwrap();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/README.md"), "# Readme").unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();

        dir
    }

    #[test]
    fn build_tree_returns_correct_structure() {
        let dir = setup_project();
        let tree = build_file_tree(dir.path()).unwrap();

        // Should have directories and files at root level
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"src"));
        assert!(names.contains(&"docs"));
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&".gitignore"));
    }

    #[test]
    fn directories_sorted_before_files() {
        let dir = setup_project();
        let tree = build_file_tree(dir.path()).unwrap();

        let first_file_idx = tree.iter().position(|n| !n.is_dir).unwrap();
        let last_dir_idx = tree.iter().rposition(|n| n.is_dir).unwrap();
        assert!(last_dir_idx < first_file_idx);
    }

    #[test]
    fn nested_files_are_included() {
        let dir = setup_project();
        let tree = build_file_tree(dir.path()).unwrap();

        let src = tree.iter().find(|n| n.name == "src").unwrap();
        assert!(src.is_dir);
        let child_names: Vec<&str> = src.children.iter().map(|n| n.name.as_str()).collect();
        assert!(child_names.contains(&"main.rs"));
        assert!(child_names.contains(&"lib.rs"));
    }

    #[test]
    fn gitignored_files_are_excluded() {
        let dir = setup_project();
        let root = dir.path();

        // Initialize git repo so .gitignore is respected
        git2::Repository::init(root).unwrap();

        // Create target/ directory (should be ignored)
        std::fs::create_dir(root.join("target")).unwrap();
        std::fs::write(root.join("target/build.txt"), "build output").unwrap();

        let tree = build_file_tree(root).unwrap();
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(!names.contains(&"target"), "target/ should be ignored");
    }

    #[test]
    fn git_directory_is_excluded() {
        let dir = setup_project();
        let root = dir.path();
        git2::Repository::init(root).unwrap();

        let tree = build_file_tree(root).unwrap();
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(!names.contains(&".git"), ".git should be excluded");
    }

    #[test]
    fn relative_paths_are_correct() {
        let dir = setup_project();
        let tree = build_file_tree(dir.path()).unwrap();

        let src = tree.iter().find(|n| n.name == "src").unwrap();
        assert_eq!(src.path, "src");

        let main = src.children.iter().find(|n| n.name == "main.rs").unwrap();
        assert_eq!(main.path, "src/main.rs");
    }

    #[test]
    fn not_a_directory_returns_error() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hello").unwrap();

        let result = build_file_tree(&file);
        assert!(result.is_err());
    }
}
