# Compaction Pipeline Review — 2026-03-08

Task: `#685`

Reviewed code:
- `src-tauri/src/session_scanner/compaction.rs`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/coordination/stores/compaction.rs`
- `src-tauri/src/coordination/stores/operational.rs`
- `src-tauri/src/coordination/reinjection.rs`

## Bottom line

The overall architecture is directionally sound and mostly fails closed rather than misrouting reinjection. The main weak spots are not in the scoring model itself; they are in boundary conditions around file tailing, delayed Codex delivery, and cleanup after team/member removal.

## A. Bugs To Fix Now

### 1. Partial JSONL writes can be lost permanently

Code:
- `src-tauri/src/session_scanner/compaction.rs:170`
- `src-tauri/src/session_scanner/compaction.rs:193`

Why it is a bug:
- `track_read_start(...)` advances the stored offset to the current file length before any parsing succeeds.
- `read_appended_lines(...)` accepts any non-empty chunk returned by `read_line(...)`, even if it did not end in `\n`.
- If Codex has only partially flushed a JSONL record when the poll runs, that partial fragment is consumed, JSON parsing fails, and the offset has already advanced past the fragment start.
- On the next poll, only the remainder of the line is visible, so the compaction event is lost forever.

Impact:
- Real compaction can be missed even though the line is fully written moments later.
- This is the highest-confidence correctness issue in the pipeline.

Fix direction:
- Keep a per-file carry-over buffer for a trailing unterminated line, or only commit the offset through the last newline seen.
- Treat truncation/rotation similarly: reset safely, but do not silently discard unread bytes that may contain a complete line.

### 2. Codex reinjection can land in a later turn, not the compacted one

Code:
- `src-tauri/src/session_scanner/compaction.rs:430`
- `src-tauri/src/session_scanner/compaction.rs:465`
- `src-tauri/src/session_scanner/compaction.rs:479`

Why it is a bug:
- Delivery only checks freshness, team/member attachment, matching `session_id`, matching `pane_id`, and foreground command still being Codex.
- None of those guards prove the agent is still at the same prompt boundary that immediately followed compaction.
- If the agent starts a new turn within the 15-second freshness window, Taurhaus can still inject the operational card into that later turn.

Impact:
- The reinjection text can contaminate a user/agent turn that is already underway.
- This is a correctness problem, not just UX roughness.

Fix direction:
- Add a stronger prompt-boundary guard before `send_tmux_keys_with_enter(...)`.
- Plausible signals: last-output age, a scanner-side “idle at prompt” bit, or a compaction-generation token proving no intervening turn has started.

### 3. Pending delivery bookkeeping can recreate state after team/member removal

Code:
- `src-tauri/src/session_scanner/compaction.rs:418`
- `src-tauri/src/session_scanner/compaction.rs:469`
- `src-tauri/src/session_scanner/compaction.rs:494`
- `src-tauri/src/session_scanner/compaction.rs:507`
- `src-tauri/src/session_scanner/compaction.rs:480`
- `src-tauri/src/coordination/stores/compaction.rs:173`

Why it is a bug:
- When a pending Codex delivery fails the attachment checks because the team was disbanded or the member was removed, the code still calls `record_delivery_at(...)`.
- `record_delivery_at(...)` persists a new compaction-state file and `MemberCompactionStore::save(...)` will recreate the directory tree if needed.
- So a disbanded team can have `teams/<team>/state/compaction/...` recreated by stale pending work.

Impact:
- Leaves orphaned state behind after disband/remove flows.
- Violates teardown expectations and can confuse later audits.

Fix direction:
- Do not persist delivery results if the team config is gone or the member no longer exists.
- Codex and Claude paths should both treat “team/member no longer exists” as terminal drop-with-log, not “save skipped state”.

## B. Hardening For Later

### 1. Session-to-member scoring is safe-biased, but stale runtime still causes avoidable skips

Code:
- `src-tauri/src/session_scanner/compaction.rs:274`

Assessment:
- The `session_id = 3`, `pane_id = 2` scoring is reasonable for the current state model.
- The implementation is more likely to skip than to misinject because delivery revalidates `pane_id`, `session_id`, and live foreground command later.
- It does not, however, consider runtime freshness (`health`, `last_seen_at`) during initial resolution, so stale records can still win the score and then fail during delivery.

Recommendation:
- If false negatives become common, incorporate runtime freshness/health into scoring or pre-filter stale runtime entries before scoring.

### 2. The 2-second paired-signal dedup window is acceptable, but still a heuristic

Code:
- `src-tauri/src/session_scanner/compaction.rs:215`

Assessment:
- For the observed Codex behavior, collapsing `compacted` followed by `context_compacted` within 2 seconds is reasonable.
- The current logic only suppresses the second signal, so two rapid real compactions still produce two `compacted` events; that part is fine.
- The weak spot is future format drift: if Codex changes ordering or timing, the heuristic can either double-inject or stop deduping cleanly.

Recommendation:
- Keep it for now.
- If Codex exposes a stable pair identifier or sequence number later, switch to that.

### 3. Claude hook fallback is conservative but can skip valid same-project cases

Code:
- `src-tauri/src/coordination/claude_hooks.rs:163`
- `src-tauri/src/coordination/claude_hooks.rs:218`

Assessment:
- Runtime `session_id` match plus `cwd` check is the right primary resolution path.
- The cwd-only fallback intentionally returns `None` on ambiguity.
- That means multiple managed Claude members on the same project root can lose reinjection when runtime state is absent or stale.

Recommendation:
- Keep the conservative skip behavior.
- If Taurhaus needs stronger recovery later, add a more explicit runtime identity token instead of relaxing ambiguity handling.

## C. Acceptable Risks

### 1. Equal-score ambiguity guard is conservative in the right direction

Code:
- `src-tauri/src/session_scanner/compaction.rs:358`

Assessment:
- Returning `None` on equal score avoids wrong-member injection when two candidates are indistinguishable.
- This may drop a valid reinjection in some multi-team/same-project edge cases, but that is preferable to injecting into the wrong agent.

### 2. Per-member idempotency is sufficient for the current scanner execution model

Code:
- `src-tauri/src/session_scanner/mod.rs:523`
- `src-tauri/src/session_scanner/compaction.rs:375`
- `src-tauri/src/coordination/stores/compaction.rs:135`

Assessment:
- Today the compaction pipeline is reached from the session scanner pass, not from multiple independent concurrent workers.
- With that call shape, a per-member `(session_id, compaction_timestamp)` state file is enough.
- If detection ever runs concurrently from multiple scanner instances or daemon workers, this must become an atomic claim/lease instead of a plain read-then-save.

### 3. Same-project multi-agent Codex resolution is mostly correct for current data

Code:
- `src-tauri/src/session_scanner/compaction.rs:317`

Assessment:
- Exact `session_id` outranking pane match is the right priority.
- The existing test covering “pane match vs exact session match” aligns with the intended model.
- Given unique session IDs and later delivery revalidation, this is acceptable for now.

## Recommended follow-up tasks

1. Fix JSONL compaction tailing so partial lines and truncation cannot drop events.
2. Add a prompt-boundary / intervening-turn guard before Codex tmux reinjection.
3. Prevent compaction delivery bookkeeping from recreating state after team/member removal or disband.
