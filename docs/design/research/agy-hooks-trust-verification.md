# agy 1.1.22 lifecycle hooks — live trust verification

Date: 2026-08-28 (UTC times below). Host: WSL2 Ubuntu, `agy` 1.1.22 at `~/.local/bin/agy`.
Purpose: close the "hooks never fire" negative in
`~/projects/taurhaus/docs/design/research/agy-report-opus.md` §6, which was blocked on the
workspace trust gate.

## Verdict

**Yes — hooks fire once the workspace is trusted, and the workspace-level `<workspace>/.agents/hooks.json`
is the file that loaded.** The earlier `loaded 0 named hooks` was the trust gate *combined with* a second,
independent factor: print mode (`agy -p`) never loads workspace customizations at all, even when trusted.

## Method

Scratch workspace `/.../scratchpad/agy-trust-probe`, `git init`-ed, with
`.agents/hooks.json` registering `PreInvocation`, `PostInvocation` and `Stop`, each running
`.agents/record.sh <Event>` which appends `{probeEvent, probeTsNs, probeIso, probeCwd, payload}` to
`hook-events.log` and prints `{}`. agy was run interactively in a detached tmux session `agyprobe1`;
the trust prompt was accepted for that scratch path only.

## Timeline of verified observations

| # | Observation | Evidence |
|---|---|---|
| 1 | Baseline, untrusted, print mode: no hooks | `agy -p /hooks --output-format json` → `{"hooks":[]}`; `cli-20260828_194241.log`: `hooks_manager.go:53] loaded 0 named hooks from 0 hooks.json file(s)` |
| 2 | Trust prompt shown for the scratch dir only | tmux pane: `Do you trust the contents of this project? … > Yes, I trust this folder` |
| 3 | Accepting trust writes **only** `~/.gemini/antigravity-cli/settings.json` `trustedWorkspaces[]` | before/after `cat`; `~/.gemini/trustedFolders.json` (a gemini-cli file) was **not** touched — sha256 `f93c56e3…` unchanged throughout |
| 4 | Trusted **print** mode still loads nothing | `cli-20260828_194405.log`: `loaded 0 named hooks from 0 hooks.json file(s)`; `/skills` in the same workspace listed only the 5 builtins, not the probe skill I planted at `.agents/skills/probe-skill/SKILL.md` |
| 5 | Trusted **interactive** TUI loads the workspace file | `cli-20260828_194529.log`: `loaded 0 named hooks from 0 hooks.json file(s)` at startup, then 20 ms later `loaded 1 named hooks from 1 hooks.json file(s)` — the reload-on-workspace-change path added in 1.1.1 |
| 6 | `/hooks` lists them | TUI: `PreInvocation … 1 hook`, `PostInvocation … 1 hook`, `Stop … 1 hook`; drilling into PreInvocation shows `[trust-probe] trust-probe  1 hook  (workspace)  [on/off]` — explicitly labelled **(workspace)** |
| 7 | Hooks actually execute | 11 payloads captured across 3 turns (below) |

## Captured payloads (verbatim, no redaction needed — nothing sensitive present)

Turn 1, prompt `reply OK`:

```json
=== PreInvocation @ 2026-08-28T17:46:21.177412200Z (cwd=<workspace>/.agents) ===
{
  "artifactDirectoryPath": "~/.gemini/antigravity-cli/brain/<uuid>",
  "conversationId": "<uuid>",
  "initialNumSteps": 1,
  "invocationNum": 0,
  "modelName": "gemini-3.7-flash-high",
  "transcriptPath": "~/.gemini/antigravity-cli/brain/<uuid>/.system_generated/logs/transcript_full.jsonl",
  "workspacePaths": ["<workspace>"]
}

=== PostInvocation @ 2026-08-28T17:46:24.646195432Z ===
{ …identical shape to PreInvocation… }

=== Stop @ 2026-08-28T17:46:24.654062354Z ===
{
  "artifactDirectoryPath": "~/.gemini/antigravity-cli/brain/<uuid>",
  "conversationId": "<uuid>",
  "error": "",
  "executionNum": 0,
  "fullyIdle": true,
  "modelName": "gemini-3.7-flash-high",
  "terminationReason": "NO_TOOL_CALL",
  "transcriptPath": ".../transcript_full.jsonl",
  "workspacePaths": ["<workspace>"]
}
```

Turn 3 used a tool (`echo hello-from-probe`, approved with the non-persisting "1. Yes"):

```
PreInvocation  17:48:51.177  invocationNum=0 initialNumSteps=5
PostInvocation 17:49:09.724  invocationNum=0 initialNumSteps=5
PreInvocation  17:49:09.733  invocationNum=1 initialNumSteps=7
PostInvocation 17:49:11.968  invocationNum=1 initialNumSteps=7
Stop           17:49:11.977  executionNum=0 fullyIdle=true terminationReason="NO_TOOL_CALL" error=""
```

## Answers

### 1. Do hooks fire under trust?

Yes. `PreInvocation`, `PostInvocation` and `Stop` all executed, in the interactive TUI, with the scratch
workspace trusted. Handler cwd was `<workspace>/.agents` — i.e. the directory containing `hooks.json`,
exactly as the shipped doc states.

Two independent gates, not one:
* **Trust** — required.
* **Interactive mode** — required for *workspace* customizations. `agy -p` (print mode) loaded 0 hooks
  from 0 files even after trust, and also failed to see a workspace `.agents/skills/` skill. So
  `agy -p /hooks` is **not** a valid health check for a workspace-scoped install (it is presumably still
  valid for user-level hooks — UNVERIFIED; would be verified by running `agy -p /hooks` on a host that has
  `~/.gemini/config/hooks.json` populated).

### 2. Workspace-level or only user-level?

**Workspace-level loads.** `/hooks` labels the entry `(workspace)` and the log counts
`1 named hooks from 1 hooks.json file(s)` when the workspace file is the only one present.

User-level was tested without touching the real `~/.gemini`, by running agy under an isolated
`HOME=<scratch>/fakehome2` containing a `config/.migrated` marker (the same state the real home is in)
plus two *different* user-level files:

```
<fakehome2>/.gemini/antigravity-cli/hooks.json  → 1 named hook  (probe-appdata-1)
<fakehome2>/.gemini/config/hooks.json           → 2 named hooks (probe-shared-1, probe-shared-2)
log: hooks_manager.go:53] loaded 3 named hooks from 2 hooks.json file(s)
```

Both user-level paths are read as distinct sources and merged. So the path taurhaus writes,
`~/.gemini/antigravity-cli/hooks.json` (`agy_hooks_installer.rs:7`), **is** loaded by 1.1.22.

### 3. What does the `Stop` payload carry?

Common fields: `conversationId`, `workspacePaths[]`, `transcriptPath`, `artifactDirectoryPath`,
`modelName`. Stop-specific: `executionNum`, `terminationReason`, `error`, **`fullyIdle`**.

* `fullyIdle: true` on all three turns, including the turn with a tool call.
* `terminationReason: "NO_TOOL_CALL"` — a **SCREAMING_SNAKE enum**, not the
  `model_stop` / `max_steps_exceeded` / `error` lowercase set §6 of the report predicted. Only
  `NO_TOOL_CALL` was observed; the other enum members are UNVERIFIED (would be verified by forcing a
  max-steps run or an API error mid-turn).
* `error: ""` on success.
* `executionNum` was `0` on every turn, and `invocationNum` restarted at `0` on every turn — these are
  **per-turn counters, not conversation-monotonic**. Do not use them for dedup across turns; use
  `conversationId` + wall clock.
* `Stop` fires **exactly once per user turn**, after the last `PostInvocation`, regardless of how many
  model invocations the turn took (turn 3: 2×Pre/Post, 1×Stop). That is the correct "turn finished" edge.

`transcriptPath` is real and written for CLI sessions (this closes another §5 unverified item):
`~/.gemini/antigravity-cli/brain/<conversationId>/.system_generated/logs/transcript_full.jsonl`,
8 lines after 3 turns, snake_case keys `{content, created_at, source, status, step_index, type}`.
Note it lives under the app-data brain dir, **not** in the workspace as the shipped doc's example implies.

### 4. Latency between turn end and `Stop`

Measured from the hook handlers' own nanosecond timestamps:

| turn | model invocations | last `PostInvocation` → `Stop` |
|---|---|---|
| 1 | 1 | **7.98 ms** |
| 2 | 1 | **6.56 ms** |
| 3 | 2 (one tool call) | **8.58 ms** |

From the user's point of view: the assistant's answer text was already on screen when `Stop` fired, and
the TUI's `Generating…` indicator cleared in the *same* 20–28 ms poll bucket in which `Stop` landed
(turn 2: busy→idle at +1.191 s, `Stop` at +1.191 s). Because hooks run synchronously and block the agent
loop, the handler's own runtime is added to the user-visible turn latency — the probe handler was a few
ms of `sh` + `date` + append.

Conclusion: `Stop` is effectively simultaneous with turn end (single-digit ms), so it is a sound idle
edge for taurhaus with no debounce needed for the "turn ended" signal itself.

## Findings that affect the taurhaus installer

`src-tauri/src/coordination/agy_hooks_installer.rs`:

1. **`HOOKS_FILE = "antigravity-cli/hooks.json"` (line 7) still works on 1.1.22** — proven above (3 named
   hooks from 2 files). Not broken.
2. **But it is the legacy path.** agy 1.0.8 changelog: *"Fixed a bug where the `/hooks` command wrote
   configurations to `~/.gemini/antigravity-cli/hooks.json` instead of the shared
   `~/.gemini/config/hooks.json`, ensuring hooks remain synchronized between the TUI and the backend."*
   The canonical user-level file is `~/.gemini/config/hooks.json`.
3. **There is a one-shot migration that can destroy data.** On a home with no `~/.gemini/config/.migrated`
   marker, startup logs:
   ```
   migrate.go:131] Migrating file <home>/.gemini/antigravity-cli/hooks.json to <home>/.gemini/config/hooks.json
   migrate.go:150] Created symlink from <home>/.gemini/antigravity-cli/hooks.json to <home>/.gemini/config/hooks.json
   ```
   In my isolated-HOME run this **overwrote** an existing `config/hooks.json` (2 named hooks) with the
   `antigravity-cli/hooks.json` content (1 named hook) and replaced the latter with a symlink.
   The real home already has `~/.gemini/config/.migrated` (dated 2026-08-28 01:06) and neither hooks.json
   exists, so that specific destructive path will not re-run here — but note that if the marker is ever
   absent (fresh machine, new user, reset config) and taurhaus has already written
   `antigravity-cli/hooks.json`, a user's `config/hooks.json` would be clobbered by taurhaus's file.
4. **`write_hooks` uses `fs::rename` onto the target (line 164).** If `antigravity-cli/hooks.json` is a
   symlink into `config/hooks.json` (the post-migration shape), the rename replaces the *symlink* with a
   regular file, silently desynchronising taurhaus's hooks from the file the TUI edits. Writing
   `~/.gemini/config/hooks.json` directly, or resolving the symlink before the atomic swap, avoids this.
5. **`agy -p /hooks` is not a usable installation health check for workspace hooks** (it reports 0). For
   user-level hooks it may still work — UNVERIFIED. A reliable check is reading back the JSON file plus,
   optionally, tailing `~/.gemini/antigravity-cli/cli.log` for
   `hooks_manager.go:53] loaded N named hooks from M hooks.json file(s)`.
6. **Design fit is good.** `PreInvocation → busy` fires per model invocation (multiple times per turn —
   idempotent "still busy" writes, fine), `Stop → idle` fires exactly once per turn with `fullyIdle`
   already accounting for subagents. Keep handlers to a few ms: they block the agent loop and add
   directly to user-visible latency. `conversationId` is on every payload and is the right key;
   `executionNum`/`invocationNum` are **not** monotonic across turns.

Also confirmed incidentally: `~/.gemini/antigravity-cli/presence/<conversationId>.lock` files are created
per live conversation (the presence registry from §4 of the report).

## Cleanup / state restored

* `~/.gemini/antigravity-cli/settings.json` restored **byte-for-byte**: sha256
  `a17dd3ecb161b875e2a42261c63154191841fb80f7b53e2085550703fc48fcf6`, mode `600`, `cmp` reports identical.
  The temporary `trustedWorkspaces` entry for the scratch path is gone.
* `~/.gemini/trustedFolders.json` never changed: sha256
  `f93c56e3fab19e5b8654646eb84130bca7ed2c4d4e8f135a54a5e99836a9f38b`, mode `600`.
* No other directory was trusted. The permission prompt was answered with "1. Yes" (session-scoped), never
  option 3 ("Persist to settings.json").
* The isolated-HOME tests wrote only under the scratchpad; the real `~/.gemini` file-state hash was
  identical before and after (`df445c53…`).
* tmux session `agyprobe1` exited with `/exit` and is gone; `tmux ls` shows only the pre-existing
  `taurhaus` session, which I did not touch. No stray `agy` process (`pgrep -x agy` → none).
* **Residue I could not remove without writing to `~/.gemini`** — ordinary artifacts any agy run produces,
  left in place deliberately rather than deleting user data:
  `antigravity-cli/log/cli-20260828_19*.log`, `antigravity-cli/brain/{71e76b0b-…,a7b76260-…}/`,
  `antigravity-cli/conversations/{71e76b0b-…,a7b76260-…}.db`,
  `antigravity-cli/annotations/*.pbtxt`, `antigravity-cli/presence/*.lock` (stale),
  appended lines in `antigravity-cli/history.jsonl`, refreshed `cache/`, `updater/update_status.json`,
  `antigravity-oauth-token` (refreshed by agy itself), and one
  `antigravity-cli/crashes/crash_262770_*.log` written when the first tmux launch was killed.
  Say the word and I will delete the conversation-scoped ones (`brain/`, `conversations/`, `annotations/`,
  `presence/` entries for those two conversation IDs).

## Scratch artifacts

* `/tmp/claude-1000/-home-mstie-projects-taurhaus/<uuid>/scratchpad/agy-trust-probe/` — probe workspace, `.agents/hooks.json`, `hook-events.log` (11 payloads)
* `…/scratchpad/settings.json.BEFORE`, `…/trustedFolders.json.BEFORE` — restore sources
* `…/scratchpad/trace.tsv`, `trace2.tsv`, `timing.txt` — latency traces
* `…/scratchpad/fakehome/`, `fakehome2/`, `fakews/` — isolated-HOME discovery tests
* `…/scratchpad/agy-changelog.txt` — full `agy changelog` (hook entries at 1.0.8 / 1.1.1 / 1.1.10)
