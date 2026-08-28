# Reasoning Effort per Task, per Model — a working ladder for a coding-agent team

**Compiled 2026-08-28.** Read-only research; no repo files touched.
Scope, as our harnesses run them:

| Harness | Model | Effort surface we actually expose |
|---|---|---|
| Claude Code | `claude-fable-5` | `--effort low\|medium\|high\|xhigh\|max` |
| Claude Code | `claude-opus-5` | `--effort low\|medium\|high\|xhigh\|max` |
| Codex CLI 0.149 | `gpt-5.6-sol` | `-c model_reasoning_effort=low\|medium\|high\|xhigh` |
| Codex CLI 0.149 | `gpt-5.6-luna` | `-c model_reasoning_effort=low\|medium\|high\|xhigh` |
| Antigravity CLI (`agy`) | `gemini-3.7-flash` | `--effort low\|medium\|high` |
| Grok CLI (`grok`) | `grok-4.6` | `--effort low\|medium\|high\|xhigh` |

Out of scope by instruction: Sonnet (any), `gpt-5.6-terra`.

## Source tiers used in this document

- **[V]** Vendor primary doc, fetched 2026-08-28, URL + doc date given.
- **[R]** Peer-reviewed / preprint research with a stated method.
- **[3P]** Third-party measurement or reporting. Directionally useful, not authoritative.
- **[INFERRED]** My synthesis. Not stated by any source. Every one of these is listed again in the closing "unsourced" block.

---

## 0. Four facts that constrain everything below

**0.1 The level names do not mean the same thing across vendors — or even across models from one vendor.** Anthropic states this outright: *"The effort scale is calibrated per model, so the same level name does not represent the same underlying value across models."* **[V]** ([Claude Code model config](https://code.claude.com/docs/en/model-config), fetched 2026-08-28). So "run everything at high" is not a policy, it is four unrelated policies. Anthropic's `high` is its **default**; OpenAI's `medium` is its **default**; xAI's `high` is its **default**; Google's `medium` is its **default**. A cross-model comparison at a fixed level name systematically mis-ranks.

**0.2 Anthropic's effort is the only one that is documented to move tool-call *breadth*, not just thinking depth.** *"It can affect all token spend including tool calls. For example, lower effort would mean Claude makes fewer tool calls."* Lower effort tends to *"Combine multiple operations into fewer tool calls / Make fewer tool calls / Proceed directly to action without preamble"*; higher effort *"Make more tool calls / Explain the plan before taking action / Provide detailed summaries of changes / Include more comprehensive code comments."* **[V]** ([Effort](https://platform.claude.com/docs/en/build-with-claude/effort)). OpenAI, xAI and Google describe their parameter as reasoning-token depth; Google adds that `high` *"Allows extended thoughts and function calls"* **[V]**, which is the closest analogue.

**0.3 Nobody publishes an adjacent-level quality curve on an agentic coding benchmark.** I searched vendor docs, announcements and model cards for all four. OpenAI's `gpt-5.6-sol` / `gpt-5.6-luna` model pages carry *no* per-effort benchmark table **[V]**; xAI's reasoning page states no deltas **[V]**; Google's 3.7 Flash launch post reports no thinking-level breakdown **[V]**; Anthropic reports headline scores without naming the effort level. The two usable numeric sources are one preprint **[R]** and one third-party leaderboard read **[3P]**, both cited below. **Treat every "marginal value of a step up" number in this report as weak evidence.**

**0.4 Effort changes invalidate prompt caching on Anthropic.** *"Because effort shapes the rendered prompt, changing it between requests does not preserve cached prefixes from earlier turns; if you rely on prompt caching across a long session, pick an effort level at the start and keep it constant."* **[V]** ([Effort](https://platform.claude.com/docs/en/build-with-claude/effort)). Practical consequence for a team runtime: **set effort per member at launch, not per turn.** For a long-lived lead agent this is the difference between paying cache-read and cache-write rates for the whole session.

---

## 1. Claude Fable 5 (Claude Code)

Specs: 1M context, 128k max output, $10/$50 per M tokens, GA 2026-06-09. Adaptive thinking is always on and **cannot be disabled**. **[V]** ([Introducing Fable 5 / Mythos 5](https://platform.claude.com/docs/en/models/fable-5/introducing-claude-fable-5-and-claude-mythos-5))

### 1.1 What each level actually changes

Effort on Claude is *"a behavioral signal, not a strict token budget"* that *"affects all tokens in the response, including: Text responses and explanations; Tool calls and function arguments; Thinking."* **[V]**

| Level | Vendor's own description | Vendor's typical use case |
|---|---|---|
| `low` | "Most efficient. Significant token savings with some capability reduction." | "Simpler tasks that need the best speed and lowest costs, **such as subagents**" |
| `medium` | "Balanced approach with moderate token savings." | "Agentic tasks that require a balance of speed, cost, and performance" |
| `high` (default) | "High capability. Equivalent to not setting the parameter." | "Complex reasoning, difficult coding problems, agentic tasks" |
| `xhigh` | "Extended capability for long-horizon work." | "**Long-running agentic and coding tasks (over 30 minutes) with token budgets in the millions**" |
| `max` | "Absolute maximum capability with no constraints on token spending." | "Tasks requiring the deepest possible reasoning and most thorough analysis" |

**[V]** ([Effort](https://platform.claude.com/docs/en/build-with-claude/effort)) — note that `xhigh`'s definition is scoped by *wall-clock horizon* ("over 30 minutes") and *budget* ("millions"), not by difficulty. That is a sharper selection rule than "hard task" and I use it below.

Claude Code layer: default is `high`; `low/medium/high/xhigh` persist across sessions, `max` is session-only unless set via `CLAUDE_CODE_EFFORT_LEVEL`. **[V]** Crucially for a team runtime: **`effort` is a supported frontmatter field on skills and subagents**, and *"Frontmatter effort applies when that skill or subagent is active, overriding the session level but not the environment variable."* **[V]** ([model config](https://code.claude.com/docs/en/model-config)). Precedence: env var > configured level > model default.

One Claude-Code-specific trap for a fleet: *"When you first run Fable 5, Opus 4.8, or Opus 4.7, Claude Code applies that model's default effort even if you previously set a different level for another model, and holds it across sessions until you make an explicit effort choice."* A non-interactive `/effort` cannot release that hold — *"pass `--effort` at launch instead."* **[V]** For taurhaus's launch path this means **always render an explicit `--effort` flag for Fable 5 members**; relying on a persisted preference will silently be overridden.

Cost/latency: no published multipliers. Anthropic's only quantitative hint is the `max_tokens` sizing guidance — at `xhigh`/`max`, *"Starting at 64k tokens and tuning from there is a reasonable default"* **[V]** — and the warning that *"Individual requests on hard tasks can run for many minutes at higher effort settings"* **[V]** ([Prompting Fable 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5)).

### 1.2 Vendor recommendation, quoted

> "Effort is the primary control for trading off intelligence, latency, and cost on Claude Fable 5. **Start with `high`, the default, for most tasks**, use `xhigh` for the most capability-sensitive workloads, and step down to `medium` or `low` for routine work. Lower effort settings on Claude Fable 5 still perform well and often exceed `xhigh` performance on prior models."
> — **[V]** [Effort](https://platform.claude.com/docs/en/build-with-claude/effort)

> "Reduce effort if a task completes but takes longer than necessary, or if you want a faster, more interactive working style."
> — **[V]** same page

And from the Claude Code effort table **[V]**:
- `low` — "Reserve for short, scoped, latency-sensitive tasks that are not intelligence-sensitive"
- `medium` — "Reduces token usage for cost-sensitive work that can trade off some intelligence"
- `high` — "Balances token usage and intelligence"
- `xhigh` — "Deeper reasoning at higher token spend"
- `max` — "Can improve performance on demanding tasks but may show diminishing returns and is **prone to overthinking**. Test before adopting broadly"

Anthropic's Claude Code blog adds the single most operational rule I found anywhere: *"For most tasks you should use the model's default effort level. Pick a higher effort level if Claude got it wrong by skipping a file, not running the tests, or not double-checking its work."* **[V]** ([Choosing a Claude model and effort level in Claude Code](https://claude.com/blog/claude-model-and-effort-level-in-claude-code), 2026-07-07). That is a *diagnostic* trigger, not a task-class trigger — raise effort in response to a specific observed failure mode (skipped file, skipped test, skipped check), not in anticipation.

### 1.3 Published quality deltas between adjacent levels

**None from Anthropic.** Headline scores (95.0% SWE-bench Verified, 80.3% SWE-bench Pro, 88.0% Terminal-Bench 2.1) are reported **[3P]** ([Vellum](https://www.vellum.ai/blog/claude-fable-5-and-mythos-5-benchmarks-explained), [morphllm](https://www.morphllm.com/claude-benchmarks)) without an effort label. The one published cross-vendor tiered read has Fable 5 at `max` scoring 70.5% on CursorBench v3.2 **[3P]** ([digitalapplied, 2026-08-12](https://www.digitalapplied.com/blog/grok-4-6-vs-gpt-5-6-sol-opus-5-fable-5-effort-tiers-2026)) — a single point, no adjacent level.

The best proxy is **[R]** arXiv:2607.02436 (Mehta, submitted 2026-07-02): 90 independent agent runs building the same app from one detailed spec, 14-criterion rubric, spanning several model generations, two harnesses, two effort levels. *"Raising reasoning effort from High to xHigh lifted first try perfect runs from 28 percent to 89 percent and cut corrective prompts about five fold, for 9 to 29 percent more cost."* The paper's own conclusion: *"most first run failures came from weak reasoning, which a stronger model or more effort prevents, not from visible flaws a checking tool would catch."* Same study found a browser-testing tool *"raised cost by 42 to 68 percent without improving functional score or reliability."*

**Read this carefully before you generalize it.** The task was *greenfield implementation from a detailed spec* — precisely one of our task classes — and the effect size on *first-try* reliability is enormous relative to the cost delta. It is one paper, one app, and the abstract does not attribute the High→xHigh cell to a named model. It is nonetheless the strongest single argument in this report for putting **spec implementation at xhigh**.

### 1.4 Pathologies at the extremes

**At high effort** (vendor-stated **[V]**, [Prompting Fable 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5)):
> "On routine work at higher effort, Claude Fable 5 can gather context and deliberate beyond what the task needs. At the same time, higher effort often produces excellent verification behavior, sophisticated reasoning, and the most rigorous output."

Concretely, unrequested tidying/refactoring, premature abstraction, defensive error handling for impossible cases, backwards-compat shims. Anthropic ships a mitigation prompt block verbatim (*"Don't add features, refactor, or introduce abstractions beyond what the task requires…"*). Also: **overplanning under ambiguity** — the *"When you have enough information to act, act"* block exists specifically for this.

**At `max`:** *"prone to overthinking"* **[V]**; and from the sibling Opus 4.7/4.8 guidance which Anthropic says also applies: *"On most workloads `max` adds significant cost for relatively small quality gains, and on some structured-output or less intelligence-sensitive tasks it can lead to overthinking."* **[V]**

**Other Fable-5-specific failure modes that interact with long high-effort runs** **[V]**:
- **Early stopping** deep in a long session — ends a turn with "I'll now run X" and no tool call.
- **Context-budget anxiety** — offers to hand off / start a new session, *"most often triggered when the harness shows a remaining-token countdown to the model."* Directly relevant to taurhaus: **do not surface remaining-context counters to Fable 5 members.**
- **Safety classifiers** decline offensive-security and biology-adjacent work with `stop_reason: "refusal"`. Claude Code routes a flagged Fable 5 session to Opus 5. **[V]** ([model config](https://code.claude.com/docs/en/model-config)). If a team member's lane is security review of exploit-shaped code, Fable 5 is the wrong assignment regardless of effort.

**At low effort:** no Fable-5-specific statement. The nearest vendor statement is on Opus 4.7 (which Anthropic says carries forward to 4.8): *"At lower effort levels, the model scopes its work to what was asked rather than doing more than requested… If you observe shallow reasoning on complex problems, raise effort rather than prompting around it."* **[V]**

### 1.5 Effort × size of the instruction set

Vendor evidence, two directions:

**Lean role + low effort is the dangerous combination.** *"`low` — Efficient, but best for short, scoped tasks. **Pair `low` with explicit checklists if your task has multiple sections.**"* and *"If you must keep effort low for latency, add targeted guidance like 'This task involves multistep reasoning. Think carefully before responding.'"* **[V]** (Opus 4.7 table; Anthropic states the guidance applies to 4.8, and the ladder shape is unchanged on 5-series). **Low effort demands *more* explicit structure, not less.**

**Heavy role + high effort is the other dangerous combination.** For Fable 5: *"Skills developed for prior models are often too prescriptive for Claude Fable 5 and can degrade output quality. Review and consider removing older instructions if default performance is better."* And: *"Instruction-following is improved enough that you can steer most behaviors with a brief instruction rather than enumerating each behavior by name."* **[V]**

**Synthesis [INFERRED]:** instruction volume and effort are **substitutes at the bottom of the ladder and compounding liabilities at the top**. A lean role at `low` under-plans; the fix is a checklist, not more effort. A prescriptive role at `xhigh`/`max` gold-plates, because the model's own thoroughness and the prompt's enumerated thoroughness stack. So: as you raise effort, **shorten** the role definition and shift it from *procedure* to *outcome + boundary*; as you lower effort, **lengthen** it into an explicit sectioned checklist. This is a synthesis of the two vendor statements above, not a vendor claim.

### 1.6 Recommended starting level per task class — Fable 5

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **xhigh** | Medium-high | arXiv 2607.02436 High→xHigh 28%→89% first-try-perfect for +9–29% cost **[R]**; `xhigh` = "long-running agentic and coding tasks (over 30 minutes)" **[V]** |
| Review with a checklist | **high**, drop to `medium` once calibrated | Medium | Checklist supplies the structure effort would otherwise buy **[V]**; Anthropic's sibling finding that review accuracy holds at lower effort is stated for Opus 5, not Fable 5 — hence "calibrate" |
| Docs verification (claims ↔ code) | **medium** | Medium | Bounded, verifiable, many small tool calls; `medium` = "balance of speed, cost, and performance" **[V]** |
| Mechanical sweeps (rename, codemod, lint fix) | **low** | High | "Reserve for short, scoped, latency-sensitive tasks that are not intelligence-sensitive" **[V]**; pair with an explicit checklist per §1.5 |
| Architecture decisions | **xhigh** (not `max`) | Medium | "deepest possible reasoning" is `max`'s slot, but `max` is "prone to overthinking… on some structured-output tasks" **[V]**; an ADR is structured output |
| Coordination / team lead | **high**, held constant | Medium-high | Long-lived session ⇒ cache stability matters (§0.4) **[V]**; Fable 5 *"dispatches parallel subagents more readily"* and manages long-running peers well at default **[V]** |

Fable 5 is expensive ($10/$50 per M **[V]**). Reserve it for the classes where its long-horizon autonomy is the point (spec implementation, lead) rather than running the whole roster on it.

---

## 2. Claude Opus 5 (Claude Code)

Specs: 1M context (default *and* maximum), 128k output, **$5/$25 per M** — *"frontier intelligence at half the cost of Claude Fable 5."* Thinking on by default. **[V]** ([What's new in Opus 5](https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5))

### 2.1 What each level changes

Same five-level ladder and same all-token semantics as §1.1. Two Opus-5-specific behaviors:

- **Effort no longer controls response length.** *"Effort controls thinking volume, not visible response length: on Claude Opus 5, changing effort does not reliably shorten responses, so prompt for length instead."* **[V]** ([Effort](https://platform.claude.com/docs/en/build-with-claude/effort)). If you want terser member output, that is a prompt change, not an effort change.
- **`xhigh`/`max` force thinking on.** `thinking: {"type": "disabled"}` at `xhigh`/`max` returns HTTP 400. **[V]**

And the headline claim, which is the reason Opus 5 gets different advice from every other model here:

> "**Effort matters more.** Claude Opus 5 converts additional effort into better results more reliably than any earlier Opus model, so the effort level you choose carries more weight."
> — **[V]** [What's new in Opus 5](https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5)

Paired with the opposite-direction claim on the same page: *"**Efficiency at lower effort levels**, with `low` and `medium` effort producing strong quality at a fraction of the tokens and latency of higher settings."* **[V]** Opus 5 is described as having a **steeper and more usable effort curve in both directions** than its predecessors — the most eval-worthy model in this set.

### 2.2 Vendor recommendation, quoted

> "Claude Opus 5 supports all five effort levels. **Start with `high`, the default**, and adjust based on your evals: step up to `xhigh` for demanding coding and agentic work, or to `max` when a task justifies unconstrained token spending, and **use `low` and `medium` liberally as your primary control for token cost and response time wherever your evals show quality holds**. If you carried effort settings over from an earlier model, run a fresh effort sweep on your evals rather than reusing them."
> — **[V]** [Effort](https://platform.claude.com/docs/en/build-with-claude/effort)

On code review specifically — the single most useful sentence in this report for a review-lane role:

> "Code review and bug-finding: Claude Opus 5 reviews code with high precision and recall… **Accuracy holds at lower effort settings, which supports a fast pass at review time and a more thorough pass later.** If your review prompt says 'only report high-severity issues' or 'be conservative,' the model may follow that instruction literally and report less; ask it to report everything and filter in a separate pass instead."
> — **[V]** [Prompting Claude Opus 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5)

On spec implementation:

> "Claude Opus 5 is strongest on difficult coding tasks: multi-file features, larger refactors, and end-to-end feature work. It completes full tasks rather than leaving stubs or placeholders, and **it performs best when given the complete task specification up front and left to run.**"
> — **[V]** same page

### 2.3 Published quality deltas between adjacent levels

None from Anthropic. **[3P]** digitalapplied places "Opus 5 max: 63.05" on the AA Intelligence Index with no adjacent rung. The vendor's qualitative claim (§2.1, "converts additional effort into better results more reliably than any earlier Opus model") is the only directional statement, and it is unquantified.

### 2.4 Pathologies at the extremes

Opus 5 has the **most explicitly documented over-verification pathology** of any model here **[V]**:

> "Claude Opus 5 verifies its own work without being told to. If your prompt contains explicit verification instructions ('include a final verification step for any non-trivial task,' 'use a subagent to verify'), **remove them: instructions like these cause over-verification on Claude Opus 5, and removing them reduces wasted tokens with no loss in quality.** The same applies to legacy harness scaffolding that adds separate verification steps."

And self-correction: *"Avoid instructing re-checks it already performs ('double-check your answer,' 're-verify before responding'); like verification instructions, these compound with the model's own behavior and add cost without improving results."* **[V]**

Also documented **[V]**:
- **Scope expansion** — "can also expand the scope of a task, adding steps that weren't requested or applying its own judgment about what the task should be."
- **Over-delegation** — "delegates to subagents more readily than prior models… it multiplies cost and time when applied to small tasks." Mitigate with `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH` / `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS` (requires Claude Code ≥ 2.1.217).
- **Verbosity up** — default responses *and files written to disk* run longer than prior Opus models.
- **Thinking-disabled artifacts** — tool calls leaking into text, `<thinking>` tags in output. Vendor's fix: *"for most tasks, thinking enabled at `low` effort performs better than thinking disabled at similar cost."* **[V]** Never disable thinking to save money; drop effort instead.

At `max`: the general Anthropic caution applies (*"prone to overthinking"*, *"diminishing returns"* **[V]**).

### 2.5 Effort × size of the instruction set

Opus 5 gives the clearest vendor-stated case of **instruction × effort compounding**: verification instructions + a model that already verifies = wasted tokens with *"no loss in quality"* from removing them **[V]**. The direction is the same as §1.5 but the evidence is stronger and it is stated for this exact model.

Second, an inverse case worth noting for checklist roles: a *restrictive* instruction ("only report high-severity issues") is followed **literally** and suppresses recall **[V]**. So for a checklist review lane, the checklist should define *coverage*, and filtering should be a separate pass — not a constraint inside the same prompt.

**[INFERRED]** For Opus 5 specifically, the CLAUDE.md-plus-role-template stack that taurhaus assembles is a live over-verification risk: the repo's own instructions include "AC-driven coverage — every acceptance criterion gets a test", "`just check-quick` is the per-task gate", and "visual dual review". Each is reasonable; stacked under `high`+ effort on a model that self-verifies by default, they are exactly the pattern Anthropic says to remove. Worth an eval before assuming it's free.

### 2.6 Recommended starting level per task class — Opus 5

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **high**, step to `xhigh` for multi-file/end-to-end | Medium-high | "performs best when given the complete task specification up front and left to run" **[V]**; "step up to `xhigh` for demanding coding and agentic work" **[V]** |
| Review with a checklist | **medium** | **High** | "Accuracy holds at lower effort settings, which supports a fast pass at review time and a more thorough pass later" **[V]** — the only vendor statement in this whole report that directly licenses a step *down* for a named task class |
| Docs verification | **low** | Medium | Bounded verification, "use `low` and `medium` liberally as your primary control" **[V]** |
| Mechanical sweeps | **low** | High | Same **[V]**; also cheapest way to keep a sweep from turning into a refactor (§2.4 scope expansion) |
| Architecture decisions | **xhigh** | Medium | "step up to `xhigh` for demanding… work"; "Deep reasoning, sustaining multistep analysis across long problem chains" is Opus 5's top listed gain **[V]** |
| Coordination / team lead | **high**, held constant, **with a delegation cap** | Medium-high | "coordinates teams of subagents well, with effective writer-verifier patterns" **[V]**; but "delegates more readily" ⇒ cap depth/concurrency **[V]** |

**Opus 5 is the default workhorse of this roster.** Half of Fable 5's price, an explicitly steeper effort curve in both directions, and the only model with a vendor-blessed *low-effort review* lane.

---

## 3. gpt-5.6-sol (Codex CLI 0.149)

Specs: 1,050,000 context / 922k max input / 128k output, knowledge cutoff 2026-02-16, **$4/$20 per M** ($0.40 cached input). Requests over 272K input tokens incur **2× input and 1.5× output multipliers for the whole request**. **[V]** ([gpt-5.6-sol model page](https://developers.openai.com/api/docs/models/gpt-5.6-sol))

That 272K cliff is a real budget hazard for a long-running lead agent on a 1M-token window and has nothing to do with effort — but it interacts, because higher effort produces more tokens that then get fed back into the next request's input.

### 3.1 What each level changes

API surface: `none`, `low`, `medium` (**default**), `high`, `xhigh`, `max`. **[V]** Our Codex CLI 0.149 surface exposes `low|medium|high|xhigh` — i.e. **we cannot reach `max` or `none` through `-c model_reasoning_effort=`** as configured. Semantics: *"lower effort favors speed and lower token usage, while at higher effort the model thinks more completely to provide higher quality responses,"* and models *"reason adaptively across reasoning efforts, using fewer tokens for simpler tasks and thinking harder for complex tasks."* **[V]** ([Reasoning guide](https://developers.openai.com/api/docs/guides/reasoning))

No published token/latency multipliers from OpenAI. A third-party Codex knowledge base gives community estimates relative to `medium` — `minimal` ~0.1×, `low` ~0.3×, `high` ~3–5×, `xhigh` ~8–15× reasoning tokens — explicitly flagged there as *"approximate based on community benchmarks"* and undocumented by the provider. **[3P]** ([Reasoning Effort Tuning](https://codex.danielvaughan.com/2026/03/27/reasoning-effort-tuning/), 2026-03-27, updated 2026-08-21). Use only for order-of-magnitude budgeting.

Codex CLI layer: `model_reasoning_effort` in `~/.codex/config.toml`, default `medium`; a separate `plan_mode_reasoning_effort` defaults to `high` when unset **[3P]** same source. Codex's own UI names the rungs Light/Low, Medium, High, Extra High, Max, plus an **Ultra** mode that *"goes beyond a single-agent run"* by delegating to subagents **[V]** ([Codex models](https://learn.chatgpt.com/docs/models)).

### 3.2 Vendor recommendation, quoted

Per-level, from the reasoning guide **[V]**:
- `none` — "Latency-critical tasks that do not benefit from any reasoning or multi-chained tool calls"
- `low` — "ideal for use cases requiring tool-use, planning, search, or multi-step decision making, while optimizing for speed and cost"
- `medium` — "Default configuration for most workloads"; "a well-balanced point on the pareto curve"; named for agentic coding, research, spreadsheets
- `high` — "Hard reasoning, complex debugging, deep planning"
- `xhigh` — "Deep research, asynchronous workflows and agentic tasks"; named for security review
- `max` — "Maximum reasoning for your most complex tasks"

From the Codex prompting guide **[V]** ([Codex prompting guide](https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide)):
> "We recommend 'medium' reasoning effort as a good all-around interactive coding model that balances intelligence and speed."

…with `high`/`xhigh` reserved for *"your hardest tasks."*

Two more, which together form OpenAI's actual policy:
> "**Use the lowest reasoning effort that produces the result you need.**" — **[V]** [Codex models](https://learn.chatgpt.com/docs/models)

> "If you are migrating from GPT-5.5 or GPT-5.4, preserve your current reasoning effort as the baseline, then compare one level lower… GPT-5.6 can often maintain or improve quality with fewer tokens." — **[V]** [Model guidance](https://developers.openai.com/api/docs/guides/latest-model)

> "start with the same model and effort as your standard-mode baseline, then compare configurations on representative tasks **instead of assuming that the highest effort is always the best tradeoff.**" — **[V]** same page

Model selection: Sol is for *"ambiguous, difficult, or high-value tasks"* like complex code changes and deep research. **[V]**

Note the **direction of OpenAI's advice is downward** (start at default, test one level lower) where Anthropic's Fable/Opus-4.x advice was upward (start at xhigh for coding). This is not a contradiction about reality; it reflects different defaults — Anthropic's default is `high` of five, OpenAI's is `medium` of six.

### 3.3 Published quality deltas between adjacent levels

**None from OpenAI.** The Sol and Luna model pages carry no per-effort tables **[V]**. Third-party reporting says OpenAI *"changed their reporting format: instead of publishing a single benchmark score, they now show performance as a curve across reasoning-effort levels"* **[3P]** ([Vellum, 2026-07-09](https://www.vellum.ai/blog/gpt-5-6-benchmarks-explained)) — but neither that article nor Artificial Analysis reproduces the per-rung numbers, and I could not fetch openai.com/index/gpt-5-6/ (HTTP 403). **If you want Sol's effort curve, the OpenAI launch page is the one primary source worth a manual look.**

What is published, all at `max` **[3P]** ([Artificial Analysis, 2026-07-09](https://artificialanalysis.ai/articles/gpt-5-6-has-landed)): Sol (max) AA Intelligence Index 59, Coding Agent Index 80, **$1.04 cost/task**, ~15k output tokens per Intelligence Index task; Terminal-Bench 2.1 88.80, DeepSWE 72.70.

### 3.4 Pathologies at the extremes

**At high effort**, the best-documented Codex-specific failure is **premature stopping caused by planning/preamble instructions** **[V]**:
> remove "prompting for the model to communicate an upfront plan, preambles, or other status updates during the rollout" — this can cause premature stopping; avoid instructions requesting intermediate summaries that might interrupt longer reasoning chains.

**At low effort**, the guide's under-planning boundary: for *"roughly the easiest 25%"* of tasks, skip planning entirely; and avoid *"single-step plans"* — effort should correlate with planning detail **[V]**.

**Over-engineering at high effort on constrained refactors** is reported **[3P]** but only for a prior generation: in a 900-run study across 5 models × 3 tiers (2026-04-23), GPT-5.5 Pro *regressed* medium→high on an Expert-SWE refactor suite (73.1% → 71.4%), and *"On 23% of Expert-SWE high-effort runs, models produced over-engineered refactors with broken type signatures and unnecessary abstractions."* ([digitalapplied](https://www.digitalapplied.com/blog/reasoning-effort-cost-vs-quality-benchmarks-2026)). Models tested were GPT-5.5 Pro / Opus 4.7 / Gemini 3 Pro DT / Grok 4.5 / DeepSeek V4 — **all one generation behind our roster**, and the harness is self-described as internal. Do not treat the −1.7pt number as applying to Sol.

**Note the tension** between that finding and arXiv 2607.02436 (§1.3), which found a *large* High→xHigh gain. They are consistent if the task shape is what matters: **greenfield build-from-spec rewards more reasoning; a bounded refactor with a fixed external contract punishes it.** That is my reading **[INFERRED]**, and it is the sharpest testable hypothesis in this report.

### 3.5 Effort × size of the instruction set

OpenAI's public position for the 5.6 generation is that these models want **fewer, better instructions**, and that prompt quality should be exhausted before effort is raised. The specific formulations circulating — *"a better prompt at medium often beats a lazy prompt at max"*, *"the reasoning slider is a compute control, not a truth control"*, and a claim that stating each instruction exactly once *"raises scores 10 to 15% while cutting tokens up to 66%"* — appear in third-party guides attributed to OpenAI **[3P]** ([techtimes](https://www.techtimes.com/articles/320650/20260715/gpt-56-prompting-guide-lean-system-prompts-now-outperform-elaborate-scaffolding.htm), [thepromptindex](https://www.thepromptindex.com/gpt-5-6-and-claude-fable-5-prompting-guide.html)). **I could not verify the 10–15% / 66% figure against any OpenAI-hosted page** and it should be treated as unverified.

The verifiable primary-source part **[V]**: the Codex guide tells you to *remove* plan/preamble/status instructions at higher effort because they truncate reasoning chains, and to scale planning detail with effort. That is the same substitutive relationship as §1.5, expressed as "don't instruct the process the effort level is already buying."

**[INFERRED]** For Sol in a taurhaus role template: keep the role's *outcome contract* (focus area, quality gates, definition of done, required artifacts) and drop any *procedural narration* fields (upfront-plan instructions, "report progress at each step") when running above `medium`.

### 3.6 Recommended starting level per task class — gpt-5.6-sol

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **high** | Medium | "Hard reasoning, complex debugging, deep planning" **[V]**; step to `xhigh` only if the spec is long-horizon/async — `xhigh` is scoped to "asynchronous workflows and agentic tasks" **[V]** |
| Review with a checklist | **medium**; `xhigh` for a security-shaped review | Medium | `medium` = "Default configuration for most workloads" **[V]**; OpenAI names **security review** under `xhigh` **[V]** |
| Docs verification | **low** | Medium-high | `low` is explicitly "ideal for use cases requiring tool-use, planning, search, or multi-step decision making, while optimizing for speed and cost" **[V]** — a near-exact description of docs↔code verification |
| Mechanical sweeps | **low** | High | Same **[V]**; plus "use the lowest reasoning effort that produces the result you need" **[V]** |
| Architecture decisions | **xhigh** | Medium | "Deep research, asynchronous workflows"; "Hard reasoning… deep planning" **[V]** |
| Coordination / team lead | **high** | Medium | Plan-mode's own default is `high` **[3P]**; lead work is planning + routing, which OpenAI puts at `high` **[V]** |

Sol's `low` rung is unusually strong on paper — OpenAI describes it as a *tool-use and multi-step-decision* tier, not a "simple task" tier. That is a meaningfully different design point from Anthropic's `low` ("simpler tasks… such as subagents") and it makes Sol the best candidate on this roster for a **cheap, high-tool-call verification lane**.

---

## 4. gpt-5.6-luna (Codex CLI 0.149)

Specs: same six-rung ladder, **default `medium`**, **$0.20/$1.20 per M** ($0.02 cached input) — roughly **1/20th of Sol's input price and 1/17th of its output price**. **[V]** ([gpt-5.6-luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna)). Same 272K input-token repricing multiplier.

### 4.1 What each level changes

Identical parameter semantics to Sol (§3.1); OpenAI's docs *"[do] not differentiate reasoning effort recommendations between sol, terra, and luna variants"* **[V]** ([Model guidance](https://developers.openai.com/api/docs/guides/latest-model)). **This is the gap the user's question is pointing at:** the vendor gives one ladder for three models with very different capability ceilings, so the per-model effort policy has to come from measurement.

### 4.2 Vendor recommendation, quoted

Model-level, not effort-level **[V]** ([Codex models](https://learn.chatgpt.com/docs/models)):
> Luna targets "specific, high-volume tasks when you know what a good result looks like."

That framing — *you know what a good result looks like* — is the selection rule. Luna is for task classes where the acceptance criterion is external and checkable, not where judgment is the deliverable.

Effort guidance is inherited from the shared GPT-5.6 text: default `medium`, *"use the lowest reasoning effort that produces the result you need"*, and the migration rule to test one level lower. **[V]**

### 4.3 Published quality deltas between adjacent levels

None from OpenAI. **[3P]** at `max`: AA Intelligence Index 51, Coding Agent Index 75, **$0.21 cost/task** — about 80% below Sol's cost/task ([Artificial Analysis, 2026-07-09](https://artificialanalysis.ai/articles/gpt-5-6-has-landed)). Note Luna at `max` still scores 75 on the Coding Agent Index against Sol's 80 — a 5-point gap for a 5× cost gap, which is why Luna is worth a lane at all.

### 4.4 Pathologies at the extremes — including the "cliff"

Two distinct phenomena get conflated as "Luna's cliff." They are not the same thing and only one of them is well-sourced:

**(a) The long-context recall cliff — well sourced, not an effort effect.** Luna scores **41.3%** on MRCR long-context recall against Sol's 91.5% and Terra's 89.6% **[3P]** ([Vellum, 2026-07-09](https://www.vellum.ai/blog/gpt-5-6-benchmarks-explained)), which that article explicitly labels *"a cliff."* Consequence: *"If your workload involves long-context recall (document analysis, large codebase reasoning, multi-document synthesis), Luna is the wrong tool."* **No effort level fixes this.** For a taurhaus member whose lane is "read the whole subsystem and reconcile it," Luna is disqualified on capability, not on effort.

**(b) Effort saturation above `medium`/`high` — reported, weakly sourced.** *"Luna fails to benefit from extra effort — Luna `xhigh` is essentially flat with `high`"* **[3P]**, from a single practitioner blog surfaced in search; I could not fetch the article directly (404) and found no corroborating measurement. Related practitioner reports on the same source: *"instruction drift when the brief is a procedure rather than an outcome; a wall-clock slow cold start at max effort that makes interactive debugging feel broken; and a model that Codex's native subagent system filters out entirely."* **Treat all of (b) as unverified.** It is, however, the exact hypothesis your evals should test first, because if true it means Luna's usable range is `low`–`medium` and everything above is pure cost.

**[INFERRED]** The general shape behind (b): a smaller model's effort curve saturates earlier than a frontier model's, because the ceiling is capability, not deliberation. Spending `xhigh` on Luna buys tokens against a wall. This is a plausible mechanism, not a measured result.

### 4.5 Effort × size of the instruction set

The one practitioner-reported Luna-specific interaction is *"instruction drift when the brief is a procedure rather than an outcome"* **[3P]**, which inverts the Sol/Fable guidance: where frontier models over-comply with procedural instructions, a smaller model reportedly **drifts off them**. **[INFERRED]** If that holds, Luna wants a *short outcome contract plus a verifiable acceptance check* rather than either a long procedure or a bare goal — and it wants that check enforced outside the model (a test, a lint gate, a diff review), because Luna's own verification is the thing you are not paying for.

### 4.6 Recommended starting level per task class — gpt-5.6-luna

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **medium** — and only for *small, single-file, fully-specified* work | Low-medium | "specific, high-volume tasks when you know what a good result looks like" **[V]**; Coding Agent Index 75 at max vs Sol's 80 **[3P]** — the gap is in the hard tail |
| Review with a checklist | **medium** | Medium | Checklist supplies structure; external acceptance criterion matches Luna's stated design point **[V]** |
| Docs verification | **low** | Medium | Cheapest possible lane; `low` is a tool-use tier for 5.6 **[V]**. **Caveat:** if verification spans a large corpus, the MRCR cliff **[3P]** disqualifies Luna regardless of effort |
| Mechanical sweeps | **low** | **High** | This is Luna's best fit on the roster: bounded, verifiable, high volume, at ~1/17th Sol's output price **[V]** |
| Architecture decisions | **do not assign** | High | Judgment is the deliverable; "you know what a good result looks like" fails **[V]**; capability gap concentrates in the hard tail **[3P]** |
| Coordination / team lead | **do not assign** | High | Lead work is long-context synthesis; MRCR 41.3% **[3P]**. Also reportedly filtered out of Codex's own subagent system **[3P]**, unverified |

**Do not step Luna above `high`.** Until the saturation claim is measured, the cost of being wrong at `xhigh` is real and the upside is unevidenced.

---

## 5. gemini-3.7-flash (Antigravity CLI)

Specs: **$0.75/$3.75 per M introductory through 2026-12-31, then $1.50/$7.50**. Launched 2026-08-13. **[V]** ([blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/))

### 5.1 What each level changes

The API parameter is `thinking_level` (which replaced `thinking_budget`), and it *"controls the maximum depth of the model's internal reasoning process before it produces a response."* For 3.7 Flash the supported values are **LOW, MEDIUM (default), HIGH** — and notably *"`thinking_level="MINIMAL"` is not available for 3.7 Flash, and explicitly setting it to MINIMAL will return an API validation error."* **[V]** ([What's new in Gemini 3.7 Flash](https://ai.google.dev/gemini-api/docs/latest-model), [Gemini thinking](https://ai.google.dev/gemini-api/docs/thinking))

Gemini 3-series models *"use dynamic thinking by default"* and *"automatically adjust the amount of reasoning effort based on the complexity of the request"* **[V]** — so `thinking_level` is a ceiling on an adaptive process, closest in spirit to Anthropic's "behavioral signal."

`high` is the only level Google describes as changing **tool behavior**: it *"Maximizes the model's ability to think and use tools… Allows extended thoughts and function calls, with higher token consumption and cost."* **[V]**

**Harness layer — and an important discrepancy.** The `agy` CLI exposes `--effort low|medium|high` plus a `/effort <level>` slash command; the CHANGELOG records `/effort` and `--effort` arriving in 1.1.5 (with a left/right timeline-gauge picker), and 1.1.2 fixing *"`--model` and `--effort` being ignored in interactive sessions and in headless `-p` runs, where the flags were applied after model configuration had already been initialized"* and *"a bare `--effort` resolving against the default model instead of the model you actually have selected."* **[V]** ([antigravity-cli CHANGELOG](https://github.com/google-antigravity/antigravity-cli/blob/main/CHANGELOG.md)). Antigravity's own model docs describe the 3.7 Flash rungs as **Fast (low) / Medium (default) / High** **[V]** ([Antigravity models](https://antigravity.google/docs/models/)).

Meanwhile the **Antigravity *agent API*** — a different surface from the CLI — supports **no `thinking_level` parameter at all**, and gates work with `max_total_tokens` instead (recommended starting budget 50,000; interaction returns `status: "incomplete"` when exhausted). **[V]** ([Antigravity agent](https://ai.google.dev/gemini-api/docs/antigravity-agent)). **[INFERRED]** The CLI's `--effort` therefore most likely maps onto the model's `thinking_level` rather than the agent API's token budget — but I found no Google document that states the mapping, and given the 1.1.2 bug history it is worth verifying empirically that `agy --effort high -p ...` actually changes token spend in your logs.

Given taurhaus's harness registry already knows Antigravity declines a compaction hook and has no account selector, this is one more place where the tool's contract is thinner than the others'.

### 5.2 Vendor recommendation, quoted

From the 3.7 Flash page **[V]**:
- **Low** — "Reduces time-to-answer for latency-critical tasks like incident response pipelines, real-time chat, writing drafts, and fast data analysis."
- **Medium (default)** — "**Best quality for most tasks. Recommended for complex code and agentic use cases, with higher first-pass accuracy.**"
- **High** — "Maximizes the model's ability to think and use tools. Best for complex reasoning, hard math, and the most difficult coding and agent tasks."

From the thinking guide's task mapping **[V]**: Minimal/Low → *"fact retrieval or classification"*; Default/Medium → *"comparing concepts or creative reasoning"*; High → *"advanced coding, math, or multi-step planning."*

Note the unusual shape: **Google recommends its *default* for complex code and agentic use, and reserves `high` for the hardest tail.** Of the four vendors, Google is the most explicit that `medium` is where agentic coding belongs, and it says so with a quality claim (*"higher first-pass accuracy"*), not just a cost claim.

Third-party framing of the CLI lever, useful and consistent: *"Effort is the largest cost and latency lever the CLI exposes. Low effort on mechanical edits is dramatically faster; high effort on a genuinely hard bug is a different tool. Switching per task rather than per account is what makes a fixed weekly limit last."* **[3P]** ([continuumcode, Aug 2026](https://continuumcode.ai/guides/antigravity-cli/)) — that last clause matters for `agy`, which is credit-metered.

### 5.3 Published quality deltas between adjacent levels

**None from Google.** The 3.7 Flash launch post gives model-level numbers only, with no thinking-level breakdown: FrontierCode 1.1 Main **43.6%** (vs 34.4% for 3.6 Flash), DeepSWE v1.1 **65.3%** (vs 49.0%), WebDev Arena Elo **1588** (vs 1538), AutomationBench **30.4%** (vs 17.0%). **[V]** ([blog.google, 2026-08-13](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/))

For orientation: DeepSWE v1.1 65.3% puts 3.7 Flash roughly level with grok-4.6's reported 65.9% **[3P]** and below Sol's 72.70 **[3P]** — a capable agentic coder at Flash pricing, not a frontier one.

### 5.4 Pathologies at the extremes

Google documents **no** pathologies at any thinking level — no overthinking warning, no under-planning warning, no non-monotonicity note. That absence is itself the finding: **[V]** confirmed across the 3.7 Flash page, the thinking guide, the Antigravity models page and the Antigravity CLI best-practices page (which contains no effort guidance at all).

What Antigravity documents instead is **task sizing**, which is the same lever expressed structurally **[3P]** ([Antigravity Lab](https://antigravitylab.net/en/articles/agents/antigravity-background-agent-advanced-production-guide)): background agents are *"most reliable on tasks equivalent to 1–3 hours of human effort"*; longer work should be split into sequentially-run subtasks; for 5-minute tasks the inline agent is more credit-efficient. And Antigravity's own best-practices page pushes an **exploration → planning → execution** partition for complex changes **[V]** ([Antigravity best practices](https://antigravity.google/docs/cli/best-practices/)).

**[INFERRED]** For `agy`, structural decomposition is the primary quality lever and `--effort` is secondary — the opposite weighting from Claude Code, where effort is explicitly *"the primary control."* Given taurhaus already runs `antigravity-ui-specialist` as a design lead with functional requirements and creative freedom (per CLAUDE.md), that role's quality is more sensitive to how you scope the brief than to which of three rungs you pick.

Two operational cautions **[V]**: **thought signatures** must be resent unmodified in stateless conversations (*"you MUST always resend all thought blocks exactly as they were received"*), and built-in tools carry their own signatures. And the 1.1.2 flag-ordering bugs mean **pin your `agy` version and verify the flag lands**.

### 5.5 Effort × size of the instruction set

No vendor statement exists. **[INFERRED]** With only three rungs and a default that Google says is already right for *"complex code and agentic use cases,"* effort is a weak instrument here. The usable levers are (a) the length/precision of the brief, and (b) the exploration/planning/execution split Antigravity documents. Reserve `high` for a specific, named hard problem — a bug that resisted `medium`, or a design task where "use tools extensively" is the point — and treat `low` as strictly a latency/credit play on work you would not review carefully anyway.

### 5.6 Recommended starting level per task class — gemini-3.7-flash

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **medium** | **High** | "Best quality for most tasks. Recommended for complex code and agentic use cases, with higher first-pass accuracy" **[V]** — this is the vendor naming our exact task class at the default |
| Review with a checklist | **medium** | Medium | Same **[V]**; no lower-effort review license exists as it does for Opus 5 |
| Docs verification | **low** | Medium | "fact retrieval or classification" maps to claim-checking **[V]** |
| Mechanical sweeps | **low** | Medium-high | "Low effort on mechanical edits is dramatically faster" **[3P]**; latency/credit tier **[V]** |
| Architecture decisions | **high** | Medium | "complex reasoning… multi-step planning" **[V]** |
| Design-led UI work (taurhaus's actual `agy` lane) | **high** | Medium-low | "Maximizes the model's ability to think and **use tools**" **[V]** — iterative visual work is tool-loop-bound; but the deciding lever is brief scoping, not effort **[INFERRED]** |
| Coordination / team lead | **do not assign** | Medium | No long-horizon claim from Google; Antigravity's own guidance caps reliable autonomous work at "1–3 hours of human effort" **[3P]** |

---

## 6. grok-4.6 (Grok CLI / Grok Build)

Specs: **500,000 context**, no stated text-output limit, knowledge cutoff January 2026, **$2/$6 per M**. Released 2026-08-12. **[V]** ([Grok 4.6](https://docs.x.ai/developers/grok-4-6))

### 6.1 What each level changes

`reasoning_effort` supports **`low`, `medium`, `high` (default), `xhigh`** on grok-4.6. `xhigh` is grok-4.6-and-later only; on grok-4.5 an `xhigh` request is silently treated as `high`. **Reasoning cannot be disabled.** The parameter is incompatible with `presencePenalty`, `frequencyPenalty` and `stop`. Reasoning tokens are billed. **[V]** ([Reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning))

Note there is **no `max` rung** on grok-4.6 — the ladder tops out at `xhigh`. And the default is `high`, one rung below the ceiling, matching Anthropic's shape rather than OpenAI's.

Harness layer, from the shipped user guide **[V]** ([grok-build docs](https://github.com/xai-org/grok-build/tree/main/crates/codegen/xai-grok-pager/docs/user-guide)):
- `/effort <level>` — *"Set reasoning effort on the **current** model without reselecting it. Levels are `low`, `medium`, `high`, and `xhigh`, and it only applies when the active model supports reasoning effort."*
- `--reasoning-effort` / `--effort <LEVEL>` — *"Canonical levels: `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` (each a distinct tier; **a model only accepts the levels its menu advertises**). Also accepts per-model menu option ids. Works in TUI and headless."*
- `models.default_reasoning_effort` config key; also settable via the `GROK_CONFIG` JSON overlay, e.g. `GROK_CONFIG='{"models": {"default_reasoning_effort": "high"}}'`.

**The most team-relevant feature of any harness here:** Grok Build's subagent personas carry their own `reasoning_effort`, resolved *"highest priority first: 1. Explicit spawn-time override, 2. Role default, 3. Persona default, 4. Parent session."* **[V]** And its workflow launcher takes `--effort LEVEL` to set **child** reasoning effort *"without changing the current session's `/effort`; a child script's own `effort` option takes precedence."* **[V]** This is a per-role effort ladder as a first-class harness primitive — structurally the same idea as Claude Code's skill/subagent frontmatter `effort`, and exactly the shape a taurhaus role template should map onto.

### 6.2 Vendor recommendation, quoted

Per level **[V]** ([Reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning)):
- `low` — "**Latency-sensitive agentic use and simple tool calling**"
- `medium` — "Complex data analysis and long-context reasoning"
- `high` (default) — "Very challenging problems, complex math, multi-step logic, competition-level tasks"
- `xhigh` — "**The hardest problems, where answer quality matters more than response time**"

xAI's ladder is worded around *problem difficulty and latency tolerance*, with no coding-specific language at any rung — the least task-typed guidance of the four vendors. The grok-4.6 model page itself gives the levels and no per-task recommendation **[V]**.

Two operational notes from the model page **[V]**: set `prompt_cache_key` to *"route a conversation's requests to the same server, making cache hits reliable"*, and *"long agent loops additionally benefit from context compaction."* Both matter more at higher effort, where reasoning tokens accumulate into subsequent inputs.

### 6.3 Published quality deltas between adjacent levels

xAI publishes none **[V]**. But grok-4.6 is the **only** model on this roster with a published *adjacent-rung* agentic-coding comparison, via CursorBench v3.2 **[3P]** ([digitalapplied, 2026-08-12](https://www.digitalapplied.com/blog/grok-4-6-vs-gpt-5-6-sol-opus-5-fable-5-effort-tiers-2026); the xhigh/high figures corroborated by [basenor](https://www.basenor.com/blogs/news/grok-4-6-hits-1-on-cursorbench-in-extra-high-thinking-mode)):

| Rung | CursorBench v3.2 | Cost/task | Δ vs rung below |
|---|---|---|---|
| medium | 67.1% | $1.28 | — |
| high (default) | 69.9% | $2.34 | **+2.8 pts for +83% cost** |
| xhigh | 70.8% | $2.81 | **+0.9 pts for +20% cost** |

**This is the single most decision-useful table in the report**, and it says something clear: the medium→high step buys ~3× more quality per rung than high→xhigh, and the high→xhigh step costs ~$0.47/task for under a point. For context, Fable 5 at `max` scores 70.5% on the same board — i.e. **grok-4.6 at `medium` is 3.4 points behind Fable 5 at `max` for a fraction of the price.**

Caveats: one benchmark, one snapshot, third-party compilation, and CursorBench measures Cursor-harness agentic edits, not Grok Build's own loop. `low` is unpublished.

Other grok-4.6 figures, effort level unlabeled **[3P]**: AA Intelligence Index 61, DeepSWE v1.1 65.9%, SWE-bench (Vals) 95.60%, GDPVal-AA v2 1753. Reported Terminal-Bench numbers diverge wildly between sources (26% vs 88.4%) due to benchmark-version differences — **do not use Terminal-Bench for grok comparisons without pinning the version.**

### 6.4 Pathologies at the extremes

xAI documents **none** at any level **[V]** — confirmed across the reasoning page, the grok-4.6 page and the Grok Build settings docs.

The CursorBench curve above is itself the pathology evidence: **[INFERRED]** grok-4.6's effort curve flattens hard between `high` and `xhigh` on agentic coding. That is a diminishing-return signature, not an over-engineering signature — the score still rises. But at +20% cost for +0.9 points, `xhigh` needs a specific justification.

Hard constraints that function as pathologies **[V]**: reasoning cannot be disabled, so there is no cheap non-reasoning tier; and `reasoning_effort` conflicts with `stop` / penalty parameters, which can silently break a harness that sets them.

**[INFERRED]** The 500K context is the smallest on this roster (vs 1M+ for Fable 5, Opus 5 and Sol). Combined with xAI's own note that *"long agent loops additionally benefit from context compaction"*, grok members in a long team run will hit compaction sooner — which for taurhaus means the grok compaction path (`MeshInbox` delivery, `PostCompact` hook, per the harness registry) gets exercised more often than the others. Effort raises token production and therefore accelerates this.

### 6.5 Effort × size of the instruction set

No vendor statement **[V]**. **[INFERRED]** Because xAI's rungs are typed by *difficulty and latency tolerance* rather than by task shape, and because the harness resolves effort through a **persona → role → session** chain, the natural design is to put the instruction set in the persona and the effort in the role default — letting one persona run at two efforts for two lanes. This is a harness-shape argument, not a measured one.

### 6.6 Recommended starting level per task class — grok-4.6

| Task class | Start at | Confidence | Evidence |
|---|---|---|---|
| Implementation of a written spec | **high** (the default) | Medium-high | +2.8 pts over medium on CursorBench **[3P]**; "very challenging problems… multi-step logic" **[V]**; the +0.9-pt xhigh step doesn't pay for routine spec work **[3P]** |
| Review with a checklist | **medium** | Medium | "Complex data analysis and long-context reasoning" **[V]**; 67.1% at medium is close to the frontier for $1.28/task **[3P]** |
| Docs verification | **low** | Medium | "Latency-sensitive agentic use and **simple tool calling**" **[V]** — the only vendor rung explicitly typed for tool calling at the bottom |
| Mechanical sweeps | **low** | Medium-high | Same **[V]** |
| Architecture decisions | **xhigh** | Medium | "The hardest problems, where answer quality matters more than response time" **[V]** — the one class where +0.9 pts for +20% is obviously worth it, because the artifact is durable |
| Coordination / team lead | **high** | Low-medium | No vendor long-horizon claim; 500K context is the constraint, not effort **[V] [INFERRED]** |

---

## 7. Closing ladder: complexity → effort, per model

Read each row as **"the least effort that is defensible for this complexity band."** Start there, then use Anthropic's diagnostic rule — raise only after observing a *skipped file, skipped test, or skipped check* — rather than raising in anticipation.

| Complexity band | Fable 5 | Opus 5 | gpt-5.6-sol | gpt-5.6-luna | gemini-3.7-flash | grok-4.6 |
|---|---|---|---|---|---|---|
| **0. Lookup / classification** (is this file referenced? which tool owns X?) | low | low | low | low | low | low |
| **1. Mechanical sweep** (rename, codemod, import fix, lint) | low | low | low | **low ★best fit** | low | low |
| **2. Docs verification** (claims ↔ code, link/anchor checks) | medium | **low** | low | low *(if corpus small)* | low | low |
| **3. Bounded single-file change against a clear AC** | medium | medium | medium | medium | medium | medium |
| **4. Checklist review of a diff** | high → medium | **medium ★vendor-licensed** | medium | medium | medium | medium |
| **5. Multi-file spec implementation** | **xhigh** | high → xhigh | high | *not assigned* | medium | high |
| **6. Ambiguous debugging / unfamiliar subsystem** | xhigh | xhigh | high → xhigh | *not assigned* | high | high |
| **7. Architecture decision / ADR** | xhigh | xhigh | xhigh | *not assigned* | high | xhigh |
| **8. Security-shaped review** | *not assigned* (classifier refusals) | xhigh | **xhigh ★vendor-named** | *not assigned* | high | xhigh |
| **9. Long-horizon autonomous run (>30 min)** | **xhigh ★vendor-scoped** | xhigh | xhigh | *not assigned* | *not assigned* | high |
| **10. Coordination / team lead (long-lived session)** | high, constant | high, constant + delegation cap | high | *not assigned* | *not assigned* | high |
| **Ceiling rung, and when** | `max`: frontier-only, expect overthinking on structured output | `max`: only when an eval shows headroom at xhigh | `max`: not reachable via our Codex CLI surface | do not exceed `high` | `high` is the ceiling | `xhigh` is the ceiling; +0.9pt/+20% |

**Cross-cutting rules that come with the ladder:**

1. **Set effort at launch, per member; hold it for the session.** Anthropic: changing effort mid-conversation invalidates the prompt cache **[V]**. Claude Code additionally holds Fable 5's default across sessions unless you pass `--effort` explicitly at launch **[V]**.
2. **Push effort into the role, not the session, where the harness allows it.** Claude Code: `effort` in skill/subagent frontmatter **[V]**. Grok Build: `reasoning_effort` on personas, resolved spawn-override → role → persona → session **[V]**. These are the right insertion points for taurhaus role templates.
3. **As effort goes up, shorten the role and shift it from procedure to outcome.** As effort goes down, lengthen it into an explicit sectioned checklist. **[INFERRED]**, built on Anthropic's *"Pair `low` with explicit checklists"* and *"too prescriptive… can degrade output quality"* **[V]** plus OpenAI's remove-the-preamble-instructions guidance **[V]**.
4. **Delete self-verification instructions from any role running Opus 5 at `high`+.** Vendor-stated: they *"cause over-verification"* and removing them costs nothing in quality **[V]**. taurhaus's stacked CLAUDE.md + role-template quality gates are a live instance of this pattern.
5. **Never disable thinking to save money.** On Opus 5, *"thinking enabled at `low` effort performs better than thinking disabled at similar cost"* **[V]**, and disabling it produces tool-calls-as-text and leaked XML tags **[V]**.
6. **Do not show Fable 5 a remaining-context countdown** — it triggers premature handoff offers **[V]**.
7. **Model choice dominates effort choice.** arXiv 2607.02436: *"Capability tier dominated"* **[R]**. Moving Luna from `medium` to `xhigh` is a worse trade than moving the task to Sol at `medium`.

---

## 8. What only an eval can decide

These are the questions where I found no adequate primary evidence and where the answer plausibly flips the ladder above. Ordered by expected value of running the eval.

1. **Does the high→xhigh step reproduce on *our* spec-implementation tasks?** The one strong number (28% → 89% first-try-perfect for +9–29% cost, arXiv 2607.02436 **[R]**) is a single greenfield app, model-per-cell unattributed. This is the highest-stakes open question in the whole report — it is the difference between running the implementation lane at `high` and at `xhigh` across the entire roster.
2. **Is the "greenfield rewards effort / bounded refactor punishes it" split real?** **[INFERRED]** in §3.4, from the tension between arXiv 2607.02436 **[R]** and the prior-generation refactor regression **[3P]**. Test: same model, same effort sweep, one greenfield-from-spec suite and one fixed-contract refactor suite.
3. **Does Luna's effort curve actually saturate at `high`?** Reported once, unverified, un-corroborated **[3P]**. If true, Luna's usable band is `low`–`medium` and everything above is pure burn. Cheapest eval on this list.
4. **Does `agy --effort` actually change token spend, and does it map to `thinking_level`?** The mapping is **[INFERRED]**, the CLI has a documented history of the flag being ignored in interactive and `-p` runs (fixed in 1.1.2) **[V]**, and the Antigravity *agent API* has no `thinking_level` at all **[V]**. Verify in `taurhaus.log.jsonl` before trusting any `agy` effort policy.
5. **Where is grok-4.6's `low` rung on agentic coding?** CursorBench publishes medium/high/xhigh only **[3P]**. If `low` is close to `medium`, grok becomes the cheapest credible sweep-and-verify lane on the roster.
6. **Does Opus 5's low-effort review claim hold on our checklists?** Anthropic states review accuracy *"holds at lower effort settings"* **[V]** but gives no number. This licenses the single largest cost saving in the ladder (review lane at `medium` instead of `high`) and deserves a direct measurement against our own review checklist.
7. **Does removing taurhaus's stacked verification instructions improve Opus 5 output?** Vendor says yes, unconditionally **[V]**. Our role templates carry `quality_gates`, `definition_of_done` and `required_artifacts`; some of that is contract (keep) and some is procedure (remove). Only an A/B on real tasks separates them.
8. **What are the actual token/latency multipliers per rung, per model, on our workloads?** No vendor publishes them. The only figures anywhere are third-party community estimates for Codex (`low` ~0.3×, `high` ~3–5×, `xhigh` ~8–15× vs `medium`) **[3P]**, explicitly disclaimed at the source. Every cost projection in this report inherits that uncertainty.
9. **Cross-model calibration at a fixed level name.** Anthropic says the scale is per-model **[V]**; nobody publishes a cross-vendor anchor. Until measured, "everyone at high" means four different things and cross-member comparisons are meaningless.
10. **Effort × instruction-set size, measured.** Every vendor statement here is qualitative and one-directional. The 2×2 (lean/heavy role × low/high effort) has never been published by anyone. Rule 3 of §7 rests entirely on inference.

---

## Sources

**Vendor primary [V]** — all fetched 2026-08-28:

- Anthropic — Effort: https://platform.claude.com/docs/en/build-with-claude/effort
- Anthropic — Prompting Claude Fable 5: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5
- Anthropic — Introducing Claude Fable 5 and Claude Mythos 5 (GA 2026-06-09): https://platform.claude.com/docs/en/models/fable-5/introducing-claude-fable-5-and-claude-mythos-5
- Anthropic — Prompting Claude Opus 5: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5
- Anthropic — What's new in Claude Opus 5: https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5
- Anthropic — Claude Code model configuration: https://code.claude.com/docs/en/model-config
- Anthropic — Choosing a Claude model and effort level in Claude Code (blog, 2026-07-07): https://claude.com/blog/claude-model-and-effort-level-in-claude-code
- OpenAI — Reasoning guide: https://developers.openai.com/api/docs/guides/reasoning
- OpenAI — Model guidance / latest model: https://developers.openai.com/api/docs/guides/latest-model
- OpenAI — gpt-5.6-sol model page: https://developers.openai.com/api/docs/models/gpt-5.6-sol
- OpenAI — gpt-5.6-luna model page: https://developers.openai.com/api/docs/models/gpt-5.6-luna
- OpenAI — Codex models: https://learn.chatgpt.com/docs/models
- OpenAI — Codex prompting guide: https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide
- OpenAI — GPT-5.2 prompting guide: https://developers.openai.com/cookbook/examples/gpt-5/gpt-5-2_prompting_guide
- Google — What's new in Gemini 3.7 Flash: https://ai.google.dev/gemini-api/docs/latest-model
- Google — Gemini thinking: https://ai.google.dev/gemini-api/docs/thinking
- Google — Antigravity agent: https://ai.google.dev/gemini-api/docs/antigravity-agent
- Google — Antigravity models: https://antigravity.google/docs/models/
- Google — Antigravity CLI best practices: https://antigravity.google/docs/cli/best-practices/
- Google — antigravity-cli CHANGELOG: https://github.com/google-antigravity/antigravity-cli/blob/main/CHANGELOG.md
- Google — Introducing Gemini 3.7 Flash (blog, 2026-08-13): https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/
- xAI — Reasoning: https://docs.x.ai/developers/model-capabilities/text/reasoning
- xAI — Grok 4.6: https://docs.x.ai/developers/grok-4-6
- xAI — Grok Build overview: https://docs.x.ai/build/overview
- xAI — Grok Build user guide (slash commands, configuration, headless mode, subagents, config reference): https://github.com/xai-org/grok-build/tree/main/crates/codegen/xai-grok-pager/docs/user-guide

**Research [R]:**

- Achint Mehta, *Reasoning effort, not tool access, buys first-try reliability in agentic code generation: an observational study*, arXiv:2607.02436, submitted 2026-07-02: https://arxiv.org/abs/2607.02436

**Third-party [3P]:**

- Artificial Analysis — GPT-5.6 benchmarks (2026-07-09): https://artificialanalysis.ai/articles/gpt-5-6-has-landed
- Vellum — GPT-5.6 Sol vs Terra vs Luna (2026-07-09): https://www.vellum.ai/blog/gpt-5-6-benchmarks-explained
- Vellum — Claude Fable 5 & Mythos 5 benchmarks: https://www.vellum.ai/blog/claude-fable-5-and-mythos-5-benchmarks-explained
- morphllm — Claude benchmarks 2026: https://www.morphllm.com/claude-benchmarks
- digitalapplied — Grok 4.6 vs Sol vs Opus 5 vs Fable 5: effort tiers (2026-08-12): https://www.digitalapplied.com/blog/grok-4-6-vs-gpt-5-6-sol-opus-5-fable-5-effort-tiers-2026
- digitalapplied — Reasoning effort: cost vs quality benchmarks (2026-04-23, prior-generation models): https://www.digitalapplied.com/blog/reasoning-effort-cost-vs-quality-benchmarks-2026
- basenor — Grok 4.6 hits #1 on CursorBench in Extra High: https://www.basenor.com/blogs/news/grok-4-6-hits-1-on-cursorbench-in-extra-high-thinking-mode
- Codex Knowledge Base — Reasoning effort tuning (2026-03-27, upd. 2026-08-21): https://codex.danielvaughan.com/2026/03/27/reasoning-effort-tuning/
- continuumcode — Antigravity CLI guide (Aug 2026): https://continuumcode.ai/guides/antigravity-cli/
- Antigravity Lab — Background agent production guide: https://antigravitylab.net/en/articles/agents/antigravity-background-agent-advanced-production-guide
- techtimes — GPT-5.6 prompting guide (2026-07-15, **unverified attribution to OpenAI**): https://www.techtimes.com/articles/320650/20260715/gpt-56-prompting-guide-lean-system-prompts-now-outperform-elaborate-scaffolding.htm
- The Prompt Index — GPT-5.6 & Fable 5 prompting guide (**unverified attribution**): https://www.thepromptindex.com/gpt-5-6-and-claude-fable-5-prompting-guide.html

**Could not fetch:** https://openai.com/index/gpt-5-6/ (HTTP 403) — the likely home of OpenAI's per-effort curves. Worth a manual read.
