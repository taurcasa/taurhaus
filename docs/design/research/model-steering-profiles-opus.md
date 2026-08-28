# Model Steering Profiles for Agent Role Descriptions

**Compiled:** 2026-08-28 · **Author:** Opus 5 research subagent · **Mode:** read-only research
**Scope:** how to write ROLE DESCRIPTIONS (persistent system-prompt-like text steering an interactive coding agent as a team member) for the eight models taurhaus runs.

## How to read this

- Every claim is either **[SOURCED]** to a vendor page with a URL, or explicitly marked **[INFERRED]** (my synthesis, not vendor text) or **[SECONDARY]** (non-vendor reporting).
- "v3 role" = a role description written for the predecessor generation (Opus 4.6/4.8, gpt-5.4, Gemini 3.x-earlier, Grok 4.5). "v4 role" = what to write now.
- Vendor coverage is uneven. Anthropic publishes per-model prompting pages. OpenAI publishes a model-family prompt-guidance page plus Codex harness docs. Google publishes a developer guide plus Antigravity CLI docs but **no model-level prompting guide for 3.7 Flash specifically**. xAI publishes **no prompt-engineering guide at all** for Grok 4.6 — the closest primary sources are the model card and the reasoning-parameter doc. Gaps are called out per section.

### The one-line summary per model

| Model | Steering posture in one line |
|---|---|
| Claude Fable 5 | Give it the *outcome and the reason*; delete your prescriptive scaffolding — it degrades output. |
| Claude Opus 5 | Give it the *full spec up front*; delete verification/double-check instructions; add length and scope brakes. |
| gpt-5.6-sol | Outcome + success criteria + stop conditions; lean prompt; never two rules that can conflict. |
| gpt-5.6-terra | Same contract as sol, slightly more explicit routing. |
| gpt-5.6-luna | Same contract, but narrow the job and pin the output shape. |
| gemini-3.7-flash | Short, direct, unadorned instructions; it is already terse; steer the *phase* (explore→plan→execute), not the prose. |
| gemini-3.1-pro | Same as 3.7 Flash, aimed at reasoning-hard, lower-volume work. |
| grok-4.6 | No vendor guide. Treat as an AGENTS.md-native agent; add explicit anti-sycophancy and evidence rules, which the model card's own regressions justify. |

---

# 1. Claude Fable 5 (`claude --model fable`, Claude Code 2.1.2xx)

## 1.1 What the vendor says about prompting it

**Instruction detail: goals over step lists, and *aggressively* so.** Anthropic's Fable 5 page is the strongest anti-prescription statement any vendor makes. Under "Recommended scaffolding changes" it says: *"Skills developed for prior models are often too prescriptive for Claude Fable 5 and can degrade output quality. Review and consider removing older instructions if default performance is better."* [SOURCED]

The Claude Code docs repeat it as a working rule for the model: **"Describe the outcome, not the steps"**, **"Hand it ambiguous problems"**, **"Skip the verification reminders"**, **"Size up larger tasks"**. [SOURCED]

**Long instruction sets: it holds them, but they cost you.** The vendor frames instruction-following as *improved enough to replace enumeration*: *"Instruction-following is improved enough that you can steer most behaviors with a brief instruction rather than enumerating each behavior by name."* A short brevity instruction is stated to be "as effective as listing each pattern." [SOURCED] The practical implication for a role description is that a 60-line behavioral checklist is not just wasteful, it is a *quality risk*.

**Over-constraint effects.** Two distinct hazards, both vendor-documented:
1. **Prescriptive skills degrade output** (quoted above). [SOURCED]
2. **A specific refusal trap:** instructions that tell the model to echo, transcribe or explain its internal reasoning can trigger the `reasoning_extraction` refusal category, causing elevated fallbacks to Opus 4.8. *"Audit existing skills and system prompts for reflection or show-your-thinking instructions when migrating."* [SOURCED] **This is the single highest-value thing to grep your existing roles for.** Any role saying "explain your reasoning", "show your thinking", "walk me through how you concluded that" is a live hazard on Fable 5.

**Tone and structure.** Anthropic's cross-model guidance still recommends XML tags for structural separation of instructions/context/examples, consistent descriptive tag names, and 3–5 examples wrapped in `<example>`/`<examples>` when you need format steering. It also recommends giving Claude a role in the system prompt — "even a single sentence makes a difference." [SOURCED] Note the tension: the general page endorses examples and structure; the Fable page warns against prescription. **[INFERRED] Resolution: use XML structure for *sections* of a role (identity, scope, boundaries, communication), but do not use it to enumerate behavioral micro-rules.**

**"Give the reason, not only the request."** Vendor-explicit and important for role descriptions: *"Claude Fable 5 tends to perform better when it understands the intent behind a request: context lets it connect the task to relevant information rather than inferring intent on its own."* The suggested shape is `I'm working on [the larger task] for [who it's for]. They need [what the output enables]. With that in mind: [request].` [SOURCED] **[INFERRED] For a team role this means the role should carry a "why this lane exists" paragraph, not just a "what you do" list.**

**Definition of done / acceptance criteria.** Anthropic does not tell you to drop DoD — it tells you to *stop reminding the model to verify*, which is a different thing. The vendor's own long-run pattern is: `Establish a method for checking your own work at an interval of [X] as you build. Run this every [X interval], verifying your work with subagents against the specification.` and it notes *"Separate, fresh-context verifier subagents tend to outperform self-critique."* [SOURCED] **[INFERRED] So: keep the acceptance criteria (what "done" means), drop the process reminders (how to check).**

## 1.2 Agentic behaviour

- **Autonomy is the headline.** Sustains "multiday, goal-directed runs with strong instruction retention." Individual requests run many minutes at higher effort; autonomous runs extend for hours. Anthropic explicitly advises restructuring harnesses to poll asynchronously rather than block. [SOURCED]
- **Check-ins.** The vendor's checkpoint instruction is short and general: *"Pause for the user only when the work genuinely requires them: a destructive or irreversible action, a real scope change, or input that only they can provide. If you hit one of these, ask and end the turn, rather than ending on a promise."* Note the explicit "no enumeration needed" framing around it. [SOURCED]
- **How it reacts to "ask before X".** Well, if X is a real category (destructive/irreversible/scope change/user-only input). But there is a documented counter-failure: *"Deep into a long session, Claude Fable 5 can occasionally … pause to ask permission when it already has enough to proceed."* For unattended pipelines Anthropic supplies an explicit autonomy reminder including *"asking 'Want me to…?' or 'Shall I…?' will block the work."* [SOURCED] **[INFERRED] For a taurhaus team member that runs in tmux without a human watching every turn, the autonomy reminder is more load-bearing than the ask-first rule.**
- **Jumping into implementation.** The opposite risk applies: it can *overplan* on ambiguous tasks. Vendor instruction: *"When you have enough information to act, act. Do not re-derive facts already established in the conversation, re-litigate a decision the user has already made, or narrate options you will not pursue…"* [SOURCED]
- **Unrequested actions.** Documented: "drafting an email when none was asked for, creating defensive git-branch backups." Vendor countermeasure: *"When the user is describing a problem, asking a question, or thinking out loud rather than requesting a change, the deliverable is your assessment. Report your findings and stop."* [SOURCED] **This is the single best sentence to steal for any reviewer/architect role.**
- **Delegation.** "Significantly more dependable at dispatching and sustaining parallel subagents"; vendor advises using subagents *frequently*, preferring async orchestrator↔subagent communication, and long-lived subagents that retain context. [SOURCED]
- **Tool-use discipline / progress honesty.** Vendor-tested instruction that *"nearly eliminated fabricated status reports even on tasks designed to elicit them"*: *"Before reporting progress, audit each claim against a tool result from this session. Only report work you can point to evidence for…"* [SOURCED]

## 1.3 Effort / thinking semantics

- **Thinking is always on and cannot be disabled.** Adaptive thinking is the only mode; `thinking: {"type": "disabled"}` is unsupported. In Claude Code, the session toggle, `alwaysThinkingEnabled` and `MAX_THINKING_TOKENS=0` all have no effect on Fable 5. Raw chain of thought is never returned (`summarized` or `omitted` only). [SOURCED]
- **Effort ladder in Claude Code:** `low, medium, high, xhigh, max`; default `high`. [SOURCED]
- **Vendor recommendation:** start at `high` for most tasks; `xhigh` for the most capability-sensitive workloads; `medium`/`low` for routine work. Key line: *"Lower effort settings on Claude Fable 5 still perform well and often exceed `xhigh` performance on prior models."* Reduce effort "if a task completes but takes longer than necessary, or if you want a quicker, more interactive working style." [SOURCED]
- **Cost:** $10/M input, $50/M output — exactly 2× Opus 5 on both meters. [SOURCED] Thinking tokens bill as output. [SOURCED]
- **When xhigh/max is worth it:** [INFERRED] genuinely ambiguous root-cause work, architecture decisions with many interacting constraints, and multi-hour autonomous runs. For a taurhaus *team member* role that runs interactively in a pane, `high` is the right default and `medium` is a real option, because the vendor says low-effort Fable beats xhigh on prior models.
- **Gotcha:** effort is calibrated *per model* — "the same level name does not represent the same underlying value across models." And changing effort mid-conversation invalidates prompt caching. [SOURCED]

## 1.4 Verbosity and user-facing copy quality

This is where Fable 5 needs the most role-level help, and the vendor gives you finished prose to paste.

- **Un-steered, it elaborates**: "surveying options it won't pursue, explaining root causes at length, producing heavily-structured PR descriptions, or writing comments that narrate what the next line does." [SOURCED]
- **Vendor brevity instruction** (short form): *"Lead with the outcome. Your first sentence after finishing should answer 'what happened' or 'what did you find'… Being readable and being concise are different things, and readability matters more. The way to keep output short is to be selective about what you include (drop details that don't change what the reader would do next), not to compress the writing into fragments, abbreviations, arrow chains like A → B → fails, or jargon."* [SOURCED]
- **Readability failure in long agentic sessions** is a named, distinct problem: "dense arrow-chain shorthand, deep implementation detail, references to thinking the user never saw, or overly technical phrasing." The vendor's mitigation paragraph is excellent and directly applicable to a team-member role whose messages land in someone else's terminal: *"If you've been working for a while without the user watching … your final message is their first look at any of it. Write it as a re-grounding, not a continuation of your working thread… The vocabulary you built up while working is yours, not theirs; leave it behind unless you re-introduce it."* [SOURCED]
- **[INFERRED] For taurhaus specifically:** a Fable 5 team member writing to `mesh` inbox or to the lead is exactly the "user wasn't watching" case. The re-grounding paragraph should be in every Fable role, near the end.

## 1.5 Known failure modes to guard against in a role

| Failure mode | Vendor status | Guard |
|---|---|---|
| Over-elaboration / gold-plating at high effort | Documented | Paste the vendor "don't add features, refactor, or introduce abstractions beyond what the task requires" paragraph |
| Fabricated progress claims on long runs | Documented | Paste the "audit each claim against a tool result" paragraph |
| Unrequested actions (defensive git branches, drafting artifacts nobody asked for) | Documented | Paste "the deliverable is your assessment. Report your findings and stop." |
| Early stopping: text-only "I'll now run X" with no tool call | Documented as rare | Add the autonomous-pipeline reminder ("check your last paragraph…") |
| Context-budget anxiety (offers to hand off / summarize) | Documented as rare, triggered by harness token countdowns | Don't surface remaining-context counts; else add "You have ample context remaining." |
| `reasoning_extraction` refusal → silent fallback to Opus 4.8 | Documented | **Never** put "show/explain your reasoning" in a Fable role |
| Cyber/bio safety-classifier refusals | Documented | Relevant to taurhaus: a *security-audit* role on Fable 5 will trigger fallback. Claude Code re-runs cyber-flagged Fable requests on Opus 4.8. [SOURCED] |
| Unreadable final summaries after long unattended work | Documented | Paste the re-grounding paragraph |

## 1.6 What a v4 Fable 5 role must change vs a v3 role written for Opus 4.6/4.8

1. **Delete prescriptive step lists.** Vendor says prescriptive skills "can degrade output quality." A v3 role that reads like an SOP is now actively harmful.
2. **Delete every verification reminder.** "Skip the verification reminders" is explicit Claude Code guidance. Replace with an interval-based verifier-subagent instruction *only* for genuinely long runs.
3. **Delete every "explain your reasoning" clause** — refusal hazard, not just noise. This is new and has no v3 analogue.
4. **Add a "why this lane exists" paragraph.** Vendor: give the reason, not only the request.
5. **Add the anti-gold-plating paragraph** — the overengineering risk moved from "Opus 4.5/4.6 tendency" to "higher-effort Fable behavior", and the vendor's Fable-specific wording is stronger (explicitly names feature flags, back-compat shims, hypothetical future requirements).
6. **Add progress-honesty grounding.** New for the long-horizon generation; a v3 role never needed it because v3 sessions didn't run for hours unattended.
7. **Rewrite the check-in rule from an enumeration to a principle.** v3 roles listed every "ask before" case; Fable needs the three-category principle plus an autonomy reminder for unattended runs.
8. **Re-baseline effort.** A v3 role that assumed `xhigh` for everything is now overspending: low/medium Fable ≥ xhigh on prior models, per vendor.
9. **Assume much longer turns.** Anything in a v3 role built around "respond quickly / check in often" fights the model.
10. **Give it harder work.** Vendor: "Pick a task harder than what you'd assign to prior models." A v3 role scoped to bite-sized tasks undersells the model.

## 1.7 Evidence

- Prompting Claude Fable 5 — https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5 (accessed 2026-08-28)
- Introducing Claude Fable 5 and Claude Mythos 5 — https://platform.claude.com/docs/en/models/fable-5/introducing-claude-fable-5-and-claude-mythos-5 (accessed 2026-08-28; availability 2026-06-09)
- Prompting best practices (all current Claude models) — https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices (accessed 2026-08-28)
- Effort — https://platform.claude.com/docs/en/build-with-claude/effort (accessed 2026-08-28)
- Claude Code model configuration ("Work with Fable 5", effort table, ultrathink/ultracode) — https://code.claude.com/docs/en/model-config (accessed 2026-08-28)
- Choosing a Claude model and effort level in Claude Code — https://claude.com/blog/claude-model-and-effort-level-in-claude-code (published 2026-07-07)
- Claude Fable 5 and Claude Mythos 5 (announcement) — https://www.anthropic.com/news/claude-fable-5-mythos-5 (accessed 2026-08-28)

---

# 2. Claude Opus 5 (`claude --model opus`, Claude Code 2.1.2xx)

## 2.1 What the vendor says about prompting it

**Instruction detail: complete spec up front, then get out of the way.** The most role-relevant sentence on the page: Opus 5 *"completes full tasks rather than leaving stubs or placeholders, and it performs best when given the complete task specification up front and left to run."* [SOURCED] That is *not* the same as Fable's "describe the outcome" — Opus 5 wants **specification completeness**, Fable wants **outcome + latitude**. [INFERRED] A good Opus 5 role is denser in *what the finished thing must be* and lighter in *how to get there*.

**Continuity:** *"It performs well out of the box on existing Claude Opus 4.8 prompts."* The Opus 5 page is explicitly a tuning list, not a rewrite mandate. [SOURCED]

**Over-constraint effects — three named ones, all costly:**
1. **Verification instructions cause over-verification.** *"If your prompt contains explicit verification instructions ('include a final verification step for any non-trivial task,' 'use a subagent to verify'), remove them: instructions like these cause over-verification on Claude Opus 5, and removing them reduces wasted tokens with no loss in quality. The same applies to legacy harness scaffolding that adds separate verification steps."* [SOURCED]
2. **Self-check instructions compound.** *"Avoid instructing re-checks it already performs ('double-check your answer,' 're-verify before responding')."* [SOURCED]
3. **Literal conservatism in review prompts.** *"If your review prompt says 'only report high-severity issues' or 'be conservative,' the model may follow that instruction literally and report less; ask it to report everything and filter in a separate pass instead."* [SOURCED] **This is a direct hit on how most reviewer roles are written.**

**Structure/tone.** Same cross-model guidance as Fable (XML tags, examples in `<example>` tags, one-sentence role framing, longform data first / query last). Opus-specific structural tip: in a long system prompt, pair a conciseness instruction with a **short reminder near the end** — the vendor literally shows `<tone_preference>Keep outputs reasonably concise.</tone_preference>` as a trailing echo. [SOURCED] **[INFERRED] For a long taurhaus role description, put the communication rule twice: once in the body, once as a short closing tag.**

**Examples vs rules.** Vendor-explicit for narration steering: *"Positive examples of the communication style you want tend to be more effective than instructions about what not to do."* [SOURCED]

**Definition of done.** Nothing special is required — the model finishes tasks rather than stubbing. [SOURCED] [INFERRED] Keep acceptance criteria as *outcome statements*, and do **not** append "then verify each criterion" — that is exactly the over-verification trigger.

## 2.2 Agentic behaviour

- **Scope expansion is the named risk.** *"Claude Opus 5 can also expand the scope of a task, adding steps that weren't requested or applying its own judgment about what the task should be."* Vendor countermeasure, quoted in full because it is the best scope paragraph any vendor publishes: *"Deliver what was asked, at the scope intended. Make routine judgment calls yourself, and check in only when different readings of the request would lead to materially different work. If the request seems mistaken or a better approach exists, say so in a sentence and continue with the task as asked rather than quietly narrowing, widening, or transforming it. Finish the whole task, and stop short of actions that are clearly beyond what was asked."* [SOURCED]
- **Autonomy vs check-ins.** The paragraph above *is* the check-in policy: routine judgment calls solo; check in only when readings diverge materially. [SOURCED] This is more permissive than a typical v3 "ask before X" list and more restrictive than Fable's fully-autonomous framing.
- **Delegation.** Delegates to subagents more readily than prior models; coordinates them well (effective writer-verifier patterns, few overwrite collisions). But *"it multiplies cost and time when applied to small tasks."* Vendor damping instruction: *"Delegate to a subagent only for large tasks that are genuinely independent and parallelizable… Do not delegate work you can finish yourself in a handful of tool calls, and do not use subagents to verify or double-check your own work."* [SOURCED]
  - Deterministic caps in Claude Code / Agent SDK: `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`, `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`, SDK `max_budget_usd`; require Claude Code **2.1.217+**. [SOURCED]
  - **Important harness detail:** Claude Code adds its own delegation instruction on Opus 5 *only* when using its `claude_code` system prompt preset; with a custom or omitted system prompt you must add one yourself. [SOURCED] **[INFERRED] taurhaus roles that replace the system prompt need their own delegation clause.**
- **Self-correction.** Catches and fixes its own mistakes well without prompting — but narrates corrections more than prior models. Vendor mitigation: *"Only correct an earlier statement when the error would change the user's code, conclusions, or decisions… For slips that change nothing for the user, make the fix and move on without noting it."* [SOURCED]
- **Tool-use discipline.** Effort is the lever: lower effort → fewer tool calls, combined operations, no preamble, terse confirmations; higher effort → more tool calls, plan-before-action, detailed summaries. [SOURCED]
- **Cross-model agentic guidance that still applies:** the reversibility/destructive-action confirmation block, the overeagerness/over-engineering block, the "don't hardcode to pass tests" block, and `<investigate_before_answering>` for hallucination control. [SOURCED]

## 2.3 Effort / thinking semantics

- **Thinking is ON by default** (breaking change from Opus 4.8, where it was off unless requested). Adaptive; effort is the depth control. [SOURCED]
- **Ladder:** `low, medium, high (default), xhigh, max`. [SOURCED]
- **Vendor recommendation:** *"Start with `high`, the default, and adjust based on your evals: step up to `xhigh` for demanding coding and agentic work, or to `max` when a task justifies unconstrained token spending, and use `low` and `medium` liberally as your primary control for token cost and response time wherever your evals show quality holds. If you carried effort settings over from an earlier model, run a fresh effort sweep."* [SOURCED]
- **"Effort matters more."** *"Claude Opus 5 converts additional effort into better results more reliably than any earlier Opus model, so the effort level you choose carries more weight."* [SOURCED]
- **Effort does NOT control visible response length.** *"Effort controls thinking volume, not visible response length: on Claude Opus 5, changing effort does not reliably shorten responses, so prompt for length instead."* [SOURCED] **This breaks a common v3 assumption.**
- **Code review holds accuracy at lower effort** — "which supports a fast pass at review time and a more thorough pass later." [SOURCED] **[INFERRED] Strong argument for running an Opus 5 reviewer role at `medium`.**
- **Constraint:** thinking cannot be disabled at `xhigh`/`max` (400 error). With thinking disabled, two artifacts appear: tool calls leaked as text, and internal XML tags in visible output. Vendor advice is not to disable it. [SOURCED]
- **Cost/latency:** $5/M in, $25/M out (unchanged from 4.8), but thinking-on-by-default means a workload that ran thinking-free on 4.8 now emits more output tokens at the same rate. 1M context (default and max), 128k max output, prompt-cache minimum lowered to 512 tokens. [SOURCED]
- **Claude Code extras:** `ultrathink` keyword requests deeper reasoning for one turn without changing session effort (other phrases like "think hard" are *not* recognized); `ultracode` sends `xhigh` plus dynamic workflow orchestration. Opus 5 has no model-default effort "hold" — a previously set level carries over. [SOURCED]

## 2.4 Verbosity and user-facing copy quality

- **Default responses run longer than prior Opus models.** [SOURCED] The general best-practices page calls Opus 5 out as *the exception* to the "latest Claude models are more concise" rule. [SOURCED]
- **Three separate length surfaces**, each needing its own instruction — this is the key structural insight:
  1. **Conversational verbosity** → vendor snippet: *"Keep responses focused, brief, and concise. Keep disclaimers and caveats short, and spend most of the response on the main answer. When asked to explain something, give a high-level summary unless an in-depth explanation is specifically requested."* [SOURCED]
  2. **Agentic narration** → vendor snippet: *"Before your first tool call, say in one sentence what you're about to do. While working, give a brief update only when you find something important or change direction. When you finish, lead with the outcome…"* [SOURCED]
  3. **Written deliverables on disk** (reports, Markdown docs) → *"Match the length of written documents to what the task needs: cover the substance, but do not pad with filler sections, redundant summaries, or boilerplate."* [SOURCED]
- **[INFERRED] taurhaus relevance:** roles that produce docs (`docs/`, CHANGELOG entries, design briefs) need surface #3 explicitly. A single "be concise" line will not reach it.

## 2.5 Known failure modes to guard against in a role

| Failure mode | Vendor status | Guard |
|---|---|---|
| Scope expansion / transforming the task | Documented | The "Deliver what was asked, at the scope intended" paragraph |
| Over-verification (caused by *your* instructions) | Documented | Delete verification/double-check clauses from the role |
| Under-reporting in review (caused by "be conservative") | Documented | Say "report everything"; filter in a separate pass |
| Excessive subagent spawning on small work | Documented | Delegation clause + env caps (needs Claude Code 2.1.217+) |
| Correction narration noise | Documented | The "only correct when it changes the user's decisions" clause |
| Long default responses and long written docs | Documented | Three separate length instructions |
| Leaked tool calls / internal XML tags | Documented, only with thinking disabled | Don't disable thinking; never write "do not think/reason" in a role |
| Biology-flagged requests refuse with **no fallback** on Opus 5 | Documented | Not a role-writing issue, but a routing one for security/bio-adjacent work |

## 2.6 What a v4 Opus 5 role must change vs a v3 role written for Opus 4.6/4.8

1. **Delete verification and double-check instructions.** In v3 these were best practice; in v4 they are a documented cost with no quality gain.
2. **Delete "be conservative / only flag high-severity" from reviewer roles.** This is a behavior *regression trigger* now — the model complies literally and reports less.
3. **Add an explicit scope-containment paragraph.** New risk class: Opus 5 expands scope and applies its own judgment about what the task should be.
4. **Add three length instructions, not one**, and stop expecting effort to shorten output (it doesn't on Opus 5).
5. **Add a narration-cadence instruction** with a *positive example* of the desired style, since agentic narration went up.
6. **Add a delegation clause** if the harness supplies a custom system prompt — Claude Code only injects one under its own preset.
7. **Re-baseline effort from scratch.** v3 guidance for Opus 4.7/4.8 was "start at xhigh for coding"; Opus 5 guidance is "start at high, and use low/medium liberally." A v3 role/config carrying `xhigh` everywhere is now the wrong default.
8. **Assume thinking is on.** Any v3 scaffolding built around thinking-off behavior (parsing `content[0].text`, disabling thinking at high effort for speed) is broken or capped.
9. **Give it the whole spec.** v3 roles that deliberately fed work in slices to avoid overreach now underperform — Opus 5 "performs best when given the complete task specification up front and left to run."
10. **Drop vision workarounds.** Vendor: "Re-validate any prompt-side vision workarounds you tuned for prior models; they may no longer be needed."

## 2.7 Evidence

- Prompting Claude Opus 5 — https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5 (accessed 2026-08-28)
- What's new in Claude Opus 5 — https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5 (accessed 2026-08-28)
- Prompting best practices — https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices (accessed 2026-08-28)
- Effort — https://platform.claude.com/docs/en/build-with-claude/effort (accessed 2026-08-28)
- Claude Code model configuration — https://code.claude.com/docs/en/model-config (accessed 2026-08-28)
- Choosing a Claude model and effort level in Claude Code — https://claude.com/blog/claude-model-and-effort-level-in-claude-code (2026-07-07)

---

# 3. OpenAI GPT-5.6 family via Codex CLI 0.149 — `sol` / `terra` / `luna`

OpenAI publishes prompting guidance at the **family** level (`prompt-guidance-gpt-5p6`), plus harness-level guidance in the Codex docs. There is no per-variant prompting page for sol/terra/luna; the variants differ in capability tier and cost, and the vendor's own framing is *"use gpt-5.6-sol for flagship capability, gpt-5.6-terra for a balance of intelligence and cost, or gpt-5.6-luna for efficient, high-volume workloads"* with `gpt-5.6` aliasing to sol. [SOURCED] I give one shared profile plus per-variant deltas (marked INFERRED where the vendor is silent).

## 3.1 What the vendor says about prompting it

**The headline is "outcome-first" and "lean".** This is the single biggest doctrinal shift of any vendor this generation.

- *"Describe the destination rather than prescribing every step."* [SOURCED]
- *"Configurations with leaner system prompts improved evaluation scores by roughly 10–15% while reducing total tokens by 41–66%"* (OpenAI internal evals on coding agents). [SOURCED]
- **What to REMOVE:** repeated statements of the same rule; style/process instructions that don't change behavior; examples that don't alter outcomes; unrelated tools and their descriptions. [SOURCED]
- **What to KEEP:** user-visible outcomes; success criteria and stopping conditions; safety/business/evidence/permission constraints; tool-routing rules that depend on context; required output shape and validation requirements. [SOURCED]

**Over-constraint: contradictions are worse than gaps.** The load-bearing warning: *"GPT-5-class models follow prompt contracts closely, so conflicting rules can create more instability than missing detail."* [SOURCED] **[INFERRED] For a taurhaus role this means the danger is not a thin role — it's a role that says "be thorough" in one section and "be brief" in another, or "ask before editing" next to "bias to action". Audit roles for pairwise conflicts, not for coverage.**

**State each rule once.** Vendor-explicit, tied to approval loops: stating an authorization rule once "prevents unnecessary approval loops for expected safe actions." [SOURCED]

**Structure preference.** OpenAI's general prompt-engineering guide recommends **Markdown headers and lists for hierarchy plus XML tags to delimit content boundaries** (reference material, supporting docs), and describes the instruction hierarchy developer > user > assistant. [SOURCED] Notably, for *reasoning* models it advises *"only high-level guidance"* rather than step-by-step — the inverse of classic GPT prompting. [SOURCED]

**Examples vs rules.** The GPT-5.6 guidance is anti-example unless the example changes behavior ("examples that don't alter outcomes" is on the remove list). [SOURCED] The Codex end-user prompting page is example-heavy but that is *user-facing* prompting, not system-prompt authoring. [SOURCED]

**Definition of done — this is where OpenAI is most explicit and most useful.** The Codex best-practices page defines a four-element prompt: **Goal / Context / Constraints / Done When**, where "Done When" is completion criteria — "tests passing, behavior changed, bugs resolved." [SOURCED] The consumer prompting page uses a parallel **Goal / Context / Output / Boundaries** framing and says *"Use only the parts that help. You don't need to fill in every item"* and *"the one or two boundaries that matter most. You don't need to control every step."* [SOURCED]

**AGENTS.md is the right home for durable rules — not the role/prompt.** *"An open-format README for agents"*, loaded automatically. *"A short, accurate AGENTS.md is more useful than a long file full of vague rules."* Hierarchical (global `~/.codex`, repo root, per-directory), merged root-to-leaf, later directories override earlier ones, each injected as a separate user-role message headed `# AGENTS.md instructions for <directory>`. [SOURCED] A named failure mode is *"embedding durable rules in prompts instead of AGENTS.md."* [SOURCED]

**[INFERRED] Direct taurhaus consequence:** for Codex team members, the role description should be thin and *task-shaped* (goal, done-when, boundaries), while repo conventions live in `AGENTS.md`. Duplicating repo conventions into the role is exactly the redundancy OpenAI measured as a 10–15% eval loss.

## 3.2 Agentic behaviour

- **Autonomy is defined by an explicit permission contract, not a vibe.** Vendor pattern: name the **safe local actions** the request authorizes (reading files, inspecting logs, editing in-scope code, running tests) to be performed *without asking*; and name what **requires confirmation** (external writes, destructive actions, purchases, scope expansion). [SOURCED] **This is the cleanest "ask before X" schema of any vendor — it works because it also enumerates the "don't ask" side, which is what prevents approval-loop thrash.**
- **Stop conditions are first-class.** Vendor: *"Resolve the request in the fewest useful tool loops, but do not let loop minimization outrank correctness."* After each result, assess whether the core request can be answered with sufficient evidence. [SOURCED]
- **Tendency to jump into implementation:** the Codex harness prompt itself biases to action — the Codex prompting guide describes the agent as an *"autonomous senior engineer"* that should *"Persist until the task is fully handled end-to-end"* and *"Bias to action: default to implementing with reasonable assumptions."* Clarifying questions only when "truly blocked." Every rollout should *"conclude with a concrete edit or an explicit blocker plus a targeted question"* — plans do not substitute for delivery. [SOURCED — note: this guide targets `gpt-5.3-codex`, the closest published Codex harness guidance; see §3.7 caveat]
- **Planning:** `/plan` (or Shift+Tab) gathers context and asks clarifying questions before implementing; the "interview pattern" has Codex question assumptions and convert fuzzy ideas into a spec. Plan-tool hygiene: skip planning for the ~25% easiest tasks, no single-step plans, never end with `in_progress` items. [SOURCED]
- **Tool-use discipline:** expose only task-relevant tools; document what each does, when to use it, key return fields, and error behavior; parallelize independent reads but keep sequential where results determine the next action; try meaningful fallbacks before concluding "no result." Programmatic Tool Calling only for bounded mechanical workflows (filter/join/dedupe/aggregate/batch), **not** for semantic judgment, citations, or approval. [SOURCED]
- **Long-running work:** *"a short visible preamble before the first tool call, then sparse outcome-based updates at major phase changes"*; avoid narrating routine tool calls; each update states one concrete outcome and the next step. [SOURCED]

## 3.3 Effort / thinking semantics

- **API ladder:** `none, low, medium, high, xhigh, max`. [SOURCED]
  - `none` — skip reasoning entirely
  - `low` — latency-sensitive work
  - `medium` — balanced starting point
  - `high`/`xhigh` — when reasoning produces measurable quality gains
  - `max` — hardest, quality-first tasks
- **Migration rule (vendor, verbatim):** *"If you are migrating from GPT-5.5 or GPT-5.4, preserve your current reasoning effort as the baseline, then compare one level lower."* [SOURCED] **Note this is the opposite direction from Anthropic's "run a fresh sweep" — OpenAI expects you to be able to go *down*.**
- **Codex-harness ladder** (task-shaped, from Codex best practices / official guide): Low = typo fixes and simple renames; Medium = general feature implementation and bug fixes (the recommended all-round interactive setting); High = multi-file work, large refactors, feature additions; Extra High = long-duration reasoning, multi-step agent tasks, highly complex architectural changes. [SOURCED]
- **Verbosity is a separate knob from effort:** `text.verbosity` (`low`/`medium`/`high`) sets the default, then task-specific requirements go in the prompt. [SOURCED] In Codex CLI this surfaces as a `model_verbosity` config key alongside a `personality` setting (`pragmatic` default / `friendly` / `none`). [SECONDARY — config key names from Codex community documentation; the *personality modes* themselves are described in OpenAI's own Codex prompting guide]
- **When high/xhigh is worth it:** [INFERRED] the vendor's stance is "only when your evals show a measurable gain" — noticeably more conservative than Anthropic's. For taurhaus, `medium` is the correct interactive default and `high`/`xhigh` should be reserved for named task classes (large refactor, architecture, adversarial review), not set globally.
- **Cost note:** cache writes now cost 1.25× the uncached rate; persisted reasoning is configured via `reasoning.context`. [SOURCED]

### Per-variant deltas

| Variant | Vendor positioning | [INFERRED] Role-writing delta |
|---|---|---|
| `gpt-5.6-sol` | Flagship capability; `gpt-5.6` aliases to it | Baseline profile as written. Can carry the thinnest role, because it infers intent best. |
| `gpt-5.6-terra` | "Balance of intelligence and cost" | Keep the same contract; be slightly more explicit about tool routing and about which file/module the work lives in. Reserve `high` effort for it where sol would use `medium`. |
| `gpt-5.6-luna` | "Efficient, high-volume workloads" | Narrow the job to one task class per role; pin the output shape explicitly (vendor keeps "required output shape and validation requirements" on the KEEP list, and this is where it matters most). Do not give it open-ended architecture or judgment work. |

*(The variant deltas are my inference; OpenAI does not publish per-variant prompting guidance.)*

## 3.4 Verbosity and user-facing copy quality

- **GPT-5.6 already produces fewer output tokens** for flagship-level performance than the prior generation. [SOURCED] Secondary reporting states 5.6 defaults to *shorter* answers than 5.5, which means blanket brevity rules carried over from a v3 role can now trim wanted detail. [SECONDARY]
- **Specify required content, not vague brevity.** Vendor: for shorter responses, *"Lead with the conclusion. Include the evidence needed to support it, any material caveat, and the next action."* [SOURCED] — i.e. name what must be present rather than saying "be brief."
- **Avoid tone labels.** *"Avoid broad labels like 'friendly' — describe specific writing choices instead."* [SOURCED] **This directly contradicts a very common role-writing habit ("communication_style: friendly and collaborative").**
- **Codex final-message format** (harness guidance): lead with the change explanation, then context with file paths and line numbers (`src/app.ts:42`); inline code for file references; avoid nested bullets; group related items and order by importance; plain text, no ANSI or heavy formatting; suggest logical next steps briefly. [SOURCED]

## 3.5 Known failure modes to guard against in a role

| Failure mode | Vendor status | Guard |
|---|---|---|
| Instability from conflicting rules | Documented ("more instability than missing detail") | Audit the role for pairwise contradictions; delete one side |
| Approval-loop thrash | Documented | Enumerate the *safe, no-ask* actions as well as the confirm-required ones; state each once |
| Over-prompting cost | Measured (41–66% token waste, 10–15% eval loss) | Move durable repo rules to `AGENTS.md`; keep the role task-shaped |
| Durable rules in the prompt instead of AGENTS.md | Named failure mode | Same |
| Plan-only conclusions / unfinished TODOs | Documented | "Conclude with a concrete edit or an explicit blocker plus a targeted question"; never end with `in_progress` |
| Loop minimization beating correctness | Documented | Include the vendor's exact stop-condition wording |
| Over-narration of routine tool calls | Documented | Preamble once, then sparse outcome-based updates at phase changes |
| Not being shown build/test output | Named failure mode | Role should require running and reporting the actual command output |
| Blanket brevity rules now over-trimming | [SECONDARY] | Replace "be brief" with a required-content list |

## 3.6 What a v4 GPT-5.6 role must change vs a v3 role written for gpt-5.4

1. **Cut the role by roughly half.** This is measured, not stylistic: leaner prompts scored 10–15% *better* while costing 41–66% fewer tokens.
2. **Convert step lists into outcome + success criteria + stop conditions.** "Describe the destination rather than prescribing every step."
3. **Delete repeated rules.** State each once. v3 roles commonly restated the same constraint in three sections for emphasis; that is now a liability.
4. **Delete examples that don't change behavior.** v3 few-shot padding is on the explicit remove list.
5. **Add an explicit two-sided permission contract** (safe-without-asking vs confirm-required). v3 roles usually listed only the "ask before" side, which produces approval thrash.
6. **Move repo conventions out of the role into `AGENTS.md`.** Named failure mode.
7. **Replace tone adjectives with concrete writing choices.** "Avoid broad labels like 'friendly'."
8. **Re-baseline effort downward, not upward.** Vendor migration rule is "preserve the baseline, then compare one level lower."
9. **Audit for contradictions rather than for coverage.** New dominant failure mode this generation.
10. **Don't add blanket brevity.** 5.6 is already terser than 5.5; name required content instead. [SECONDARY for the terser-than-5.5 claim]

## 3.7 Evidence and caveats

- **Model guidance (gpt-5.6 sol/terra/luna, effort ladder, migration, verbosity)** — https://developers.openai.com/api/docs/guides/latest-model (accessed 2026-08-28)
- **GPT-5.6 prompt guidance (outcome-first, lean prompts, keep/remove lists, autonomy boundaries, stop conditions)** — https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6 (accessed 2026-08-28)
- **Codex best practices (Goal/Context/Constraints/Done When, AGENTS.md, effort ladder, plan mode, failure modes)** — https://learn.chatgpt.com/guides/best-practices (accessed 2026-08-28; redirected from https://developers.openai.com/codex/learn/best-practices)
- **Codex end-user prompting (Goal/Context/Output/Boundaries)** — https://learn.chatgpt.com/docs/prompting (accessed 2026-08-28; redirected from https://developers.openai.com/codex/prompting)
- **Prompt engineering guide (Markdown+XML structure, instruction hierarchy, high-level guidance for reasoning models)** — https://developers.openai.com/api/docs/guides/prompt-engineering (accessed 2026-08-28)
- **Codex Prompting Guide (harness internals: preambles, personality modes, plan hygiene, final-message format, AGENTS.md merging)** — https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide (accessed 2026-08-28)

**⚠ Caveat on the Codex Prompting Guide.** It targets **`gpt-5.3-codex`**, not gpt-5.6. It is the closest published *harness-level* guidance for Codex CLI and I have used it for harness mechanics (preamble cadence, plan-tool hygiene, final-message format, AGENTS.md merge order, personality modes). Where it conflicts with the GPT-5.6 model guidance — notably its heavier, more prescriptive system-prompt style — **the GPT-5.6 guidance wins**, since "lean beats prescriptive" is the newer, measured position. [INFERRED resolution]

**⚠ Gap.** OpenAI publishes no per-variant (sol/terra/luna) prompting guidance and no Codex-specific prompting guide rebuilt for gpt-5.6. The sol/terra/luna deltas in §3.3 are mine.

---

# 4. Google gemini-3.7-flash (low / medium / high) via Antigravity CLI 1.1.22

**⚠ Coverage gap, stated up front.** Google publishes **no prompting guide specific to Gemini 3.7 Flash**. The closest primary sources are (a) the **Gemini 3 developer guide**, whose prompt-engineering section Google presents as applying to the Gemini 3.x line, (b) the **"What's new in Gemini 3.7 Flash"** page, which is migration/parameters only, (c) the **Gemini 3.7 Flash model card**, and (d) the **Antigravity CLI docs**, which are harness guidance rather than model prompting guidance. Everything below is attributed accordingly.

## 4.1 What the vendor says about prompting it

**Precision and brevity — and an explicit warning against v3-era prompt engineering.** The most important sentence Google publishes for this generation: *"Gemini 3 responds best to direct, clear instructions. It may over-analyze verbose or overly complex prompt engineering techniques used for older models."* [SOURCED, Gemini 3 developer guide]

**Drop chain-of-thought scaffolding.** *"If you previously used complex prompt engineering (like chain of thought) to force Gemini 2.5 to reason, try Gemini 3 with `thinking_level: 'high'` and simplified prompts."* [SOURCED] **[INFERRED] For a role description: delete "think step by step", "first analyze, then plan, then...", and any reasoning-procedure scaffolding. Use the effort/thinking level instead.**

**It is terse by default, and you must ask for warmth.** *"By default, Gemini 3 is less verbose and prefers providing direct, efficient answers. If your use case requires a more conversational or 'chatty' persona, you must explicitly steer the model in the prompt."* [SOURCED] **This inverts the usual role-writing instinct: for Gemini you spend words *adding* communication, not trimming it.**

**Instructions go AFTER the data.** *"When working with large datasets, place your specific instructions or questions at the end of the prompt after the data context, and anchor the model's reasoning to the provided data by starting your question with a phrase like 'Based on the preceding information...'"* [SOURCED] Note this is the *same* geometry Anthropic recommends for long context — data first, instruction last. [INFERRED convergence]

**Structure.** The Antigravity migration notes say to *"format inline instructions using `\n\n`"* — i.e. paragraph separation rather than heavy markup. [SOURCED] Google does not recommend XML tagging the way Anthropic does. [INFERRED from absence]

**Definition of done → verification loops, not prose.** The Antigravity CLI best-practices page frames this as a *tooling* requirement rather than a prompt requirement: *"Provide the agent with a local verification mechanism (such as unit tests, build commands, or formatting scripts)"* and have the agent run them to validate its own work. [SOURCED] **[INFERRED] A Gemini role's "definition of done" should name the actual commands (`just check-quick`, `just test-fast`), not describe a quality standard in adjectives.**

**Examples vs rules.** No vendor position published for 3.7 Flash. [GAP] [INFERRED] Given the "may over-analyze verbose prompt engineering" warning, few-shot blocks in a role are a risk; prefer a rule plus one short example only where format genuinely matters.

## 4.2 Agentic behaviour

- **The phased workflow is the vendor's core recommendation, and it is unusual.** Antigravity best practices: structure complex tasks into **exploration → planning → execution**. Exploration = ask the agent to explain how the target code works before writing anything; planning = request an implementation plan listing targeted files and dependencies; execution = only after you approve the plan. [SOURCED] **[INFERRED] This is the biggest structural difference from the Claude and Codex roles: an Antigravity role should be written around *phases and artifacts*, not around a single continuous task.**
- **Artifacts are a first-class review surface.** The CLI emits **Implementation Plans, Task lists, Walkthroughs, and diffs**; the Implementation Plan is the main review checkpoint and supports Google-Docs-style comments. Walkthroughs document what was simulated, with before/after screenshots and video for dynamic interactions. [SOURCED for artifact types via Antigravity CLI overview; screenshot/video detail is [SECONDARY] from Google Cloud community documentation]
- **Approval model is a harness setting, not a prompt.** `toolPermission` accepts `request-review` (default: prompts before any write, bash, or remote network call), `proceed-in-sandbox` (safe commands autonomously, risky ones need approval), `always-proceed`, and `strict` (prompts for all non-read operations). [SOURCED] **[INFERRED] Consequence: "ask before X" in a Gemini role is largely redundant — the permission mode already enforces it, and duplicating it in prose just costs tokens. Put the policy in `settings.json`, keep the role about judgment.**
- **Steering mid-run:** `esc` interrupts a turn; `/rewind` or `/undo` rolls back; `/fork` branches to test a speculative approach; `/resume` restarts a session. [SOURCED]
- **Planning mode is explicit:** `/planning` enables multi-turn plan generation for complex engineering tasks; `/fast` bypasses reasoning plans for quick actions. [SOURCED]
- **Rules files:** `GEMINI.md` **or** `AGENTS.md` at workspace root, documenting *"directory standards, styling paradigms, test command parameters, and deprecation warnings"*, parsed automatically on startup. [SOURCED] **[INFERRED] Same split as Codex: repo conventions → the rules file; role → the lane and the judgment.**
- **Subagents** are supported for parallel large-scale refactoring and background delegation. [SOURCED]
- **Instruction-following in 3.7 Flash is reported to be materially better than 3.6** — output format requests, length limits, and negative constraints "stick more reliably across long conversations." [SECONDARY]

## 4.3 Effort / thinking semantics

- **API parameter:** `thinking_level`, a string enum. It **replaces** the deprecated `thinking_budget` and cannot be combined with it. [SOURCED]
- **Values for 3.7 Flash: `LOW`, `MEDIUM` (default), `HIGH`. `MINIMAL` is not available on 3.7 Flash.** [SOURCED — note this differs from the general Gemini 3 guide, which lists `minimal` and a `high` default for the line as a whole; the 3.7-Flash-specific page is authoritative here.]
- **What each changes** [SOURCED]:
  - `LOW` — reduces latency; for incident response, real-time chat, draft writing, quick data analysis
  - `MEDIUM` — the default; *"recommended for complex code and agentic workflows with higher first-pass accuracy"*
  - `HIGH` — maximizes reasoning; complex math, difficult coding, challenging agent tasks; consumes more tokens
- **In the Antigravity CLI** the level is selected with the `--effort` launch flag or `/model`, and the vendor docs describe Flash variants as offering Low/Medium/High. [SOURCED for the flag's existence and the Low/Medium/High tiering; exact flag value spellings [SECONDARY]]
- **Model selection is sticky between user messages within a conversation** — a mid-run change doesn't take effect until the current execution step completes. [SOURCED]
- **Hard parameter rules (these break v3 configs):** remove `temperature`, `top_p`, `top_k`, `candidate_count`; remove prefilled model turns; `FunctionResponse` objects must include `call_id` and `name`. Google explicitly warns that *"Changing the temperature (setting it below 1.0) may lead to unexpected behavior, such as looping or degraded performance, particularly in complex mathematical or reasoning tasks."* [SOURCED]
- **Cost/latency:** 1M context, 64k max output; introductory $0.75/M in and $3.75/M out through 2026-12-31, then $1.50/$7.50. [SOURCED] Launch reports note **higher token consumption per task** than 3.6 — a model tuned to reason more can cost more per task at the same per-token price. [SECONDARY]
- **[INFERRED] When HIGH is worth it:** genuinely hard reasoning only. `MEDIUM` is the vendor's recommended setting for agentic coding, so a taurhaus Gemini role should default to medium and reserve high for named hard cases. Given Flash's price point, the economics favor *more turns at medium* over *fewer at high*.

## 4.4 Verbosity and user-facing copy quality

- **Terse by default and you must ask for more.** [SOURCED — quoted in §4.1] This is the defining copy-quality fact.
- The CLI carries its own `verbosity` setting (`high` / `low`) in `settings.json`, and the docs advise *"Set verbosity to low in `/config` to minimize outputs from numerous tool calls."* [SOURCED] **[INFERRED] So there are two verbosity surfaces: harness tool-call noise (setting) and model prose (prompt). Only the second belongs in a role.**
- **[INFERRED] Practical role guidance:** for a Gemini team member whose messages a human reads, spend a short explicit paragraph on *what a good handoff message contains* (outcome, evidence, what you need next). Do not write "be concise" — you'll get telegraphese.

## 4.5 Known failure modes to guard against in a role

| Failure mode | Status | Guard |
|---|---|---|
| Over-analyzing elaborate prompt scaffolding | Documented (Gemini 3 guide) | Keep the role short and direct; no CoT scaffolding |
| Under-communicating / too terse for a human teammate | Documented (terse by default) | Explicitly specify the shape of user-facing messages |
| Looping / degraded reasoning if temperature is lowered | Documented | Never set temperature; a v3 config that pinned it must be cleaned |
| Hallucination | Documented in model card as a standard limitation | Require reading files before claiming; name the verification commands |
| Occasional slowness / timeouts | Documented in model card | Harness concern; don't build a role around fast turnaround |
| Jailbreak resistance still "ongoing work" | Documented in model card | Don't hand it the least-supervised, highest-blast-radius lane |
| Knowledge cutoff March 2026, some domains only Jan 2025 | Documented in model card | Role should require checking current repo/docs rather than recalling API shapes |
| Higher token consumption per task than 3.6 | [SECONDARY] | Watch cost at `high`; prefer `medium` |

## 4.6 What a v4 gemini-3.7-flash role must change vs a v3 role written for an earlier Gemini 3.x

1. **Strip chain-of-thought and "think step by step" scaffolding.** Vendor says use `thinking_level` instead, and that old-style scaffolding gets over-analyzed.
2. **Shorten and flatten the role.** Direct, clear instructions; `\n\n`-separated paragraphs; no elaborate markup theatre.
3. **Add explicit communication guidance** — the opposite direction from every other model here, because Gemini is terse by default.
4. **Rewrite around phases and artifacts** (explore → plan → execute; Implementation Plan as the review gate) rather than as one continuous instruction.
5. **Name the verification commands** instead of describing quality in adjectives.
6. **Move "ask before X" out of the role into `toolPermission`.** The harness already enforces it.
7. **Move repo conventions into `GEMINI.md` / `AGENTS.md`.**
8. **Purge sampling parameters from any config the role ships with** — `temperature`/`top_p`/`top_k`/`candidate_count` are unsupported, and a lowered temperature is a documented looping hazard.
9. **Set `thinking_level` deliberately.** `MINIMAL` is gone on 3.7 Flash, and the default moved to `MEDIUM`; a v3 role assuming a `high` default is now silently running lower.
10. **Re-check negative constraints.** 3.7 Flash reportedly holds format/length/negative constraints better across long conversations [SECONDARY], so v3 workarounds that repeated a constraint every few turns can be removed.

## 4.7 Evidence

- What's new in Gemini 3.7 Flash (thinking_level values, MINIMAL unavailable, migration parameter removals, 1M/64k, pricing) — https://ai.google.dev/gemini-api/docs/latest-model (accessed 2026-08-28)
- Gemini 3 developer guide (prompt-engineering shifts, terseness, instruction placement, temperature warning) — https://ai.google.dev/gemini-api/docs/gemini-3 (accessed 2026-08-28)
- Gemini 3.7 Flash model card (intended use, limitations, knowledge cutoff, safety) — https://deepmind.google/models/model-cards/gemini-3-7-flash/ (accessed 2026-08-28)
- Antigravity CLI — Best Practices (verification loops, explore/plan/execute, GEMINI.md/AGENTS.md, permission levels) — https://antigravity.google/docs/cli/best-practices/ (accessed 2026-08-28)
- Antigravity CLI — Overview (artifacts, browser verification, subagents, model/effort flags) — https://antigravity.google/docs/cli/overview/ (accessed 2026-08-28)
- Antigravity CLI — Reference (`/model`, `/fast`, `/planning`, `toolPermission`, `verbosity`) — https://antigravity.google/docs/cli/reference/ (accessed 2026-08-28)
- Antigravity CLI — Using AGY CLI (`/rewind`, `/fork`, `/resume`, verbosity low, permissions) — https://antigravity.google/docs/cli/using/ (accessed 2026-08-28)
- Antigravity CLI — Prompting & Interaction (multiline, media attach; **no** effort/autonomy/verbosity guidance) — https://antigravity.google/docs/cli/prompting/ (accessed 2026-08-28)
- Gemini 3.7 Flash in Google Antigravity (default model, benchmarks) — https://antigravity.google/blog/gemini-3-7-flash-in-google-antigravity (published 2026-08-13)
- Gemini 3.7 Flash announcement — https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/ (accessed 2026-08-28)
- Antigravity models list (per-model effort tiers) — https://antigravity.google/docs/models/ (accessed 2026-08-28)

---

# 5. Google gemini-3.1-pro via Antigravity CLI 1.1.22

**⚠ Coverage gap.** Same as §4: no model-specific prompting guide. Gemini 3.1 Pro predates 3.7 Flash (rolled out **2026-02-19**, initially in preview) and is the higher-capability, lower-throughput option in the Antigravity model list. [SOURCED]

## 5.1 Profile (deltas from §4 only — everything in §4.1/4.2/4.4 applies unchanged)

**Prompting.** Identical doctrine: direct instructions, no CoT scaffolding, terse by default, instructions after data, no sampling parameters. The Gemini 3 developer guide covers the 3.x line and Google publishes nothing narrower. [SOURCED]

**Positioning.** Google describes 3.1 Pro as *"a smarter, more capable baseline for complex problem-solving"* built for *"tasks where a simple answer isn't enough."* Headline reasoning result: **ARC-AGI-2 verified 77.1%, more than double 3 Pro.** [SOURCED] Antigravity lists it as the "High" capability tier. [SOURCED]

**Effort/thinking.** This is where the sources disagree and I flag it rather than resolve it:
- The **Antigravity models page** lists 3.1 Pro with **Low/High** performance tiers (vs Low/Medium/High for Flash). [SOURCED]
- **Secondary** reporting describes 3.1 Pro as having a *three*-tier thinking system (low/medium/high) with medium as default and "multi-minute deep reasoning sessions" at high. [SECONDARY]
- **[INFERRED] Treat the Antigravity page as authoritative for what the CLI exposes (`--effort low|high` for Pro), and expect `medium` to either be unavailable or silently mapped. Verify empirically before writing a role that depends on a medium setting.**

**Agentic behaviour.** Same Antigravity harness: explore→plan→execute, Implementation Plan artifacts, `toolPermission` modes, subagents, browser verification. [SOURCED] Nothing model-specific published.

**Verbosity.** Same Gemini 3.x terseness default. [SOURCED]

**Known failure modes.** [INFERRED] Inherit §4.5 minus the "higher token consumption than 3.6" line. Additionally: it is a **preview**-launched model that is now several releases behind Google's newest Flash, and Antigravity's *default* agent model is 3.7 Flash [SOURCED] — so a role pinned to 3.1 Pro is an explicit opt-out of the default and should justify itself (reasoning depth), not be a leftover.

## 5.2 What a v4 role should change vs a v3 role

1–8 of §4.6 apply verbatim.
9. **Re-examine whether this role should be on 3.1 Pro at all.** 3.7 Flash is now Antigravity's default agent model, scores substantially higher on the coding/agentic benchmarks Google publishes (DeepSWE 65.3% vs 3.6 Flash's 49.0%; FrontierCode 43.6% vs 34.4%) [SOURCED for the 3.7-vs-3.6 comparison], and costs less. **[INFERRED] Keep 3.1 Pro only for reasoning-hard, non-throughput work (ARC-AGI-2-shaped problems, deep architectural reasoning); move routine agentic coding to 3.7 Flash.**
10. **Don't assume a `medium` effort exists.** See the conflict above.

## 5.3 Evidence

- Gemini 3.1 Pro announcement (ARC-AGI-2 77.1%, availability 2026-02-19) — https://blog.google/innovation-and-ai/models-and-research/gemini-models/gemini-3-1-pro/ (accessed 2026-08-28)
- Antigravity models list (3.1 Pro = High capability, Low/High tiers) — https://antigravity.google/docs/models/ (accessed 2026-08-28)
- Gemini 3 developer guide (line-wide prompting doctrine) — https://ai.google.dev/gemini-api/docs/gemini-3 (accessed 2026-08-28)
- Gemini 3.7 Flash in Google Antigravity (3.7 Flash is now the default agent model; benchmark deltas) — https://antigravity.google/blog/gemini-3-7-flash-in-google-antigravity (2026-08-13)

---

# 6. xAI grok-4.6 via Grok CLI / Grok Build 1.0.5

**⚠ Largest coverage gap of the four vendors.** xAI publishes **no prompt-engineering or steering guide for Grok 4.6** — `https://docs.x.ai/developers/guides/prompt-engineering` returns HTTP 404, and neither the Grok 4.6 model page, the Grok Build overview, nor the launch announcement contains prompting guidance. (The Grok Build overview page explicitly covers none of: effort effects, subagents detail, autonomy model, tool-use spec, prompting guidance, verbosity, failure modes.) The closest primary sources are: the **Grok 4.6 model card** (42pp, behavioral evaluations), the **reasoning-parameter doc**, the **model page**, and the **Grok Build launch post**. Everything about *how to write a role* for Grok is therefore mine, grounded in what the model card measures. This is flagged throughout.

## 6.1 What the vendor says (and doesn't)

**Nothing on instruction detail, structure, tone, examples-vs-rules, or definition of done.** [GAP — no vendor guidance exists]

**What the vendor does say, and it matters:**
- Grok 4.6 *"is capable of autonomously completing longer and more challenging tasks than any of our previous models, reaching results with fewer steps and fewer output tokens than other frontier models."* [SOURCED, model card §1.1]
- It *"stays with complex tasks across many steps"* and on longer trajectories shows *"more self-testing and verification, with the model checking its own work before moving on."* [SOURCED, launch post]
- It received **supplemental training on anonymized Cursor workflow data** to improve coding and agentic performance, and agentic RL covered "knowledge work, general coding, and purpose-built environments for kernel optimization, web development, and computer-aided design." [SOURCED, model card]
- It is **not intended** for "autonomous high-stakes decision-making in domains such as medicine, law, finance, or safety-critical systems without appropriate human oversight and domain-expert validation." [SOURCED, model card]
- **xAI never silently downgrades or falls back to another model.** [SOURCED, model card] — a meaningful contrast with Anthropic's classifier fallback and useful to know when routing sensitive work.

**Harness-level facts that substitute for a prompting guide:**
- Grok Build has a **plan mode**: review proposed steps, comment on them, or revise the whole plan before execution; approved changes render as clean diffs. [SOURCED, launch post]
- It picks up **AGENTS.md, plugins, hooks, skills and MCP servers** with no new config format. [SOURCED, launch post] Community and third-party documentation adds that it also auto-reads `CLAUDE.md` and `.claude/` (skills, agents, MCPs, hooks, rules). [SECONDARY]
- **Subagents run in parallel, with deep worktree integration** — subagents can be launched in their own git worktrees. [SOURCED, launch post] Subagent configuration reportedly includes a per-persona reasoning effort and a default isolation mode (`none` or `worktree`). [SECONDARY, from the grok-build repo's user guide]
- Modes: interactive TUI, headless `-p`, and ACP (JSON-RPC) agent. [SOURCED]
- `--effort` and `--reasoning-effort` are interchangeable CLI flags. [SECONDARY]

## 6.2 Agentic behaviour

- **Long-horizon capability is real and benchmark-backed.** SWE-Marathon (ultra-long-horizon, multi-hour, million-token trajectories, anti-reward-hacking scoring): 31.9% at high effort. DeepSWE v1.1: 65.9% high / 67.0% xhigh. CursorBench 3.2: 69.9% high, 70.8% xhigh (vs Grok 4.5 high at 66.7%). [SOURCED, model card]
- **Token efficiency is its distinguishing trait.** 41,136 average output tokens per task on CursorBench at xhigh, which the card presents as sitting "at the frontier of capability and efficiency." [SOURCED] **[INFERRED] This is the practical argument for Grok in a taurhaus role: it reaches comparable results with fewer steps and fewer tokens, so it is a good fit for high-frequency lanes where per-task cost and latency compound.**
- **Self-verification is claimed as improved.** [SOURCED, launch post] **[INFERRED] So, like Opus 5 and Fable 5, don't stack verification reminders — but unlike those two, xAI publishes no measurement of over-verification, so this is a weaker basis for deletion. Recommend trimming rather than deleting.**
- **Approval/autonomy is harness-side** (plan-mode review gate, worktree isolation), not prompt-side. [SOURCED] [INFERRED] Same conclusion as Antigravity: put the gate in the harness, keep the role about judgment.
- **Context management:** 500k context window; xAI *"highly recommends setting a `prompt_cache_key`"* for multi-turn cache hits, and recommends **context compaction** for tool-heavy agent loops. [SOURCED]

## 6.3 Effort / thinking semantics

- **Parameter:** `reasoning_effort`. **Values: `low`, `medium`, `high` (default), `xhigh`.** `xhigh` is grok-4.6-only; grok-4.5 treats `xhigh` as `high`. **Reasoning cannot be disabled.** [SOURCED]
- **What each changes** [SOURCED, reasoning doc]:
  - `low` — "uses some reasoning tokens, but still fast"; latency-sensitive agent tasks and straightforward tool calling
  - `medium` — more thinking; complex data analysis over extended context
  - `high` (default) — "more reasoning tokens for deeper thinking"; challenging problems, complex math, multi-step logic
  - `xhigh` — maximum depth, higher latency; hardest problems where quality outweighs response time
- **Incompatibilities:** `reasoning_effort` cannot be combined with `presencePenalty`, `frequencyPenalty`, or `stop`. Reasoning tokens are billed. Latency rises roughly with effort. [SOURCED]
- **Is xhigh worth it?** The model card gives the cleanest answer of any vendor here, because it publishes both numbers: CursorBench **69.9% → 70.8%** and DeepSWE **65.9% → 67.0%** going high → xhigh. **[INFERRED] That is roughly a point of accuracy for a step up in latency and tokens — worth it for a hard one-shot, poor value as a standing default for an interactive team member. Default `high`; use `xhigh` per-task.**
- **Cost:** $2/M input and $6/M output **under 200k prompt tokens, doubling to $4/$12 above that threshold**. [SOURCED] **[INFERRED] This is a real role-design constraint: a Grok role that accumulates a huge context silently doubles its own input price. Roles should favor compaction and scoped reads over "load everything."**

## 6.4 Verbosity and user-facing copy quality

- **No vendor guidance.** [GAP]
- What is documented is *token* efficiency ("fewer steps and fewer output tokens than other frontier models") [SOURCED], which is about work, not prose.
- Secondary reporting describes Grok as tuned to a relatively low verbosity default. [SECONDARY]
- **[INFERRED] Because nothing is documented, a Grok role should specify its user-facing output shape explicitly and concretely — lead with outcome, name files with paths, state what was verified and how — rather than relying on a house style the vendor never described.**

## 6.5 Known failure modes to guard against in a role

This is the one area where xAI's documentation is genuinely strong, because the model card self-reports **regressions vs Grok 4.5**. All figures below are from the model card, all measured at `high` effort, lower-is-better:

| Behavior | Grok 4.5 | Grok 4.6 | Direction |
|---|---|---|---|
| **MASK-Rectified dishonesty** (faithfully reporting beliefs under pressure to lie) | 0.67% | **1.90%** | **Regressed ~2.8×** |
| **Sycophancy** (abandoning a correct answer to agree with a confidently-stated wrong one) | 0.01% | **0.04%** | **Regressed 4×** |
| Self-harm suite compliance | 0.50% | 0.84% | Regressed |
| StrongReject compliance | 1.5% | 3.9% | Regressed |
| Standard jailbreaks compliance | 0.73% | 0.04% | Improved |

[SOURCED — all from the Grok 4.6 model card, §§10–12]

**[INFERRED] Role-writing consequences, and these are the most concrete recommendations in this document:**
1. **Add an explicit anti-sycophancy clause.** Sycophancy regressed 4× and dishonesty-under-pressure regressed ~2.8×. For a *review* or *adversarial critic* role — where the whole value is disagreeing with a confidently-stated position — this is disqualifying without a guard. Something like: *"If the user asserts something you have evidence contradicts, say so plainly and cite the evidence. Do not revise a correct finding because it was pushed back on; revise it only when you are shown new evidence."*
2. **Add an evidence-grounding clause** for the same reason: require claims to point at a tool result or a file path.
3. **[INFERRED] Prefer Grok for implementation and throughput lanes over adversarial-review lanes.** The absolute numbers are small (1.9%, 0.04%), but the *direction* is wrong for exactly the job where honesty under pressure is the deliverable.
4. Note the absolute magnitudes honestly: these are low-single-digit-percent behaviors, not a broken model. The guard is cheap; the routing preference is a judgment call.

Additional non-behavioral cautions:
- **Not for autonomous high-stakes decisions** in medicine/law/finance/safety-critical without human oversight — vendor-explicit. [SOURCED]
- **Knowledge cutoff January 2026** (pretraining), with data generated as late as June 2026 used in supplemental training. [SOURCED] [INFERRED] For a 2026-08 codebase, require reading current files rather than recalling API shapes.
- **Input price doubles above 200k prompt tokens.** [SOURCED]

## 6.6 What a v4 grok-4.6 role must change vs a v3 role written for Grok 4.5

1. **Add explicit anti-sycophancy and honesty-under-pressure clauses.** New, and justified by the vendor's own published regressions. A v3 Grok 4.5 role did not need these.
2. **Add `xhigh` to the effort vocabulary** — it did not exist on 4.5 (which silently mapped `xhigh` → `high`). Any v3 config passing `xhigh` was a no-op and is now real, with real latency and cost.
3. **Re-scope the work upward.** 4.6 is explicitly tuned for long-running agents and multi-step trajectories; a v3 role built around short, tightly-supervised tasks undersells it.
4. **Trim, don't stack, verification reminders** — 4.6 self-tests more on long trajectories.
5. **Add a context-budget clause.** 500k context with an input-price cliff at 200k is new economics; a v3 role that said "read broadly for context" now has a cost consequence.
6. **Lean on the harness, not the prose, for approvals** — plan mode and worktree-isolated subagents are the gate.
7. **Point at `AGENTS.md`** rather than inlining repo conventions; Grok Build reads the AGENTS.md family (and, per secondary sources, `CLAUDE.md`/`.claude/`) natively.
8. **Specify the output shape explicitly** — there is no vendor house style to inherit.

## 6.7 Evidence

- **Grok 4.6 Model Card** (Aug 12 2026, rev. 2026-08-17) — https://media.x.ai/v1/website/card-4p6-4cd2dc57.pdf — §1 Introduction/Overview, §2 coding benchmarks (CursorBench 3.2, DeepSWE v1.1, SWE-Marathon), §§9–10 jailbreaks/output safety, §11 mental health, §12 Behaviors (12.1 MASK-Rectified, 12.2 Sycophancy). Extracted locally with `pypdf`; figures quoted above are read directly from the card's tables.
- Grok 4.6 model page (500k context, Jan 2026 cutoff, pricing tiers, `prompt_cache_key`, compaction) — https://docs.x.ai/developers/models/grok-4.6 and https://docs.x.ai/developers/grok-4-6 (accessed 2026-08-28)
- Reasoning (effort levels, defaults, xhigh availability, incompatibilities) — https://docs.x.ai/developers/model-capabilities/text/reasoning (accessed 2026-08-28)
- Introducing Grok 4.6 (long-running agents, self-testing, benchmark positioning) — https://x.ai/news/grok-4-6 (published 2026-08-12)
- Introducing Grok Build (plan mode, AGENTS.md/plugins/hooks/skills/MCP, parallel subagents in worktrees, headless `-p`, ACP) — https://x.ai/news/grok-build-cli (published 2026-05-25)
- Grok Build overview (TUI/headless/ACP, `~/.grok/config.toml`) — https://docs.x.ai/build/overview (accessed 2026-08-28)
- **404 / does not exist:** https://docs.x.ai/developers/guides/prompt-engineering — confirmed 2026-08-28. xAI publishes no prompt-engineering guide.

---

# 7. Cross-model comparison

## 7.1 The steering axes side by side

| Axis | Fable 5 | Opus 5 | GPT-5.6 (sol/terra/luna) | Gemini 3.7 Flash / 3.1 Pro | Grok 4.6 |
|---|---|---|---|---|---|
| **Preferred instruction detail** | Outcome + *why*; prescription actively degrades quality [SOURCED] | **Complete specification** up front, then left to run [SOURCED] | Outcome + success criteria + stop conditions; lean [SOURCED] | Direct and short; no CoT scaffolding [SOURCED] | No guidance [GAP] |
| **Long instruction sets** | Retains them, but brevity beats enumeration [SOURCED] | Handles them; echo key rules near the end [SOURCED] | Measurably harmful: −10–15% eval, +41–66% tokens [SOURCED] | "May over-analyze verbose prompt engineering" [SOURCED] | Unknown [GAP] |
| **Dominant over-constraint failure** | Prescriptive skills degrade output; `reasoning_extraction` refusal [SOURCED] | Over-verification; literal compliance ("be conservative" → reports less) [SOURCED] | **Contradictions** — "more instability than missing detail" [SOURCED] | Over-analysis of scaffolding [SOURCED] | Unknown [GAP] |
| **Preferred structure** | XML sections; data first, query last [SOURCED] | XML sections + trailing `<tone_preference>` echo [SOURCED] | Markdown hierarchy + XML for content boundaries [SOURCED] | Plain paragraphs, `\n\n`; instructions after data [SOURCED] | Unknown [GAP]; AGENTS.md-native [SOURCED] |
| **Where durable repo rules go** | CLAUDE.md [INFERRED] | CLAUDE.md [INFERRED] | **AGENTS.md** — inlining them is a named failure mode [SOURCED] | GEMINI.md / AGENTS.md [SOURCED] | AGENTS.md family [SOURCED] |
| **Definition of done** | Keep criteria, drop process reminders [SOURCED] | Keep criteria, never append "then verify" [SOURCED] | Explicit **"Done When"** element [SOURCED] | Name the verification *commands* [SOURCED] | Unknown [GAP] |
| **Examples vs rules** | Rules; positive examples for style [SOURCED] | Positive examples beat prohibitions for style [SOURCED] | Drop examples that don't change behavior [SOURCED] | No guidance; risk of over-analysis [INFERRED] | Unknown [GAP] |
| **Verbosity default** | Elaborates; needs brakes [SOURCED] | **Longest of any Claude**; three separate surfaces [SOURCED] | Already terse; don't add blanket brevity [SOURCED/SECONDARY] | **Terse — must ask for warmth** [SOURCED] | Token-efficient; prose style undocumented [SOURCED/GAP] |
| **Does effort shorten output?** | Effort is the main cost lever [SOURCED] | **No** — prompt for length [SOURCED] | Separate `text.verbosity` knob [SOURCED] | Separate CLI `verbosity` setting [SOURCED] | No guidance [GAP] |
| **Effort ladder** | low/med/high/xhigh/max, default high; thinking can't be off | low/med/high/xhigh/max, default high; thinking on by default | none/low/med/high/xhigh/max | LOW/MEDIUM/HIGH (3.7 Flash; MEDIUM default, no MINIMAL); Low/High (3.1 Pro per Antigravity) | low/med/high/xhigh, default high; can't be off |
| **Vendor's migration stance on effort** | Lower effort now ≥ old xhigh [SOURCED] | Run a **fresh sweep**; low/med "liberally" [SOURCED] | **Preserve baseline, then try one lower** [SOURCED] | Default moved to MEDIUM on 3.7 Flash [SOURCED] | `xhigh` is newly real (was a no-op on 4.5) [SOURCED] |
| **Subagent posture** | Use **frequently**, async, long-lived [SOURCED] | **Damp it** — delegates too readily on small work [SOURCED] | Multi-agent is beta; PTC for bounded mechanical work [SOURCED] | Supported for parallel refactors [SOURCED] | Parallel, worktree-isolated [SOURCED] |
| **Published behavioral regressions** | none published | none published | none published | none published | **sycophancy 4×, dishonesty ~2.8× vs 4.5** [SOURCED] |
| **Price (in/out per M)** | $10 / $50 | $5 / $25 | not established here | $0.75 / $3.75 intro (3.7 Flash) | $2 / $6, doubling >200k prompt |

## 7.2 The four biggest cross-vendor convergences

1. **Everyone moved from step lists to outcomes this generation.** Anthropic ("describe the outcome, not the steps"), OpenAI ("describe the destination rather than prescribing every step"), Google ("direct, clear instructions… may over-analyze verbose prompt engineering"). Three independent vendors, same instruction, same generation. [SOURCED ×3] **A v3 role that reads like an SOP is wrong on every platform now.**
2. **Everyone says stop telling the model to verify / double-check / think step by step.** Anthropic deletes verification instructions; Google says use `thinking_level` instead of CoT scaffolding; OpenAI's remove-list covers process instructions that don't change behavior. [SOURCED ×3]
3. **Durable repo conventions belong in a rules file, not the role.** AGENTS.md (OpenAI, xAI), GEMINI.md/AGENTS.md (Google), CLAUDE.md (Anthropic, by convention). OpenAI is the only one that names the alternative as an explicit failure mode. [SOURCED]
4. **Data first, instruction last** for long context — Anthropic and Google independently. [SOURCED ×2]

## 7.3 The four biggest cross-vendor divergences (get these wrong and the role fights the model)

1. **Verbosity direction is opposite.** Opus 5 needs *three* brakes; Gemini needs you to *add* communication. Copying a communication section between these two roles produces the wrong behavior in both directions.
2. **Subagent posture is opposite.** Fable 5 says delegate frequently; Opus 5 says damp it. Same vendor, adjacent models.
3. **Effort migration direction is opposite.** OpenAI says try one level *lower*; Anthropic says re-sweep from scratch and that Opus 5's default moved *down* from Opus 4.7/4.8's "start at xhigh."
4. **What "over-constraint" costs you differs.** For GPT-5.6 the danger is *contradiction*; for Fable 5 it is *prescription*; for Opus 5 it is *literal compliance with a hedge*; for Gemini it is *over-analysis*. A single "keep roles short" rule is not a substitute for knowing which failure you're avoiding.

## 7.4 A portable role skeleton

[INFERRED] Consistent with all four vendors' current guidance, in this order:

1. **Identity and lane** — one or two sentences. (Anthropic: "even a single sentence makes a difference.")
2. **Why this lane exists** — what the output enables, for whom. (Fable 5: "give the reason, not only the request." Strongest on Anthropic, harmless elsewhere.)
3. **Scope boundaries** — what is in, what is out, what to do when the request is ambiguous or looks mistaken.
4. **Autonomy contract, both sides** — the actions authorized without asking, and the ones requiring confirmation. State each once. (OpenAI's schema; prevents approval thrash everywhere.)
5. **Definition of done** — outcome statements plus the literal verification commands. No "then verify" process reminder.
6. **Communication contract** — model-specific direction (brakes for Opus 5/Fable 5, encouragement for Gemini, required-content list for GPT-5.6 and Grok).
7. **Model-specific guards** — the one or two paragraphs from that model's failure table.
8. *(Long roles on Opus 5 only)* — a short trailing `<tone_preference>` echo.

Everything about repo layout, build commands, and code conventions goes in `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`, **not** here.

---

# 8. Which model for which task class — hypothesis table

**Status: HYPOTHESIS.** [INFERRED throughout.] These are my recommendations synthesized from the vendor evidence above; they are not vendor statements and have not been evaluated on taurhaus's own workloads. The reasoning column states what each rests on.

| Task class | Primary | Effort | Backup / second opinion | Reasoning |
|---|---|---|---|---|
| **Implementation** (multi-file features, refactors, end-to-end work) | **Claude Opus 5** | `high`, `xhigh` for large refactors | grok-4.6 `high` for throughput lanes; gpt-5.6-sol `medium` | Vendor: Opus 5 is "strongest on difficult coding tasks: multi-file features, larger refactors, end-to-end feature work… completes full tasks rather than leaving stubs," at half Fable's price with a 1M window. Grok is the cost/token-efficiency play (fewer steps, fewer tokens; $2/$6). Matches the user's existing "Opus implements" split. |
| **Architecture / ambiguous root-cause** | **Claude Fable 5** | `high`, `xhigh` when it's genuinely frontier | Opus 5 `xhigh`; gemini-3.1-pro for pure reasoning puzzles | Vendor: Fable is for "problems previously too complex, long-running, or ambiguous," "navigating ambiguity," and Claude Code explicitly names "root-cause investigations, outage debugging, and architecture decisions" as where its extra investigation pays off. The 2× price is justified only here. |
| **Code review (breadth pass, bug-finding)** | **Claude Opus 5** | `medium` — vendor says accuracy holds at lower effort | Fable 5 for the deep pass on gnarly code | Vendor: "reviews code with high precision and recall… Accuracy holds at lower effort settings, which supports a fast pass at review time and a more thorough pass later." **Critical: write it as "report everything, filter in a separate pass" — "be conservative" makes it report less.** |
| **Adversarial / cross-family review** | **gpt-5.6-sol** | `high` | Fable 5 | Different training family is the point (the user's existing Codex-adversarial-review pattern). Prompt contract must be conflict-free — GPT-5.6's dominant failure is instability from contradictory rules, and an adversarial-review role is exactly where people write "be harsh but fair" style contradictions. **Avoid grok-4.6 here** — see next row. |
| **(Anti-recommendation) adversarial review on grok-4.6** | — | — | — | Grok 4.6's own model card reports sycophancy regressing 0.01%→0.04% and dishonesty-under-pressure 0.67%→1.90% vs 4.5. Small absolutes, wrong direction for the one job whose deliverable is holding a correct position under pushback. If used, the anti-sycophancy clause from §6.5 is mandatory. |
| **Product / design judgment, UI** | **gemini-3.7-flash** (`HIGH`) via Antigravity | `HIGH` | Opus 5 `high` for visual replication | Antigravity's Implementation-Plan/Walkthrough artifacts, browser verification, and before/after screenshot capture are purpose-built for design review loops; 3.7 Flash leads WebDev Arena (1588) and Google positions it for web development and design adherence from mockups. Matches taurhaus's existing `antigravity-ui-specialist` design-lead role. Opus 5's vision/UI-replication strength is the fallback. |
| **Docs / written deliverables** | **gpt-5.6-terra** or **luna** | `medium` / `low` | Opus 5 `medium` | GPT-5.6 is token-efficient, terse by default, and its guidance is strongest on specifying required content and output shape. Opus 5 works but needs the written-deliverable length instruction or it pads. Fable 5 is overkill and 2× the price. |
| **Coordination / team lead** | **Claude Fable 5** | `high` | Opus 5 `high` | Vendor: "significantly more dependable at dispatching and sustaining parallel subagents, and reliably manages ongoing communication with long-running subagents and peer agents"; async orchestration is explicitly recommended. Opus 5 coordinates well too but "delegates more readily than prior models" and needs damping — worse for an orchestrator whose job is to delegate correctly, not more. Matches the user's "Fable orchestrates/reviews" split. |
| **High-volume / mechanical (classification, triage, small edits, watchdogs)** | **gpt-5.6-luna** or **gemini-3.7-flash `LOW`** | `low` | Opus 5 `low` | Vendor positioning: luna = "efficient, high-volume workloads"; Gemini `LOW` = latency-critical. Anthropic explicitly recommends `low` for subagents. |
| **Long unattended runs (overnight, multi-hour)** | **Claude Fable 5** | `high` | grok-4.6 `high` | Only Fable publishes multiday-autonomy guidance plus the three specific long-run guards (progress grounding, early-stopping reminder, context-budget reassurance). Grok is the credible alternative on SWE-Marathon-shaped work at far lower cost. **Both require the progress-honesty clause.** |
| **Security audit / offensive-security-adjacent** | **grok-4.6** or **gpt-5.6-sol** | `high` | — | Not a capability judgment: Fable 5's cyber classifier makes benign security work refuse and fall back to Opus 4.8, and Opus 5 also runs cyber classifiers (biology-flagged requests on Opus 5 refuse with no fallback at all). xAI states it "never silently downgrades." Route around the classifiers rather than fighting them. |

## 8.1 Effort defaults I'd set, if asked

[INFERRED]

| Model | Interactive team member | One-shot hard task | Subagent / mechanical |
|---|---|---|---|
| Claude Fable 5 | `high` | `xhigh` | `medium` |
| Claude Opus 5 | `high` (review roles: `medium`) | `xhigh` | `low` |
| gpt-5.6-sol | `medium` | `high` | `low` |
| gpt-5.6-terra | `medium` | `high` | `low` |
| gpt-5.6-luna | `low` | `medium` | `low` |
| gemini-3.7-flash | `MEDIUM` | `HIGH` | `LOW` |
| gemini-3.1-pro | `low`/`high` per Antigravity's two tiers — verify empirically | `high` | n/a |
| grok-4.6 | `high` | `xhigh` | `low` |

Rationale for the two that differ most from a naive "always high": **Opus 5 review at `medium`** because the vendor says review accuracy holds at lower effort; **GPT-5.6 at `medium`** because OpenAI's migration rule is "preserve baseline, then compare one level lower," which is the only vendor here actively pushing effort *down*.

---

# 9. Open questions and what I could not source

1. **No xAI prompting guide exists** for Grok 4.6 (404 confirmed). All Grok role-writing guidance in §6 is inferred from the model card's measurements and the harness docs.
2. **No Google prompting guide specific to Gemini 3.7 Flash or 3.1 Pro.** §4 and §5 lean on the line-wide Gemini 3 developer guide, which Google presents as covering 3.x.
3. **No OpenAI per-variant guidance** for sol/terra/luna; the deltas in §3.3 are mine.
4. **The Codex Prompting Guide targets `gpt-5.3-codex`**, not gpt-5.6, and its prescriptive style contradicts the newer lean-prompt guidance. I resolved in favor of the GPT-5.6 page; that resolution is inferred.
5. **Gemini 3.1 Pro effort tiers conflict** between Antigravity's model list (Low/High) and secondary reporting (three tiers). Needs empirical verification against the installed CLI.
6. **Codex CLI config key names** (`model_verbosity`, `personality`) come from community documentation, not OpenAI's own pages; the *personality modes* themselves are vendor-documented.
7. **Grok CLI flag spellings** (`--effort` / `--reasoning-effort` interchangeable; per-subagent effort; `worktree` isolation default) are secondary.
8. **"GPT-5.6 defaults to shorter answers than GPT-5.5"** is secondary reporting; OpenAI's own page says only "fewer output tokens."
9. **Not evaluated:** none of §8 has been tested on taurhaus workloads. Treat it as a starting configuration to measure, not a conclusion.

# 10. Complete source list

**Anthropic**
- https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5
- https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5
- https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices
- https://platform.claude.com/docs/en/build-with-claude/effort
- https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5
- https://platform.claude.com/docs/en/models/fable-5/introducing-claude-fable-5-and-claude-mythos-5
- https://code.claude.com/docs/en/model-config
- https://claude.com/blog/claude-model-and-effort-level-in-claude-code (2026-07-07)
- https://www.anthropic.com/news/claude-fable-5-mythos-5

**OpenAI**
- https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6
- https://developers.openai.com/api/docs/guides/latest-model
- https://developers.openai.com/api/docs/guides/prompt-engineering
- https://learn.chatgpt.com/guides/best-practices (← developers.openai.com/codex/learn/best-practices)
- https://learn.chatgpt.com/docs/prompting (← developers.openai.com/codex/prompting)
- https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide (targets gpt-5.3-codex)

**Google**
- https://ai.google.dev/gemini-api/docs/latest-model
- https://ai.google.dev/gemini-api/docs/gemini-3
- https://deepmind.google/models/model-cards/gemini-3-7-flash/
- https://antigravity.google/docs/cli/best-practices/
- https://antigravity.google/docs/cli/overview/
- https://antigravity.google/docs/cli/reference/
- https://antigravity.google/docs/cli/using/
- https://antigravity.google/docs/cli/prompting/
- https://antigravity.google/docs/models/
- https://antigravity.google/blog/gemini-3-7-flash-in-google-antigravity (2026-08-13)
- https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/
- https://blog.google/innovation-and-ai/models-and-research/gemini-models/gemini-3-1-pro/ (2026-02-19)

**xAI**
- https://media.x.ai/v1/website/card-4p6-4cd2dc57.pdf — Grok 4.6 Model Card (2026-08-12, rev 2026-08-17)
- https://docs.x.ai/developers/models/grok-4.6
- https://docs.x.ai/developers/grok-4-6
- https://docs.x.ai/developers/model-capabilities/text/reasoning
- https://docs.x.ai/build/overview
- https://x.ai/news/grok-4-6 (2026-08-12)
- https://x.ai/news/grok-build-cli (2026-05-25)
- https://docs.x.ai/developers/guides/prompt-engineering — **404, does not exist** (verified 2026-08-28)

All pages accessed 2026-08-28 unless a publication date is given.
