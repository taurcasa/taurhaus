# Communication Version Timeline

Observed on 2026-03-07 from the local `taurhaus` repository plus the local `~/.claude` communication store.

This document maps the active `taurhaus-team` communication history onto the mesh versions embedded in this repo.

## Sources Used

- `src-tauri/resources/mesh.version`
- `src-tauri/resources/mesh.lock.json`
- `git log` history for the version files and `CHANGELOG.md`
- `~/.claude/teams/taurhaus-team/inboxes/*.json`
- `~/.claude/teams/taurhaus-team/state/task_mutations.jsonl`
- `~/.claude/tasks/taurhaus-team/*.json` file mtimes

## Version Cut Points

The repo currently embeds `mesh 0.2.1`:

- `src-tauri/resources/mesh.version` = `0.2.1`
- `src-tauri/resources/mesh.lock.json` = version `0.2.1`, protocol `1`, schema `1`, git commit `59b2d6a`
- Installed binary on this machine: `mesh 0.2.1`

Version transitions from git history:

| Mesh version | Start point | Evidence |
| --- | --- | --- |
| Pre-bundled / unknown | Before 2026-03-06 04:37 CET | No tracked `mesh.version` file before commit `9b61532` |
| `0.2.0` | 2026-03-06 04:37 CET | Commit `9b61532` created `mesh.version` and `mesh.lock.json` with `0.2.0` |
| `0.2.0` rebuilt | 2026-03-06 10:41 CET | Commit `bc33d5e` updated `mesh.lock.json` git commit from `1a364e9` to `5f72493` but kept semantic version `0.2.0` |
| `0.2.1` | 2026-03-06 19:00 CET | Commit `461662a` changed `mesh.version` and `mesh.lock.json` from `0.2.0` to `0.2.1` |

For communication slicing, the meaningful semantic-version ranges are:

- Pre-bundled / unknown: before 2026-03-06 04:37 CET
- `mesh 0.2.0`: 2026-03-06 04:37 CET through 2026-03-06 18:59 CET
- `mesh 0.2.1`: 2026-03-06 19:00 CET onward

## Timeline

Approximate volume is based on:

- Messages: inbox entry timestamps from `~/.claude/teams/taurhaus-team/inboxes/*.json`
- Task activity: both task-mutation events and task-file mtimes, because task JSON files do not carry their own created-at field

| Mesh version slice | Date range | Approximate message volume | Approximate task volume | Notes |
| --- | --- | --- | --- | --- |
| Pre-bundled / unknown | Before 2026-03-06 04:37 CET | About `1,099` inbox messages | About `300` task JSON files have mtimes in this window; `25` recorded task-mutation events touching `18` tasks | Team communication clearly predates the version lock files. This slice is partly pre-audit for tasks, so message counts are stronger than task-mutation counts |
| `mesh 0.2.0` | 2026-03-06 04:37 CET to 2026-03-06 18:59 CET | About `240` inbox messages | About `34` task JSON files with mtimes in this window; `31` task-mutation events touching `27` tasks | Includes the same-day `0.2.0` lockfile git-commit refresh at 10:41 CET, but no semantic version change |
| `mesh 0.2.1` | 2026-03-06 19:00 CET onward | About `561` inbox messages | About `89` task JSON files with mtimes in this window; `83` task-mutation events touching `81` tasks | Most recent and most useful slice for follow-on analysis. This is also the currently embedded and installed version |

## Range Coverage

Observed communication span for the active team:

- Inbox messages: `2026-03-05T11:56:37.647Z` through `2026-03-07T12:00:17.825Z`
- Task mutation stream: `2026-03-06T02:14:01.713Z` through `2026-03-07T11:59:55.442Z`
- Task files present: `423` files, numeric IDs `84` through `509`

## Interpretation

- The active `taurhaus-team` data spans at least two semantic mesh versions and a meaningful pre-lock period.
- `mesh 0.2.1` is the best slice for analysis because it contains heavy recent activity under the version that is both embedded in the repo and installed locally.
- `mesh 0.2.0` is a shorter transitional slice on 2026-03-06.
- Pre-2026-03-06 communication should be treated as "pre-bundled/unknown" rather than force-labeled, because the repo did not yet track mesh version locks.

## Caveats

- Task JSON files do not include explicit created-at timestamps, so task volume is approximate and based on file mtimes plus the mutation audit stream.
- The mutation audit stream starts later than the oldest task file mtimes, so older task activity is undercounted if measured only from `task_mutations.jsonl`.
- All semantic-version boundaries above come from repo commit history, not from a separate deployment log.
