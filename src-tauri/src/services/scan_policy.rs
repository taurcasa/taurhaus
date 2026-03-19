use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rusqlite::Connection;

use crate::db::settings_queries;
use crate::errors::AppError;
use crate::models::Settings;
use crate::sentinels::PYTHON_CACHE_DIR;

const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    ".cache",
    ".next",
    PYTHON_CACHE_DIR,
    ".venv",
    "venv",
    ".tox",
    "build",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIndexPolicy {
    scan_directories: Vec<String>,
    ignore_patterns: Vec<String>,
}

impl Default for ScanIndexPolicy {
    fn default() -> Self {
        Self::from_settings(&Settings::default())
    }
}

impl ScanIndexPolicy {
    pub fn load(conn: &Connection) -> Result<Self, AppError> {
        let settings = settings_queries::get_all_settings(conn)?;
        Ok(Self::from_settings(&settings))
    }

    pub fn from_settings(settings: &Settings) -> Self {
        let scan_directories = normalized_unique(settings.scan_directories.iter().cloned());

        let mut ignore_patterns = normalized_unique(
            DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|pattern| pattern.to_string()),
        );
        for pattern in normalized_unique(settings.ignore_patterns.iter().cloned()) {
            if !ignore_patterns.contains(&pattern) {
                ignore_patterns.push(pattern);
            }
        }

        Self {
            scan_directories,
            ignore_patterns,
        }
    }

    pub fn scan_directories(&self) -> &[String] {
        &self.scan_directories
    }

    pub fn ignore_patterns(&self) -> &[String] {
        &self.ignore_patterns
    }

    pub fn matcher_for_root(&self, root: &Path) -> ScanIndexMatcher {
        ScanIndexMatcher::new(root, &self.ignore_patterns)
    }
}

#[derive(Debug, Clone)]
pub struct ScanIndexMatcher {
    matcher: Gitignore,
}

impl ScanIndexMatcher {
    fn new(root: &Path, patterns: &[String]) -> Self {
        let mut builder = GitignoreBuilder::new(root);

        for pattern in patterns {
            if let Err(error) = builder.add_line(None, pattern) {
                tracing::warn!(
                    pattern,
                    error = %error,
                    "scanner/index policy ignored invalid pattern"
                );
            }
        }

        let matcher = builder.build().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                "scanner/index policy matcher build failed; falling back to empty matcher"
            );
            GitignoreBuilder::new(root)
                .build()
                .expect("empty gitignore matcher should build")
        });

        Self { matcher }
    }

    pub fn ignores_path(&self, path: &Path, is_dir: bool) -> bool {
        self.matcher
            .matched_path_or_any_parents(path, is_dir)
            .is_ignore()
    }
}

fn normalized_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let owned = trimmed.to_string();
        if !normalized.contains(&owned) {
            normalized.push(owned);
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_merges_defaults_and_saved_patterns() {
        let settings = Settings {
            scan_directories: vec!["  ~/projects  ".into(), "".into(), "~/projects".into()],
            ignore_patterns: vec![" dist ".into(), "generated".into(), "".into()],
            ..Settings::default()
        };

        let policy = ScanIndexPolicy::from_settings(&settings);

        assert_eq!(policy.scan_directories(), &["~/projects".to_string()]);
        assert!(policy.ignore_patterns().contains(&"dist".to_string()));
        assert!(policy.ignore_patterns().contains(&"generated".to_string()));
        assert_eq!(
            policy
                .ignore_patterns()
                .iter()
                .filter(|pattern| pattern.as_str() == "dist")
                .count(),
            1
        );
    }

    #[test]
    fn matcher_ignores_matching_paths_and_descendants() {
        let settings = Settings {
            ignore_patterns: vec!["generated".into()],
            ..Settings::default()
        };
        let policy = ScanIndexPolicy::from_settings(&settings);
        let root = tempfile::TempDir::new().expect("temp dir");
        let matcher = policy.matcher_for_root(root.path());

        let ignored = root.path().join("generated").join("build.md");
        let kept = root.path().join("src").join("main.rs");

        assert!(matcher.ignores_path(&ignored, false));
        assert!(!matcher.ignores_path(&kept, false));
    }
}
