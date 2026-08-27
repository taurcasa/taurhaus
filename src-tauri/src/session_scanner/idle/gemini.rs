use super::*;
use std::path::PathBuf;

#[cfg(test)]
static BASE_DIR_FOR_TEST: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

#[cfg(test)]
struct BaseDirOverride;

#[cfg(test)]
impl Drop for BaseDirOverride {
    fn drop(&mut self) {
        *BASE_DIR_FOR_TEST.lock().expect("Gemini test root lock") = None;
    }
}

#[cfg(test)]
fn set_base_dir_for_test(base_dir: PathBuf) -> BaseDirOverride {
    *BASE_DIR_FOR_TEST.lock().expect("Gemini test root lock") = Some(base_dir);
    BaseDirOverride
}

/// Resolves Gemini CLI session files from `~/.gemini/tmp/<sha256>/chats/`.
///
/// Gemini CLI uses SHA-256 of the project path as the directory name.
/// Chat sessions are stored as JSON files in the `chats/` subdirectory.
pub struct GeminiResolver {
    /// `~/.gemini/tmp/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
}

impl GeminiResolver {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".gemini").join("tmp"));
        Self { base_dir }
    }

    fn resolved_base_dir(&self) -> Option<PathBuf> {
        #[cfg(test)]
        {
            if let Some(base_dir) = BASE_DIR_FOR_TEST
                .lock()
                .expect("Gemini test root lock")
                .clone()
            {
                return Some(base_dir);
            }
        }

        self.base_dir.clone()
    }
}

impl Default for GeminiResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionResolver for GeminiResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let base = match self.resolved_base_dir() {
            Some(dir) => dir,
            None => return IdleResult::idle(),
        };
        gemini_detect_idle(project_path, &base)
    }
}

impl SessionSource for GeminiResolver {
    fn resolve(&self, project_path: &str, _pid: u32, _pane_id: Option<&str>) -> IdleResult {
        self.detect_idle(project_path)
    }
}

/// Core Gemini idle detection — testable with custom base dir.
pub(super) fn gemini_detect_idle(project_path: &str, base_dir: &Path) -> IdleResult {
    // Gemini CLI has used two naming schemes for session directories:
    //   - Newer (0.29+): project directory name (e.g. "my-project")
    //   - Older: SHA-256 hash of the full project path
    // Try the directory name first, then fall back to the hash.
    let dir_name = Path::new(project_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let chats_dir_by_name = base_dir.join(dir_name).join("chats");
    let hash = project_path_sha256(project_path);
    let chats_dir_by_hash = base_dir.join(&hash).join("chats");

    let chats_dir = if chats_dir_by_name.is_dir() {
        &chats_dir_by_name
    } else {
        &chats_dir_by_hash
    };

    // Find the most recently modified .json file in the chats directory
    let session_file = match find_latest_file(chats_dir, "json") {
        Some(entry) => entry,
        None => return IdleResult::idle(),
    };

    // Extract session ID from filename: "session-2026-02-10T19-57-4574fc66.json" -> "4574fc66"
    // The last segment after the final dash is the UUID prefix.
    let session_id = session_file
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.rsplit('-').next())
        .map(|s| s.to_string());

    let file_path = session_file.to_string_lossy().to_string();

    let output_mtime = file_mtime(&session_file);
    let state = output_mtime
        .map(|t| classify_mtime(t, ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(file_path),
        last_output_age_secs: output_mtime.map(age_secs_since_mtime),
        authoritative: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    #[test]
    fn gemini_detect_idle_active_session() {
        let tmp = TempDir::new().unwrap();

        // Create hash dir for project path
        let project = "/home/user/projects/myapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        // Create a recent session file
        let session = chats_dir.join("session-2026-02-21T10-30-abc12345.json");
        let mut f = File::create(&session).unwrap();
        writeln!(f, r#"{{"sessionId":"abc12345"}}"#).unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("abc12345"));
        assert!(result.jsonl_path.is_some());
    }

    #[test]
    fn gemini_runtime_source_preserves_transcript_identity() {
        // Regression: f90b362 replaced Gemini's project-scoped resolver with
        // NoSessionSource, dropping its session id, transcript path, and mtime
        // activity from every runtime scan.
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/runtime-gemini";
        let chats_dir = tmp.path().join("runtime-gemini").join("chats");
        fs::create_dir_all(&chats_dir).unwrap();
        let session = chats_dir.join("session-2026-08-27T10-00-feedface.json");
        File::create(&session).unwrap();

        let _base_dir = set_base_dir_for_test(tmp.path().to_path_buf());
        let result = detect_runtime_idle(project, 4242, Some("%42"), CliTool::Gemini);

        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("feedface"));
        assert_eq!(result.jsonl_path.as_deref(), session.to_str());
        assert!(result.last_output_age_secs.is_some());
        assert!(!result.authoritative);
    }

    #[test]
    fn gemini_detect_idle_old_session() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/oldapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let session = chats_dir.join("session-2026-01-01T00-00-deadbeef.json");
        File::create(&session).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&session, old_time);

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(result.session_id.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn gemini_detect_idle_no_hash_dir() {
        let tmp = TempDir::new().unwrap();
        let result = gemini_detect_idle("/nonexistent/project", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn gemini_detect_idle_empty_chats_dir() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/emptyapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn gemini_picks_most_recent_session() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/multiapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        // Old session
        let old = chats_dir.join("session-2026-01-01T00-00-old11111.json");
        File::create(&old).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old, old_time);

        // Recent session
        let new = chats_dir.join("session-2026-02-21T12-00-new22222.json");
        let mut f = File::create(&new).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("new22222"));
    }

    #[test]
    fn gemini_detect_idle_by_dir_name() {
        // Newer Gemini CLI (0.29+) uses project directory name, not SHA-256 hash
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/tapcount-gemini";
        let chats_dir = tmp.path().join("tapcount-gemini").join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let session = chats_dir.join("session-2026-02-23T22-17-80291013.json");
        let mut f = File::create(&session).unwrap();
        writeln!(f, r#"{{"sessionId":"80291013"}}"#).unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("80291013"));
    }

    #[test]
    fn gemini_prefers_dir_name_over_hash() {
        // When both exist, the directory-name version should win
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/myapp";

        // Create hash-based dir with old session
        let hash = project_path_sha256(project);
        let hash_chats = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&hash_chats).unwrap();
        let old = hash_chats.join("session-2026-01-01T00-00-oldhash1.json");
        File::create(&old).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old, old_time);

        // Create name-based dir with fresh session
        let name_chats = tmp.path().join("myapp").join("chats");
        fs::create_dir_all(&name_chats).unwrap();
        let new = name_chats.join("session-2026-02-23T12-00-newname1.json");
        let mut f = File::create(&new).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("newname1"));
    }
}
