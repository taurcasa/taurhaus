# SPIKE 1b — Can a taurhaus-style team member invoke Workflow when told to?

**Answer: YES — the baseline variant works. No variant was needed.**

Date: 2026-08-28 · Claude Code v2.1.251 · tmux 3.4 · model `--model opus` (resolved `claude-opus-5`)

---

## 1. Result in one line

A team member launched exactly the way taurhaus launches them, handed a mesh-style
`tmux send-keys` operator notice, **invoked the `Workflow` tool by name on the first try,
with zero permission prompts and zero opt-in prompts, and replied with the returned word.**

Verified 4 times (words ZED, ACE, BOP, KIT) across two independent config dirs.

---

## 2. Variant matrix

| Variant | Tried? | Outcome |
|---|---|---|
| **Baseline** — notice says literally `Invoke Workflow({name:"ok-args", args:{"word":"ZED"}})` | yes | **WORKS.** Tool invoked, `ZED` returned and reported. |
| (a) phrased "use a workflow" | **not needed** | Baseline did not refuse, so the fallback was never exercised. Untested. |
| (b) `--settings '{"ultracode":true}'` | **not needed** | Baseline did not refuse. Untested. **The `Workflow` tool was available and callable without any ultracode opt-in.** |

---

## 3. Exact launch used

```bash
cd <scratch>/wf-spikes/proj
CLAUDE_CONFIG_DIR=<iso> \
CLAUDECODE=1 \
CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 \
claude --model opus --dangerously-skip-permissions \
  --team-name wf-spike --agent-name worker --agent-id worker-1 --agent-type worker
```

Notes:
- `--team-name / --agent-name / --agent-id / --agent-type` are **hidden flags** — they do not
  appear in `claude --help`, with or without `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`.
  They were accepted silently (no "unknown option"); team mode was confirmed live by the
  pane rendering the prompt as `@worker` and the footer offering `← for agents`.
- Startup banner: `Opus 5 with xhigh effort · Claude Max`, `⏵⏵ bypass permissions on`.

## 4. Delivery, exactly as mesh does it

Two separate calls, text then Enter:

```bash
tmux send-keys -t <sess> -l 'ACTION REQUIRED: Invoke Workflow({name:"ok-args", args:{"word":"ZED"}}) and reply with the returned word.'
tmux send-keys -t <sess> Enter
```

This delivered cleanly. The member picked the notice up as a normal user turn.

## 5. What the member actually did

Tool call recorded in the member transcript:

```json
{"name": "ok-args", "args": "{\"word\":\"ZED\"}"}
```

- It invoked **by name** — it did **not** read the script file and pass it inline.
  (`Workflow` was the only tool call in the turn.)
- `args` was passed as a **JSON string**, not an object, because the notice text was
  transcribed literally. The runtime **parsed it anyway** — the script saw `args.word === "ZED"`.
  So a slightly malformed `args` in an operator notice is tolerated.
- The tool returned immediately: `Workflow launched in background. Task ID: w6n3rvyos`.
  The member then sat on `✻ Waiting for 1 dynamic workflow to finish` and reported once the
  `<task-notification>` with `<result>` arrived. **Workflow is async** — the member does not
  block inside the tool call.

Pane transcript of the answer:

```
● Workflow(dynamic workflow: ok-args)
  ⎿  ✔ Completed in 1s · 1 agent · 19k tokens
● The workflow is running in the background — I'll report the returned word once it completes.
✻ Waiting for 1 dynamic workflow to finish
● Dynamic workflow "Echo back the word passed in args.word" completed · 1s
● The workflow completed and returned:
  ZED
  Full result: {"word":"ZED","agentSaid":"ZED"}
```

**The member answered ZED.** ✅

## 6. Run dir — confirmed present

Run artifacts landed exactly where expected, under the isolated `CLAUDE_CONFIG_DIR`:

```
<iso>/projects/<slugified-cwd>/<session-uuid>/subagents/workflows/wf_62c479fd-302/
    journal.jsonl                      # {"type":"result",...,"result":"ZED"}
    agent-a6686644a7860582c.jsonl
    agent-a6686644a7860582c.meta.json  # {"agentType":"workflow-subagent","spawnDepth":1}
<iso>/projects/<slugified-cwd>/<session-uuid>/workflows/
    wf_62c479fd-302.json               # full run record incl. result + totalTokens
    scripts/ok-args-wf_62c479fd-302.js # snapshot of the script that ran
```

The run record confirms `"defaultModel": "claude-opus-5"` — **the workflow subagent inherited
the member's `--model opus`**, it did not fall back to the `settings.json` model
(`claude-fable-5[1m]`).

## 7. Workflow registry: where the script is found

Two follow-up tests, both decisive:

**(i) Project-level `.claude/workflows/` IS discovered.**
With a private config dir containing **no** global `workflows/` directory, and only
`<proj>/.claude/workflows/ok-args.js` present (description edited to the marker
`PROJECTLEVEL echo of args.word`), the member resolved the name and ran it:

```
◯ ok-args  PROJECTLEVEL echo of args.word   1/1 agents done · 1s · ↓ 18.9k tokens
● KIT   Full result: {"word":"KIT","agentSaid":"KIT"}
```

So a workflow file committed into a project repo is visible to a taurhaus-launched member.

**(ii) The script is snapshotted at session start, not re-read per invocation.**
Within one live member session I (a) edited the project file's `description` and (b) then
renamed the global copy away entirely. Runs 2 and 3 in that same session **still executed the
original pre-edit text**, and run 3 succeeded even though the global file no longer existed.

> **Operational consequence for taurhaus:** editing or adding a workflow file does **not**
> affect an already-running member. A member must be restarted to pick up workflow changes.

*UNVERIFIED:* precedence between global `<CLAUDE_CONFIG_DIR>/workflows/` and project
`.claude/workflows/` when a same-named file exists in both. Runs 1–3 had byte-identical files
in both locations, so which one supplied the text cannot be attributed. Only "project-level
alone works" (i) and "cached at start" (ii) are established.

## 8. Permission / opt-in prompts observed

| # | Prompt | When | Genuine? |
|---|---|---|---|
| 1 | **Workspace trust gate** — "Quick safety check: Is this a project you created or one you trust?" → `No, exit` / `Yes, I trust this folder` | Every first launch in an untrusted cwd, on **both** config dirs | **YES — real, and it blocks startup.** |
| 2 | Theme picker ("Choose the text style…") | One relaunch only | **NO — contamination.** See §9. |
| 3 | Login method ("Select login method: 1. Claude account with subscription…") | Same relaunch only | **NO — contamination.** See §9. |

**Prompts NOT seen — the important negatives:**
- **No permission prompt for the `Workflow` tool itself**, on any of the 4 invocations.
- **No opt-in / ultracode gate.** `Workflow` was callable straight away.
- No dangerous-mode confirmation. (Caveat: the copied `settings.json` carries
  `"skipDangerousModePermissionPrompt": true`, so that particular prompt was pre-suppressed
  and this spike cannot say whether it would otherwise appear.)

**The trust gate (#1) is the only real blocker**, and it reproduced cleanly on a fresh
private config dir. It appears **despite** `--dangerously-skip-permissions`. Any taurhaus
automation launching members into a new project dir must pre-seed trust in `.claude.json`
or drive this prompt, or the member will sit at a menu forever.

## 9. Test-environment contamination (disclosure)

Concurrent sibling agents (spikes 1a / 2) were working in the **same** `wf-spikes/` scratch
directory and shared the same `iso/` config dir — they created `cwd-clean/`, `out/`,
`spike1a-private/`, `spike2/` and rewrote `iso/.claude.json` and `iso/.credentials.json`
mid-run. That rewrite dropped `hasCompletedOnboarding`, which is what produced prompts #2
and #3 on one relaunch. **Those two prompts are artifacts of the shared directory, not of the
taurhaus launch pattern.**

I therefore re-ran the final test in a private `iso-1b-private/` config dir, where the clean
launch showed **only** the trust gate. I also narrowed my cleanup script to kill only my own
named tmux sessions (never `pkill` by path) so as not to kill sibling processes, and I
restored the global workflow file I had temporarily parked.

An unrelated stray run dir `…-wf-spikes-cwd-clean/…/wf_d59164ea-417` in the shared `iso/`
belongs to a sibling agent and is **excluded** from all figures below.

## 10. Token cost (from the JSON run records)

| runId | args | result | tokens | duration |
|---|---|---|---|---|
| `wf_62c479fd-302` | `{"word":"ZED"}` | `{"word":"ZED","agentSaid":"ZED"}` | 19,022 | 1592 ms |
| `wf_3b14d065-96d` | `{"word":"ACE"}` | `{"word":"ACE","agentSaid":"ACE"}` | 19,022 | 1615 ms |
| `wf_c47add9d-2dc` | `{"word":"BOP"}` | `{"word":"BOP","agentSaid":"BOP"}` | 19,020 | 1873 ms |
| `wf_a0447e24-183` | `{"word":"KIT"}` | `{"word":"KIT","agentSaid":"KIT"}` | 18,938 | 1686 ms |

**Total: 76,002 tokens across 4 workflow runs** (1 agent each, 0 tool calls each).

Note the floor: a *trivial* one-agent workflow costs ~19k tokens, essentially all of it the
subagent's system prompt. Workflow fan-out is not cheap per-agent.

Member-session (outer TUI) tokens are not included — they were not separately recorded.

## 11. Cleanup

- `/exit` cleanly terminated both members (tmux session ended with the process).
- All tmux sessions I created (`wfspike1b`, `wfspike1ba`, `wfspike1bb`) killed; verified only
  the user's pre-existing `taurhaus` session remains.
- No `claude` process of mine left running; watchdog timer retired.
- Never wrote to real `~/.claude*`; never touched `~/projects/taurhaus`.

## 12. Bottom line for taurhaus

1. A taurhaus-launched Claude team member **can** invoke `Workflow` on a plain
   `tmux send-keys` operator notice. Baseline phrasing is enough; no ultracode flag,
   no rephrasing, no permission grant.
2. The **workspace trust gate is the one thing that will block an automated launch** into a
   fresh project dir — pre-seed it.
3. `Workflow` is **async**: the member returns from the tool immediately and reports later off
   a task notification. Any mesh-side "wait for the member's answer" logic must tolerate the
   gap (~5–20 s here for a 1.6 s workflow).
4. Workflow scripts are **snapshotted at member start** — ship workflow files before launching
   members, and restart members to pick up edits.
5. Budget ~19k tokens per workflow agent as a floor.
