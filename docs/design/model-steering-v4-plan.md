# Model steering and v4 roles — findings and the evaluation plan

Status: Phase A (evidence, incl. the effort study) complete 2026-08-28; Phase B/C (measured baseline, v4 authoring, re-run) awaiting the go. Sources: [`research/model-steering-profiles-opus.md`](research/model-steering-profiles-opus.md) and [`research/model-steering-profiles-codex.md`](research/model-steering-profiles-codex.md) (two independent researchers, vendor documentation first, 37+ primary URLs), [`research/model-lower-tiers-profiles.md`](research/model-lower-tiers-profiles.md) (cheaper tiers and subscription-bucket economics), [`research/model-evidence-internal.md`](research/model-evidence-internal.md) (our own 25-PR ledger mined per model family, plus the taureval judge check), [`research/effort-ladder-opus.md`](research/effort-ladder-opus.md) and [`research/effort-ladder-codex.md`](research/effort-ladder-codex.md) (reasoning-effort semantics per model, two independent researchers). Team decision 2026-08-28: Claude runs Fable 5 and Opus 5 only (no Sonnet); Codex runs gpt-5.6-sol, with gpt-5.6-luna only as a small-task lane (terra is not used). Everything marked *inferred* below is the researchers' synthesis, not a vendor statement.

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
| **gpt-5.6 sol / luna** (Codex) | Goal / Context / Constraints / **Done When**; the two-sided permission matrix; tool-routing rules; required output shape; concrete writing choices instead of tone adjectives | half the role: repeated rules, process/style text that changes nothing, inert examples, repo conventions (→ `AGENTS.md`), blanket brevity (5.6 is already terser) | keep the v3 baseline then try one level lower; ladder low=renames, medium=features/bugfixes, high=multi-file, xhigh=long agentic/architecture. luna deltas are *inferred*: give it a narrow job and a pinned output shape, never below `high` |
| **gemini-3.7-flash / 3.1-pro** (Antigravity) | direct instructions *after* the data; flat paragraphs, light markup; explicit communication guidance (terse by default); the explore → plan → execute phases with the Implementation Plan as the review gate; named verification commands | chain-of-thought scaffolding, heavy nesting, "ask before X" clauses (approvals belong in `toolPermission`), any sampling parameters | `medium` is Google's recommendation for agentic coding; `high` for review/product judgment; flash-medium/low quality is unpublished — measure before unattended use |
| **grok-4.6 / 4.5** | evidence-grounding + anti-sycophancy clauses; a context-budget clause (price doubles above 200k prompt tokens); plan mode as the approval gate; `AGENTS.md`-native; specify output shape (no vendor style guidance exists) | stacked verification, "load everything" instructions | `high` default; `xhigh` buys ~1 point for much more latency; 4.5 silently maps xhigh→high |

## Which model for which task class — hypotheses to test

Evidence: vendor guidance + our ledger. Ledger signals (25 PRs): Opus reviewers found the operational hazards and the abstraction leaks (PR 15 blocker, 16b's concurrency class), Codex reviewers found config-file and upgrade edge classes and verified claims relentlessly (19: 60+ doc findings, three wrong counts); Opus implementers overbuilt when the spec invited it and fixed instances rather than classes across rounds; Codex implementers followed procedural specs literally and over-ran wall-clock budgets; Fable terminated loops and wrote the specs — but its same-family approvals on PRs 2–3 missed majors Codex then found.

| Task class | Hypothesis (frontier) | Cheaper alternative worth testing | Bucket note |
|---|---|---|---|
| Coordination / lead | Fable 5 @ high (most dependable at dispatching and sustaining subagents; Opus 5 over-delegates) | Opus 5 when the Fable bucket is exhausted | Fable is the only true bucket switch on Claude |
| Multi-file implementation, refactors | Opus 5 @ high (UI/design) · gpt-5.6-sol @ medium→high (backend) | grok-4.6 @ high for throughput lanes | Codex: one bucket; effort, not tier, is the lever now that terra is out |
| Architecture decisions | Fable 5 @ high; second opinion gpt-5.6-sol @ xhigh | none | — |
| Adversarial / cross-family review | gpt-5.6-sol @ high (never the implementer's family) | — | not Grok (sycophancy trend) |
| Broad code review | Opus 5 @ medium, "report everything" | — | — |
| Product / design / UI judgment | gemini-3.7-flash @ high via Antigravity (plan/walkthrough artifacts, browser verification) · Opus 5 | — | Antigravity: Gemini bucket separate from the Claude+GPT bucket |
| Docs drift, claim verification, mechanical sweeps | gpt-5.6-luna @ high (small, narrow jobs) · gemini-3.7-flash @ low→medium | — | luna ≈ 25× Sol's messages |
| Research digests, log analysis, PR triage | gemini-3.7-flash @ high · luna @ high | — | — |
| Security audit | Opus 5 / gpt-5.6-sol | none (no cheap tier) | Claude cyber classifiers may refuse; Codex as the alternative |
| Long unattended runs | Fable 5 @ high (published multiday guardrails) · grok-4.6 @ high | — | — |

Dominated or excluded, no role: `gpt-5.5`, `gpt-5.6-terra` (team decision: sits between sol and luna without a clear job), `gemini-3.6-flash-*`, `gemini-3.5-flash-*`, Sonnet (team decision).

## Judges (taureval)

Updated 2026-08-28 (taureval master `1dbd0b1`): the Anthropic judges are `opus` and `fable` at `medium`, the OpenAI judges `gpt-5.6-sol` at `medium` and `gpt-5.6-luna` at `high` — Sonnet 4.5 and terra are gone, per the team decision above. The same-family exclusion stays; `claude --model fable -p` was verified to answer before the switch.

## Effort is a per-task variable, not a default

Two independent studies agree on the shape (details and the disagreements in the effort reports):

| Complexity | Fable 5 | Opus 5 | gpt-5.6-sol | gpt-5.6-luna | gemini-3.7-flash | grok-4.6 |
|---|---|---|---|---|---|---|
| Mechanical sweeps, lookups | low | low | low | low | low | low |
| Docs verification, checklist review of a diff | medium | low→medium (accuracy holds low) | low→medium | low→medium | low→medium | medium |
| Implementation from a complete written spec | high | high | **medium** | high | **medium** | high |
| Ambiguous debugging, architecture decisions | xhigh | high→xhigh | high→xhigh | switch to sol | high | high→xhigh |
| Long-horizon autonomous runs (> 30 min) | xhigh | xhigh | xhigh | — | — | high |
| Coordination / lead | high (one study says xhigh) | high + delegation cap | high→xhigh | — | — | high |

Rules that follow from the vendors: the scale is calibrated per model, so "everyone at high" is four unrelated policies (vendor defaults: Anthropic high, OpenAI medium, Google medium, xAI high); Anthropic's own Opus 5 curve puts `medium` ~2 points below `high` at about half the cost and shows a *medium-first, retry-at-high* policy beating high-only; `max` regressed on their chart — never a default; effort is set at launch per member and held (mid-session changes invalidate caching); as effort goes **up** the role gets shorter and outcome-shaped, as it goes **down** the role gains an explicit checklist; delete self-verification instructions from Opus 5 roles at `high` and above; grok's `xhigh` buys ~1 point for +20 % cost while `medium→high` buys ~3 points. Only an eval can settle: the luna cliff, Gemini flash at medium on our tasks, and whether Claude implementation from a written spec really needs `high`.

## Phase B — measured baseline (done 2026-08-29)

Results and rationale themes: [`research/phase-b-baseline.md`](research/phase-b-baseline.md). Headline: `no_fake_features` 61/61 everywhere; the dominant `correct_action` failure is "planned instead of built" in every family (partly an eval-design artefact — no fixture workspace — partly the v3 escalation framing); copy failures are process jargon lifted from the role text; **medium beats or ties high** for developer roles on judgment-style tasks (Opus 0.75 vs 0.63, sol 0.90 vs 0.86, grok 1.00 vs 0.71, flash 0.75 vs 0.78). Effort ladder for developer roles: medium by default, high only with a stated reason.

## Phase C — v4 and the comparison

Status 2026-08-29: the four v4 developer roles are authored (`src-tauri/resources/templates/roles/v4-developer-{claude,codex,agy,grok}.yaml`, Codex-reviewed for contradictions and permission-matrix completeness); the taureval judge rubric v2 (decision scored, quoted copy only) and the `fable` judge (after the bucket reset) precede the re-run.

For each B cell, author the v4 variant by the rules above (Fable writes; Codex reviews the spec for contradictions and permission-matrix completeness — the reviewer lens the vendors themselves recommend), re-run the same cases, report v3→v4 deltas per cell and per criterion. Then lead/architect cells on Fable 5 / gpt-5.6-sol @ xhigh, and one cheaper-tier probe per task class where the table names one. Winners feed the role defaults, presets and `docs/architecture/harness-model.md`; the `fable` alias is in the model catalog (PR #49) so judgment roles can declare it; `opus` stays the catalog default a member falls back to.
