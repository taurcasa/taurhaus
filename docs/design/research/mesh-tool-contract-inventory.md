# Mesh CLI-Tool Contract Inventory — adding `agy` + `grok`, dropping `gemini`

Read-only inventory of `~/projects/mesh` (Rust, v0.2.20, HEAD `9994754db5cf40c74bcd361bdb9084322481a1a3`, clean tree) for every site that depends on **which CLI a team member runs**, plus the taurhaus mesh version/lock/bundle flow needed to ship a mesh change.

Facts below are read from source or command output at the cited `file:line`. Inferences are marked **UNVERIFIED** with the check that would settle them.

---

## 0. Headline

**The CLI-dependent surface in mesh is astonishingly small — five files, and only one of them is load-bearing code.**

```
$ grep -rn -iE "\b(gemini|codex|agy|grok|antigravity)\b" . \
    --exclude-dir=target --exclude-dir=.git -l
README.md
USAGE.md
src/config.rs          # test fixture strings only
src/daemon.rs          # ONE allowlist + ONE doc comment
docs/taurhaus-integration-proposal.md
```

`.github/workflows/` is empty. `tests/` contains no CLI-name references (only `CLAUDE_DIR` env and the `task ingest-claude` adapter, which is a task-file format, not a CLI harness).

The **only behavioural** CLI dependency in mesh is the pane-ownership guard:

```rust
// src/daemon.rs:344-352
fn known_agent_cli(command: &str) -> Option<String> {
    let basename = Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    matches!(basename.as_str(), "claude" | "codex" | "gemini").then_some(basename)
}
```

Everything else — delivery keystrokes, idle/busy, wake, notification text, the member `tool` field — is CLI-agnostic today. That means the mesh change for agy/grok is **one line of behaviour plus tests and docs**, not a refactor.

---

## 1. Pane / process identification

### 1.1 `known_agent_cli` — the basename allowlist  ⚠️ THE change site

| | |
|---|---|
| **Site** | `src/daemon.rs:344-352` (allowlist literal on **:351**) |
| **What it does** | Takes `#{pane_current_command}`, strips directory + `.exe`, lowercases, and returns `Some(basename)` only for `claude`/`codex`/`gemini`. `None` means "not a known agent CLI". |
| **Consumed by** | `decide_pane_delivery` (`:362`) — a `None` result is an **early `return PaneDeliveryDecision::Deliver`**, i.e. the guard fails open. |

**Live consequence today** (read from `:344-379`; the control flow is unambiguous):

| Member `cli_tool` | Pane foreground | `known_agent_cli` | Decision | Correct? |
|---|---|---|---|---|
| `codex` | `claude` | `Some("claude")` | Skip `pane_cli_mismatch` | ✅ |
| `agy` | `agy` | **`None`** | Deliver (guard inert) | ✅ by luck |
| `codex` | **`grok`** | **`None`** | **Deliver** | ❌ **wake injected into a foreign agent's CLI** |
| `grok` | `claude` | `Some("claude")` | Skip `pane_cli_mismatch` | ✅ |
| `agy` | `codex` | `Some("codex")` | Skip | ✅ |

So the regression the guard exists to prevent (commit `6574d41f`, per the test comments at `:1748-1750`) is **reopened for any pane reused by an agy or grok process**.

**What agy/grok need:** replace the literal with `"claude" | "codex" | "agy" | "grok"`. Drop `"gemini"`.

**Why a plain basename match is sufficient — verified:**
- agy: *"Under tmux, `#{pane_current_command}` is `agy` for an interactive session."* (`docs/design/research/agy-report-opus.md:42`; corroborated `agy-report-codex.md:259`)
- grok: *"In tmux, `pane_current_command` and `pane_title` were both `grok` before, during, and after the turn."* (`grok-report-codex.md:326`)
- Both are single self-contained binaries with no launcher shim: agy is a 199 MB Go ELF with **no children** under `ps --ppid` (`agy-report-opus.md:24-31`); grok "execs directly; there is no wrapper script, no node/python shim" (`grok-report-opus.md:26-32`). So unlike a node-based CLI, `pane_current_command` will not be `node`.

**Naming collision risk — UNVERIFIED, low.** `agy` and `grok` are short tokens; a user's unrelated binary named `grok` (e.g. the Perl/Ruby `grok` log parser) in a member's pane would now be treated as a known agent CLI and could cause a spurious `pane_cli_mismatch` skip instead of a delivery. *Verify by:* checking `command -v grok agy` on target machines, or by accepting the risk — the failure mode is a retryable skip (see §1.4), not a lost message.

### 1.2 `probe_pane` / `parse_pane_snapshot` — where the command string comes from

| | |
|---|---|
| **Sites** | `src/daemon.rs:302-317` (probe), `:319-338` (parse), `:340-342` (`pane_current_command` convenience), `:121-125` (`PaneSnapshot`) |
| **What it does** | `tmux display-message -p -t <pane> '#{pane_id}\t#{pane_dead}\t#{pane_current_command}'`, tab-split into `pane_id` / `dead` / `current_command`. An empty `current_command` is valid (regression test at `:1802`). |
| **agy/grok need** | **No change.** Both report a stable `pane_current_command`. |

Note: mesh reads only `pane_current_command`, never `pane_title`. That is the right call for these two — agy does **not** set `#{pane_title}` (stays at hostname, `agy-report-opus.md:42, 327`), and grok's title is just `grok` and "did not encode busy/idle" (`grok-report-codex.md:326`). grok *can* emit OSC-2 busy/idle transitions (`grok-report-opus.md:457-462`) but that is taurhaus's idle lane, not mesh's.

### 1.3 `configured_cli_tool` — the ONLY read of the member tool field

| | |
|---|---|
| **Site** | `src/daemon.rs:385-399`, key literal on **:396** — `member.extra.get("cli_tool")` |
| **What it does** | Re-reads `config.json` on every delivery, finds the member by name, pulls `cli_tool` out of the flattened `extra` map as a bare `&str`. No enum, no allowlist, no normalisation beyond `str::trim` + `to_ascii_lowercase` at the comparison site (`:365-376`). |
| **agy/grok need** | **No change.** A new value flows through as a plain string. |

**Key/format contract verified against taurhaus:**
- mesh `Member` (`src/types.rs:102-158`) is `#[serde(rename_all = "camelCase")]` with `#[serde(flatten)] pub extra: BTreeMap<String, Value>` at `:156-157`. `flatten` does **not** apply `rename_all` to map keys, so the JSON key is preserved verbatim — mesh looks for exactly `cli_tool`.
- taurhaus writes exactly that: `src-tauri/src/coordination/stores/config.rs:119` declares `cli_tool: CliTool` on `MeshCompatibleMemberWire` with **no `#[serde(rename)]`**, while every sibling field has one (`agentId`, `agentType`, `tmuxPaneId`, …). So the emitted key is snake_case `cli_tool` and the guard does fire. ✅
- The value vocabulary is taurhaus's `CliTool` enum, `#[serde(rename_all = "lowercase")]` (`src-tauri/src/session_scanner/cli_tool.rs:15-26`): `claude` | `codex` | `agy` | `grok` | `unknown` (`#[serde(other)]`). **taurhaus has already dropped Gemini and already ships Agy + Grok** — mesh is the lagging side of this contract.
- Beware the near-miss: `src/config.rs:517,533` (mesh's own test fixture) writes `"cliTool"` camelCase. That fixture is only asserted for round-trip preservation (`:568`); it is **not** the key the daemon reads. Do not "fix" one to match the other without checking both sides.

### 1.4 `decide_pane_delivery` + skip plumbing

| Site | Role |
|---|---|
| `src/daemon.rs:354-379` | The decision: missing/dead pane → `Skip(PaneMissing)`; unknown foreground → `Deliver`; known CLI ≠ expected → `Skip(ForeignAgentCli)`; match → `Deliver` |
| `src/daemon.rs:381-383` | `pane_is_available` — non-empty `pane_id` && `!dead` |
| `src/daemon.rs:127-142` | `PaneSkipReason` + `Display` → journal strings `pane_dead_or_missing`, `pane_cli_mismatch: expected {e}, found {f}` |
| `src/daemon.rs:144-148` | `PaneDeliveryDecision` |
| `src/daemon.rs:1288-1313` | `handle_delivery_with_snapshot` — calls `configured_cli_tool` (`:1298`) then `decide_pane_delivery` (`:1300`) |
| `src/daemon.rs:1382-1438` | `handle_skipped_delivery` — a `ForeignAgentCli` skip is **retryable** (does not increment `failures`); only `PaneMissing` counts toward the 5-failure shutdown (`:1392`, `:1429-1436`) |
| `src/daemon.rs:64-118` | `DeliverySkipTracker` — journals an unchanged skip once per daemon run |

**agy/grok need:** no structural change. Once `agy`/`grok` are in the allowlist, their mismatches become retryable skips with the existing journal strings.

### 1.5 `is_shell_command` — the *other* pane command check

| | |
|---|---|
| **Site** | `src/daemon.rs:488-490` — `matches!(command, "zsh" \| "bash" \| "sh" \| "dash" \| "fish")` |
| **What it does** | Decides the payload shape: a shell pane gets `printf '%s\n' '<escaped>'` (`:511-513`, escape at `:492-494`); anything else gets the raw escaped text (`:514`). |
| **agy/grok need** | **No change.** `agy` and `grok` are not shells, so they take the raw-text branch, same as `claude`/`codex`. Verified they are not shell wrappers (§1.1). |

---

## 2. Message delivery keystrokes

### 2.1 The single delivery path — already grok-shaped

| | |
|---|---|
| **Sites** | `src/daemon.rs:518-525` (`deliver_to_tmux`), `:527-534` (`deliver_to_tmux_with_command`), `:536-563` (`deliver_payload_to_tmux`), `:496-516` (payload shaping), `:294-300` (`tmux_escape`) |
| **What it does** | Exactly two `send-keys` calls: <br>`:538-539` `tmux send-keys -l -t <pane> <payload>` (literal, no key interpretation)<br>`:549` `sleep(100ms)`<br>`:552-553` `tmux send-keys -t <pane> Enter` |
| **Stale doc comment** | **`src/daemon.rs:518-521`** — *"This ensures agent CLIs (Codex, Gemini) receive it as one prompt submission."* Only the CLI names are wrong; the mechanism is correct. |

**This already satisfies grok's stated requirement.** From `grok-report-opus.md:941-944`: *"Verified end to end: `tmux send-keys -t <target> "<text>"` then a separate `tmux send-keys -t <target> Enter` delivered a prompt to a live TUI… The two-call split (text, then Enter) matters — grok's input box handles them as distinct events."* mesh's `-l` flag plus the 100 ms gap is a strictly safer version of the verified recipe.

taurhaus runs the identical shape independently — `src-tauri/src/coordination/runtime/system.rs:157-178` (`send-keys -l` → `TMUX_TEXT_TO_ENTER_DELAY` → `Enter`) and `src-tauri/src/session_scanner/control.rs:743-768` (200 ms gap). No divergence to reconcile.

**agy/grok need:**
- **Required:** change only the comment at `:521` to name the current harnesses.
- **Not required:** bracketed paste. Neither report found any need for it, and `tmux_escape` (`:296-300`) already strips newlines so every payload is a single line.

### 2.2 Interject vs queue — grok's two-tier delivery (the one genuinely new capability)

**Verified fact** (`grok-report-opus.md:946-951`, citing grok's own `03-keyboard-shortcuts.md:277`): plain **`Enter` queues** a message that grok picks up at the next turn boundary without stopping the running turn, while the interject chord (**`Ctrl+Enter`/`Ctrl+I`**, or `Ctrl+L` on VS Code-family terminals) is "send-now" and *"intentionally interruptive — it reads as stop what you're doing and take this."*

**mesh already has the tier signal it would need.** `InboxMessage.priority` is `"urgent"` (default) or `"low"` (`src/types.rs:31-34`), and the daemon already branches on it — `src/daemon.rs:988-991` skips the wake entirely for low-priority messages, journaling `all_messages_empty_or_low_priority` (`:1041`). The cron reminder is sent low-priority (`:1160`).

So mesh's existing tiers are `low = no wake` / `urgent = Enter`. grok adds a natural third: `urgent = interject chord`.

**Recommendation: do NOT ship the chord in this change.** mesh's current plain-`Enter` behaviour is grok's *queue* semantic, which is the correct default for an inbox wake — it lands at the next turn boundary without corrupting an in-flight turn. Shipping the interject requires resolving:

> **UNVERIFIED: whether `tmux send-keys -t PANE C-Enter` actually reaches grok as Ctrl+Enter.**
> Neither report drove the chord through tmux — the delivery "How verified" line (`grok-report-opus.md:988-991`) covers only the plain text+Enter round trip. Two concrete hazards: (a) `Ctrl+Enter` is not distinguishable from `Enter` in legacy terminal encoding — it needs the kitty keyboard protocol / CSI-u, which depends on the terminal *and* on grok negotiating it; (b) **`C-i` is ASCII `0x09`, i.e. literally Tab** — `send-keys C-i` will almost certainly be read as Tab, not Ctrl+I.
> *Verify by:* launch grok in a tmux pane, start a long turn, `tmux send-keys -t %N -l "test"; tmux send-keys -t %N C-Enter`, and `capture-pane` to see whether the turn is interrupted or the message queues. Repeat with `C-i`. Do this before writing any code.

### 2.3 Delivery channels mesh deliberately does not use

Both CLIs expose a richer programmatic channel than keystrokes. Recorded here so the mesh change is not over-scoped — **none of this belongs in mesh**, which owns the filesystem protocol, not agent launch:

- **agy** — `--input-format stream-json` reads NDJSON `{"event":"user","message":{"content":"…"}}` from a held-open stdin, one turn per line, same conversation, process stays alive (`agy-report-opus.md:550-568`). Unknown `event` values are **silently skipped** with only a stderr warning, so schema drift looks like a hang.
- **grok** — ACP JSON-RPC over `grok agent --no-leader stdio`; `initialize` → `session/new`|`session/load` → `session/prompt` (`grok-report-opus.md:953-959`). The report's own recommendation: *"Keep tmux `send-keys` as the delivery mechanism… Treat the leader as a future optimization, not a v1 dependency"* (`:1003-1011`).

---

## 3. Idle / busy / wake logic

**Finding: mesh has zero per-CLI idle logic. No change needed.**

| Site | What it does | CLI dependency |
|---|---|---|
| `src/idle_monitor.rs:192-258` | `evaluate_idle_decision` — pure function over status / heartbeat / runtime-health / activity-snapshot inputs | **none** |
| `src/idle_monitor.rs:991-1044` | `read_activity_snapshot_observation` — reads `{team_dir}/state/activity/{member}.json`, 120 s staleness window (`:24`) | **none** — mesh is a *consumer* |
| `src/idle_monitor.rs:604-651` | `format_auto_nudge_message` / `format_assignment_message` — action-first text | **none** |
| `src/idle_monitor.rs:666-710` | `send_nudge` — writes an `InboxMessage` from `mesh-idle-monitor` into the member's inbox | **none** (inbox write, not a keystroke) |
| `src/idle_monitor.rs:713-771` | `send_escalation` — escalates to the lead's inbox | **none** |
| `src/idle_monitor.rs:873-951` | `read_runtime_health` | **none** |
| `src/projections.rs:587-641` | Projection-side reuse of the same decision function | **none** |
| `src/daemon.rs:988-991` | Low-priority messages skip the wake entirely | **none** |

The CLI-specific part of idle sensing lives **entirely in taurhaus**, which writes the snapshot mesh reads: `src-tauri/src/coordination/activity_export.rs:646` builds `teams_dir/{team}/state/activity`, and `:196-215` enriches each row with the member's `configured_cli_tool` and a `pane_foreign` probe. taurhaus's `CliCapabilities.authoritative_idle` is `false` for both agy and grok (`cli_tool.rs:385`, `:515` region), so taurhaus already knows these two need snapshot-based inference. **That is a taurhaus concern; mesh needs no edit.**

Nudges and escalations are inbox writes, so they reach agy/grok through the same daemon wake path as any other message — no per-CLI wake variant exists or is needed.

---

## 4. Member `tool` field validation

**Finding: mesh performs no validation of the member tool value anywhere. No change needed.**

- `mesh join` (`src/cli.rs:31-47`) accepts only `--team`, `--name`, `--type`, `--model`, `--color`. There is **no `--tool` / `--cli-tool` flag**, and `cmd_join` (`src/main.rs:828-850`) constructs `Member { …, extra: BTreeMap::default() }` — mesh never writes `cli_tool` at all. taurhaus is the sole writer.
- `mesh who` (`src/main.rs:1408-1429`) prints `name (agent_type) [model]` and, with `--json`, serialises the whole `Member` — so `cli_tool` round-trips into JSON output for free, no allowlist involved.
- `src/validate.rs` and `src/lint.rs` contain no tool-value checks (grep for `cli_tool` across `src/` returns exactly four hits, all in `daemon.rs`, plus test helpers).

**Implication:** a `cli_tool` value of `agy`, `grok`, or even taurhaus's `unknown` sentinel is already accepted and preserved losslessly today. The `extra` flatten map (`src/types.rs:156-157`) is the compatibility mechanism and it is doing its job.

---

## 5. Onboarding / help / user-facing text

### 5.1 Runtime output — CLI-agnostic, no change

| Site | Text |
|---|---|
| `src/daemon.rs:274-292` | `format_notification` — `[mesh] You are "{name}" on team "{team}". Inbox update from {senders}. …` |
| `src/daemon.rs:169-197` | `format_task_notification` — task assignment wake with the full lifecycle command list |
| `src/main.rs:1292-1301` | `print_read_footer` — the universal "Determine what this message requires" classifier |
| `src/cli.rs:6-10` | `--help` root: *"Filesystem-based IPC for co-located AI agents"* |

None name a CLI. The Codex-specific `post_compaction_context` display special-case that `taurhaus/docs/design/harness-realignment-plan.md:117` lists for deletion at `mesh/src/main.rs:1285-1292` **is already gone** — `grep -in codex src/main.rs` returns nothing at HEAD `9994754`.

### 5.2 Docs — this is where the work is

| File:line | Content | Change |
|---|---|---|
| `README.md:3` | *"lets non-Claude agents (Gemini CLI, Codex, custom agents)…"* | → Codex, Antigravity CLI, Grok CLI |
| `README.md:147` | Pane-guard paragraph: *"a known agent CLI that conflicts with the member's `cli_tool` is skipped and retried"* | Accurate; only extend if you list the allowlist |
| `USAGE.md:5` | Same intro sentence as README:3 | same |
| `USAGE.md:12` | ASCII diagram: `External Agent (Codex/Gemini in tmux)` | → `(Codex/agy/grok in tmux)` |
| `USAGE.md:75, 80` | `tmux new-window -n "gemini-agent"` + sample `list-panes` output | rename |
| `USAGE.md:87-93` | Auto-approve flag table: Codex `--yolo`, Gemini `--yolo`, Claude `--dangerously-skip-permissions` | **add** agy `--dangerously-skip-permissions`, grok `--always-approve`; **drop** Gemini row |
| `USAGE.md:128-139` | `#### Gemini CLI (Google)` launch block, incl. the *"model ids and unattended flags are unverified"* hedge | **replace** with verified agy + grok blocks |
| `USAGE.md:398-400` | `### For Gemini CLI` prompt template pointer | → `### For Antigravity CLI (agy)` / `### For Grok CLI` (the mesh command surface is genuinely universal, so the "same as Codex" framing still holds) |
| `USAGE.md:477` | Troubleshooting: *"Use `--yolo` (Codex/Gemini)…"* | → per-CLI flags |
| `USAGE.md:478` | *"Tool execution denied by policy (Gemini) → Relaunch with `gemini --yolo`"* | **delete**; replace with the agy soft-deny row (below) |
| `USAGE.md:480` | *"New CLI command typed into running CLI → `tmux send-keys -t PANE C-c`, wait 2s, then relaunch"* | ⚠️ **actively wrong for agy** — see below |
| `docs/taurhaus-integration-proposal.md:168, 311, 337, 389, 435, 442` | Gemini in the historical enum / launch-flow prose | Doc is banner-marked *"Historical proposal (superseded in part)"* at `:3`. Lowest priority; a one-line note is enough |

**Verified facts to write into USAGE.md:**

*Launch (agy — `agy-report-opus.md:62-88`, v1.1.22):*
- `--model <id>` (ids from `agy models`), `--effort low|medium|high` — **some models require `--effort`**
- `--dangerously-skip-permissions` for auto-approve
- **Flag-order trap:** `--print`/`-p` is *string-valued*, so `agy -p --input-format stream-json` swallows the next flag as the prompt. Use `--print=` last.
- **Silent soft-deny:** without `--dangerously-skip-permissions`, headless *exits 0 with empty output* when a tool needs confirmation. This is the agy replacement for the Gemini "denied by policy" troubleshooting row, and it is worse because it is silent.

*Launch (grok — `grok-report-opus.md:121-160`, v1.0.5):*
- `-m, --model <MODEL>` (validated eagerly — a bad id exits non-zero); `grok models` lists them
- `--reasoning-effort` / `--effort` ∈ `xhigh|high|medium|low` (grok-4.6), default `high`
- `--always-approve` ≡ `--yolo` ≡ `--permission-mode bypassPermissions`
- **Ambiguity trap:** `grok "fix the bug"` is *interactive* (positional seeds a TUI); `grok -p "…"` is headless. Detect on the flag, never on "has a positional".

*Stop — `USAGE.md:480` and `:144-146` need a per-CLI split:*
- **agy: never `Ctrl+C`.** Verified live (`agy-report-opus.md:641-666`): `Ctrl+C` **during a turn interrupts the turn only — the process stays alive**, leaving a live idle session that mesh's pane guard will then happily deliver into. Graceful stop is **`/exit`** (or `/quit`), verified to end both the process and the tmux session.
- **grok: `/quit`** (alias `/exit`) is the verified graceful exit — it terminated the process, removed the row from `~/.grok/active_sessions.json`, and let the tmux server exit (`grok-report-codex.md:508`).
- The existing `C-c` advice remains correct for Codex/Claude.

taurhaus already encodes exactly this: `exit_command: "/exit"` + `StopStrategy::SlashExit` + 5 s timeout for agy (`cli_tool.rs:413-414`), `exit_command: "/quit"` + 15 s for grok (`cli_tool.rs:554-555`).

*Onboarding delivery hint (grok only)* — taurhaus already ships this sentence at `cli_tool.rs:546-548` and asserts it at `coordination/delivery.rs:478`:
> "Plain Enter queues a message until the running turn ends; Ctrl+Enter interjects immediately."

Worth mirroring into the mesh USAGE grok prompt template so an agent understands why a `[mesh]` wake may land a turn late.

---

## 6. Tests

All CLI-name-bearing tests are inline in `src/daemon.rs` (`#[cfg(test)] mod tests` at `:1564`). None are in `tests/`.

| Test | Line | Uses | Action |
|---|---|---|---|
| `pane_delivery_decision_skips_missing_pane` | `:1751-1757` | `Some("codex")` | keep |
| `pane_delivery_decision_skips_known_foreign_agent_cli` | `:1761-1770` | expected `codex`, found `claude` | keep; **add an `agy`/`grok` pair** |
| `pane_delivery_decision_allows_matching_agent_cli` | `:1772-1778` | `codex`/`codex` | keep; **add `agy`/`agy` and `grok`/`grok`** |
| `pane_delivery_decision_allows_unknown_foreground_command` | `:1780-1788` | `codex` vs `cat` | keep — **still must pass**; the fail-open path for genuinely unknown commands is deliberate |
| `pane_delivery_decision_skips_dead_pane` | `:1790-1800` | `codex` | keep |
| `handle_delivery_journals_skip_for_foreign_agent_cli` | `:1853-1900` | asserts journal `pane_cli_mismatch: expected codex, found claude` (`:1896`) | keep |
| `regression_c4df8c4_missing_pane_reaches_failure_limit` | `:1902-1969` | `Some("codex")` at `:1978` region | keep |
| `regression_d1efb4a_foreign_cli_skip_remains_retryable` | `:2029-2052` | asserts `pane_cli_mismatch: expected codex, found claude` (`:2041`) | keep |
| `shell_detection_matches_supported_shells` | `:2127-2133` | `assert!(!is_shell_command("codex"))` (`:2132`) | keep; optionally add `agy`/`grok` |
| `stale_pane_guard_allows_cat_for_codex_member` | `:2194-2247` | real tmux, `cli_tool: Some("codex")`, `cat` pane | keep |
| Fixture helper `write_test_team_config_with_cli_tool` | `:1673-1690` | writes `member_extra["cli_tool"]` — the **snake_case** key | reuse as-is for new cases |
| `TmuxSessionGuard` | `:1607-1668` | spawns a real tmux session, `Drop` kills it (`:1660-1666`); skips if tmux unavailable (`:1599-1605`) | reuse |
| Config round-trip fixture | `src/config.rs:495-573` | `"roleId": "codex-architect"`, `"cliTool": "codex"` (**camelCase**, `:517`/`:533`) | pure round-trip preservation — leave alone, or add an `agy`/`grok` case |

**New regression test to add** (per CLAUDE.md's non-negotiable regression rule, which mesh's own test comments follow — see the `// Regression: 6574d41f` / `c4df8c4` / `d1efb4a` style at `:1748`, `:1786`, `:2027`):

```
// Regression: <this commit> — agy/grok panes were not in the known-CLI
// allowlist, so a pane reused by an agy or grok process failed the guard
// open and a wake was injected into a foreign agent's CLI.
decide_pane_delivery(Some("codex"), Some(&pane_snapshot("%7", "grok")))
    == Skip(ForeignAgentCli { expected: "codex", found: "grok" })
```

---

## 7. The mesh ↔ taurhaus contract, in one table

| Concern | Owner | Surface |
|---|---|---|
| Which CLI a member runs | **taurhaus** | writes `cli_tool` (snake_case) into `~/.claude/teams/{team}/config.json` members — `stores/config.rs:119` |
| Tool vocabulary | **taurhaus** | `CliTool` enum, lowercase: `claude`/`codex`/`agy`/`grok`/`unknown` — `session_scanner/cli_tool.rs:15-26` |
| Launching / stopping CLIs | **taurhaus** | mesh has no spawn command (`docs/taurhaus-integration-proposal.md:335`) |
| Pane ownership guard (2nd layer) | **mesh** | `daemon.rs:344-379` ← **the gap** |
| Pane foreign detection (1st layer) | **taurhaus** | `coordination/activity_export.rs:196-215`, quarantines foreign panes |
| Wake keystrokes | **mesh** | `daemon.rs:536-563` |
| Idle activity snapshot | **taurhaus** writes → **mesh** reads | `activity_export.rs:646` → `idle_monitor.rs:991-999` |
| Nudge / escalation decision | **mesh** | `idle_monitor.rs:192-258` |
| Whether mesh daemon runs for a member at all | **taurhaus** | gated on `capabilities.native_inbox_poller` — **`false` for both agy and grok**, so both go through the mesh daemon wake path |

---

## 8. Mesh version / lock / bundle flow (for shipping this change)

### Current pinned state — verified

```
taurhaus src-tauri/resources/mesh.lock.json   → 0.2.20, protocol 1, schema 1,
                                                 git_commit 9994754db5cf40c74bcd361bdb9084322481a1a3
taurhaus src-tauri/resources/mesh.version     → 0.2.20
taurhaus src-tauri/resources/mesh.manifest.json → same 4 fields + bundled_at_utc 2026-08-28T17:32:16Z
mesh Cargo.toml:3                             → version = "0.2.20"
mesh git rev-parse HEAD                       → 9994754db5cf40c74bcd361bdb9084322481a1a3  (clean tree)
```

Lock and source are **currently in sync**, so any mesh edit immediately breaks the gate — by design.

### The moving parts

| Artifact | Path | Role |
|---|---|---|
| Lock manifest | `taurhaus/src-tauri/resources/mesh.lock.json` | build-time source of truth: `version`, `protocol_version`, `schema_version`, `git_commit` |
| Resolver | `taurhaus/scripts/resolve-mesh-binary.sh` | `MESH_BIN` (returned unchecked) → else `$MESH_PROJECT` (default `~/projects/mesh`) rebuilt via `cargo build --release --bin mesh` when its `version --json` `git_commit` ≠ lock (`:16-61`) → else `src-tauri/resources/mesh` → else `~/.local/bin/mesh` (`:68-76`) |
| Gate | `taurhaus/justfile:325-374` `mesh-verify-lock` | compares all four fields; `bundle-mesh` (`:533`) and `install-mesh` (`:513`) both depend on it |
| Bump entry point | `taurhaus/justfile:377-396` `update-mesh-lock VERSION [PROTO] [SCHEMA] [COMMIT]` | rewrites the lock JSON — the only sanctioned way |
| Embedded metadata | `mesh/src/version_info.rs:3-4, 12-37` + `mesh/build.rs` | `PROTOCOL_VERSION = 1`, `SCHEMA_VERSION = 1`; `git_commit`/`git_dirty`/`build_time_utc` injected at compile time from `.git` |
| Runtime gate | `taurhaus/src-tauri/src/commands/mesh.rs:245-320` | startup refuses an installed mesh whose version / protocol / schema / **git_commit** differ from the bundled contract |

### The release checklist (identical in both repos)

`taurhaus/CONTRIBUTING.md:162-174` and `mesh/README.md:285-305`:

1. Bump mesh patch version in `Cargo.toml`, refresh `Cargo.lock`.
2. `just check` **in the mesh repo** (`fmt-check` + `clippy -D warnings` + `cargo test`).
3. **Commit mesh first**, then `just build-release` — `build.rs` embeds the *committed* revision, so building before committing bakes in the wrong commit (and `git_dirty: true`).
4. From taurhaus: `just update-mesh-lock <version> <protocol_version> <schema_version> <git_commit>` using the exact values from `mesh version --json`.
5. `just bundle-mesh` then `just mesh-verify-lock`.
6. `just install-mesh`, then restart running member daemons so the dev host uses the lock-matching binary.
7. Commit `mesh.lock.json` + `mesh.manifest.json` + `mesh.version` **together with** the taurhaus change; then the normal taurhaus release recipes.

If mesh has no configured remote, stop after the local commit — do not invent a push target (`CONTRIBUTING.md:174`).

### Bundling verdict for this change

- **Version bump: patch.** `0.2.20 → 0.2.21`.
- **`protocol_version` and `schema_version` stay at 1.** Adding `agy`/`grok` to a basename allowlist changes no wire format and no file schema. `cli_tool` is an opaque string inside the flattened `extra` map (§1.3, §4) — a mesh that does not know a value simply fails the guard open, exactly as it does today. `version_info.rs:8-10` states the compatibility rule as "additive-only within a line", which this satisfies.
- **Cross-repo ordering matters** (`taurhaus/docs/design/harness-realignment-plan.md:151`): *bump → `just check` in mesh → taurhaus `update-mesh-lock` → `bundle-mesh` → commit → release.* Because startup auto-install refuses mismatched commits, **no mesh change reaches users without a taurhaus release.**
- **Do not run `just check` as an agent** in taurhaus (CLAUDE.md: team-lead owns serialized full-gate runs). `just check` **in mesh** is explicitly part of the checklist and is a different, mesh-local gate.

---

## 9. Recommended change set (minimal, ordered)

**Mesh — code (1 line + tests)**
1. `src/daemon.rs:351` — `matches!(basename.as_str(), "claude" | "codex" | "agy" | "grok")`.
2. `src/daemon.rs:521` — comment: name the current harnesses instead of "(Codex, Gemini)".
3. Add the `agy`/`grok` cases to `pane_delivery_decision_*` (`:1761-1788`) and the regression test in §6.

**Mesh — docs**
4. `README.md:3`; `USAGE.md:5, 12, 75, 80, 87-93, 128-139, 398-400, 477-478, 480`.
5. `USAGE.md:480` + `:144-146` — split the stop guidance: `/exit` for agy, `/quit` for grok, **never `Ctrl+C` for agy**.
6. One-line note on `docs/taurhaus-integration-proposal.md` (already banner-marked historical).

**Explicitly out of scope**
7. The grok interject chord (§2.2) — blocked on a tmux-level verification that no report performed.
8. agy `--input-format stream-json` and grok ACP (§2.3) — agent-launch concerns, taurhaus's lane.
9. Any change to idle/busy (§3) or tool-field validation (§4) — no CLI dependency exists there.

**Ship**
10. `0.2.20 → 0.2.21`, protocol/schema unchanged; run the §8 checklist end to end.

---

## 10. Unverified items, consolidated

| Claim | Status | How to verify |
|---|---|---|
| `tmux send-keys C-Enter` reaches grok as the interject chord | **UNVERIFIED** | Live tmux: long turn → `send-keys -l "test"` + `send-keys C-Enter` → `capture-pane`. Expect failure without kitty-keyboard/CSI-u negotiation. |
| `tmux send-keys C-i` interjects | **UNVERIFIED, expected false** | Same probe. `C-i` is ASCII `0x09` (Tab); almost certainly read as Tab. |
| An unrelated user binary named `grok`/`agy` in a member pane could cause a spurious skip | **UNVERIFIED, low impact** | `command -v grok agy` on target hosts. Failure mode is a retryable skip (`daemon.rs:1392`), not a lost message. |
| agy/grok panes never surface as `node`/a wrapper in `pane_current_command` | **Verified for Linux/WSL** by both reports; **UNVERIFIED on Windows/macOS builds** (`agy-report-opus.md:51`) | `tmux display-message -p '#{pane_current_command}'` in a live pane on each platform. |
| No `.exe` variants matter | **UNVERIFIED** | `known_agent_cli` already strips `.exe` (`:349`), so `agy.exe`/`grok.exe` are handled if they ever appear. No action needed. |

---

*Compiled read-only. No files were modified in `~/projects/mesh`, `~/projects/taurhaus`, or `~/projects/taureval`; no processes were started.*
