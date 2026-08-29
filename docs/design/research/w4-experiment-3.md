# W4 experiment 3 — a managed Codex member completes a bounded task through the assignment contract (2026-08-29)

Experiment 3 of [`../w4-managed-stages-design.md`](../w4-managed-stages-design.md): does a *stage* — an assignment to a managed member — actually carry a bounded implementation task end to end, with the assignment's effort in force before the member reads it? It ran live on the development host against a real Codex subscription. It does, and the whole gate costs about a second.

The lane is `e2e/specs/managed-stage-codex.js`. It is paid, so `e2e/specList.js` keeps it out of the default WDIO spec list and it only runs when named.

## Commands

```bash
# once, if the installed mesh is not the locked build (see "Host state" below)
just install-mesh

# the lane (builds the E2E binary by default; E2E_SKIP_BUILD=1 only when it is known fresh)
E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex
```

The measured run used `E2E_SKIP_BUILD=1` on a binary built by the immediately preceding run of the same recipe. Nothing else was passed; the isolated roots, the fixture project and the scratch `CODEX_HOME` are all created by the lane and its wdio config.

## Setup as measured

| | |
|---|---|
| Host | Linux (WSL2), app + daemon 0.8.3, daemon protocol 14 |
| mesh | 0.2.23 (`806258f`), both on `PATH` and at `~/.local/bin/mesh` |
| Codex | 0.150.1, `gpt-5.6-sol`, launched at effort `low` |
| Team | Claude lead `e2e-lead` (launch-new, isolated credential-free `CLAUDE_CONFIG_DIR`, never took a turn) + one managed Codex member `codex-stage` |
| Project | throwaway git repo under the wdio session temp root: `package.json`, one Svelte file, one commit |
| Assignment | `--effort medium --why "experiment 3: bounded slice"`, first step / deliverable / completion signal as the design specifies |
| Deliverable | one commit adding `src/lib/greet.js` (exported `greet(name)`) and `src/lib/greet.test.js`, `bun test` passing |

## Measured

Every duration below comes from a record, not from the test process's own stopwatch: the hold from mesh's attention projection (`assignedAt` → `deliveredAt`), the resume from the app's `effort.resume.started` / `.completed` events, the member's time from `deliveredAt` → the `RESULT` message's own timestamp.

| Phase | Duration | Source |
|---|---:|---|
| Hold — assignment to notice delivery, spanning the whole effort switch | **1.23 s** | `assignedAt` → `deliveredAt` |
| ↳ of which: stop + relaunch + reattach | 0.97 s | `effort.resume.started` 15:44:56.523Z → `.completed` 15:44:57.497Z |
| ↳ of which: assignment to `appliedEffort = medium` observed | 1.47 s | task assign → runtime record poll (1 s poll granularity) |
| Member wall clock — notice delivered to `RESULT` | **43.67 s** | `deliveredAt` → `RESULT` message timestamp |
| Total — assignment to `RESULT` | **44.90 s** | `assignedAt` → `RESULT` message timestamp |
| Second assignment, no mismatch: assignment to delivery | 0.59 s | `assignedAt` → `deliveredAt` |

Two `it`s, both passing, 3 m 10 s wall clock for the spec including team setup and teardown.

### What the records said

```
launch  {"event":"launch.command.rendered","tool":"codex","reasoning_effort":"low",
         "command":"CODEX_HOME='…/codex-home' codex --yolo --dangerously-bypass-hook-trust
                    -c 'notify=[…]' -m 'gpt-5.6-sol' -c 'model_reasoning_effort=\"low\"'"}
15:44:56.523Z {"event":"effort.resume.started","member_name":"codex-stage","effort":"medium","previous_effort":"low"}
15:44:56.608Z {"event":"launch.command.rendered","tool":"codex","reasoning_effort":"medium",
               "command":"CODEX_HOME='…/codex-home' codex resume '01a04e30-2ab9-7d93-8d83-afa2a58c69fe'
                          --yolo --dangerously-bypass-hook-trust -c 'notify=[…]'
                          -m 'gpt-5.6-sol' -c 'model_reasoning_effort=\"medium\"'"}
15:44:57.497Z {"event":"effort.resume.completed","member_name":"codex-stage","effort":"medium","previous_effort":"low"}
```

The member's completion signal, in the lead's inbox:

```
15:45:05.911Z  [mesh] codex-stage accepted task #1: "Add greet(name) to the stage fixture"
15:45:05.940Z  [mesh] codex-stage started task #1 (Adding greet(name) to the stage fixture)
15:45:41.300Z  RESULT #1 {"commit":"f9370e722d8991eb9e7690c35daaba46e376e637",
                          "files":["src/lib/greet.js","src/lib/greet.test.js"],
                          "validation":"bun test passed"}
```

The lane does not take that last line on trust: `f9370e7` was verified to be a commit object in the fixture repo, and `bun test` was re-run there by the lane and passed.

## What this establishes

1. **The contract carries a real slice.** Create → assign → notice → accept → start → commit → `RESULT` with a machine-readable payload worked first time, with no steering, on a task the member had never seen. The member also used `mesh task accept` / `task start` on its own, which the assignment did not ask for — the mesh onboarding is enough for a member to drive the task lifecycle.
2. **The effort gate is real and effectively free.** mesh reported `pendingEffort: true` and `deliveryState: "pending"` for the assignment while the member ran at `low`; taurhaus stopped it, resumed the *same* conversation with `-c model_reasoning_effort="medium"`, `appliedEffort` caught up, and mesh delivered. The hold cost 1.23 s of a 44.9 s stage — under 3%.
3. **The gate acts on a mismatch, not on every assignment.** A second assignment at `medium`, with the member already there, reported `pendingEffort: false` from the first read, was delivered in 0.59 s, and produced no `effort.resume.*` event at all.
4. **Delivery followed the switch, so no effort wait expired.** taurhaus spawns the member daemon with `Stdio::null` (`coordination/runtime/process.rs`), so mesh's own `effort wait expired for …` line reaches nobody; the lane asserts the observable equivalent instead — `deliveredAt` at or after `effort.resume.started`, with `deliveredAt` null while `pendingEffort` was still true. An expired wait delivers during the hold, which is exactly what did not happen.
5. **43.7 s of member time for one function and one test** is the number to compare the `codex exec` babysitter against. It is a floor, not a typical stage: the slice was deliberately tiny and needed no installs.

## Cost

The lane spends Codex subscription turns and nothing else.

- **Claude: zero.** The lead is launched into the isolated, credential-free `CLAUDE_CONFIG_DIR`, so it never authenticates and never takes a turn. It exists as a mesh identity and an inbox, which is all the completion signal needs.
- **Codex: four turns** on `gpt-5.6-sol` — two before the assignment (the onboarding message plus the one-word reply that opens the thread, see below), one for the stage itself at `medium`, one for the second assignment's no-op acknowledgement.
- **Token counts were not captured.** The lane does not read Codex's usage endpoint: that reports the whole account, not this run, and reading it would mean handling a live credential for a number that would not be attributable anyway. Turns, model and effort are the honest unit here.
- A run that fails before the member's first turn — as the first attempt did — costs nothing at all.

## Two findings for the design

**The onboarding notice can sit unsent in the composer, and a member that has taken no turn has no conversation to resume.** Measured twice on this host: the member's mesh daemon types the onboarding notice into the Codex pane and sends its own submit key, and when that lands while Codex is still starting the text stays in the composer. Codex opens its thread — and writes the rollout under `$CODEX_HOME/sessions` — on its *first turn*, so a member parked like this has no session id, and `pending_member_effort` refuses the effort switch outright ("member has no recorded session to resume"). The first run of this lane died there, before spending anything.

A bare `Enter` did not submit it; typing a one-word prompt and pressing Enter did, submitting both. The lane now does that and fails loudly if neither works. **For W4 this is a `stage()` concern, not a test concern**: a stage must not assign work to a member that has never taken a turn, because the effort it asks for cannot be put into force. The cheapest fix is for `stage()` to require a member whose runtime record names a session, and to prompt it once if it does not.

**Session binding under a scratch `CODEX_HOME` works.** The lane carries a documented fallback that reads the rollout id out of the scratch home itself if the scanner has not bound one; the measured run reported `sessionBinding: "scanner"`, so the fallback was not used. `session_scanner/idle/codex.rs` resolves `$CODEX_HOME`, and the id arrived within seconds of the first turn.

## Host state worth recording

`~/.local/bin/mesh` was replaced with a stale bundled 0.2.22 build partway through this work (an app startup installing a bundled mesh whose binary did not match its own manifest). On 0.2.22 the gate does not exist and `mesh task get --json` simply omits `pendingEffort` — the lane would have failed on an `undefined` rather than on a named cause. It was restored with `just install-mesh`, which is lock-verified.

Because `coordination/mesh_cli.rs` resolves `~/.local/bin/mesh` by *absolute path* rather than through `PATH`, the mesh that holds the notice can be a different build from the one a test calls. The lane's host check therefore version-checks both and names the fix.

## Limits

- One run, one host, one model (`gpt-5.6-sol`), one deliberately tiny slice. The wall clock is a floor; nothing here says what a feature-sized stage costs.
- The third `it` in the spec is the skip recorder; wdio's `afterTest` treats a `this.skip()` as not-passed and collects a failure-artifact bundle for it. A green run therefore still prints one `failure artifacts collected` line. That is the shared wdio config's behaviour, not a failure.
- Deadline and nudge semantics (experiment 4) and concurrent stages (experiment 5) are untouched here.
