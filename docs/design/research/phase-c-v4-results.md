# Phase C — v4 developer roles vs v3, like-for-like under judge rubric v2 (2026-08-29)

Phase C ran the four v4 developer roles (PR #62) through taureval with judge rubric v2 (decision scored, not code; quoted copy only), one fresh session per case: runs 50–54. To compare like for like, the eight Phase B runs (v3 roles, rubric v1) were re-scored under rubric v2 with `eval:rejudge` — same stored subject replies, new judge verdicts: runs 55–62. Same judge rotation, `fable` still out (bucket), one run per cell, so ±1 case (≈0.04–0.05) is noise, and the judge differs between some cells (sol vs luna).

## Scores under rubric v2

| Family / effort | v3 (re-judged) | v4 | Δ |
|---|---|---|---|
| Claude opus · medium | 22/24 (0.92) | 23/27 (0.85) | −0.07 (2 cases; the v3 run has one timeout excluded from its max) |
| Claude opus · high | 19/24 (0.79) | 25/27 (0.93) | **+0.14**, and no timeout (v3 timed out on `rewrite_codex_copy` at both efforts) |
| Codex gpt-5.6-sol · medium | 21/21 (1.00) | 16/21 (0.76); 16/18 (0.89) without the empty-reply artefact | −0.11 (one real miss, one artefact) |
| Antigravity flash · medium | 27/27 (1.00) | 27/27 (1.00) | 0 |
| Grok 4.6 · medium | 21/21 (1.00) | 20/21 (0.95) | −0.05 (1 case) |
| Codex · high (v3 only) | 18/21 (0.86) | — | |
| Antigravity · high (v3 only) | 27/27 (1.00) | — | |
| Grok · high (v3 only) | 18/21 (0.86) | — | |

## What this says

1. **Most of the Phase B "planned instead of built" signal was the rubric, not the roles.** Re-judged under v2, the v3 roles at medium score 0.92 / 1.00 / 1.00 / 1.00. The rubric-v1 failures were the judge grading the absence of code that could not exist; rubric v2 fixed that (`docs/design/research/phase-b-baseline.md` stands corrected on the size of the wording half).
2. **Effort: at medium, v3 and v4 are within noise for every family; the one clear v4 gain is Claude at high** (0.79 → 0.93, no timeouts). That is the report contract working: v4 tells Opus to send a progress note within ten minutes and to commit before the completion report; v3 let a high-effort Opus disappear into the rewrite. For developer roles the effort rule from Phase B stands — medium by default — with the note that v4 makes high *safe* for Claude, not necessary.
3. **The remaining v4 misses are the eval-design gap, not wording**: v4 Claude and Codex still occasionally block on the absent workspace (`implement_with_copy_ownership`, `scaffold_labeled_clearly`) although the rubric now forgives an honest blocker that keeps the scope; the honest fix is a real fixture workspace per case (queued). One Codex case (`ambiguous_scope`) was an empty inbox message accepted as the reply — harness follow-up recorded.
4. **What the eval cannot see**: v4's action matrix, the two-register copy rule, the labeled report shapes and the ten-minute progress note are operational contracts for a real team; these decision scenarios exercise only the build-or-escalate boundary and copy, where v3 was already good. Adopting v4 is justified by the operational lessons (Phase B timeouts, the 25-PR ledger) and the Claude-high result, not by a medium-effort score gain — the eval does not show one.

## Decision

- Adopt the v4 developer roles as the bundled defaults for new teams (presets updated), keep the v3 roles bundled for one more release for comparison, and say plainly in the harness-model doc that at medium the two are indistinguishable on the current cases.
- Before any further wording claim: (a) a fixture workspace per case; (b) n=3 runs per cell (the `--runs` default) and the `fable` judge back after the bucket reset; (c) cases that exercise the report contract (a long task where the subject must report progress; a completion report judged for the labeled shape).

## Harness changes on the way (taureval, master)
`eval:rejudge <run-id>` (re-score stored replies under the current rubric; new run row with `rejudged_from`), judge rubric v2 (`rubric_version` in `run_config`), one session per case, quiet-before-send and drain, per-adapter workspace pre-trust with a fail-fast startup detector, subjects in a detached `taureval` tmux session, msgV-1 inbox ids.
