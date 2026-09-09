# Codex 0.153.4 integration trial — INCONCLUSIVE at step 1 setup

2026-09-09. **Eligibility remains disabled.** The prescribed canonical setup
stopped before member launch: the real Mesh command `team delivery --owner team`
refused the newly created team's active external lead because it had no runtime
record. This is a setup failure, **not an observed app-server or attached-TUI
failure**. No Codex model session was launched. Steps 2–7 were not run.

Pair: Taurhaus `9d3589355a11199ec2b3317bd1a13e784e64cd6f`, branch
`feat/integration-trial`, daemon protocol **26**; Mesh
`4388d6a1590e3072c9dfdc61ccd08b00bff2508b`, branch `feat/native-push`.
The temporary compiled Mesh change set only the 0.153.4 descriptor to
`disposition: "trial", enabled: true`; all other descriptor fields and builds
were unchanged. The edit was reverted after the failure. **No descriptor flip
commit exists. No Taurhaus registry entry was needed or added:** the existing
`HostedDescriptor::codex()` already names build 0.153.4 and Unix WebSockets.

S = observed runtime; I = source-based inference; U = unverified.
Retained text captures/logs omit trailing empty lines; the event JSONL preserves
full pane output including blank terminal rows.

| Brief step | Outcome | Evidence / missing observation |
| --- | --- | --- |
| 1. Daemon launches host and attached TUI | **INCONCLUSIVE: setup FAIL (S)** | Private daemon ping reports protocol 26; RPC launches the canonical setup script; Mesh creates the team, then refuses delivery-owner verification for missing `runtime/lead.json`. No member/thread/host attachment is published. |
| 2. Idle send → start, visible marker, receipt and explicit read | **NOT RUN (U)** | No thread, native request, receipt, journal consumption or marker reply. |
| 3. Active send → steer | **NOT RUN (U)** | No active turn or expectedTurnId. |
| 4. Operator/socket ordering and mutual exclusion | **NOT RUN (U)** | No host-operation holder exists; the retained `owner.lock` is a Mesh delivery-owner lock, not host-operation evidence. |
| 5. Compaction and recovery card | **NOT RUN (U)** | No compaction or recovery delivery. |
| 6. Daemon restart and retained identity | **NOT RUN (U)** | Fixture retry uses a new scratch root; it is not the restart experiment. |
| 7. Rollback and tmux delivery | **NOT RUN (U)** | No hosted member to roll back. Cleanup of the processes actually started passed separately. |

## Exact failure and identities

Authoritative runtime sidecars: [events](integration/pre-defect/events.jsonl),
[setup pane](integration/pre-defect/setup-pane-1.txt),
[daemon log](integration/pre-defect/daemon.log),
[team config](integration/pre-defect/team/config.json),
[setup commands](integration/pre-defect/setup-command.json).
The `pre-defect` directory name means that neither recorded product defect was
reached; it does not assert that a defect was reproduced.

Scratch root `/tmp/th-int-ssxg881f`, private port **26417**. Team `integration`,
lead `lead`, member agent ID `036a904f-5221-49e7-a756-82415a4e9ec1`, team incarnation
`4d7d1c2aeaa832a131697fc0e5cdd5ad971b25f98b3e27e90773fdc115d94e2b`.
The config's leadSessionId is Mesh bootstrap metadata, **not a Codex thread ID**.
Daemon request `88aaa715752816c7` returned:

```json
{"data_root":"/tmp/th-int-ssxg881f/data","protocol_version":26,"uptime_secs":0,"version":"0.9.7"}
```

Request `a9a6ba191db6a8e9`, method `launch_session`, ran the setup script in the
scratch project and returned pane `%1`, session `taurhaus`, window `project`.
The daemon RPC was used to execute the canonical Mesh CLI commands; the typed
`coordination.create_team` RPC only creates a noncanonical empty config and
does not accept the specified retention/canonical flags.

```sh
/tmp/th-int-ssxg881f/home/.local/bin/mesh --claude-dir /tmp/th-int-ssxg881f/claude --team integration --name lead team create --messaging-canonical --isolated --retention-policy /tmp/th-int-ssxg881f/policy.json
/tmp/th-int-ssxg881f/home/.local/bin/mesh --claude-dir /tmp/th-int-ssxg881f/claude --team integration --name lead team delivery --owner team
```

Pane capture, verbatim:

```text
bootstrapped new team config
[mesh] lead joined team integration
error: IO error: delivery: pending: runtime lead: IO error: No such file or directory (os error 2)
```

The setup shell used `set -eu` and never wrote its success sentinel. The
controller exited **1**. The daemon's launch RPC returned successfully before
the setup command completed, so it exposes no shell exit status; the exact Mesh
exit code was not separately captured. The config already contains
`delivery_owner: "team"` from canonical creation; that field does not prove the
following owner command passed or a dispatcher started. The epoch artifact is
retained under [state/delivery](integration/pre-defect/team/state/delivery/epoch.json).

I: Mesh `src/delivery/runtime.rs::compatible_team` reads a runtime record for
every active member and requires `terminalContract: 1`. The canonical creator
registers an active external lead but does not supply its runtime. This explains
the observed refusal; it does not prove that an appropriately launched roster
cannot work. No forged runtime record, alternate setup ordering, or additional
product repair was used to bypass the failed prerequisite.

No `mesh send` was executed, so there are **no wire start/steer excerpts,
native_enqueued receipts, consumed_by_read journal rows, host-operation holder
files or recovery cards to report**. Their absence is not a passing assertion.
The random marker reserved for this attempt was `5f19c4057b8d614a`. It was never
submitted to a model.

## Cost and isolation

| Attempt | Model turns / generations / compactions | Spend |
| --- | --- | --- |
| Initial private-tmux fixture attempt | 0 / 0 / 0 | $0.00 |
| Corrected private-server setup | 0 / 0 / 0 | $0.00 |
| Total against hard caps | **0 / 8 model turns** | **$0.00 / $3.00** |

Ledgers: [initial](integration/initial/cost-ledger.json),
[corrected setup](integration/pre-defect/cost-ledger.json). Only the pinned
binary's `--version` ran; neither member launch RPC nor paid input was reached.
There is no estimated bill or unreported compaction spend in this run.
Model/effort planned and generated in the scratch config: `gpt-5.6-luna`, `low`.

Both roots were created with mode 0700 beneath `/tmp`. The sole credential copy
was `auth.json`, mode 0600, into an initially empty CODEX_HOME. No source config,
sessions, hooks, history, MCP definitions or skills were copied. Scratch HOME,
all harness/data roots, TMPDIR and TMUX_TMPDIR were explicit; TMUX was absent.
The scratch project was a fresh git repository with this short AGENTS.md:

```text
This is an isolated transport trial. Reply briefly. Never execute tools or commands.
```

Bubblewrap hid `/home`, `/tmp` and `/run`, exposed only the writable scratch root,
and provided a private PID namespace. The native Codex executable and both built
binaries were copied into scratch `$HOME/.local/bin`; their SHA-256 identities
are in the event logs. The reference Node wrapper was replaced by a copy of its
native executable, verified as `codex-cli 0.153.4`, avoiding exposure of any
operator installation directory. Non-Codex harness executables were blocked.
No real operator tmux or port 17233 was contacted. No installation recipe ran.

Cleanup used the controller's finally block and only its own process groups;
ending the private PID namespace removed daemon/tmux descendants. The initial
root `/tmp/th-int-z4u32ik_` used port **30356**. Both ports refused connections,
both roots and credential copies were removed, and environment-token scans
found no survivors. PID/start-tick inventories and verification:
[initial](integration/initial/identities.json),
[corrected setup](integration/pre-defect/identities.json),
[initial cleanup](integration/initial/cleanup.json),
[corrected cleanup](integration/pre-defect/cleanup.json).

## Reproduction, gates and limits

[controller.py](integration/controller.py) reproduces the failed setup and
always cleans up. Archived controllers preserve the initial and corrected
runtime sequences; later controller edits only redact a credential hash from
evidence, express the policy source relative to the checkout's parent to avoid
an operator-home path, and record the generated setup script. They did not rerun
the trial. Archived scripts must be restored to `integration/controller.py` to
replay their original output-directory placement.
Run the controller from this checkout root with a new output label:

```sh
python3 docs/design/evidence/native-eligibility/integration/controller.py NEW_LABEL
```

Before reproduction, build both pinned binaries as in
[daemon build](integration/daemon-build-retry.json) and
[Mesh build](integration/mesh-build.json), applying only the authorized temporary
descriptor edit. Every build first polled `pgrep -af '(^|/)cargo( |$)'` until exit
1, with a 30-minute limit; both observed no competing Cargo. The daemon used
checkout-local `src-tauri/target`, Mesh used its own `target`. The first daemon
build exited 101 because resource placeholders were absent; `just
ensure-tauri-resources` exited 0 and the repeated `cargo build --bin
taurhaus-daemon` exited 0. Mesh `cargo build --bin mesh` exited 0. Revert with:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The required unpaid gate runner is [gates.py](integration/gates.py): separate
credential-free HOME/harness roots, rejecting CLI/tmux/Mesh shims, private PID
namespace, checkout-local target, and no real harness homes visible. Tool cache
paths in retained logs are redacted. `bun install --frozen-lockfile` exited 0.
Gate exits are recorded in `integration/gate-{check-quick,lint,test-contracts}.json`.

| Exact gate from checkout root | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust tests compile; typecheck passes; 150 frontend files / 2,469 tests pass. |
| `just lint` | **0** | Rust/frontend/workflow and gate-recipe guards pass. |
| `just test-contracts` | **0** | 68 Rust contracts pass (15 + 20 + 33). |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on an eligibility flip; none was made. |

The final PID/start-tick, port, descriptor and evidence-hygiene audit is retained
in [final-verification.json](integration/final-verification.json).

No product code changed, so no regression tests were added or red/green claimed;
`just test-rust-unit` is not required. The instruction-source refusal and
rollback pane leak remain untouched because member launch and rollback were
never exercised. Non-evidence inserted lines: **0 / 400**. No dependency changes,
ledger edits, registry additions or feature work.

Deviations: one credential-free tmux setup retry (the initial daemon-created
server disappeared before readiness); the corrected fixture explicitly retained
`tmux -D -f /dev/null`, matching the reference trial. Initial gate invocations
exited 127 because Node was missing inside their wrapper; the wrapper was
corrected with a scratch copy of Node and all exact gates rerun. No continuation
after the canonical-owner refusal. No numbered step was green, so the deliverable
is one evidence commit rather than seven step commits. The requested Opus evidence
lens was unavailable in this session's model/tool surface and was not replaced
with a claimed cross-family review. A new authorized trial must resolve the
canonical setup prerequisite before any eligibility conclusion.

## Attempt 2 — production initialization passes; step 1 setup remains INCONCLUSIVE

2026-09-10 (Europe/Berlin; runtime logs use 2026-09-09 23:14 UTC).
**No eligibility enabled. No app-server input was exercised.** Production
`coordination.initialize_team` successfully created the canonical team, launched
both seats, and then completed `opt_in_delivery`. The controller next set the
scratch member's `adapter_mode` to `app_server` and called
`coordination.resume_member` while that seat was online. The RPC refused at
`load_member`: **`Conflict: member 'seat' in team 'integration' is not offline`**.
Controller exit **1**; it stopped immediately and restored the descriptor.

This is a **controller/setup limitation, not evidence of an app-server launch
failure**. In particular, it does not establish that stop-then-resume would fail:
that sequence was not run after the first refusal. The live seat never reached
`HostedMembers::launch`, and neither recorded product defect was exercised.
The code's later `app_server_switch_requires_5b_recoverable_relaunch_packet`
guard is source evidence only, not this run's observed refusal.

Pair: Taurhaus **`0f1a6e90`**, based on canonical activation **`06d1267b`**
(PR #157), protocol **27**, branch `feat/integration-trial`; Mesh
**`4388d6a1590e3072c9dfdc61ccd08b00bff2508b`**, branch `feat/native-push`.
Both binaries were rebuilt from these worktrees with checkout-local targets.
The temporary compiled descriptor changed exactly the 0.153.4 disposition to
`trial` and enabled it, leaving host/configuration/trust and all other builds
unchanged. [Exact diff](integration/attempt2/mesh-trial-descriptor.diff).
The descriptor has been reverted; **Mesh flip commit: none**.

| Brief step | Attempt 2 outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon-owned app-server + attached strict TUI | **INCONCLUSIVE: setup refusal** | Production initialization passes all nine stages. Subsequent resume refuses the already-online member at `load_member`; the runtime has a regular pane, no `appServer`, and no discovered session/thread ID. |
| 2. Idle send → start, marker, receipt, explicit read | **NOT RUN** | No `mesh send`, `turn/start`, marker reply, `native_enqueued`, or explicit read. |
| 3. Active send → steer | **NOT RUN** | No active model turn or expectedTurnId. |
| 4. Operator/socket ordering + mutual exclusion | **NOT RUN** | No operator input or host-operation holder. Mesh owner.lock is not host-operation evidence. |
| 5. Compaction + recovery card | **NOT RUN** | Startup inbox acceptance exists; it does not prove compaction recovery or model uptake. |
| 6. Daemon restart | **NOT RUN** | No restart or persistent hosted identity. |
| 7. Rollback + tmux delivery | **NOT RUN** | No hosted attachment to roll back. Separate cleanup of the started processes passed. |

### Runtime sequence and identities

[Executed controller](integration/attempt2-controller.py),
[event JSONL](integration/attempt2/run/events.jsonl),
[initialize result](integration/attempt2/run/initialize-result.json),
[resume result](integration/attempt2/run/resume-result.json),
[daemon log](integration/attempt2/run/daemon.log),
[structured log](integration/attempt2/run/taurhaus.log.jsonl).

Scratch root **`/tmp/th-int-ik66igdt`**, private port **31164**, team
`integration`, lead `lead`, member `seat`. Scratch project is a fresh committed
git repository containing the short `AGENTS.md` shown in attempt 1; creation
commands are `events.jsonl:5–10`. No operator repository content was copied.
Random reserved marker **`990084b281d9d0ea`** was never submitted.

The exact `coordination.initialize_team` request is `events.jsonl:18`, request
ID **`66967cbcee57b97d`**. Its messaging object was parsed directly from
`DEFAULT_CANONICAL_POLICY` in `src/lib/components/meshTabUtils.js`, with no policy
substitution; [retained policy](integration/attempt2/run/policy.json).
Lead: Claude `claude-haiku-4-5` on credential-free scratch CLAUDE_CONFIG_DIR.
Member: Codex **0.153.4**, **gpt-5.6-luna**, effort **low**, read-only sandbox,
never approvals. Binary SHA-256 identities are `events.jsonl:1–4`.

Initialization run **`init_ea40f09e6e724233a3e98cd30046b847`** returned:

```json
{"failed_step":null,"message":"team initialized","succeeded_steps":["validate_configuration","create_team","add_lead","create_panes","launch_sessions","join_mesh","start_daemons","opt_in_delivery","send_onboarding"]}
```

The lead reached its first-run theme screen, not a model turn:
[lead capture](integration/attempt2/run/initialize-pane-1.txt).
The Codex pane showed v0.153.4 with `model: loading` and an empty input:
[member capture](integration/attempt2/run/initialize-pane-2.txt).
The production rendered commands are `taurhaus.log.jsonl:12–13`.
Both runtimes have `terminalContract: 1`, attachment generation 1,
context generation `"0"`, and null `session_id`; neither has `appServer`.
Member pane **`%2`**, pane PID **134** inside the private PID namespace,
start ticks **21621355**; lead pane **`%1`**, PID **115**, start ticks
**21621350**. These namespace PIDs are distinct from the host PID inventory.
[Member runtime](integration/attempt2/run/team/runtime/seat.json),
[lead runtime](integration/attempt2/run/team/runtime/lead.json).

Team incarnation
**`c7ac345bdeb68789a58744da96d42a955d16f8ab2bcc6e2eb5ffbc83e27d4637`**;
member incarnation **`c836b70c-b762-4c0e-b31c-fd1b37b2a950`**.
The config's bootstrap leadSessionId is not a Codex thread ID.

After the scratch config opt-in (`events.jsonl:30`), request
**`7d1b699ded323c7e`** called `coordination.resume_member`; run
**`resume_e48c1ef9a1e949cfb4ae14e6548d380c`** returned:

```json
{"failed_step":"load_member","resumed":false,"pane_id":null,"message":"Conflict: member 'seat' in team 'integration' is not offline"}
```

`events.jsonl:42` records the stop. There was no retry, pane identity deletion,
runtime forgery, or continued numbered step. AgentDefinition has no adapter-mode
field and `member_from_agent_setup` initializes `extra` empty; therefore the
production initialize request itself did not opt this seat into hosting. The
subsequent config-plus-resume attempt did not satisfy the launch prerequisites.

### Receipts, journal references and spend

The two startup cards reached **inbox acceptance only**. For `seat`, structured
log line 27 and its runtime record retain this journal reference:

```json
{"message_id":"8940e5ae-bb88-4d91-b705-2b49d67aa484","delivery_id":"918588c2-009d-4b3d-9205-16b797cab5bf","sequence":2,"projection":"pending"}
```

Associated observation: `stage: accepted`, `path: inbox`, `accepted_bytes: 2273`,
`offered_bytes: 0`, `returned_by_read_bytes: 0`. The member dispatcher health
records **`activity not freshly idle`**, zero completed deliveries and zero
failures. These are runtime/journal-reference excerpts, **not exported original
canonical journal rows**: the inherited snapshot glob did not retain those rows.
No `consumed_by_read` or native enqueue is claimed. No app-server wire frames or
host-operation holder files exist in this attempt's evidence.

| Spend item | Model turns / generations / compactions | USD |
| --- | --- | --- |
| Scratch Claude first-run screen | 0 / 0 / 0 | $0.00 |
| Scratch Codex launch, still loading; onboarding pending | 0 observed / 0 observed / 0 | $0.00 model spend observed |
| Controller input or compaction | 0 / 0 / 0 | $0.00 |
| Attempt 2 total versus caps | **0 / 8 observed model turns** | **$0.00 / $3.00 observed** |

[Cost ledger](integration/attempt2/run/cost-ledger.json),
[usage-event export](integration/attempt2/run/usage-events.json).
No work prompt, TUI Enter, native input, or compaction was requested. The usage
export is empty, session IDs remain null, and delivery remained pending before
teardown. No vendor billing statement was queried; $0 is the observed no-turn
result, not a claimed subscription invoice measurement. There were no paid
reruns. Attempt 1 plus attempt 2 remain at zero observed model turns.

### Isolation, cleanup and reproduction

Scratch root mode 0700; CODEX_HOME started with only copied `auth.json` mode
0600 and then a generated minimal config. HOME and all harness/data roots were
scratch, inherited TMUX was absent, TMUX_TMPDIR pointed to the private server.
Bubblewrap hid `/home`, `/tmp`, `/run`, exposed only the writable scratch root,
and put the real daemon, Mesh, Claude, Codex and tmux in a disposable PID
namespace. All executables were copied into scratch `$HOME/.local/bin` and no
operator installation directory was exposed to the child namespace.

The controller's finally block stopped and waited its own process groups,
ending the descendant PID namespace. The ten recorded host PID/start-tick
identities are in [identities.json](integration/attempt2/run/identities.json).
[Cleanup](integration/attempt2/run/cleanup.json): no surviving scratch processes,
private port closed, root and auth copy removed. Descriptor restoration command
exited **0**, and the Mesh worktree is clean. The native trial binary remained
only as an unused build artifact in the authorized Mesh target; its scratch
executable copy was deleted. No standing daemon, server or team was contacted.

Exact reproduction of this **failed setup controller**, not a passing seven-step
trial, from the specified checkout root:

```sh
python3 docs/design/evidence/native-eligibility/integration/attempt2-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt2-controller.py attempt2/NEW_RUN
python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

Do not treat the inspection checkpoint in the controller as seven-step coverage:
that branch was not reached. A future commissioned run must resolve the hosted
activation route before submitting model work.

Both builds first ran `pgrep -af '(^|/)cargo( |$)'`; each returned **1**, no Cargo
running. Daemon `cargo build --bin taurhaus-daemon` and Mesh `cargo build --bin
mesh` each exited **0**, using `src-tauri/target` and Mesh `target` respectively.
[Daemon build](integration/attempt2/daemon-build.json),
[Mesh build](integration/attempt2/mesh-build.json).
No install recipe or forbidden daemon port was used.

### Attempt 2 gates and deviations

All exact gates ran from this checkout root in the credential-free
[gate controller](integration/attempt2-gates.py): private HOME, harness roots,
PID namespace, rejecting CLI/tmux/Mesh shims and checkout-local Rust target.
Each Cargo preflight returned 1 (no competing Cargo).

| Exact gate | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, frontend typecheck; 150 frontend files / 2,486 tests passed. |
| `just lint` | **0** | Rust, frontend, workflow and recipe guards passed. |
| `just test-contracts` | **0** | All 68 Rust contract tests passed. |
| `just test-rust-unit` | N/A | No `src-tauri/` changes. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on a passing flip; no flip was made. |

Gate logs and exit metadata are under [attempt2/gates](integration/attempt2/gates).
[Final verification](integration/attempt2/final-verification.json) checks retained
identities, port/socket/root cleanup, descriptor restoration, evidence hygiene,
and the production initialize ordering.

No product code changed, no new dependency or registry entry was needed, and
non-evidence inserted lines are **0 / 400**. Neither product defect was reached,
so neither was fixed and no regression red/green is claimed. The existing
`HostedDescriptor::codex()` already covers the pinned build and transport.

Deviations and limits:

- The controller used scratch config opt-in followed by resume on a live pane.
  That invalid activation sequence stopped before hosting; it is not a product
  regression or a conclusive native eligibility test. No additional activation
  repair was authorized within the two-defect product scope.
- Original canonical journal rows were not exported by the inherited glob;
  retained runtime and logging records contain journal references and inbox
  acceptance observations only. There is no wire/native-receipt evidence.
- There was no completed numbered trial step, so this is one evidence commit,
  not seven green-step commits. Attempt 1 remains intact above.
- An Opus evidence lens was unavailable in the exposed model/tool surface;
  no cross-family review is claimed. Final verification is a local artifact
  audit, not that independent lens.
- No continuation after the first refusal; no changes to plan ledger rows,
  deployment, release, installation, or descriptor eligibility.

Retained text pane captures and the contract-test log omit trailing blank lines;
full pane output remains in the event JSONL.
