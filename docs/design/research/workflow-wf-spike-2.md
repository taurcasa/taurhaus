# SPIKE 2 — live run tree from files

Everything below is measured from one controlled run unless marked **UNVERIFIED**.

## What ran

- CLI: `claude 2.1.251`, isolated `CLAUDE_CONFIG_DIR` containing only `.credentials.json`,
  `.claude.json` (`oauthAccount` key only) and `settings.json`.
  Scratch root: `/tmp/claude-1000/-home-mstie-projects-taurhaus/f3286b16-ffc7-4d16-915d-046705823a3d/scratchpad/wf-spikes/spike2/`
  (a sibling spike was already using `wf-spikes/` directly, so this spike stayed in `spike2/`).
- Parent command (cwd `…/spike2/proj`):
  `CLAUDE_CONFIG_DIR=<iso> claude -p --model opus --dangerously-skip-permissions --output-format json "<prompt>"`
  The prompt told it to call the Workflow tool with the script inline via `script`, unchanged.
- Workflow script — three trivial agents through `parallel()`:

```js
export const meta = {
  name: 'spike2-ok',
  description: 'Three trivial parallel agents, for a file-watcher spike',
  whenToUse: 'spike only',
  phases: [{ title: 'Fanout' }],
}
phase('Fanout')
const rs = await parallel([
  () => agent('Reply with the single word OK and nothing else. [alpha]', { label: 'alpha', phase: 'Fanout' }),
  () => agent('Reply with the single word OK and nothing else. [bravo]', { label: 'bravo', phase: 'Fanout' }),
  () => agent('Reply with the single word OK and nothing else. [charlie]', { label: 'charlie', phase: 'Fanout' }),
])
log(`fanout done: ${rs.join(',')}`)
return { rs }
```

- Result: `{"rs":["OK","OK","OK"]}`, `subtype: success`, parent wall clock 10.11 s,
  workflow `durationMs` 2336, `runId` `wf_e23f71b6-543`, session `7c403fe7-…`.
- Watcher ran at 50 ms stat poll / 1 s print, started 1.5 s before the CLI.
  Second independent process (`fs_probe.py`) walked the whole config dir at 10 Hz and
  recorded first-appearance time of every path — that is the layout ground truth.

## Observed run-dir layout

Everything hangs off one session dir
`<config>/projects/<cwd-slug>/<sessionId>/`:

```
<sessionId>/
  workflows/
    scripts/spike2-ok-wf_e23f71b6-543.js   612 B   t+5.597   <- <name>-<runId>.js, byte-identical to the submitted script
    wf_e23f71b6-543.json                  2663 B   t+7.960   <- the summary; see below
  subagents/
    workflows/
      wf_e23f71b6-543/
        journal.jsonl                      789 B   t+5.597 created, appended 4x
        agent-a23851b6e8d607833.meta.json    48 B   t+5.597
        agent-a23851b6e8d607833.jsonl      9992 B   t+5.701
        agent-a702ce972985fa239.meta.json    48 B   t+6.426
        agent-a707c3cec3f1be8aa.meta.json    48 B   t+6.426
        agent-a707c3cec3f1be8aa.jsonl      9998 B   t+6.530
        agent-a702ce972985fa239.jsonl      8601 B   t+6.530
```

(`t+` = seconds after the `claude` process started.) Also written at the session level:
`<sessionId>.jsonl` (the parent transcript, t+1.578).

- `meta.json` is `{"agentType":"workflow-subagent","spawnDepth":1}` — it carries **no** label,
  model, phase or token data.
- `journal.jsonl` records are minimal: `{"type":"started"|"result","key":"v2:<sha>","agentId":…}`
  (`result` may be a string or an object when the agent used a schema). **No** label, model,
  phase, timestamp or token fields.
- So label / model / tokens / tools must be reconstructed from `agent-*.jsonl` +
  the persisted script. That is what the watcher does.

## Latency: transcript write → watcher shows it

Two components, measured per file event (`mtime` from the same `stat` the watcher used):

| stage | n | min | median | max |
|---|---|---|---|---|
| file `mtime` → watcher read the bytes (50 ms poll) | 12 | 9.8 ms | 42.6 ms | 58.0 ms |
| file `mtime` → next 1 s table print | 12 | 9.8 ms | ~700 ms | 907.7 ms |

Per event (all times relative to CLI start):

```
journal.jsonl                 create   mtime t+5.560  read +51.1ms  shown t+5.663  (102.6ms)
agent-a23851b6e8d607833.jsonl create   mtime t+5.653  read + 9.8ms  shown t+5.663  (  9.8ms)
journal.jsonl                 append   mtime t+6.366  read +18.4ms  shown t+6.693  (326.7ms)
agent-a702ce972985fa239.jsonl create   mtime t+6.467  read +20.2ms  shown t+6.693  (225.4ms)
agent-a707c3cec3f1be8aa.jsonl create   mtime t+6.467  read +20.4ms  shown t+6.693  (225.4ms)
journal.jsonl                 append   mtime t+7.020  read +33.5ms  shown t+7.725  (704.6ms)
agent-a23851b6e8d607833.jsonl append   mtime t+7.105  read +52.8ms  shown t+7.725  (620.2ms)
journal.jsonl                 append   mtime t+7.843  read +36.0ms  shown t+8.751  (907.7ms)
wf_e23f71b6-543.json    summary-create  mtime t+7.881  read +49.1ms  shown t+8.751  (869.7ms)
journal.jsonl                 append   mtime t+7.877  read +53.4ms  shown t+8.751  (874.0ms)
agent-a702ce972985fa239.jsonl append   mtime t+7.927  read +53.7ms  shown t+8.751  (823.3ms)
agent-a707c3cec3f1be8aa.jsonl append   mtime t+7.923  read +58.0ms  shown t+8.751  (827.5ms)
```

Read latency is bounded by the poll interval and nothing else — no buffering delay was
observed on the writer side. Display latency is entirely the 1 s print quantisation.
A 10 Hz independent walker (`fs_probe`) saw new paths 7–80 ms after their `ctime`, so
polling `stat` is sufficient; no inotify was needed for this scale.

Run-dir discovery itself: the watcher found the run dir 37 ms after it was created
(t+5.560 vs the dir's t+5.597 first-walk sighting), i.e. 5.6 s into the CLI run —
that 5.6 s is CLI startup + the parent model's first turn, not watcher lag.

## Does `<runId>.json` appear mid-run?

**No — it appeared once, at the end.**

- Exactly one write event for `workflows/wf_e23f71b6-543.json`, at `mtime` t+7.881,
  already containing `"status":"completed"`. No earlier partial version, no rewrites.
- That is **4.3 ms after** the last `result` record was appended to `journal.jsonl`
  (t+7.877) — i.e. after the third and final agent finished, and 2.2 s before the CLI exited.
- Corroborating second data point from a different, long-running workflow observed live
  (`wf_e8705bda-7f0`, 10 agents, running ~3 h at observation time): its run dir had
  `journal.jsonl` + 10 `agent-*.jsonl`, several agents `done` and two still `running`, and
  **no `workflows/<runId>.json` at all**.
- Content when it does land: `runId, status, result, logs, agentCount, durationMs, totalTokens,
  totalToolCalls, defaultModel, workflowName, phases, script, scriptPath, taskId, startTime,
  timestamp, summary, workflowProgress`. `workflowProgress` is the only place that carries
  per-agent `label / phaseTitle / model / state / tokens / toolCalls / durationMs /
  promptPreview / resultPreview / queuedAt / startedAt / lastProgressAt`.

Consequence for a live run tree: **the summary file is useless while the run is live.**
The only mid-run sources are `journal.jsonl` (identity + started/done + result),
`agent-*.jsonl` (prompt, model, tools, tokens) and the persisted script (label, phase).

The persisted script, by contrast, **is** available from the start: it appeared in the same
10 Hz sample as the run dir itself (t+5.597), before any agent had produced a token. So
label and phase are recoverable from the first frame.

## Session registry (`<config>/sessions/<pid>.json`)

For a headless `claude -p` run the registry entry exists but **never says busy**:

```json
{"pid":960428,"sessionId":"7c403fe7-dfe4-4096-b21f-9fd5d529a776",
 "cwd":".../spike2/proj","startedAt":1787949431152,"procStart":"57490500",
 "version":"2.1.251","peerProtocol":1,
 "peerFeatures":["notify_idle","reply_across_default_dirs","artifact_yield"],
 "kind":"interactive","entrypoint":"sdk-cli",
 "pidDomain":"linux:e5064869ecdc46b588b5d286fe6e84d7:pid:[4026532226]",
 "tmux":"taurhaus:@1.%1","messagingSocketPath":"/run/user/1000/cc-socks/960428.sock",
 "name":"proj-6c","nameSource":"derived","nameSince":1787949431152}
```

- The `status` and `statusUpdatedAt` keys are **absent entirely** — not `"idle"`, not `"busy"`.
  The file was written once at t+1.375 and its bytes never changed for the whole run
  (10 Hz content diff, zero updates).
- `kind` is `"interactive"` even though this is `-p`; `entrypoint` is `"sdk-cli"` — that is the
  field that distinguishes headless here.
- Both `<pid>.json` and its sibling `<pid>.<sha>.key` were **deleted** between t+8.75 and
  t+9.78 (1 s sampling bracket), ~0.3–1.4 s before the process exited at t+10.11.
- `tmux` was inherited from the launching shell's pane, so it points at the *parent agent's*
  tmux pane, not at anything this run owns. Treating that field as ownership would be wrong.

So a taurhaus-side activity signal cannot come from the registry for headless workflow runs:
no busy transition is ever published. Watching the run dir is the only live signal.

## Watcher

`…/scratchpad/wf-spikes/spike2/wf_watch.py`. The version below is what ran, plus one
additive `--script` flag added afterwards (for runs launched with `{scriptPath}`, which
persist no `*<runId>*.js`) and re-validated by replaying the completed run through it.
The as-run copy is kept at `wf_watch.as-run.py`.

Usage:

```
python3 wf_watch.py --config <CLAUDE_CONFIG_DIR> [--run <runId>] [--script <file.js>] \
                    --interval 1.0 --poll 0.05 --events events.jsonl --duration 400
```

```python
#!/usr/bin/env python3
"""wf_watch.py - live run tree for a Claude Code Workflow run, built from files only.

Given a CLAUDE_CONFIG_DIR, discovers
    <config>/projects/<slug>/<sessionId>/subagents/workflows/<runId>/
and tails journal.jsonl + agent-*.jsonl inside it, printing one table per second:

    label | model | last tool | tokens (in/out/cacheR/cacheW) | phase | state

Sources per column
  label      persisted script <session>/workflows/scripts/*<runId>*.js  (label: '...' opts,
             matched to an agent by its first prompt line); falls back to the first
             prompt line from agent-<id>.jsonl
  model      message.model of the newest assistant record
  last tool  name of the newest content[].type == "tool_use" block
  tokens     summed over unique assistant message.id (usage.input_tokens,
             output_tokens, cache_read_input_tokens, cache_creation_input_tokens)
  phase      <session>/workflows/<runId>.json workflowProgress[].phaseTitle when that
             file exists, else derived from the script's phase('...') calls / phase: opts
  state      journal.jsonl: "started" record -> running, "result" record -> done

It also records, for every append it sees, the gap between the file's mtime and the
moment the watcher noticed - that is the detection latency measurement.
"""

import argparse
import json
import os
import re
import sys
import time
from pathlib import Path

# ---------------------------------------------------------------- discovery


def find_run_dirs(config: Path):
    """All subagents/workflows/<runId> dirs under a config dir, newest first."""
    out = []
    root = config / "projects"
    if not root.is_dir():
        return out
    for slug in root.iterdir():
        if not slug.is_dir():
            continue
        for sess in slug.iterdir():
            wfd = sess / "subagents" / "workflows"
            if not wfd.is_dir():
                continue
            for run in wfd.iterdir():
                if run.is_dir():
                    out.append(run)
    out.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return out


# ------------------------------------------------------------ script parsing

PHASE_CALL = re.compile(r"""\bphase\(\s*['"`]([^'"`]+)['"`]\s*\)""")
AGENT_CALL = re.compile(r"""\bagent\(\s*(['"`])""")
OPT_LABEL = re.compile(r"""\blabel\s*:\s*['"`]([^'"`]*)['"`]""")
OPT_PHASE = re.compile(r"""\bphase\s*:\s*['"`]([^'"`]*)['"`]""")
OPT_MODEL = re.compile(r"""\bmodel\s*:\s*['"`]([^'"`]*)['"`]""")


def _read_js_string(text, i):
    """Read the string literal starting at text[i] (its quote char). Returns (value, end)."""
    quote = text[i]
    i += 1
    buf = []
    while i < len(text):
        c = text[i]
        if c == "\\":
            buf.append(text[i : i + 2])
            i += 2
            continue
        if c == quote:
            return "".join(buf), i + 1
        buf.append(c)
        i += 1
    return "".join(buf), i


def parse_script(text):
    """Source-order list of {label, phase, prompt, prefix} for each agent() call.

    prefix is the prompt's literal head, cut at the first ${...} so a template
    literal still matches the concrete prompt written to the transcript.
    """
    entries = []
    cur_phase = None
    # phase() calls and agent() calls, interleaved in source order
    marks = []
    for m in PHASE_CALL.finditer(text):
        marks.append((m.start(), "phase", m.group(1)))
    for m in AGENT_CALL.finditer(text):
        marks.append((m.start(), "agent", m))
    marks.sort(key=lambda t: t[0])

    for pos, kind, payload in marks:
        if kind == "phase":
            cur_phase = payload
            continue
        qpos = payload.end() - 1  # index of the opening quote
        prompt, end = _read_js_string(text, qpos)
        tail = text[end : end + 400]  # the options object, if any
        label = OPT_LABEL.search(tail)
        ph = OPT_PHASE.search(tail)
        mdl = OPT_MODEL.search(tail)
        prefix = prompt.split("${")[0].strip()
        entries.append(
            {
                "label": label.group(1) if label else None,
                "phase": ph.group(1) if ph else cur_phase,
                "model": mdl.group(1) if mdl else None,
                "prompt": prompt,
                "prefix": prefix,
            }
        )
    return entries


# ----------------------------------------------------------- transcript state


class AgentState:
    def __init__(self, agent_id):
        self.agent_id = agent_id
        self.prompt = None
        self.model = None
        self.last_tool = None
        self.tool_calls = 0
        self.usage = {}  # message.id -> usage dict (last write wins)
        self.state = "?"
        self.result = None
        self.agent_type = None
        self.first_seen = None  # wall clock, watcher
        self.last_record_ts = None  # timestamp field of newest record
        self.records = 0

    def tokens(self):
        i = o = cr = cw = 0
        for u in self.usage.values():
            i += u.get("input_tokens", 0) or 0
            o += u.get("output_tokens", 0) or 0
            cr += u.get("cache_read_input_tokens", 0) or 0
            cw += u.get("cache_creation_input_tokens", 0) or 0
        return i, o, cr, cw


class RunWatcher:
    def __init__(self, run_dir: Path, events_path: Path, quiet=False, script_override=None):
        self.run_dir = run_dir
        self.run_id = run_dir.name
        self.session_dir = run_dir.parent.parent.parent  # .../<sessionId>
        self.wf_dir = self.session_dir / "workflows"
        self.summary_path = self.wf_dir / f"{self.run_id}.json"
        self.scripts_dir = self.wf_dir / "scripts"
        self.agents = {}  # agentId -> AgentState
        self.order = []  # agentIds in journal / discovery order
        self.offsets = {}  # path -> byte offset consumed
        self.mtimes = {}  # path -> last mtime seen
        self.script_entries = []
        self.script_path = None
        # Runs launched with {scriptPath} persist no *<runId>*.js next to the run,
        # so allow the caller to point at the script explicitly.
        self.script_override = Path(script_override) if script_override else None
        self.summary = None
        self.summary_first_seen = None
        self.events = open(events_path, "a", buffering=1)
        self.quiet = quiet
        self.seen_paths = set()

    # ---- events -------------------------------------------------------
    def emit(self, kind, path, extra=None):
        now = time.time()
        try:
            st = os.stat(path)
            mtime, size = st.st_mtime, st.st_size
        except OSError:
            mtime, size = None, None
        rec = {
            "detected_at": round(now, 4),
            "kind": kind,
            "file": str(path),
            "mtime": round(mtime, 4) if mtime else None,
            "size": size,
            "lag_ms": round((now - mtime) * 1000, 1) if mtime else None,
        }
        if extra:
            rec.update(extra)
        self.events.write(json.dumps(rec) + "\n")

    # ---- incremental jsonl read ---------------------------------------
    def read_new(self, path: Path):
        """Yield newly appended parsed JSON records; emits an event on append."""
        try:
            st = path.stat()
        except OSError:
            return
        if self.mtimes.get(path) == (st.st_mtime, st.st_size):
            return
        first = path not in self.seen_paths
        self.seen_paths.add(path)
        self.mtimes[path] = (st.st_mtime, st.st_size)
        off = self.offsets.get(path, 0)
        if st.st_size < off:  # truncated/rewritten
            off = 0
        if st.st_size == off and not first:
            return
        with open(path, "rb") as fh:
            fh.seek(off)
            blob = fh.read()
        # keep only whole lines
        cut = blob.rfind(b"\n")
        if cut == -1:
            return
        self.offsets[path] = off + cut + 1
        self.emit("create" if first else "append", path, {"bytes": cut + 1})
        for line in blob[: cut + 1].splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except Exception:
                continue

    # ---- pollers ------------------------------------------------------
    def poll(self):
        self.poll_script()
        self.poll_summary()
        self.poll_journal()
        self.poll_agents()

    def poll_script(self):
        if self.script_entries:
            return
        cands = []
        if self.script_override and self.script_override.exists():
            cands.append(self.script_override)
        if self.scripts_dir.is_dir():
            cands.extend(sorted(self.scripts_dir.glob(f"*{self.run_id}*.js")))
        for p in cands:
            try:
                text = p.read_text()
            except OSError:
                continue
            self.script_entries = parse_script(text)
            self.script_path = p
            self.emit("script", p, {"agent_calls": len(self.script_entries)})
            break

    def poll_summary(self):
        if not self.summary_path.exists():
            return
        try:
            st = self.summary_path.stat()
        except OSError:
            return
        key = (st.st_mtime, st.st_size)
        if self.mtimes.get(self.summary_path) == key:
            return
        first = self.summary_path not in self.seen_paths
        self.seen_paths.add(self.summary_path)
        self.mtimes[self.summary_path] = key
        try:
            self.summary = json.loads(self.summary_path.read_text())
        except Exception:
            return
        if self.summary_first_seen is None:
            self.summary_first_seen = time.time()
        self.emit(
            "summary" if not first else "summary-create",
            self.summary_path,
            {"status": self.summary.get("status"), "agents": self.summary.get("agentCount")},
        )

    def agent(self, aid):
        st = self.agents.get(aid)
        if st is None:
            st = self.agents[aid] = AgentState(aid)
            st.first_seen = time.time()
            self.order.append(aid)
        return st

    def poll_journal(self):
        for rec in self.read_new(self.run_dir / "journal.jsonl"):
            aid = rec.get("agentId")
            if not aid:
                continue
            st = self.agent(aid)
            t = rec.get("type")
            if t == "started":
                st.state = "running"
            elif t == "result":
                st.state = "done"
                st.result = rec.get("result")
            else:
                st.state = t or st.state

    def poll_agents(self):
        for p in sorted(self.run_dir.glob("agent-*.jsonl")):
            aid = p.name[len("agent-") : -len(".jsonl")]
            st = self.agent(aid)
            meta = self.run_dir / f"agent-{aid}.meta.json"
            if st.agent_type is None and meta.exists():
                try:
                    st.agent_type = json.loads(meta.read_text()).get("agentType")
                except Exception:
                    pass
            for rec in self.read_new(p):
                st.records += 1
                if rec.get("timestamp"):
                    st.last_record_ts = rec["timestamp"]
                msg = rec.get("message") or {}
                if rec.get("type") == "user" and st.prompt is None:
                    c = msg.get("content")
                    if isinstance(c, str):
                        st.prompt = c
                    elif isinstance(c, list):
                        for b in c:
                            if b.get("type") == "text":
                                st.prompt = b.get("text")
                                break
                if rec.get("type") == "assistant":
                    if msg.get("model"):
                        st.model = msg["model"]
                    if msg.get("usage") and msg.get("id"):
                        st.usage[msg["id"]] = msg["usage"]
                    for b in msg.get("content") or []:
                        if isinstance(b, dict) and b.get("type") == "tool_use":
                            st.last_tool = b.get("name")
                            st.tool_calls += 1
                if st.state == "?":
                    st.state = "running"

    # ---- labelling ----------------------------------------------------
    def label_for(self, st, idx):
        # 1. summary file (authoritative once written)
        if self.summary:
            for wp in self.summary.get("workflowProgress") or []:
                if wp.get("agentId") == st.agent_id and wp.get("label"):
                    return wp["label"], "summary"
        # 2. persisted script, matched by prompt prefix
        if self.script_entries and st.prompt:
            for e in self.script_entries:
                if e["prefix"] and st.prompt.startswith(e["prefix"]) and e["label"]:
                    return e["label"], "script"
        # 3. script by position
        if idx < len(self.script_entries) and self.script_entries[idx]["label"]:
            return self.script_entries[idx]["label"], "script-pos"
        # 4. first prompt line
        if st.prompt:
            for ln in st.prompt.splitlines():
                if ln.strip():
                    return ln.strip()[:38], "prompt"
        if st.prompt is not None:
            return "(empty prompt)", "prompt"
        return st.agent_id[:10], "id"

    def phase_for(self, st, idx):
        if self.summary:
            for wp in self.summary.get("workflowProgress") or []:
                if wp.get("agentId") == st.agent_id and wp.get("phaseTitle"):
                    return wp["phaseTitle"]
        if self.script_entries and st.prompt:
            for e in self.script_entries:
                if e["prefix"] and st.prompt.startswith(e["prefix"]) and e["phase"]:
                    return e["phase"]
        if idx < len(self.script_entries):
            return self.script_entries[idx]["phase"] or "-"
        return "-"

    # ---- rendering ----------------------------------------------------
    def render(self, t0, sessions_line):
        lines = []
        el = time.time() - t0
        lines.append(
            f"[{el:6.1f}s @{time.time():.3f}] run={self.run_id}  dir={self.run_dir}  "
            f"agents={len(self.agents)}  summary={'yes' if self.summary else 'NO'}"
            + (f" (status={self.summary.get('status')})" if self.summary else "")
        )
        lines.append(
            f"{'label':<16}{'src':<10}{'model':<30}{'lastTool':<12}"
            f"{'in':>8}{'out':>8}{'cacheR':>11}{'cacheW':>10}  {'phase':<12}{'state':<9}result"
        )
        lines.append("-" * 136)
        for idx, aid in enumerate(self.order):
            st = self.agents[aid]
            label, src = self.label_for(st, idx)
            i, o, cr, cw = st.tokens()
            res = st.result if isinstance(st.result, str) else (
                json.dumps(st.result) if st.result is not None else ""
            )
            res = res[:20].replace("\n", " ")
            lines.append(
                f"{label[:15]:<16}{src:<10}{(st.model or '-')[:29]:<30}"
                f"{(st.last_tool or '-')[:11]:<12}"
                f"{i:>8}{o:>8}{cr:>11}{cw:>10}  {self.phase_for(st, idx)[:11]:<12}"
                f"{st.state:<9}{res}"
            )
        if not self.order:
            lines.append("(no agents yet)")
        lines.append(f"sessions/: {sessions_line}")
        return "\n".join(lines)


def sessions_snapshot(config: Path):
    d = config / "sessions"
    if not d.is_dir():
        return "no sessions dir"
    js = sorted(d.glob("*.json"))
    if not js:
        keys = len(list(d.glob("*.key")))
        return f"0 *.json ({keys} *.key)"
    out = []
    for p in js:
        try:
            r = json.loads(p.read_text())
        except Exception:
            continue
        out.append(f"{r.get('pid')}:{r.get('status')}:{r.get('kind')}")
    return " ".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True, help="CLAUDE_CONFIG_DIR to watch")
    ap.add_argument("--run", help="runId (default: newest run dir seen)")
    ap.add_argument("--interval", type=float, default=1.0, help="print period, seconds")
    ap.add_argument("--poll", type=float, default=0.05, help="stat poll period, seconds")
    ap.add_argument("--script", help="script file to read labels/phases from when the "
                                     "run persisted none (scriptPath-launched runs)")
    ap.add_argument("--events", default="wf-events.jsonl")
    ap.add_argument("--duration", type=float, default=600.0)
    a = ap.parse_args()

    config = Path(a.config)
    t0 = time.time()
    watcher = None
    last_print = 0.0
    print(f"# watching {config} for workflow runs (poll={a.poll}s print={a.interval}s)", flush=True)

    while time.time() - t0 < a.duration:
        if watcher is None:
            runs = find_run_dirs(config)
            if a.run:
                runs = [r for r in runs if r.name == a.run]
            if runs:
                watcher = RunWatcher(runs[0], Path(a.events), script_override=a.script)
                watcher.emit("run-dir-discovered", runs[0])
                print(f"# discovered run dir at +{time.time()-t0:.3f}s: {runs[0]}", flush=True)
        if watcher is not None:
            watcher.poll()
        now = time.time()
        if now - last_print >= a.interval:
            last_print = now
            sl = sessions_snapshot(config)
            if watcher is None:
                print(f"[{now-t0:6.1f}s] waiting for a run dir...  sessions/: {sl}", flush=True)
            else:
                print(watcher.render(t0, sl), flush=True)
        time.sleep(a.poll)
    print("# watcher duration reached, exiting", flush=True)


if __name__ == "__main__":
    main()
```

### Sample live output (from the real run, t+9.2 s — summary file did not exist yet)

```
[   9.2s @1787949437.515] run=wf_e23f71b6-543  dir=…/subagents/workflows/wf_e23f71b6-543  agents=3  summary=NO
label           src       model                         lastTool          in     out     cacheR    cacheW  phase       state    result
----------------------------------------------------------------------------------------------------------------------------------------
alpha           script    claude-opus-5                 -                  2       4          0      8431  Fanout      done     OK
bravo           script    -                             -                  0       0          0         0  Fanout      running
charlie         script    -                             -                  0       0          0         0  Fanout      running
sessions/: 960428:None:interactive
```

`src=script` proves labels and phases resolved from the persisted script alone, with no
summary file present. `sessions/: 960428:None:interactive` is the registry line —
`None` is the missing `status` key.

Tool extraction (`lastTool`) reported `-` here because trivial agents call no tools; it was
verified separately by replaying a real tool-using workflow's `agent-*.jsonl` through the same
parser, which printed `Bash` and `StructuredOutput` correctly.

## Known limits of this watcher

- Picks the single newest run dir; concurrent workflow runs in one config dir are not
  multiplexed.
- Label/phase from the script are matched to an agent by prompt-prefix, then by position.
  Identical prompts across agents would collapse onto the first matching label
  (the spike's prompts were tagged `[alpha]`/`[bravo]`/`[charlie]` to avoid that).
- A workflow launched with `{scriptPath}` persists no `*<runId>*.js`; without `--script`
  the label falls back to the agent's first prompt line. Observed on `wf_e8705bda-7f0`.
- Token columns sum `usage` per unique assistant `message.id`; that does not exactly equal
  the `tokens` figure the summary file reports: the summary said 8434 for each agent, the
  transcript `usage` sums to 8437 for each (2 in + 4 out + cacheR + cacheW). Which of the two
  is canonical, and what the 3-token gap is, is **UNVERIFIED**.
- 50 ms polling on a run dir is cheap here (13 files); cost at hundreds of agents is
  **UNVERIFIED**.

## Cost of the run

From the `--output-format json` result: `total_cost_usd = 0.21942475`.

| model | in | out | cache read | cache write | USD |
|---|---|---|---|---|---|
| claude-opus-5 | 12 | 420 | 56414 | 23107 | 0.21818 |
| claude-haiku-4-5-20251001 | 1179 | 13 | 0 | 0 | 0.00124 |

The three subagents ran on `claude-opus-5` (inherited from `--model opus`), confirmed both in
`agent-*.jsonl` `message.model` and in `workflowProgress[].model`; `defaultModel` in the
summary is `claude-opus-5`. The haiku line is auxiliary (not a workflow agent).
Workflow-reported totals: `totalTokens` 25302, `totalToolCalls` 0, `agentCount` 3.
