# SessionResolver Trait Design

## Problem

The idle detector (`idle.rs`) is hardcoded to Claude Code's session layout:
- Slug-based project dirs (`~/.claude/projects/<slug>/`)
- JSONL transcript files (`<uuid>.jsonl`)
- Subagent compaction dirs (`<uuid>/subagents/`)

Codex and Gemini use fundamentally different layouts. We need a per-tool abstraction.

## Design: Trait + Per-Tool Implementations

```rust
/// Trait for tool-specific session file resolution and activity detection.
///
/// Each CLI tool stores session data differently. Implementations know how to:
/// 1. Find session files for a given project path
/// 2. Determine the latest session's activity state
pub trait SessionResolver: Send + Sync {
    /// Detect whether a session for `project_path` is active or idle.
    ///
    /// Checks tool-specific session files and returns activity state,
    /// session ID, and path to the active session file.
    fn detect_idle(&self, project_path: &str) -> IdleResult;
}
```

### Implementation: `ClaudeResolver`

Existing logic from `detect_idle_in()`, extracted into a struct:

```rust
pub struct ClaudeResolver {
    /// Base directory: `~/.claude/projects/`
    base_dir: PathBuf,
}

impl SessionResolver for ClaudeResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        // 1. path_to_slug(project_path) → slug
        // 2. base_dir / slug → project_dir
        // 3. find_latest_jsonl(project_dir) → latest .jsonl
        // 4. Check mtime of .jsonl + subagent dir
        // 5. classify_mtime() → Active/Idle
    }
}
```

### Implementation: `CodexResolver`

New. Handles Codex's date-organized layout:

```rust
pub struct CodexResolver {
    /// Base directory: `~/.codex/sessions/`
    base_dir: PathBuf,
}

impl SessionResolver for CodexResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        // 1. Scan recent date dirs (today, yesterday) for .jsonl files
        // 2. For each file, read first line → session_meta → check cwd
        // 3. If cwd matches project_path, check mtime
        // 4. classify_mtime() → Active/Idle
        //
        // Optimization: cache session_id → file_path mapping to avoid
        // re-scanning on every poll. Invalidate when date changes.
    }
}
```

**Key challenge:** Codex organizes by date, not project. Finding a session for a specific project requires scanning potentially many files. Mitigation:
- Only scan today's and yesterday's directories (active sessions are recent)
- Cache the session_id → path mapping
- The process scanner already confirmed the process exists — we're just checking activity

### Implementation: `GeminiResolver`

New. Handles Gemini's hash-based layout:

```rust
pub struct GeminiResolver {
    /// Base directory: `~/.gemini/tmp/`
    base_dir: PathBuf,
}

impl SessionResolver for GeminiResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        // 1. SHA-256(project_path) → hash
        // 2. base_dir / hash / chats / → session dir
        // 3. Find latest session-*.json by mtime
        // 4. Check file mtime → classify_mtime() → Active/Idle
        // 5. Optionally read lastUpdated from JSON for more precision
    }
}
```

**Key advantage:** Deterministic O(1) directory lookup via hash. Very similar to Claude's slug approach.

## Factory Function

```rust
/// Get the resolver for a CLI tool.
pub fn resolver_for(tool: CliTool) -> &'static dyn SessionResolver {
    static CLAUDE: OnceLock<ClaudeResolver> = OnceLock::new();
    static CODEX: OnceLock<CodexResolver> = OnceLock::new();
    static GEMINI: OnceLock<GeminiResolver> = OnceLock::new();

    match tool {
        CliTool::Claude => CLAUDE.get_or_init(|| ClaudeResolver::new()),
        CliTool::Codex => CODEX.get_or_init(|| CodexResolver::new()),
        CliTool::Gemini => GEMINI.get_or_init(|| GeminiResolver::new()),
    }
}
```

## Integration Point

In `idle.rs`:

```rust
pub fn detect_idle(project_path: &str, tool: CliTool) -> IdleResult {
    resolver_for(tool).detect_idle(project_path)
}
```

This replaces the current implementation which delegates to `detect_idle_in()` with a tool-specific base directory (but still uses Claude's slug logic for all tools).

## File Organization

```
src-tauri/src/session_scanner/
  idle.rs                    → Keep: IdleResult, classify_mtime(), common utilities
  idle/
    claude_resolver.rs       → NEW: ClaudeResolver (extracted from current idle.rs)
    codex_resolver.rs        → NEW: CodexResolver
    gemini_resolver.rs       → NEW: GeminiResolver
    mod.rs                   → NEW: SessionResolver trait, resolver_for()
```

Alternative (simpler): keep everything in `idle.rs` with the trait and all three implementations. The file is currently 444 lines — adding two more implementations might push it to ~600-700 lines, which is still manageable. Split if it gets unwieldy.

## Shared Infrastructure

All three resolvers share:
- `classify_mtime(SystemTime) → SessionState` — same threshold logic
- `file_mtime(Path) → Option<SystemTime>` — stat helper
- `newest_file_mtime(dir) → Option<SystemTime>` — dir scan helper
- `IdleResult` struct — shared return type
- `find_latest_by_mtime(dir, extension)` — generalized file finder

These stay in `idle.rs` as module-level functions.

## Caching Strategy

| Resolver | What to cache | Invalidation |
|----------|--------------|-------------|
| Claude | Latest JSONL path per project dir | Dir mtime change (existing) |
| Codex | Session file path per project_path | Date rollover (new day) |
| Gemini | Latest chat file per hash dir | Dir mtime change (same as Claude) |

## Testing

Each resolver gets unit tests with tempdir-based session structures:
- `ClaudeResolver`: existing tests, just refactored
- `CodexResolver`: create date-organized dirs with JSONL containing session_meta with cwd
- `GeminiResolver`: create SHA-256 hash dirs with chat JSON files

## Implementation Order

1. Extract `ClaudeResolver` from current `idle.rs` (pure refactor, no behavior change)
2. Add `GeminiResolver` (simpler — deterministic hash lookup like Claude)
3. Add `CodexResolver` (more complex — date scanning + cwd matching)
4. Wire up `resolver_for()` factory
5. Update `detect_idle()` to use the factory
6. Run all tests
