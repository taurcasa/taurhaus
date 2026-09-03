# Coordination Through the Daemon

Status: **complete**. Protocol 22 closes B3: the Windows app never mutates team state directly; the daemon and the hook processes (WSL-side, plus the compact hook's native-Windows wrapper runtime) are the only writers, and the module-boundary suite enumerates those exceptions. Protocol 23 applies the same accept-then-poll ownership to team account switching. Protocol 24 makes that authority root-aware and enables Claude team account switching. Protocols 15-22 remain the worked daemon-routing slices.

## The problem, from the field

Team state lives in WSL (`~/.claude/teams`), but on Windows the app process performs coordinated read-modify-write file I/O on it across the `\\wsl.localhost` 9p bridge. Verified live on the operator's machine (2026-09-01, the 0.8.8 team-init failure):

- `LockFileEx` over 9p answers `ERROR_INVALID_FUNCTION` — byte-range locks never engage; the stores' locking degrades to advisory-in-name-only.
- Renaming over a file **any** handle holds open answers `ERROR_ACCESS_DENIED` — the atomic-replace pattern every store save uses cannot work whenever the target is open (including by our own target lock).
- Even where each side's locks "work", they do not interoperate: the daemon's `flock` (Linux) and the app's `LockFileEx` (Windows) are invisible to each other. **There has never been mutual exclusion between the app-side and daemon-side writers.** The per-team critical section is exclusive per process, not per system.

Symptoms this produced: team initialization failed at the inbox append (`os error 5`); every runtime-record save left a zero-byte file; three successive review rounds of fallback engineering (0.8.9-pending: uniform move-aside publish + torn-read-tolerant readers) to make the workaround safe.

The round-3 workaround is sound *as a workaround*: no reader can observe torn content, transients heal, persistent corruption repairs deliberately. But cross-process consistency remains convention, not mechanism.

## Target architecture

**The Windows app never mutates team state directly.** All team-state mutations execute in the daemon, WSL-side, where renames are atomic (ext4) and `flock` provides real exclusion — including against mesh CLI and the tool hooks, which write these same files from WSL and finally share a lock domain with us.

The app's coordination layer becomes a client of daemon RPCs. The daemon serializes mutations per team; the app-side per-team critical section is retired in favor of one that is actually system-wide.

## Scope shape (thin vs thick)

- **Option A — store-level RPCs** (`inbox.append`, `runtime.commit`, `task.commit_status`, …): cheapest, but read-modify-write cycles still span the boundary; every RPC must be compare-and-swap shaped for correctness. Several stores already have CAS shapes (`commit_status_if_unchanged`, probe-then-commit runtime), so this is feasible but leaves orchestration racing.
- **Option B — orchestration moves daemon-side**: the app sends intents (`initialize_team`, `add_member`, `apply_effort`, …) and receives step progress; the daemon owns the pipelines. This also removes the `wsl.exe` interop hop for tmux control (tmux lives in WSL; the daemon already probes it every cycle).

**Recommendation: B, phased.**

1. **B1 — background passes**: deadline pass, self-heal, effort relaunch sweeps move into the daemon. Lowest UI coupling (they already run headless on a timer), highest race-elimination value, and they exercise the daemon-side pipeline host end to end.
2. **B2 — interactive pipelines**: initialize / add / resume / stop, with step-progress delivery to the app (open design question below).
3. **B3 — retire app-side writers**: module-boundary assertion that nothing outside the daemon (and the WSL-native hook/CLI processes) writes under `teams_dir()`; app-side store code becomes read-only, then reads migrate opportunistically to daemon-served snapshots.

The round-3 move-aside/tolerant-reader machinery is **kept**, not reverted: it protects the WSL-side multi-writer set (daemon, mesh CLI, compact-hook processes) and any deployment where the daemon is briefly down. On non-Windows the same routed path runs with the daemon in-process-equivalent locality; one code path everywhere.

## Wire contract

- `PROTOCOL_VERSION` bumped 14 → 15 for the deadline pass, 15 → 16 for initialization, 16 → 17 for add/resume/stop, 17 → 18 for resume-team/reonboard, 18 → 19 for standalone create/disband and roster edits, 19 → 20 to retire the redundant stop-member wire pair, 20 → 21 for self-heal/effort ownership, 21 → 22 for the final writer intents and boundary, 22 → 23 for member account ids and team account switching, and 23 → 24 for per-team root authority; app and daemon move in lockstep via the existing exact-match gate and repair flow.
- New namespaced methods (`coordination.*`), each carrying its expectation (CAS semantics) even in phase B1 — the daemon validates under `flock` and returns typed conflict outcomes, mirroring today's `RuntimeCommitOutcome::Skipped` shapes.
- **B2 progress decision**: initialize uses a `run_id`-keyed in-memory registry and status polling. The app polls at roughly 500 ms and re-emits the existing Tauri progress event; the daemon retains terminal runs for a bounded TTL. Push progress remains a possible later optimization, not part of this slice.

## Testing strategy — closing the blind spot

This entire failure class was invisible to CI because E2E is Linux-only and every path was same-filesystem. The milestone must ship with:

- Store/pipeline tests moving daemon-side unchanged (they are already filesystem-real).
- A module-boundary assertion (B3) that forbids team-state writes from app-side modules, so the class cannot silently return.
- The one thing CI cannot simulate — 9p semantics — stays covered by the field probes documented in the 0.8.9 ledger; a `just` probe recipe (PowerShell `LockFileEx`/`MoveFileEx` checks against `\\wsl.localhost`) makes them repeatable on the operator's machine after Windows/WSL updates.

## Per-team root authority audit (protocol 24)

The protocol-23 account slice mapped the full `teams_dir` consumer set before attempting a Claude-root move. Protocol 24 shipped the mapped authority migration atomically:

- bootstrap still holds one process-wide `CoordinationState`, but it owns the default-root registry and caches orchestrators by root instead of constructing divergent states;
- every name-based operation resolves `team_name → teams_dir` before reading config, while a missing registry entry returns the historical default path without creating a file;
- schedulers, discovery/live status, task scanning, account relationships, and hook reconciliation enumerate the default root plus every registered root;
- daemon team/member/roster runs and the session-activity, live-presence, task-snapshot, and active-project writers mutate beneath the resolved root and retain the B3 writer boundary;
- initialization pins mesh, member-daemon, compact-hook, and native launch routing to the selected Claude root.

A Claude switch stops the team, writes its handoff manifest, moves the team directory by rename or copy/verify/remove, and only then commits the registry entry. A failed move leaves exactly one named team directory loadable and the old registry authority intact. Transcripts remain external and are referenced by the manifest rather than migrated.

The landing is atomic across the audited consumer groups, and the B3 module-boundary assertion remains the release gate. Claude remains one account per team; Codex and Grok remain per-member selectors.

## Risks

- **Daemon becomes a hard dependency for team mutations.** Today's UX already treats the daemon as required on Windows; on Linux dev the daemon is optional — routed coordination makes it required there too for team ops. Acceptable; the app's degrade message must say so plainly.
- **Lockstep migration**: protocol bump plus moving pipeline ownership is the largest coordination change since the daemon hub. Phasing (B1 first) keeps each PR shippable and revertible.
- **No data migration needed**: the state format does not change; only the writer moves.

## Non-goals

- No change to mesh's ownership of its files or the assignment contract.
- No change to the template store (local app-data dir; already has its own correct fallback).
- Not a rewrite of the stores: the daemon hosts the existing store code.
