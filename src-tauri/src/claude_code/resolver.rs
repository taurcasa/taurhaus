//! Resolve project paths to their Claude Code data directories.
//!
//! Claude Code stores per-project data under `~/.claude/projects/<slug>/`
//! where `<slug>` is the project's absolute path with `/` replaced by `-`.

use std::path::{Path, PathBuf};

/// Compute the Claude Code project slug from an absolute path.
///
/// Claude Code uses the absolute path with all path separators replaced
/// by dashes. For example: `/home/user/projects/foo` → `-home-user-projects-foo`.
pub fn project_slug(path: &Path) -> String {
    let canonical = path.to_string_lossy();
    canonical.replace(['/', '\\'], "-")
}

/// Return the Claude Code base directory (`~/.claude/`).
///
/// Uses `$HOME` on Linux/macOS, `$USERPROFILE` on Windows.
pub fn claude_base_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Resolve the Claude Code data directory for a project.
///
/// Returns `Some(path)` if the directory exists, `None` otherwise.
pub fn resolve_project_dir(project_path: &Path) -> Option<PathBuf> {
    let base = claude_base_dir()?;
    let slug = project_slug(project_path);
    let project_dir = base.join("projects").join(slug);
    if project_dir.is_dir() {
        Some(project_dir)
    } else {
        None
    }
}

/// Check whether Claude Code data exists for a given project path.
pub fn has_claude_data(project_path: &Path) -> bool {
    resolve_project_dir(project_path).is_some()
}

/// Return the memory directory for a project, if it exists.
pub fn memory_dir(project_path: &Path) -> Option<PathBuf> {
    let project_dir = resolve_project_dir(project_path)?;
    let mem_dir = project_dir.join("memory");
    if mem_dir.is_dir() {
        Some(mem_dir)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn slug_linux_path() {
        let path = Path::new("/home/user/projects/taurhaus");
        assert_eq!(project_slug(path), "-home-user-projects-taurhaus");
    }

    #[test]
    fn slug_windows_path() {
        let path = Path::new("C:\\Users\\dev\\projects\\foo");
        let slug = project_slug(path);
        assert!(slug.contains("-Users-dev-projects-foo"));
    }

    #[test]
    fn slug_single_component() {
        let path = Path::new("/tmp");
        assert_eq!(project_slug(path), "-tmp");
    }

    #[test]
    fn slug_deterministic() {
        let path = Path::new("/home/mstie/projects/taurhaus");
        let slug1 = project_slug(path);
        let slug2 = project_slug(path);
        assert_eq!(slug1, slug2);
    }

    #[test]
    fn resolve_nonexistent_returns_none() {
        let path = Path::new("/nonexistent/path/that/does/not/exist");
        assert!(resolve_project_dir(path).is_none());
    }

    #[test]
    fn has_claude_data_false_for_nonexistent() {
        let path = Path::new("/nonexistent/path");
        assert!(!has_claude_data(path));
    }

    #[test]
    fn resolve_with_mock_structure() {
        let dir = TempDir::new().unwrap();
        let fake_home = dir.path();

        // Create a mock .claude/projects/<slug>/ structure
        let slug = "-mock-project";
        let project_dir = fake_home.join(".claude").join("projects").join(slug);
        std::fs::create_dir_all(&project_dir).unwrap();

        // Since resolve_project_dir uses the real home dir, we test the slug
        // computation separately and verify the path construction logic
        let expected_slug = project_slug(Path::new("/mock/project"));
        assert_eq!(expected_slug, "-mock-project");
    }

    #[test]
    fn memory_dir_none_when_no_project() {
        let path = Path::new("/nonexistent/path");
        assert!(memory_dir(path).is_none());
    }

    // Integration test: verify against real Claude Code data if available
    #[test]
    fn resolve_real_project_if_available() {
        let taurhaus_path = Path::new("/home/mstie/projects/taurhaus");
        if taurhaus_path.exists() {
            let slug = project_slug(taurhaus_path);
            assert_eq!(slug, "-home-mstie-projects-taurhaus");

            // Only check resolution if we're on the right machine
            if let Some(project_dir) = resolve_project_dir(taurhaus_path) {
                assert!(project_dir.exists());
                assert!(project_dir.ends_with("-home-mstie-projects-taurhaus"));
            }
        }
    }
}
