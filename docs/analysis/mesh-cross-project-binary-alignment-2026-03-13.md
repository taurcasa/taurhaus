# Mesh Cross-Project Binary Alignment

**Date:** 2026-03-13
**Task:** #1262

## Verdict

Cross-project shell alignment is now **fixed**.

The installed Mesh CLI at `~/.local/bin/mesh` was upgraded from:

- version `0.2.10`
- commit `f127eaead49e57679873b817d089929b5f5706b3`

to:

- version `0.2.12`
- commit `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

After the upgrade, every active project shell checked in this audit resolved
`mesh` to the same installed binary and reported the same `0.2.12` version.

The feature gap that prompted this task is resolved at the shell level:

- `mesh --help` now includes `broadcast`
- `mesh task review --help` and `mesh task complete --help` now include
  `--as-lead` and `--admin-reason`
- `mesh task assign --help` now includes `--admin-reason`

## Scope Checked

Active project environments checked:

- `/home/user/projects/taurhaus`
- `/home/user/projects/mesh`
- `/home/user/projects/taursec`
- `/home/user/projects/taurcraft`

In all four environments:

- `which mesh` -> `/home/user/.local/bin/mesh`
- `mesh version --json` -> version `0.2.12`, commit `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

## Before State

Before the upgrade, the installed Mesh CLI was:

```json
{
  "version": "0.2.10",
  "git_commit": "f127eaead49e57679873b817d089929b5f5706b3",
  "git_dirty": false,
  "build_time_utc": "2026-03-12T21:47:33Z",
  "protocol_version": 1,
  "schema_version": 1
}
```

That older CLI lacked the user-facing surfaces reported in the field:

- no `broadcast` command in top-level help
- no lead-repair flags in task review/complete help

The same stale binary was being resolved from all active project shells.

## After State

The installed Mesh CLI now reports:

```json
{
  "version": "0.2.12",
  "git_commit": "fabb518681d6f4336e715ae2a22ed2f3166b4db9",
  "git_dirty": false,
  "build_time_utc": "2026-03-13T14:01:05Z",
  "protocol_version": 1,
  "schema_version": 1
}
```

Installed executable identity after the upgrade:

- path: `/home/user/.local/bin/mesh`
- device/inode: `2096:263862`
- mtime: `2026-03-13 15:01:22 +0100`

Verified CLI surfaces after the upgrade:

- top-level help includes `broadcast`
- `mesh task review --help` includes `--as-lead` and `--admin-reason`
- `mesh task complete --help` includes `--as-lead` and `--admin-reason`
- `mesh task assign --help` includes `--admin-reason`

## Live Process Drift Check

Shell alignment is clean, but live daemon alignment is **not yet fully clean**.

At the time of this audit:

- installed binary identity: `2096:263862`
- running Mesh daemon processes inspected: `12`
- running processes already on the new inode: `2`
- running processes still on the old inode `2096:254283`: `10`

Aligned to the new binary:

- `taurhealth-team` member daemons:
  - PID `4148013`
  - PID `4148993`

Still running the prior binary identity:

- `taurhaus-team` member daemons:
  - PID `863100`
  - PID `865500`
  - PID `865562`
  - PID `865639`
  - PID `865745`
  - PID `865810`
- `taurmuse-team` member daemons:
  - PID `864841`
  - PID `864928`
- team daemons:
  - PID `3836505` (`taurmuse-team`)
  - PID `3842772` (`taurhaus-team`)

This means the upgrade fixed all newly launched shell usage immediately, but
already-running Mesh daemons for `taurhaus-team` and `taurmuse-team` still need
to be restarted or rotated before live daemon behavior fully matches the
installed `0.2.12` CLI.

## Operational Conclusion

The active project shell environments are now aligned on Mesh `0.2.12`, which
resolves the reported missing `broadcast` and lead-admin CLI surfaces for new
commands launched from those shells.

However, the live environment is only partially rotated:

- shell resolution: aligned
- installed CLI: aligned
- running daemons: partially stale

Any follow-up should focus on daemon rotation for the old `2096:254283`
processes so all live teams converge on the same executable identity.
