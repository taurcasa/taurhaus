# Mesh telemetry and wait conformance

These Linux regression tests generate their inputs with the lock-matching Mesh
binary (`src-tauri/resources/mesh.lock.json`), never handwritten task, ruling,
monitor or workflow records. Install that binary at `~/.local/bin/mesh`, or set
`MESH_CONTRACT_BIN` to its absolute path. Python 3 and `cc` are also required.
Missing prerequisites or a lock mismatch fail explicitly, including in CI.

Run from the checkout root:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib mesh_binary_ -- --test-threads=1
just test-rust-unit
just check-quick
just lint
just test-contracts
```

`scripts/mesh-contract-fixture.py` creates only temporary teams with an explicit
`--claude-dir`, empty child environment and scratch HOME. No AI harness or tmux
client runs. The monitor fixture advances only Mesh's realtime clock with a
scratch shared library, leaving host time and monotonic polling unchanged. Each
foreground monitor is terminated and reaped in `finally`. No generated inputs
are checked in. Taurhaus launch/snapshot fixtures remain Taurhaus-produced.

## Reader decisions

- Monitor metadata and workflow echoes are joined by originating message ID.
  Two ignored nudges count as two `monitor_nudges`; the subsequent launch-health
  record counts separately in the report footer, attributed to the affected
  seat rather than the lead who receives the health notice. A leadless health
  episode uses the originating ignored-nudge IDs. Existing sidecar events keep
  their prior meanings; legacy sidecars carry no identity for a cross-source
  join. The new adapter does not write sidecars.
- Report membership remains task-based: a recent sidecar event, monitor record
  or ledger ruling selects the task, then its launch history supplies role/model
  attribution. This preserves existing task-lifetime metric semantics. A recent
  ruling must not disappear solely because the launch predates the report cut.
  Missing launch attribution remains excluded and disclosed in the footer.
- Mesh GO markers are assignment tokens, with boolean backward compatibility.
  Reason-bearing member blocks do not expire with the activity TTL. See
  [the wait contract](../architecture/data-architecture.md#declared-waits-at-the-taurhaus-deadline-boundary).

## M3 archive reconciliation — open check

The locked binary serializes `--kind ruling --field oversize_diff --value failed`
with those exact `kind`, `field`, and `value` strings, plus `seq`, `by`, and `at`.
The existing scanner recognizes it and excludes it from review acceptance. A
subsequent `budget_raised` ruling remains independently countable. The binary
fixture proves owner attribution and the old-launch report-cut repair.

**M3-WAVE2-31:** The exact reason task #31 displayed zero remains unverified.
Resolve only from a supplied closed wave export: ruling sequence 3 and its
amendments, exact fields and timestamps, owner/assignment history, launch
sidecars, included team roots and report cut. Compare raw, recognized,
attributed and excluded records. The report-cut fix is demonstrated by the
scratch fixture; it is not claimed as #31's historical cause. No live wave state
or plan-ledger row was read or modified for this lane.
