# Mesh Version Drift Explanation

**Date:** 2026-03-13
**Task:** #1263

## Executive Summary

Taurhaus pinning was correct while the live Mesh CLI on PATH was stale.

That mismatch is possible because these are separate layers:

1. Taurhaus pin files declare which Mesh build should be bundled with the app.
2. Taurhaus build and bundle flows copy that Mesh binary into app resources.
3. Everyday shell usage in project directories resolves `mesh` from
   `~/.local/bin/mesh`, which is a shared per-user install, not a
   project-scoped binary.
4. Already-running Mesh daemons keep their original executable inode until they
   are restarted or repaired.

So Taurhaus can be pinned to Mesh `0.2.12` while terminals, tmux panes, and
already-running daemons still behave like Mesh `0.2.10`.

## Exact Mismatch Path

### 1. Taurhaus pinning was already updated

Taurhaus currently pins Mesh `0.2.12` / commit
`fabb518681d6f4336e715ae2a22ed2f3166b4db9` in:

- `src-tauri/resources/mesh.lock.json`
- `src-tauri/resources/mesh.version`
- `src-tauri/resources/mesh.manifest.json`

Those files define the compatibility contract Taurhaus expects.

### 2. Taurhaus pinning does not update PATH by itself

Taurhaus pinning only affects:

- bundle-time resource selection
- app-side compatibility checks against the bundled manifest
- installer flows that explicitly copy the bundled Mesh binary into the active
  environment

It does **not** automatically replace the user's shared installed CLI at
`~/.local/bin/mesh`.

That replacement only happens when an explicit install path runs, such as:

- Taurhaus IPC `install_mesh`
- `just install-mesh`

Until one of those runs, shell users still see whatever binary was previously
installed in `~/.local/bin/mesh`.

### 3. Live project shells all shared the same stale install

The reported issue first appeared as a project-local symptom, but it was not
actually project-local.

Checked project environments:

- `/home/mstie/projects/taurhaus`
- `/home/mstie/projects/mesh`
- `/home/mstie/projects/taursec`
- `/home/mstie/projects/taurcraft`

Before the fix, all four resolved:

- `which mesh` -> `/home/mstie/.local/bin/mesh`
- `mesh version --json` -> `0.2.10`
- commit `f127eaead49e57679873b817d089929b5f5706b3`

That means the "project that reported Mesh 0.2.10" was only the first place the
shared PATH-level drift was noticed. The underlying problem affected every
active project shell for this user.

### 4. Missing features followed the installed PATH binary

The observed missing features came directly from the stale installed CLI:

- `mesh --help` did not include `broadcast`
- task help did not expose the newer lead-repair/admin flags

After replacing `~/.local/bin/mesh` with Mesh `0.2.12`, those features became
available immediately in all checked project shells.

This confirms the feature gap was caused by the shared installed CLI version,
not by Taurhaus pin files being wrong.

### 5. Running daemons are a fourth layer

Even after `~/.local/bin/mesh` was upgraded, most already-running Mesh daemons
were still executing the previous binary inode.

At the time of verification:

- installed Mesh binary identity: `2096:263862`
- stale running daemon identity: `2096:254283`
- running daemons inspected: `12`
- already aligned to the new binary: `2`
- still on the old binary: `10`

So there are really two different kinds of drift:

- **shell drift**: fixed as soon as `~/.local/bin/mesh` is replaced
- **live daemon drift**: persists until running daemons are restarted or
  self-healed

## Why Taurhaus Pinning Did Not Prevent The Drift

Taurhaus pinning is a compatibility contract, not an always-on system package
manager.

What it guarantees:

- Taurhaus knows which Mesh version it expects
- Taurhaus can compare installed Mesh against the bundled contract
- Taurhaus can install the bundled Mesh into the active environment when asked

What it does not guarantee by itself:

- every shell has already run the installer
- `~/.local/bin/mesh` has been refreshed recently
- long-lived Mesh daemons have already rotated onto the new binary

That is why all of the following can be true at the same time:

- Taurhaus source tree is pinned to `0.2.12`
- Taurhaus bundle resources contain `0.2.12`
- `~/.local/bin/mesh` is still `0.2.10`
- active tmux Mesh daemons are still running the old executable identity

## Project-Specific Usage Pattern Explanation

There is no separate Mesh install per project here.

For the active Linux-side projects we checked, the usage patterns split like
this:

- Taurhaus bundle resources: project-specific and pin-controlled
- shell `mesh` commands: shared per-user install in `~/.local/bin/mesh`
- Mesh member/team daemons: launched from that shared installed CLI and kept
  alive independently of the source tree

So a user working in `taurhaus`, `mesh`, `taursec`, or `taurcraft` is still
hitting the same installed Mesh binary unless they deliberately override PATH.

## Root Cause

The exact root cause was:

1. Taurhaus had already been repinned and rebundled to Mesh `0.2.12`.
2. The shared installed CLI at `~/.local/bin/mesh` had not yet been updated and
   remained on Mesh `0.2.10`.
3. All active project shells resolved `mesh` from that stale shared install.
4. Most already-running Mesh daemons were still attached to the older inode
   even after the CLI install was corrected.

## Operational Implication

When Mesh behavior changes, there are three separate rollout checkpoints:

1. update Taurhaus pin/bundle resources
2. update the installed shared CLI on PATH (`~/.local/bin/mesh`)
3. rotate running Mesh daemons so live sessions stop using the old inode

If step 1 happens without steps 2 and 3, Taurhaus looks correct in source and
bundle metadata while real project usage still behaves like the old Mesh
release.
