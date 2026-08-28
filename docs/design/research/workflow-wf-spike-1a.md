# SPIKE 1a — user-scope workflow resolution

Claude Code **2.1.251**. Date 2026-08-28. All runs `--model opus` (resolved `claude-opus-5`),
`-p --dangerously-skip-permissions`, isolated `CLAUDE_CONFIG_DIR`.

## Verdict

**YES — `<CLAUDE_CONFIG_DIR>/workflows/<name>.js` resolves as a user-scope named workflow.**
No `.claude/workflows/` in cwd is required. Both the `Workflow({name})` tool path and the
`/<name>` slash form resolve from it, and the file is executed **byte-identically** (verified by
diff against the persisted run script).

## Environment

- Isolated config dir: `<ISO>` = `…/scratchpad/wf-spikes/spike1a-private/iso`
  containing only `.credentials.json`, `.claude.json` (oauthAccount subset), `settings.json`,
  plus the `workflows/` dir under test. No `projects/`, `sessions/`, `teams/`, `todos/` copied.
- Clean cwd: `…/spike1a-private/cwd-clean` (empty, **no** `.claude/`).
- Project-scope cwd: `…/spike1a-private/proj` with `.claude/workflows/`.
- `$HOME/.claude/workflows/` did **not** exist at any point (child `ls` returned exit 2).
- Leak check: 0 files matching any of my 4 child session ids under `~/.claude` or
  `~/.claude-account2`. Nothing written to the real config dirs.

## Workflow file under test (exact bytes, 92 B, md5 `123bcd3865f4d9a26773cac0bf2a06fb`)

```js
export const meta = {name: "ok-args", description: "echo args"}

return { word: args.word }
```

## Runs

| # | Location of `ok-args.js` | cwd | Invocation | `ok-args` in `slash_commands` | Result |
|---|---|---|---|---|---|
| A | `<ISO>/workflows/` (user scope) | `cwd-clean` | prompt: "Run the named workflow ok-args with args {"word":"ZED"} …" | **true** | `ZED` |
| B | `<ISO>/workflows/` (user scope) | `cwd-clean` | `/ok-args {"word":"ZED"}` | **true** | workflow returned `{"word":"ZED"}` |
| C | `<cwd>/.claude/workflows/` (project scope; user scope moved aside) | `proj` | same as A | **true** | `ZED` |
| D | nowhere (both scopes empty) | `cwd-clean` | "Is a workflow named ok-args available…" | **false** | `NO` |

D is the negative control: with the file removed from both locations the name disappears from
the session's `slash_commands` list, which pins the registry to those two directories.

### Run A — the primary result

Tool call emitted by the child model:

```json
{"name": "ok-args", "args": "{\"word\":\"ZED\"}"}
```

Tool result (truncated):

```
Workflow launched in background. Task ID: wbeo02sxg
Summary: echo args
Transcript dir: <ISO>/projects/-tmp-…-spike1a-private-cwd-clean/f30b462c-…/subagents/workflows/wf_d95d43cb-591
Script file:    <ISO>/projects/-tmp-…-spike1a-private-cwd-clean/f30b462c-…/workflows/scripts/ok-args-wf_d95d43cb-591.js
Run ID: wf_d95d43cb-591
```

`Summary: echo args` is my `meta.description` verbatim. The persisted script file
`diff`s **IDENTICAL** to my source file. `md5sum` of the source file was unchanged across the run.

Workflow return value (task output file):

```json
{"summary":"echo args","agentCount":0,"logs":[],"result":{"word":"ZED"},
 "workflowProgress":[],"totalTokens":0,"totalToolCalls":0}
```

Final stdout `result`: `ZED`.

### Run dirs that appear under the isolated config dir

```
<ISO>/projects/-tmp-…-spike1a-private-cwd-clean/<session-uuid>/workflows/scripts/ok-args-wf_<runid>.js
<ISO>/projects/-tmp-…-spike1a-private-cwd-clean/<session-uuid>/subagents/workflows/wf_<runid>/
        agent-<id>.jsonl  agent-<id>.meta.json  journal.jsonl
<ISO>/sessions/  <ISO>/backups/  <ISO>/projects/-tmp/memory/
```

The `subagents/workflows/wf_<runid>/` dir is created only when the script spawns at least one
`agent()`. The minimal script spawns none, so run A produced only the `workflows/scripts/` entry;
an earlier run of a script with one `agent()` produced the full `wf_<runid>/` dir with
`journal.jsonl` containing `{"type":"result",…,"result":"ZED"}`.

The **task output** file is *not* under the config dir — it lands in the session scratch tree:
`/tmp/claude-1000/<cwd-slug>/<session-uuid>/tasks/<taskid>.output`.

## Incidental findings

1. **`args` may arrive as a JSON string and is still parsed.** The child model passed
   `args: "{\"word\":\"ZED\"}"` (a string, not an object) in every run, yet `args.word`
   evaluated to `"ZED"` inside the script. So the runtime JSON-parses a string `args`.
   UNVERIFIED whether a non-JSON string is passed through raw.
2. **Named workflows run as a background task.** The `Workflow` tool returns immediately with
   "Workflow launched in background. Task ID: …". In the `stream-json` transcript of run A1 the
   `-p` session emitted **two** `result` events — first
   `"The workflow is running in the background; I'll report the returned word once it completes."`,
   then, after the `task_notification`, `"ZED"`. With `--output-format json` the single emitted
   `result` was already `ZED`. UNVERIFIED whether a slow workflow can outlive the `-p` turn.
3. The `/<name>` slash form does not invoke the script directly — it expands into an instruction
   and the model then issues the `Workflow({name, args})` tool call (visible in run B's transcript).
4. **Cross-agent collision (procedural, not a product finding).** My first two runs used the shared
   `…/scratchpad/wf-spikes/` dir; a sibling spike agent overwrote `wf-spikes/iso/workflows/ok-args.js`
   between my write (22:30) and my run (22:32), so those two runs executed a *different* script that
   happened to carry the same `meta.name`. Detected by diffing the persisted run script. All
   conclusions above come from re-runs in the private `spike1a-private/` subtree with md5 checks
   before and after each run.

## Cost

Per-run `total_cost_usd` from the JSON `result` events:

| Run | Cost USD | in | out | cache write | cache read |
|---|---|---|---|---|---|
| A0 named/user-scope, `--output-format json` (contaminated) | 0.108476 | 4 | 267 | 8759 | 26400 |
| A1 named/user-scope, stream-json (contaminated) | 0.163455 | 2 | 5 | 1160 | 18053 |
| A named/user-scope (clean) | 0.109508 | 4 | 259 | 8881 | 26414 |
| B slash `/ok-args` (user scope) | 0.076787 | 4 | 164 | 5790 | 29533 |
| C named/project-scope control | 0.127607 | 6 | 500 | 9239 | 43383 |
| D negative control | 0.075246 | 4 | 730 | 4127 | 29438 |
| **TOTAL** | **0.661079** | 24 | 1925 | 37956 | 173221 |

(A1's `usage` block reports only the final turn's tokens; its `total_cost_usd` is the whole-run figure.
Costs are the CLI's own list-price accounting, not billed spend.)

## Artifacts

- `…/scratchpad/wf-spikes/spike1a-private/out/A_named_userscope.jsonl`
- `…/scratchpad/wf-spikes/spike1a-private/out/B_slash_userscope.jsonl`
- `…/scratchpad/wf-spikes/spike1a-private/out/C_named_projectscope.jsonl`
- `…/scratchpad/wf-spikes/spike1a-private/out/D_negative.jsonl`
- `…/scratchpad/wf-spikes/spike1a-private/out/ok-args.reference.js`
- `…/scratchpad/wf-spikes/spike1a-private/out/analyze.py`

## Cleanup

No lingering `claude -p` processes; no tmux sessions were started. The isolated config dir and
all scratch files remain under `…/scratchpad/wf-spikes/spike1a-private/` for inspection.
