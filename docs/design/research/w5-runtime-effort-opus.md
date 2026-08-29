# W5 — Runtime reasoning effort per harness, and where a task-level `effort` would travel

Researcher 1 of 2 (Opus lane). Scope assigned: **Claude Code and Antigravity first, then the taurhaus
delivery path.** Codex and Grok are researcher 2's lane; what appears here for them is a cross-check
only and is labelled as such.

Read-only run. No file in any repository was modified, no git write command was run, no CLI was
started interactively (`--help` / `--version` only), and nothing under `~/.claude*`, `~/.codex`,
`~/.gemini` or `~/.grok` was written. Binaries were inspected with `strings` + offset extraction into
the session scratchpad.

Host date 2026-08-29. Checkout `/home/mstie/projects/taurhaus`, branch `feat/compaction-hooks-e2e`.

> **Path note.** The assignment named the report path
> `docs/design/research/docs/design/research/w5-runtime-effort-opus.md` (the segment is duplicated).
> The file was written at exactly that path so the caller finds it where it asked; the natural home is
> `docs/design/research/w5-runtime-effort-opus.md` and moving it is a plain `git mv`.

---

## Result

### The one-line answer per harness

| Harness | Version verified on host | Change effort in an ALREADY RUNNING interactive session? | Mechanism |
|---|---|---|---|
| **Claude Code** | `2.1.251` | **Yes** | `/effort <low\|medium\|high\|xhigh\|ultracode\|auto>` typed into the pane. Also `apply_flag_settings` over the Remote Control bridge (not wired in taurhaus). |
| **Antigravity (`agy`)** | `1.1.22` | **Yes** | `/effort <low\|medium\|high>` typed into the TUI (also reachable through the `/model` picker's effort gauge). No non-interactive form. |
| **Codex CLI** | `0.150.1` | *researcher 2's lane* — no `--effort` flag exists; effort is `-c model_reasoning_effort=…` at launch. A runtime path was **not** established in this lane. | see "Cross-check" below |
| **Grok CLI** | `1.0.13` | *researcher 2's lane* — no `/effort` in the documented slash-command table; `--reasoning-effort` / `--effort` is a launch flag. | see "Cross-check" below |

### The three things that make Claude Code's runtime path non-obvious

1. **`/effort <level>` in an interactive terminal also writes the user's settings.** It is not a
   session-scoped knob when typed by a human — the same code path persists
   `modelSettings[<model>].effortLevel` into `userSettings` and prints
   *"(saved as your default for new sessions)"*. Driving `/effort` into a member pane would therefore
   mutate the operator's own default. **Verified in the bundle** (see E-C6).
2. **A launch-effort *pin* exists for exactly three model families** — `opus-4-7`, `opus-4-8`,
   `fable-5`. While the pin holds, a *non-interactive* effort change is refused with
   *"Not applied: the launch-effort pin holds effort at X this session. Run /effort Y in an
   interactive terminal to release the pin."* An interactive `/effort` releases it. **Verified** (E-C7).
3. **`CLAUDE_CODE_EFFORT_LEVEL` outranks everything and cannot be changed at runtime.** It is read
   from the process environment, so for an already-running session it is frozen; if taurhaus ever set
   it at launch, `/effort` would become a no-op that only prints a refusal. **Verified** (E-C8).
   taurhaus does not set it today (E-T11).

### Where a task-level `effort` + `why` would travel in taurhaus

The short version: **not in the task record** — taurhaus does not author tasks, it scans them. The
authored assignment surface is the *operator notice* delivery, and it already has a structured,
persisted, non-rendered side-channel built for exactly this shape of field.

```
frontend / team-lead
      │
      ▼
DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
    member_name, team_name, message, sender_name,
    operational_context: Option<OperationalContextUpdate> {
        task:              OperationalTaskContext { id, subject, status },
        assignment_footer: OperationalAssignmentFooter {          ◄── the assignment contract
            execution_mode, file_ownership_boundary,
            adjacent_fix_policy, validation_expectation,
            response_expectation,
            //  ← `effort` + `effort_rationale` belong here
        },
        ownership, working_set,
    },
})
      │
      ├── message text ──► DeliveryRenderer::render_operator_notice
      │                    "[taurhaus] operator_notice from {team}: {message}"
      │                          │
      │                          ▼
      │                    MeshBridgedBackend::send_operator_notice
      │                    → MeshInboxMessage { id, from, text, timestamp, read,
      │                                         summary:"operator_notice", …, extra ◄── flatten map }
      │                    → teams/<team>/inboxes/<member>.json   (DeliveryMethod::InboxFile)
      │
      └── operational_context ──► operational_context::apply_delivery_context
                                  → OperationalContextSnapshot
                                  → teams/<team>/state/operational/<member>.json
```

Three candidate homes, in order of fit:

| Home | File | Fit |
|---|---|---|
| **`OperationalAssignmentFooter`** (+ its `…Snapshot` twin) | `requests.rs:62`, `stores/operational.rs:24` | **Best.** It *is* the assignment contract, it is already persisted per member, and `apply_delivery_context` writes it "without parsing the message" — a structured field, not prose. Needs two new fields on two mirrored structs. |
| `MeshInboxMessage.extra` | `stores/inbox.rs:40` | Good for the *wire*: `#[serde(flatten)] extra: BTreeMap<String, Value>` round-trips unknown mesh-owned keys, so an `effort` key survives a mesh round trip with no schema change. But it is per-message, not per-assignment state. |
| `Member.reasoning_effort` | `domain.rs:48` | **Wrong altitude.** This is the *launch* effort, consumed by `LaunchSpec::render()`. Overwriting it per task would silently redefine the member's next launch. |

`ScannedTask` (`task_scanner/types.rs:29`) is a **read-only projection** of each harness's own task
store — it has no `effort`, no `why`, and no writer. It must not grow one.

### What the UI shows today

- **Mesh runtime node** (`MeshNode.svelte`): the model string only — **no effort**.
- **Node detail** (`MeshNodeDetail.svelte`) and **AgentCard**: `model · effort` (e.g. `opus · high`).
- **Nothing** anywhere renders the assignment footer, and nothing renders a per-task effort. A
  task-level effort would be a new surface in both places.

### The delivery-time gap

taurhaus applies model and effort **only at launch** (`LaunchSpec::render()`), and the only text it
ever pushes into a live pane is a launch command. The primitive to do better already exists and is
already exercised: `send_tmux_keys_with_enter(pane_id, keys)` → `tmux send-keys -t <t> -l <keys>`,
delay, `Enter` (`runtime/system.rs:157`). Making effort per-task means calling that existing method
with `/effort <level>` before the notice, for the two harnesses that accept it.

---

## Evidence

Every claim below carries the command output, file:line, or extracted binary offset it rests on.
`VERIFIED` = observed directly this session. `INFERRED` = read from code/docs but not executed.

### Claude Code 2.1.251

**E-C1 — version and binary.** VERIFIED.
```
$ claude --version
2.1.251 (Claude Code)
$ readlink /home/mstie/.local/bin/claude
/home/mstie/.local/share/claude/versions/2.1.251
$ file /home/mstie/.local/share/claude/versions/2.1.251
ELF 64-bit LSB executable, x86-64, … not stripped   (205 MB, bun-compiled JS bundle)
```
Strings extracted once to `…/scratchpad/claude.strings` (44 MB); byte offsets below are into that file.

**E-C2 — `--effort` is a launch flag.** VERIFIED, `claude --help`:
```
  --effort <level>                      Effort level for the current session
                                        (low, medium, high, xhigh, max)
```
And `--model <model>` on the same help page. There is no `--set-effort`, no runtime subcommand, and
no `claude effort` command in the `Commands:` list.

**E-C3 — `/effort` exists as a slash command.** VERIFIED. Telemetry event name `tengu_effort_command`
and usage string `Usage: /effort <…|auto>` are both in the bundle.

**E-C4 — the `/effort` command body.** VERIFIED, `claude.strings` byte 27591847:
```js
async function f(s,t){
  let e=s.trim(), o=t.getAppState(),
      r=Dt(o.mainLoopModelForSession??o.mainLoopModel??eb());
  if(_I.includes(e))return{type:"text",value:Vnt()};                       // help aliases
  if(e==="current"||e==="status"){let{message:a}=Sje(tu(t),r,o.ultracode); // report only
    return{type:"text",value:a}}
  if(!e)return{type:"text",value:`Usage: /effort <${j9(r).join("|")}${Fb(r)?"|ultracode":""}|auto>`};
  return{type:"text",value:(await hse(e,t.setAppState,!t.options.isNonInteractiveSession,t.storageV5)).message}}
export{f as call};
```
Reading: a **bare `/effort` prints usage, it does not open a picker in this path**; `/effort current`
reports; `/effort <level>` applies. The third argument to `hse` — the persist/pin-release flag — is
`!isNonInteractiveSession`, i.e. **true in a tmux pane**.

**E-C5 — accepted levels.** VERIFIED, same chunk:
```js
function UJt(t,o){let n=t.toLowerCase();
  if(n==="auto"||n==="unset")return{value:void 0};
  if(n==="ultracode"&&Fb(o))return{value:"xhigh"};
  let r=UDe(t);return r?{value:r}:null}
function E(t){let o=j9(t),n=Fb(t)?", ultracode":"";return `${o.join(", ")}${n}, auto`}
```
So `low|medium|high|xhigh` plus `ultracode` (gated on `Fb(model)`) plus `auto`/`unset`. Note `max`
appears in the `--effort` help line (E-C2) but not in this validator — the launch flag and the slash
command do not have byte-identical vocabularies. INFERRED: `max` is normalised elsewhere; not chased.

**E-C6 — interactive `/effort` persists to user settings.** VERIFIED, byte 15140567 and 15139292:
```js
async function Izt(e,o,t){return rn("userSettings",K(o,e),void 0,t)}
async function W9(e,o,t=!0,r){let u=e!==void 0?z9(e):void 0;
  if(t&&(e===void 0||u!==void 0)&&!Wr()){let f=await Izt(u,o,r); if(f.error)return f.error}
  if(t)$m(r); return}
function K(e,o){let t=S8e(e);
  return Object.hasOwn(Object.prototype,t)?{effortLevel:o}:{modelSettings:{[t]:{effortLevel:o}}}}
```
and the user-facing string, byte 27583305:
```js
let v = i!==void 0 && o && !Wr() ? " (saved as your default for new sessions)" : " (this session only)";
```
`o` is the same persist flag as E-C4's third argument. INFERRED (naming convention, not executed):
`rn("userSettings", …)` writes `~/.claude/settings.json`. **This is the one real side effect of
driving `/effort` from taurhaus** — settle it by watching the file mtime during a manual `/effort`
in a scratch `CLAUDE_CONFIG_DIR`, which this read-only lane did not do.

**E-C7 — the launch-effort pin.** VERIFIED, byte 15137529 and 27583305:
```js
function RM(e){let o=Xe(e);
  if(o.includes("opus-4-7"))return!oe().unpinOpus47LaunchEffort;
  if(o.includes("opus-4-8"))return!oe().unpinOpus48LaunchEffort;
  if(o.includes("fable-5")||zL(e))return!oe().unpinFable5LaunchEffort;
  return!1}
…
if(!o&&RM(f))return{message:`Not applied: the launch-effort pin holds effort at ${B0(f)} this session. `
                          +`Run /effort ${IM(e)} in an interactive terminal to release the pin.`, …};
```
and the release, byte 15139567:
```js
function $m(e){Ae((o)=>o.unpinOpus47LaunchEffort&&o.unpinOpus48LaunchEffort&&o.unpinFable5LaunchEffort
  ? o : {...o,unpinOpus47LaunchEffort:!0,unpinOpus48LaunchEffort:!0,unpinFable5LaunchEffort:!0}, e)}
```
The pin is scoped to those three model families and is bypassed whenever `o` (interactive) is true.
Resolution order is in `_w`:
```js
function _w(e,o,{honorLaunchPin:t=!0}={}){ if(!ag(e))return; let r=t&&RM(e), u=A(e), f=uR();
  if(f===null&&!r)return; return _(f ?? (r?u:void 0) ?? o ?? u, e)}
```
→ **env var > launch pin > session/settings**.

**E-C8 — `CLAUDE_CODE_EFFORT_LEVEL` wins and is frozen per process.** VERIFIED, byte 15137529 region
and 27583305:
```js
function uR(){let e=a.CLAUDE_CODE_EFFORT_LEVEL;
  return e?.toLowerCase()==="unset"||e?.toLowerCase()==="auto" ? null : CC(e)}
…
let d=Wr()?void 0:uR();
if(d!==void 0&&d!==e){ …
  return{message:`CLAUDE_CODE_EFFORT_LEVEL=${L} overrides this session — clear it and ${IM(e)} takes over`, …}}
```
`a` is the process env object, read per call but never re-sourced from outside the process, so for a
running session the value is fixed at exec time. VERIFIED that it is a recognised env key (it is in
the bundle's `CLAUDE_CODE_*` export list and in the prompt-cache-affecting key list).

**E-C9 — `/effort` runs immediately, it does not queue behind the turn.** VERIFIED as a changelog
line in the bundle, byte 23678362:
> "Changed `/model`, `/fast`, and `/effort` to also run immediately instead of queueing until the
> turn ends on Bedrock, Vertex, and Foundry and when telemetry is disabled"

INFERRED from the word *also*: on the ordinary first-party path immediacy was already the behaviour,
and this entry extended it to the three cloud providers. Not executed.

**E-C10 — Remote Control can change a running session's effort programmatically.** VERIFIED, byte
25040616 (handler) and 9777480 (error strings):
```js
function ist(e,{model:r,getAppState:o,setAppState:E,storageV5:v}){
  if(!_Wt())return f("bridge_flag_settings","disabled"),
    {ok:!1,error:"apply_flag_settings: effort changes over Remote Control are turned off"};
  let a,l=!1;
  if("effortLevel"in e){ if(l=!0, e.effortLevel!=null){
      let t=CC(e.effortLevel)??BDe(e.effortLevel);
      if(typeof t!=="string"||!jk(t))return …{ok:!1,error:"apply_flag_settings: unrecognized effortLevel"};
      a=MF(t,r)}
    let s=uR(); if(s!==void 0&&a!==s)return …
      {ok:!1,error:"apply_flag_settings: CLAUDE_CODE_EFFORT_LEVEL overrides effort for this session"}}
  …
  E((s)=>{ let t=s; if(l){let g=G9(a); if(!ZW(t.sessionEffort,g)) t={...t,sessionEffort:g}} … return t});
```
plus the allowlist `var Ae=new Set(["effortLevel","ultracode"])` and the refusal string
`"… cannot be changed over Remote Control (only effortLevel and ultracode can)"`.

Two consequences worth noting: this handler **never consults `RM`**, so Remote Control overrides the
launch pin that a non-interactive local caller cannot; and it **does** honour
`CLAUDE_CODE_EFFORT_LEVEL`. It writes `sessionEffort` directly and does not persist to user settings.

**E-C11 — editing `settings.json` mid-session probably does NOT change effort.** INFERRED (code read
only). The settings-changed handler, byte 24344771:
```js
function Lnt(t,o,r,i){ let l=Je(); n(`Settings changed from ${t}, updating app state`); …
  if(o((k)=>{ …
    if(k.settings.effortLevel!==l.effortLevel||S(k.settings.modelSettings)!==S(l.modelSettings))$m(r);
    return{...k, settings:l, toolPermissionContext:R, …}}), p)uJt.emit()}
```
It refreshes `settings` and calls `$m(r)` — which by E-C7 only *releases the launch pin* — but it does
**not** recompute `settingsEffortTable`, the table `il()` resolves a `{kind:"inherit"}` session
against. A full sweep of `settingsEffortTable` (12 occurrences) shows it assigned in exactly two
places: the initial app-state literal `sessionEffort:{kind:"inherit"}, settingsEffortTable:{default:void 0,byModel:{}}`
(byte 24323491) and the startup derivation `jDe(e,o){… let r={sessionEffort:Y(t),settingsEffortTable:J()}; …}`
(byte 15140876). **Settle it by** launching a throwaway session under a scratch `CLAUDE_CONFIG_DIR`,
editing `effortLevel`, and running `/effort current` — deliberately not done here (write to a
`~/.claude`-shaped dir).

**E-C12 — resume carries `--effort`.** INFERRED from E-C2 + the help text for `-r, --resume`: `--effort`
is documented as "for the current session", and a resume creates a session, so
`claude --resume <id> --effort high` sets it. Not executed. This is a *restart*, not a live change.

### Antigravity CLI 1.1.22

**E-A1 — version and binary.** VERIFIED.
```
$ agy --version
1.1.22
$ file /home/mstie/.local/bin/agy
ELF 64-bit LSB pie executable, x86-64, … stripped   (208 MB, Go)
```
Strings extracted to `…/scratchpad/agy.strings` (35 MB).

**E-A2 — `--effort` is a launch flag with three levels.** VERIFIED, `agy --help`:
```
  --effort                        Reasoning effort for the current CLI session (low|medium|high)
  --model                         Model for the current CLI session
```
The same sentence appears verbatim in the binary's string table (byte 20147714), confirming it is the
flag's registered usage text and not a shell artefact. Note the vocabulary is **narrower than
Claude's** — three levels, no `xhigh`/`max`.

**E-A3 — `/effort` is a real slash command that changes effort live.** VERIFIED, two independent
sources on this host.

Source 1 — the shipped changelog, `/home/mstie/.gemini/antigravity-cli/cache/CHANGELOG.md`:
- line 281 (release 1.1.5): *"Added a `/effort` command to view and change the current model's
  reasoning effort, with a left/right timeline-gauge picker and a direct `/effort <level>` form so you
  can trade latency for depth on the fly."*
- line 282: *"Added an `--effort` flag to select a model's reasoning-effort variant when launching the CLI."*
- line 285: *"Redesigned the `/model` picker to group models by their base model and choose reasoning
  effort from a timeline gauge navigable with Left and Right, and added an effort badge to the status
  line for models that expose multiple effort variants."*
- line 10 (current release): *"Improved the `/effort` hint so it completes what you have actually typed
  instead of always showing a fixed `[low|medium|high]` placeholder."*

Source 2 — Go symbols and UI strings in the binary:
```
third_party/jetski/cli/commands/effort.go
third_party/jetski/cli/model/effort_selector.go
third_party/jetski/cli/backend/effort.go
commands.(*effortCommand).Name / .Description / .Execute / .Execute.func1
backend.ValidEfforts / IsValidEffort / effortIndex / availableEfforts / ResolveEffort
backend.CurrentBaseEffort / EffortsForBase / ModelForBaseEffort / CurrentEffortSelection
store.(*Manager).SetRequestedEffort / store.ModelEffortDisplay / store.stripEffortSuffix
"Set the reasoning effort"        ← the command's Description (byte 19219337)
"Exited /effort command"          ← the picker's dismissal string
"/effort"                         ← the command token (byte ~ same blob)
```
The **direct `/effort <level>` form** (changelog line 281) is what matters for automation: it avoids
the interactive gauge entirely.

**E-A4 — effort is a model *variant*, not an orthogonal parameter.** VERIFIED from the symbol set in
E-A3: `ModelForBaseEffort`, `EffortsForBase`, `stripEffortSuffix`, `ModelEffortDisplay`. Antigravity
resolves `(base model, effort)` to a concrete model slug. Practical consequence: a member's effort and
model are not independently settable — some base models expose one effort only ("models that expose
multiple effort variants", changelog line 285), and taurhaus's `ModelCatalog::supports_effort` gate
already models this.

**E-A5 — print mode `/effort` is read-only.** VERIFIED, CHANGELOG.md line 188:
> *"Added non-interactive answers for the read-only slash commands in print mode, so `-p "/usage"`,
> `/quota`, `/credits`, `/model`, `/effort` and `/skills` emit one tab-separated record per line — or a
> structured payload under `--output-format json` and `stream-json` — without starting an agent turn,
> spending quota, or leaving a conversation behind."*

So `agy -p "/effort"` **reports**; it is not a way to set effort, and it does not touch the running
session anyway. **The interactive TUI is the only runtime write path.**

**E-A6 — no effort environment variable.** VERIFIED by absence: `ANTIGRAVITY_EFFORT`, `GEMINI_EFFORT`
and `JETSKI_EFFORT` all return zero hits in `agy.strings`. `requestedEffort` (byte 26961450) appears
in a run of Go struct-field names next to `generationStart`, `cachedMcpStates`, `deferredOutputs` —
in-memory session state, not a config key.

**E-A7 — persistence of a runtime `/effort` is unresolved.** UNVERIFIED. `store.(*Manager).SetRequestedEffort`
with a `.deferwrap1` (a deferred mutex unlock) is consistent with either an in-memory-only setter or a
setter that also flushes settings. The two settings files on this host carry no model or effort key:
```
$ cat ~/.gemini/settings.json
{ "security": { "auth": { "selectedType": "oauth-personal" } } }
$ cat ~/.gemini/antigravity-cli/settings.json
{ "colorScheme": "solarized dark", "enableTelemetry": false, "trustedWorkspaces": [ … ] }
```
Settle it by running `/effort high` in a scratch `HOME` and diffing `antigravity-cli/settings.json` —
not done, because it writes a `~/.gemini`-shaped tree.

**E-A8 — no compaction hook, so no hook-shaped side door.** VERIFIED,
`src-tauri/src/session_scanner/cli_tool.rs` agy entry: `compaction_hook: false`. Antigravity's only
managed hooks are the activity hooks in `~/.gemini/config/hooks.json`
(`coordination/agy_hooks_installer.rs`), which carry busy/idle signals, not model configuration.

### Cross-check: Codex and Grok (researcher 2's lane — do not treat as this lane's conclusion)

**E-X1 — Codex 0.150.1 has no effort flag.** VERIFIED, `codex --version` → `codex-cli 0.150.1`;
`codex --help` lists `-m, --model <MODEL>` and `-c, --config <key=value>` ("Override a configuration
value that would otherwise be loaded from `~/.codex/config.toml`") but **no `--effort`**. This matches
taurhaus's registry, which is the only harness using `EffortFlag::Config`.

**E-X2 — Codex has a `queue` subcommand.** VERIFIED, `codex --help` `Commands:` block:
```
  resume            Resume a previous interactive session (picker by default; use --last to continue the most recent)
  queue             Queue a message for an existing session
```
UNVERIFIED and **flagged to researcher 2**: whether `codex queue` accepts a slash command, and whether
Codex's TUI `/model` picker carries a reasoning-effort selection the way Antigravity's does. If both
hold, Codex gains a *non-tmux* runtime path that neither Claude nor agy has.

**E-X3 — Grok 1.0.13 documents no `/effort`.** VERIFIED, `grok --version` → `grok 1.0.13 (5e9a58528b76)`.
`~/.grok/README.md:586`:
> `| --reasoning-effort / --effort <LEVEL> | Reasoning effort (none, minimal, low, medium, high, xhigh,
> max; also per-model menu ids like deep). TUI and headless. |`

and `:638`: *"`--reasoning-effort`/`--effort` and `--permission-mode` work in both modes."* The
slash-command table at `:492-513` lists `/model <name>` (alias `/m`, "Switch to a different model")
and **no effort command**. UNVERIFIED: whether a `/model` argument can name an effort variant (the
help's "per-model menu ids like `deep`" hints yes). Grok also exposes `--leader-socket <PATH>`
(default `~/.grok/leader.sock`) and a `grok leader` subcommand — a plausible programmatic channel that
researcher 2 should chase.

### taurhaus — the registry

**E-T1 — the effort-flag vocabulary.** VERIFIED, `src-tauri/src/session_scanner/cli_tool.rs:86-96`:
```rust
/// How a reasoning-effort value is expressed by a harness.
pub enum EffortFlag {
    Argument { flag: &'static str },
    Config   { flag: &'static str, key: &'static str },
}
```

**E-T2 — per-harness flags.** VERIFIED, same file:

| Tool | `model_flag` | `effort_flag` | line |
|---|---|---|---|
| Claude | `--model` | `Argument { flag: "--effort" }` | `:235-236` |
| Codex | `-m` | `Config { key: "model_reasoning_effort" }` | `:295-298` |
| Agy | `--model` | `Argument { flag: "--effort" }` | `:384-385` |
| Grok | `--model` | `Argument { flag: "--effort" }` | `:520-521` |
| *(unknown)* | `None` | `None` | `:592-593` |

All four agree with the installed binaries' own help (E-C2, E-A2, E-X1, E-X3). **The registry is correct
as of these versions.**

**E-T3 — effort is applied at launch only.** VERIFIED, `src-tauri/src/session_scanner/launch.rs`:
`ModelSpec { model, reasoning_effort }` (`:9-12`), `LaunchSpec::render()` appends the flags
(`:261-281` for the `Config` shape, `:312-330` for the `Argument` shape), gated by
`ModelCatalog::supports_effort(tool, model, effort)` (`:263`, `:313`), emitting
`launch.effort.ignored` / `launch.effort.invalid` (`:159`, `:163`). **There is no runtime
counterpart anywhere in the codebase** — no function renders `/effort` or `/model` as a slash command.

### taurhaus — the delivery path

**E-T4 — the delivery request shape.** VERIFIED, `src-tauri/src/coordination/requests.rs`:
```rust
:108  pub struct OperatorNoticeDelivery {
:109      pub member_name: String,
:110      pub team_name: String,
:111      pub message: String,
:113      pub sender_name: Option<String>,
:115      pub operational_context: Option<OperationalContextUpdate>,
      }
:123  pub enum DeliveryRequest { Bootstrap(..), RecoveryNudge(..), OperatorNotice(Box<..>) }
:136  pub enum DeliveryMethod { InboxFile, TmuxInjection, NativeMessageApi }
```

**E-T5 — the assignment contract.** VERIFIED, `requests.rs:62-75`:
```rust
pub struct OperationalAssignmentFooter {
    pub execution_mode: String,
    pub file_ownership_boundary: Vec<String>,
    pub adjacent_fix_policy: String,
    pub validation_expectation: String,
    pub response_expectation: String,
}
```
wrapped by `OperationalContextUpdate { task, assignment_footer, ownership, working_set }` (`:95-106`),
whose task slot is `OperationalTaskContext { id, subject, status }` (`:50-59`). The mirrored persisted
types are `OperationalAssignmentFooterSnapshot` / `OperationalTaskSnapshot` /
`OperationalContextSnapshot` at `stores/operational.rs:24-64`.

**E-T6 — where the snapshot lands.** VERIFIED, `stores/operational.rs:160-165`:
```rust
fn operational_snapshot_dir(teams_dir, team_name)  -> teams_dir/<team>/state/operational
fn operational_snapshot_path(teams_dir, team, member) -> …/state/operational/<member>.json
```
written atomically via a `.tmp` sibling (`:105`, `:168`).

**E-T7 — the context is structured, never parsed out of prose.** VERIFIED,
`coordination/operational_context.rs:95` `apply_delivery_context(teams_dir, team, member, context)`,
guarded by the test named at `:493`:
`apply_delivery_context_updates_structured_footer_without_parsing_message`. And the renderer keeps the
two apart — `coordination/delivery.rs:154-159`:
```rust
pub fn render_operator_notice(payload: &OperatorNoticeDelivery) -> String {
    format!("[taurhaus] operator_notice from {}: {}", payload.team_name, payload.message)
}
```
**The operational context is never rendered into the delivered text.** A new `effort` field added to
the footer would be persisted and queryable but *invisible to the agent* unless it is also written
into `message` — which is a design decision W5 has to make explicitly.

**E-T8 — the wire.** VERIFIED, `coordination/backend/bridged.rs:426-447`:
```rust
fn send_operator_notice(&self, payload: OperatorNoticeDelivery) -> Result<DeliveryResult, …> {
    let message = MeshInboxMessage::operator_originated(
        &payload.member_name, payload.message, Some(NOTICE_SUMMARY.to_string()),
        Utc::now(), payload.sender_name.as_deref());
    MeshInboxStore::append(&self.teams_dir, &payload.team_name, &payload.member_name, &message)?;
    Ok(DeliveryResult { delivered: true, method: DeliveryMethod::InboxFile })
}
```
with `const NOTICE_SUMMARY: &str = "operator_notice";` (`bridged.rs:28`, and the identical
`OPERATOR_NOTICE_SUMMARY` at `backend/claude.rs:17`). Both backends accept **only** `OperatorNotice`
(`bridged.rs:469-476`, `claude.rs:76-81`), and the selector forces `MeshBridged` for every tool today
(`backend/selector.rs:17-24`, `m0()`).

**E-T9 — the inbox record, and the extension point.** VERIFIED, `stores/inbox.rs:18-41`:
```rust
/// Message entry stored in `teams/<team>/inboxes/<member>.json`.
#[serde(rename_all = "camelCase")]
pub struct MeshInboxMessage {
    pub id: Option<String>, pub from: String, pub text: String, pub timestamp: String,
    pub read: bool, pub summary: Option<String>, pub color: Option<String>,
    pub priority: Option<String>, pub acked_at: Option<String>, pub acked_by: Option<String>,
    pub external_relay: Option<Value>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,          // ◄── mesh-owned round-trip map
}
```
`remove_authored_keys_from_extra` (`:73-88`) protects the eleven authored keys from collision.

**E-T10 — there is no `task_assignment` message type in taurhaus.** VERIFIED by absence:
```
$ grep -rn "task_assignment" --exclude-dir={.git,node_modules,target} .
docs/design/research/taureval-harness-inventory.md:142:| task | inbox write (`spawner.ts:256`) | `mesh send … --summary task_assignment` (`spawner.ts:249-253`) |
```
The single hit is a *description of taureval's* spawner, not taurhaus code. taurhaus's own summary
value is `"operator_notice"` (E-T8). **The research question's premise — "mesh inbox `task_assignment`"
— is taureval's convention, and adopting it would be a new contract, not an existing one.**

**E-T11 — taurhaus sets no effort env var and drives no Remote Control.** VERIFIED by absence:
```
$ grep -rn "remote-control\|remote_control\|CLAUDE_CODE_EFFORT_LEVEL" src-tauri/src src
(no output)
```
So E-C8's trap is not armed today, and E-C10's channel is not available today.

**E-T12 — the task record has no effort and no author.** VERIFIED,
`src-tauri/src/task_scanner/types.rs:29-68`, `ScannedTask`: `id`, `source_key`, `subject`,
`description`, `active_form`, `status`, `source: CliTool`, `blocks`, `blocked_by`, `owner`,
`session_id`, `state_changed_at`, `updated_at`, `archived_at`, `last_status`, `archived_reason`.
The doc comment at `:28` reads *"A task normalized from any CLI tool's native format"* and several
fields are marked `(Claude only)`. Tasks flow **inward** from the harnesses.

**E-T13 — task → member snapshot.** VERIFIED, `coordination/operational_context.rs:26-53`
`sync_member_snapshot` feeds `latest_owned_task_from_tasks(&tasks, member_name)` into the snapshot's
`task` slot; `services/task_sync.rs:171` calls `sync_project_task_snapshots` after task persistence.
So the *currently owned* task already reaches the member's operational snapshot automatically — an
`effort` decided per task would ride the same sync.

**E-T14 — the member record carries launch model + effort.** VERIFIED,
`src-tauri/src/coordination/domain.rs:24-53`:
```rust
pub struct Member {
    pub name: String, pub role: MemberRole, …
    #[serde(default)] pub model: Option<String>,
    #[serde(default)] pub reasoning_effort: Option<String>,
    pub project_path: PathBuf, pub cli_tool: CliTool,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}
```

**E-T15 — the tmux injection primitive already exists.** VERIFIED,
`src-tauri/src/coordination/runtime/system.rs:157-178`:
```rust
fn send_tmux_keys_with_enter(&self, pane_id: &str, keys: &str) -> Result<(), CoordinationError> {
    let target = tmux_target_for_pane(pane_id);
    run_tmux(&["send-keys".into(), "-t".into(), target.clone(), "-l".into(), keys.to_string()])?;
    thread::sleep(TMUX_TEXT_TO_ENTER_DELAY);
    run_tmux(&["send-keys".into(), "-t".into(), target, "Enter".into()])?;
    thread::sleep(TMUX_POST_ENTER_DELAY);
    Ok(())
}
```
It is a trait method (`runtime/mod.rs:98`) with a recording double (`runtime/recording.rs:308`) and
retry handling with pane diagnostics (`pipelines/helpers.rs:410-428`). Today its only production
callers pass a **launch command** (`helpers.rs:410`, `runtime/mod.rs:68`). The `-l` (literal) flag is
important: a leading `/` is sent as a character, not interpreted by tmux.

**E-T16 — what the UI renders.** VERIFIED:
- `src/lib/components/MeshNode.svelte:15` `model = ''` prop, `:33` `safeModel`, `:120`
  `<span class="mesh-node-model" …>{safeModel}</span>` — **model only, no effort.**
- `src/lib/components/MeshCanvas.svelte:291,310,319` build the node's `model` from
  `member.model ?? modelName ?? model_name`, and pass it at `:604,:627` — **effort is never passed to
  the node.**
- `src/lib/components/MeshNodeDetail.svelte:76-77`
  `modelDisplay = model ? \`${model}${reasoningEffort ? \` · ${reasoningEffort}\` : ''}\` : ''`,
  shown as the `Model` detail row at `:268` and inline at `:636-638`.
- `src/lib/components/AgentCard.svelte:88-90` the same `model · reasoningEffort` composition.
- No component references `operationalContext`, `assignmentFooter` or `operational` — verified by a
  zero-hit grep over `src/`. **The assignment footer has no UI at all today.**

---

## Recommendation

### 1. Treat "runtime effort" as a two-tier capability, and put it in the registry

The registry already distinguishes *how* a harness spells effort (`EffortFlag::Argument` vs `Config`).
W5 needs a second, orthogonal axis — *whether the harness accepts effort after launch, and through
what*. Proposal, alongside `effort_flag` in `CliCapabilities`:

```rust
/// How a harness accepts a reasoning-effort change for a session already running.
pub enum RuntimeEffort {
    /// No runtime path: effort is fixed until relaunch.
    None,
    /// A slash command typed into the harness's own interactive input.
    SlashCommand { command: &'static str },   // claude: "/effort", agy: "/effort"
}
```

Seeded from this lane's evidence: Claude `SlashCommand{"/effort"}` (E-C4), Agy
`SlashCommand{"/effort"}` (E-A3), Codex and Grok `None` **pending researcher 2** (E-X2, E-X3 both
leave a live thread). Keeping it in `cli_tool.rs` honours the standing rule that tool identity may
only fan out there.

### 2. Deliver a task-level effort as `/effort <level>` down the existing tmux primitive

Reuse `send_tmux_keys_with_enter` (E-T15) — do not invent a channel. The sequence for an assignment
that carries an effort:

1. resolve `RuntimeEffort` for the member's tool; if `None`, skip step 2 and record why;
2. `send_tmux_keys_with_enter(pane_id, "/effort <level>")`;
3. deliver the operator notice as today (E-T8).

Order matters: the effort must land **before** the task text, or the first turn runs at the old level.

### 3. Put `effort` and `why` in `OperationalAssignmentFooter`, not in the task and not in `Member`

Add two fields to `OperationalAssignmentFooter` (`requests.rs:62`) and its snapshot twin
(`stores/operational.rs:24`):

```rust
#[serde(default)] pub reasoning_effort: Option<String>,
#[serde(default)] pub effort_rationale: Option<String>,   // the "why", one sentence
```

Both `#[serde(default)]`, so existing `state/operational/<member>.json` files keep deserialising. This
is the right home because (a) it *is* the assignment contract, (b) `apply_delivery_context` already
writes it structurally without prose-parsing (E-T7), and (c) it is per-assignment state that survives
the message, which `MeshInboxMessage.extra` would not.

Explicitly **do not**:
- add `effort` to `ScannedTask` — it is a read-only projection of the harnesses' own task stores (E-T12);
- overload `Member.reasoning_effort` — that is the *launch* effort and drives `LaunchSpec::render()`
  (E-T3, E-T14). A task-level value that overwrote it would silently redefine the next launch.

### 4. Decide deliberately whether the "why" reaches the agent

`render_operator_notice` does not render the operational context (E-T7). If the rationale is meant to
steer the agent rather than only to explain the operator's choice in the UI, it has to be appended to
`message` — a one-line renderer change with a golden test in `src-tauri/tests/cli_renderers.rs`
alongside the existing `DeliveryRenderer` goldens. My recommendation: **render it**, in one line, e.g.
`Effort: high — multi-file refactor across the daemon boundary.` An effort change the agent cannot see
is an effort change it cannot honour in its own planning.

### 5. Guard the Claude persistence side effect before shipping

E-C6 is the sharpest edge in this whole area: an interactive `/effort high` in a member pane is
expected to write `modelSettings[<model>].effortLevel` into the operator's user settings, changing the
default for **every future Claude Code session on the machine**, including the operator's own. Before
W5 ships:

1. confirm the write empirically under a scratch `CLAUDE_CONFIG_DIR` (this lane could not — read-only);
2. if confirmed, decide the mitigation. The cleanest is to launch managed Claude members with an
   explicit `--effort` so the pin is *already* at the member's baseline, and to accept that a per-task
   `/effort` moves the user default — or to prefer relaunch-with-`--effort` over a live `/effort` for
   Claude specifically, and use the live path only for Antigravity.

Note the pin interaction, too (E-C7): for `fable-5` / `opus-4-7` / `opus-4-8` members, the *first*
interactive `/effort` releases the launch pin for the rest of that session, so a later programmatic
attempt behaves differently from the first. Any test must cover the second change, not just the first.

### 6. Show effort on the mesh node

`MeshNode` renders the model but not the effort (E-T16), while the detail panel and AgentCard render
`model · effort`. If effort becomes per-task and mutable at runtime, the runtime node is exactly where
an operator needs to see it — otherwise the canvas shows a member at a level it no longer has. This is
a small `MeshCanvas.svelte` → `MeshNode.svelte` prop addition mirroring the existing `modelDisplay`
composition.

### 7. Open threads for the other lane and for a follow-up

- **Researcher 2 must settle Codex**: does `codex queue` (E-X2) accept a slash command, and does the
  Codex TUI `/model` picker carry reasoning effort? A yes gives Codex the only *non-tmux* runtime path.
- **Researcher 2 must settle Grok**: `--leader-socket` / `grok leader` (E-X3) as a control channel, and
  whether `/model <name>` can name an effort variant.
- **Unresolved here**: Claude's live settings-reload behaviour (E-C11), Claude's user-settings write
  path (E-C6), and Antigravity's `/effort` persistence (E-A7). All three need a scratch-`HOME` runtime
  test that this read-only lane was correctly barred from running.
