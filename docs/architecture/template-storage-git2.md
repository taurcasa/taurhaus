# Git-Backed Template Storage (git2)

## Scope
Design a local template storage subsystem for Team Templates using filesystem YAML + optional git history via `git2`.

### Goals
- Templates always work as plain files.
- Git history is automatic when available.
- No app-start hard dependency on git repo readiness.
- One logical user action maps to one logical commit (with debounce batching).

### Non-goals
- No remote/push workflow.
- No branch management UI in v1.
- No merge conflict UI for external concurrent editors in v1.

## Directory and File Layout

### Base directory
- `app_data_dir()/templates/`

Rationale:
- Runtime-mutable user data belongs in app data, not repo or bundled resources.
- Works across platforms with Tauri path resolver.

### Layout
- `templates/roles/*.yaml`
- `templates/presets/*.yaml`
- `templates/_meta/index.yaml` (optional cached lookup)
- `templates/_meta/state.json` (debounce/recovery metadata)
- `templates/.git/` (lazy-initialized)
- `templates/.gitignore`

### Built-in template tracking decision
- Built-ins remain bundled in `src-tauri/resources/templates/` and are treated as read-only defaults.
- Built-in reads converge in `TemplateStore`: catalog/list operations scan the bundled role and preset directories, direct lookups probe `<id>.yaml`, reconciliation reads bundled presets, and lazy git initialization copies missing bundled YAML into app data. Before the packaged manifest was introduced, each of those paths trusted every YAML present in the installed resource directories, so an upgrade-leftover file could be listed and seeded again.
- `resources/templates/manifest.txt` is embedded in the binary as the authoritative closed set for all of those built-in paths. A conformance test keeps that set equal to the checked-in role and preset YAML; extra installed YAML is ignored.
- The git-backed app-data repository tracks only user-authored templates and user overrides.
- Editing a built-in creates/updates a user override file in `app_data_dir()/templates/...` and commits that override.
- Rationale: app upgrades can change bundled defaults; committing bundled files into user history would create noisy, version-coupled churn.

### `.gitignore`
- Ignore only internal volatile files:
  - `_meta/state.json`
  - `*.tmp`
  - `.lock`
- Do **not** ignore role/preset YAML or index metadata.

## Data Model

### Primary data (plain YAML)
- Runtime source of truth is a merged catalog:
  - built-ins from `src-tauri/resources/templates/{roles,presets}`, gated by the
    embedded closed manifest (`resources/templates/manifest.txt`): a packaged file
    absent from the manifest is invisible to listing and seeding, so stale files a
    Windows upgrade leaves behind cannot resurrect. Any add/remove under those
    directories must mirror into the manifest (a conformance test enforces it).
  - user files from `app_data_dir()/templates/{roles,presets}`
- Resolution rule: user template with matching ID overrides bundled template.
- Storage layer git history only includes files under `app_data_dir()/templates`.

### Git metadata (best-effort)
- Commit history and diffability for audit/revert.
- If git unavailable/failing, app still reads/writes YAML normally.

### Runtime state
- `_meta/state.json` stores debounce queue + pending logical actions.
- Example keys:
  - `pending_actions[]` (`action`, `type`, `id`, `changed_paths`, `first_seen_at`, `last_seen_at`)
  - `last_commit_at`
  - `repo_initialized`

## Core Operations

## 1) Lazy git init
Trigger: first successful template mutation (create/update/delete).

Flow:
1. Ensure `templates/` exists.
2. If `.git` missing, call `Repository::init(templates_dir)`.
3. Write `.gitignore` if missing.
4. Continue mutation even if init fails; record warning telemetry.

Failure behavior:
- init failure sets storage mode to `PlainFilesystem` for current operation.
- no user-facing hard failure unless file mutation itself fails.

## 2) Write-through filesystem (always)
Every CRUD operation writes YAML atomically first:
- write to `*.tmp`
- fsync/write
- rename to target

Then schedule commit candidate (if git available).

## 3) Debounced auto-commit (30-60s)
Recommended default: 30s inactivity window.

Policy:
- CRUD operations enqueue logical action in pending queue.
- Any additional edits reset debounce timer and coalesce touched files.
- Commit message uses first action context + summary count when coalesced.
- Crash during debounce does not lose template data (write-through already persisted); at worst it loses intended batching and falls back to recovery commit attribution.

Message format:
- Single action: `templates: <action> <type> <id>`
- Coalesced batch: `templates: batch <n> changes`

Examples:
- `templates: create role codex-developer`
- `templates: update preset full-stack-core`
- `templates: delete role qa-reviewer`

## 4) Crash recovery commit
Trigger: app startup or first template interaction after startup.

Flow:
1. Open/init storage in read mode.
2. Load `_meta/state.json`; if pending actions exist, retain them for recovery attribution.
3. If repo exists, detect dirty tree (`repo.statuses(...)`).
4. If dirty and changed paths are within managed template scope, auto-commit:
   - message: `templates: recovery auto-commit`
5. Clear pending debounce metadata.

Safety:
- Run schema validation before recovery commit.
- If validation fails, keep files but skip commit and surface diagnostics.

## 5) External edit detection
Trigger: next template interaction (list/get/update) or file watcher event.

Flow:
1. Compare mtimes/hash/index against known state.
2. Detect changed/new/deleted YAML files not initiated by app mutation context.
3. Validate changed files.
4. Auto-commit with message:
   - `templates: external sync <count> changes`

## git2 API Design

### API sketch (Rust module shape)
```rust
pub struct TemplateGitStore {
    templates_dir: PathBuf,
    debounce: DebounceState,
}

impl TemplateGitStore {
    pub fn ensure_repo_for_mutation(&self) -> Result<Option<Repository>, TemplateStoreError>;
    pub fn stage_and_commit(
        &self,
        repo: &Repository,
        paths: &[PathBuf],
        message: &str,
    ) -> Result<Option<Oid>, TemplateStoreError>;
    pub fn recover_dirty_tree(&self) -> Result<Option<Oid>, TemplateStoreError>;
    pub fn get_history(&self, limit: usize, cursor: Option<String>) -> Result<TemplateCommitPage, TemplateStoreError>;
    pub fn get_diff(&self, commit_id: &str) -> Result<TemplateDiff, TemplateStoreError>;
    pub fn revert_files(&self, request: TemplateRevertRequest) -> Result<Option<Oid>, TemplateStoreError>;
}
```

## Repository open/init
- `Repository::open(templates_dir)` if exists.
- fallback `Repository::init(templates_dir)` for lazy init.

## Commit creation sequence
1. `let mut index = repo.index()?`
2. stage changed paths (`index.add_path(...)` / `index.remove_path(...)`)
3. `index.write()?`
4. `let tree_id = index.write_tree()?`
5. `let tree = repo.find_tree(tree_id)?`
6. resolve signature:
   - `repo.signature()`
   - fallback `Signature::now("taurhaus", "templates@local")`
7. resolve parent:
   - `repo.head().ok()?.peel_to_commit().ok()` (optional)
8. `repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)`

## Dirty status check
- `StatusOptions::new()` with:
  - include untracked
  - recurse untracked dirs false/true based on scope
- Filter statuses to managed template paths only.

## History read
- `repo.revwalk()` from `HEAD`
- parse commit metadata and touched files for template-scoped commits.

## Revert strategy
Prefer safe “restore file at commit -> new commit” (not reset/checkout branch moves).

Flow:
1. find target commit/tree
2. extract blob for target file(s)
3. overwrite working file(s)
4. stage + commit
5. message: `templates: revert <type> <id> to <short_sha>`

## Validation and Error Handling

## Pre-commit validation
- YAML parse + schema validation for changed role/preset files.
- Reject commit if validation fails.
- Keep files on disk; return structured validation error.

## Fallback mode
- If any git step fails:
  - mutation remains persisted in YAML
  - return success with warning field (`git_commit_skipped`)
  - log with `tracing::warn!`

## Locking
- Use process-local mutex for template mutations.
- Use advisory file lock pattern aligned with `src-tauri/src/coordination/stores/lock.rs`:
  - lock file per template storage root (for example `templates/.lock`)
  - tolerate Windows lock unsupported error (`raw_os_error == 1`) with warning + continue.
- Use atomic temp-write + rename pattern aligned with `src-tauri/src/coordination/stores/config.rs` (`*.tmp` then rename, Windows fallback to direct write).

## Alignment with existing git module patterns
- Follow `src-tauri/src/git/status.rs` for dirty detection defaults:
  - `StatusOptions::include_untracked(true)`
  - `StatusOptions::recurse_untracked_dirs(false)` unless explicitly scanning nested template trees.
- Follow `src-tauri/src/git/commits.rs` for revwalk semantics:
  - `Sort::TOPOLOGICAL | Sort::TIME`
  - graceful behavior when `push_head()` fails (repo with no commits yet).
- Keep error mapping style consistent (`AppError`/domain error wrapping with contextual path details).

## IPC Surface (shipped)

Registered names, not the proposal names. See [ipc-reference.md](./ipc-reference.md) for full signatures.

### CRUD + read
- `templates_list_roles_full() -> Vec<RoleTemplateFull>`
- `templates_get_role(role_id) -> RoleTemplate`
- `templates_upsert_role(request: TemplatesUpsertRoleRequest) -> RoleTemplate`
- `templates_delete_role(role_id) -> ()`
- `import_role_from_file(request) -> ImportRoleFromFileResult` / `export_role_to_file(request) -> RoleExportResult`
- `templates_list_presets_full() -> Vec<TeamPresetFull>`
- `templates_get_preset(preset_id) -> TeamPreset`
- `templates_upsert_preset(request: TemplatesUpsertPresetRequest) -> TeamPreset`
- `templates_delete_preset(preset_id) -> ()`

Mutations return the saved template re-read from the store, or unit for deletes — no commit envelope crosses IPC. Commit state is read separately from `templates_get_storage_status`.

### Git insights
- `templates_get_storage_status() -> TemplateStorageStatus { mode, repo_initialized, dirty, pending_actions, last_commit }`
- `templates_get_history(limit?, cursor?) -> TemplateCommitPage`
- `templates_get_diff(commit_id) -> TemplateDiff`
- `templates_revert(request: TemplateRevertRequest) -> ()`
- `templates_flush_pending() -> TemplateFlushResult { committed, commit_id }` (manual force-commit)

### Git IPC payloads
- `TemplateCommitPage`:
  - `commits: Vec<TemplateCommit>`
  - `next_cursor: Option<String>` (cursor is commit SHA from last item)
- `TemplateCommit`:
  - `commit_id: String`
  - `short_id: String`
  - `message: String`
  - `author: String`
  - `timestamp: i64`
  - `changed_paths: Vec<String>` (template-scoped only)
- `TemplateDiff`:
  - `commit_id: String`
  - `files: Vec<TemplateDiffFile>` (`path`, `status`, `hunks`)
  - `stats: TemplateDiffStats { files_changed: u32, insertions: u32, deletions: u32 }`
- `TemplateRevertRequest`:
  - `id: String` (the template being reverted)
  - `commit_hash: String`
  - Revert restores one template to one commit state. There is no path filter and no `validate_only` dry run.

### Compose hooks
- `templates_compose_team(request) -> CompositionResult { roster, warnings, validation_errors }`

There is no `templates_validate_composition` command; composition is validated inside `templates_compose_team`, which reports problems as `validation_errors` instead of failing the call. It returns a composed roster, not an `InitializeTeamRequest` — the caller builds the initialize payload from that roster.

## Store mutation result (internal)

`TemplateStore` write methods return `TemplateMutationResult`, which is consumed inside the command layer and never serialized to the frontend:

- `commit_id: Option<String>`
- `committed: bool`

## Startup/Shutdown behavior

### Startup
1. Load template storage.
2. If repo exists, run dirty-tree recovery commit attempt before accepting new writes.
3. Rehydrate debounce state from `_meta/state.json` (if present) and merge with current dirty status.
4. If recovery fails validation, surface non-blocking warning in template UI.

### Shutdown
- If debounce queue pending:
  - persist pending queue metadata to `_meta/state.json`
  - attempt force flush immediate commit with message `templates: shutdown flush <n> changes`
  - if flush fails or app exits abruptly, startup recovery still commits filesystem state

## Security and Safety Notes
- Never execute git hooks from template repo (do not invoke external git CLI).
- Limit staged paths to `roles/`, `presets/`, `_meta/index.yaml`.
- Validate IDs/filenames to prevent path traversal.

## Testing Strategy

### Unit tests
- lazy init success/failure
- debounce coalescing logic
- commit message formatting
- fallback path when commit fails
- recovery commit on dirty startup
- external edit auto-commit

### Integration tests
- tempdir storage + git repo initialization
- CRUD -> expected YAML + commit history
- revert creates forward commit with restored content
- crash simulation: write YAML + pending state, restart triggers recovery

## Recommended rollout
1. Phase A: plain YAML CRUD + schema validation + compose APIs
2. Phase B: lazy git init + direct per-action commit (no debounce)
3. Phase C: debounce queue + startup/shutdown recovery + external edit auto-commit
4. Phase D: history/diff/revert IPC and UI

This sequencing keeps template functionality available immediately while de-risking git automation complexity.
