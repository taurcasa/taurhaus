# Git Worktree Viability For AI-Driven Multi-Agent Development

Date: 2026-03-08
Owner: architect
Task: #706

## Executive Summary

Recommendation: **defer per-agent git worktrees as the default execution model** for taurhaus's current multi-agent loop, and pursue **lighter shared-worktree coordination fixes first**.

Reason:
- the shared-worktree friction is real and recurring
- but the highest-value conflicts in our current setup are not all repo-file conflicts
- per-task worktree lifecycle cost is non-trivial for 10-30 minute agent tasks
- worktree isolation does not isolate `~/.claude/teams/...` runtime/config/compaction state, which is a large part of Taurhaus's actual coordination surface
- adopting worktrees would trade one coordination problem for another: frequent merge/integration gates that agents are worse at handling than humans

Short version:
- **Do not adopt default per-agent worktrees now.**
- **Use targeted process/tooling alternatives first.**
- Revisit worktrees only for a narrower mode: long-lived, high-overlap, mostly-independent implementation streams.

## What We Know From Our Own Team History

The friction is real.

Evidence from prior retros:
- [retro-quality-sprint-2026-03-05.md](/home/user/projects/taurhaus/docs/archive/retro-quality-sprint-2026-03-05.md:43) recorded shared-worktree contention as a universal pain point during a 14-task sprint.
- The same retro logged `3/4` agents independently recommending worktree isolation, but still concluded `D5: Worktree isolation (DECIDED — not pursuing)` because task cycles were too short and merge/setup overhead was likely to exceed the saved contention time ([retro-quality-sprint-2026-03-05.md](/home/user/projects/taurhaus/docs/archive/retro-quality-sprint-2026-03-05.md:96), [retro-quality-sprint-2026-03-05.md](/home/user/projects/taurhaus/docs/archive/retro-quality-sprint-2026-03-05.md:132)).
- The more recent consolidated retro still lists `worktree overlap / concurrent file ownership ambiguity` as one of the three strongest common friction points across respondents ([retro-2026-03-08-survey-findings.md](/home/user/projects/taurhaus/docs/retro/retro-2026-03-08-survey-findings.md:17), [retro-2026-03-08-survey-findings.md](/home/user/projects/taurhaus/docs/retro/retro-2026-03-08-survey-findings.md:80)).

What we do **not** have yet:
- a direct metric for "unexpected file changes caused a stop" as a percentage of total tasks
- a per-file hotspot frequency table over completed tasks

So the honest answer to question 1 is:
- this is clearly a recurring friction source
- it is not yet measured precisely enough to justify a heavy execution-model shift on frequency data alone

## Local Benchmark: Actual Worktree Overhead In This Repo

I measured the basic lifecycle in this repository.

Observed locally on 2026-03-08:
- `git worktree add --detach <temp>`: `0.374s`
- fresh worktree has **no** `node_modules`
- fresh worktree has **no** `src-tauri/target`
- `bun install --frozen-lockfile` in the fresh worktree: `3.059s`
- a fresh `cargo check --tests` in the worktree ran long enough to become material and had already created a new `src-tauri/target` before I stopped the benchmark

Interpretation:
- the mechanical worktree create/remove step is cheap
- the real cost is environment/bootstrap duplication
- each linked worktree arrives without the local untracked assets our agents rely on for fast iteration
- unless we redesign shared build-output strategy, per-agent worktrees imply repeated cold or semi-cold install/build overhead

For a human working on a branch all day, this is acceptable.
For an agent finishing tasks in 10-30 minutes, it is not negligible.

## Git Worktree Semantics That Matter Here

Official Git docs confirm that linked worktrees share repository metadata and refs, but each linked worktree has its own working tree and its own private worktree metadata under `$GIT_DIR/worktrees/...`.

Important implications from the official `git-worktree` manual:
- worktrees are meant to let you check out more than one branch at a time
- they are removed with `git worktree remove`
- linked worktree administrative files live in `$GIT_DIR/worktrees/...`
- refs are largely shared across worktrees
- Git explicitly notes that multiple checkout support is still "experimental" in some areas and submodule support remains incomplete

Source:
- Git official manual: https://git-scm.com/docs/git-worktree.html

This matters because worktrees solve only **repo working tree isolation**. They do not magically solve broader process/runtime integration issues.

## Claude Code Worktree Support: What Is Publicly Clear

Two things are clearly documented publicly by Anthropic:
- Claude Code supports subagents with separate context windows
- Claude Code documentation explicitly recommends using `git worktree` for parallel Claude Code sessions when you need to work on multiple branches simultaneously, and reminds you to initialize each worktree separately

Source:
- Anthropic common workflows: https://docs.anthropic.com/en/docs/claude-code/common-workflows
- Anthropic subagents: https://docs.anthropic.com/en/docs/claude-code/sub-agents

Important nuance:
- I did **not** find a stable public Anthropic doc page that formally documents an Agent-tool field like `isolation: "worktree"` as an external integration contract.
- So for Taurhaus planning, the safer assumption is: Claude Code is compatible with manual worktree-based parallel sessions, but any built-in agent-isolation API surface should be treated as preview/internal unless we verify a public stable contract.

Implication for Taurhaus:
- yes, Claude Code can operate in worktrees
- no, that does not by itself prove Taurhaus should make per-agent worktrees its default orchestration model

## Question 1: Actual Conflict Frequency

Current answer: **real but not quantified enough**.

What we can support with evidence:
- the friction is universal enough to show up repeatedly in retros
- the strongest overlap pain is concentrated in hotspot files and shared validation surfaces, not every task
- the team already reduced a large part of the pain by changing verification policy from `just check` to `just check-quick` ([retro-quality-sprint-2026-03-05.md](/home/user/projects/taurhaus/docs/archive/retro-quality-sprint-2026-03-05.md:43))

What we cannot honestly claim yet:
- exact stop rate caused by unrelated file edits
- exact minutes lost specifically to "unexpected repo changes" independent of quality-gate contention

Assessment:
- this is not a 50% catastrophe that obviously forces architectural isolation now
- it is also not a 5% non-issue
- best description today: **meaningful recurring friction, but still mixed with policy/ownership/validation friction that worktrees would not fully remove**

## Question 2: Merge Overhead For Our Task Size

For our task shape, merge overhead is the biggest reason to defer.

Typical agent task in taurhaus:
- 10-50 file changes
- 10-30 minute duration
- frequent dependence on work completed just minutes earlier
- high chance that completion requires re-running targeted tests against the freshest tree

With worktrees, every task would add some combination of:
1. create worktree
2. initialize environment in that worktree
3. make changes
4. commit in that isolated branch/worktree
5. merge or cherry-pick back into the integration branch
6. resolve conflicts or rebase when another agent landed first
7. run validation again in the target integration tree
8. remove worktree

The mechanical Git steps are not the expensive part.
The expensive parts are:
- duplicated environment bootstrap
- repeated integration/merge decisions
- repeated validation against the shared destination branch
- conflict handling by agents, which is exactly where AI agents are less trustworthy than humans

For this team, the current unblock pattern is often:
- lead tells agent to proceed or ignore unrelated changes
- agent continues in seconds

That is much cheaper than:
- merge branch A from worktree 1
- rebase/merge branch B from worktree 2
- revalidate after integration
- explain failures caused by merge order instead of shared live tree order

## Question 3: Daemon / Watcher Architecture

Taurhaus can technically watch multiple roots, but the architecture is not naturally optimized around spinning up one isolated app/runtime per agent worktree.

Relevant current behavior:
- native project watchers are managed by the app in [watchers.rs](/home/user/projects/taurhaus/src-tauri/src/startup/watchers.rs:28)
- watchers are activity-based and keyed to discovered project paths
- coordination/runtime state is managed separately under the Claude teams directory, not inside the repo
- team discovery and resume are built around shared `~/.claude/teams/...` state ([ARCHITECTURE.md](/home/user/projects/taurhaus/ARCHITECTURE.md:165), [docs/features/mesh.md](/home/user/projects/taurhaus/docs/features/mesh.md:45))

What multiple worktrees would imply:
- each worktree path is a distinct project root from the watcher's point of view
- if multiple worktrees are all considered active, watcher count and reconcile complexity go up
- if each agent also needs an isolated Taurhaus data root and isolated Claude root, we are no longer talking about just git worktrees; we are talking about near-full per-agent runtime isolation

Conclusion:
- the current daemon/watcher architecture could be extended to tolerate multiple worktree roots
- but it does not make worktrees operationally free
- default per-agent worktrees would spill into watcher planning, project discovery, and possibly project identity semantics

## Question 4: Shared State Outside The Repo

This is the strongest argument against treating git worktrees as the main fix.

Taurhaus multi-agent coordination relies heavily on shared filesystem state outside the repository:
- `~/.claude/teams/<team>/config.json`
- `~/.claude/teams/<team>/runtime/<member>.json`
- `~/.claude/teams/<team>/state/activity/<member>.json`
- `~/.claude/teams/<team>/state/compaction/<member>.json`
- mesh inboxes and runtime state under the same team root

Relevant references:
- [ARCHITECTURE.md](/home/user/projects/taurhaus/ARCHITECTURE.md:165)
- [coordination-architecture.md](/home/user/projects/taurhaus/docs/coordination-architecture.md:24)
- [compaction-reinjection-investigation-2026-03-08.md](/home/user/projects/taurhaus/docs/analysis/compaction-reinjection-investigation-2026-03-08.md:15)

Meaning:
- worktrees isolate tracked source files
- they do **not** isolate team runtime state, mesh state, daemon pidfiles, compaction state, or operational snapshots
- several of our most difficult bugs have been exactly in those shared state layers, not in repo-file collisions

So even if worktrees removed every repo-file overlap, they would only solve **one slice** of multi-agent coordination risk.

## Question 5: Better Alternatives

Yes. Several lighter alternatives fit our actual failure modes better.

### Alternative A: stronger ownership metadata in assignment messages

This is already supported by retro findings and current process direction.

Add to every assignment footer:
- owned files or owned area
- adjacent-file fix allowance: yes/no
- validation expectation
- if overlap is expected, state the teammate and intended sequencing explicitly

Why it helps:
- directly addresses the stop-vs-proceed ambiguity
- cheaper than worktree setup
- works for both shared-worktree and any future isolated mode

### Alternative B: completion-triggered "safe to proceed" signals

When task X completes, team-lead or tooling should emit a short structured note to affected assignees:
- task complete
- changed files
- safe to proceed / please rebase / please re-open specific file

This is basically codifying the unblock message that already works.

Why it helps:
- removes the need for agents to guess whether unexpected changes are hostile or expected
- preserves the low-latency advantage of the current model

### Alternative C: hotspot file serialization

Use narrow serialization only for repeated conflict magnets:
- shared controllers
- shared integration tests
- high-churn runtime files

This can be done as:
- temporary file claims in task assignments
- lead-side sequencing for hotspot tasks
- lightweight lock metadata if needed later

Why it helps:
- isolates the real conflict hotspots without isolating the whole repo

### Alternative D: per-task commit cadence on shared mainline

This was already adopted conceptually in prior retro decisions.

Why it helps:
- narrows drift windows
- makes unrelated changes attributable quickly
- gives other agents a fresh baseline without branch-merging complexity

### Alternative E: targeted worktree mode, not default mode

If we want to experiment, do it only when all of these are true:
- task expected duration is long enough to amortize setup cost
- work is mostly independent of near-term teammate edits
- task touches large hotspot surfaces where shared edits are likely
- merge owner is explicit

This would make worktrees an exception tool, not the standard operating model.

## Question 6: Would Claude Code Worktree Isolation Integrate Cleanly With Mesh/Taurhaus?

Not cleanly enough to justify default adoption yet.

If we assume Claude-side agent isolation can use worktrees, Taurhaus still has to answer:
- which project path becomes the canonical member `projectPath`
- whether multiple worktrees of the same repo should appear as separate projects or one logical project
- how watcher reconciliation treats those roots
- whether the lead sees one team working on one project or many parallel shadow projects
- how cross-tool teams behave when Codex/Gemini members do not share the same Claude-native isolation model

This is the real integration problem:
- Claude-only worktree isolation does not automatically generalize to cross-tool team orchestration
- Taurhaus is not orchestrating only Claude subagents; it is orchestrating Claude, Codex, Gemini, mesh daemons, tmux panes, and shared runtime state

So even if Claude's own lifecycle handles worktree setup nicely, Taurhaus would still need its own policy for:
- path identity
- merge-back timing
- runtime metadata ownership
- watcher and project listing semantics

## Cost / Benefit Summary

### Benefits if adopted

- fewer direct file-edit collisions inside the tracked repo
- fewer compile/typecheck surprises caused by another agent's uncommitted edits
- more local autonomy during longer independent implementation tasks
- cleaner per-agent diffs before integration

### Costs if adopted as default

- per-task setup/bootstrap overhead
- separate `node_modules` and Rust `target` per worktree unless we add more build-sharing machinery
- more branch/merge/cherry-pick operations per session
- higher conflict-resolution burden at integration time
- no isolation for shared `~/.claude/teams/...` state
- more complicated project/watch semantics for Taurhaus itself
- more complicated support story across Claude, Codex, and Gemini together

### Net for our current setup

For today's Taurhaus operating model, the costs outweigh the benefits.

## Final Recommendation

Recommendation: **defer default per-agent git worktrees**.

More precise position:
- do **not** switch the standard multi-agent loop to one-worktree-per-agent
- do **not** invest now in worktree orchestration inside mesh/Taurhaus core
- do implement lighter overlap-reduction measures first
- optionally pilot a **targeted worktree mode** later for long-lived/high-overlap tasks only

## Concrete Next Proposals

### Proposal 1: add ownership footer enforcement to assignments

Every assignment should include:
- owned files/area
- adjacent-file fix policy
- expected overlap, if any
- validation depth

### Proposal 2: add a completion-aware unblock signal

When a teammate lands a task, automatically or procedurally send:
- changed files
- whether it is safe to proceed without action
- whether rebasing/re-reading is required

### Proposal 3: identify and serialize hotspot files

Start with files that repeatedly attract overlap:
- shared controllers
- shared tests
- coordination runtime/orchestrator hot paths

### Proposal 4: instrument the problem before changing the execution model

Before revisiting worktrees, measure for 1-2 sessions:
- number of tasks completed
- number of stalls caused by unexpected repo-file changes
- files involved in each stall
- unblock time when lead sends context
- whether the stall was really repo overlap vs. unrelated red validation

This will answer the missing frequency question honestly.

### Proposal 5: if we pilot worktrees, constrain the experiment hard

Pilot only when:
- at least 2 agents are expected to work independently for 30+ minutes
- merge owner is explicit
- no shared controller/runtime hotspot is in scope
- success metric is pre-declared: fewer stalls **without** increasing merge/validation churn

## Bottom Line

Git worktrees are a good tool for human parallel branch work and can be used manually with Claude Code.

They are **not** the best default answer for Taurhaus's current AI-driven multi-agent workflow.

Our actual pain is a mix of:
- shared repo overlap
- ownership ambiguity
- validation contention
- shared coordination/runtime state outside the repo

Worktrees only solve the first of those cleanly.

For the current team shape, the pragmatic path is:
- **defer default worktrees**
- **tighten ownership + unblock signaling + hotspot serialization first**
- **revisit targeted worktree mode only if measured overlap remains high after those fixes**
