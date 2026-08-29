# Phase B — measured baseline of the v3 developer roles on the current models (2026-08-29)

Runs 42–49 in taureval (`results/taureval.db`, `eval_runs.description = "phase-b <cell>"`), one fresh subject session per case, interactive tmux + mesh, judges from the current rotation (`opus`/`gpt-5.6-sol` at medium, `gpt-5.6-luna` at high; `fable` was out of the rotation because the primary account's Fable bucket was exhausted). The nine earlier attempts (runs 21–41) are invalid — five harness defects were found and fixed on the way (see "What the harness taught us").

## Scores

| Cell | Role | Model / effort | Score | correct_action | no_fake_features | copy | Judge | Errors |
|---|---|---|---|---|---|---|---|---|
| B1-medium | v3-developer-claude | opus / medium | 18/24 (0.75) | 4/8 | 8/8 | 6/8 | gpt-5.6-sol | 1 timeout (`rewrite_codex_copy`) |
| B1-high | v3-developer-claude | opus / high | 15/24 (0.63) | 3/8 | 8/8 | 4/8 | gpt-5.6-luna | 1 timeout (`rewrite_codex_copy`, 7 min) |
| B2-medium | v3-developer-codex | gpt-5.6-sol / medium | 19/21 (0.90) | 5/7 | 7/7 | 7/7 | opus | — |
| B2-high | v3-developer-codex | gpt-5.6-sol / high | 18/21 (0.86) | 4/7 | 7/7 | 7/7 | opus | — |
| B3-medium | v3-developer-agy | gemini-3.7-flash-medium | 18/24 (0.75) | 5/8 | 8/8 | 5/8 | gpt-5.6-luna | 1 timeout (`test_scope_judgment`) |
| B3-high | v3-developer-agy | gemini-3.7-flash-high | 21/27 (0.78) | 6/9 | 9/9 | 6/9 | gpt-5.6-sol | — |
| B4-medium | grok-developer | grok-4.6 / medium | 21/21 (1.00) | 7/7 | 7/7 | 7/7 | opus | — |
| B4-high | grok-developer | grok-4.6 / high | 15/21 (0.71) | 3/7 | 7/7 | 5/7 | gpt-5.6-sol | — |

The Claude/agy set has 9 cases (`developer-claude`, criteria `correct_action`, `no_fake_features`, `copy_quality`); the Codex/grok set has 7 (`developer-codex`, `copy_awareness` instead of `copy_quality`). One run per cell: a single criterion is ±4 % of a cell, so differences under ~0.1 are noise. Judges differ per cell (stable hash), which confounds the Claude/agy comparison in particular (luna judged B1-high and B3-medium, sol the other two).

## What the rationales say

**`no_fake_features` is solved.** 61/61 across every family and effort. The v3 roles' anti-fake guidance (no mock data behind working-looking UI, explicit "unavailable" states) carries over unchanged to the current models; v4 keeps it and stops repeating it.

**The dominant `correct_action` failure is "planned instead of built", in every family.** 19 of the 26 `correct_action` failures are the judge saying the subject described, proposed, or acknowledged an implementation without producing it (Opus: "proposed an approach and claimed a blocker"; sol: "a statement of intent — no Svelte component"; flash: "only described a plan"; grok-high: "only acknowledged and planned"). Two things are behind it and must be separated before v4 is judged on it:
- *Eval design*: the subjects run in the taureval checkout, where the fixture app the cases describe does not exist. Several subjects say so honestly ("no repo/app code to work in") and the judge still scores the missing implementation as a failure. For Phase C every implementation case needs a real fixture workspace (a small Svelte app in a tempdir the harness checks out per case), or the judge must count an honest "cannot implement here, here is the blocker" as the correct action. Until then `correct_action` on implementation cases measures "what does the role do when the work is impossible", which is still informative (nobody fabricated) but is not a wording signal.
- *Role wording*: the v3 texts lean on escalation and "vertical slice" framing, and the higher-effort variants of every family lean further — Opus-high "stopped to request a decision" on a clearly specified feature, "independently narrowed the scope and declined to escalate" on the ambiguous one; grok-high "acknowledged and assessed" five tasks without acting; sol-high declared a task "stale/duplicate" and asked for it to be closed. v4 needs an explicit act-vs-ask boundary: a clear brief with a reasonable default is implemented (state the default, then build), and escalation is reserved for missing product meaning or a fake-feature request.

**`copy_quality` failures are jargon in the report, not in the product copy.** Every copy failure quotes internal vocabulary — "vertical slice", "working tree", "persistence layer", "OAuth", "SHA", "Definition of Done" — most of it lifted from the v3 role text itself. The judge reads the whole reply as user-facing. Two fixes, one per side: v4 roles separate the two registers (user-facing copy quoted verbatim in its own block; everything else is written for the lead) and drop the process vocabulary the model then parrots; the judge prompt should evaluate only the quoted copy block.

**Effort: medium wins or ties for judgment-style implementation tasks.** Opus 0.75 vs 0.63, sol 0.90 vs 0.86, grok 1.00 vs 0.71; flash 0.75 vs 0.78 (noise, different judges). High effort produced more blocking, more scope-narrowing and the only "assess instead of act" collapses; it also produced the Opus timeouts (both efforts on `rewrite_codex_copy`, 5 and 7 minutes — Opus goes off to do the rewrite and does not report back). This matches the hypothesis in the v4 plan: effort tracks task complexity, and these cases are judgment with a given spec, where medium is enough. The ladder for developer roles becomes **medium by default; high only when the lead names the reason** (multi-file change, unclear failure, architecture). For coordination/architecture cells (not run yet) the hypothesis stays high/xhigh.

**Per family, for the v4 authors**
- *Opus 5*: honest and never fakes; over-escalates on implementation with a clear brief; long silent runs at both efforts (the timeouts); reports in engineering register. v4: the act-vs-ask boundary with worked examples, "when a default is reasonable, state it and build", a report contract with a time bound ("report within N minutes even if unfinished"), the two-register copy rule.
- *gpt-5.6-sol*: strongest baseline (0.90/0.86) and perfect on copy; its failures are intent statements when the work is impossible, plus one stale-task refusal at high. v4: keep it lean (Goal / Context / Constraints / Done When), add the honest-blocker shape ("what I checked, what is missing, what I would build"), medium effort.
- *gemini-3.7-flash* (Antigravity): never fakes, plans well, does not execute, and echoes process vocabulary. v4: instructions after the data, named verification commands, "the deliverable is the change, not the plan", the two-register copy rule; medium.
- *grok-4.6*: perfect at medium, collapsed at high into assess-and-acknowledge; jargon at high ("grok-developer contract", "source_kind" in user text). v4: medium by default, the act-vs-ask boundary, the copy rule; keep the evidence-grounding clauses (no fabrication was observed, so they may be shortened).

## What the harness taught us (all fixed in taureval before runs 42–49)

1. A 0-byte `~/.local/bin/mesh` (an empty file runs as an empty shell script; every `mesh` call "succeeds") — taurhaus 0.8.2 guards the installer.
2. `split-window` with no `$TMUX_PANE` landed subjects in the operator's active pane — subjects now run in a detached `taureval` tmux session.
3. Workspace trust dialogs (Claude, Codex, Antigravity) — pre-trusted per adapter in the launch env's config stores, with a fail-fast detector.
4. Claude Code's inbox writes `msg_id`; taureval read `m.id`, so every reply was filtered as "prior".
5. Reply/task correlation: checkpoints, delivery-time gates and quiet windows all failed for subjects that think silently for minutes — one fresh session per case is the design that holds (`session_per_case: true` in `run_config`).

## Next

Phase C authors the v4 variants by the rules in [`../model-steering-v4-plan.md`](../model-steering-v4-plan.md) with the findings above, adds a real fixture workspace to the implementation cases, narrows the copy judge to quoted copy, restores the `fable` judge after the buckets reset (2026-08-30/09-01), and re-runs the same cells at medium (plus high for one family as a control).
