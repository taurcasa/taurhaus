# Mesh Live Daemon Version Alignment

**Date:** 2026-03-13
**Task:** #1260

## Verdict

Live Mesh daemon version alignment is currently **clean**.

At the time of inspection:

- installed Mesh binary: `~/.local/bin/mesh`
- installed Mesh version: `0.2.10`
- installed Mesh commit: `f127eaead49e57679873b817d089929b5f5706b3`
- installed executable identity: device `830`, inode `254283`
- running Mesh daemon processes inspected: `19`
- mismatches against the installed executable identity: `0`

Every running Mesh member daemon and every running Mesh team daemon is currently
executing the same installed Mesh binary identity.

## Installed Binary Identity

Installed Mesh contract from `~/.local/bin/mesh version --json`:

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

Installed executable identity:

- path: `/home/mstie/.local/bin/mesh`
- device: `830`
- inode: `254283`
- size: `4419608`
- mtime: `2026-03-12 22:47:54 +0100`

## Running Process Inventory

Running Mesh daemon processes found:

- `16` member daemons (`mesh daemon --pane ...`)
- `3` team daemons (`mesh team-daemon start ...`)
- active teams represented: `taurhaus-team`, `taurmuse-team`, `taurhealth-team`

Per-team summary:

| Team | Member daemons | Team daemon | Alignment |
|------|----------------|-------------|-----------|
| `taurhaus-team` | 8 | 1 | all aligned |
| `taurmuse-team` | 4 | 1 | all aligned |
| `taurhealth-team` | 4 | 1 | all aligned |

## Per-Process Alignment

Every inspected daemon process resolved to:

- executable path: `/home/mstie/.local/bin/mesh`
- executable identity: device `830`, inode `254283`

That exactly matches the installed Mesh CLI identity for all running daemons.

Detailed inventory:

| PID | Type | Team | Name | Start time | Executable | Inode | Aligned |
|-----|------|------|------|------------|------------|-------|---------|
| `863100` | member | `taurhaus-team` | `dev-2` | `Thu Mar 12 22:49:09` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `864569` | member | `taurmuse-team` | `dev-1` | `Thu Mar 12 22:49:51` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `864841` | member | `taurmuse-team` | `dev-2` | `Thu Mar 12 22:49:53` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `864928` | member | `taurmuse-team` | `team-lead` | `Thu Mar 12 22:49:55` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865500` | member | `taurhaus-team` | `architect-1` | `Thu Mar 12 22:50:33` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865562` | member | `taurhaus-team` | `code-quality-auditor` | `Thu Mar 12 22:50:35` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865639` | member | `taurhaus-team` | `dev-1` | `Thu Mar 12 22:50:38` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865745` | member | `taurhaus-team` | `dev-3` | `Thu Mar 12 22:50:41` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865810` | member | `taurhaus-team` | `mesh-architect` | `Thu Mar 12 22:50:43` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865906` | member | `taurhaus-team` | `security-auditor` | `Thu Mar 12 22:50:46` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `865975` | member | `taurhaus-team` | `team-lead` | `Thu Mar 12 22:50:48` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3187839` | member | `taurhealth-team` | `team-lead` | `Fri Mar 13 10:54:58` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3188311` | member | `taurhealth-team` | `dev-1` | `Fri Mar 13 10:54:59` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3188915` | member | `taurhealth-team` | `dev-2` | `Fri Mar 13 10:55:00` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3189352` | member | `taurhealth-team` | `architect-1` | `Fri Mar 13 10:55:02` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3836505` | team | `taurmuse-team` | `team-lead` | `Fri Mar 13 12:35:46` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3842772` | team | `taurhaus-team` | `team-lead` | `Fri Mar 13 12:36:52` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3844760` | team | `taurhealth-team` | `team-lead` | `Fri Mar 13 12:37:13` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |
| `3845447` | member | `taurmuse-team` | `architect-1` | `Fri Mar 13 12:37:15` | `/home/mstie/.local/bin/mesh` | `830:254283` | yes |

## Interpretation

Two things matter here:

1. **Path alignment is clean.** No running daemon was still executing an older
   renamed binary, deleted inode, or alternate install path.
2. **Time alignment is also clean.** The installed Mesh build time is
   `2026-03-12T21:47:33Z`, and all running daemons were started after that build
   was produced. That is consistent with a successful post-upgrade rotation.

This is exactly the healthy state Taurhaus's drift detection is meant to reach:

- no member daemon binary drift
- no team daemon binary drift
- one installed Mesh identity across all running teams

## Operational Conclusion

No immediate live Mesh daemon version-alignment intervention is needed right
now.

If this exact check is repeated after a future Mesh rollout, the failure modes
to watch for are:

- `/proc/<pid>/exe` resolving to a deleted binary or a path other than
  `~/.local/bin/mesh`
- device or inode mismatch versus the installed Mesh binary
- daemons whose start times predate the installed Mesh build or install event
- teams with member daemons rotated but team daemon still stale, or vice versa

None of those conditions were present in the current live audit.
