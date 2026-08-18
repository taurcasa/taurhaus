# Inotify Pre-Pruning Analysis — 2026-03-09

## Question

Can taurhaus reduce inotify watch count by avoiding registration of obviously-ignored subtrees up front, instead of recursively watching whole project trees and filtering events only after they arrive?

## Short answer

Yes.

The current `notify` usage does not provide a subtree-exclusion hook for recursive registration. The practical implementation is:

- replace whole-tree recursive registration with one watcher per root that registers many `RecursiveMode::NonRecursive` directory watches
- build that directory set from a pruned traversal of the tree
- skip subtrees that are never useful to taurhaus (`node_modules`, `target`, `dist`, `.cache`, `.next`, `.nuxt`, `.svelte-kit`, `.playwright-mcp`, Python cache dirs, and gitignored directories)
- keep a minimal `.git` subset watched for correctness: `.git`, `.git/refs`, and `.git/refs/heads/**`
- reconcile the watched directory set when directory topology changes or `.gitignore` changes

That preserves correctness while materially reducing watch count.

## Feasibility analysis

### `notify` crate capability

Current taurhaus code uses only this registration surface:

- `watch(path, RecursiveMode::Recursive)`
- `watch(path, RecursiveMode::NonRecursive)`

There is no subtree-skip callback or ignore predicate in the watcher registration API currently used by taurhaus. So selective recursion is not available through the existing `notify` surface.

That makes the feasible path a manual directory walk plus non-recursive per-directory registration.

### Candidate pre-prune directories

The existing hardcoded post-filter exclusions in [watcher.rs](/home/user/projects/taurhaus/src-tauri/src/fs/watcher.rs) are:

- `node_modules`
- `target`
- `dist`
- `.cache`
- `.playwright-mcp`
- `.next`
- `.nuxt`
- `.svelte-kit`
- Python cache dir (`__pycache__` via `PYTHON_CACHE_DIR`)

Additionally, gitignore rules already suppress event handling for arbitrary ignored paths, so those ignored subtrees are also valid pre-prune candidates.

### Side-effect analysis

#### 1. Would pre-pruning miss events taurhaus actually needs?

For the currently hardcoded tool/build directories, no meaningful taurhaus behavior depends on file-by-file events from inside them.

What taurhaus needs from watched trees is:

- source/content edits for sidebar freshness and search reindexing
- session handoff file creation under `docs/sessions/`
- git state changes from `.git/HEAD`, `.git/index`, and branch refs

None of those require watching inside `node_modules`, `target`, `dist`, `.next`, `.nuxt`, `.svelte-kit`, `.cache`, `.playwright-mcp`, or Python cache directories.

#### 2. What about `npm install` / `cargo build` completion?

Those commands primarily mutate pruned directories such as `node_modules` and `target`.

Taurhaus does not currently rely on watching those directories to detect command completion. Build/install completion is reflected elsewhere:

- CLI session activity / daemon session scanning
- eventual git status changes if tracked files are affected
- source-tree changes outside the ignored subtree if the command updates lockfiles or manifests

So pruning those directories does not remove a correctness-critical completion signal.

#### 3. What about dynamically created ignored directories?

Example: `node_modules/` does not exist when the watch starts, then appears later.

With pre-pruning, the parent directory still emits the create event for `node_modules/`. The watcher can inspect the new directory path, determine it is pruned, and deliberately avoid adding watches under it.

That preserves the intended reduction and does not lose anything taurhaus needs.

#### 4. What about dynamically created non-ignored directories?

Those must become watched.

The implementation therefore needs to respond to directory-create events by reconciling the watched directory set and registering any newly eligible directories. This is required because the new model no longer relies on one recursive root watch.

#### 5. What about `.gitignore` changes?

This is the main correctness-sensitive case.

If ignore rules change, the desired watched subtree set changes too. So the implementation must:

- rebuild the `Gitignore` matcher
- re-walk the root
- add newly unignored directories
- unwatch newly ignored directories

This is the right tradeoff: `.gitignore` changes are rare compared with routine file churn, so an occasional full tree reconcile is acceptable.

#### 6. What about `.git`?

A naive prune of `.git/` would break git-status refresh signals.

Taurhaus still needs to see:

- `.git/HEAD`
- `.git/index`
- `.git/refs/heads/**`

So the correct pre-pruned model is not “ignore `.git` entirely.” It is:

- watch `.git` itself
- watch `.git/refs`
- watch `.git/refs/heads/**`
- continue ignoring the rest of `.git` internals

That preserves the existing signal contract while still avoiding the large, low-value parts of `.git`.

## Implementation decision

Pre-pruning is feasible without correctness regression.

The concrete implementation tasks are:

1. Add shared watch-planning helpers in [watcher.rs](/home/user/projects/taurhaus/src-tauri/src/fs/watcher.rs):
   - determine whether a directory should be watched
   - build the desired per-root watched-directory set
   - reconcile a live watcher against that desired set
2. Replace app-local `watch_project(...)` whole-tree recursive registration with per-directory non-recursive registration.
3. Mirror the same logic in daemon-owned WSL watches in [watch.rs](/home/user/projects/taurhaus/src-tauri/src/daemon/watch.rs).
4. Reconcile the watched-directory set on:
   - directory topology changes
   - `.gitignore` / `.taurhausignore` changes
5. Add per-root watched-directory count logging so reductions are directly visible in logs.

## Verification strategy

Verification needs three layers:

1. **Unit/regression tests**
   - tool/build dirs are absent from the watched set
   - `.git` signal dirs remain watched
   - `.gitignore` changes shrink/expand the watched set correctly
   - daemon-owned WSL watch registration gets the same pruning behavior
2. **Quality gate**
   - `just check-quick`
3. **Watch-count comparison**
   - compare the old whole-tree directory count against the new pre-pruned directory count for a representative root
   - collect a short resource-monitor CSV while a daemon process is holding a pre-pruned watch set, to confirm the live inotify count matches the reduced shape

## Expected effect

The biggest watch-count reduction comes from removing heavy generated/vendor subtrees from registration entirely.

Expected wins are largest in repos that contain:

- `node_modules/`
- `target/`
- framework caches/output dirs (`.next`, `.nuxt`, `.svelte-kit`, `dist`)
- large gitignored generated trees

The reduction will not be noticeable for tiny repos, but it should be material for the large active project set that currently drives the `~60k-90k` daemon watch counts.
