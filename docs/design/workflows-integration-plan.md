# Workflows in taurhaus — integration plan

Status: approved 2026-08-28; execution W1 → W5. Builds on [`workflows-and-multi-model-orchestration.md`](workflows-and-multi-model-orchestration.md) (the who/how split: taurhaus orchestrates *who* over days, a workflow orchestrates *how* over minutes) and settles its preconditions with three spikes ([`research/workflow-wf-spike-1a.md`](research/workflow-wf-spike-1a.md), [`-1b`](research/workflow-wf-spike-1b.md), [`-2`](research/workflow-wf-spike-2.md), all on Claude Code 2.1.251):

- **Named workflows resolve from exactly two places**: `<CLAUDE_CONFIG_DIR>/workflows/<name>.js` (user scope) and `<project>/.claude/workflows/<name>.js` (project scope). Both `Workflow({name, args})` and the slash form `/name {…}` work; a named run is launched as a background task and its return value lands in the session's task output, the run tree under `<session>/subagents/workflows/<runId>/`.
- **A taurhaus-launched team member invokes a workflow when told to** — launched with the team flags, handed a mesh-style tmux notice (`ACTION REQUIRED: Invoke Workflow({name:"…", args:{…}})`), it ran the workflow first try, four times, with no opt-in and no permission prompt beyond the folder-trust dialog at launch. Design A's trigger works as designed.
- **The live run tree is reconstructible from files**: `journal.jsonl` (results), `agent-*.jsonl` (transcripts) and the persisted script (`workflows/scripts/<name>-<runId>.js`, present from the first frame, carries labels and phases) — 10–60 ms behind the writes with a 50 ms stat poll. The `<runId>.json` summary is written once, at the end. The sessions registry never marks a headless parent busy during a run, so activity for workflow runs must come from the transcripts.

## What a week of running Design B by hand taught us

The procedures (`implement-plan-pr.js`, `fix-round-pr.js`, `docs-sweep.js`, research sweeps) are the real asset and were unversioned, living in one session's directory; every PR's ledger row was filled by hand from journals; non-Claude stages ran as Opus "babysitter" agents polling detached `codex exec` runs — workable, but the source of the timeouts, three-turn resumes and lost work on a terminal crash; roles for workflow subagents and roles for mesh members were two different texts.

## Plan

| # | Deliverable | Size / lane |
|---|---|---|
| **W1** | **Versioned procedures.** `.claude/workflows/` in the repo: `feature-pr.js` (implement → two-lens cross-family review → fix loop → gate), `small-change.js` (one implementer, one lens, one fix round), `fix-round.js`, `research-sweep.js` (N independent researchers + synthesis contract), `docs-sweep.js`; each parametrised by `args` (spec path, branch/worktree, implementer family, size class, effort), each returning a ledger-shaped result; the sizing policy and commit discipline encoded; a README with the model split; CLAUDE.md "How changes are made" points at them. Also installable to user scope per account (the hook-installer precedent) so a lead in any project can run them. | small — Opus, one Codex lens |
| **W3** | **One role source.** Export role templates as Claude Code agent definitions (`<project>/.claude/agents/<role_id>.md`, frontmatter `model`/`effort`/`tools`, body = the role's steering text) from `TemplateStore` — generated, not hand-written; the workflows' `agentType` uses them; the same text drives mesh members, so taureval evaluates what runs in both. | small–medium — Opus, one Codex lens |
| **W2** | **W2a (backend) / W2b (UI): run scanner + canvas + ledger.** W2a adds the daemon/app scanner over each known session's `subagents/workflows/<runId>/` (journal + transcripts + persisted script), the completed-summary history API, transcript-write activity hint, and ledger-row export. W2b consumes that IPC for the live tree (phase → agent → label/model/last tool/tokens/state) under the member/session node, session list, and run-history view. Automatic plan editing remains out of scope; the export replaces hand transcription without mutating this ledger. | feature — Codex (backend) / Opus (UI), cross-family review |
| **W5** | **Task-level effort in the assignment contract.** Assignments carry `effort` + `why`; taurhaus refuses an assignment without them; applies them before delivery — `/effort` typed into the pane for harnesses with a runtime effort command (Claude Code, Grok; a registry capability), resume-with-flag for launch-only harnesses (Codex, Antigravity); shows the level on the task card and member node; logs it. The lead roles (v4) carry the proportionality paragraph and worked examples from Phase B; the lead's onboarding carries live inputs (members' remaining usage windows, load). | small–medium — Opus, one Codex lens; after Phase B |
| **W4** | **taurhaus-managed non-Claude stages.** A workflow stage that hands a task to a managed member (Codex/agy/grok in tmux + mesh) and awaits its inbox result — persistent session, observability, compaction reinjection, later steering — replacing detached-`codex exec` babysitters. Design first (delivery contract, completion signal, timeouts, worktree handling); Experiments 3–5 of the design note gate it. | feature — spec by the orchestrator, then Codex/Opus |

Order: W1 + W3 (parallel, small) → W2 → W5 (with the v4 lead roles) → W4.

## Ledger

| Item | Implementer | Reviewers | Rounds | Majors | Merged |
|---|---|---|---|---|---|
| W1 | Opus | Codex gpt-5.6 (one lens) | 5 | 4 → 5 → 3 → 2 → 2 (the round-2 spike was an installer a fix round added outside the spec; removed. Last two: gate opt-out fixed by the orchestrator, same-basename worktree collisions accepted and documented) | #51 |
| W3 | Opus | Codex gpt-5.6 (one lens) | 4 | 1 → 1 → 3 → 2 (last two: recipe quoting fixed by the orchestrator; the Windows two-rename fallback accepted as the best replacement where rename cannot replace) | #50 |
| W2 | tbd | tbd | tbd | tbd | tbd |
| W5 | tbd | tbd | tbd | tbd | tbd |
| W4 | tbd | tbd | tbd | tbd | tbd |
