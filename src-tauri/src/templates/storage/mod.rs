use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

use fs2::FileExt;
use git2::{Oid, Repository, Signature, Status};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::DiffHunk;

use super::types::{RoleTemplate, TeamPreset};

mod git;
mod presets;
mod roles;
mod state;

#[cfg(test)]
mod tests;

const TEMPLATES_DIRNAME: &str = "templates";
const ROLES_DIRNAME: &str = "roles";
const PRESETS_DIRNAME: &str = "presets";
const META_DIRNAME: &str = "_meta";
const GITIGNORE_FILENAME: &str = ".gitignore";
const LOCK_FILENAME: &str = ".lock";
const LOCK_FALLBACK_FILENAME: &str = ".lock.fallback";
const STATE_FILENAME: &str = "state.json";
const RECOVERY_COMMIT_MESSAGE: &str = "templates: recovery auto-commit";
const DEFAULT_DEBOUNCE_WINDOW_SECS: i64 = 30;
const FALLBACK_LOCK_RETRY_DELAY_MS: u64 = 20;
const FALLBACK_LOCK_RETRY_ATTEMPTS: usize = 250;
const TEMP_FILE_RANDOM_RETRY_ATTEMPTS: usize = 16;
const BUILTIN_CATALOG_REVISION: u32 = 1;

const GITIGNORE_CONTENTS: &str = "_meta/state.json\n*.tmp*\n.lock\n.lock.fallback\n";

// Exact SHA-256 fingerprints of template bytes shipped from 0.8.0 through
// 0.8.5. A file must match both an old path and known shipped bytes before the
// catalog migration may remove it; locally edited copies remain user-owned.
// Reconciliation removes redundant copies, so later bundle edits are read
// directly and do not require another fingerprint for already-migrated stores.
const PREVIOUS_BUNDLED_TEMPLATE_HASHES: &[(&str, &str)] = &[
    (
        "presets/dev-team.yaml",
        "0b21738499d30483be03427845cca63da1bd399caf4431d634790876749f28ed",
    ),
    (
        "presets/dev-team.yaml",
        "bf753d6513394eb6c61e33788b1b65f77e2d3e7fd05b4f4bbf559ec257980648",
    ),
    (
        "presets/full-team.yaml",
        "d39d3082c563769a249246b769dd2f46c612eaea0f7877c1d122aafba67c44a0",
    ),
    (
        "presets/full-team.yaml",
        "1352a9cb3b709188d1f8bfd52ae69de7c90cae164d5e32bf0b87dc92a9ea720f",
    ),
    (
        "presets/grok-pair.yaml",
        "869793213f7aeb8c204719ff1dc726c9b0211f1b0908b3d8c907996685c14c72",
    ),
    (
        "presets/grok-pair.yaml",
        "4044bfd247c210dd44506fac800af8d2de2b3c9b90394030b532297b3faa1803",
    ),
    (
        "presets/pair.yaml",
        "9f81fce260790bd02215c9b596fe32465e17b3872384cfb7df662fe0e8c9087f",
    ),
    (
        "presets/research-team.yaml",
        "16196e7a24218a3679fff2a6fcda41f6e4ca8bb195a34c02eb7c000c7b9813ee",
    ),
    (
        "presets/research-team.yaml",
        "06be090b1c326440526554415f9967c2adf0436c8fc578b0a364fb0157c82021",
    ),
    (
        "roles/adversarial-reviewer-claude.yaml",
        "88455a7e05876400d23f6bc10a6946bcfd14f297d8825aaaf16e058399a9e5e2",
    ),
    (
        "roles/antigravity-orchestrator.yaml",
        "4f8934db51767a484b397693cc39ebfa7b350dd382bc5e1c42b27aa3049b1c78",
    ),
    (
        "roles/antigravity-ui-specialist.yaml",
        "7c088eef6e08c3a2721fd096c575611b0c20b8fe63489129c5b80ee8032ca8c4",
    ),
    (
        "roles/claude-design-lead.yaml",
        "8a7fb6ada013ee8d287ab5fa1742be204d9776a424f4846a66511e93d0351b13",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "1a78b6d3caa1b49d39ed30cce5b628cef3cac1d1cced327e4396c67a8b686dd6",
    ),
    (
        "roles/claude-product-checker.yaml",
        "8e928d24bf3bcd22ea21f8aec1e6bfd5c4aaeebc0cb382cc4c1e6794b961513c",
    ),
    (
        "roles/claude-researcher.yaml",
        "c02160b84df2c472d30d3d90fc63c733468f4b1dcedc55a60751e007cf13536a",
    ),
    (
        "roles/claude-reviewer.yaml",
        "0e0e0fd25390d779003af92edaf102bca08f4b49019c0c3d70207698198361dd",
    ),
    (
        "roles/codex-architect.yaml",
        "b258f6df20bbbc58d3e80e096c62125486817771a904fd5f87f09e86e15e62e4",
    ),
    (
        "roles/codex-developer.yaml",
        "8e4ef423a83fb05345e0ed85de67f72071fecb6c46adf6dce17ecb6e9ace0865",
    ),
    (
        "roles/codex-orchestrator.yaml",
        "56fa5a6c81e8f60cbacea56e6b3d20e947188f6533ccb4e5aae0fbc17c97e213",
    ),
    (
        "roles/codex-product-lead.yaml",
        "a2aadfd219c4cd0f7ca127c90e1a6a458f7c52329300bd01efaa8ee0a691bc77",
    ),
    (
        "roles/codex-qa.yaml",
        "5e1bed1a9ab767147411a79da1bd4f4f66a5604a0626fc5e252ec428a6d7c8ce",
    ),
    (
        "roles/codex-vertical-slice-developer.yaml",
        "3fc7b2da5f99b8d1063ec654f6329f572e6792b206623b86ddf1370de16369b9",
    ),
    (
        "roles/docs-verifier-codex.yaml",
        "f99584b53393743b7b45915775a9c9bdd2d70e4f544265fe0882407badee31b5",
    ),
    (
        "roles/frontend-design-skill-developer.yaml",
        "4f00b61eb26e3427713be0256d298333771235e570673c72d6d91f1af02bf251",
    ),
    (
        "roles/grok-developer.yaml",
        "f320dd351e40ce9ad60df393e758c4c894b51c86eaa8c5f8ee6d47ceaae78dc2",
    ),
    (
        "roles/quick-dev-codex.yaml",
        "0bd3723a6371fa7ee26494f24538482df5a1c4dfa26e2bf7b69cedcd48fe9058",
    ),
    (
        "roles/taurhaus-architect.yaml",
        "da6f7d5925563b8c293e3101b458149d36339faebb2ebdbd00ed4946bc8a96be",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "83074a1b8331f963bf3e6dd1adda751f7a83f4f177087525840a703479887b89",
    ),
    (
        "roles/taurhaus-developer.yaml",
        "b6479a34469f6841564fd63ea4a7f8b2ee276689dc3765c6173366009e2f5cd6",
    ),
    (
        "roles/taurhaus-lead-claude.yaml",
        "8fb95527acd038562f821f60a2f24492af0685cf5adc6ff0da74c4af506e79be",
    ),
    (
        "roles/taurhaus-lead-codex.yaml",
        "15732abf22322f2b760fa59f4df2615a953644cf52b3eefd7e170ad36cca348b",
    ),
    (
        "roles/v2-architect-claude.yaml",
        "2249efc466d309cadece5506fcb741846f7b022da5167398085f67893bda34e0",
    ),
    (
        "roles/v2-architect-codex.yaml",
        "4deb2843884cfd9aace251ef2b18cb97cc48dcc803bf800e2d51426768e2dfa5",
    ),
    (
        "roles/v2-design-lead-claude.yaml",
        "e783c0e24f043b060c1cf5d54ccad6b3c23496e653f64fae2c8bcf009e1bf16c",
    ),
    (
        "roles/v2-developer-claude.yaml",
        "24378b7f1ffb3ce1c7a48d977c8216c8c99790c036759785cca2e30fd0f69e3c",
    ),
    (
        "roles/v2-developer-codex.yaml",
        "c33c164de1f92f769885f45dbb1adc9b3a8c1599088b85505930e1e403b26f61",
    ),
    (
        "roles/v2-lead-claude.yaml",
        "ea9519cef9ca21d82ca84ce709b5f2432878b955a48d73bb4f5a964acac2480c",
    ),
    (
        "roles/v2-lead-codex.yaml",
        "bf07a10022a86b39ef7530d87fb0351bec0eecc136d5272561b17b25dbd2e00b",
    ),
    (
        "roles/v2-product-checker-claude.yaml",
        "9af635391efdb3207237a77454ed539651eea0646af074470da232778264dda8",
    ),
    (
        "roles/v3-architect-claude.yaml",
        "e24ce908c7f24be6cc891cf6f9bb5f770a42d6a5b3f8ea3dee9cf711fa9b08c2",
    ),
    (
        "roles/v3-architect-codex.yaml",
        "bcbaf9a5f2ba18bf6bb134cbca804e9f23854719b28f65f0460823e7b2b8cdf5",
    ),
    (
        "roles/v3-design-lead-claude.yaml",
        "7e9de33cf464a4344f4d4eb5498a600df22f176d82e681cff2e48d8988c781bd",
    ),
    (
        "roles/v3-developer-agy.yaml",
        "734fe7efacdcb9597e32db4d1c14f83dbd04b16c8a4926cf59e5f6598f605c3c",
    ),
    (
        "roles/v3-developer-claude.yaml",
        "82d543bb5f754a4f752a91c898335a58cab10cdf39fc92975469d52a69575144",
    ),
    (
        "roles/v3-developer-codex.yaml",
        "f815b1b4fb1d231b4ef758307d53d57efe52a05ffc54a1cc9873e9598a67341e",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "9743b89d88ea1f706119cf2b5b2079348c56e3b14998e4635df9d31c63cd1a50",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "48ad6b77969c9e37deaa5fc466c4e644315d71a1f1c8d46536deb46193f5c014",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "353777544aff901fed99dc882fb6ddafce23f3e861a4fdbde49ce4092623c8e1",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "db8a1f434df71e4fcf145fbebd1aaf5722dbe5a01485829ecba14053fc0390ac",
    ),
    (
        "roles/v3-product-checker-claude.yaml",
        "2240e616bafa22fc213c764fe5ecfb4efac4947d690f8c01b78d2040dceffcae",
    ),
    (
        "roles/v4-developer-agy.yaml",
        "1d006f6ddd06e895fe069814889ce8eb79778b2bdd85aa380923e071f6508dc3",
    ),
    (
        "roles/v4-developer-claude.yaml",
        "9a9db98fa92396f70011b3ef83f23e2888b17bf1942ec20f76c62dd6e8e91c38",
    ),
    (
        "roles/v4-developer-codex.yaml",
        "4dd3867ed317a686fc3231d7c9d3aabffb59944707cd2aa852b5595d0ba81fb2",
    ),
    (
        "roles/v4-developer-grok.yaml",
        "98f59514df7ba5cb046a654a29463bb48b30e174f6aaaf7b23dfe368f209341e",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCatalog {
    pub roles: Vec<RoleTemplate>,
    pub presets: Vec<TeamPreset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    BuiltIn,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleTemplateRecord {
    pub template: RoleTemplate,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPresetRecord {
    pub template: TeamPreset,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommit {
    pub commit_id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommitPage {
    pub commits: Vec<TemplateCommit>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffFile {
    pub path: String,
    pub status: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiff {
    pub commit_id: String,
    pub files: Vec<TemplateDiffFile>,
    pub stats: TemplateDiffStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateMutationResult {
    pub commit_id: Option<String>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFileMutation {
    pub relative_path: PathBuf,
    pub contents: Option<Vec<u8>>,
}

impl TemplateFileMutation {
    pub fn write(relative_path: PathBuf, contents: Vec<u8>) -> Self {
        Self {
            relative_path,
            contents: Some(contents),
        }
    }

    pub fn delete(relative_path: PathBuf) -> Self {
        Self {
            relative_path,
            contents: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TemplateStoreState {
    #[serde(default)]
    pub pending_actions: Vec<PendingAction>,
    pub last_commit_at: Option<i64>,
    #[serde(default)]
    pub repo_initialized: bool,
    #[serde(default)]
    pub builtin_catalog_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct DebounceState {
    pending_actions: Vec<PendingAction>,
    window_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAction {
    pub action: String,
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub changed_paths: Vec<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid template path: {0}")]
    InvalidTemplatePath(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Read-only template: {0}")]
    ReadOnly(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Lock timeout: {0}")]
    LockTimeout(String),
}

#[derive(Debug, Clone)]
struct PathChange {
    path: PathBuf,
    deleted: bool,
}

#[derive(Debug, Clone)]
struct MutationDescriptor {
    action: String,
    kind: String,
    id: String,
}

#[derive(Debug)]
struct FallbackLockGuard {
    path: PathBuf,
    _file: File,
}

impl Drop for FallbackLockGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    lock_path = %self.path.display(),
                    error = %err,
                    "failed to remove template fallback lockfile"
                );
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TemplateStoreLockGuard {
    _advisory_file: Option<File>,
    _fallback_guard: Option<FallbackLockGuard>,
}

impl TemplateStoreLockGuard {
    fn advisory(file: File) -> Self {
        Self {
            _advisory_file: Some(file),
            _fallback_guard: None,
        }
    }

    fn fallback(guard: FallbackLockGuard) -> Self {
        Self {
            _advisory_file: None,
            _fallback_guard: Some(guard),
        }
    }
}

#[derive(Debug, Clone)]
struct RoleTemplateFile {
    template: RoleTemplate,
}

#[derive(Debug, Clone)]
struct TeamPresetFile {
    template: TeamPreset,
}

impl DebounceState {
    fn from_store(state: TemplateStoreState, window_secs: i64) -> Self {
        Self {
            pending_actions: state.pending_actions,
            window_secs,
        }
    }

    fn is_empty(&self) -> bool {
        self.pending_actions.is_empty()
    }

    fn oldest_first_seen(&self) -> Option<i64> {
        self.pending_actions.iter().map(|a| a.first_seen_at).min()
    }

    fn should_flush_lazy(&self, now_ts: i64) -> bool {
        self.oldest_first_seen()
            .map(|oldest| now_ts.saturating_sub(oldest) >= self.window_secs)
            .unwrap_or(false)
    }

    fn enqueue(&mut self, descriptor: MutationDescriptor, changed_paths: &[PathBuf], now_ts: i64) {
        if let Some(existing) = self
            .pending_actions
            .iter_mut()
            .find(|action| action.kind == descriptor.kind && action.id == descriptor.id)
        {
            existing.action = descriptor.action;
            existing.last_seen_at = now_ts;
            for path in changed_paths {
                let path_str = path.to_string_lossy().to_string();
                if !existing
                    .changed_paths
                    .iter()
                    .any(|existing| existing == &path_str)
                {
                    existing.changed_paths.push(path_str);
                }
            }
            return;
        }

        self.pending_actions.push(PendingAction {
            action: descriptor.action,
            kind: descriptor.kind,
            id: descriptor.id,
            changed_paths: changed_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            first_seen_at: now_ts,
            last_seen_at: now_ts,
        });
    }

    fn commit_message(&self) -> String {
        if self.pending_actions.len() == 1 {
            let action = &self.pending_actions[0];
            return format!("templates: {} {} {}", action.action, action.kind, action.id);
        }
        format!("templates: batch {} changes", self.pending_actions.len())
    }

    fn shutdown_message(&self) -> String {
        format!(
            "templates: shutdown flush {} changes",
            self.pending_actions.len()
        )
    }

    fn take_changed_paths(&self) -> Vec<PathBuf> {
        let mut unique = BTreeMap::<PathBuf, bool>::new();
        for action in &self.pending_actions {
            for raw in &action.changed_paths {
                let path = PathBuf::from(raw);
                unique.entry(path).or_insert(true);
            }
        }
        unique.into_keys().collect()
    }
}

#[derive(Debug, Clone)]
pub struct TemplateStore {
    templates_dir: PathBuf,
    builtins_dir: PathBuf,
    debounce_window_secs: i64,
}

impl TemplateStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir: default_builtins_dir(),
            debounce_window_secs: DEFAULT_DEBOUNCE_WINDOW_SECS,
        }
    }

    pub fn with_builtins_dir(app_data_dir: PathBuf, builtins_dir: PathBuf) -> Self {
        Self::with_builtins_and_debounce(app_data_dir, builtins_dir, DEFAULT_DEBOUNCE_WINDOW_SECS)
    }

    pub fn with_builtins_and_debounce(
        app_data_dir: PathBuf,
        builtins_dir: PathBuf,
        debounce_window_secs: i64,
    ) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir,
            debounce_window_secs,
        }
    }

    pub fn templates_dir(&self) -> &Path {
        &self.templates_dir
    }

    pub fn ensure_directories(&self) -> Result<(), TemplateStoreError> {
        fs::create_dir_all(self.roles_dir())?;
        fs::create_dir_all(self.presets_dir())?;
        fs::create_dir_all(self.meta_dir())?;
        Ok(())
    }

    pub fn load_catalog(&self) -> Result<TemplateCatalog, TemplateStoreError> {
        self.reconcile_builtins_if_needed()?;
        self.load_catalog_without_reconcile()
    }

    fn load_catalog_without_reconcile(&self) -> Result<TemplateCatalog, TemplateStoreError> {
        let roles = self.load_role_catalog()?;
        let presets = self.load_preset_catalog(&roles)?;

        Ok(TemplateCatalog { roles, presets })
    }

    fn reconcile_builtins_if_needed(&self) -> Result<(), TemplateStoreError> {
        self.ensure_directories()?;
        if self.load_state_unlocked()?.builtin_catalog_revision >= BUILTIN_CATALOG_REVISION {
            return Ok(());
        }

        let lock = self.acquire_lock()?;
        if self.load_state_unlocked()?.builtin_catalog_revision >= BUILTIN_CATALOG_REVISION {
            return Ok(());
        }

        let mutations = self.builtin_reconciliation_mutations()?;
        let changed_paths = mutations
            .iter()
            .map(|mutation| mutation.relative_path.clone())
            .collect::<Vec<_>>();

        for mutation in &mutations {
            let target = self.templates_dir.join(&mutation.relative_path);
            match mutation.contents.as_ref() {
                Some(contents) => write_atomic_file(&target, contents)?,
                None => match fs::remove_file(&target) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(TemplateStoreError::Io(err)),
                },
            }
        }

        if !changed_paths.is_empty() && self.git_dir().is_dir() {
            let repo = Repository::open(self.templates_dir())?;
            let changes = changed_paths
                .iter()
                .map(|path| PathChange {
                    path: path.clone(),
                    deleted: !self.templates_dir.join(path).exists(),
                })
                .collect::<Vec<_>>();
            let _ = self.commit_with_repo(
                &repo,
                &changes,
                &format!("templates: reconcile built-in catalog v{BUILTIN_CATALOG_REVISION}"),
            )?;
        }

        let mut state = self.load_state_unlocked()?;
        state.builtin_catalog_revision = BUILTIN_CATALOG_REVISION;
        self.save_state_unlocked(&state)?;
        drop(lock);
        Ok(())
    }

    fn builtin_reconciliation_mutations(
        &self,
    ) -> Result<Vec<TemplateFileMutation>, TemplateStoreError> {
        let mut mutations = Vec::new();
        let mut removed_preset_paths = BTreeSet::new();
        let mut preset_entries = fs::read_dir(self.presets_dir())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        preset_entries.sort();

        for target in &preset_entries {
            if !target.is_file() || !is_yaml_file(target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(PRESETS_DIRNAME).join(file_name);
            let existing = fs::read(target)?;
            if was_previously_shipped_builtin(&relative, &existing) {
                removed_preset_paths.insert(relative.clone());
                mutations.push(TemplateFileMutation::delete(relative));
            }
        }

        let mut referenced_role_ids = BTreeSet::new();
        for preset in self.load_presets_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            insert_preset_role_ids(&preset, &mut referenced_role_ids);
        }
        for target in preset_entries {
            if !target.is_file() || !is_yaml_file(&target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(PRESETS_DIRNAME).join(file_name);
            if removed_preset_paths.contains(&relative) {
                continue;
            }
            let raw = fs::read_to_string(&target)?;
            let preset = serde_norway::from_str::<TeamPreset>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!(
                    "failed to parse preset {}: {err}",
                    target.display()
                ))
            })?;
            insert_preset_role_ids(&preset, &mut referenced_role_ids);
        }

        let mut role_entries = fs::read_dir(self.roles_dir())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        role_entries.sort();
        for target in role_entries {
            if !target.is_file() || !is_yaml_file(&target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(ROLES_DIRNAME).join(file_name);
            let existing = fs::read(&target)?;
            if !was_previously_shipped_builtin(&relative, &existing) {
                continue;
            }

            let bundled = self.builtins_dir.join(&relative);
            if !bundled.is_file() {
                let raw = String::from_utf8(existing).map_err(|err| {
                    TemplateStoreError::Parse(format!(
                        "failed to read role {} as UTF-8: {err}",
                        target.display()
                    ))
                })?;
                let role = serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
                    TemplateStoreError::Parse(format!(
                        "failed to parse role {}: {err}",
                        target.display()
                    ))
                })?;
                if referenced_role_ids.contains(&role.role_id) {
                    continue;
                }
            }
            mutations.push(TemplateFileMutation::delete(relative));
        }
        Ok(mutations)
    }

    fn load_role_catalog(&self) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut roles_by_id: BTreeMap<String, RoleTemplate> = BTreeMap::new();
        for role in self.load_role_templates_from_dir(&self.builtins_dir.join(ROLES_DIRNAME))? {
            roles_by_id.insert(role.role_id.clone(), role);
        }
        for role in self.load_role_templates_from_dir(&self.roles_dir())? {
            roles_by_id.insert(role.role_id.clone(), role);
        }

        let roles = roles_by_id.into_values().collect::<Vec<_>>();
        for role in &roles {
            role.validate()
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;
        }
        Ok(roles)
    }

    fn load_preset_catalog(
        &self,
        role_catalog: &[RoleTemplate],
    ) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut presets_by_id: BTreeMap<String, TeamPreset> = BTreeMap::new();
        for preset in self.load_presets_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            if let Err(err) = preset.validate_with_role_catalog(role_catalog) {
                tracing::warn!(
                    preset_id = %preset.preset_id,
                    source = "built_in",
                    error = %err,
                    "skipping invalid team preset"
                );
                continue;
            }
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }
        for preset in self.load_presets_from_dir(&self.presets_dir())? {
            if let Err(err) = preset.validate_with_role_catalog(role_catalog) {
                tracing::warn!(
                    preset_id = %preset.preset_id,
                    source = "user",
                    error = %err,
                    "skipping invalid team preset"
                );
                continue;
            }
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }
        Ok(presets_by_id.into_values().collect())
    }

    fn roles_dir(&self) -> PathBuf {
        self.templates_dir.join(ROLES_DIRNAME)
    }

    fn presets_dir(&self) -> PathBuf {
        self.templates_dir.join(PRESETS_DIRNAME)
    }

    fn meta_dir(&self) -> PathBuf {
        self.templates_dir.join(META_DIRNAME)
    }

    fn git_dir(&self) -> PathBuf {
        self.templates_dir.join(".git")
    }

    fn state_path(&self) -> PathBuf {
        self.meta_dir().join(STATE_FILENAME)
    }

    fn gitignore_path(&self) -> PathBuf {
        self.templates_dir.join(GITIGNORE_FILENAME)
    }

    fn ensure_gitignore(&self) -> Result<(), TemplateStoreError> {
        let gitignore = self.gitignore_path();
        if gitignore.exists() {
            return Ok(());
        }
        write_atomic_file(&gitignore, GITIGNORE_CONTENTS.as_bytes())
    }

    fn seed_builtins_if_missing(&self) -> Result<(), TemplateStoreError> {
        self.copy_missing_from_dir(&self.builtins_dir.join(ROLES_DIRNAME), &self.roles_dir())?;
        self.copy_missing_from_dir(
            &self.builtins_dir.join(PRESETS_DIRNAME),
            &self.presets_dir(),
        )?;
        Ok(())
    }

    fn copy_missing_from_dir(
        &self,
        source_dir: &Path,
        target_dir: &Path,
    ) -> Result<(), TemplateStoreError> {
        if !source_dir.exists() {
            return Ok(());
        }

        fs::create_dir_all(target_dir)?;

        let mut entries = fs::read_dir(source_dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        entries.sort();

        for source_path in entries {
            if !source_path.is_file() {
                continue;
            }
            if !is_yaml_file(&source_path) {
                continue;
            }

            let Some(file_name) = source_path.file_name() else {
                continue;
            };
            let target_path = target_dir.join(file_name);
            if target_path.exists() {
                continue;
            }

            let bytes = fs::read(&source_path)?;
            write_atomic_file(&target_path, &bytes)?;
        }

        Ok(())
    }

    fn role_file_path(&self, role_id: &str) -> PathBuf {
        PathBuf::from(ROLES_DIRNAME).join(format!("{role_id}.yaml"))
    }

    fn preset_file_path(&self, preset_id: &str) -> PathBuf {
        PathBuf::from(PRESETS_DIRNAME).join(format!("{preset_id}.yaml"))
    }

    fn apply_single_template_mutation(
        &self,
        mutation: TemplateFileMutation,
        commit_message: &str,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let commit_id = self.mutate_and_commit(&[mutation], commit_message)?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    fn load_role_templates_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        Ok(self
            .load_role_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_role_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplateFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut roles = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let mut parsed = serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
            })?;
            parsed.normalize_model_fields();
            roles.push(RoleTemplateFile { template: parsed });
        }

        Ok(roles)
    }

    fn load_role_file_by_id(
        &self,
        dir: &Path,
        role_id: &str,
    ) -> Result<Option<RoleTemplateFile>, TemplateStoreError> {
        let path = dir.join(format!("{role_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let mut parsed = serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
        })?;
        parsed.normalize_model_fields();
        Ok(Some(RoleTemplateFile { template: parsed }))
    }

    fn load_presets_from_dir(&self, dir: &Path) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        Ok(self
            .load_preset_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_preset_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<TeamPresetFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut presets = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let mut parsed = serde_norway::from_str::<TeamPreset>(&raw).map_err(|err| {
                TemplateStoreError::Parse(format!(
                    "failed to parse preset {}: {err}",
                    path.display()
                ))
            })?;
            parsed.normalize_model_fields();
            presets.push(TeamPresetFile { template: parsed });
        }

        Ok(presets)
    }

    fn load_preset_file_by_id(
        &self,
        dir: &Path,
        preset_id: &str,
    ) -> Result<Option<TeamPresetFile>, TemplateStoreError> {
        let path = dir.join(format!("{preset_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let mut parsed = serde_norway::from_str::<TeamPreset>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse preset {}: {err}", path.display()))
        })?;
        parsed.normalize_model_fields();
        Ok(Some(TeamPresetFile { template: parsed }))
    }
}
fn resolve_signature(repo: &Repository) -> Result<Signature<'_>, TemplateStoreError> {
    match repo.signature() {
        Ok(sig) => Ok(sig),
        Err(_) => Signature::now("taurhaus", "templates@local").map_err(TemplateStoreError::Git),
    }
}

fn default_builtins_dir() -> PathBuf {
    resolve_packaged_builtins_dir().unwrap_or_else(dev_builtins_dir)
}

fn resolve_packaged_builtins_dir() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    packaged_builtins_dir_candidates(&current_exe)
        .into_iter()
        .find(|path| path.is_dir())
}

fn packaged_builtins_dir_candidates(current_exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(exe_dir) = current_exe.parent() {
        candidates.push(exe_dir.join("resources").join("templates"));

        if let Some(contents_dir) = exe_dir.parent() {
            candidates.push(
                contents_dir
                    .join("Resources")
                    .join("resources")
                    .join("templates"),
            );
        }
    }

    candidates
}

fn dev_builtins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates")
}

fn current_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_mutation_descriptor(message: &str) -> Option<MutationDescriptor> {
    let payload = message.strip_prefix("templates: ")?;
    let mut parts = payload.split_whitespace();
    let action = parts.next()?.to_string();
    let kind = parts.next()?.to_string();
    let id = parts.collect::<Vec<_>>().join(" ");
    if id.trim().is_empty() {
        return None;
    }
    Some(MutationDescriptor { action, kind, id })
}

fn validate_template_id(id: &str, kind: &str) -> Result<(), TemplateStoreError> {
    if id.trim().is_empty() {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id must not be empty"
        )));
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id '{id}' must use only [a-zA-Z0-9_-]"
        )));
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, TemplateStoreError> {
    if path.is_absolute() {
        return Err(TemplateStoreError::InvalidTemplatePath(format!(
            "absolute paths are not allowed: {}",
            path.display()
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            _ => {
                return Err(TemplateStoreError::InvalidTemplatePath(format!(
                    "path contains invalid component: {}",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(TemplateStoreError::InvalidTemplatePath(
            "empty relative path".to_string(),
        ));
    }

    Ok(normalized)
}

fn is_managed_template_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Normal(first))
            if first == OsStr::new(ROLES_DIRNAME)
                || first == OsStr::new(PRESETS_DIRNAME)
                || first == OsStr::new(META_DIRNAME)
    )
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("yaml") | Some("yml")
    )
}

fn was_previously_shipped_builtin(relative_path: &Path, bytes: &[u8]) -> bool {
    let digest = format!("{:x}", Sha256::digest(bytes));
    PREVIOUS_BUNDLED_TEMPLATE_HASHES
        .iter()
        .any(|(path, hash)| Path::new(path) == relative_path && *hash == digest)
}

fn insert_preset_role_ids(preset: &TeamPreset, role_ids: &mut BTreeSet<String>) {
    role_ids.insert(preset.lead_role_id.clone());
    role_ids.extend(preset.agent_slots.iter().map(|slot| slot.role_id.clone()));
}

fn temp_path_for(path: &Path) -> PathBuf {
    let random_suffix = format!("{:016x}", rand::thread_rng().next_u64());
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) => path.with_extension(format!("{ext}.tmp.{random_suffix}")),
        None => path.with_extension(format!("tmp.{random_suffix}")),
    }
}

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

#[cfg(test)]
fn should_force_fallback_lock_for_tests() -> bool {
    std::env::var_os("TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK")
        .map(|value| value == "1")
        .unwrap_or(false)
}

#[cfg(not(test))]
fn should_force_fallback_lock_for_tests() -> bool {
    false
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn acquire_fallback_lock(
    fallback_lock_path: &Path,
) -> Result<FallbackLockGuard, TemplateStoreError> {
    let mut last_conflict = None;
    for _ in 0..FALLBACK_LOCK_RETRY_ATTEMPTS {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(fallback_lock_path)
        {
            Ok(mut file) => {
                let pid = std::process::id();
                let _ = writeln!(file, "{pid}");
                file.sync_all()?;
                return Ok(FallbackLockGuard {
                    path: fallback_lock_path.to_path_buf(),
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_conflict = Some(err);
                thread::sleep(Duration::from_millis(FALLBACK_LOCK_RETRY_DELAY_MS));
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let cause = last_conflict
        .map(|err| err.to_string())
        .unwrap_or_else(|| "unknown fallback lock contention".to_string());
    Err(TemplateStoreError::LockTimeout(format!(
        "timed out acquiring fallback lock {}: {}",
        fallback_lock_path.display(),
        cause
    )))
}

/// Write `bytes` to `target` through a unique temp file and a rename, so a
/// reader never observes a half-written file. Where the rename itself cannot
/// replace an existing file, `replace_without_atomic_rename` keeps that
/// promise the long way round.
pub(crate) fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), TemplateStoreError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut tmp_open_error = None;
    let mut selected_tmp = None;
    let mut selected_file = None;
    for _ in 0..TEMP_FILE_RANDOM_RETRY_ATTEMPTS {
        let candidate = temp_path_for(target);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => {
                selected_tmp = Some(candidate);
                selected_file = Some(file);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                tmp_open_error = Some(err);
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let tmp = selected_tmp.ok_or_else(|| {
        let err = tmp_open_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "failed to allocate unique temp path after {} attempts for {}",
                    TEMP_FILE_RANDOM_RETRY_ATTEMPTS,
                    target.display()
                ),
            )
        });
        TemplateStoreError::Io(err)
    })?;

    let mut file = selected_file.ok_or_else(|| {
        TemplateStoreError::Io(std::io::Error::other(format!(
            "internal temp-file selection mismatch while writing {}",
            target.display()
        )))
    })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    if let Err(err) = fs::rename(&tmp, target) {
        if is_windows_unsupported_rename_error(&err) {
            tracing::warn!(
                target = %target.display(),
                "atomic rename cannot replace this path; moving the old file aside instead"
            );
            let replaced = replace_without_atomic_rename(&tmp, target);
            let _ = fs::remove_file(&tmp);
            return replaced.map_err(TemplateStoreError::Io);
        }

        let _ = fs::remove_file(&tmp);
        return Err(TemplateStoreError::Io(err));
    }

    Ok(())
}

/// Put `tmp` at `target` on a filesystem whose rename refuses to replace a file
/// that is already there — Windows answers `ERROR_INVALID_FUNCTION` on some
/// WSL-backed and network paths.
///
/// Rewriting `target` in place is not the answer: a reader would see a
/// half-written file and an interruption would leave one on disk, which is the
/// very thing an atomic write exists to prevent. So the old file is moved aside
/// first and the new one renamed into the name it vacated — every state a
/// reader can observe is a whole file, the old one or the new one — and a
/// failure puts the old file back and reports itself rather than claiming a
/// write that did not happen.
fn replace_without_atomic_rename(tmp: &Path, target: &Path) -> std::io::Result<()> {
    let displaced = temp_path_for(target);
    let had_target = match fs::rename(target, &displaced) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };

    match fs::rename(tmp, target) {
        Ok(()) => {
            if had_target {
                let _ = fs::remove_file(&displaced);
            }
            Ok(())
        }
        Err(err) => {
            if had_target {
                if let Err(restore) = fs::rename(&displaced, target) {
                    return Err(std::io::Error::new(
                        err.kind(),
                        format!(
                            "{err}; and restoring the previous file failed ({restore}) — it is at {}",
                            displaced.display()
                        ),
                    ));
                }
            }
            Err(err)
        }
    }
}

fn is_deleted_status(status: Status) -> bool {
    status.intersects(Status::INDEX_DELETED | Status::WT_DELETED | Status::CONFLICTED)
}
