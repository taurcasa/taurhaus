# Grok attach spike — can an external process deliver into a RUNNING Grok TUI?

Host: WSL2 Linux, user `mstie`. Binary `~/.local/bin/grok` → `grok 1.0.5 (5115b46bc9) [stable]`.
Date: 2026-08-28. Scratch workspace: `…/scratchpad/grok-spike/{work,work2}` (non-git, verified
`fatal: not a git repository`). All repos untouched; no git write commands run.

This spike settles the question left **UNVERIFIED** in `grok-report-opus.md:993`
("`session/load` + `session/prompt` into a session that a live TUI currently owns") and
independently re-tests the claim in `grok-report-codex.md:478` that same-session injection works.

---

## Result

**Yes — but only when the TUI itself was launched leader-attached. Every other configuration
silently forks the session instead of delivering.**

| # | Path tested | Delivered into the live TUI? | On-disk outcome |
|---|---|---|---|
| a | `grok agent --no-leader stdio` → `session/load` live session → `session/prompt` | **NO** — pane unchanged, pane title still the old title | **Silent fork.** Two turns both recorded `turn_number: 1`, `conversation_message_count: 7` |
| b1 | Leader on a custom socket + TUI launched `--leader --leader-socket S` + external `grok agent --leader --leader-socket S stdio` | **YES** — pane rendered `❯ reply OK2`, spinner, then `OK2`, live | **Coherent.** turn_number 0→1→2, message counts 3→7→10 |
| b2 | Same leader, but TUI launched **without** `--leader` (host default) | **NO** — pane unchanged | **Silent fork** again |
| c | Documented `grok … send`-like subcommand | **Does not exist** | — |

Three further behaviours, all verified:

- **ACP `session/prompt` queues; it does not interrupt.** Prompting a *busy* leader-attached
  session was accepted, held at queue `position: 0` behind the TUI's running turn, and ran as its
  own turn 7 ms after the running turn ended. ACP therefore reproduces the **plain-`Enter` tier
  only**. There is no ACP equivalent of the send-now/interject chord in the advertised capability
  document.
- **Leader mode wrote to the user's real `~/.grok/config.toml`** (see Side effects — this is the
  one thing in this spike that needs a decision).
- **Unix-socket path limit is real.** A leader socket inside the scratchpad (117 chars) failed with
  `Error: Timeout waiting for IPC socket to be created`; a 34-char path under `/tmp/claude-1000/`
  worked. `SUN_LEN` caps the path at ~107 bytes.

### Why (a) and (b2) fork rather than fail

Leader mode is **per-client opt-in** and off by default on this host (`~/.grok/config.toml` has no
`[cli] use_leader`). A TUI started without `--leader` runs its agent **in-process**. An external ACP
client therefore instantiates a *second, independent* agent that loads the same session directory
from disk. Both agents then believe they own the conversation. Nothing errors, nothing locks, and
the on-disk JSONL is an interleaved merge of two divergent branches — it *looks* coherent when read
back, which is what makes this dangerous.

---

## Evidence

### Setup

```
tmux server:   tmux -L gspikef697bf08          (dedicated server, never the user's)
leader:        grok agent --leader-socket /tmp/claude-1000/gsp-f697bf08.sock leader \
                    --relay-on-demand --no-auto-update --no-exit-on-disconnect     (pid 270608)
TUI (plain):   grok --no-alt-screen --model grok-4.6 --reasoning-effort low \
                    --disable-web-search --no-subagents
TUI (leader):  grok --leader --leader-socket /tmp/claude-1000/gsp-f697bf08.sock  …same flags…
ACP client:    grok agent [--leader --leader-socket S | --no-leader] stdio
               newline-delimited JSON-RPC 2.0; script at scratchpad/grok-spike/acp_client.py
```

`--leader` / `--no-leader` are **undocumented-but-accepted global flags on the top-level `grok`
command**, not just on `grok agent`. `grok --help` lists only `--leader-socket`; `README.md:304` and
`docs/user-guide/02-authentication.md:224` mention `--leader`. Verified by parse probe: the parser
rejects unknown flags (`error: unexpected argument '--zzz-not-a-flag' found`) but accepted
`grok --leader …` and `grok --no-leader …`.

### (a) `--no-leader` stdio into a live TUI session — silent fork

Live TUI session `<uuid>`, pid 258872, registered in
`~/.grok/active_sessions.json`. External client output:

```
ARGV: ~/.local/bin/grok agent --no-leader stdio
initialize: OK   loadSession=True   sessionCaps=['list','resume','close']
session/list: 1 sessions
   <uuid>  title='Reply OK Simple Acknowledgment Request'
session/load:   OK
session/prompt: OK (2.3s) -> {"stopReason":"end_turn", … "promptId":"47fc9e0e-…"}
```

TUI pane immediately after — **unchanged**, still showing only the first exchange:

```
   ❯ reply OK                                        7:44 PM
   ◆ Thought for 0.1s
   OK                                                7:44 PM
   Worked for 4.0s
```

`pane_title` also stayed on the stale title (`Reply OK Simple Acknowledgment Request - grok`) while
`summary.json` on disk had already advanced to `generated_title: "Simple OK and OK2 replies"`,
`num_messages: 8`.

A third prompt was then typed into the TUI by keystroke. `events.jsonl` shows the fork:

```
17:44:53 turn_started turn_number=0 conversation_message_count=3
17:45:29 turn_started turn_number=1 conversation_message_count=7   <- external ACP client
17:46:01 turn_started turn_number=1 conversation_message_count=7   <- the TUI, same numbers
```

Both processes started "turn 1" from the same 7-message prefix. The TUI's model call did **not**
contain the externally injected `reply OK2` turn. `chat_history.jsonl` nevertheless reads as a
clean `OK → OK2 → OK3` sequence, because both branches appended to the same file in wall-clock
order. **The corruption is invisible to anything that reads the transcript back.**

### (b1) Leader-attached TUI — real, live delivery

Leader socket confirmed carrying the TUI as a client:

```
$ ss -xp | grep gsp-f697bf08
u_str ESTAB 0 0 /tmp/claude-1000/gsp-f697bf08.sock 65672370 * 65650196 users:(("grok",pid=270608,fd=35))
```

External client (separate process) against session `<uuid>`:

```
ARGV: ~/.local/bin/grok agent --leader --leader-socket /tmp/claude-1000/gsp-f697bf08.sock stdio
session/list: 2 sessions
   <uuid>  title='User Requests Simple OK Reply'   <- the live TUI
session/load:   OK
session/prompt: OK (2.8s) -> {"stopReason":"end_turn", … "sessionId":"01a0497c-…"}
```

Pane captures while the external prompt ran — the TUI renders the injected turn **live**:

```
t=3s  pane_title=[⠙ - Waiting for response… - User Requests Simple OK Reply - grok]
   ❯ reply OK2                                       7:48 PM
  ⠼ Waiting for response… 2.5s                             2.5s ⇣13.4k [stop]
   Shift+Tab:mode  │  Esc:cancel  │  Ctrl+x:shortcuts

t=6s  pane_title=[Simple OK ping replies - grok]
   ❯ reply OK2                                       7:48 PM
   ◆ Thought for 0.0s
   OK2                                               7:48 PM
   Worked for 2.8s
```

Both clients see one backend and one context. A keystroke turn typed afterwards continued
coherently — no duplicate turn numbers anywhere:

```
turn_started 0 count=3      (TUI keystroke   "reply OK")
turn_started 1 count=7      (external ACP    "reply OK2")
turn_started 2 count=10     (TUI keystroke   "reply OK3")
turn_started 3 count=13     (TUI keystroke   "count from 1 to 30 …")
turn_started 4 count=16     (external ACP    "reply OK4")
turn_started 5 count=19     (TUI keystroke   "print the numbers 1 through 200 …")
turn_started 6 count=22     (external ACP    "reply OK5")
```

`updates.jsonl` `user_message_chunk` records read `reply OK → reply OK2 → reply OK3 → count from 1
to 30 … → reply OK4` — TUI-typed and externally-injected prompts are indistinguishable in the
session record.

Disconnecting the ACP client left the TUI resident and usable.

### (b2) Leader-backed client aimed at a non-leader TUI — forks

Session `<uuid>` (TUI pid 290943, launched without `--leader`).
`session/load` + `session/prompt` returned `OK (2.9s)`, disk advanced to `turn_number: 1`, and the
TUI pane was unchanged. Being connected to a leader is not enough — **the TUI must be the one
attached to it.**

### Queue vs. interject (ACP prompt into a busy session)

TUI busy on a long turn (`pane_title=[⠹ - Responding - …]`). External ACP prompt sent mid-turn:

```
_x.ai/queue/changed  entries=[{"id":"04fe348f-…","kind":"prompt","text":"reply OK5","position":0}]
                     runningPromptId="f450d610-…"
                     runningText="print the numbers 1 through 200, one per line, nothing else"
_x.ai/queue/changed  entries=[]  runningPromptId="04fe348f-…"  runningText="reply OK5"
```

`events.jsonl`: `turn_ended` 17:52:07.452 → `turn_started` 17:52:08... the injected turn began
**7 ms after** the running turn finished. `session/prompt` blocked 6.16 s wall until its own turn
completed. So ACP delivery == plain `Enter` (queue). The `initialize` capability document advertises
only `x.ai/fs_notify`, `x.ai/hooks`, `x.ai/capabilities` — **no send-now/interject extension.**

### (c) `send`-like subcommand

None. `grok --help` command list contains no `send`. `grep -rniE "grok +send|send-message|sendMessage"`
over `~/.grok/docs/user-guide/*.md` returns no command; the only delivery API documented is ACP
(`15-agent-mode.md:102` "**Send prompts** — client sends `session/prompt` with user messages").

The keystroke tiers *are* documented, and are richer than they look
(`03-keyboard-shortcuts.md:261,263,277`):

- plain `Enter` → **queues** a follow-up, picked up at the next turn boundary, does not stop the agent
- send-now chord (`Ctrl+Enter` / `Ctrl+I`; `Ctrl+L` on VS Code-family terminals) → **cancel-and-send**:
  "it stops the current turn (background tasks, subagents, and the rest of the queue keep running)
  and sends your message as the next turn". Doc: *"Send-now is intentionally interruptive — it reads
  as 'stop what you're doing and take this'."*
- `[ui] follow_up_behavior = "steer"` makes plain `Enter` inject mid-turn at the next safe gap
  (`05-configuration.md:79`)

### Operational gotchas found

- **Socket path length.** `--leader-socket <117-char path under the scratchpad>` →
  `Error: Timeout waiting for IPC socket to be created`, no socket created. Keep it under ~107 bytes.
- **`grok leader list|info` ignore `--leader-socket`.** Both reported the *home-default*
  `~/.grok/leader.sock` (stale, `classification: "Unreachable"`) regardless of flag position:
  `Error: leader target for wss://code.grok.com/ws/code-agent has an unreachable socket`.
  taurhaus cannot health-check a custom leader socket with these commands — use `ss -xp` / the
  socket file + a probe `initialize` instead.
- Grok writes TUI escape sequences to **stderr**. Redirecting stderr blanks the tmux pane; do not
  redirect it if you intend to `capture-pane`.
- In leader mode the TUI briefly showed the leader's default effort (`Grok 4.6 (high)`) before
  settling to the requested `(low)`.

---

## Side effects on `~/.grok`

Read-only baseline taken before the spike (`grok-spike/grok-home-baseline.txt`); diff by mtime after.
Everything below is a write by **grok itself**; I edited nothing under `~/.grok`.

**Needs a decision — grok persisted my launch flags as the user's global defaults:**

`~/.grok/config.toml` was rewritten at 19:47:49 (440 → 504 bytes), gaining:

```toml
[models]
default = "grok-4.6"
default_reasoning_effort = "low"
```

Attribution: `unified.jsonl` shows `pid 270608` (**the leader**) logging `model changed
{"model":"grok-4.6"}` at 17:47:49.329Z and 17:47:49.347Z, and `config.toml` mtime is 17:47:49.356Z.
The earlier **non**-leader TUI launched at 19:44:07 with *identical* `--model` / `--reasoning-effort`
flags did **not** touch `config.toml`. So: **in leader mode, a client's per-launch `--model` /
`--reasoning-effort` are applied as a global model change on the shared backend and written through
to the user's `config.toml`.**

I did not revert it (hard rule: do not modify `~/.grok`). `default_reasoning_effort = "low"` will
lower the default effort of the user's future Grok sessions until it is removed. Recommend the user
or team-lead delete those three lines.

**Routine writes (expected):**

| Path | Change |
|---|---|
| `sessions/…%2Fgrok-spike%2Fwork/{01a04979…,01a0497c…}/` | 2 new session dirs (this spike) |
| `sessions/…%2Fgrok-spike%2Fwork2/01a0497e…/` | 1 new session dir (this spike) |
| `sessions/session_search.sqlite` | 114,688 → 118,784 B (FTS index) |
| `active_sessions.json` | registered 3 sessions, back to `[]` after `/quit` |
| `logs/unified.jsonl` | 542,290 → 755,189 B |
| `memtrace/1787939076-258872.jsonl`, `…-271585.jsonl`, `…-270608.jsonl`, `…-290943.jsonl` | 4 new per-process traces |
| `relocations/*.lock` | 3 new zero-byte session locks |
| `campaigns_state.json` (+ `.lock`) | **new file**, `{"dismissed_ids":["grok-4.6-launch"]}` |
| `models_cache.json`, `version.json`, `CHANGELOG.{md,json}`, `tip_cursor.json`, `slash-mru.json` | refreshed by startup/update check |

**Untouched:** `auth.json` (never read or copied — the spike used the real `GROK_HOME`, so no
credential material was duplicated anywhere), `agent_id`, `worktrees.db`,
and the pre-existing stale `~/.grok/leader.sock` / `leader.lock` (mtime still `01:23:13`, from an
earlier probe, not mine — left in place).

A session group `…%2Fscratchpad%2Fprobe` was written at 19:43:42, ~25 s before my first launch. Not
mine; concurrent activity from another agent on this host.

**Cleanup (trap-based, `grok-spike/cleanup.sh`):** both TUIs `/quit` gracefully, leader `SIGTERM`,
`tmux -L gspikef697bf08 kill-server`, custom socket removed, stale tmux socket file removed.
Verified after: no `grok` processes, `active_sessions.json == []`, custom socket gone. No process
that I did not start was signalled.

---

## Recommendation

**Keep tmux delivery. Add the interject key as a second tier. Do not adopt ACP for delivery in v1.**

The opus report's recommendation stands, and this spike hardens it with a reason it did not have:
ACP delivery is not merely "unverified", it is **conditionally correct and silently destructive when
the condition fails**.

1. **Keep `tmux send-keys` as the delivery mechanism.** It is the only path that works against a
   Grok TUI regardless of how it was launched, it is the code path taurhaus already runs for Claude
   and Codex, and it cannot fork a session.

2. **Add the interject tier now — it is free.** Grok's keystroke API is genuinely two-tier and maps
   onto taurhaus's existing message prefixes:
   - `INFO ONLY:` → text + plain `Enter` → **queued**, lands at the next turn boundary, does not
     stop the member. (This is exactly what taurhaus does today.)
   - `ACTION REQUIRED:` → text + send-now chord → **cancel-and-send**, stops the current turn and
     takes the message next.
   Chord selection is terminal-dependent (`Ctrl+Enter` / `Ctrl+I` normally; `Ctrl+L` on VS
   Code-family terminals per `03-keyboard-shortcuts.md:297`; WezTerm needs
   `enable_kitty_keyboard = true` per `:279`), so make it a per-runtime setting with plain `Enter`
   as the safe default. Note this is a *real* interrupt — it cancels the running turn — so gate it
   behind the escalation path only.

3. **Do not adopt ACP as the writer.** Adopting it would require all four of: taurhaus owns every
   member launch, taurhaus runs and supervises a leader per socket, the socket path stays under
   ~107 bytes, and no member is ever restarted outside taurhaus. The moment the last one is
   violated — a user restarts a member by hand, or attaches to an existing session — writes stop
   reaching the TUI and start forking the transcript **with no error to detect it by**. That is a
   worse failure mode than a mistyped keystroke. It also buys less than it looks: ACP gives the
   queue tier only, so keystrokes are still needed for the interrupt tier, and leader mode leaks
   per-member `--model` / `--effort` into the user's global `config.toml` (see Side effects) and
   into every other member on that leader.

4. **Revisit ACP later as a read-only telemetry channel, not a delivery channel.** A leader-attached
   `grok agent --leader --leader-socket S stdio` observer gets `_x.ai/sessions/changed`
   (`activity: working|idle`), `_x.ai/queue/changed` (with `runningPromptId` / `runningText`),
   `_x.ai/session/prompt_complete`, and per-turn usage — strictly better busy/idle than pane-title
   scraping, and read-only traffic cannot fork anything. That is the follow-up worth doing; it is
   independent of how messages are delivered.

5. **If ACP delivery is ever pursued anyway**, treat "is this session resident on my leader?" as a
   hard precondition on every write: `session/list` over the leader is **not** sufficient (it also
   lists non-resident on-disk sessions — that is how (b2) forked). Gate on
   `_x.ai/sessions/changed`'s `resident: true` for that exact `sessionId`, and refuse to prompt
   otherwise.

### Unverified / would settle it

- Whether `resident: true` in `_x.ai/sessions/changed` reliably distinguishes a leader-hosted
  session from a disk-only one. Verify: subscribe an observer to the leader, then start one TUI
  with `--leader` and one without, and compare the `resident` flags for the two session ids.
- Whether any `_x.ai/*` request method exposes send-now / dequeue (the queue is only ever *reported*
  in what I observed). Verify: capture a leader-attached TUI's own outbound frames while pressing
  the interject chord.
- What happens when the TUI and an external client submit at exactly the same instant. Both my
  attempts landed after the running turn started, so I only demonstrated queueing, not true
  simultaneous arbitration.
- Whether the `config.toml` write-through is triggered by leader attachment specifically or by the
  `grok-4.6-launch` campaign dismissal that shares its timestamp. Verify: relaunch a leader-attached
  TUI with `--reasoning-effort medium` against an isolated `GROK_HOME` and re-read `config.toml`.
