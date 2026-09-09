# Mesh telemetry and wait conformance

These Linux regression tests generate their inputs with the lock-matching Mesh
binary (`src-tauri/resources/mesh.lock.json`), never handwritten task, ruling,
monitor or workflow records. Install that binary at `~/.local/bin/mesh`, or set
`MESH_CONTRACT_BIN` to its absolute path. Python 3 and `cc` are also required.
`just test-mesh-contracts` is the explicit Linux operator gate for changes to
these readers or fixtures. Missing prerequisites or a lock mismatch fail there.
The four `mesh_binary_` tests carry `#[ignore]` with a prerequisite message;
the operator recipe explicitly selects them with `--ignored`.
The default `just test-rust-unit` recipe prints a NOT RUN notice; the integration
recipe does not select these lib tests either. Neither CI lane nor an unfiltered
`cargo test` needs a host Mesh installation.

Run from the checkout root:

```sh
just test-mesh-contracts
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
Monitor cycles wait up to 30 seconds for the producer's completed-cycle log;
the final cycle must persist `launch_health` in task metadata. The exact final
record sequence remains asserted. Fake-clock wait regressions run in the
default unit lane (or `python3 scripts/mesh-contract-fixture.py --self-test`),
without starting any CLI.

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

## M3 archive reconciliation — oversize value encoding

**M3-WAVE2-31:** The orchestrator's source-verified finding in
[the follow-up brief](../design/m3-oversize-encoding-brief.md) identifies task
#31's sequence-3 ruling as `field: oversize_diff, value: recorded`: a working-diff
breach recorded by the lead, rather than a failed shipping candidate. The old
`is_oversize_failure` predicate matched only `failed`, silently omitting this
encoding. The earlier report-cut repair remains independently covered; it was
not the demonstrated cause of #31's zero count.

The shared predicate now recognizes every `oversize_diff` ruling regardless of
value and excludes all of them from review acceptance. The `oversize_diffs`
column totals attributed rulings. An `Oversize rulings: N total` section prints
one `category | value | count` line per value seen: `failed` means a
shipped-candidate breach, `recorded` a working-diff breach, and `other` preserves
the supplied spelling. Values are JSON-encoded to keep free text on one line;
missing values render as `<missing>`, and non-string values remain visible in
`other`. These counts retain owner-only, ruling-time launch attribution;
ownerless rulings and owners without launch telemetry remain excluded.

The locked-binary fixture emits both `--value failed` and `--value recorded`,
followed by an independently countable `budget_raised` ruling. It proves both
encodings are counted with recent and old launches. No live wave state or
plan-ledger row is read or modified by these tests.

## Runtime attachment and terminal exclusion (v1.1)

Deploy the Taurhaus app and matching daemon that implement `terminalContract: 1`
before opting a team into Mesh team-owned delivery; activation also requires the
paired Mesh contract and its locked stage-2b-or-later binary. Taurhaus skips member
daemon launch, liveness repair and inbox wake for `delivery_owner: "team"`.
Claude's native mailbox remains its delivery path. Launch/resume sends,
stop/interrupt, effort teardown and compaction-test injection share the permanent
`teams/<team>/state/terminal/<member>.lock` flock. Acquisition waits at most 2 s;
terminal children inherit the fd and share a 10 s hold deadline. Contention defers
without clearing input; unavailable flock support refuses terminal I/O and reports
`coordination.terminal.unavailable`. The adjacent `<member>.holder.json` is only
`{owner, op, epoch, since}` diagnostics, replaced by the next acquirer and cleared
on release. Recovery cards and deadline nudges use the shared inbox delivery
seam (journal acceptance on canonical teams) and take no terminal lock. Unit and
contract tests use scratch roots and fake terminal transports; the compaction
transport check is `python3 scripts/test_runtime_exclusion.py`.

A legacy member remains `terminalContract: 0` until a launch atomically publishes
all attachment facts, including Linux process-start ticks. A heartbeat cannot
certify it, and its historical tmux epoch value is not a ticks mismatch. Missing
attachment inventory defers stop/interrupt; unrelated corrupt records do not
block a member that was resolved. The Windows app defers managed terminal writes
to the native daemon without creating lock or holder state on its UNC mount.
Unsupported flock is reported once per path and never permits unlocked I/O.
Install the matching app and daemon together before either writes these records:
old apps cannot decode the new decimal-string `paneStartTime`. The mandated
unchanged protocol number does not guard against that mixed-version deployment.

Member terminal operations and scanner inventory probes use explicit `-S`.
Session bootstrap and emulator attachment helpers still use tmux's ambient
socket resolution (`TMUX`, otherwise `TMUX_TMPDIR` and the effective uid).
Those session-level helpers do not consume a member's recorded socket; custom
bootstrap/emulator socket selection remains outside this member-write change.

### Canonical journal producer

For teams whose config carries `messaging_format: 2`, the Taurhaus daemon
(and its existing native compaction bridge) is a journal producer, not an
inbox writer. The shared delivery seam submits once through `mesh journal
accept --producer taurhaus-daemon`, with the resolved team's `--claude-dir`,
recipient, task links and a delivery-derived idempotency key. Mesh derives the
current assignment from each task; snapshot assignment tokens are not delivery
preconditions. The authenticated actor is the explicit sender or team lead,
while the producer remains `taurhaus-daemon`. Successful capability probes
are cached per binary invocation and resolved root.
Mesh assigns `origin: generated`; acceptance is independent of projection,
transport, read and uptake. `onboarding.delivery.observed` retains the local
card `delivery_id` and the returned `journal.message_id` and
`journal.delivery_id`; compaction bookkeeping retains the same journal receipt.
An unavailable, incompatible, refusing or timed-out Mesh emits
`coordination.journal.accept_failed` with team, recipient, idempotency key and
reason (including recognized Mesh error codes, never CLI prose or credentials).
Canonical delivery never falls back to an array write or resends after a
submission with an uncertain outcome. A preflight or executable-spawn failure
records `Failed` on the matching recovery claim; the existing single bounded
retry retains the same delivery identity. A deadline preflight or executable-spawn
failure releases the nudge claim for the next pass; it does not emit an
unconfirmed-submission event. A cached binary that rejects `journal` at argument
parsing is also a pre-submission failure. A spent deadline nudge with no
confirmed acceptance emits `deadline.nudge.unconfirmed` with team, member,
task and deadline fields. Non-canonical teams keep direct append.
This adapter adds no protocol version, terminal writer, delivery owner or
activation permission; the existing runtime-exclusion contract still applies.

## Stage-5b owned-host evidence — not an eligibility packet

The fake executable speaks newline-delimited JSON-RPC without `jsonrpc` on a
private Unix socket in a tempdir. Tests cover launch/account/permission rendering,
atomic complete runtime publication, named resume, foreign-field preservation,
root/attachment revalidation, lost-response suppression, liveness/effort relaunch,
owned shutdown and daemon/UI transcript-input round trips. A fake flock holder
proves deferral and stable-inode reuse at `teams/TEAM/state/app-server/MEMBER.lock`.
The same exclusion covers startup, UI/approval/interrupt, effort relaunch,
compaction output, attachment publication and shutdown. Team/runtime locks are
short snapshots only; no data lock or model-turn wait spans native RPC I/O.
Acquisition is capped at two seconds, submission at five seconds, frames at
64 KiB and events processed per response at 128. Closing never unlinks the lock.

Startup and next-turn recovery use the existing `onboarding.delivery.observed`
receipt path (`app_server`). `Submitted` requires the correlated native response;
it proves neither consumption nor task acceptance. `hostInputUnknown` persists
before possible input and remains true after ambiguous loss, blocking replay.
The compaction-hook test probes exclusion during actual CLI stdout; the fallback
test consumes existing pending compaction at its current context generation.
The detector and compaction ledger retain their existing semantics.

These are fake software proofs only. Pane conversion tests assert refusal pending the paired recoverable-relaunch
contract. Successful switching and rollback remain unimplemented.
The unresolved Unix framing mismatch and Mesh switch dependency are recorded in
[the harness model](../architecture/harness-model.md#owned-codex-hosting-stage-5b-disabled-pending-pairing).
No real harness, live daemon, operator tmux server, account credentials or paid
trial is used. All native descriptors remain disabled.

### Lane verification (2026-09-09)

All gates ran from the hosting checkout with its own target directory. Each gate
polled machine-wide Cargo activity before starting. Harness homes, app data and
TMUX_TMPDIR were disposable; PATH guards refused ambient harness/tmux execution.
No real Codex run, install, deployment, paid packet or descriptor activation ran.

| Exact gate | Exit | Evidence |
|---|---:|---|
| `just check-quick` | 0 | Rust test compilation, frontend typecheck, 2,463 frontend tests |
| `just lint` | 0 | Clippy, frontend dependency/structure and repository script checks |
| `just test-contracts` | 0 | 15 renderer, 20 harness and 31 module-boundary tests |
| `just test-rust-unit` | 101 | 2,598 passed, 5 failed, 9 ignored, 100 filtered |

The five unit failures require real scratch tmux and were refused by the guard:
`coordination::runtime::tmux::tests::{resume_add_nine_same_project_members_share_one_window,resume_add_other_policies_do_not_tile,resume_add_tiling_failure_removes_only_the_new_pane}`
and `session_scanner::control::tests::{nine_same_project_members_share_one_window,scratch_tmux_resolves_binary_from_path}`.
All 18 new Rust hosting/exclusion tests and three frontend hosted-control tests
passed. Red was observed for the new seams and for attachment identity, member UI
reuse, startup readiness, unreviewed builds, descriptor parity and declared module
boundaries before their respective fixes. No render-role-section goldens changed.
A visual cross-family review and successful paired switch/rollback proof remain
outstanding; this evidence is not a completion or eligibility declaration.
