# Codex 0.153.4 integration — IN PROGRESS: attempt 12

## Attempt 1 — INCONCLUSIVE: setup FAIL (2026-09-09)

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

## Attempt 2 continuation — stop/reconcile succeeds; step 1 host switch refused

2026-09-10, newly authorized by the user's instruction to continue from the
current tree. Prior results and their commits remain intact. **Step 1 FAIL at
`launch_host`; native eligibility remains disabled.** This continuation corrects
the previous online-seat resume mistake: it calls the production `stop_session`
RPC, waits for that owned pane to disappear, then calls
`coordination.reconcile_live_presence` and verifies `health: session_dead` before
setting the scratch member opt-in and resuming. The resume now passes
`load_member` and reaches the host, which refuses:

```text
Conflict: app_server_switch_requires_5b_recoverable_relaunch_packet
```

This is an observed activation guard, not the previous controller prerequisite
failure. It still does not test the app-server process or attached TUI: refusal
precedes `HostProcess::launch`. No identity was erased by the controller to get
past it. The user limited product changes to the two recorded defects (plus a
registry entry if needed); a recoverable activation/switch packet is additional
product scope. The trial stopped here, as the original stop-on-failure rule
requires. Neither instruction-source refusal nor rollback pane leak was reached.

| Step | Continuation outcome | Evidence |
| --- | --- | --- |
| 1. Daemon-owned host and attached strict TUI | **FAIL: activation refused** | Canonical initialize passes; daemon stops the pane and reconciles offline; resume reaches `launch_host` and returns the named switch-packet refusal. No host/thread/socket published. |
| 2. Idle Mesh input and explicit read | **NOT RUN** | No send, native input, marker reply or native receipt. |
| 3. Active steer | **NOT RUN** | No model turn or expectedTurnId. |
| 4. Operator/socket interleave and host exclusion | **NOT RUN** | Stop used terminal exclusion; this is not proof of mutual host-operation exclusion. |
| 5. Compaction | **NOT RUN** | Startup inbox acceptance does not establish compaction recovery. |
| 6. Restart | **NOT RUN** | Fresh scratch run is not the daemon restart experiment. |
| 7. Rollback | **NOT RUN** | No hosted attachment. Final owned-process cleanup passes separately. |

### Continuation commands and S-runtime identities

[Exact controller](integration/continuation-controller.py),
[event JSONL](integration/attempt2-continuation/run/events.jsonl),
[initialize report](integration/attempt2-continuation/run/initialize-result.json),
[resume report](integration/attempt2-continuation/run/resume-result.json),
[daemon log](integration/attempt2-continuation/run/daemon.log).
Source pair: Taurhaus **`6ff97a25`** / canonical implementation **`06d1267b`**,
protocol **27**; Mesh **`4388d6a`**. Both rebuilt locally; both builds exit **0**,
each Cargo preflight exit **1**, no competing Cargo. Temporary descriptor diff
still changes only 0.153.4 `disposition: trial`, `enabled: true`.

Scratch root **`/tmp/th-int-4mx35359`**, private port **30058**; marker
**`68573aa36d40ed95`**, never submitted. The same credential isolation, private
tmux and PID namespace were used. The member project again contains a committed
short AGENTS.md; only auth.json was copied into the otherwise-empty Codex home.
Member model **gpt-5.6-luna**, effort **low**; Claude lead remains credential-free
and takes no model turn. Binary digests are `events.jsonl:1–4`.

Exact operation order and request IDs:

1. `coordination.initialize_team`, request `50031efd220fdbcb`, run
   `init_3de2ad0682bf4320a7e2210ddb8fa9e7` — all nine stages succeeded, including
   launch before delivery opt-in (`events.jsonl:18–23`). Uses the unchanged
   builder `DEFAULT_CANONICAL_POLICY` through the production RPC.
2. `stop_session`, request `f1d2cf681888d633`, params
   `{"tmux_pane":"%2","cli_tool":"codex"}` (`events.jsonl:30`). Private tmux
   pane enumeration then confirms `%2` absent. The daemon's graceful-exit and
   pane teardown messages are in the daemon log.
3. `coordination.reconcile_live_presence`, request `4d905e5cd619d95e`, params
   `{"team_name":"integration"}` (`events.jsonl:42`). Saved
   [before-stop](integration/attempt2-continuation/run/before-stop.json) and
   [after-stop](integration/attempt2-continuation/run/after-stop.json) records show
   health changed from `healthy` to `session_dead`, while paneId **`%2`**, panePid
   **134**, paneStartTime **21676363**, terminalContract **1** and attachment
   generation **1** were retained. Session ID is null, appServer absent.
4. Scratch member config `adapter_mode: app_server`, then
   `coordination.resume_member`, request `7e6dd1af11606534`, run
   `resume_9047a57f21c549a998fc512834b5cdb0` (`events.jsonl:46`). Result:

```json
{"failed_step":"launch_host","succeeded_steps":["validate","load_member"],"resumed":false,"message":"Conflict: app_server_switch_requires_5b_recoverable_relaunch_packet"}
```

`events.jsonl:55` records the immediate stop. The source explanation is now
corroborated at runtime: `HostedMembers::launch` rejects a record with no
appServer and an existing pane or session identity. Production reconciliation
retains the old paneId even after that pane is gone. No guard was removed, no
runtime record fabricated, and no additional feature was implemented.

### Continuation receipts, cost and cleanup

Only startup inbox observations exist. The member runtime's journal reference is
message **`51b5a0fe-5c17-4002-957d-3085f1415615`**, delivery
**`984ab1be-73d6-454b-b00e-42d69bd5d69f`**, sequence **2**, projection `pending`,
stage `accepted`, path `inbox`, 2,273 accepted bytes, zero offered/read bytes.
No native wire, native enqueue, explicit read, compaction card, or host-operation
holder evidence was generated. Journal references are not original journal rows.

[Cost ledger](integration/attempt2-continuation/run/cost-ledger.json): **0 observed
Codex turns / $0 observed model spend**, **0 Claude turns / $0**, **0 compactions**.
The [usage-event export](integration/attempt2-continuation/run/usage-events.json)
is empty. No work prompt or model-turn submission was sent. Across attempt 1,
attempt 2 and this continuation: **0 observed turns / $0**, versus caps of 8 and
$3. No subscription invoice measurement is claimed.

Controller exit **1**. Its finally block restored the Mesh descriptor with the
specified `git checkout -- src/delivery/app_server/capabilities.rs` command,
exit **0**, stopped and waited its own process groups, ended the private PID
namespace and removed the scratch root/auth copy. Eight remaining owned process
identities were recorded before final cleanup; the member pane had already
closed through `stop_session`. [Cleanup](integration/attempt2-continuation/run/cleanup.json)
reports no survivors and the private port closed. Both worktrees remain on the
specified branches; no operator daemon, tmux server, home or standing team was used.

Reproduction from this checkout root (the controller stops at the guard):

```sh
TRIAL_EVIDENCE_LABEL=attempt2-continuation python3 docs/design/evidence/native-eligibility/integration/attempt2-build.py
python3 docs/design/evidence/native-eligibility/integration/continuation-controller.py attempt2-continuation/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt2-continuation python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

The optional evidence label on the build/gate helpers preserves previous results;
default behavior is unchanged. Original attempt 2 controller remains unchanged.

### Continuation gates and final disposition

| Exact gate from checkout root | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, typecheck, 150 frontend files / 2,486 tests passed. |
| `just lint` | **0** | All lint and recipe guards passed. |
| `just test-contracts` | **0** | All 68 contract tests passed. |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh gates | N/A | No passing eligibility flip. |

[Gate artifacts](integration/attempt2-continuation/gates),
[final cleanup/evidence audit](integration/attempt2-continuation/final-verification.json).
The read-only audit verifies offline reconciliation, retained pane identity,
named host refusal, PID/start-tick absence, closed private port/socket, removed
roots/auth copy, clean Mesh tree, and artifact hygiene. Text captures and logs
omit trailing blank lines; complete captured pane bytes remain in event JSONL.

**Descriptor flip: none. Mesh commit: none. Registry entry: not needed.** Product
code changes **0**, non-evidence insertions **0 / 400**, tests added **0**, regression
red/green **not applicable**: neither authorized defect was reached or fixed.
The missing activation packet cannot be supplied by either requested defect fix.
No runtime/native attachment or eligibility tuple was fabricated to bypass it.

Continuation deviation: one newly authorized corrected setup run after the
previous turn's stop, using production stop/reconcile before resume. This run
stopped at its first failed stage. No numbered trial step was green; the completed
deliverable is an evidence commit with the requested co-author trailer. The
independent Opus lens remains unavailable. Original canonical journal rows and
native wire receipts remain unproved; no model spend or uptake is inferred from
startup inbox acceptance. No plan ledger, deployment or install changes.

## Attempt 3 — fresh hosted seat; startup recovery rejected (2026-09-10)

**Step 1 FAIL.** The production `coordination.initialize_team` request carried
`delivery: "app_server"` on the fresh Codex seat. The daemon reached the real
host, created thread **`01a088d7-f6e4-74c3-b4d2-8ab1b821510f`**, and logged one
loaded instruction source. Startup recovery then failed with the exact public
refusal **`Conflict: host rejected request; reconcile before retrying`**.
No retry or later numbered step ran. Mesh eligibility remains disabled; no
product code changed in either repository.

S = observed runtime/artifact; I = source-based inference; U = unverified.

| Step | Outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon host + strict attached TUI | **FAIL: `launch_host` rejected** | [Initialize report](integration/attempt3/run/initialize-result.json), [structured log](integration/attempt3/run/taurhaus.log.jsonl): real thread and instruction-source count, then failed startup card. No attached member pane, published attachment snapshot, or terminalContract 1 proof survived. |
| 2. Idle `mesh send`, native receipt, explicit read | **NOT RUN** | No send/read, marker reply, native enqueue or consumption. |
| 3. Active `turn/steer` | **NOT RUN** | No model turn or expectedTurnId. |
| 4. Operator input + host-lock interleave | **NOT RUN** | No operator input or retained holder file; no mutual exclusion claim. |
| 5. Compaction | **NOT RUN** | Failed startup recovery is not a compaction trial. |
| 6. Daemon restart | **NOT RUN** | No restart or recovery claim. |
| 7. Operational rollback + tmux send | **NOT RUN** | No rollback RPC, refusal probe, remove/add or tmux delivery. Final teardown passed separately. |

### Executed pair, isolation and commands

Taurhaus source **`de71587e`**, product base **`80290f36`** (#158), branch
`feat/integration-trial`; Mesh **`4388d6a`**, branch `feat/native-push`.
The daemon's real `ping` returned protocol **27**, version **0.9.7**.
Both checkout-local builds exited **0**, each after
`pgrep -af '(^|/)cargo( |$)'` returned **1** (no competing Cargo):

```sh
# Under /home/mstie/projects/taurhaus-trial/src-tauri, target=./target
cargo build --bin taurhaus-daemon
# Under /home/mstie/projects/mesh-push, target=./target
cargo build --bin mesh
```

[Build controller](integration/attempt3-build.py),
[daemon build metadata](integration/attempt3/daemon-build.json),
[Mesh build metadata](integration/attempt3/mesh-build.json),
[exact temporary descriptor diff](integration/attempt3/mesh-trial-descriptor.diff).
Only the 0.153.4 descriptor was temporarily admitted, with `disposition: "trial"`,
`enabled: true`, transport `unix-websocket`, and the three strings verified
against this checkout's `coordination/hosted.rs`:
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1`.
The wildcard and other harness descriptors remained disabled.

[Executed runtime controller](integration/attempt3-controller.py) copied those
binaries only into scratch `$HOME/.local/bin`. SHA-256 identities are the first
four rows of [events.jsonl](integration/attempt3/run/events.jsonl). The isolated
version probe returned **codex-cli 0.153.4**; the unused Claude lead was **2.1.267**.
Scratch root **`/tmp/th-int-h0o7f0s_`** was mode 0700; all writable homes/data roots
were beneath it. CODEX_HOME began with only the authorized auth.json copy, mode
0600, then received generated config: Luna, low effort, read-only sandbox, never
approvals, disabled web search, 32768 context window and scratch-project trust.
No operator config, hooks, history or sessions were imported. Auth/token contents
were never logged or exported.

Bubblewrap hid `/home`, `/tmp`, `/run`, re-exposed only the writable scratch root,
and supplied a private PID namespace. A credential-free Claude lead and one
Codex seat used a new committed scratch git project with this entire AGENTS.md:

```text
This is an isolated transport trial. Reply briefly. Never execute tools or commands.
```

The private tmux server used `TMUX_TMPDIR=/tmp/th-int-h0o7f0s_/tmux`, inherited
TMUX absent, `tmux -D -f /dev/null`; all capture commands used that environment.
The private daemon bound the successfully probed **28528** port, with explicit
`--port 28528 --data-dir /tmp/th-int-h0o7f0s_/data`. Its token was used only on
that private RPC connection. No installed daemon or operator tmux was contacted.

Initialization request **`e3ae3202b2a588cc`**, run
**`init_fd5d137eb6fe4dd5a223b3aee53acd3c`**, is retained in `events.jsonl:18`.
Its messaging policy was parsed directly from the builder's unchanged
`DEFAULT_CANONICAL_POLICY`; [policy.json](integration/attempt3/run/policy.json)
retains it. [Team config](integration/attempt3/run/team/config.json) proves
`messaging_format: 2`, `delivery_owner: "team"`, `minimum_writer: "mesh-journal/2"`
and the freshly created seat's `adapter_mode: "app_server"`. There was no
post-launch config switch or resume attempt.

### Failure, receipt and journal excerpts

Final status request **`a418df344fbcd848`** returned:

```json
{"failed_step":"launch_host","message":"Conflict: host rejected request; reconcile before retrying","retryable":true,"succeeded_steps":["validate_configuration","create_team","add_lead"]}
```

S: `taurhaus.log.jsonl:17` records `hosted.instruction_sources.loaded`, count **1**,
thread **`01a088d7-f6e4-74c3-b4d2-8ab1b821510f`**.
At line 20, `onboarding.delivery.observed` records:

```json
{"delivery_id":"9b0be71718566219a263ac974d6742889cd84736530f245314a4f0cd34d7aebd","kind":"baseline","path":"app_server","stage":"failed","generated_bytes":2172,"accepted_bytes":0,"offered_bytes":0,"returned_by_read_bytes":0}
```

Card context is `[1,0]`, member incarnation
`e469b72e-2b23-424a-940b-cb3b3e2396f6`, team incarnation
`86317ec32ff7c286e992383466edb027da8c1508ca9afbb373df2d0181e921df`.
The original [canonical segment](integration/attempt3/run/team/state/messaging-v2/segments/000001.jsonl)
was retained and is **empty (0 bytes)**; there is no invented journal row.
No `native_enqueued` or `consumed_by_read` receipt exists in this attempt.

I: the loaded-source event proves `thread/start` passed the daemon's real
initialize/transport checks. In `hosted_process.rs`, `input()` first calls
`transcript()`; errors after submitting turn/start carry `outcome_unknown:`.
The unprefixed rejection plus failed recovery observation locate this failure
in pre-input recovery processing, consistent with the empty usage export.
The daemon intentionally replaces arbitrary host error text with this generic
refusal (`RpcError::Rejected`). **U:** the underlying host error object, exact
rejected wire request, Unix socket pathname, child PID/start ticks, and actual
instruction-source string were not exported. No more specific root cause is
claimed and no tracing/product patch or second run was made to obtain them.

The final [seat record](integration/attempt3/run/team/runtime/seat.json) has null
session/pane/launchRoot, no appServer, terminalContract **0**, attachment generation
**0**, and health `session_dead`. This is the post-failure snapshot; it does not
prove an attachment was never transiently published. The only remaining capture,
[bootstrap pane %0](integration/attempt3/run/initialize-pane-0.txt), is an empty
shell. Production failure cleanup had already removed member panes. The
instruction-source warning fix was reached; the required runtime record proof
and strict attached-TUI proof were not obtained.

### Spend, cleanup and gates

Reserved random marker **`a6796ec88e60e144`** was never submitted.
[Cost ledger](integration/attempt3/run/cost-ledger.json) and
[usage export](integration/attempt3/run/usage-events.json) show:

| Spend item | Observed model turns / generations / compactions | Observed USD |
| --- | --- | --- |
| Claude lead startup, no work | 0 / 0 / 0 | $0.00 |
| Codex host/thread creation; failed startup recovery | 0 / 0 / 0 | $0.00 |
| Mesh messages, operator input, compaction | 0 / 0 / 0 | $0.00 |
| Attempt 3 total versus hard caps | **0 / 8 turns** | **$0.00 / $3.00** |

No accepted paid submission or token usage was observed. Actual subscription
billing was not queried; zero is the observed no-generation result, not an
invoice measurement. The controller included startup turns in its rollout
watchdog, with a one-turn/$0.60 reserve and conservative packet-rate accounting;
that watchdog's nonzero-spend path was not exercised.

Controller exit **1** was observed. Its finally block waited its own children,
ended the descendant PID namespace, and removed the scratch root/auth copy.
[Cleanup](integration/attempt3/run/cleanup.json): no survivors, port closed, root
removed. Five remaining host PID/start-tick identities are retained in
[identities.json](integration/attempt3/run/identities.json); the short-lived
app-server was already gone before that inventory. Descriptor restoration ran
exactly as requested, exit **0**:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The final read-only audit additionally checks PID/start-tick absence, private
socket/port/root removal, restored descriptor, evidence hygiene and zero product
diff. The built Mesh target artifact remains unused; its scratch executable copy
was deleted. No process started by another worker was killed.

Exact reproduction from `/home/mstie/projects/taurhaus-trial` (new authorized run
only; this controller reproduces the failed setup, not seven passing steps):

```sh
python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt3-controller.py attempt3/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt3 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

The gate helper is reused unchanged. It runs each exact recipe from this checkout
root with a credential-free HOME, all harness roots isolated, rejecting
CLI/tmux/Mesh shims, a private PID namespace, and checkout-local Cargo target.

| Required gate | Exit | Observed result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, frontend typecheck, 150 frontend files / 2,492 tests passed. |
| `just lint` | **0** | Rust/frontend/workflow/recipe gates passed. |
| `just test-contracts` | **0** | All contract test binaries passed; see retained log. |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on seven-step PASS; no descriptor flip or fix. |

[Gate logs and exit metadata](integration/attempt3/gates),
[final audit](integration/attempt3/final-verification.json),
[audit controller](integration/attempt3-audit.py).
Each Cargo preflight returned 1, with no competing Cargo; gate cleanup passed.
**Descriptor flip: none. Mesh commit: none. Taurhaus registry entry: not needed**
(`HostedDescriptor::codex()` already pins 0.153.4 and unix-websocket).
Non-evidence product insertions **0 / 200**, dependencies **0**, tests added **0**.
The live acceptance assertion observed red at step 1; there was no product repair
and no claimed regression red/green cycle. The conditional Mesh missing-record
refusal test/fix was not undertaken after this failure.

Deviations / evidence limits:

- The fresh-seat request follows the sanctioned production path. In this source,
  `members.rs` executes hosted shared activation during the `CreatePanes` pass
  and skips it in `LaunchSessions`. The refusal therefore occurred before the
  later launch/opt-in steps; the controller did not reorder those stages.
  The lead's command was rendered and its version probed, but its model CLI
  launch and login-screen observation were not established before cleanup.
- The brief's full wire, holder, attachment and pane evidence is incomplete at
  this stopped boundary. The public rejection deliberately omits its raw error;
  no exact underlying host cause, instruction-source string, or turn is inferred.
  The controller's unexecuted action loop is not a seven-step implementation or
  proof; it was never reached. No paid rerun was performed.
- No numbered step passed, so there is one failed-trial evidence commit rather
  than seven green-step commits. All prior attempt evidence remains intact.
- An Opus evidence lens is unavailable in the exposed agent/model/tool surface;
  no independent cross-family review is claimed. The read-only audit is local
  verification, not a replacement for that lens.
- Text captures and gate logs trim trailing blank lines; the actual captured
  pane output remains in events.jsonl. No plan ledger, product code, release,
  deployment, operator installation or other Taurhaus checkout was changed.

## Attempt 4 — INCONCLUSIVE: step 1 observer confounded host initialization

2026-09-10. **No eligibility enabled.** The production initialize operation
returned `Conflict: unsupported app-server handshake` at `launch_host`.
**This run does not establish a product defect:** the trial controller added an
observer that could initialize before the daemon. Codex 0.153.4 seeds its
`userAgent` from the first client, as the binding transport amendment already
warns. The observer therefore was not passive with respect to initialization.
This was a controller mistake. The trial stopped immediately, without a retry,
product patch, paid input, or continuation to steps 2–7.

Pair: Taurhaus `ae9ae356cce35829e8449fca34246ca9a32399f1` on
`feat/integration-trial`, containing PR #159 / `6f61f611`, protocol **27**;
Mesh `dfa22bc103fc84c32de77a7b6cbe054d58cba13b` on `feat/native-push`.
[Execution metadata](integration/attempt4/execution.json),
[daemon build](integration/attempt4/daemon-build.json),
[Mesh build](integration/attempt4/mesh-build.json): both builds exited **0**.
Each build used its checkout-local target after the exact Cargo preflight
`pgrep -af '(^|/)cargo( |$)'` returned **1** (no competing Cargo).
The temporary [descriptor diff](integration/attempt4/mesh-trial-descriptor.diff)
enabled only 0.153.4 as `trial`, with exact class identities
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1` and
`unix-websocket`. The candidate binary was copied only into scratch
`HOME/.local/bin`; its SHA-256 and the other copied executable hashes are in
[events](integration/attempt4/run/events.jsonl).

| Ordered step | Outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon launches host, attached pane, runtime record and startup card | **INCONCLUSIVE; execution assertion FAIL** | Production RPC failed at `launch_host`: `Conflict: unsupported app-server handshake`. Observer's Unix WebSocket Upgrade returned 101 and initialized against the scratch home. No thread/turn, appServer record, attached TUI or startup-card uptake established. |
| 2. Idle Mesh delivery, marker/reply, receipt and explicit-read consumption | **NOT RUN** | Stopped at step 1; no `mesh send`, receipt or explicit read. |
| 3. Active-thread pending, then one idle `turn/start` | **NOT RUN** | No thread or delivery obligation. The binding deferred-delivery rule was not exercised. |
| 4. Typed input plus pending delivery; mutual host lock exclusion | **NOT RUN** | No attached TUI; no holder-file proof. |
| 5. Compaction and recovery card on the same thread | **NOT RUN** | No compaction or recovery generation. |
| 6. Daemon restart and post-restart delivery | **NOT RUN** | Teardown only; no restart or identity-resume claim. |
| 7. Operational stop/remove/re-add tmux rollback and delivery | **NOT RUN** | No hosted seat to roll back; the documented refusal was read in source, not observed at runtime. Cleanup nevertheless completed. |

### Exact stopped boundary and the observer confound

[Controller](integration/attempt4-controller.py) created the team only through
`coordination.initialize_team`; its request has one Claude lead and one Codex
seat, `model: "gpt-5.6-luna"`, `reasoning_effort: "low"`,
`delivery: "app_server"`, and `messaging.mode: "canonical"` with the exact
[DEFAULT_CANONICAL_POLICY](integration/attempt4/run/policy.json) parsed from
`meshTabUtils.js`. It created a scratch git project with a short AGENTS.md,
read-only sandbox and never approvals. No launched seat was opted in later.
As in attempt 3, this source executes shared hosted activation during the
`CreatePanes` pass; the controller did not reorder production stages.

S: root `/tmp/th-int-02qek9op` (created private, 0700), daemon port **30683**,
private tmux socket `/tmp/th-int-02qek9op/tmux/tmux-1000/default`.
All product/account homes were under this root; inherited `TMUX` was absent.
Only the authorized `auth.json` was copied to the initially empty CODEX_HOME.
The daemon and its descendants ran inside a disposable Bubblewrap PID namespace,
with operator homes, `/tmp` and `/run` hidden except for the scratch bind.
The lead CLI version probe succeeded (Claude 2.1.267); a lead login screen was
not reached. No Claude model turn was requested.

The production operation id was **`init_62bda75b0bbe41d783a36fcd9518b23e`**.
[initialize-result.json](integration/attempt4/run/initialize-result.json) retains:

```json
{"failed_step":"launch_host","message":"Conflict: unsupported app-server handshake","retryable":true,"succeeded_steps":["validate_configuration","create_team","add_lead"]}
```

The [observer](integration/attempt4-observer.py) ran before initialization and
watched for the daemon-created Unix socket. It independently sent:

```json
{"id":"observer-init","method":"initialize","params":{"clientInfo":{"name":"trial_observer","version":"4"},"capabilities":{"experimentalApi":true}}}
```

S: [host-events.jsonl](integration/attempt4/run/host-events.jsonl), six rows,
records the exact HTTP Upgrade/accept, successful observer initialize against
`/tmp/th-int-02qek9op/codex`, and server close. It contains no `thread/started`,
`turn/started`, `thread/tokenUsage/updated`, or host JSON-RPC error object.
The observer retained only `codexHome` from its initialize result; it did **not**
retain `userAgent`. I: first-client seeding is the likely direct cause of the
refusal: `hosted_process.rs` parses the successful initialize response using
`strip_prefix("taurhaus_host/")` and returns the exact observed refusal when
that prefix is absent. U: the exact daemon initialize response and relative
initialize ordering were not captured, so that causal explanation is not a
wire-proven root cause. A future controller must avoid competing for first
initialization; this run was not repeated to test that correction.

S: [taurhaus.log.jsonl](integration/attempt4/run/taurhaus.log.jsonl) records
`coordination.step.failed` for `launch_host` at
`2026-09-10T02:53:31.315Z`. **There is no `hosted.rpc.rejected` log line or host
error object to quote**: the observed refusal is local validation of an
initialize response, not an observed JSON-RPC rejection. The raw error required
for a host-rejected-request case is therefore unavailable, not fabricated.
No `hosted.instruction_sources.loaded` event was reached; loaded AGENTS.md
sources cannot be claimed. Failure cleanup removed the provisional team before
the snapshot, so there are no runtime records, journal rows, receipts or holder
files for this attempt. The only surviving pane capture is the private bootstrap
shell [initialize-pane-0.txt](integration/attempt4/run/initialize-pane-0.txt).

### Spend, teardown and reproduction

Reserved random marker **`74212e7ee11810b2`** is in the isolation event; it was never submitted.
[Cost ledger](integration/attempt4/run/cost-ledger.json) and
[usage export](integration/attempt4/run/usage-events.json) agree:

| Spend item | Model turns / compactions | Observed spend |
| --- | --- | --- |
| Claude lead version probe; no work | 0 / 0 | $0.00 |
| Codex version probe and app-server initialization | 0 / 0 | $0.00 |
| Mesh inputs, operator inputs, compaction, restart, rollback | 0 / 0 | $0.00 |
| Attempt 4 total | **0 / 8 turns; 0 compactions** | **$0.00 / $3.00** |

No tokenUsage event or rollout turn was observed. No nonzero charge or null
per-turn cost is hidden. The unexercised observer ledger calculates each
`tokenUsage.last` at the packet's $0.20/$0.02/$1.20 per million
input/cached/output rates, plus a conservative all-tokens-at-$1.20 bound;
these are API-equivalent estimates, not actual subscription invoices.
Its pre-initialization connection hazard prevents calling that metering design
validated. Installation identifiers were redacted before logging, account usage
notifications omitted, and no credential contents retained.

Controller exit **1** was observed. Its finally block stopped and joined the
observer, ended the owned namespace, waited its child processes, verified no
run-tagged survivors and a closed port, and removed the scratch root and auth
copy. [Identities](integration/attempt4/run/identities.json) retain five
PID/start-tick pairs; the short-lived app-server had already exited before that
inventory. [Cleanup](integration/attempt4/run/cleanup.json) and the later
[read-only audit](integration/attempt4/final-verification.json) confirm all five
absent, no run-tagged process, private socket/root absent and port 30683 closed.
No foreign process was killed. Exact descriptor restoration exited **0**:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The Mesh worktree is clean. **Descriptor flip: none; Mesh commit: none.**
The scratch executable was deleted; the rebuilt target artifact is not installed.
**Taurhaus registry entry: not needed**; the existing hosted launch descriptor
already pins Codex 0.153.4 / unix-websocket.

Exact reproduction from `/home/mstie/projects/taurhaus-trial`, for a separately
authorized reproduction only (these scripts reproduce this stopped, confounded
setup, not a passing seven-step trial):

```sh
TRIAL_EVIDENCE_LABEL=attempt4 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt4-controller.py attempt4/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt4 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt4-audit.py
```

The audit targets the retained `attempt4/run`; a new evidence directory needs a
matching audit path. No paid retry, alternate client probe or product fix ran.

### Required gates and deviations

| Exact gate, from checkout root | Exit / result |
| --- | --- |
| `just check-quick` | **0**; Rust test compilation, frontend typecheck, 150 files / 2,495 frontend tests. |
| `just lint` | **0**. |
| `just test-contracts` | **0**. |
| `just test-rust-unit` | N/A: no `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A: conditional seven-step PASS was not reached. |

[Gate logs and exit metadata](integration/attempt4/gates) retain the exact
commands and preflights (all Cargo probes exited 1), using the unchanged
credential-free gate helper: scratch HOME, rejecting CLI/tmux/Mesh shims,
private PID namespace, checkout-local target. Gate cleanup removed its root.
[Audit controller](integration/attempt4-audit.py) additionally checks branch,
merged prerequisite, isolation, policy, zero spend, cleanup and artifact hygiene.

- Material deviation: the added observer could become the first client and
  invalidate the intended production initialization. The trial is **INCONCLUSIVE**,
  not an eligibility FAIL attributed to either product. No downstream acceptance
  claim is made; all steps after the refusal were stopped as instructed.
- No numbered step passed, so this is one evidence commit, not seven green-step
  commits. The live step-1 assertion observed red; no product regression was
  repaired, no artificial red/green unit-test claim, and no tests added.
- The conditional Mesh `runtime_record_missing:<member>` test/fix was not made.
  Product insertions **0 / 200**, new dependencies **0**, plan ledger edits **0**.
- An Opus evidence lens is unavailable in the exposed model surface. No
  cross-family review is claimed; the audit is local verification only.

Attempt-4 text captures and logs trim trailing whitespace/empty lines; events.jsonl
preserves the original pane capture string. No artifact contains auth contents.

## Attempt 5 — FAIL at step 2: missing native adapter selection (2026-09-10)

Pair: Taurhaus `f2a7553b` (includes PR #159 / `6f61f611`, protocol 27),
Mesh `dfa22bc`, Codex 0.153.4, gpt-5.6-luna / low. Both checkout-local builds
exited 0 after Cargo preflight found no competing build. Only the Mesh scratch
working-tree descriptor enables the exact paired class identities.

**Step 1 PASS (S).** Production `coordination.initialize_team` completed all
nine stages, including `opt_in_delivery`, with the builder's canonical policy,
credential-free Claude lead and fresh `delivery: app_server` Codex seat.
The runtime publishes terminalContract 1, Unix WebSocket, all three class
identities, and `/tmp/th-int-3r8tuw4q/project/AGENTS.md` as an instruction source.
The daemon-generated strict config and attached pane are retained under
[integration/attempt5/step1](integration/attempt5/step1).
Thread `01a08948-2281-74c0-8bba-67bb9da54f94` received the recovery card in
turn `01a08948-22b4-7a73-afe8-d42785e3b31a`; the attached pane shows the card
and the model's reply. No observer connection was opened. The daemon transcript
RPC supplied the actual turn events and usage; structured logs show instruction
loading and startup delivery. One generation: 10,787 input, 6,912 cached,
32 output, 0 reasoning output; API-equivalent **$0.00095164**, conservative
all-tokens-at-$1.20/M **$0.01298280**. These are metered-token calculations at
the packet's rates, not billing invoices. Claude: zero turns / $0.

Controller: [attempt5-controller.py](integration/attempt5-controller.py), based
on attempt 3. It never connects to an app-server socket; reads go through the
daemon after successful initialization. Evidence helpers have three offline
checks, observed red (missing helper module), then green. No product code changed.

### Attempt 5 final outcomes

**FAIL, stopped at step 2. Descriptor disabled; no Mesh commit.** Step 1 is a
real production launch success, not an eligibility PASS. The idle Mesh message
never reached the thread. No product change was made to Taurhaus or Mesh.
No Taurhaus registry entry is needed: the existing launch path succeeded.
The conditional Mesh named-refusal fix and descriptor flip were not performed.

| Brief step | Outcome | S-runtime evidence |
|---|---|---|
| 1. Daemon host + strict attached TUI + startup card | **PASS** | [Frozen step-1 evidence](integration/attempt5/step1), runtime, strict config, completed startup transcript and pane. |
| 2. Idle Mesh send, marker reply, native receipt, explicit-read semantics | **FAIL** | [Send acceptance](integration/attempt5/run/step2-send.txt), [scheduler diagnostic](integration/attempt5/run/step2-diagnostic.json), [idle transcript](integration/attempt5/run/step2-hosted-transcript.json), [pane](integration/attempt5/run/step2-pending-pane-2.txt). Accepted message stayed pending for approximately 84 seconds; no marker input/reply or seat receipt. |
| 3. Active thread defers, then exactly one idle delivery | **NOT RUN** | Stop-on-failure rule at step 2. Binding behavior would be `pending: thread_active`, then `turn/start`, not steer. |
| 4. Operator/socket ordering and reciprocal host-lock contention | **NOT RUN** | No operator input or contention experiment; no holder-file proof claimed. |
| 5. Actual compaction and recovery card | **NOT RUN** | No compaction; identity/recovery at that boundary unverified. |
| 6. Daemon restart and subsequent delivery | **NOT RUN** | No restart or identity-preservation claim. |
| 7. In-place refusal and operational remove/re-add rollback | **NOT RUN** | No rollback attempted after step-2 failure. Teardown nevertheless completed and was verified. |

### Exact failure boundary

At `2026-09-10T03:08:20.836203067Z`, this command ran through a new window on
the private tmux server, inside the daemon's PID namespace, using only the
scratch `$HOME/.local/bin/mesh` candidate:

```sh
mesh send seat 'ACTION REQUIRED: Reply exactly cobalt90cc90082c. Do not execute tools.' --team integration --name lead --summary 'idle marker'
```

It exited **0** and returned:

```json
{"message_id":"6446e66f-2b6b-44a3-9614-bae46484550d","sequence":4,"status":"accepted","projection":"pending"}
```

Delivery ID: `3a4769cb-348e-46fd-83bf-ab89ccb3570e`.
The [canonical journal](integration/attempt5/run/team/state/messaging-v2/segments/000001.jsonl)
contains exactly one marker `message_accepted`, with
`dispatch_decision: {expectation: "action", wake_eligible: true}`.
No marker `delivery_attempt`, receipt or `consumed_by_read` followed. The
journal's earlier `native_enqueued` row is **the Claude lead's native-mailbox
startup projection**, not a Codex delivery. No explicit read was performed:
that part of step 2 remains unverified because native delivery already failed.

The live scheduler's heartbeat advanced through `03:09:44.717969379Z`, with:

```json
{"last_defer_reason":"IO error: delivery: pending: activity absent","failures":0,"completed":0}
```

The [runtime](integration/attempt5/run/team/runtime/seat.json) retains
`appServer.state: "ready"`, the original thread, and
`activitySnapshotPath: null`. The daemon transcript reports `status.type:
"idle"` and only the startup turn. The [team config](integration/attempt5/run/team/config.json)
retains `adapter_mode: "app_server"`; the directory inventory proves
`state/delivery/adapter-seat.json` is **absent** while the app-server socket
still exists. No `.native` attempt evidence exists.

**Source explanation (I, supported by S runtime):** Mesh
`src/delivery/hook/state.rs::selected_mode` loads its own
`state/delivery/adapter-MEMBER.json`; an absent file returns `Selection::default`
whose mode is `Tmux`. It does not read the config's `adapter_mode`.
`src/delivery/runtime.rs::RuntimeFactory::resolve` therefore takes the tmux
branch and calls `Record::idle`, which refuses the missing activity path before
constructing the native adapter. Taurhaus's fresh hosted activation publishes
its config/runtime identity but did not publish Mesh's selection file. This
is a paired activation-seam failure, not proof that WebSocket delivery failed.
No selection/runtime files were forged and no post-launch opt-in was attempted.

There is **no `hosted.rpc.rejected` line or host error object for step 2**:
this refusal occurs before a host input RPC. The exact Mesh refusal above is
the available error evidence. The [daemon JSONL](integration/attempt5/run/taurhaus.log.jsonl)
and [daemon transcript responses](integration/attempt5/run/events.jsonl)
are retained; the final audit verifies the absence of host rejection events.

### Cost and isolation ledger

| Input / boundary | Model turns / generations | Metered usage and spend |
|---|---|---|
| Startup card, step 1 | 1 / 1 | Turn `01a08948-22b4-7a73-afe8-d42785e3b31a`; 10,787 input, including 6,912 cached; 32 output, 0 reasoning. **$0.00095164 API-equivalent**; **$0.01298280 conservative**. |
| Marker, step 2 | 0 / 0 | Never reached host; **$0.00 additional**. |
| Steps 3–7, including compaction/restart/rollback | 0 / 0 | Not run; **$0.00 additional**. |
| Claude lead | 0 / 0 | Credential-free setup screen; **$0.00**. |
| Total vs caps | **1 / 8 turns**, one generation, zero compactions | **$0.00095164 API-equivalent**, conservative **$0.01298280 / $3.00**. Actual billed dollars are not exposed. |

[Cost ledger](integration/attempt5/run/cost-ledger.json) and
[host tokenUsage events](integration/attempt5/run/host-events.jsonl) agree with
[rollout usage](integration/attempt5/run/usage-events.json). The controller
polled the daemon's bounded event buffer; repeated snapshots are retained and
must not be counted as additional turns/generations. The helper deduplicates
identical usage events by thread/turn/cumulative usage. No unmetered turn exists.
No observer client connected, before or after startup. Daemon `hosted_transcript`
was the only host-state read path, so first-client identity was never contested.

The private root `/tmp/th-int-3r8tuw4q` was mode 0700; HOME, all harness/data roots,
TMUX_TMPDIR and generated configs were inside it. Only `auth.json` was copied
into initial CODEX_HOME; its contents were never printed or retained. Private
port **25925**, never 17233. Bubblewrap hid operator homes, `/tmp` and `/run`,
with one writable scratch bind and a private PID namespace. The namespace
held the daemon, real Claude/Codex executables, private tmux server, attached
TUI, and team delivery owner. No model tool execution occurred.

[Cleanup](integration/attempt5/run/cleanup.json),
[eleven PID/start-tick identities](integration/attempt5/run/identities.json), and
[final audit](integration/attempt5/final-audit.json) prove all recorded processes
absent, port closed, socket/root absent and the scratch credential copy destroyed.
The controller's finally block terminated/waited only its children/process groups;
namespace teardown removed their descendants. No operator process was killed.
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
exited **0**, and the Mesh worktree is clean. Its built scratch candidate is not
installed into the operator's bin directory; source eligibility remains disabled.

### Reproduction, gates, and deviations

Executed from `/home/mstie/projects/taurhaus-trial`, without a branch switch:

```sh
TRIAL_EVIDENCE_LABEL=attempt5 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt5-controller.py attempt5/run
TRIAL_EVIDENCE_LABEL=attempt5 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt5_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt5-audit.py
```

[Execution metadata](integration/attempt5/execution.json) records build exits 0,
runtime exit **1** (the deliberate failed-step stop), gate-controller exit 0,
and helper/audit exits 0. For a newly authorized reproduction, choose a fresh
output label. Once `inspection_ready` and idle startup completion are observed,
submit the [recorded actions](integration/attempt5/actions.json) one at a time
as `action.json` in that run directory, waiting for `action_done`. These are
all executed actions, including the final failure stop; they are not a claimed
implementation of unrun steps 3–7. The short AGENTS.md, request, exact child argv,
private tmux bootstrap and cleanup are in the executed controller. The step-1
strict-config capture was a read of the published socket's sibling
`tui/config.toml`, recorded in `step1/config-capture.json`.

| Gate (checkout root; private credential-free home, inert CLI/tmux shims) | Exit |
|---|---|
| `just check-quick` | **0**; 150 frontend files, 2,495 tests; Rust tests compiled |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: conditional passing descriptor flip was not reached |

[Gate logs](integration/attempt5/gates) retain exact commands and Cargo preflight
exit **1** (no competing Cargo) before each recipe. All gate scratch roots were
removed. Four offline evidence-helper tests pass; the final audit checks 48
facts. Initial red was missing helper import; an additional real red caught an
over-broad evidence sanitizer before its correction. No product regression fix
or associated product test was commissioned after the runtime failure.

Deviations/limits:

- The marker command emitted a warning about missing formal assignment fields,
  but accepted it as an actionable, wake-eligible message; this was a bounded
  echo probe, not a task assignment. The scheduler diagnostic predates the send
  and identifies the earlier adapter-selection/runtime boundary.
- The executed sanitizer also redacted `author`, authority revision, and some
  scratch executable path suffixes because it matched `auth` and `/home` too
  broadly. These placeholders remain honest gaps; journal checksums describe
  pre-redaction rows, not byte-identical exports. Sender is still retained in
  the envelope, root revision in the card text, and full executable construction
  in the controller. No redacted observation was guessed back into existence.
  The [executed helper](integration/attempt5/support-executed.py) is archived;
  the current helper fixes only export filtering. A red regression test naming
  `d3b95226` preceded that correction. Final export additionally removes rollout
  account rate-limit metadata; no credentials, installation IDs or account
  usage rows are retained.
- No Opus evidence lens was available among the callable models/tools. The
  independent Opus review remains outstanding; the report claims only the
  runtime results and offline audit, not cross-family approval.
- One commit records green step 1; a second records failed step 2, final audit,
  gates and teardown. Steps 3–7 have no commits because they were not executed.
  Non-evidence product diff: **0 lines**, no dependencies, release, installation,
  plan-ledger changes, or Taurhaus registry change.

Attempt-5 text logs and pane exports trim trailing whitespace/empty lines;
JSONL retains the repeated daemon event snapshots, with documented redactions.


## Attempt 6 — production paired selection candidate

Taurhaus baseline `7fc64f0e` (includes hosted thread-state PR #159; protocol 27),
Mesh `a6ee296`. Both checkout-local builds exited **0**, after Cargo preflight
exit 1 (none running). The scratch-only compiled descriptor matches the daemon
class identities; no registry entry is needed by the launch path.

### Step 1 — PASS

The production `coordination.initialize_team` request used the builder's exact
DEFAULT_CANONICAL_POLICY, a credential-free Claude lead, and one
`gpt-5.6-luna` low seat with `delivery: app_server` at creation. Every pipeline
step completed, including `launch_sessions` and `opt_in_delivery`.
[Request and outcomes](integration/attempt6/step1/initialize-result.json),
[policy](integration/attempt6/step1/policy.json).

The [runtime record](integration/attempt6/step1/step1-runtime.json) publishes
terminalContract 1, thread `01a0898d-222f-7581-8cd8-332d3b3e2fb4`, Unix WebSocket
host identity and the scratch project's AGENTS.md instruction source. The
[process identities](integration/attempt6/step1/step1-identities.json) record
the app-server child and attached TUI; the [strict config](integration/attempt6/step1/generated-config-0.toml)
sets the matching model/effort/read-only/never policies.
The [pane](integration/attempt6/step1/step-1-pane-2.txt) shows the startup card
and the model reply. [Host events](integration/attempt6/step1/host-events.jsonl)
and [structured log](integration/attempt6/step1/taurhaus.log.jsonl) retain
turn/start acceptance, instruction loading, and onboarding delivery. No observer
connection was opened.

Startup turn `01a0898d-225c-77a1-b591-bd7e93510d55`: 10,777 input, 6,912 cached,
27 output, zero reasoning; **$0.00094364 API-equivalent**, **$0.01296480
conservative**. [Metered ledger](integration/attempt6/step1/cost-ledger.json).
The host emits token counts, not invoiced USD; rates use the attached-TUI packet.

Reproduction: `TRIAL_EVIDENCE_LABEL=attempt6 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py`,
then `python3 docs/design/evidence/native-eligibility/integration/attempt6-controller.py attempt6/run`.
The controller reuses attempt 5 with lossless overlapping-buffer export and
retains only new host events. Its offline regression names `d3b95226`; red was
`ModuleNotFoundError: attempt6_support`, then one passing regression test after
implementation. No product code or product regression fix was made.

### Step 2 — PASS

[Owner status](integration/attempt6/step2/step2-status-before.txt) reports
`adapter seat: mode=app_server source=config diagnostic=none`; live team owner,
epoch 2. The [send](integration/attempt6/step2/step2-send.txt) accepted random
marker **cobalta6e6c2cd57**, message `c9199479-21ad-4b17-aab2-0d4a13ae0e31`,
delivery `d498033d-dfd5-4533-baed-5f3ef9081c53`.
The [journal before read](integration/attempt6/step2/journal-before-read.jsonl)
records exactly one native enqueue for it, method `turn/start`, request
`69eabd28-632e-4dd4-8f1c-bc9c80810bc7`, turn
`01a0898e-39df-79e2-bf52-02e8ac1ca7bc`, unchanged thread. It contains no
`consumed_by_read` for the marker. The [pane](integration/attempt6/step2/step2-pane-2.txt)
shows the input once and exact reply once. Only the subsequent
`mesh read --unread --mark-read --team integration --name seat` produced the
[explicit read receipt](integration/attempt6/step2/explicit-read-receipt.json).

The echo turn used 11,840 input (9,984 cached), 14 output, no reasoning:
**$0.00058768 API-equivalent**, **$0.01422480 conservative**. Cumulative:
**2 turns, $0.00153132 / $0.02718960 conservative**.
The send warned about absent formal task-assignment fields; it remained an
authorized bounded echo message, accepted as actionable and wake-eligible.

### Attempt 6 final outcome — INCONCLUSIVE

| Ordered step | Verdict | Runtime evidence / boundary |
|---|---|---|
| 1. Production daemon launch + startup card | **PASS** | Runtime, strict config, child/TUI identities, pane and startup usage above. |
| 2. Idle native delivery + explicit-read semantics | **PASS** | Config-selected app_server; one marker and reply; native enqueue precedes explicit read, above. |
| 3. Active-thread deferral, then one idle delivery | **INCONCLUSIVE as a full step; protocol behavior PASS** | Five `pending: ... thread_active` receipts precede one `turn/start` enqueue; one user item and one reply. Active pane captured, final reply pane not captured before controller reserve stop. |
| 4. Typed operator/socket ordering + both lock directions | **NOT RUN** | Controller already stopped; no holder contention claim. |
| 5. Compaction and recovery boundary | **NOT RUN** | No compaction submitted. |
| 6. Scratch daemon restart | **NOT RUN** | No restart submitted. |
| 7. Operational rollback | **NOT RUN** | No in-place refusal, stop/remove/re-add or tmux delivery claimed. Final safety teardown did run. |

**Stopping reason is a controller limit, not a product rejection.** The exact
controller exception was `AssertionError: evidence reserve reached` at step 3,
exit **1**, at epoch timestamp `1789014389.2898245`. The implementation added
an 850,000-byte runtime-output reserve to protect the requested <1 MB retained
sidecar ceiling. It successfully stopped further work, but counted unfiltered
periodic daemon telemetry and stderr, so it stopped prematurely. This is an
avoidable trial-controller deviation. No second paid run or product patch was
made to work around it. The final retained evidence is below 1 MB after offline
filtering; that does not retroactively complete the stopped trial.

#### Step 3 observations

The [exact action driver](integration/attempt6-step3.py) starts one bounded,
80-line response through `coordination.hosted_input`, then sends random marker
**deferred4e17e869** through the scratch Mesh binary while that turn is active.
[Start result](integration/attempt6/run/step3-active-start.json),
[send result](integration/attempt6/run/step3-send.txt),
[active pane](integration/attempt6/run/step3-active-pane-2.txt),
[active status](integration/attempt6/run/step3-active-status.txt).
No observer connected; no observer turn/start, steer, or interrupt was used.

The [canonical journal](integration/attempt6/run/team/state/messaging-v2/segments/000001.jsonl)
retains message `15b2d258-0b8f-49ef-b114-6927e55eee5a`, delivery
`46008da4-087a-4bf8-988a-40c12bc97983`. Receipt sequences **10, 12, 14, 16, 18**
are pending, each with this exact evidence class:

```text
pending: pre_input_failure: IO error: delivery: thread_active
```

Each pending receipt has null method/request/turn fields because it is
**pre-input**, not an unmetered model turn. Sequence **20** is the sole native
enqueue for this obligation, with request `ea8dc458-d2e5-4b16-bcc3-0be38367a447`,
method `turn/start`, turn `01a08991-1738-7d52-a973-4b2148cd3b9d`, and unchanged
thread `01a0898d-222f-7581-8cd8-332d3b3e2fb4`. Its host start followed the
active seed's completion. [Host events](integration/attempt6/run/host-events.jsonl)
retain all four starts/completions and all four tokenUsage events, appended
only once across rolling polls. `item/completed` contains exactly one user
item and one exact reply for each random marker. The final
[hosted state](integration/attempt6/run/hosted-transcript.json) is idle and
contains the deferred reply; its bounded `eventsTruncated: true` view no longer
contains early turns, which remain in the appended event archive.

The active status capture reported `failed to acquire lock: delivery mirror
busy`, and the later status reported `error=none deferred=none` after completion.
Those are retained observations, not evidence for the separate Step 4 host-lock
requirement. The journal provides the required named active deferral. There is
**no `hosted.rpc.rejected` event and no host error object** in this run; none is
fabricated for the controller stop. The final host turns report `error: null`.
The stderr also repeatedly warns about `_active-project-teams/config.json`
being absent; this is retained with counts, not patched or assigned causality.

#### Complete cost ledger

All generations used `gpt-5.6-luna`, low effort. Input includes the cached subset;
output includes reasoning. Dollar figures are numerical API-equivalent estimates
from the host's actual tokenUsage events and the attached-TUI packet's rates
($0.20/$0.02/$1.20 per million input/cached/output), **not invoiced charges**.
Conservative cost applies $1.20/M to every input and output token.

| Input | Turn ID | Input / cached / output / reasoning | API-equivalent USD | Conservative USD |
|---|---|---|---|---|
| Startup | `01a0898d-225c-77a1-b591-bd7e93510d55` | 10,777 / 6,912 / 27 / 0 | 0.00094364 | 0.01296480 |
| Idle Mesh echo | `01a0898e-39df-79e2-bf52-02e8ac1ca7bc` | 11,840 / 9,984 / 14 / 0 | 0.00058768 | 0.01422480 |
| Step 3 active seed | `01a08990-e90d-7693-8669-a5dfdcfd3f87` | 11,885 / 11,008 / 519 / 34 | 0.00101836 | 0.01488480 |
| Deferred Mesh echo | `01a08991-1738-7d52-a973-4b2148cd3b9d` | 12,508 / 11,008 / 11 / 0 | 0.00053336 | 0.01502280 |
| **Total** | **4 / 8 turns; 4 generations; 0 compactions** | **47,010 / 38,912 / 571 / 34** | **0.00308304** | **0.05709720 / 3.00** |
| Claude lead and unrun steps 4–7 | 0 turns | 0 / 0 / 0 / 0 | 0.00 | 0.00 |

[Ledger](integration/attempt6/run/cost-ledger.json) and
[rollout accounting events](integration/attempt6/run/usage-events.json) agree:
`metering_complete: true`, no unmetered turn IDs. No tool-execution item occurred.

#### Isolation, teardown, export, and gates

Private scratch root `/tmp/th-int-3s1yjoo9`, mode 0700; private port **28897**.
Only auth.json was copied into the initially empty CODEX_HOME. The allowlisted
HOME, harness/data roots and TMUX_TMPDIR all remained beneath scratch, TMUX was
absent from the parent environment, and bubblewrap hid operator homes, /tmp and
/run while owning a private PID namespace. The Claude lead stayed at its
credential-free setup screen. [Exact commands, requests, binary hashes and
identities](integration/attempt6/run/events.jsonl).

The controller's finally block terminated and waited only its own process
groups; namespace teardown removed the daemon, child, attached TUI and private
tmux server. [Cleanup](integration/attempt6/run/cleanup.json) records no
survivors, closed port, and removed scratch root/auth copy. The final offline
PID/start-tick audit independently found every recorded process absent, and
checked that the private port and app socket were closed/absent. No operator
process was killed. Descriptor restoration ran the exact authorized
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
and exited **0**. Mesh is clean, its descriptor disabled, **no Mesh commit**.
No Taurhaus registry entry is needed; no Taurhaus product code changed.

Reproduction after the build/controller commands above: replay the
[recorded actions](integration/attempt6/actions.json) one at a time through
`action.json`, waiting for `action_done`; the Step 3 driver preserves the timed
sequence. These scripts reproduce the executed portion only, not unrun steps.
A new live run requires a newly authorized trial and a fresh output label.
The executed controller is retained unchanged, including its premature reserve.
Offline completion commands, from this checkout root:

```sh
TRIAL_EVIDENCE_LABEL=attempt6 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt6-export.py
python3 docs/design/evidence/native-eligibility/integration/attempt6_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt6-audit.py
```

The exporter is a one-time post-teardown transform of the fresh run directory.
It retains first/last periodic telemetry/stderr samples and all other structured
records, with original counts in the [manifest](integration/attempt6/export-manifest.json).
Identical frozen/final files are retained once and mapped there; final-view
history points to the append-only event file. Redactions remove installation
IDs, credentials/account metadata and disallowed operator-home paths. These
are sanitized exports, not byte-identical journal checksum validation.
Initial unfiltered evidence exceeded 1 MB (**1,163,225 bytes**, observed red);
the final bounded export passes, including the scripts and audit. Frozen
per-step snapshots remain distinct time-scoped evidence, not extra usage.

| Exact gate, credential-free isolated environment at checkout root | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: conditional all-seven-step PASS was not reached |
| Offline overlapping-buffer regression / final audit | **0 / 0**; one regression test and 140 initial audit assertions |

[Gate records](integration/attempt6/gates/gate-check-quick.json),
[lint](integration/attempt6/gates/gate-lint.json),
[contracts](integration/attempt6/gates/gate-test-contracts.json),
[final audit](integration/attempt6/final-audit.json). Every gate's Cargo preflight
exited 1 (no competing Cargo); gates used inert harness/tmux shims, no credentials,
and private PID namespaces. Their scratch roots were removed.

Remaining deviations: the reserve guard prematurely ended the run; Step 3's
final pane was not captured; Steps 4–7 and the conditional Mesh named-refusal
fix/descriptor flip were not reached. No Opus model/tool is callable here, so the
requested independent Opus evidence lens remains unavailable. No review approval
is claimed. The initial offline audit assumed full history in the bounded final
view and user items in turn/completed; observed red corrected it to authoritative
item/completed events in the complete append-only archive. No paid retry was used.
Non-evidence product diff **0 lines**, no dependencies, installation, release,
branch switch or plan-ledger edit.


### Continuation preparation — offline green, no additional model turns

On the user's continuation request, the checkout was clean: Step 1 was already
committed as `45abdcf7`, Step 2 as `e1d004a9`, and the stopped Step 3 result,
gates and teardown as `c4735af5`. No completed green runtime step was left
uncommitted. The original scratch root/thread was deleted; a live continuation
requires a new production initialization. The original hard cap has **four
model turns remaining**. No budget reset was inferred from “continue.”

The prepared [next controller](integration/attempt6-next-controller.py) reuses
attempt 6's production initialization, private namespace, auth-only scratch
home, exact pinned tuple, token ledger and teardown. It requires an explicit
new-turn allowance argument (1–8); the operator must supply the authorized
remaining allowance. The executed attempt-6 controller and earlier continuation
controller are preserved as historical evidence. This prepared controller has
**not been run against a live daemon or model** and proves no new runtime step.

The premature evidence-reserve defect is corrected in collection: raw stderr
stays beneath scratch, only its last 16 KB is retained at teardown; periodic
structured telemetry retains first/last samples and counts; final transcript
snapshots reference the single appended event archive; streaming text deltas
are omitted while completed items, thread/turn boundaries, receipts and token
usage remain intact. The 850 KB retained-output reserve still applies.

[Three offline regression tests](integration/continuation_retention_test.py)
name `c4735af5`; red was missing `continuation_retention`, followed by three
green assertions suites with the helper and controller integration.
[Replay of real retained events](integration/attempt6/continuation-preflight/retention-replay.json)
reduced 569 events / 165,547 bytes to 50 events / 27,152 bytes while preserving
every tokenUsage, turn start/completion and completed item. This replay opened
no runtime socket and read no credentials.

Fresh gates ran from the checkout root with the existing credential-free
private-namespace gate controller:

```sh
python3 docs/design/evidence/native-eligibility/integration/continuation_retention_test.py
TRIAL_EVIDENCE_LABEL=attempt6/continuation-preflight python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

| Exact gate | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| New offline collector regression | **0**, 3 tests |

[Gate records](integration/attempt6/continuation-preflight/gates/gate-check-quick.json),
[lint](integration/attempt6/continuation-preflight/gates/gate-lint.json),
[contracts](integration/attempt6/continuation-preflight/gates/gate-test-contracts.json).
Each Cargo preflight found no competing Cargo. Gate scratch cleanup completed.
No `src-tauri/` change, product code, registry entry, descriptor edit, or Mesh
commit. **Additional spend: zero turns / $0.00.** The next live run awaits the
user's answer whether its budget is fresh or shared with the four spent turns;
no missing capture or unrun step has been promoted to PASS.

## Attempt 7 — FAIL at step 4, 2026-09-10

**Latest verdict: FAIL at step 4 under an injected process pause.** Steps 1–3
passed on the real daemon; the typed-input portion of step 4 also passed.
The lock interleave used SIGSTOP/SIGCONT on verified trial-owned processes.
It observed exclusion in both directions, but the daemon's interrupted read
failed, then subsequent reads returned `uncorrelated host response`. This is
not evidence of failure under ordinary, unpaused scheduling. No product code
was patched; steps 5–7 were not run. The compiled Mesh descriptor is restored
to disabled, the Mesh tree is clean, and there is no Mesh flip/fix commit.
The existing Taurhaus registry launched the seat successfully; no entry is needed.

Pair: Taurhaus worktree `feat/integration-trial`, starting `ebca1917`, including
hosted thread-state merge `6f61f611`, protocol 27; Mesh `feat/native-push`
`a6ee296`. The trial-only descriptor matches `taurhaus-daemon-owned-thread/1`,
`strict-config/1`, `daemon-owned/1`, and `unix-websocket` on exactly 0.153.4.
Builds used checkout-local targets after the required Cargo probes found no
competing Cargo. Both build exits were 0.

### Ordered outcomes (S: observed runtime)

Paths below are beneath [attempt7/run](integration/attempt7/run/); identical
snapshots have aliases in [duplicate-aliases.json](integration/attempt7/duplicate-aliases.json).

| Step | Outcome | Runtime evidence |
|---|---|---|
| 1. Hosted launch/startup | **PASS** | `step1-runtime.json`, `step1-identities.json`, `generated-config-0.toml`, `step-1-pane-2.txt`: child on Unix socket, attached strict-config TUI, terminalContract 1; scratch AGENTS.md instruction source. Startup recovery card and reply. `initialize-result.json` proves production canonical initialization and opt-in completed. |
| 2. Idle native delivery/read | **PASS** | `step2-status-after.txt`: seat mode=app_server source=config. `step2-pane-2.txt`: saffron7c82ab input/reply. Message `7bc10aed-1448-4372-b95f-41c5e5d30c06`, receipt native_enqueued, turn/start `01a089a5-2174-76c1-a7d4-3ee845098b65`. Journal sequences 1–6 have no consumed_by_read; `step2-read-receipt.json` records it at sequence 7 after explicit `mesh read --unread --mark-read`. |
| 3. Active-thread deferral | **PASS** | `step3-receipts.json`: message `052292df-27e8-4cc9-b5c0-18ac2b05b4bc` first pending/thread_active, then exactly one native_enqueued turn/start. `step3-exposure.json`: one completed user item and one reply for juniper8e21cd. `step3-final-pane-2.txt` captures the reply. |
| 4. Typed input + host exclusion | **FAIL after lock interleave** | `step4-final-pane-2.txt` and completed items prove maple7d91cb typed through private tmux while cedar9a38ef was pending; operator reply precedes socket delivery, each once. `step4-locks.json` proves actual kernel lock owners and both deferrals, followed by a failed host read. `events.jsonl` records the terminal uncorrelated-response failure and teardown. |
| 5. Compaction | **NOT RUN** | Stopped at step 4. |
| 6. Daemon restart | **NOT RUN** | Stopped at step 4. |
| 7. Operational rollback | **NOT RUN** | Stopped at step 4; no in-place rollback refusal or operational remove/re-add is claimed. Final process teardown was completed separately. |

Thread throughout the completed observations:
`01a089a4-2183-70a1-b1c6-566c00026910`. Root `/tmp/th-int-9n5s3dud`, private
port **23640**; private tmux socket beneath that root. Namespace host PID 124
corresponded to OS PID 1744349/start ticks 23616732. All panes, the team owner,
Claude login lead, host, daemon and scratch credentials were isolated. The
controller used an allowlisted environment without inherited TMUX, copied only
auth.json, and hid operator homes and services through Bubblewrap. No observer
connected to the member socket; host evidence came through the daemon RPC.
Claude ran no model turn.

### Exact failure and lock-holder evidence

The first single read-only sample missed the brief lock hold (no turn spent;
`step4-locks-initial-sample.json`). The second bounded sample briefly paused the
owned app-server to let a real daemon transcript request hold the host-operation
lock, then paused the owning daemon and resumed the app-server. During the
1.1-second hold, Mesh recorded this receipt for the lock marker:

```text
message_id: 63dc0eca-8cbd-457b-82c3-79d14807fd01
stage: pending
class: pending: pre_input_failure: IO error: delivery: host_operation_busy
```

Kernel holder evidence (same stable inode, preserved in `step4-locks.json`):

```text
FLOCK ADVISORY WRITE 1744225 08:30:1155236 0 EOF  # daemon
FLOCK ADVISORY WRITE 1745865 08:30:1155236 0 EOF  # Mesh
```

The host-operation implementation does not publish a `.holder.json`; those
files belong to terminal locks. The sidecar captures actual `/proc/locks` and
`/proc/<pid>/fdinfo` holder records, not an invented product holder file.
After resuming the daemon, its outstanding transcript RPC returned:

```json
{"code":"HOST_OPERATION_FAILED","message":"host read failed; outcome may be unknown"}
```

The helper then captured Mesh holding the same lock and issued one daemon
transcript read, which waited about two seconds and returned the expected:

```json
{"code":"HOST_OPERATION_FAILED","message":"Conflict: host operation deferred: lock busy"}
```

The helper's local `PASS` means only these two exclusion observations. Once
all paused processes resumed, the main controller and its final read returned:

```json
{"code":"HOST_OPERATION_FAILED","message":"uncorrelated host response"}
```

It exited **1**, stopped at step 4, and tore down immediately. The lock probe
had continued its read-only reverse-exclusion check after the first read error;
that continuation and its paused-process condition are deviations, not normal
host behavior. Source inspection suggests an interrupted socket read followed
by a late correlated response rejected on the next RPC (inference, not an
established underlying OS error: the implementation discards that error).
No `hosted.rpc.rejected` row or app-server JSON-RPC error object was emitted:
these are host-client transport/correlation errors. `taurhaus.log.jsonl`
preserves the actual structured instruction/onboarding/coordination rows;
`final-audit.json` records the absence rather than fabricating a rejection.

### Spend (S token counts; I API-equivalent dollars)

The model was **gpt-5.6-luna, low**, as pinned in the packet. Rates are the
packet's $0.20/$0.02/$1.20 per million input/cached/output tokens; amounts are
not billing receipts. Every completed generation has real host tokenUsage
numbers in `host-events.jsonl`, checked against rollout events in
`usage-events.json`. Ledger: [cost-ledger.json](integration/attempt7/run/cost-ledger.json).

| Input / generation | Turn ID | Input / cached / output | API-equivalent USD |
|---|---|---|---|
| Startup card | `01a089a4-21b1-7660-9d59-c37505975bd3` | 10773 / 6912 / 30 | $0.00094644 |
| Idle marker | `01a089a5-2174-76c1-a7d4-3ee845098b65` | 11838 / 9984 / 11 | $0.00058368 |
| Active timing input | `01a089a5-c1e8-7c13-880d-2662be2ba50e` | 11880 / 11008 / 517 | $0.00101496 |
| Deferred marker | `01a089a5-edd8-7021-ba5d-cd7452e0ba50` | 12505 / 11008 / 19 | $0.00054236 |
| Typed-order timing input | `01a089a7-310b-7a93-b7ae-7cef13d84fb9` | 12555 / 12032 / 495 | $0.00093924 |
| Typed operator steer (same turn) | `01a089a7-310b-7a93-b7ae-7cef13d84fb9` | 13069 / 12032 / 10 | $0.00046004 |
| Pending socket marker | `01a089a7-6204-76e0-aea4-2e38a79c9075` | 13182 / 12032 / 10 | $0.00048264 |
| Lock marker, interrupted | `01a089a9-946d-7ca3-8b43-17d518b961f1` | **Unreported** | **Unreported** |

**Seven protocol turns started / eight paid inputs including the typed steer.**
Seven completed generations have a metered subtotal of **$0.00496936**
(conservative all-tokens-at-$1.20/M subtotal **$0.1042728**). The final lock
marker started at 04:53:07.311Z as the paused Mesh process resumed; the main
controller failed at 04:53:07.320Z. Cleanup raced that already-pending delivery.
It has task_started but no tokenUsage/token_count before namespace termination.
No zero, null charge, invented token count, or verified full-run USD total is
claimed. **The required complete cost ledger was not achieved.** Counts stayed
below 16; the full-run dollar cap cannot be independently verified from the
missing final usage. No further paid retry was made.

### Reproduction, tests, gates and cleanup

Exact retained controllers (all are evidence, no product changes):

```sh
TRIAL_EVIDENCE_LABEL=attempt7 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt7_test.py
python3 docs/design/evidence/native-eligibility/integration/continuation_retention_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt7-controller.py attempt7/run
# While the controller is live: replay the serialized action objects in events.jsonl
# through attempt7-actions.py, in order; the step-4 lock probe is:
python3 docs/design/evidence/native-eligibility/integration/attempt7-locks.py
# After teardown:
TRIAL_EVIDENCE_LABEL=attempt7 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt7-audit.py
```

Attempt 7 reuses attempt 6's controller and ebca1917 retention helpers. It drops
stream deltas, deduplicates overlapping event buffers, excludes verbose periodic
telemetry, caps pane captures at 60 lines, and has no size or overall-time abort.
The new offline budget test first failed on a 1.1 MB evidence sample and on the
missed seventeenth host-only turn, then all three passed after correction.
The three existing retention tests also passed. No test invoked a real CLI or
read credentials. Three per-step commits retain completed steps 1–3.

| Exact gate (checkout root, isolated credential-free homes and inert harness shims) | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: passing descriptor flip/named-refusal fix was conditional on seven passing steps |

Gate records and bounded log tails are under `attempt7/gates/`. No dependencies,
product changes, releases, installations or plan-ledger edits. Non-evidence
insertions: **0/200**. No Opus evidence review was run; this session exposes no
Opus review model. The interrupted-read condition and incomplete final metering
remain explicit limits for review.

Cleanup: `cleanup.json`, `identities.json`, and `final-audit.json` verify no
PID/start-tick survivor, port 23640 closed, private tmux/server namespace ended,
and scratch root including copied auth removed. All paused processes were
resumed in the lock helper's finally block before controller teardown. No
foreign process was signalled. Mesh restoration used the exact authorized
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
and exited 0; its whole working tree is clean. Deduplicated retained evidence
is about **372 KB**, excluding controller scripts. Latest verdict remains
**FAIL step 4**, with steps 5–7 unrun and eligibility disabled.


## Attempt 8 — FAIL step 3 acceptance; product result INCONCLUSIVE, 2026-09-10

**Latest verdict: FAIL at step 3's premature controller assertion.** Steps 1–2
passed. The controller checked for `pending: thread_active` only 0.91 seconds
after sending the active-turn marker, while Mesh reported `delivery mirror busy`,
and stopped immediately. This does **not** establish a native active-thread
product defect. No host JSON-RPC rejection occurred. Steps 4–7 were not run;
there was no process-pause probe or other fault injection. The descriptor remains
disabled. No Mesh flip/fix commit, no Taurhaus product changes, and no new registry
entry: the existing registry successfully launched the hosted seat.

Pair: Taurhaus `47b7b8c2`, including hosted merge `6f61f611` / protocol 27;
Mesh `feat/native-push` `a6ee296`. Both checkout-local builds exited **0** after
`pgrep -af '(^|/)cargo( |$)'` returned 1 (no competing Cargo). Only the scratch
binary enabled 0.153.4 as `trial`, matching `taurhaus-daemon-owned-thread/1`,
`strict-config/1`, `daemon-owned/1`, transport `unix-websocket`. The authorized
Mesh worktree descriptor edit was reverted after both teardowns.

### Ordered outcomes (S: observed runtime)

Paths below are beneath [attempt8/run](integration/attempt8/run/). Byte-identical
snapshots are mapped by [duplicate-aliases.json](integration/attempt8/duplicate-aliases.json).

| Step | Outcome | Runtime evidence |
|---|---|---|
| 1. Hosted launch/startup | **PASS** | `initialize-result.json`: production canonical initialize, all seats launched, opt-in completed. `step1-runtime.json`, `step1-identities.json`, `generated-config-0.toml`, `step-1-pane-2.txt`: app-server child on Unix socket, attached strict-config TUI, terminalContract 1, scratch AGENTS.md instruction source, startup recovery card/reply. |
| 2. Idle native delivery/read | **PASS** | `step2-after-status.txt`: seat mode=app_server source=config. `step2-pane-2.txt`: saffronb1f96e input/reply. `step2-receipts.json`: native_enqueued via turn/start with thread/turn IDs. `step2-journal-before-read.json` has no consumed_by_read; `step2-journal-after-read.json` records it after explicit `mesh read --unread --mark-read`. |
| 3. Active-thread deferral | **FAIL acceptance; product INCONCLUSIVE** | `step3-active-start.json`, `step3-active-pane-2.txt`: timing turn started. `step3-send.txt`: juniper17f73c accepted. `step3-active-status.txt`: delivery mirror busy. `step3-pending.json`: required thread_active receipt absent at the first snapshot. `step3-outcome.json` and `events.jsonl`: immediate assertion/stop. No deferred-marker exposure observed. |
| 4. Operator input/passive lock evidence | **NOT RUN** | Stopped at step 3. The offline-tested passive sampler was never activated; no live holder claim. |
| 5. Compaction | **NOT RUN** | Stopped at step 3. |
| 6. Normal daemon restart | **NOT RUN** | Stopped at step 3. |
| 7. Operational rollback | **NOT RUN** | No in-place refusal or remove/re-add result claimed. Final teardown is separately verified. |

Continuing run thread: `01a089b8-ad39-7870-8d03-84bb4e729314`.
Step-2 message `b5428b50-c394-4e60-86d3-ad4d722371f9`, delivery
`f6691926-f617-4308-8797-d5067dee9539`; receipt evidence:

```json
{"class":"rpc_accepted_not_comprehension","expected_turn_id":null,"method":"turn/start","reconciliation_required":false,"request_id":"bd030d8a-5623-4d5d-9831-d778e73e4c0b","thread_id":"01a089b8-ad39-7870-8d03-84bb4e729314","turn_id":"01a089ba-4b6e-7160-8fa1-23ba4ba3e3e0"}
```

Step-3 message `af125f9f-72ba-4fb7-bc3d-d70ead896adb`, delivery
`6c719232-3272-4f43-a067-641157debf66`, accepted at journal sequence 8.
The active timing turn was `01a089bb-0b09-7c10-a86d-2c9c847ab562`.
Controller action times (Unix seconds): hosted input **1789017131.745834**,
Mesh send **1789017132.004219**, status **1789017132.318732**, capture
**1789017132.688976**, fail **1789017132.911521**. Exact diagnostics:

```text
[mesh] delivery seat: ... failures=22 completed=1
error=failed to acquire lock: delivery mirror busy deferred=none stale=false
AssertionError: no active-thread deferral
{"kind":"stopped","step":3,"error":"step 3: no active-thread deferral","type":"RuntimeError"}
```

`hosted.rpc.rejected` rows: **none**. Host error object: **none emitted**.
The absent pending receipt triggered the harness assertion; it was not a rejected
host call. The 80-line timing turn completed during read-only final metering.
No marker delivery turn ran. Whether normal scheduler retries would have produced
thread_active then delivered once idle remains untested in this attempt.

### Isolation, collector deviation, and spend

Both setups used a 0700 `/tmp` root, scratch HOME and all harness roots, only a
copied auth.json plus generated config, no inherited TMUX, private TMUX_TMPDIR,
private PID namespace with operator homes hidden, private probed daemon ports,
and the candidate Mesh binary copied only into scratch `$HOME/.local/bin`.
Production `coordination.initialize_team` carried the exact
`DEFAULT_CANONICAL_POLICY`, a Claude login-only lead (zero model turns), and one
Codex `gpt-5.6-luna` / low seat with `delivery: app_server` at creation. No observer
connected to the member socket. Reads used the daemon's hosted_transcript RPC.

The first setup, port **29926**, root `/tmp/th-int-6rz5c0xy`, successfully displayed
its startup card and reply. Its collector parsed an in-flight partial JSONL row
and exited before recording step 1 as complete. This non-product abort violated
the requested collector discipline. The collector was corrected to read complete
JSONL rows after an offline red/green test. Repeated setup used port **30008**,
root `/tmp/th-int-1ppvr92f`, and charged the first launch to the **same** attempt-8
budget (not a new budget). That earlier setup's original failure, host events,
usage and cleanup remain in `attempt8/setup-collector-abort/`.

[Combined ledger](integration/attempt8/cost-ledger.json): every started turn has
real host tokenUsage.last counts, checked against retained rollout usage events.
Rates are the packet's $0.20/$0.02/$1.20 per million input/cached/output tokens.
USD values are API-equivalent estimates, not exposed subscription invoices.

| Turn / generation | Turn ID | Input / cached / output | API-equivalent USD |
|---|---|---|---|
| First setup startup | `01a089b7-9f21-7ce3-9e85-5802747ef36b` | 10783 / 6912 / 35 | $0.00095444 |
| Continuing setup startup | `01a089b8-ad65-7972-9de5-2fbad20b3a20` | 10767 / 6912 / 32 | $0.00094764 |
| Idle marker | `01a089ba-4b6e-7160-8fa1-23ba4ba3e3e0` | 11825 / 6912 / 12 | $0.00113524 |
| Active timing input | `01a089bb-0b09-7c10-a86d-2c9c847ab562` | 11868 / 11008 / 517 (32 reasoning) | $0.00101256 |

**4/16 turns; $0.00404988 API-equivalent total; $0.0550068 conservative total**
(all input/output charged at the highest packet rate). No unmetered started turn,
no compaction, no typed steer, and no model turn for the undelivered step-3 marker.
Both caps remain unexhausted. No further paid retry followed the step-3 assertion.

### Exact reproduction and verification

Controllers are retained evidence, not default gate executables:

```sh
TRIAL_EVIDENCE_LABEL=attempt8 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt8_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt8-controller.py attempt8/run
# After inspection_ready, in a separate shell, one step at a time:
python3 docs/design/evidence/native-eligibility/integration/attempt8-steps.py 2
python3 docs/design/evidence/native-eligibility/integration/attempt8-steps.py 3
# Step 3 preserves the actual premature assertion, not a repaired replay.
# After teardown:
TRIAL_EVIDENCE_LABEL=attempt8 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt8-audit.py
```

The current controller preserves the corrected collector used for the continuing
run. `events.jsonl` records every action and production RPC request/response;
`attempt8-actions.py` sends those serialized actions. Deltas are dropped, pane
captures capped at 60 lines, and byte-identical sidecars deduplicated with aliases.
There is no evidence-size abort. The passive sampler reads only matching kernel
FLOCK/fdinfo records; its fixture test passed after observed missing-module red.
The collector's partial-row regression test also observed red then green. These
are two offline tests; neither invokes a real CLI nor reads harness credentials.
The retained step-3 driver still has the premature assertion so the failed
acceptance run is reproducible, not silently rewritten as a passing controller.

| Exact gate, from checkout root in credential-free isolated homes with inert harness/tmux shims | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no src-tauri diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: seven-step PASS/flip/fix condition not met |

Gate metadata and bounded tails are under `attempt8/gates/`; all Cargo preflights
returned 1 (no competing Cargo). No dependencies, product changes, installations,
release actions, plan-ledger edits, or non-evidence inserted lines (**0/200**).
No Opus evidence lens was run: this session exposes no Opus review model.

[Final audit](integration/attempt8/final-audit.json) rechecks both setup runs by
PID/start ticks: **no surviving daemon, host, TUI, Mesh owner or tmux server**;
ports 29926/30008 closed; scratch roots and copied auth removed. Gate children
were waited and their root removed. The exact authorized Mesh descriptor checkout
exited 0; the Mesh working tree is clean. Retained sidecars are about **379 KB**,
excluding scripts. The two collector/timing failures are explicit harness
deviations, not evidence against native eligibility under ordinary scheduling.

### Attempt 8 continuation — FAIL step 5, 2026-09-10

**Latest verdict: FAIL step 5 — no recovery card at compaction or first following
input.** The user authorized continuation from the committed tree. Steps 1–4
passed on a fresh isolated setup; actual `/compact` completed on the same thread,
and the next input/reply succeeded, but neither contained a recovery card.
Runtime contextGeneration remained `"0"`, admitted_boundary remained null, and
the recovery receipt still referenced the startup card. Steps 6–7 were not run.
No product defect was patched. The Mesh descriptor was restored to disabled;
there is no flip/fix commit. The existing Taurhaus registry needs no addition.

The earlier four turns and $0.0550068 conservative subtotal remain charged to
attempt 8; **no budget reset**. The corrected driver waits for current-message
**journal** receipts through ordinary scheduler retries. The earlier driver had
both checked too soon and checked only delivery-state files. Four offline tests
covered retry waiting, message identity, refusing enqueue as proof of deferral,
and the cumulative budget. Earlier controllers remain unchanged as evidence.

Pair: Taurhaus `4ff2ac17` at continuation start (same product code from hosted
merge `6f61f611`, protocol 27), Mesh `a6ee296`. Both checkout-local builds exited
0 after no-Cargo preflights. The temporary 0.153.4 descriptor used the same three
class identities and unix-websocket transport; only the scratch copy ran.

#### Ordered outcomes (S runtime)

Paths below are relative to [continuation/run](integration/attempt8/continuation/run/).
[Duplicate aliases](integration/attempt8/continuation/duplicate-aliases.json)
resolve byte-identical snapshots. Each completed step was separately committed.

| Step | Outcome | Evidence |
|---|---|---|
| 1. Hosted startup | **PASS** | initialize-result.json, step1-runtime.json, step1-identities.json, generated-config-0.toml, step-1-pane-2.txt. Production canonical initialize launched the login-only Claude lead and Luna/low hosted seat, loaded scratch AGENTS.md, delivered the startup card and attached the strict-config TUI. |
| 2. Idle delivery/read | **PASS** | step2-after-status.txt shows mode=app_server source=config. saffronaf494b appears once in input/reply; native_enqueued via turn/start; explicit mesh read alone creates consumed_by_read. step2-receipts.json and before/after journal snapshots. |
| 3. Active deferral | **PASS** | Message 2954e62b-f51f-4e1e-ab4d-3c857f4450cb first pending/thread_active, then exactly one native_enqueued turn/start after idle. step3-pending.json, step3-receipts.json, step3-exposure.json, step3-final-pane-2.txt. |
| 4. Typed input/passive locks | **PASS** | maple84cf3a typed into the private pane while cedar430145 pending; operator reply precedes socket delivery, each once. step4-exposure.json, step4-final-pane-2.txt, step4-receipts.json. Kernel holder samples in step4-locks.jsonl identify both daemon and Mesh on the same stable inode. |
| 5. Compaction recovery | **FAIL** | contextCompaction item completed; pane says Context compacted; thread unchanged. The first following daemon hosted_input and hazel12976c reply succeed without a recovery card. step5-boundary-events.json, step5-runtime-before.json, step5-runtime-boundary.json, step5-active-start.json, step5-final-pane-2.txt. |
| 6. Normal daemon restart | **NOT RUN** | Stop-on-failure at step 5. |
| 7. Operational rollback | **NOT RUN** | Stop-on-failure; no in-place refusal or remove/re-add claimed. Process teardown verified separately. |

Thread: `01a089c5-2225-7bb3-973a-2a77719269f1`; private port **26777**;
scratch root `/tmp/th-int-u97c8_bn`. Auth-only credential copy, private HOME,
harness roots, TMUX_TMPDIR, PID namespace and scratch project preserved the
packet's isolation. No observer connected to the app-server socket. The only
signal use was final teardown; no process pause/fault injection during any step.
The lead consumed zero model turns.

Passive step-4 holder excerpts, inode **1158695**:

```text
1789017879.496476  FLOCK ADVISORY WRITE 1908030 08:30:1158695 0 EOF  # taurhaus-daemon
1789017880.917861  FLOCK ADVISORY WRITE 1909718 08:30:1158695 0 EOF  # Mesh team owner
```

Both observations have matching `/proc/<pid>/fdinfo` lock records and PID/start
ticks. Later Mesh holds are retained too. Nothing was frozen to manufacture
contention. Ordinary pending receipts, eventual submission and transcript order
provide delivery evidence alongside those passive holder records.

#### Exact step-5 failure

Compaction turn `01a089c7-9994-7261-afcc-082ec2169813`, item
`01a089c7-999e-70f1-a754-565f2eb7ec7a`: started at **1789017954718 ms**, completed
at **1789017963589 ms**. The next turn was
`01a089c7-c0d9-7420-b3dc-f1c8cc3fca51`. The input/reply was:

```text
• Context compacted
› Reply exactly hazel12976c. Do not execute tools.
• hazel12976c
```

No new `[taurhaus] recovery_card` occurs in the retained boundary events or
completed next-turn user item. The next-input RPC returned an inProgress turn,
not an error. The controller then stopped with:

```text
AssertionError: no recovery card at compaction boundary or first following input
```

`hosted.rpc.rejected` rows: **none**. Host error object: **none emitted**.
Daemon `compaction.*` log rows: **none**. Mesh health at failure had `error: null`,
`last_defer_reason: null`, `pending_since: null`, completed 3. These absence
observations are retained in final-audit.json; there was no rejected Mesh input
or host RPC to quote. The unchanged context/recovery record demonstrates the
missing recovery transition; its deeper cause was not patched or established.

#### Spend and accounting limit

[Continuation and cumulative ledger](integration/attempt8/continuation/cost-ledger.json).
All values below come from actual host tokenUsage.last. Rates remain the packet's
$0.20/$0.02/$1.20 per million input/cached/output tokens, API-equivalent estimates.

| Generation | Turn ID | Input / cached / output | Metered USD |
|---|---|---|---|
| Startup | `01a089c5-2251-7860-9837-bb2c3e888f0a` | 10773 / 6912 / 30 | $0.00094644 |
| Idle marker | `01a089c5-8802-78e2-bcfa-01f4a26b1f38` | 11828 / 9984 / 10 | $0.00058048 |
| Active timing input | `01a089c5-ee6d-73f3-a929-05463b4292f6` | 11869 / 11008 / 523 | $0.00101996 |
| Deferred marker | `01a089c6-1d81-7681-8c6e-68d2fa02b715` | 12499 / 6912 / 12 | $0.00127004 |
| Typed-order timing input | `01a089c6-73c4-7221-ab29-d6206ff53dce` | 12542 / 6912 / 495 | $0.00185824 |
| Typed operator steer, same turn | `01a089c6-73c4-7221-ab29-d6206ff53dce` | 13056 / 12032 / 10 | $0.00045744 |
| Pending socket marker | `01a089c6-a319-7372-9dcb-6d6bec6b32cb` | 13170 / 12032 / 8 | $0.00047784 |
| Compaction | `01a089c7-9994-7261-afcc-082ec2169813` | 0 / 0 / 0; totalTokens 6344 | **Unreported cost; counter reset** |
| First input after compact | `01a089c7-c0d9-7420-b3dc-f1c8cc3fca51` | 11967 / 6912 / 19 | $0.00117204 |

Continuation: **8 protocol turns**, including compaction, with one extra typed
steer generation. Cumulative attempt 8: **12 protocol turns / 13 paid inputs
including the steer**, within 16. Ordinary-generation subtotal **$0.00778248**
for this continuation; with the prior $0.00404988, **$0.01183236 cumulative
metered subtotal**. Conservative ordinary-token subtotal: **$0.17358**.

The compaction event's token classes are all zero while totalTokens is 6344 and
cumulative usage is unchanged. This is retained exactly, but is **not proof of
zero billed compaction usage**. The inherited ledger initially marked any token
event complete; the final audit corrects that claim and an additional offline
red/green test guards this reset shape. Full compaction cost and therefore the
full-run dollar cap cannot be independently verified from these counters.
There is no fabricated count or zero-charge claim, and no further turn after
the failure. All ordinary turns have real non-null token/cost numbers.

#### Reproduction, gates and teardown

```sh
TRIAL_EVIDENCE_LABEL=attempt8/continuation python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt8_continuation_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-controller.py attempt8/continuation/run
# Separate shell after inspection_ready; inspect and commit after each:
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-steps.py 2
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-steps.py 3
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-steps.py 4
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-steps.py 5
# Step 5 stops the controller and tears down. Then:
TRIAL_EVIDENCE_LABEL=attempt8/continuation python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt8-continuation-audit.py
```

| Exact gate from checkout root, isolated credential-free homes/inert CLI shims | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| Offline continuation regressions | **0**, 5 tests after red |
| `just test-rust-unit` | Not required: no src-tauri diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: passing flip/fix condition not met |

[Final audit](integration/attempt8/continuation/final-audit.json) verifies no
PID/start-tick survivors, port 26777 closed, root/auth removed, and Mesh tree
clean after the authorized descriptor checkout. Gate children were waited and
their isolated root removed. Earlier runs' cleanup remains in their audits.
Sidecars are deduplicated (about **500 KB** for the continuation), panes capped
at 60 lines, deltas dropped; size never stopped a step. Credential/operator-path
checks passed. No new dependencies, install/release action, plan-ledger edits,
Taurhaus registry change or product patch; non-evidence insertions **0/200**.
Opus evidence review remains unavailable in this session. The remaining limits
are the step-5 recovery failure, unrun lifecycle steps, and unreported compact
billing; the earlier step-3 harness limitation has been resolved by this run.


### Attempt 9 — FAIL step 7: known defect (codex identity lane), 2026-09-10

**Steps 1–6 PASS. Step 7 FAIL — known defect (codex identity lane).** The
hosted-compaction fix works on the real paired host. Normal daemon restart also
preserves the thread and delivery. Operational remove/re-add launches a plain
tmux seat, but its delivery stays `pending: activity not freshly idle` and the
published Mesh activity snapshot reports `activity_confidence: "uncertain"`.
The 120-second delivery deadline expired. **No descriptor flip, no Mesh fix
commit, no Taurhaus product change.** Mesh's scratch descriptor edit was restored;
the detached `a6ee296` tracked tree was clean, but its trial-enabled build artifact
remained until the review correction below. All required Taurhaus gates passed.

#### Pair, isolation and ordered outcomes

Taurhaus start `ddb7aef1`, containing PR #160 / `06031992`, protocol **27**;
Mesh detached RC **`a6ee296`** in `/home/mstie/projects/mesh-trial` only. Both
checkout-local builds exit **0**, with the requested no-Cargo polling before
builds (daemon waited 270 seconds). `just ensure-tauri-resources` exited 0.
The temporary compiled 0.153.4 descriptor used `disposition: "trial"`, enabled
true and exactly `taurhaus-daemon-owned-thread/1`, `strict-config/1`,
`daemon-owned/1`, transport `unix-websocket`. Binary hashes and launch commands
are in the controller event trace. The inherited path sanitizer over-redacted
Mesh build metadata's cwd/target; attempt9-build.py pins their exact worktree
and checkout-local target. Review adds an explicit `mesh_worktree` field to
mesh-build.json, reconstructed from that pin and the matching binary hash;
original over-redacted cwd/target fields remain historical. The sanitizer now
preserves mesh-trial alongside the other worktree paths.

Scratch `/tmp/th-int-1vc78b94`, private port **31269**, private HOME, tmux server
and PID namespace, scratch Codex and credential-free Claude homes. Only the
explicitly authorized auth.json was copied, mode 0600; the model's scratch git
project contains a short AGENTS.md. Production `coordination.initialize_team`
created a login-only Claude lead plus one Luna/low member, with delivery
app_server **at creation**, canonical messaging and the MeshTeamBuilder default
retention policy. No observer connected to the app-server socket. No process was
paused or frozen. Only the normal step-6 daemon stop/start, production step-7
operations and final owned-process teardown intervened in process lifecycles.

Paths below are relative to [attempt9/run](integration/attempt9/run/).
[Duplicate aliases](integration/attempt9/duplicate-aliases.json) resolve snapshots
stored only once. Each green runtime step has its own commit.

| Step | Outcome | S-runtime evidence |
|---|---|---|
| 1. Hosted startup | **PASS** | initialize-result.json; step1-runtime.json; step1-identities.json; generated-config-0.toml; step-1-pane-2.txt. Socket child and strict-config attached TUI launched, instructionSources names scratch AGENTS.md, startup card displayed and answered. |
| 2. Idle delivery/read | **PASS** | step2-after-status.txt selects app_server source=config; marker in pane and model reply; native_enqueued turn/start with thread/turn ids. step2-journal-before-read.json contains no consumed_by_read; step2-explicit-read.txt alone creates it in step2-journal-after-read.json. |
| 3. Active deferral | **PASS** | The active turn is opened with `coordination.hosted_input`, followed by `mesh send`; deferral/exposure assertions are unaffected by that opener. step3-pending.json records pending/thread_active; step3-receipts.json records eventual native_enqueued turn/start; step3-exposure.json proves one user item and one reply; step3-final-pane-2.txt. |
| 4. Typed input/passive locks | **PASS** | The active turn is opened with `coordination.hosted_input`; deferral/exposure assertions are unaffected by that opener. step4-pending.json; step4-exposure.json shows one input/reply per marker, operator reply before socket reply; step4-final-pane-2.txt. step4-locks.jsonl has daemon and Mesh flock/fdinfo holders on one stable inode. |
| 5. Compaction recovery | **PASS** | step5-runtime-before/after.json: contextGeneration 0→1, same thread; complete taurhaus.log.jsonl contains compaction.codex_host.received/delivered; step5-boundary-events.json shows recovery-card user item; step5-compacted-pane-2.txt shows it after Context compacted. |
| 6. Daemon restart | **PASS** | Event trace records normal SIGINT stop/start; step6-resume-result.json reports owned thread resumed in pane %14; step6-runtime-before/after.json preserves thread; post-restart mesh send/reply/native_enqueued receipt (step6-receipts.json, step6-final-pane-14.txt). |
| 7. Operational rollback | **FAIL — known defect (codex identity lane)** | In-place refusal in step7-inplace-refusal.txt; stop/remove/add RPCs succeed (step7-stop/remove/add.json), delivery tmux at re-add. step7-runtime-after.json: terminalContract 1, no appServer/member daemon; plain pane %18. step7-final-status.txt defers on activity not freshly idle; step7-mesh-activity.json says uncertain; step7-final-pane-18.txt lacks the marker after 120 seconds. |

Managed-launch hook reconciliation also emitted two WARN rows in the complete
run log (rows 593 and 722), during steps 6 and 7 respectively:

```text
2026-09-10T09:10:35.586Z compaction.codex_hook.degraded: Managed launch continued without compact-hook trust
2026-09-10T09:11:25.027Z compaction.codex_hook.degraded: Managed launch continued without compact-hook trust
error.message (both): Not found: team config not found for '_active-project-teams' at /tmp/th-int-1vc78b94/claude/teams/_active-project-teams/config.json
```

Emit site: `src-tauri/src/commands/terminal_settings.rs:863`,
`log_managed_account_hook_degraded`. `_active-project-teams` is the lock name
in `src-tauri/src/coordination/stores/active_project.rs:14`, enumerated as a team
by hook reconciliation. This does not change step 6's PASS: the hosted path owns
compaction. The re-added step-7 tmux seat launched **without compact-hook trust**;
this additional defect is recorded, with no product fix in this lane.

Hosted thread throughout steps 1–6:
`01a08a91-af30-78f1-8cda-ee5e52d604e0`. Compaction turn
`01a08a94-9ddf-79e1-aa32-d95aec010449`, item
`01a08a94-9deb-7e52-94e1-acbfac1bc31e`: daemon received at
**09:09:57.549Z**, delivered at **09:09:57.634Z**. Recovery card turn
`01a08a94-b8cf-7322-97da-75bad48b64e4` completed and was answered:

```text
• Context compacted
› [taurhaus] recovery_card
  ... "context":[1,1] ...
• Awaiting a valid assignment and required context from the team lead.
```

#### Exact rollback refusal and failure

The real in-place Mesh command was:

```sh
mesh team adapter --member seat --mode tmux --team integration --name lead
```

Its exact response reason and CLI error (exit 1):

```text
IO error: delivery: app_server_switch_requires_5b_recoverable_relaunch_packet
error: unauthorized: adapter change not applied; inspect outcome
```

Then the scratch daemon accepted `stop_session` for `%14`,
`coordination.remove_member` and `coordination.add_agent` for the same seat name,
with `delivery: "tmux"`, Luna/low and the scratch project. Exact payloads and
run ids are in the event trace. The new plain pane `%18` displays the input
prompt without a message. Mesh selects `mode=tmux source=default`, but reports:

```text
deferred=IO error: delivery: pending: activity not freshly idle
```

Its published activity file has `pane_alive: true`, `pane_foreign: false`,
`active_non_shell_process: true`, `recent_io: false`, and
`activity_confidence: "uncertain"`. The daemon's separate UI-facing session
snapshot labels the process idle with low confidence/no attribution and maps it
to the old hosted rollout. This is the commissioned known identity defect;
there is no claimed tmux exposure or successful named-session rollback.
The raw controller failure is `plain tmux delivery stalled`. The final audit
classifies it from the authoritative Mesh activity file; the inherited driver
looked for uncertain in the separate UI snapshot, so it did not itself attach
the known-defect label. A read-only supplemental collector retained the actual
published activity file before cleanup.

`hosted.rpc.rejected` rows: **none in the complete run log**. Host error object:
**none emitted**. This is a pre-input activity deferral; receipts and health
reasons are retained, not replaced by an invented app-server error.

#### Every generation and cost

Fresh budget: **≤16 Codex inputs/turns, ≤USD 3**. Observed: **10 protocol turns,
11 paid inputs/generations including typed steer and compaction**, **zero Claude
turns**, and **no additional model turn in step 7**. Source:
[host events](integration/attempt9/run/host-events.jsonl),
[final ledger](integration/attempt9/run/cost-ledger.json) and retained rollout
usage-events.json. Packet rates: $0.20/$0.02/$1.20 per million
input/cached/output tokens; these are API-equivalent estimates, not invoices.

| Generation | Turn ID | Input / cached / output | API-equivalent USD |
|---|---|---|---|
| Startup | `01a08a91-af5b-7bc2-b4a6-260c5ec08a35` | 10772 / 6912 / 32 | $0.00094864 |
| Idle delivery | `01a08a92-cfb6-7792-9765-b8466cd9c00c` | 11838 / 6912 / 11 | $0.00113664 |
| Active timing input | `01a08a93-422f-7fa1-8990-7f36f67d11a7` | 11880 / 11008 / 519 | $0.00101736 |
| Deferred delivery | `01a08a93-7d42-7f30-9fd8-f3dd3671e78b` | 12505 / 9984 / 23 | $0.00073148 |
| Ordering timing input | `01a08a93-d85e-76e3-93ea-15c58892454b` | 12559 / 12032 / 495 | $0.00094004 |
| Typed steer (same turn) | `01a08a93-d85e-76e3-93ea-15c58892454b` | 13073 / 11008 / 9 | $0.00064396 |
| Pending socket delivery | `01a08a94-0f9c-7260-8672-1887b859d85d` | 13184 / 12032 / 9 | $0.00048184 |
| Compaction | `01a08a94-9ddf-79e1-aa32-d95aec010449` | 0 / 0 / 0 | **Unreported; counter reset** |
| Daemon recovery card | `01a08a94-b8cf-7322-97da-75bad48b64e4` | 12463 / 6912 / 59 | $0.00131924 |
| Restart recovery card | `01a08a95-4f2c-7980-ac5f-5ddb56fed7b1` | 13733 / 12032 / 17 | $0.00060124 |
| Post-restart delivery | `01a08a95-625f-7033-8265-4c561fbd139b` | 14572 / 13056 / 9 | $0.00057512 |

Ordinary-generation subtotal **$0.00839556**; charging every ordinary token at
$1.20/M gives **$0.1533144**. Compaction reported nonzero totalTokens with all
billable classes zero, a counter-reset shape. Its cost is **unreported, not
zero**. The final ledger marks that generation's costs null and metering
incomplete. Actual billed USD and the full-run USD cap cannot be independently
verified from this interface; ordinary measured spend is far below $3. No
further paid attempt or budget reset occurred.

#### Reproduction, gates, retention and teardown

The retained attempt9-controller.py/actions.py/steps.py adapt the attempt-8
continuation, preserving receipt polling through scheduler retries and passive
lock observation. Run from this checkout only:

```sh
just ensure-tauri-resources
TRIAL_EVIDENCE_LABEL=attempt9 python3 docs/design/evidence/native-eligibility/integration/attempt9-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt9_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt9-controller.py attempt9/run
# Separate shell after inspection_ready; inspect and commit after each:
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 1
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 2
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 3
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 4
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 5
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 6
python3 docs/design/evidence/native-eligibility/integration/attempt9-steps.py 7
# While step 7 waits, after step7-runtime-after.json exists, another shell:
python3 docs/design/evidence/native-eligibility/integration/attempt9-activity.py
# Gates use separate credential-free homes, private namespace and inert CLIs:
TRIAL_EVIDENCE_LABEL=attempt9 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
# Once the failed controller has torn down (audit is a one-time finalizer):
python3 docs/design/evidence/native-eligibility/integration/attempt9-audit.py
# Losslessly expand the retained command/RPC trace:
python3 docs/design/evidence/native-eligibility/integration/attempt9-audit.py --expand
```

| Gate / verification | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| attempt9 offline evidence tests | **0**, four tests; initial missing-module red, then missing pack_events red before lossless-dedup implementation |
| Reused continuation/passive/retention tests | **0**, ten tests |
| Final evidence/cleanup audit | **0** |
| `just test-rust-unit` | Not required: no src-tauri diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: passing flip/fix condition not met |

[Final audit](integration/attempt9/final-audit.json) verifies all owned PID/start
identities absent, private port **31269** closed, scratch root/auth removed and
Mesh tracked tree clean. The original audit checked source only and missed
`/home/mstie/projects/mesh-trial/target/debug/mesh`, whose SHA-256
`56228853cb7d7ad7de1d8bc4f60ad8567cbfed272e5963f596aeceda78c0c845`
matched the trial binary event. Review removed that exact artifact (hash check,
then `Path.unlink()`), and `attempt9-audit.py --verify-cleanup` now exits 0 only
when it is absent too. `mesh_tree_clean` now covers both tracked cleanliness and
absence of this trial build output; `review_cleanup` records the later correction.
Controller teardown removes the artifact alongside source restoration and measures
`auth_removed` after deleting scratch; the original cleanup row's literal is
historical, while the review audit independently checks root/auth absence.
Controller and step-7 driver exit 1 for the observed delivery
failure; gates, build and supplemental collector children were waited to exit.

The **complete daemon JSONL contains 935 deduplicated rows**, all 17 event
families retained, including every periodic telemetry row. Its SHA-256 is
`6354d13541995c8e228f145ae5045abdcc255f336ca5af5977f2482ec85e5fc9`.
The 2,070 controller event rows retain timestamps/order through 508 interned
payloads (events.jsonl + event-payloads.json); the offline round-trip assertion
verifies lossless expansion. Duplicate file aliases preserve every snapshot;
pane captures are ≤60 lines, with trailing blank padding removed (the lossless
command trace keeps the original output). Stderr/build/gate excerpts are bounded, with source
line counts and sanitized digests in excerpt-manifest.json. Omission markers
now name their source and omitted line count; omitted build/gate text is not
recoverable from the daemon JSONL. Only daemon excerpts link to that JSONL. No secrets, account usage rows or installation
ids are retained. Evidence is about **1.73 MB**, exceeding the approximate 1 MB
guidance to preserve complete JSONL and lossless command/RPC evidence; size never
aborted a step.

No Taurhaus product/registry patch, new dependency, install, release, plan-ledger
edit, branch switch, foreign checkout mutation or unowned process intervention.
Non-evidence insertions **0/200**. No descriptor flip or conditional missing-runtime
refusal fix was made because step 7 failed. The original session lacked an Opus
evidence lens; the subsequent operator-supplied Opus review is addressed below.
Remaining limitations are the known rollback identity defect, managed-launch
compact-hook degradation, and unreported compaction billing.

#### Attempt 9 review correction — 2026-09-10

All seven supplied findings were verified and fixed locally in the evidence,
controller/audit, and sanitizer. No trial rerun, descriptor flip, product change,
new dependency, or additional paid input: **0 turns / USD 0 additional spend**.
The original 10 protocol turns / 11 generations and ordinary API-equivalent
subtotal **USD 0.00839556** remain unchanged; compaction billing remains unreported.

Red-first command: `python3 docs/design/evidence/native-eligibility/integration/attempt9_test.py`.
After extracting the existing behavior into testable helpers, exit **1** showed
four regressions: teardown retained the fake trial binary, the audit accepted it
despite disabled source, build excerpts lacked their source, and mesh-trial paths
were redacted. After correction, exit **0**, **8 tests**, including measured
credential absence with a simulated incomplete scratch deletion. The real
`attempt9-audit.py --verify-cleanup` also failed on the leftover binary before
its hash-checked removal and passed afterwards. All fixtures are temporary,
use fake data, and execute no harness CLI.

Unpaid gate reproduction from the checkout root (the reused wrapper runs the
exact recipes with empty harness homes, inert CLIs and a private PID namespace):

```sh
TRIAL_EVIDENCE_LABEL=/tmp/taurhaus-attempt9-review-gates python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 -m unittest discover -s docs/design/evidence/native-eligibility/integration -p '*_test.py'
python3 docs/design/evidence/native-eligibility/integration/attempt9-audit.py --verify-cleanup
```

| Review verification | Exit / outcome |
|---|---|
| `just check-quick` | **0**, 19.46 s |
| `just lint` | **0**, 6.33 s |
| `just test-contracts` | **0**, 5.88 s |
| All offline evidence tests | **0**, 26 tests |
| Read-only cleanup audit | **0** |
| Gate cleanup | Children waited; isolated root removed |

Every Cargo preflight exited 1 (no concurrent Cargo). Raw review gate logs and
exit metadata are at `/tmp/taurhaus-attempt9-review-gates/gates/`; original run
gate metadata remains historical. Rust unit execution is not required because
there is no `src-tauri/` diff. The audit's read-only cleanup mode unpacks the
retained event trace so this verification does not rerun the one-time finalizer
or rewrite daemon evidence. The original daemon JSONL hash is unchanged.


### Attempt 10 — INCONCLUSIVE: setup/activation race; hosted launch observed, 2026-09-10

**Latest verdict: INCONCLUSIVE.** The production `coordination.initialize_team`
RPC launched both seats, including the hosted Codex member and its attached TUI,
but canonical activation hit a retryable `opt_in_delivery` refusal. The original
controller treated that setup state as a terminal step failure and tore down the
fixture without retrying. This is not evidence of a hosted-launch failure.
Round-1 review takes the expressly offered no-rerun correction: no additional
paid run, product patch or descriptor flip. Steps 2–7 remain unanswered.
[Initialize response](integration/attempt10/run/initialize-result.json).

The original [final audit](integration/attempt10/final-audit.json) and numbered
outcome sidecars retain the historical `FAIL` classification; this section
supersedes that interpretation without rewriting runtime evidence. The corrected
[read-only audit](integration/attempt10-audit.py) prints the current verdict.

| Brief step | Outcome | S-runtime evidence |
|---|---|---|
| 1. Production hosted launch | **INCONCLUSIVE** (setup/activation race; hosted launch observed). The step’s host/TUI/runtime identities were observed; canonical activation remained retryable. | [Runtime record](integration/attempt10/run/team/runtime/seat.json), [attached pane](integration/attempt10/run/initialize-pane-2.txt), [initialize report](integration/attempt10/run/initialize-result.json) |
| 2. Idle Mesh send / explicit read | **NOT RUN**, setup stopped before delivery checks | [Outcome](integration/attempt10/run/step2-outcome.json) |
| 3. Active-thread deferral | **NOT RUN**, setup stopped before delivery checks | [Outcome](integration/attempt10/run/step3-outcome.json) |
| 4. Typed input / passive exclusion | **NOT RUN**, setup stopped before delivery checks | [Outcome](integration/attempt10/run/step4-outcome.json) |
| 5. Compaction recovery | **NOT RUN**, setup stopped before delivery checks | [Outcome](integration/attempt10/run/step5-outcome.json) |
| 6. Daemon restart | **NOT RUN**, setup stopped before delivery checks | [Outcome](integration/attempt10/run/step6-outcome.json) |
| 7. Operational tmux rollback | **NOT RUN**, setup stopped before delivery checks; #163 attribution fix not exercised | [Outcome](integration/attempt10/run/step7-outcome.json) |

#### Pair and isolation

Taurhaus source `5c4132a9` on `feat/integration-trial` includes identity merge
`6398bfa3` as an ancestor; protocol 27. Controller preparation commit `b8149dd4`
changes evidence only. Mesh stayed detached at `fcb9647`. Its disposable compiled
0.153.4 descriptor used `trial`, `enabled: true`, with exact identities
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1` and
`unix-websocket`. The wildcard and all other descriptors remained disabled.
[Scratch descriptor diff](integration/attempt10/mesh-trial-descriptor.diff).
Both checkout-local builds exited **0**: [daemon](integration/attempt10/daemon-build.json),
[Mesh](integration/attempt10/mesh-build.json). Cargo exclusion polled at 30-second
intervals; daemon build waited 60 seconds for a competing build to clear.

Scratch root `/tmp/th-int-yzuf3uvh`, private port **27742**, private tmux socket
`/tmp/th-int-yzuf3uvh/tmux/tmux-1000/default`. The environment removed inherited
`TMUX`; Bubblewrap isolated the PID namespace and hid operator homes and `/run`.
Only the authorized auth file was copied into an initially empty Codex home,
mode 0600; scratch root mode 0700. Both native siblings (`codex` and
`codex-code-mode-host`) were copied from the resolved installation. Their hashes,
the daemon/Mesh/Claude hashes, environment, complete initialize request and
commands are in the losslessly packed [event trace](integration/attempt10/run/events.jsonl)
with [payload dictionary](integration/attempt10/run/event-payloads.json).
No observer connection was opened to the app-server socket. The only host
inspection attempted was the daemon's read-only hosted transcript RPC.

Production initialization used one credential-free Claude lead and one Codex
member, `gpt-5.6-luna`, effort `low`, `delivery: app_server` at creation.
The exact `DEFAULT_CANONICAL_POLICY` was parsed from MeshTeamBuilder's shared
module and sent as canonical messaging [policy](integration/attempt10/run/policy.json).
The scratch project was a git checkout with a short AGENTS.md. The runtime record
and `hosted.instruction_sources.loaded` confirm that instruction source was loaded.
The random reserved marker was `cobaltd495131153`; no marker send occurred.

#### Exact setup refusal and observed launch

The initialize RPC's operation status was `completed`, meaning the operation
finished; its report explicitly carries `failed_step: opt_in_delivery` and
`retryable: true`. It is **not a successful initialization**. The product retains
the selected team root for precisely this refusal in
`src-tauri/src/daemon/initialize_runs.rs`; `finish_initialize` persists the pending
canonical request after seat launch so Retry does not launch twice
(`src-tauri/src/coordination/pipelines/initialize.rs`). The retained fixture was
already removed, so retrying it is no longer possible. A fresh fixture would be
needed to exercise steps 2–7. Exact refusal:

```text
Backend error: mesh team activation failed: error: IO error: delivery: quiescent required before opt-in: IO error: delivery: team owner already holds lifetime lock
```

Before that refusal, `launch_sessions`, `join_mesh` and `start_daemons` reported
success. The retained daemon stderr records `team daemon ensured running`, PID
**679 inside the private namespace**, at `12:39:00.354253Z`. The
[owner epoch](integration/attempt10/run/team/state/delivery/epoch.json) records
that same PID/start identity. [Process identities](integration/attempt10/run/identities.json)
map it to host PID **1140030**, argv `mesh team-daemon start --team integration
--name lead --claude-dir /tmp/th-int-yzuf3uvh/claude`. These observations establish
an owner existed before opt-in. Complete daemon JSONL row 32 also records
`self_heal.pass.completed`, `team_daemons_ensured: 1`, at `12:39:00.354Z`;
row 47 records `coordination.step.failed` / `opt_in_delivery` at
`12:39:02.183Z`. A self-heal/initialization ordering race is the evidence-backed
inference; this lane does not patch or independently reproduce that diagnosis.

The Codex record had `terminalContract: 1`, `health: active`, attachment generation
1 and app-server state `ready`, thread **01a08b54-1471-77a2-899b-f608bf08a8dd**.
The child was namespace PID **126** / host PID **1138406**, start ticks
**26447360**. Its command carried daemon-selected read-only sandbox, never
approval, Luna/low overrides and the Unix socket. The attached TUI was host PID
**1140108**, pane **%2**, `codex --remote unix://… resume <exact-thread>
--no-alt-screen --strict-config`, using the daemon-generated per-member TUI home.
The pane visibly displayed **OpenAI Codex v0.153.4**, **gpt-5.6-luna low** and
`Working (1s)`. The generated config file itself was not retained: initialization
failed before the controller's post-success config capture. The server argv did
not include `--strict-config`; the TUI argv did. The hosted-launch observations stand; the incomplete setup
does not establish an end-to-end transport verdict.

The complete [daemon JSONL](integration/attempt10/run/taurhaus.log.jsonl) retains
**50 rows**, all event families, including scanner, inotify and self-heal rows.
Its digest is in the final audit. `onboarding.delivery.observed` records a baseline
card of **2172 offered bytes**, stage `submitted`, path `app_server`, delivery ID
`7097619faf9c4f80dd2a7cbd420d45e80af59cb3d14578e5df721bf194d9d9f8`.
No `hosted.rpc.*` wire events were emitted in the retained log. The final read-only
hosted transcript attempt returned `HOST_OPERATION_FAILED: host member busy`.
No host event stream was obtained; no fabricated `turn/start` response is claimed.
No Mesh send receipts, explicit reads or holder samples exist for unrun steps;
the retained canonical segment is empty.

#### Every turn and spend

Fresh authorization: **≤16 Codex turns and ≤USD 3**. One automatic startup turn
was observed, with **zero manually submitted turns** and **zero Claude turns**.
No continuation, retry or second paid run occurred. **15 of 16 authorized turn
slots remain**; the budget was not exhausted. This review correction uses
**0 additional turns / $0 additional spend**. Essentially the dollar budget may
remain, but its exact remainder cannot be established without the missing usage;
we do not claim a verified $3 balance.

| Generation | Thread / turn | Input / cached / output | USD |
|---|---|---|---|
| Automatic startup recovery card | Thread `01a08b54-1471-77a2-899b-f608bf08a8dd`; turn `01a08b54-1a52-7971-b02e-39f42cc2edd3` | **Unreported**: only `task_started` was retained | **Unknown, not $0** |
| Claude lead | No model turn | 0 / 0 / 0 | $0 |

[Rollout usage evidence](integration/attempt10/run/usage-events.json) contains
only that start. Neither a `token_count` nor host `thread/tokenUsage/updated`
event was captured before failure teardown. The [final ledger](integration/attempt10/run/cost-ledger.json)
therefore marks metering incomplete, with null total cost and actual billed USD.
Its zero measured subtotal is an empty sum, **not proof of free execution**.
The turn cap is verified; the actual USD cap cannot be verified from this run's
missing usage data. No cost was invented and no additional paid call was made to
fill that gap. The controller's final read-only drain failed immediately with
`host member busy`, then its mandated failure cleanup ended the owned namespace.

#### Reproduction and cleanup

The attempt-9 controller/actions/steps/support were reused with attempt-10 output
paths, the native sibling copy, and stronger step-7 activity assertions. New
offline guards observed red, then green; their synthetic inputs never use a CLI
or credential. The metering finalizer also observed red before preserving missing
usage as unknown. [Tests](integration/attempt10_test.py),
[red](integration/attempt10/red.txt), [green](integration/attempt10/green.txt),
[metering red](integration/attempt10/metering-red.txt),
[metering green](integration/attempt10/metering-green.txt).

Exact controller invocation from the trial checkout root:

```sh
TRIAL_EVIDENCE_LABEL=attempt10 python3 docs/design/evidence/native-eligibility/integration/attempt10-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt10-controller.py attempt10/run
# Actual run exited 1 before inspection_ready; no numbered action was sent.
TRIAL_EVIDENCE_LABEL=attempt10 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt10-audit.py
```

Use a fresh output directory for any newly commissioned replay; do not overwrite
this run. The retained numbered action script is
[integration/attempt10-steps.py](integration/attempt10-steps.py).

[Cleanup](integration/attempt10/run/cleanup.json) and the independent audit verify
all **12 recorded PID/start identities** absent, no scratch daemon, child, TUI,
Mesh owner or private tmux server, port 27742 closed, and scratch root absent.
The historical `auth_removed` field was computed after root deletion and is not
an independent credential check; the audit checks root absence directly.
The controller's `finally` restored `src/delivery/app_server/capabilities.rs` and
removed `target/debug/mesh`; Mesh remains clean and detached at `fcb9647`.
No process outside this run was signaled. No `just install-daemon`, live-daemon
restart, operator tmux contact, or work in another Taurhaus checkout occurred.

**Descriptor flip: none. Mesh commit: none.** The conditional missing-runtime
named-refusal fix and Mesh flip gates are **not run**, because all seven passing
steps are their authorization condition. No Taurhaus registry entry was needed:
the existing launch path demonstrably created the host. No Taurhaus product or
`src-tauri/` file changed, so `just test-rust-unit` is not required.

All unpaid exact gates ran from this checkout root in a separate credential-free
Bubblewrap namespace with inert harness/tmux/Mesh shims. No paid CLI was invoked.
Cargo preflight waited 450 seconds before check-quick and 60 seconds before lint;
contracts needed no additional wait. Each recipe exited **0**:

| Exact command | Exit | Evidence |
|---|---|---|
| `just check-quick` | **0**; 46.24s, typecheck and 150 frontend files / 2509 tests passed | [Exit record](integration/attempt10/gates/gate-check-quick.json), [complete output](integration/attempt10/gates/gate-check-quick.txt) |
| `just lint` | **0**; 37.27s | [Exit record](integration/attempt10/gates/gate-lint.json), [complete output](integration/attempt10/gates/gate-lint.txt) |
| `just test-contracts` | **0**; 9.60s | [Exit record](integration/attempt10/gates/gate-test-contracts.json), [complete output](integration/attempt10/gates/gate-test-contracts.txt) |
| `python3 docs/design/evidence/native-eligibility/integration/attempt10_test.py` | **0**; 3 offline guards | [Output](integration/attempt10/final-tests.txt) |
| `python3 docs/design/evidence/native-eligibility/integration/attempt10-audit.py` | **0** | [Audit](integration/attempt10/final-audit.json) |

[Gate teardown](integration/attempt10/gates/gate-cleanup.json) confirms waited
children and removed scratch root; the independent audit also found no process
carrying the gate namespace's run token. The complete daemon JSONL remains
byte-identical to its pre-audit hash. Non-evidence inserted lines: **0/200**.

**Deviations / limits:** retryable setup was not retried, leaving steps 2–7 unrun; missing startup token usage
prevents a numeric spend total and USD-cap verification; config-file capture and
host transcript were unavailable after initialize failed. The supplied Opus round-1 review is addressed here; no additional review model was run. No product defect was patched.

#### Round-1 correction verification

All seven supplied findings were verified and addressed in the four named files.
The controller restores the descriptor before scratch deletion, records removal
errors without skipping sanitization, and fails cleanup if removal/restoration
is incomplete. It drops the tautological auth flag, resolves the authorized
source with `Path.home()`, and explicitly refuses an exhausted private-port range.
The read-only audit derives cleanliness/metering flags from their checks and
leaves the original runtime artifacts intact. The gate wrapper records its own
scratch root and derives whether its children were waited. The separately named
stale `/tmp/th-int-recheck-gptq2_6a` contained only gate output files; it was removed
and absence verified. That directory was not part of attempt 10’s process audit.

`python3 docs/design/evidence/native-eligibility/integration/attempt10-audit.py --self-test`
first exited **1**: seven tests exposed eight failed assertions and three errors
(the cleanup exception, missing gate-root field and fixed auth-path expression).
After the fixes it exited **0**, all seven tests passing. Tests extract only
isolated controller statements/functions with synthetic files and mocked process,
socket and home interfaces; they never import the paid controller or use a CLI.
The existing `attempt10_test.py` also exited **0**, three tests passing. The
read-only `attempt10-audit.py` exited **0**, verifying the 12 original process
identities absent, private port closed, Mesh clean/disabled with no trial binary,
and the unchanged 50-row daemon JSONL SHA-256
`093938e910b24a7f4b9e25f571bc90490b18a54117d05b42dae252a905b8812e`.

The exact gates were rerun through `attempt2-gates.py`, from this checkout root,
with inert runtime shims and credential-free homes. Final results:

| Command | Exit | Runtime | Cargo preflight wait |
|---|---|---|---|
| `just check-quick` | **0** | 20.95s | 0s |
| `just lint` | **0** | 6.33s | 150s |
| `just test-contracts` | **0** | 5.08s | 30s |

`check-quick` passed 150 frontend files / 2,509 tests; all 68 contract tests
passed. `just test-rust-unit` remains unnecessary: no `src-tauri/` changes.
The first gate set exited 0 / 0 / **101**: the contract scanner read the newly
generated `.check-logs/attempt10-review/gates/gate-isolation.json` as repository
source and flagged its scratch directory name. Moving generated output outside
the checkout resolved that verification contamination without a product patch.
Both the failing and passing logs, command arrays, exit records, Cargo preflights
and gate-root cleanup records are retained locally in
`.check-logs/attempt10-review-gates.tar.gz` (an archive, outside the source scan).
For the passing rerun, `TRIAL_EVIDENCE_LABEL` was
`/tmp/th-int-review-output-quxxon10/rerun`; the controller command was
`python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py`.

Independent cleanup checked both gate roots absent and found no process bearing
either run’s environment identity. After archiving, the temporary output root
`/tmp/th-int-review-output-quxxon10` was removed and absence verified as well.
Only the four named evidence files changed; historical runtime sidecars and
plan ledgers are untouched. The final regression comments identify their
introducing commits via Git blame. This review correction spent **$0 / 0 turns**;
the original one unmetered startup turn remains the only attempt-10 expenditure.

## Attempt 11 — ordered integration trial (2026-09-10)

Preparation: Taurhaus `8ac28c3e` on `feat/integration-trial` contains required
base `1db4f9bf`; protocol 27. Mesh detached explicitly at `ed59187`.
The attempt-10 controller/actions/steps are reused as `integration/attempt11-*`
with a fresh 16-turn / USD 3 cap, zero prior spend, both native Codex siblings,
production canonical initialize, and complete daemon JSONL retention.
Only the authorized credential file is copied into the disposable paid home.
No observer connects directly to the app-server socket.

Offline red/green: `python3 docs/design/evidence/native-eligibility/integration/attempt11_test.py`
first exited 1 (`test_rollback_requires_attributed_idle`: missing native readiness
source was accepted), then 0 (3 tests). The guard now joins the daemon's attributed
PID/tool/session/idle record with the Mesh activity snapshot's `launch_ready` or
`notify` source. Synthetic files and snapshots only; no harness invoked.
See [red](integration/attempt11/red.txt), [green](integration/attempt11/green.txt).
Builds are waiting on the required Cargo preflight. Paid turns so far: 0 / USD 0.
Numbered outcomes, spends, cleanup and gate exits will be appended as observed.

### Attempt 11 runtime result — stopped at step 1

**Eligibility INCONCLUSIVE. Step 1 evidence collection FAIL; production initialize
succeeded.** The reused controller's first `coordination.hosted_transcript` read
returned `{"code":"HOST_OPERATION_FAILED","message":"host member busy"}`.
`poll_host(force=True)` propagated it as fatal before `inspection_ready`; the
read-only final drain received the same refusal, then `finally` tore down the
namespace. This is a controller-confounded result, not proof of a broken launch
or permanent host failure. No paid retry or product patch was made.

| Step | Outcome | S-runtime evidence |
|---|---|---|
| 1. Hosted launch + attached TUI | **FAIL to complete observation; launch succeeded** | [Initialize result](integration/attempt11/run/initialize-result.json): all nine stages succeeded, including `opt_in_delivery`; [runtime](integration/attempt11/run/step1-runtime.json), [strict config](integration/attempt11/run/generated-config-0.toml), [attached pane](integration/attempt11/run/step-1-pane-2.txt), [controller failure](integration/attempt11/controller-output.txt). |
| 2. Idle native send/read | **NOT RUN** | [Outcome](integration/attempt11/run/step2-outcome.json); no marker message submitted. |
| 3. Active-turn deferral | **NOT RUN** | [Outcome](integration/attempt11/run/step3-outcome.json). |
| 4. Typed/socket interleave + passive locks | **NOT RUN** | [Outcome](integration/attempt11/run/step4-outcome.json). |
| 5. Compaction recovery | **NOT RUN** | [Outcome](integration/attempt11/run/step5-outcome.json). |
| 6. Daemon restart | **NOT RUN** | [Outcome](integration/attempt11/run/step6-outcome.json). |
| 7. Operational tmux rollback | **NOT RUN** | [Outcome](integration/attempt11/run/step7-outcome.json). |

Initialize run `init_9ba26e2893bb46af83f079b703416cc7`; private port **26452**.
Thread/session **`01a08c27-ffe9-7951-bc69-8b445d0e6163`**, member pane `%2`,
app-server namespace PID `160`, start ticks `27835987`; attached pane PID `663`,
start ticks `27836620`. The runtime publishes terminal contract 1, build 0.153.4,
Unix WebSocket transport, the three required class identities, and the scratch
AGENTS.md instruction source. The attached pane shows `gpt-5.6-luna low` and
`Working`; a completed reply/recovery-card display was not observed.

The daemon log records `hosted.instruction_sources.loaded` and
`onboarding.delivery.observed` with app-server stage `submitted`, offered bytes
2172 and delivery id
`c62fa30aa67f92270822953d5e017061fc7e1758dba3e047fbfa2e391b6ba148`.
The complete [daemon JSONL](integration/attempt11/run/taurhaus.log.jsonl) is
retained without event-family filtering; no `hosted.rpc.*` event was emitted.
The refusal is the daemon host API's busy result, not a Codex wire error.
Retained rows: **81**; SHA-256
`00a94ee5e093cd19fb35114504e685147a80048e2ef2019a6363f0876459acd2`.

**Every spend:** one automatic startup turn
`01a08c28-0564-7e92-88db-c07dc8336447`, model `gpt-5.6-luna`, effort low.
[Rollout usage evidence](integration/attempt11/run/usage-events.json) contains
`task_started` only; no `token_count` or host `tokenUsage` was retained before
teardown. Input/cached/output tokens and USD spend are **unknown**, not zero.
[Cost ledger](integration/attempt11/run/cost-ledger.json) explicitly uses null
totals and `metering_complete: false`. The observed turn count is **1/16**; the
USD 3 ceiling cannot be verified from this run. There were no manually submitted
model turns, Claude lead turns, compactions, paid retries or review turns.

**Cleanup / pin:** [Controller cleanup](integration/attempt11/run/cleanup.json)
reports no survivors, port closed, scratch root removed, descriptor restoration
exit 0 and trial-enabled Mesh binary removed. [Independent audit](integration/attempt11/runtime-audit.json)
rechecks all 12 recorded host PID/start identities, private port and root absence.
Mesh is clean and detached at `ed59187`; its descriptor remains disabled.
**Descriptor flip commit: none.** The conditional named-refusal fix and Mesh
`just check-quick` / `just lint` / `just test` gates were not run because full PASS
is their prerequisite. No Taurhaus registry entry is needed: the launch path
already created the owned host. No product, dependency, lock manifest or ledger
row was changed.

**Reproduction:** from this checkout root, pin Mesh with
`git -C /home/mstie/projects/mesh-trial checkout --detach ed59187`, then run
`python3 docs/design/evidence/native-eligibility/integration/attempt11-build.py`
and `python3 docs/design/evidence/native-eligibility/integration/attempt11-controller.py attempt11/run`
in fresh output directories. [Build records](integration/attempt11/daemon-build.json)
show 41 Cargo probes (40 busy intervals, approximately 1200 seconds), followed by
daemon build exit 0; [Mesh build](integration/attempt11/mesh-build.json) exit 0.
[Scratch descriptor diff](integration/attempt11/mesh-trial-descriptor.diff) records
the exact trial-only tuple. The controller contains the exact initialize payload,
credential isolation, native sibling copying, strict configuration and cleanup;
[events](integration/attempt11/run/events.jsonl) retain commands and RPC ids.
The numbered `attempt11-steps.py N` action runner never ran because the controller
stopped before its action loop. Rerunning would require a newly commissioned paid
attempt; this report performs no such retry.

**Deviations / limits:** the inherited controller treats the potentially transient
`host member busy` read as fatal rather than retrying the observation, preventing
the ordered seven-step trial from completing. Missing usage prevents numeric
spend and dollar-cap verification. No independent cross-family evidence review
ran inside the paid session; review is performed by the surrounding workflow.
The referenced [integration brief](../../app-server-integration-trial-brief.md#outcome)
requests “One Opus lens on the evidence.”
No product defect was patched. The unpaid gate results follow below.

### Attempt 11 final gates and audit

All exact gates ran from `/home/mstie/projects/taurhaus-trial`, through the reused
`attempt2-gates.py` wrapper with
`TRIAL_EVIDENCE_LABEL=/tmp/attempt11-gate-evidence`. The separate credential-free
Bubblewrap namespace hides operator homes and uses inert harness/tmux/Mesh shims.
No real CLI or credential source was used by the gates.

| Command | Exit | Observed result |
|---|---|---|
| `just check-quick` | **0** | 73.98s; typecheck, Rust test compilation, 150 frontend files / 2,518 tests passed. |
| `just lint` | **0** | 34.91s; all lint lanes passed. |
| `just test-contracts` | **0** | 18.52s; 15 + 20 + 33 = 68 tests passed. |
| `python3 docs/design/evidence/native-eligibility/integration/attempt11_test.py` | **0** | Three offline guards passed again. |
| `python3 docs/design/evidence/native-eligibility/integration/attempt11-audit.py` | **0** | Runtime, pin, redaction, complete log, gate cleanup and PID/start checks passed. |

Exact commands, Cargo preflights and exit records: [check-quick](integration/attempt11/gates/gate-check-quick.json),
[lint](integration/attempt11/gates/gate-lint.json), [contracts](integration/attempt11/gates/gate-test-contracts.json).
Their `.txt` siblings preserve complete output.
[Gate isolation](integration/attempt11/gates/gate-isolation.json) and
[cleanup](integration/attempt11/gates/gate-cleanup.json) record the disposable
namespace and waited children. The final audit independently found no process
bearing its run token; the gate root and temporary evidence-export directory
were removed. [Final audit](integration/attempt11/final-audit.json) verifies the
unchanged 81-row daemon log and all 12 runtime process identities absent.
No `src-tauri/` diff, so `just test-rust-unit` is not required.
Non-evidence inserted lines: **0/200**. Gate/review spend: **0 turns / USD 0**;
the one unmetered startup turn remains the attempt's only expenditure.


### Attempt 11 controller review corrections (unpaid; prepares attempt 12)

The paid result above remains **INCONCLUSIVE**: step 1 observation failed;
steps 2, 3, 4, 5, 6 and 7 were not run. This correction spends **0 additional
model turns / USD 0**, changes no product code, and does not rerun the paid trial.
The single historical startup turn remains unmetered (total USD unknown).
No descriptor flip or Mesh commit was made; eligibility remains disabled.

Confirmed findings and fixes in the retained attempt-11 reproduction scripts:

- `poll_host` now defers only `HOST_OPERATION_FAILED` / `host member busy`, logs
  `host_poll_deferred`, preserves the event cursor, and retries at the existing
  one-second poll interval. All other refusals remain fatal. Step 1 waits up to
  120 seconds with budget checks before failing persistent contention. The final
  read-only drain also skips an absent/stale transcript when its poll defers.
- Step 7 refreshes the retained runtime record on each readiness poll. Null paths,
  absent files and incomplete attribution retry within its existing 120-second
  bound. The subsequent activity evidence uses the record that passed validation.
- The cleanup failure verdict is assigned before sanitizing artifacts, and
  non-UTF-8 files are skipped by the text sanitizer.
- The claimed absence of an Opus requirement was **not confirmed**: the spec
  incorporates the brief, whose Outcome section explicitly requests that lens.
  The attempt-11 paragraph now cites that source and distinguishes the paid
  session from the surrounding review workflow.

Offline TDD: six added regression tests use AST-extracted controller code,
synthetic RPCs, a virtual clock and temporary files; no controller imports,
credentials, real CLI, daemon, Mesh or tmux process. Before implementation, five
failed: fatal busy RPC, missing bounded-wait helper, absent final-drain transcript,
null rollback path, and non-UTF-8 sanitize decode. The negative-refusal guard passed.
After implementation all **9 tests passed (exit 0)**. Regression comments name
`15633a94`, which introduced these attempt-11 scripts.

[Recaptured red](integration/attempt11/red.txt) now includes the exact current
`TrialGuards` test with only the native readiness assertion removed (exit 1),
then the current full suite against both controller scripts from `15633a94`
(exit 1; one failure and four errors). Both temporary mutations were restored in
`finally`; the current suite passed again. The original three-test
`green.txt` remains historical runtime-preparation evidence. Reproduce current
green with `python3 docs/design/evidence/native-eligibility/integration/attempt11_test.py`.

Only the named controller, steps, tests, red capture and this evidence file change.
The complete historical daemon JSONL and runtime/cost/cleanup sidecars are untouched.
This repair prepares a newly commissioned attempt 12; it establishes no new
S-runtime proof. The unpaid gate rerun results follow below.


#### Controller review gate rerun and cleanup

All exact commands ran from `/home/mstie/projects/taurhaus-trial` through the
unchanged `attempt2-gates.py` wrapper, with
`TRIAL_EVIDENCE_LABEL=/tmp/attempt11-review-gates-rPmI8E`.

| Command | Exit | Observed result |
|---|---|---|
| `just check-quick` | **0** | 19.40s; Rust tests compiled, typecheck passed, 150 frontend files / 2,518 tests passed. |
| `just lint` | **0** | 19.72s; all lint lanes passed. |
| `just test-contracts` | **0** | 5.43s; 15 + 20 + 33 = 68 tests passed. |

Complete gate logs and command/exit JSON records remain under
`/tmp/attempt11-review-gates-rPmI8E/gates/`. Cargo preflight waited one 30-second
interval before check-quick; both later preflights found no competing Cargo.
The wrapper exited 0, waited all three children and removed its credential-free
scratch root `/tmp/th-int-gates-0y50_kp1`. An independent `/proc` check found no
process carrying that gate run token. Operator homes were hidden and external
harness/tmux/Mesh commands shadowed by inert shims. No paid runtime was started.
No Rust diff, so `just test-rust-unit` was not required. `git diff --check` passed.
The historical 81-row daemon JSONL still hashes to
`00a94ee5e093cd19fb35114504e685147a80048e2ef2019a6363f0876459acd2`.
Additional gate spend: **0 turns / USD 0**. No Mesh gate or descriptor decision
was re-executed; the recorded disabled descriptor and inconclusive verdict stand.

## Attempt 12 — controller prepared (2026-09-10)

Reuses the review-fixed attempt-11 controller from `19ffe981` / `3a3d2517`,
against production base `1db4f9bf`, protocol 27, with Mesh detached at `ed59187`.
Fresh hard budget: 16 Codex model turns / USD 3; previous attempts excluded.
The sole controller behavior change admits exactly both `HOST_OPERATION_FAILED`
messages `host member busy` and `Conflict: host operation deferred: lock busy`
as transient reads, keeping the cursor and one-second backoff inside the 120-second
startup / 100–120-second step windows. All other refusals remain fatal.

Offline red: three errors across the new flock/cursor and deadline subtests;
green: all 11 tests pass. Synthetic data only, AST-extracted helpers, no live
controller import or harness invocation. See [red](integration/attempt12/red.txt),
[green](integration/attempt12/green.txt), [tests](integration/attempt12_test.py),
and [controller](integration/attempt12-controller.py). Gate logs will be retained
under `integration/attempt12/gates/`. No Taurhaus product change.

### Step 1 — PASS (S-runtime)

Production initialize `init_de9fb0e2a2534574bbc1425e9694ff23` passed all nine stages.
Owned thread `01a08c4e-36ef-7b81-8d5f-152614676d07`; strict config, loaded AGENTS.md,
Unix socket and attached pane `%2` verified. Startup card completed and visible.
Evidence: `attempt12/run/initialize-result.json`, `step1-runtime.json`,
`step1-identities.json`, `step1-final-pane-2.txt`, `host-events.jsonl`, and the full
`taurhaus.log.jsonl`. One metered startup turn: $0.00097004 API-equivalent,
$0.01296120 conservative. Both checkout-local builds exited 0.

### Step 2 — PASS (S-runtime)

Idle `mesh send` produced one native turn, marker in attached pane and model reply.
`step2-receipts.json` retains `native_enqueued` IDs; `step2-journal-before-read.json`
and `step2-journal-after-read.json` prove `consumed_by_read` appeared only after
`step2-explicit-read.txt`. Second turn $0.00057968; cumulative two turns
$0.00154972 API-equivalent / $0.02716800 conservative.

### Step 3 — PASS (S-runtime)

A send during the bounded active turn recorded `pending: thread_active`, then
`turn/start` after idle. `step3-pending.json`, `step3-receipts.json`, and
`step3-exposure.json` prove one user exposure and one reply.
Timing turn $0.00099976; deferred delivery $0.00052736; cumulative four turns
$0.00307684 API-equivalent / $0.05700600 conservative.
