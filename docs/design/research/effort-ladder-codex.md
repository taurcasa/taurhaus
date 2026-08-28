# Reasoning-effort ladders for coding-agent teams

**As of:** 2026-08-28  
**Harness scope:** Claude Fable 5 and Opus 5 in Claude Code; OpenAI GPT-5.6 Sol and Luna in Codex CLI 0.149; Gemini 3.7 Flash in Antigravity CLI; Grok 4.6 in Grok CLI. Sonnet and Terra are deliberately excluded.

## Executive answer

Effort is not a cross-vendor unit. It is a model-specific control over how much hidden reasoning a model is encouraged to do. It generally buys more planning, search, self-checking, and persistence, but it does **not** directly request a longer visible answer, and—except where a vendor explicitly says so—it does not set parallel-agent count or a fixed tool-call quota.

For a team implementing a specification that is already written, the best default is usually one rung below the level appropriate for discovering the architecture from scratch:

- Fable 5 and Opus 5: start `high` for implementation, `medium` for constrained review, `low` for mechanical work, and `xhigh` for architecture or long-horizon lead work. Reserve `max` for eval-proven frontier cases.
- GPT-5.6 Sol: start `medium` for most written-spec implementation, `low` for mechanical verification, and `high` for architecture/debugging; use `xhigh` only when measured gains justify it.
- GPT-5.6 Luna: start `high` for nontrivial implementation, `low` for mechanical work, and prefer escalating the model to Sol over asking Luna to own frontier architecture.
- Gemini 3.7 Flash: start `medium` for most implementation/review, `low` for mechanical work, and `high` for architecture or long multi-step agents.
- Grok 4.6: start `high` for nontrivial implementation, `medium` for bounded analysis/review, `low` for simple tools, and `xhigh` for quality-first frontier work.

These starting points are **INFERRED recommendations**, not vendor promises. Confidence and direct evidence appear in the team matrix below.

### Evidence notation

- **DIRECT:** the vendor explicitly documents the behavior or measurement.
- **INFERRED:** synthesis, visual reading of a chart, extrapolation across a model family, or a recommendation not directly tested by the vendor.
- “No adjacent curve” means no vendor-published coding/agentic benchmark broken out at every adjacent effort level was found by 2026-08-28. A single high/max score cannot establish the marginal value of moving up one rung.
- Costs are API list prices, not necessarily the effective cost of a subscription CLI. Per-task cost also depends on prompt/cache size, hidden reasoning/output tokens, tools, retries, and provider quota accounting.

## What effort changes across vendors

| Dimension | Best-supported interpretation |
|---|---|
| Hidden thinking budget | Usually a relative behavioral target. Anthropic explicitly says effort is not a strict token budget; Google calls `thinking_level` relative and dynamic. OpenAI reasoning tokens may range from hundreds to tens of thousands. |
| Planning and verification | Generally increase with effort. Anthropic explicitly includes thinking, tool use, and self-verification; OpenAI and xAI describe deeper planning/reasoning; Google describes increasing reasoning depth. |
| Tool calls | Anthropic explicitly says lower effort tends to make fewer calls. Other vendors associate effort with agentic/tool reasoning but do not publish a deterministic call-count mapping. |
| Parallelism/breadth | Not the same control. OpenAI multi-agent/“ultra,” xAI multi-agent models, and orchestration-layer concurrency are separate from the single-agent effort flag. **INFERRED:** more reasoning can discover more tool work, but effort does not guarantee parallelism. |
| Visible verbosity | Separate or not promised. Anthropic says effort controls thinking volume, not visible response length; OpenAI exposes text verbosity separately. No vendor evidence found that the Google or xAI effort flag is a reliable visible-verbosity control. |
| Latency and cost | Usually rise because more output/reasoning and tool work are generated. Only Anthropic publishes useful multi-effort cost/quality curves for the models in scope; the others mostly publish one benchmark setting or qualitative guidance. |

## Claude Fable 5

### 1. What each level changes

Anthropic’s [effort documentation](https://platform.claude.com/docs/en/build-with-claude/effort) (accessed 2026-08-28) says effort controls the total response behavior—thinking, visible response, tool calls, and tool arguments—and is a behavioral signal rather than a hard token allocation. Its [thinking/cost guide](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost) (accessed 2026-08-28) says adaptive thinking can be interleaved with tools and hidden thinking is billed as output tokens.

| Level | Vendor-described behavior | Practical cost/latency implication |
|---|---|---|
| `low` | Minimizes thinking and may skip it on simple requests; intended for simple work and subagents. Fewer tool calls are likely. | Fastest/cheapest. DIRECT research benchmarks gave up 1–3 points for 33–50% savings; one DeepWideSearch run took 4.5 minutes versus 7.9 at default. |
| `medium` | Moderate reasoning; may skip thinking on simple tasks; balanced cost/capability. | DIRECT research results matched high/default at roughly 70–85% of its cost on four measured benchmarks. |
| `high` | Almost always thinks deeply; Anthropic’s default and recommended starting point for most tasks. | More reasoning/tool use. Routine work can gather more context and deliberate longer than needed. |
| `xhigh` | Always thinks deeply with extended exploration; intended for demanding coding/agentic and long-horizon work. | Can run for tens of minutes and very large token counts; higher tail latency/cost. |
| `max` | Unconstrained maximum capability; always thinks with no effort constraint. | Highest and least predictable usage; diminishing returns must be demonstrated. |

Fable 5 API list price was published as **$10/M input and $50/M output tokens** in Anthropic’s [Fable 5 launch](https://www.anthropic.com/news/claude-fable-5-mythos-5) (published 2026-06-09; updated 2026-07-01). The vendor’s [cost-optimization guide](https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence) (accessed 2026-08-28) reports a 57-document corpus taking 7.9 hours at low, 9.1 at medium, and 11.4 at default/high. These are workload measurements, not universal multipliers.

### 2. Vendor recommendation

Anthropic’s Fable prompting guide says: **“Use `high` as the default for most tasks, with `xhigh` for the most capability-sensitive workloads.”** ([source](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5), accessed 2026-08-28). It recommends medium/low for routine tasks where evals show quality holds.

### 3. Adjacent quality deltas

Anthropic published Fable 5 effort curves in the [Opus 5 launch report](https://www.anthropic.com/news/claude-opus-5) (published 2026-07-24). The underlying [Frontier-Bench chart](https://www-cdn.anthropic.com/images/4zrzovbb/website/7530b1086992936d7e9d5796a892d1e8fa063253-3840x2160.png) and [CursorBench chart](https://www-cdn.anthropic.com/images/4zrzovbb/website/1af9dbd742e3812be4bf66903740188fb8fd2e33-3840x2160.png) are primary vendor artifacts.

The numerical points below are **INFERRED from visual chart digitization**, approximately ±0.3–0.5 percentage points; Anthropic did not provide a downloadable point table.

| Benchmark | `low` | `medium` (Δ) | `high` (Δ) | `xhigh` (Δ) | `max` (Δ) |
|---|---:|---:|---:|---:|---:|
| Frontier-Bench | ~17.2 | ~25.0 (**+7.8**) | ~29.0 (**+4.0**) | ~31.8 (**+2.8**) | ~34.0 (**+2.2**) |
| CursorBench | ~61.9 | ~65.2 (**+3.3**) | ~66.7 (**+1.5**) | ~68.7 (**+2.0**) | ~70.5 (**+1.8**) |

The result is not “always use max.” The largest Fable gains in these two charts occur low→medium; later steps buy smaller absolute quality increments at progressively higher cost. On Anthropic’s separate research/knowledge suite, medium matched high/default on all four named benchmarks, so the marginal high step was measured as zero there ([cost guide](https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence), accessed 2026-08-28).

### 4. Extreme-level pathologies

- **DIRECT, low:** minimized thinking and fewer tools can miss dependencies or stop after the obvious edit.
- **DIRECT, high/xhigh:** Anthropic warns Fable can gather context or deliberate beyond what routine work needs. High also supplies strong verification, but can perform unrequested tidying/refactoring unless scope is explicit ([Fable prompting](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5), accessed 2026-08-28).
- **INFERRED, max:** Fable’s measured chart remains monotonic, but the last step has small marginal value and the highest cost; “gold-plating” risk therefore comes from excess time/tool work, not a demonstrated Fable quality regression.

### 5. Interaction with instruction-set size

This is one of the few areas with direct vendor evidence. The Fable prompting guide says older, highly prescriptive skills can degrade results and that brief instructions are often enough; Anthropic recommends removing obsolete scaffolding. Therefore a lean role does **not** need higher effort merely because it is lean. A clear spec reduces ambiguity and often makes medium/high sufficient. Conversely, a large, conflicting role can make high effort spend more tokens reconciling bad instructions. That final causal interpretation is **INFERRED**, but it matches Anthropic’s warning to refactor older prompts.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical sweep `low`; docs/checklist verification `medium` (use `low` when checks are deterministic); written-spec implementation `high`; architecture/coordination `xhigh`; `max` only after an eval shows a material gain. Confidence: **high** for the broad ladder, because Anthropic publishes both task guidance and adjacent curves; **medium** for each repository-specific boundary.

## Claude Opus 5

### 1. What each level changes

Opus uses the same five names and the same response-wide effort mechanism as Fable, but Claude Code’s [model configuration guide](https://code.claude.com/docs/en/model-config) (accessed 2026-08-28) explicitly warns that the scale is calibrated per model: identical labels are not identical amounts of thinking across models.

| Level | Vendor-described behavior | Appropriate use |
|---|---|---|
| `low` | Minimal thinking, fastest/cheapest, fewer likely tool calls. | Short, tightly scoped edits or deterministic checks. |
| `medium` | Balanced reasoning and cost; may skip unnecessary thinking. | Constrained review, routine code, cost-sensitive agents. |
| `high` | Deep reasoning on almost every request; default. | Most substantive coding and difficult debugging. |
| `xhigh` | Extended exploration and longer-horizon work. | Demanding coding/agentic tasks, architecture, lead/coordination. |
| `max` | No effort constraint; deepest available reasoning. | Only frontier, unconstrained work with measured value. |

Opus 5 is **$5/M input and $25/M output** at standard API speed; Anthropic also announced an approximately 2.5× faster mode at 2× price ([Opus 5 launch](https://www.anthropic.com/news/claude-opus-5), published 2026-07-24). Thinking tokens are billed output, so higher effort can dominate task cost even when the input prompt is unchanged.

### 2. Vendor recommendation

Anthropic’s Opus guide says: **“Start with the default (`high`) and adjust based on your evals.”** ([source](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5), accessed 2026-08-28). It recommends xhigh for the hardest capability-sensitive work and says code-review quality can hold at lower effort, enabling a fast first pass before a thorough pass.

### 3. Adjacent quality deltas

The figures below are **INFERRED from visual digitization** of the same vendor [Frontier-Bench](https://www-cdn.anthropic.com/images/4zrzovbb/website/7530b1086992936d7e9d5796a892d1e8fa063253-3840x2160.png) and [CursorBench](https://www-cdn.anthropic.com/images/4zrzovbb/website/1af9dbd742e3812be4bf66903740188fb8fd2e33-3840x2160.png) charts (published 2026-07-24), approximately ±0.3–0.5 points.

| Benchmark | `low` | `medium` (Δ) | `high` (Δ) | `xhigh` (Δ) | `max` (Δ) |
|---|---:|---:|---:|---:|---:|
| Frontier-Bench | ~25.5 | ~35.0 (**+9.5**) | ~39.5 (**+4.5**) | ~44.3 (**+4.8**) | ~43.3 (**−1.0**) |
| CursorBench | ~62.8 | ~64.3 (**+1.5**) | ~66.7 (**+2.4**) | ~69.3 (**+2.6**) | ~70.0 (**+0.7**) |

Anthropic’s separate [cost guide](https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence) (accessed 2026-08-28) reports Opus 5 SWE-bench Pro medium about 2 points below high at roughly half the cost, and low about 8 points below high at roughly one-quarter cost. That implies approximately **low→medium +6 points** and **medium→high +2**, although the report gives rounded values. It also measured an adaptive retry policy: low-first, retry-high reached about 93% versus high-only 91.7% at about half the dollars; medium-first/retry-high reached about 94% for about $0.95. Those are policy outcomes, not fixed-effort benchmark scores.

### 4. Extreme-level pathologies

- **DIRECT, low:** short/scoped tasks are the target; nontrivial implementation can under-plan or under-verify.
- **DIRECT, high/xhigh:** Anthropic says inherited blanket verification instructions can cause over-verification because Opus now self-checks more strongly. Explicitly constrain scope and stopping conditions ([what is new for Opus 5](https://platform.claude.com/docs/en/about-claude/models/whats-new-claude-4-6), accessed 2026-08-28).
- **DIRECT/MEASURED, max:** Claude Code’s model guide warns of diminishing returns and possible overthinking; the Frontier-Bench vendor chart also shows an **INFERRED ~1-point max regression**, while CursorBench shows only ~0.7 gain. Max is not a safe default.

### 5. Interaction with instruction-set size

The [Opus prompting guide](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5) (accessed 2026-08-28) says a complete specification supplied up front works well. It also recommends removing redundant verification scaffolding. **INFERRED:** with a complete written spec, use effort for implementation reasoning, not rediscovery; bulky or contradictory role text is more likely to waste high-effort tokens than to compensate for low effort.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical `low`; docs/checklist review `medium`; written-spec implementation `high`; architecture/coordination `xhigh`; `max` only for an eval-confirmed frontier tail. Confidence: **high** for review-at-medium and avoiding max by default, because those have direct guidance plus curves; **medium** for architecture/lead boundaries.

## OpenAI GPT-5.6 Sol

### 1. What each level changes

The Codex CLI 0.149 harness exposes `low|medium|high|xhigh`, even though the API model also documents `none` and `max`. This report stays within the harness ladder. OpenAI’s [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning) and [model guidance](https://developers.openai.com/api/docs/guides/latest-model) (both accessed 2026-08-28) describe effort as adaptive: hidden reasoning can range from hundreds to tens of thousands of tokens and can interleave with tools.

| Level | Vendor-described change | Recommended use in OpenAI guidance |
|---|---|---|
| `low` | Less planning/search/multi-step reasoning; lower token use and latency. | Execution-oriented coding, simple tools, bounded transformations. |
| `medium` | Balanced planning, judgment, reliability, cost and latency; API default. | Most agentic coding, research, and delegation. |
| `high` | Deeper planning, debugging, and verification. | Hard reasoning, difficult debugging, high-value decisions. |
| `xhigh` | Extended exploration and verification with a larger latency/cost tail. | Deep research and long/asynchronous agentic tasks only when evals show a benefit. |

Reasoning effort does not set visible answer length; OpenAI exposes text verbosity separately. Nor does it directly choose parallel-agent count—the vendor’s multi-agent/“ultra” orchestration is a separate mechanism. **INFERRED:** high effort may discover more tool actions, but there is no published call-count multiplier.

The [Sol model page](https://developers.openai.com/api/docs/models/gpt-5.6-sol) (accessed 2026-08-28) lists a 1.05M context window, 128K maximum output, default `medium`, and promotional API rates of **$4/M input, $0.40/M cached input, and $20/M output** through at least 2026-11-21. Hidden reasoning is billed as output. No vendor per-task cost curve by Sol effort was found.

### 2. Vendor recommendation

OpenAI describes medium as the **“default configuration for most workloads, and a well-balanced point on the pareto curve of latency, performance and cost.”** ([reasoning guide](https://developers.openai.com/api/docs/guides/reasoning), accessed 2026-08-28). Its migration guidance says compare the same effort and one level lower, and reserve high/xhigh for measured gains.

### 3. Adjacent quality deltas

**No vendor-published adjacent-effort coding/agentic curve was found for GPT-5.6 Sol as of 2026-08-28.** OpenAI publishes qualitative task guidance, but not low→medium→high→xhigh deltas on SWE-Bench Pro, Terminal-Bench, CursorBench, or a comparable coding suite for this model. Therefore any numerical marginal value assigned to those steps would be unsourced.

### 4. Extreme-level pathologies

- **DIRECT, low:** reduced planning/search can fail on dependency-rich or multi-step work even when each individual edit looks simple.
- **DIRECT, high/xhigh:** OpenAI warns that higher effort is not automatically better when prompts contain conflicting instructions, weak stopping conditions, or unnecessarily open tool access; overthinking, unnecessary search, and regressions can result ([model guidance](https://developers.openai.com/api/docs/guides/latest-model), accessed 2026-08-28).
- **INFERRED:** xhigh is most vulnerable to “review forever,” speculative refactors, and expensive verification loops in a role whose completion rule is vague.

### 5. Interaction with instruction-set size

OpenAI reports that leaner internal coding-agent prompts improved eval results by **10–15%**, reduced tokens **41–66%**, and reduced cost **33–67%** in one internal sample ([model guidance](https://developers.openai.com/api/docs/guides/latest-model), accessed 2026-08-28). This directly rejects the idea that a lean role inherently needs higher effort. **INFERRED:** put non-negotiable constraints, acceptance tests, and stopping rules in the prompt; remove generic process narration. A large role should be simplified before effort is raised.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical/docs `low`; ordinary written-spec implementation and checklist review `medium`; architecture, cross-cutting debugging, and team lead `high`; `xhigh` for long asynchronous work, security-critical review, or stubborn failures after eval evidence. Confidence: **medium-high** for the qualitative ladder from direct guidance, **low** for exact boundaries because adjacent Sol measurements are absent.

## OpenAI GPT-5.6 Luna

### 1. What each level changes

Luna supports the same harness ladder and defaults to medium, but the effort labels are model-relative; they must not be treated as Sol-equivalent reasoning tokens or quality. The general OpenAI [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning) (accessed 2026-08-28) supplies the behavior mapping: low for reduced planning/execution, medium for balanced reasoning, high for difficult planning/debugging, and xhigh for long quality-first work.

The [Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna) (accessed 2026-08-28) describes it as **“designed for cost-sensitive, high-volume workloads”**, lists default `medium`, and prices it at **$0.20/M input, $0.02/M cached input, and $1.20/M output**. No effort-specific per-task cost curve is published.

### 2. Vendor recommendation

OpenAI’s effort table includes **“execution-oriented coding”** among low-effort uses ([reasoning guide](https://developers.openai.com/api/docs/guides/reasoning), accessed 2026-08-28), while medium is the general balanced/default setting. Combining that guidance with Luna’s high-volume purpose supports low/medium for large mechanical fleets; it does not prove medium is enough for every implementation.

### 3. Adjacent quality deltas

**No vendor-published adjacent-effort coding/agentic curve was found for GPT-5.6 Luna as of 2026-08-28.** In particular, no primary source found documents a “Luna medium cliff.” Treat that phrase as an **INFERRED harness hypothesis** until a controlled low/medium/high/xhigh sweep reproduces it.

### 4. Extreme-level pathologies

- **DIRECT, low:** lower planning is a poor fit for multi-file dependency reasoning, despite the attractive token price.
- **INFERRED, medium cliff:** a smaller/faster model may cross a behavioral threshold only when extra planning budget is available, making medium appear discontinuously worse than high on some repo tasks. This is plausible but not vendor-measured.
- **INFERRED, xhigh:** long reasoning can erase much of Luna’s per-task speed/cost advantage without overcoming a model-capability ceiling. When xhigh becomes routine, compare Sol at medium/high instead.

### 5. Interaction with instruction-set size

The same OpenAI lean-prompt evidence applies, but no Luna-specific instruction-length ablation was found. **INFERRED:** Luna benefits especially from a short, explicit checklist and a complete spec because capacity is better spent on the code than reconciling role prose. Do not compensate for bloated instructions by reflexively raising effort.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical/docs `low`; short bounded agents `medium`; nontrivial written-spec implementation and subtle review `high`; xhigh only for an eval-confirmed Luna win, otherwise escalate to Sol. Confidence: **high** for mechanical-low, **medium-low** for implementation-high, and **low** for any purported medium cliff because there is no adjacent vendor curve.

## Google Gemini 3.7 Flash

### 1. What each level changes

Antigravity’s [headless CLI guide](https://antigravity.google/docs/cli/headless/) (accessed 2026-08-28) exposes `--effort low|medium|high`. Google’s [Gemini thinking guide](https://ai.google.dev/gemini-api/docs/generate-content/thinking) (accessed 2026-08-28) maps that to model-relative `thinking_level`, not a fixed token budget. Gemini 3.7 Flash supports all three levels and defaults to medium; there is no “minimal/off” setting.

| Level | Vendor-described behavior | Implication |
|---|---|---|
| `low` | Minimizes reasoning depth for latency and cost. | Simple lookups, transcript searches, deterministic transformations. |
| `medium` | Dynamic balanced reasoning for most work; default. | General coding, review, and ordinary multi-step agents. |
| `high` | Maximum reasoning depth for the model. | Difficult architecture, dense/multi-source analysis, long multi-step work; significantly longer time to first token. |

Google’s thinking budget is dynamic per request. No numerical hidden-token allotment, tool-call multiplier, or parallelism mapping is published for these 3.7 levels. Visible verbosity is likewise not documented as an effort control.

The [Gemini 3.7 Flash developer guide](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/guides/gemini-3-7-flash) (updated 2026-08-27) lists a 1,048,576-token context and 65,536-token output maximum. The [launch post](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/) (published 2026-08-13) gives an introductory API price through 2026 of **$0.75/M input and $3.75/M output**. It does not publish per-task cost by effort.

### 2. Vendor recommendation

Google labels medium **“Balanced thinking for most tasks”** and high as **“Maximizes reasoning depth”** ([thinking guide](https://ai.google.dev/gemini-api/docs/generate-content/thinking), accessed 2026-08-28). Its 3.7 guide recommends high for dense visual reasoning or multi-step analysis across very long video, medium for general Q&A/summaries, and low for transcript search or metadata extraction; that pattern transfers naturally to coding complexity, though the transfer itself is **INFERRED**.

### 3. Adjacent quality deltas

**No vendor-published adjacent low/medium/high coding curve was found for Gemini 3.7 Flash.** Google’s launch reports single-setting headline results including TerminalBench 2.1 **85.1** and DeepSWE **63.7** in the enterprise model guide, and FrontierCode **43.6** and DeepSWE **65.3** in the launch post. The differing harness/version figures reinforce why they must not be used as adjacent effort deltas. The model’s marginal medium→high value remains unknown.

### 4. Extreme-level pathologies

- **DIRECT, low:** minimal reasoning can under-plan multi-file changes or fail to reconcile a long context.
- **DIRECT, high:** Google warns of significantly longer time to first token; higher reasoning tokens also increase output-billed work.
- **INFERRED for 3.7:** Google’s Gemini 3.x prompting guidance says verbose, complex legacy prompts can cause over-analysis and unnecessary tool calls; applying this family guidance specifically to 3.7 Flash is reasonable but not a published 3.7 ablation ([Gemini 3.x prompting notes](https://ai.google.dev/gemini-api/docs/whats-new-gemini-3.5), accessed 2026-08-28).

### 5. Interaction with instruction-set size

Google recommends direct, concise instructions for the Gemini 3.x family and warns against carrying forward excessive prompt engineering. **INFERRED for 3.7 Flash:** a lean, explicit role makes medium more viable; it does not require high. A long instruction set should be deduplicated and ordered before increasing effort.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical/docs `low`; written-spec implementation and checklist review `medium`; architecture, long-context debugging, and coordination `high`. Confidence: **medium-high** for the three-rung complexity mapping because it follows direct thinking-level guidance; **low-medium** for coding-specific boundaries because Google publishes no adjacent coding curve.

## xAI Grok 4.6

### 1. What each level changes

xAI’s [reasoning guide](https://docs.x.ai/developers/model-capabilities/text/reasoning) (accessed 2026-08-28) says Grok 4.6 always reasons and supports `low|medium|high|xhigh`, with high the default.

| Level | Vendor-described behavior | Typical task |
|---|---|---|
| `low` | Some reasoning with priority on speed. | Simple tools and latency-sensitive agents. |
| `medium` | More thinking and planning. | Complex data analysis and long-context reasoning. |
| `high` | Deeper reasoning; default. | Hard mathematics, difficult coding, multi-step work. |
| `xhigh` | Maximum depth and highest latency. | Hardest quality-first problems. |

The effort control changes single-agent reasoning depth. xAI’s models that vary agent count are a separate multi-agent mechanism; no tool-call or parallelism multiplier is documented for Grok 4.6 effort. No visible-verbosity mapping is documented.

The [Grok 4.6 model guide](https://docs.x.ai/developers/grok-4-6) (updated 2026-08-21) and [pricing page](https://docs.x.ai/developers/pricing) (accessed 2026-08-28) list a 500K context and standard rates under 200K context of **$2/M input, $0.50/M cached input, and $6/M output**. Above 200K context the rates double to $4/$1/$12. The [launch post](https://x.ai/news/grok-4-6) (published 2026-08-12) says the fast variant costs 2×. No per-task effort curve is published.

### 2. Vendor recommendation

xAI recommends low for **“Latency-sensitive agentic use and simple tool calling”**, medium for **“Complex data analysis and long-context reasoning”**, and xhigh for **“The hardest problems, where answer quality matters more than response time”** ([reasoning guide](https://docs.x.ai/developers/model-capabilities/text/reasoning), accessed 2026-08-28).

### 3. Adjacent quality deltas

**No vendor-published adjacent-effort coding/agentic curve was found for Grok 4.6.** The launch’s high-effort measurements include CursorBench 3.2 **69.9**, DeepSWE 1.1 **65.9**, FrontierCode 1.1 extended **61.3**, TerminalBench 3.0 **26.0**, and APEX-SWE **56.4** ([launch](https://x.ai/news/grok-4-6), 2026-08-12). Because every reported in-scope score is high effort, none measures low→medium, medium→high, or high→xhigh.

### 4. Extreme-level pathologies

- **DIRECT, low:** only “some reasoning”; under-planning is the expected risk on dependency-rich work.
- **DIRECT, xhigh:** maximum reasoning depth comes with the highest latency.
- **INFERRED, high/xhigh:** vague stopping conditions can invite repeated self-testing or gold-plating. xAI highlights stronger self-testing on long trajectories, but does not publish a pathology study showing that it becomes excessive.

### 5. Interaction with instruction-set size

No Grok 4.6 vendor ablation relating prompt/role length to effort was found. **INFERRED:** instruction clarity and effort are orthogonal controls. Keep the written spec and acceptance criteria complete but the process role lean; raise effort for intrinsic dependency/decision complexity, not for prompt length itself.

### 6. Starting levels when the spec is given

**INFERRED recommendation:** mechanical/docs `low`; bounded analysis and checklist review `medium`; nontrivial implementation and hard debugging `high`; architecture and long-horizon lead work `xhigh`. Confidence: **medium** for the qualitative task mapping from direct guidance; **low** for exact coding boundaries because adjacent measurements are absent.

## Team task matrix: written spec already exists

All cells in “start” are **INFERRED recommendations**. Confidence is H/M/L and reflects evidence for that model-task boundary, not model quality.

| Task class | Fable 5 | Opus 5 | Sol | Luna | Gemini 3.7 Flash | Grok 4.6 | Why |
|---|---|---|---|---|---|---|---|
| Mechanical sweeps, renames, formatting, deterministic checks | `low` H | `low` H | `low` H | `low` H | `low` H | `low` H | Every vendor assigns low to simple/execution/latency-sensitive work. |
| Docs verification against explicit sources | `low→medium` M | `low→medium` M | `low` M | `low` M | `low` M | `low` M | Mostly retrieval/comparison; move up when sources conflict or tool navigation is hard. |
| Checklist code review | `medium` M | `medium` H | `medium` M | `high` L | `medium` M | `medium` M | Constraint reduces search space; Opus vendor explicitly says review can hold at lower effort. Luna-high is an eval hypothesis. |
| Implementation of a complete written spec | `high` H | `high` H | `medium` M | `high` L | `medium` M | `high` M | Anthropic defaults high; OpenAI/Google default medium for most work; Grok defaults high. Spec removes architecture discovery. |
| Cross-cutting debugging/integration | `high→xhigh` M | `high→xhigh` M | `high` M | `high→xhigh` L | `high` M | `high` M | Dependency search, hypothesis testing, and verification benefit from deeper planning. |
| Architecture/API/security decisions | `xhigh` M | `xhigh` M | `high→xhigh` M | **prefer Sol** L | `high` M | `xhigh` M | High-value irreversible decisions justify depth; Luna substitution needs model-vs-effort evaluation. |
| Coordination/lead: decompose, delegate, merge, stop | `xhigh` M | `xhigh` M | `high→xhigh` M | **prefer Sol** L | `high` M | `xhigh` M | Lead work adds planning and stopping-policy complexity; parallel-agent count remains an orchestrator setting. |

Two operational rules follow:

1. **INFERRED:** use low-first/high-on-failure only when failure is cheaply and reliably detected (tests, schemas, or a checklist). Anthropic’s measured retry policy shows why this can dominate all-high, but the policy must be re-evaluated on each model and repository.
2. **INFERRED:** review should not blindly use a higher level than implementation. A tight checklist often makes medium sufficient; security, concurrency, or architectural review can justify high/xhigh.

## Closing complexity → effort ladder

| Complexity | Fable 5 | Opus 5 | GPT-5.6 Sol | GPT-5.6 Luna | Gemini 3.7 Flash | Grok 4.6 |
|---|---|---|---|---|---|---|
| 0. Deterministic/mechanical | `low` | `low` | `low` | `low` | `low` | `low` |
| 1. Bounded retrieval/docs/checklist | `medium` | `medium` | `low→medium` | `low→medium` | `low→medium` | `medium` |
| 2. Ordinary implementation from a complete spec | `high` | `high` | `medium` | `high`* | `medium` | `high` |
| 3. Cross-cutting debug/architecture | `high→xhigh` | `high→xhigh` | `high` | `xhigh` or Sol* | `high` | `high→xhigh` |
| 4. Long-horizon lead/frontier/quality-first | `xhigh` | `xhigh` | `xhigh` | prefer Sol* | `high` | `xhigh` |
| 5. Absolute maximum after positive eval | `max` | `max` | outside harness | outside harness | no such rung | no such rung |

The ladder is **INFERRED** except where it directly restates vendor level descriptions. `*` Luna-high and model escalation are deliberately low-confidence until harness evals establish whether a medium cliff or capability ceiling exists.

## What only an eval can decide

1. The actual adjacent marginal quality of every rung on the team’s repository—especially Sol, Luna, Gemini, and Grok, for which vendors provide no adjacent coding curve.
2. Whether Luna has a reproducible medium→high “effort cliff,” and whether Luna-high beats Sol-medium on success, latency, and total dollars.
3. Whether Fable/Opus `max` or any model’s xhigh reduces quality through over-verification, speculative refactoring, or failure to stop on the team’s task distribution.
4. Whether a leaner role preserves required constraints. Test instruction groups by ablation; do not compare a concise good prompt with a verbose contradictory one and call the result an effort effect.
5. Tool-call count, unnecessary reads/searches, real parallelism, wall-clock tail, and the fraction of tasks that gold-plate after acceptance criteria are met.
6. Cost per successful task, including retries, cached input, hidden reasoning, tool charges, failed runs, and CLI subscription/quota behavior—not merely API token price.
7. Whether low-first escalation wins once failure-detection false negatives and retry latency are included.
8. Model substitution: Luna-high versus Sol-medium, Fable-medium versus Opus-medium, and fast-model retries versus one stronger run.
9. Run-to-run variance. Agentic benchmarks need multiple seeds; adjacent differences smaller than the confidence interval are not decision-grade.
10. Snapshot/CLI mapping. Verify that each CLI version sends the intended effort value and that provider aliases have not changed.
11. Long-context effects, prompt-cache invalidation after effort changes, and safety/fallback routing that may change latency or model behavior.
12. Human-facing noise: review usefulness, comment precision, diff scope, and whether higher effort produces more prose rather than more correctness.

A minimal harness should stratify tasks by the seven matrix classes, run at least 3–5 seeds per model/effort cell, pin model snapshots and tool permissions, and score: acceptance-test pass rate, checklist recall/precision, unintended diff rate, wall-clock p50/p95, reasoning/output tokens, tool calls, retries, and total cost per accepted result. The 3–5 seed suggestion and metric design are **INFERRED methodology**, not a vendor prescription.

## Primary source register

### Anthropic

- [Effort](https://platform.claude.com/docs/en/build-with-claude/effort) — accessed 2026-08-28.
- [Thinking: steering and cost](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost) — accessed 2026-08-28.
- [Cost and intelligence optimization](https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence) — accessed 2026-08-28.
- [Prompting Fable 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5) — accessed 2026-08-28.
- [Prompting Opus 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5) — accessed 2026-08-28.
- [What is new for Opus 5](https://platform.claude.com/docs/en/about-claude/models/whats-new-claude-4-6) — accessed 2026-08-28; the vendor URL retains an older slug.
- [Claude Code model configuration](https://code.claude.com/docs/en/model-config) — accessed 2026-08-28.
- [Fable 5 launch](https://www.anthropic.com/news/claude-fable-5-mythos-5) — published 2026-06-09; updated 2026-07-01.
- [Opus 5 launch and effort charts](https://www.anthropic.com/news/claude-opus-5) — published 2026-07-24.

### OpenAI

- [Reasoning models guide](https://developers.openai.com/api/docs/guides/reasoning) — accessed 2026-08-28.
- [Latest model guidance](https://developers.openai.com/api/docs/guides/latest-model) — accessed 2026-08-28.
- [GPT-5.6 Sol model page](https://developers.openai.com/api/docs/models/gpt-5.6-sol) — accessed 2026-08-28.
- [GPT-5.6 Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna) — accessed 2026-08-28.

### Google

- [Antigravity headless CLI](https://antigravity.google/docs/cli/headless/) — accessed 2026-08-28.
- [Gemini thinking guide](https://ai.google.dev/gemini-api/docs/generate-content/thinking) — accessed 2026-08-28.
- [Gemini 3.7 Flash developer guide](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/guides/gemini-3-7-flash) — updated 2026-08-27.
- [Gemini 3.7 Flash launch](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-gemini-3-7-flash/) — published 2026-08-13.
- [Gemini 3.x prompting notes](https://ai.google.dev/gemini-api/docs/whats-new-gemini-3.5) — accessed 2026-08-28; application to 3.7 is marked INFERRED.

### xAI

- [Reasoning effort](https://docs.x.ai/developers/model-capabilities/text/reasoning) — accessed 2026-08-28.
- [Grok 4.6 model guide](https://docs.x.ai/developers/grok-4-6) — updated 2026-08-21.
- [Grok 4.6 launch and benchmarks](https://x.ai/news/grok-4-6) — published 2026-08-12.
- [xAI pricing](https://docs.x.ai/developers/pricing) — accessed 2026-08-28.

## Compact JSON summary

```json
{"as_of":"2026-08-28","ladder":{"claude-fable-5":{"mechanical":"low","bounded_review":"medium","spec_implementation":"high","architecture_lead":"xhigh","max":"eval-only"},"claude-opus-5":{"mechanical":"low","bounded_review":"medium","spec_implementation":"high","architecture_lead":"xhigh","max":"eval-only"},"gpt-5.6-sol":{"mechanical":"low","bounded_review":"low|medium","spec_implementation":"medium","architecture_debug":"high","long_horizon":"xhigh"},"gpt-5.6-luna":{"mechanical":"low","bounded":"medium","spec_implementation":"high (INFERRED, low confidence)","architecture_lead":"prefer Sol"},"gemini-3.7-flash":{"mechanical":"low","spec_implementation_review":"medium","architecture_lead":"high"},"grok-4.6":{"mechanical":"low","bounded_review":"medium","spec_implementation":"high","architecture_lead":"xhigh"}},"sources":{"anthropic":9,"openai":4,"google":5,"xai":4},"unsourced":["Sol/Luna/Gemini/Grok adjacent-effort coding deltas","Luna medium cliff","repository-specific task boundaries","xAI prompt-length interaction","per-task CLI cost and effort-to-parallelism mapping"]}
```
