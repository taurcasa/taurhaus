use std::path::Path;

use crate::services::scan_policy::{ScanIndexMatcher, ScanIndexPolicy};

/// A project discovered during directory scanning.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredProject {
    pub path: String,
    pub name: String,
    pub has_git: bool,
}

/// Scan a directory for project directories up to `max_depth` levels deep.
///
/// Identifies directories containing a `.git/` folder as git repos.
/// Skips hidden directories (starting with `.`) at the top level, and
/// common non-project directories (node_modules, target, etc.) at all levels.
pub fn scan_directory(root: &Path, max_depth: u32) -> Result<Vec<DiscoveredProject>, String> {
    scan_directory_with_policy(root, max_depth, &ScanIndexPolicy::default())
}

pub fn scan_directory_with_policy(
    root: &Path,
    max_depth: u32,
    policy: &ScanIndexPolicy,
) -> Result<Vec<DiscoveredProject>, String> {
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let mut discovered = Vec::new();
    let matcher = policy.matcher_for_root(root);
    scan_recursive(root, 1, max_depth, &matcher, &mut discovered)?;
    discovered.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(discovered)
}

fn scan_recursive(
    current: &Path,
    depth: u32,
    max_depth: u32,
    matcher: &ScanIndexMatcher,
    results: &mut Vec<DiscoveredProject>,
) -> Result<(), String> {
    if depth > max_depth {
        return Ok(());
    }

    let entries = std::fs::read_dir(current)
        .map_err(|e| format!("Cannot read directory {}: {e}", current.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();

        if !entry_path.is_dir() {
            continue;
        }

        let name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip hidden directories
        if name.starts_with('.') {
            continue;
        }

        if matcher.ignores_path(&entry_path, true) {
            continue;
        }

        let has_git = entry_path.join(".git").is_dir();

        if has_git {
            // This is a project — add it and don't recurse deeper
            results.push(DiscoveredProject {
                path: entry_path.to_string_lossy().to_string(),
                name,
                has_git: true,
            });
        } else {
            // Not a git repo — could be a container directory, recurse
            // But still record it at depth 1 as a non-git project candidate
            if depth == 1 {
                results.push(DiscoveredProject {
                    path: entry_path.to_string_lossy().to_string(),
                    name,
                    has_git: false,
                });
            }
            scan_recursive(&entry_path, depth + 1, max_depth, matcher, results)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;
    use crate::services::scan_policy::ScanIndexPolicy;
    use tempfile::TempDir;

    fn setup_project_tree() -> TempDir {
        let root = TempDir::new().unwrap();

        // Direct git project
        let p1 = root.path().join("project-a");
        std::fs::create_dir_all(p1.join(".git")).unwrap();

        // Non-git directory at depth 1
        let p2 = root.path().join("project-b");
        std::fs::create_dir(&p2).unwrap();

        // Nested git project at depth 2 (inside a container dir)
        let container = root.path().join("work");
        let nested = container.join("nested-project");
        std::fs::create_dir_all(nested.join(".git")).unwrap();

        // Hidden directory (should be skipped)
        std::fs::create_dir(root.path().join(".hidden")).unwrap();

        // node_modules (should be skipped)
        std::fs::create_dir(root.path().join("node_modules")).unwrap();

        root
    }

    // AC1: Discovers git repos at depth 1
    #[test]
    fn finds_git_repos_at_depth_1() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 1).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"project-a"));
        assert!(
            results
                .iter()
                .find(|d| d.name == "project-a")
                .unwrap()
                .has_git
        );
    }

    // AC1b: Discovers git repos at depth 2
    #[test]
    fn finds_git_repos_at_depth_2() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 2).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"project-a"));
        assert!(names.contains(&"nested-project"));
    }

    // AC1c: Respects depth limit
    #[test]
    fn respects_depth_limit() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 1).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(
            !names.contains(&"nested-project"),
            "Should not find depth-2 project at depth 1"
        );
    }

    // AC2: Skips hidden directories
    #[test]
    fn skips_hidden_dirs() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 2).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(!names.contains(&".hidden"));
    }

    // AC2b: Skips excluded directories
    #[test]
    fn skips_excluded_dirs() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 2).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(!names.contains(&"node_modules"));
    }

    // AC2c: Skips excluded dirs at depth 2
    #[test]
    fn skips_excluded_dirs_at_depth_2() {
        let root = TempDir::new().unwrap();
        let container = root.path().join("work");
        std::fs::create_dir(&container).unwrap();
        std::fs::create_dir(container.join("target")).unwrap();
        std::fs::create_dir(container.join("node_modules")).unwrap();

        let results = scan_directory(root.path(), 2).unwrap();
        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        assert!(!names.contains(&"target"));
        assert!(!names.contains(&"node_modules"));
    }

    // Returns empty for empty directory
    #[test]
    fn empty_dir_returns_empty() {
        let root = TempDir::new().unwrap();
        let results = scan_directory(root.path(), 2).unwrap();
        assert!(results.is_empty());
    }

    // Error for nonexistent path
    #[test]
    fn nonexistent_path_returns_error() {
        let result = scan_directory(Path::new("/nonexistent/path"), 2);
        assert!(result.is_err());
    }

    // Results are sorted by name
    #[test]
    fn results_sorted_by_name() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 2).unwrap();

        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    // Non-git directory at depth 1 is still returned
    #[test]
    fn non_git_dir_at_depth_1_returned() {
        let root = setup_project_tree();
        let results = scan_directory(root.path(), 1).unwrap();

        let p2 = results.iter().find(|d| d.name == "project-b");
        assert!(p2.is_some());
        assert!(!p2.unwrap().has_git);
    }

    #[test]
    fn saved_ignore_patterns_skip_matching_directories() {
        let root = TempDir::new().unwrap();
        let kept = root.path().join("project-a");
        std::fs::create_dir_all(kept.join(".git")).unwrap();

        let ignored_container = root.path().join("vendor");
        let ignored = ignored_container.join("project-b");
        std::fs::create_dir_all(ignored.join(".git")).unwrap();

        let policy = ScanIndexPolicy::from_settings(&Settings {
            ignore_patterns: vec!["vendor".into()],
            ..Settings::default()
        });

        let results = scan_directory_with_policy(root.path(), 2, &policy).unwrap();
        let names: Vec<&str> = results.iter().map(|d| d.name.as_str()).collect();

        assert!(names.contains(&"project-a"));
        assert!(!names.contains(&"vendor"));
        assert!(!names.contains(&"project-b"));
    }
}
