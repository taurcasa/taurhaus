# Model steering and v4 roles — findings and the evaluation plan

Status: Phase A (evidence) complete 2026-08-28; Phase B/C (measured baseline, v4 authoring, re-run) awaiting the go. Sources: [`research/model-steering-profiles-opus.md`](research/model-steering-profiles-opus.md) and [`research/model-steering-profiles-codex.md`](research/model-steering-profiles-codex.md) (two independent researchers, vendor documentation first, 37+ primary URLs), [`research/model-lower-tiers-profiles.md`](research/model-lower-tiers-profiles.md) (cheaper tiers and subscription-bucket economics), [`research/model-evidence-internal.md`](research/model-evidence-internal.md) (our own 25-PR ledger mined per model family, plus the taureval judge check). Everything marked *inferred* below is the researchers' synthesis, not a vendor statement.

## Why the roles must change, not just their model ids

Every bundled Claude role pins `claude-opus-4-6` (one Sonnet 4.5), every Codex role `gpt-5.4`; only the Antigravity and Grok roles are current. More important than the ids: the v3 texts were written for those models' habits (taureval's own model notes describe Opus 4.6 and GPT 5.4), and **all four vendors' current guidance says the same thing — the step-by-step scaffolding that steered the previous generation now hurts**:

- Anthropic: prescriptive, SOP-style skills "can degrade" Fable 5; Opus 5 self-verifies by default, so "double-check" instructions cause over-verification, and "be conservative / only high-severity" in review prompts makes it report *less*.
- OpenAI: leaner system prompts measured +10–15 % on internal coding-agent evals at −41–66 % tokens; the dominant failure is **contradiction, not thinness**; collapse all permission language into one action matrix (both the safe-without-asking list and the confirm-required list) or the agent thrashes on approvals.
- Google: Gemini 3.x "may over-analyze verbose or overly complex prompt engineering techniques used for older models"; it is terse by default, so a role must *add* communication guidance, put instructions after the data, and name the literal verification commands.
- xAI: publishes no prompting guide at all for Grok 4.6; the model card self-reports higher sycophancy and dishonesty rates than 4.5 at high effort (low absolute numbers, wrong direction) — so Grok roles need explicit evidence-grounding and anti-sycophancy clauses, and Grok should not be the adversarial reviewer.

## v4 wording rules per family

Common to all: mission + domain context + hard constraints + one two-sided permission matrix + measurable "done when" criteria; each rule stated once; no repeated reminders; conventions live in the repo's `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`, not in the role.

| Family | Keep / add | Delete from v3 | Effort |
|---|---|---|---|
| **Fable 5** (`fable`) | outcome + *why this lane exists*; non-goals and an assess-vs-implement boundary (it takes useful-but-unrequested actions); progress honesty ("audit each claim against a tool result"); principle-based check-ins with an autonomy reminder; a final-copy contract (long runs end in dense arrow-chain summaries); use subagents freely | step lists, verification reminders, any "show/explain your reasoning" (triggers a reasoning-extraction refusal and a silent fallback), scope-expanding "improve as you go" | start at `high`; lower often beats the previous model's xhigh; xhigh for long unattended runs |
| **Opus 5** (`opus`) | the complete specification up front, then leave it to run; scope containment ("deliver what was asked, at the scope intended"); **three separate length rules** (conversational verbosity, agentic narration, deliverables on disk) — effort does not shorten output; a delegation-damping clause (it over-delegates); reviewers told "report everything, filter in a separate pass" | "double-check", verification reminders, "be conservative"/severity thresholds in reviewer roles, workarounds for older-model vision limits | implementers `high`; reviewers `medium` (review accuracy holds at low) |
| **gpt-5.6 sol / terra / luna** (Codex) | Goal / Context / Constraints / **Done When**; the two-sided permission matrix; tool-routing rules; required output shape; concrete writing choices instead of tone adjectives | half the role: repeated rules, process/style text that changes nothing, inert examples, repo conventions (→ `AGENTS.md`), blanket brevity (5.6 is already terser) | keep the v3 baseline then try one level lower; ladder low=renames, medium=features/bugfixes, high=multi-file, xhigh=long agentic/architecture. Terra/luna deltas are *inferred*: give terra more explicit routing, luna a narrow job and a pinned output shape, never below `high` for luna |
| **gemini-3.7-flash / 3.1-pro** (Antigravity) | direct instructions *after* the data; flat paragraphs, light markup; explicit communication guidance (terse by default); the explore → plan → execute phases with the Implementation Plan as the review gate; named verification commands | chain-of-thought scaffolding, heavy nesting, "ask before X" clauses (approvals belong in `toolPermission`), any sampling parameters | `medium` is Google's recommendation for agentic coding; `high` for review/product judgment; flash-medium/low quality is unpublished — measure before unattended use |
| **grok-4.6 / 4.5** | evidence-grounding + anti-sycophancy clauses; a context-budget clause (price doubles above 200k prompt tokens); plan mode as the approval gate; `AGENTS.md`-native; specify output shape (no vendor style guidance exists) | stacked verification, "load everything" instructions | `high` default; `xhigh` buys ~1 point for much more latency; 4.5 silently maps xhigh→high |
| **Sonnet** (`sonnet`) | same shape as Opus 5 with a fuller spec and tighter scope | — | `high`; note it is a slower drain of the *same* weekly bucket, not a separate pool |

## Which model for which task class — hypotheses to test

Evidence: vendor guidance + our ledger. Ledger signals (25 PRs): Opus reviewers found the operational hazards and the abstraction leaks (PR 15 blocker, 16b's concurrency class), Codex reviewers found config-file and upgrade edge classes and verified claims relentlessly (19: 60+ doc findings, three wrong counts); Opus implementers overbuilt when the spec invited it and fixed instances rather than classes across rounds; Codex implementers followed procedural specs literally and over-ran wall-clock budgets; Fable terminated loops and wrote the specs — but its same-family approvals on PRs 2–3 missed majors Codex then found.

| Task class | Hypothesis (frontier) | Cheaper alternative worth testing | Bucket note |
|---|---|---|---|
| Coordination / lead | Fable 5 @ high (most dependable at dispatching and sustaining subagents; Opus 5 over-delegates) | Opus 5 when the Fable bucket is exhausted | Fable is the only true bucket switch on Claude |
| Multi-file implementation, refactors | Opus 5 @ high (UI/design) · gpt-5.6-sol @ high (backend) | gpt-5.6-terra @ high (−1.2 SWE-Bench Pro, 2–2.5× the messages) · grok-4.6 @ high for throughput lanes | Codex: one bucket, drain rate differs per tier |
| Architecture decisions | Fable 5 @ high; second opinion gpt-5.6-sol @ xhigh | none | — |
| Adversarial / cross-family review | gpt-5.6-sol @ high (never the implementer's family) | terra @ high | not Grok (sycophancy trend) |
| Broad code review | Opus 5 @ medium, "report everything" | Sonnet @ high | — |
| Product / design / UI judgment | gemini-3.7-flash @ high via Antigravity (plan/walkthrough artifacts, browser verification) · Opus 5 | — | Antigravity: Gemini bucket separate from the Claude+GPT bucket |
| Docs drift, claim verification, mechanical sweeps | gpt-5.6-terra @ medium | gpt-5.6-luna @ high · gemini-3.7-flash @ medium | luna ≈ 25× Sol's messages |
| Research digests, log analysis, PR triage | gemini-3.7-flash @ high · luna @ high | — | — |
| Security audit | Opus 5 / gpt-5.6-sol | none (no cheap tier) | Claude cyber classifiers may refuse; Codex as the alternative |
| Long unattended runs | Fable 5 @ high (published multiday guardrails) · grok-4.6 @ high | — | — |

Dominated, no role: `gpt-5.5`, `gemini-3.6-flash-*`, `gemini-3.5-flash-*` (worse *and* not cheaper).

## Judges (taureval)

The two Anthropic judges are on `claude-opus-4-6` and `claude-sonnet-4-5` (active but stale; Sonnet 4.5 retires ~2026-09-29); the OpenAI judges are current (`gpt-5.6-sol`, `gpt-5.6-terra`). Before Phase B: `opus` and `sonnet` aliases for the Anthropic judges, keep the same-family exclusion, reconsider the hard-coded `effort: high` per judge. A third judge per family (`fable`, `luna`) is optional.

## Phase B — measured baseline (needs the go)

v3 roles **as written**, on the current model per family, dev split, one run per cell; the deliverable is the judge rationales per model, not the scores.

| Cell | Role (existing) | Adapter | Model / effort | Cases |
|---|---|---|---|---|
| B1 | `v3-developer-claude` | claude-code | `opus` / high | 9 (implementation/dev) |
| B2 | `v3-developer-codex` | codex-cli | `gpt-5.6-sol` / high | 7 |
| B3 | `v3-developer-agy` | agy-cli | `gemini-3.7-flash-high` / medium | 9 |
| B4 | `grok-developer` | grok-cli | `grok-4.6` / high | 7 |
| B5 (optional) | `v3-developer-codex` | codex-cli | `gpt-5.6-terra` / high | 7 |

≈ 32–39 interactive agent sessions plus judging. Fable cells (lead/architect) wait until the Fable buckets recover after Aug 30/Sep 1.

## Phase C — v4 and the comparison

For each B cell, author the v4 variant by the rules above (Fable writes; Codex reviews the spec for contradictions and permission-matrix completeness — the reviewer lens the vendors themselves recommend), re-run the same cases, report v3→v4 deltas per cell and per criterion. Then lead/architect cells on Fable 5 / gpt-5.6-sol @ xhigh, and one cheaper-tier probe per task class where the table names one. Winners feed the role defaults, presets and `docs/architecture/harness-model.md`; the `fable` alias is added to the model catalog first so judgment roles can declare it.
