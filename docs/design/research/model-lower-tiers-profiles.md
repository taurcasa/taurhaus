# Lower-Tier Model Profiles for the taurhaus Team Runtime

Research date: **2026-08-28**. Read-only study. Companion to the frontier-tier study.

**Confidence key**

- **[SRC]** — stated in a cited source (vendor doc, vendor blog, or measured third-party benchmark).
- **[INFERRED]** — my reasoning from cited numbers, not a vendor claim.
- **[ANECDOTE]** — single community report, unverified.

**Harnesses in scope** (as configured in taurhaus): Claude Code (`--model fable|opus|sonnet`), Codex CLI 0.149, Antigravity CLI 1.1.22, Grok CLI 1.0.5.

---

## 0. Cross-vendor snapshot

Artificial Analysis Intelligence Index (AA II) is the only metric that spans all four vendors on a common harness, so it anchors the table. Everything else is per-vendor.

| Model (effort) | AA II | Cost / AA II task | API $/1M in-out | Notes |
|---|---|---|---|---|
| Grok 4.6 | 60.9 | $0.837 | $2 / $6 | Grok CLI default [SRC] |
| GPT-5.6 Sol (max) | 59 | $1.04 | $5 / $30 | Codex frontier [SRC] |
| **Grok 4.5 (high)** | **56** | **$0.43** | **$2 / $6** | same token price as 4.6 [SRC] |
| **Gemini 3.7 Flash (high)** | **56** | — | $0.75 / $3.75 (promo) | **beats Gemini 3.1 Pro** [SRC] |
| **GPT-5.6 Terra (max)** | **55** | **$0.55** | **$2 / $12** | [SRC] |
| **GPT-5.6 Luna (max)** | **51** | **$0.21** | **$0.20 / $1.20** | [SRC] |
| GPT-5.6 Terra (high) | 50 | $0.23 | $2 / $12 | [SRC] |
| Gemini 3.1 Pro Preview | 48 | — | ~$1.74 blended | [SRC] |
| **GPT-5.6 Luna (medium)** | **39** | **$0.01** | $0.20 / $1.20 | [SRC] |
| Gemini 3.5 Flash | 35.8 | — | $1.50 / $9 | [SRC] |

Anthropic models are not on this table because AA II figures for Sonnet 5 / Opus 5 were not retrieved; Anthropic is covered on its own benchmarks in §7.

**Two results reframe the whole study:**

1. **Gemini 3.7 Flash (high) at AA II 56 is *more* intelligent than Gemini 3.1 Pro Preview at 48**, ~3x faster (324 vs 118 tok/s), and ~3x cheaper ($0.58 vs $1.74 per 1M blended) [SRC]. In the Antigravity harness the "pro" model is *not* the frontier option — the Flash line has overtaken it. Treating `gemini-3.1-pro-high` as the quality ceiling is wrong as of 2026-08.
2. **Grok 4.5 is faster, cheaper per task, and better at agentic coding than Grok 4.6** on LiveBench, despite 4.6 winning on raw intelligence [SRC]. The lesser Grok is genuinely preferable for a whole class of work.

---

## 1. GPT-5.6 Terra (Codex CLI)

### 1.1 Positioning and benchmarks

OpenAI's own model guidance describes the tiers as: Sol = "flagship capability", **Terra = "strong performance at a lower price"**, Luna = "efficient, high-volume workloads"; the bare `gpt-5.6` alias routes to Sol [SRC]. In the new naming scheme the number is the generation and Sol/Terra/Luna are durable capability tiers that advance independently [SRC].

Vendor and third-party numbers vs its frontier sibling:

| Benchmark | Terra | Sol | Gap |
|---|---|---|---|
| SWE-Bench Pro | 63.4% | 64.6% | −1.2 pts [SRC] |
| Terminal-Bench 2.1 | 87.4% | 88.8% | −1.4 pts [SRC] |
| AA Coding Agent Index | 77 | 80 | −3 pts [SRC] |
| AA Intelligence Index (max) | 55 | 59 | −4 pts [SRC] |
| Cost per AA II task | $0.55 | $1.04 | **−47%** [SRC] |

OpenAI did **not** report SWE-Bench Verified for this family [SRC]. Context window is 1,050,000 tokens with 128K max completion; knowledge cutoff 2026-02-16; released 2026-07-09 [SRC].

The vendor framing is that Terra has "competitive performance to GPT-5.5 while being 2x cheaper" and that each GPT-5.6 model pushes past GPT-5.5 on the Pareto frontier [SRC]. OpenAI's own docs do *not* claim Terra matches GPT-5.5 — that framing comes from third-party writeups [SRC].

**Effort-level caveat.** GPT-5.6 exposes `none | low | medium | high | xhigh | max`; the API default is `medium` [SRC]. taurhaus's Codex slice exposes `low | medium | high | xhigh`, so **the AA "max" numbers above are one notch above anything taurhaus can select**. The directly-comparable figure is Terra (high) = AA II **50** at **$0.23/task** [SRC]; Terra at `xhigh` lands between 50 and 55 [INFERRED]. Terra is also unusually token-frugal: 24M output tokens across the AA index against a 72M median, verbosity rank #18/187 [SRC].

### 1.2 Bucket economics in our setting

This is where Terra earns its place, and the numbers are from OpenAI's own Codex pricing doc. Codex uses **one shared 5-hour bucket that different models drain at different rates** — not separate buckets [SRC].

| Model | Plus, 5h local messages | Pro 20x | Credits /1M in | /1M cached | /1M out |
|---|---|---|---|---|---|
| GPT-5.6 Sol | 10–100 | 200–2,000 | 100 | 10 | 500 |
| **GPT-5.6 Terra** | **25–200** | **500–4,000** | **50** | **5** | **300** |
| GPT-5.6 Luna | 250–2,000 | 5,000–40,000 | 5 | 0.5 | 30 |
| GPT-5.5 | 15–80 | 300–1,600 | 125 | 12.5 | 750 |

"On ChatGPT plans, local messages and cloud chats share a five-hour window. Additional weekly limits may apply." [SRC]

**Terra buys roughly 2x–2.5x the message allowance of Sol inside the same window, at half the credit rate** [SRC]. It does not open a different bucket — it drains the same one at half speed. For a team runtime where several Codex members run concurrently against one ChatGPT plan, that is the single largest lever available short of Luna.

### 1.3 Task classes where Terra is good enough or preferable

Given a 1.2–1.4 point gap on both coding benchmarks and a 47% cost cut, Terra is the **default** for a Codex team member and Sol is the exception [INFERRED — the benchmark gaps are [SRC], the conclusion is mine].

Plausibly good enough, high confidence: mechanical sweeps and renames; test-hygiene fixes; docs drift verification; claim checking against a named file; PR triage; log analysis; research digests; code review under a narrow lens (one rule, one file set); QA/product checking against written acceptance criteria. [INFERRED]

Preferable to Sol: anything run **many times per hour** — a PR-triage member, a docs-drift watcher, a log-analysis sweeper. The message allowance dominates the quality gap at that duty cycle [INFERRED].

### 1.4 Steering notes

- OpenAI's migration advice is to keep your current effort as a baseline and **"test... one level lower"** [SRC]. Coming off Sol at `high`, try Terra at `high` first, not `xhigh` — the token-frugality means Terra at `high` may already match your Sol-at-`medium` habit.
- Terra's verbosity rank (#18/187, "very concise") means it will under-explain unless asked. For a review or QA lane, ask explicitly for the evidence line and the file:line citation, or you get a verdict with no trail [INFERRED].
- Do not benchmark it at `max` and then ship at `medium`. This is a general Codex trap and it applies hardest here because the AA gap between Terra (high) 50 and Terra (max) 55 is larger than the entire Terra-vs-Sol gap at matched effort [INFERRED from [SRC] numbers].

### 1.5 Failure modes that rule it out

- **Long-horizon multi-file implementation with self-correction.** Sol is specifically positioned for "tasks that need persistence across files, tests, and follow-up fixes" [SRC]; the AA Coding Agent Index gap (77 vs 80) is a composite of exactly those evals [SRC]. Do not put Terra on a build-the-feature-end-to-end lane.
- **Anything where the −4 AA II points land on the reasoning-hard part** (architecture decisions, subtle concurrency bugs) [INFERRED].

---

## 2. GPT-5.6 Luna (Codex CLI)

### 2.1 Positioning and benchmarks

OpenAI: Luna is for **"efficient, high-volume workloads"** [SRC]; third-party framing is "the fastest and lowest-cost tier... a low-reasoning lane for high-volume work where speed and cost are the main constraints" [SRC].

| Metric | Luna (max) | Luna (medium) | Sol (max) |
|---|---|---|---|
| AA Intelligence Index | 51 | **39** | 59 [SRC] |
| AA Coding Agent Index | 75 | — | 80 [SRC] |
| Cost per AA II task | $0.21 | **$0.01** | $1.04 [SRC] |
| Output speed | — | 110.5 tok/s | — [SRC] |
| Time to first token | — | 2.65 s | — [SRC] |

Luna carries the same 1M-token context class as the rest of the family [SRC].

**The effort cliff is the headline.** Luna at `max` scores 51 — only 4 points under Terra (max) and 8 under Sol, on the same eval. Luna at `medium` collapses to **39** [SRC]. That is a bigger drop than any model-to-model gap in this study. The `medium` default is *not* the thing the "Luna is 75 on Coding Agent Index" story is about.

**Price.** On 2026-07-30 OpenAI cut Luna's API price by 80% (from $1/$6 to **$0.20/$1.20** per 1M) and Terra's by 20% ($2.50/$15 → $2/$12), three weeks after the 2026-07-09 launch [SRC]. Sol was unchanged at $5/$30 [SRC].

### 2.2 Bucket economics

The most dramatic entry in the Codex table: **Luna is 5 credits/1M input and 30 credits/1M output against Sol's 100 and 500 — a 20x and 16.7x discount — and the 5-hour message allowance is 250–2,000 on Plus against Sol's 10–100, roughly 25x** [SRC].

For a taurhaus member that runs continuously (a log tailer, a docs-drift checker, a PR-triage bot), Luna is effectively free against the shared bucket relative to Sol [INFERRED]. This is the single biggest quota lever in any of the four harnesses.

### 2.3 Task classes where Luna is good enough

Grounded: OpenAI's own guidance is `none` as a latency baseline, `low` for latency-sensitive workloads, `medium` as a balanced start [SRC]; and OpenAI's tier description is high-volume routine work [SRC].

Good enough, **at `high` or `xhigh` effort** [INFERRED]:

- **Mechanical sweeps and renames** — deterministic, verifiable by the compiler and `just check-quick`. The cost of an error is a red test, not a shipped bug.
- **Log analysis** — taurhaus's JSONL sink is structured; this is pattern extraction over a long, low-ambiguity stream. Luna's 1M context and $0.20/1M input make whole-log passes affordable.
- **PR triage** — label, route, summarize. Wrong routing is cheap to fix.
- **Docs drift verification** — "does `CLAUDE.md` still describe what `cli_tool.rs` does" is a comparison task, not a reasoning task, *if* you hand it both texts.
- **Research digests** — summarization of supplied material.

Marginal: claim checking, test-hygiene fixes, narrow-lens code review [INFERRED]. These need the model to notice an *absence*, which is where the low-reasoning tier is weakest.

### 2.4 Steering notes — the important ones

- **Never run Luna below `high` for anything a human will trust.** AA II 39 at `medium` vs 51 at `max` [SRC] is the whole argument. The cost saving from dropping effort is real ($0.01 vs $0.21/task) but you are buying a materially different model.
- **Give it the text, don't make it find the text.** With input at $0.20/1M, pasting the whole file into the prompt costs almost nothing and removes the search step, which is where a low-reasoning tier drifts [INFERRED].
- **Structure the output contract explicitly.** A JSON schema or a fixed table format, not "write me a report". Lower tiers hold format better than they hold judgment [INFERRED].
- **More examples, shorter tasks.** Two or three worked examples in the prompt, and one concern per invocation, rather than a multi-part brief [INFERRED].
- Cached input is 0.5 credits/1M [SRC] — a stable system prompt across a sweep is nearly free. Design the sweep so the varying part is last.

### 2.5 Failure modes that rule it out

- **Any task where the deliverable is a judgment the team acts on without re-checking.** The 8-point AA II gap to Sol at matched top effort, and the 20-point gap at default effort [SRC], both land on judgment.
- **Multi-hop verification** — "check whether this claim in the changelog is true" where the answer requires chaining across three files. [INFERRED]
- There is one unverified community report of "a serious regression in instruction following and reliability" on Luna (OpenAI Developer Community case #13739760), with no OpenAI staff response and no technical validation [ANECDOTE]. I would not weight it, but it argues for a smoke test before you put Luna on an unattended lane.

---

## 3. GPT-5.5 (Codex CLI)

**Recommendation: do not use it in this harness. It is strictly dominated.**

- **It costs more credits than Sol.** GPT-5.5 is 125 credits/1M input and 750/1M output; Sol is 100 and 500 [SRC]. It is *2.5x Terra's* credit rate (50 / 300) [SRC].
- Its 5-hour message allowance (15–80 on Plus) is in the same band as Sol's (10–100) and a fraction of Terra's (25–200) [SRC].
- On capability, "each new GPT-5.6 model pushes past GPT-5.5 on the Pareto frontier (excluding non-reasoning)" and Terra offers "GPT-5.5-class performance at less than half Sol's price" [SRC].

So GPT-5.5 is *at best* Terra-equivalent in quality while costing 2.5x Terra's credits and yielding a third of Terra's messages [INFERRED from [SRC] numbers]. There is no task class in a software team where it beats Terra on this harness.

The only reasons to keep it selectable: reproducing an old result, or a prompt that was tuned against 5.5's exact behavior and has not been re-validated [INFERRED]. OpenAI's own migration doc offers no workload where 5.5 remains preferable [SRC].

---

## 4. Gemini 3.7 Flash at medium / low (Antigravity CLI)

### 4.1 Positioning and benchmarks

Google's model card (released 2026-08-13, 1M in / 64K out, knowledge cutoff March 2026) positions 3.7 Flash for **"agentic workflows, coding tasks, and enterprise workflows"** [SRC]. It does **not** publish guidance comparing Flash to Pro-tier [SRC].

Against the previous Flash generation:

| Benchmark | 3.7 Flash | 3.6 Flash |
|---|---|---|
| FrontierCode (production code quality) | **43.6%** | 34.4% |
| DeepSWE v1.1 (long-horizon SWE) | **65.3%** | 48.6% |
| Terminal-bench 2.1 (agentic coding) | **85.8%** | 78.0% |
| AutomationBench (enterprise workflow) | **30.4%** | 17.0% |
| GDM-MRCR v2 (long context, 128k avg) | **97.0%** | 91.8% |

[SRC — Google DeepMind model card]. WebDev Arena 1588 Elo, the highest of any Flash-tier model [SRC].

Against 3.1 Pro, the model taurhaus treats as the Antigravity frontier: **3.7 Flash (high) wins on AA II (56 vs 48), speed (324 vs 118 tok/s), time-to-first-token (9.93s vs 33.74s), and price ($0.58 vs $1.74 per 1M blended)** [SRC]. Gemini 3.1 Pro's published 80.6% SWE-Bench Verified and 68.5% Terminal-Bench 2.0 are on different benchmark versions and harnesses, so they are not directly comparable [SRC].

### 4.2 The effort question — where the real uncertainty is

Antigravity exposes Low / Medium / High thinking for **all three Flash generations**, but **Gemini 3.1 Pro only at High** [SRC — Antigravity models doc].

The critical gap in the public record: **essentially every published 3.7 Flash number, including the AA II 56, carries a "(high)" label** [SRC]. Google publishes no per-thinking-level benchmark table [SRC]. Documented behavior is only qualitative:

- higher level = more reasoning tokens = better on hard tasks, worse on cost and latency [SRC]
- **medium is the API default and "provides the best quality for most tasks"** [SRC]
- low targets latency-critical work: incident response, real-time chat, drafts, fast data analysis [SRC]
- the explicit warning: "test at the thinking level you actually plan to ship, not the one that produces the best headline number" [SRC]

So: `gemini-3.7-flash-medium` is the vendor's own default and their claimed quality sweet spot [SRC], but there is **no published number quantifying the medium-vs-high drop** [SRC]. Treat any specific claim about that gap as unmeasured [INFERRED].

### 4.3 Bucket economics in our setting

Google's Antigravity docs confirm **two separate quota pools**: the UI shows "Weekly Limit Remaining" and "Five Hour Limit Remaining" *separately* for Gemini models and for Claude/GPT models [SRC]. Ultra gets "highest, most generous quota, refreshed every five hours"; Pro gets "high, generous quota, refreshed every five hours until weekly limit reached"; Free gets "meaningful quota, refreshed weekly" [SRC]. Google publishes no numeric limits [SRC].

Crucially for us: the plans page treats **all Gemini models — 3.1 Pro, 3.5 Flash, and the rest — as a single Gemini pool** [SRC]. So switching an Antigravity member from `gemini-3.1-pro-high` to `gemini-3.7-flash-medium` **does not move to a different bucket**. What it does buy: Google states rate limits are "correlated with the amount of work done by the agent" [SRC], so a model that is 3x faster and finishes in fewer tokens should drain the shared Gemini pool more slowly [INFERRED].

A community claim that Flash and Pro have *separate* rate limits circulates but **is not in Google's documentation** — treat as unconfirmed [ANECDOTE].

The **real** bucket lever in Antigravity is the other direction: routing a member to `gemini-*` instead of Antigravity's Claude/GPT models spends the Gemini pool and preserves the Claude+GPT pool entirely [SRC]. In a taurhaus team where the UI-specialist role runs on Antigravity, keeping it on Gemini keeps the third-party pool free for whatever needs it.

### 4.4 Task classes

`gemini-3.7-flash-high` is not a lower tier at all — on the evidence it is the strongest Antigravity option and should be the default for any Antigravity member [INFERRED from [SRC] benchmarks].

`gemini-3.7-flash-medium`: vendor-default, claimed best-quality-for-most-tasks [SRC]. Reasonable for docs drift verification, claim checking, research digests, PR triage, QA against acceptance criteria, narrow-lens code review [INFERRED]. Its 97.0% GDM-MRCR v2 at 128k [SRC] makes the *high* setting genuinely strong for long-context sweeps; whether medium holds that is unmeasured [INFERRED].

`gemini-3.7-flash-low`: Google names the fit — latency-critical pipelines, drafts, fast data analysis [SRC]. In our setting that maps to log analysis and mechanical sweeps where a human or a test gates the output [INFERRED]. Its 340 tok/s class speed [SRC] makes it the right choice when a member is in a tight interactive loop with a human.

### 4.5 Steering notes

- Because Antigravity's own frontier option (3.1 Pro) is capped at High-only [SRC], the *thinking level* is the only quality dial on Flash — and the only one you actually control. Set it deliberately per role, not globally.
- Google's explicit instruction is to evaluate at the shipped thinking level [SRC]. If you promote a role from 3.1 Pro to 3.7-flash-medium, re-run its acceptance checks; do not assume the headline 56 transfers.
- Antigravity has **no compaction hook** in taurhaus (`compaction_hook: false`), so a long-running Flash member loses context without a reinjection card. Prefer shorter, self-contained assignments for Antigravity roles regardless of tier [INFERRED from the repo's harness model].

### 4.6 Failure modes

- Any claim about medium/low quality is currently unfalsifiable from public data [SRC]. Do not put a Flash-medium member on an unattended lane without your own eval.
- Google publishes no numeric quota, so a Flash member can exhaust the shared Gemini pool and take 3.1 Pro down with it [SRC + INFERRED].

---

## 5. Gemini 3.6 Flash and 3.5 Flash (Antigravity CLI)

**Recommendation: neither has a role. Both are dominated by 3.7 Flash.**

**3.5 Flash is dominated by 3.6 Flash on both axes.** 3.6 Flash is **$0.75/$3.75** per 1M through 2026-12-31 against 3.5 Flash's **$1.50/$9** — half the input price and less than half the output — while scoring higher on DeepSWE, SWE-Bench Pro and MLE-Bench, running faster (304 vs 289 tok/s), using 17% fewer output tokens on the AA index, and adding built-in computer use [SRC]. Computer use went 78.4% → 83.0% on OSWorld-Verified and GDM-MRCR long-context went 26.6% → 54.0% [SRC]. Gemini 3.5 Flash sits at AA II 35.8 [SRC] — the lowest number in this entire study.

**3.6 Flash is in turn dominated by 3.7 Flash.** They carry the *same* promotional price ($0.75/$3.75 through 2026-12-31, then $1.50/$7.50) [SRC], and 3.7 wins every published benchmark, several by wide margins: FrontierCode 43.6 vs 34.4, DeepSWE 65.3 vs 48.6, Terminal-bench 78.0 → 85.8, AutomationBench 17.0 → 30.4, GDM-MRCR 91.8 → 97.0 [SRC].

Since all Gemini models share one Antigravity quota pool [SRC], there is not even a bucket argument for the older generations — they cost the same pool for less capability, and slower work drains a work-correlated pool *harder* [INFERRED].

Keep them selectable only for reproducing a prior result or bisecting a behavior regression [INFERRED]. `gemini-3.5-flash-*` in particular should be considered retired for team-member use.

---

## 6. Grok 4.5 vs Grok 4.6 (Grok CLI)

### 6.1 Positioning and benchmarks

xAI released Grok 4.6 on 2026-08-12, a month after 4.5, as continued training of 4.5 rather than a new pretrain, with extra attention on agent tasks [SRC]. xAI positions 4.6 for "long-running agents and more ambitious interactive and visual work", strong at "turning a broad product idea into a working first version", with "more self-testing and verification" on longer tasks [SRC].

xAI's own table:

| Benchmark | Grok 4.6 | Grok 4.5 |
|---|---|---|
| AA Intelligence Index | 61 | 56 |
| GDPVal-AA v2 | 1753 | 1526 |
| CursorBench v3.2 | 69.9% | 66.7% |
| DeepSWE v1.1 | 65.9% | 54% |
| FrontierCode v1.1 | 61.3% | 56.6% |
| APEX-Agents | 57.5% | 47.1% |

[SRC — x.ai/news/grok-4-6]

**But the independent picture is much less one-sided, and in one place it reverses:**

- **Agentic coding regressed.** LiveBench puts Grok 4.6 agentic coding at **54.2 against Grok 4.5's 56.5**, while non-agentic coding improved by over 6 points [SRC]. That regression sits directly against xAI's own long-running-agent framing.
- **Time to first token more than tripled**: 8.7s on 4.5 → **31.2s** on 4.6 [SRC]. AA measures Grok 4.5 (high) at 14.86s TTFT and 51.6 tok/s [SRC].
- **Cost per task more than doubled**: AA's standard task set costs **$0.837 on 4.6 vs $0.360 on 4.5** — a 2.32x increase driven by ~47% more output tokens plus a cache price rise [SRC]. AA's own Grok 4.5 (high) page reports $0.43/task [SRC]; the $0.360 figure is the generation-comparison number [SRC].
- Vals AI measures 4.6 at 95.60% SWE-bench, ~9 points above 4.5 [SRC] — single-shot code generation is where 4.6's gains are real.

### 6.2 Price and quota in our setting

**Per-token price is identical.** Both are $2/1M input and $6/1M output below 200K tokens, doubling to $4/$12 for any request at or above 200K, both with a 500K context window. The only difference is cached input: **$0.30/1M on 4.5 vs $0.50/1M on 4.6** — the lesser model is *cheaper* on cache reads [SRC].

So **Grok 4.5's cost advantage is entirely token efficiency, not rate.** At the same $/token, 4.6 burns ~47% more output tokens per task [SRC]. In a subscription setting that maps directly to the shared pool.

On the subscription side: paid Grok plans use **one shared weekly usage pool across all Grok products** — chat, image, video, voice, coding and multi-agent all draw from the same weekly pool, with extra credits purchasable and usage visible in Settings → Usage [SRC]. Consumer-app Grok 4.5 usage is metered by **message count, not tokens** [SRC]. xAI does not publish fixed numeric quotas [SRC]. The developer API is a completely separate account, credit pool, and rate limit — a SuperGrok subscription grants no programmatic allowance [SRC].

**Implication for taurhaus:** there is no separate bucket to move to. But if metering is by message and the pool is shared across every Grok product, the lever is turns-per-task, and 4.5 finishing in fewer tokens and 3.5x faster TTFT is worth real pool [INFERRED].

### 6.3 Effort levels — a harness detail that matters

xAI's docs: `low` = "latency-sensitive agentic use and simple tool calling"; `medium` = "complex data analysis and long-context reasoning"; `high` (the default) = "very challenging problems, complex math, multi-step logic"; `xhigh` = "the hardest problems, where answer quality matters more than response time" [SRC].

**grok-4.5 supports only low/medium/high. "On models that do not support it, such as grok-4.5, requests with 'xhigh' are treated as 'high'"** — silently [SRC]. This matches taurhaus's registry note. It means a role configured `grok-4.5 + xhigh` is silently running at `high`; the launch renderer should not imply otherwise, and a reviewer comparing a `4.6 xhigh` run to a `4.5 xhigh` run is comparing xhigh to high [INFERRED].

### 6.4 Task classes where Grok 4.5 is good enough or preferable

**Preferable to 4.6, on evidence:**

- **Agentic / tool-loop work in the Grok CLI** — LiveBench agentic coding 56.5 vs 54.2 [SRC]. This is the one place in the whole study where a vendor's newer model is measurably *worse* at the thing taurhaus actually does with it.
- **Anything interactive with a human in the loop** — 8.7s vs 31.2s TTFT [SRC] is the difference between a usable and an unusable side-panel companion.
- **High-frequency, repeated invocations** — 2.32x cheaper per task at identical token rates [SRC].

**Good enough** [INFERRED]: docs drift verification, claim checking, mechanical sweeps, test-hygiene fixes, PR triage, log analysis, narrow-lens code review, research digests, QA against acceptance criteria. AA II 56 puts 4.5 level with Gemini 3.7 Flash (high) and above Terra (max).

**Where 4.6 earns its cost:** single-shot code generation and bug fixing (LiveBench coding +6, Vals SWE-bench +9 [SRC]) and knowledge work — 4.6 takes first place on every knowledge-work benchmark in xAI's table, winning GDPval-AA v2, AA-Briefcase, and the Harvey legal eval [SRC]. Also long-horizon efficiency: 4.6 finished long agentic tasks in ~53 turns against Claude Opus 5's 103 [SRC].

### 6.5 Steering notes and failure modes

- Grok is the one harness in taurhaus with **no usage meter** (`usage: false` in the registry) — you cannot see the pool draining. That raises the value of the cheaper-per-task model independent of any benchmark [INFERRED].
- Grok's compaction delivery is `MeshInbox`, not `HookStdout`, and its passive-hook stdout is ignored [repo]. A long Grok session is more fragile across compaction than a Claude or Codex one, which argues for shorter assignments on either Grok model [INFERRED].
- **Rules out 4.5:** research/knowledge-work digests where 4.6 sweeps the vendor's benchmarks [SRC]; single-shot "write this function / fix this bug" work [SRC].

### 6.6 Note on "grok 3.6"

Resolved as a typo for **grok-4.6** per the user. No existence check performed. **Grok 4.5 is the lesser Grok** and is the comparison used throughout §6.

---

## 7. Claude Sonnet (alias `sonnet`) vs Opus 5 (Claude Code)

### 7.1 What `sonnet` resolves to, and price

The current Anthropic lineup is Haiku 4.5, **Sonnet 5**, Opus 5, and Fable 5 / Mythos 5 [SRC]. In Claude Code, `/model` lists what your account can reach and Anthropic notes "exact model names, versions, and availability change over time" [SRC]; the `sonnet` alias resolves to Sonnet 5 [INFERRED — the alias table published by Anthropic covers fable/opus/mythos explicitly].

| Model | ID | Context | $/1M in | $/1M out |
|---|---|---|---|---|
| Claude Fable 5 | `claude-fable-5` | 1M | $10 | $50 |
| Claude Opus 5 | `claude-opus-5` | 1M | $5 | $25 |
| **Claude Sonnet 5** | `claude-sonnet-5` | 1M | **$2** | **$10** |

[SRC — Anthropic model table, cached 2026-06-24]. One source reports $2/$10 as introductory through 2026-08-31 with $3/$15 standard after; another reports the intro rates became permanent [conflicting SRC]. Today is within the intro window either way — **verify before budgeting past 2026-08-31** [INFERRED].

Sonnet 5 is thus **2.5x cheaper than Opus 5 and 5x cheaper than Fable 5** per token [SRC].

### 7.2 Positioning and benchmarks

Anthropic's own Claude Code guidance is unusually direct and is the strongest vendor statement in this entire study:

> **"Sonnet ... is the default and is the right choice for the large majority of coding work"**; Opus "offers deeper reasoning for harder problems"; Haiku is "the fastest and cheapest option." Allocation: **"Sonnet for most coding (features, tests, known bugs, refactors); Opus when you're genuinely stuck or the change is wide"**, and **"plan with Opus, execute with Sonnet."** [SRC]

On the model page, Anthropic positions Sonnet 5's performance as "close to that of Opus 4.8, but at lower prices", with a "wider range of cost-performance options than Sonnet 4.6" that "in some cases matches Opus 4.8's capability levels" [SRC]. Anthropic explicitly steers *away* from Sonnet for one domain: "we recommend Claude Opus 4.8 for cybersecurity work that requires reduced guardrails" [SRC].

Measured, vs Opus 4.8 (Anthropic published **no** SWE-bench Verified number for Opus 5, so a direct Sonnet-5-vs-Opus-5 coding comparison is not possible from public data [SRC]):

| Benchmark | Sonnet 5 | Opus 4.8 | Sonnet 4.6 |
|---|---|---|---|
| SWE-Bench Pro | 63.2 | 69.2 | 58.1 |
| **Terminal-Bench 2.1** | **80.4** | 74.6 | 67.0 |
| Humanity's Last Exam (tools) | 57.4 | 57.9 | 46.8 |
| OSWorld-Verified (computer use) | 81.2 | 83.4 | 78.5 |
| **GDPval-AA v2 (knowledge work)** | **1,618** | 1,615 | — |
| SWE-bench Verified | 72.7 | 79.4 | 62.3 |

[SRC]. Sonnet 5 **beats** Opus 4.8 on Terminal-Bench 2.1 (+5.8) and ties/edges it on GDPval-AA v2 knowledge work, while trailing by ~6 points on SWE-Bench Pro and ~6.7 on SWE-bench Verified [SRC]. Opus 5 sits above Opus 4.8 on the newer agentic and knowledge-work suites [SRC], so widen the Sonnet-to-Opus-5 gap accordingly [INFERRED].

### 7.3 Bucket economics — the part that surprises

Claude Code enforces a rolling 5-hour window plus weekly caps: **one weekly cap across all models, plus a separate weekly cap specific to Sonnet** [SRC]. On Max, Sonnet and Opus draw from separate 5-hour and weekly buckets and the dashboard shows two weekly bars; on Pro they share one pool [SRC].

**But the Sonnet weekly cap is an additional ceiling, not a separate spend pool.** A filed Claude Code issue (#57875) reports that Sonnet usage drains **both** the "All Models" weekly bucket *and* the "Weekly Sonnet" bucket simultaneously, leaving the user blocked with 78% of the Sonnet quota remaining because All Models hit 100%. The issue was closed as *not planned / duplicate*, with related reports #12487, #57050, #14362, #23690, and no staff explanation in the thread [SRC / ANECDOTE for the mechanism]. Treat the Sonnet bar as a *sub-limit*, not an escape hatch [INFERRED].

**What Sonnet actually buys is a lower drain rate on the shared weekly budget.** Third-party analysis puts it at roughly **Opus 5 consuming the cap ~5x faster than Sonnet 5 for equivalent tasks** [SRC — not an Anthropic figure]. That is directionally consistent with the 2.5x per-token price gap plus Opus's higher token spend at matched effort [INFERRED].

**Fable is the genuinely different bucket.** Through 2026-07-07 Fable 5 was included in Pro/Max/Team for up to 50% of weekly usage limits; **from 2026-07-08 Fable no longer draws from subscription limits at all and bills through usage credits** — except Max, Team Premium and Enterprise premium seats, which include it up to 50% of weekly limits; Pro and Team Standard get it only via credits, softened by a one-time $100 credit [SRC]. Anthropic's Claude Code models article does not mention Fable at all [SRC].

**So the taurhaus-specific ranking of levers on the Claude side is:** moving a member from Opus 5 → Sonnet 5 stretches the *same* weekly budget roughly 5x [SRC]. Moving work onto Fable spends a *different* pool (credits, or the Max 50% carve-out) and is the only true bucket switch — but it is the most expensive model, so it is a bucket switch you pay for [SRC + INFERRED].

Also note: usage limits are shared across Claude and Claude Code — "all activity in both tools counts against the same usage limits" [SRC]. Max $100 ≈ 5x Pro; Max $200 ≈ 10x [SRC]. Weekly limits have run 50% above standard on paid plans since 2026-05-13, extended on 2026-08-19 through **2026-08-31** [SRC] — that boost expires in three days, which will make the Sonnet lever materially more valuable from September.

### 7.4 Task classes

Anthropic's own list, verbatim, is the recommendation: **features, tests, known bugs, refactors** on Sonnet; Opus "when you're genuinely stuck or the change is wide" [SRC].

Mapped onto the task classes in scope, all Sonnet-appropriate [INFERRED, on Anthropic's guidance plus the Terminal-Bench win]:

docs drift verification · claim checking · mechanical sweeps and renames · test-hygiene fixes · QA against acceptance criteria · research digests · PR triage · narrow-lens code review · log analysis.

Sonnet 5's Terminal-Bench 2.1 = 80.4, above Opus 4.8 [SRC], makes it *first choice*, not a fallback, for terminal-driven agentic sweeps.

### 7.5 Steering notes

- **The prompt shape matters more than the model here.** Anthropic's "plan with Opus, execute with Sonnet" [SRC] is a workflow instruction: give Sonnet a written plan and it performs near Opus; give it an open-ended brief and you are paying for the gap. In a taurhaus role template, that means Sonnet roles want a filled `definition_of_done` and `required_artifacts`, not creative latitude [INFERRED].
- **Effort matters as much as model.** Sonnet 5 "at low/medium effort" is the value sweet spot; "at xhigh it can cost more than Opus 4.8 for similar quality" [SRC]. A Sonnet role pinned to xhigh throws away the entire cost argument.
- Sonnet 5 does **not** support mid-conversation system messages (Opus 5, Opus 4.8, Fable 5 and Mythos 5 do) [SRC]. If a taurhaus lane injects operator instructions mid-run, that path differs on Sonnet.

### 7.6 Failure modes that rule it out

- **Wide changes across many modules** — Anthropic's own carve-out [SRC], consistent with the ~6-point SWE-Bench Pro gap [SRC].
- **Security review with reduced guardrails** — Anthropic explicitly recommends Opus [SRC]. This directly affects the `/security-audit` lane taurhaus runs at phase boundaries: **do not move that role to Sonnet.**
- **"Genuinely stuck" debugging** — the case Anthropic reserves for Opus [SRC].

---

## 8. Where the cheaper model spends a *different* bucket

Summarizing §1.2, §4.3, §6.2, §7.3, because this is the question with the least intuitive answer:

| Harness | Does the lesser model spend a different bucket? | What it actually buys |
|---|---|---|
| **Codex CLI** | **No — one shared 5h bucket** [SRC] | Drains it far slower. Terra ≈ 2–2.5x Sol's messages at half the credit rate; Luna ≈ 25x Sol's messages at 1/20 input, 1/16.7 output credits [SRC]. **Biggest lever in the fleet.** |
| **Antigravity CLI** | **No within Gemini** — all Gemini models share one pool; Gemini vs Claude/GPT are two pools [SRC] | Faster/cheaper models drain a work-correlated pool more slowly [INFERRED]. The real bucket switch is Gemini ↔ Claude/GPT, not Flash ↔ Pro. |
| **Grok CLI** | **No — one shared weekly pool across all Grok products** [SRC] | Identical $/token; 4.5 wins purely on token efficiency (2.32x cheaper per task) and 3.5x faster TTFT [SRC]. No usage meter in taurhaus, so this is unobservable. |
| **Claude Code** | **Partly.** Sonnet has its own weekly cap on Max but still drains All Models [SRC/ANECDOTE]. **Fable is the one true separate bucket** — usage credits, or a 50% carve-out on Max/premium seats [SRC] | Opus→Sonnet stretches the *same* weekly budget ~5x [SRC]. The +50% limit boost expires 2026-08-31 [SRC], raising the value of this lever. |

---

## 9. Task class × recommended cheaper model

"Frontier" here means the harness's top option: Sol, `gemini-3.7-flash-high`, grok-4.6, Opus 5 / Fable 5.

| Task class | Codex | Antigravity | Grok | Claude | Reasoning | Confidence |
|---|---|---|---|---|---|---|
| **Docs drift verification** (does the doc still describe the code) | **Luna @ high** | 3.7-flash-medium | **4.5** | **Sonnet 5** | Comparison of two supplied texts, not reasoning. Luna at high scores AA II 51 [SRC] and costs 1/5 of Sol; hand it both texts rather than making it search. | High |
| **Claim checking** (is this assertion true of the repo) | **Terra @ high** | 3.7-flash-medium | **4.5** | **Sonnet 5** | Needs to notice absence and chain 2–3 files. Luna's low-reasoning lane is where absence-detection is weakest [INFERRED]; Terra costs half of Sol at −1.2 SWE-Bench Pro [SRC]. | Medium |
| **Mechanical sweeps / renames** | **Luna @ high** | 3.7-flash-low | **4.5** | **Sonnet 5** | Deterministic and compiler-verified; `just check-quick` is the gate. Error cost is a red test. Luna's 25x message allowance dominates [SRC]. | High |
| **Test-hygiene fixes** | **Terra** | 3.7-flash-medium | **4.5** | **Sonnet 5** | Anthropic names "tests" in Sonnet's own remit [SRC]; needs enough judgment to not delete the assertion that was doing the work. | High |
| **QA / product check vs written acceptance criteria** | **Terra** | 3.7-flash-medium | **4.5** | **Sonnet 5** | Criteria are supplied, so it is verification not design. Sonnet 5 leads Opus 4.8 on GDPval-AA v2 knowledge work (1,618 vs 1,615) [SRC]. | Medium-High |
| **Research digests** | **Luna @ high** | 3.7-flash-medium | **4.6** | **Sonnet 5** | Summarization of supplied material — the classic cheap-tier win. Grok is the exception: 4.6 sweeps every knowledge-work benchmark in xAI's table [SRC]. | Medium |
| **PR triage** (label, route, one-line summary) | **Luna @ high** | 3.7-flash-low | **4.5** | **Sonnet 5** | High volume, cheap to correct, format-bound. Fix the output schema and Luna holds it [INFERRED]. | High |
| **Narrow-lens code review** (one rule, one file set) | **Terra** | 3.7-flash-medium | **4.5** | **Sonnet 5** | Narrowing the lens is what makes a cheaper tier viable; the tier that fails is the open-ended "review this PR". Terra's terseness needs an explicit evidence-line contract [INFERRED]. | Medium-High |
| **Log analysis** (taurhaus JSONL sink) | **Luna @ high** | 3.7-flash-low | **4.5** | **Sonnet 5** | Structured, low-ambiguity, long. Luna at $0.20/1M input makes whole-log passes affordable; 3.7 Flash holds 97.0% GDM-MRCR v2 at 128k (at *high*) [SRC]. | High |
| **Interactive side-panel companion** (human in loop) | Terra | **3.7-flash-low** | **4.5** | Sonnet 5 | Latency is the product. Grok 4.5 TTFT 8.7s vs 4.6's 31.2s [SRC]; Flash runs 324 tok/s vs 3.1 Pro's 118 [SRC]. | High |
| **Long-horizon feature implementation** | **none — Sol** | none — 3.7-flash-high | none — 4.6 | **none — Opus 5** | Sol is positioned exactly for "persistence across files, tests, and follow-up fixes" [SRC]; Anthropic reserves Opus for "when the change is wide" [SRC]. | High |
| **Architecture decisions / design proposals** | **none — Sol** | none — 3.7-flash-high | none — 4.6 | **none — Opus 5 / Fable 5** | The −4 to −8 AA II points land squarely on judgment [INFERRED]. | High |
| **Security audit** (`/security-audit` lane) | **none — Sol** | none — 3.7-flash-high | none — 4.6 | **none — Opus 5** | Anthropic explicitly recommends Opus for cybersecurity work needing reduced guardrails [SRC]. Hard rule. | High |
| **"Genuinely stuck" debugging** | **none — Sol** | none — 3.7-flash-high | **4.6** | **none — Opus 5** | Anthropic's own carve-out [SRC]. Grok inverts: 4.6 is +9 on Vals SWE-bench for single-shot bug fixing [SRC]. | High |
| **Agentic tool-loop orchestration** | Terra | 3.7-flash-high | **4.5** ⚠ | **Sonnet 5** | The one reversal in the study: Grok 4.6 *regressed* on LiveBench agentic coding (54.2 vs 4.5's 56.5) [SRC]. Sonnet 5 beats Opus 4.8 on Terminal-Bench 2.1 (80.4 vs 74.6) [SRC]. | Medium-High |
| **Single-shot code generation** ("write this function") | Terra | 3.7-flash-medium | **4.6** | Sonnet 5 | Where 4.6's gains are real: LiveBench coding +6, Vals SWE-bench +9 [SRC]. | Medium |

**Models with no recommended role at all:** `gpt-5.5` (costs more credits than Sol, capability below Terra [SRC]); `gemini-3.5-flash-*` (AA II 35.8, 2x the price of 3.6 Flash [SRC]); `gemini-3.6-flash-*` (same price as 3.7 Flash, loses every published benchmark [SRC]).

---

## 10. Cross-cutting steering notes for lower tiers

Grounded where cited, otherwise [INFERRED] from the pattern across all four vendors.

1. **Effort dominates model choice below the frontier.** Luna at `max` (51) beats Terra at `high` (50) [SRC]. Choosing "cheaper model, top effort" usually beats "better model, low effort" at equal spend. Every vendor says some version of this — OpenAI ("test one level lower"), Google ("test at the thinking level you actually plan to ship") [SRC].
2. **Never publish a benchmark at one effort and ship at another.** Google states this explicitly for 3.7 Flash [SRC]; the Luna medium/max cliff (39 → 51) [SRC] shows the cost of ignoring it.
3. **Buy back capability with context, not reasoning.** Cheap input ($0.20/1M Luna, $0.30/1M cached Grok 4.5) means pasting the file beats making the model find it [SRC prices, INFERRED tactic].
4. **Fix the output contract.** Lower tiers hold *format* far better than they hold *judgment*. A JSON schema or fixed table costs nothing and removes a failure mode [INFERRED].
5. **One concern per invocation.** Split a multi-part brief into separate runs; the message-allowance economics make this nearly free on Terra/Luna and it removes the class of failure where a cheaper tier answers part 1 and forgets part 3 [INFERRED].
6. **Demand the evidence line.** Terra is verbosity rank #18/187, "very concise" [SRC]. A verdict without `file:line` is not checkable, and checkability is the entire reason a cheaper tier is acceptable for verification work [INFERRED].
7. **Match the tier to the harness's compaction story.** Antigravity has no compaction hook and Grok delivers via MeshInbox rather than hook stdout [repo]; both argue for shorter assignments regardless of tier, and short assignments are exactly where cheap tiers are strongest [INFERRED].
8. **Watch for silent effort downgrades.** `grok-4.5` + `xhigh` silently runs as `high` [SRC]. Any A/B across Grok generations at xhigh is measuring xhigh against high.

---

## Sources

**OpenAI / Codex**
- Codex pricing and rate limits (primary): https://learn.chatgpt.com/docs/pricing
- OpenAI model guidance (primary): https://developers.openai.com/api/docs/guides/latest-model
- OpenAI reasoning guide: https://developers.openai.com/api/docs/guides/reasoning
- GPT-5.6 launch: https://openai.com/index/gpt-5-6/ *(403 to automated fetch; content reached via search index)*
- Price-performance update: https://openai.com/index/advancing-the-price-performance-frontier-with-gpt-5-6/ *(403 to automated fetch)*
- Artificial Analysis, GPT-5.6: https://artificialanalysis.ai/articles/gpt-5-6-has-landed
- Artificial Analysis, Terra (high): https://artificialanalysis.ai/models/gpt-5-6-terra-high
- Artificial Analysis, Luna (medium): https://artificialanalysis.ai/models/gpt-5-6-luna-medium
- OpenRouter, GPT-5.6 Terra: https://openrouter.ai/openai/gpt-5.6-terra
- VentureBeat on the Luna price cut: https://venturebeat.com/technology/ai-price-wars-openai-cuts-gpt-5-6-luna-prices-by-80-as-model-competition-shifts-toward-cost
- Community regression report [ANECDOTE]: https://community.openai.com/t/case-13739760-gpt-5-6-luna-a-serious-regression-in-instruction-following-and-reliability/1392979

**Google / Antigravity**
- Gemini 3.7 Flash model card (primary): https://deepmind.google/models/model-cards/gemini-3-7-flash/
- Antigravity models (primary): https://antigravity.google/docs/models/
- Antigravity plans / quotas (primary): https://antigravity.google/docs/plans/
- Gemini API latest-model notes: https://ai.google.dev/gemini-api/docs/latest-model
- Artificial Analysis comparison: https://artificialanalysis.ai/models/comparisons/gemini-3-7-flash-vs-gemini-3-1-pro-preview
- Artificial Analysis, Gemini 3.5 Flash: https://artificialanalysis.ai/models/gemini-3-5-flash

**xAI / Grok**
- Grok 4.6 announcement (primary): https://x.ai/news/grok-4-6
- xAI reasoning effort levels (primary): https://docs.x.ai/developers/model-capabilities/text/reasoning
- Grok 4.6 model docs: https://docs.x.ai/developers/grok-4-6
- Grok 4.6 vs 4.5 analysis: https://emergent.sh/learn/grok-4-6-benchmarks
- Artificial Analysis, Grok 4.5: https://artificialanalysis.ai/models/grok-4-5
- Grok usage pool / SuperGrok limits: https://ai-x.chat/guide/grok-usage-limits/

**Anthropic / Claude Code**
- Models, usage and limits in Claude Code (primary): https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code
- Using Claude Code with Pro or Max (primary): https://support.claude.com/en/articles/11145838-using-claude-code-with-your-pro-or-max-plan
- Claude Sonnet 5 announcement (primary): https://www.anthropic.com/news/claude-sonnet-5
- Anthropic model IDs and aliases (primary): https://github.com/anthropics/skills/blob/main/skills/claude-api/shared/models.md
- Sonnet 5 benchmarks: https://www.vellum.ai/blog/claude-sonnet-5-benchmarks-explained
- Weekly-limit structure and Fable credits: https://www.explainx.ai/blog/claude-usage-limits-2026-timeline-explained
- Claude Code limits analysis: https://www.morphllm.com/claude-code-usage-limits
- Sonnet dual-bucket drain report [ANECDOTE]: https://github.com/anthropics/claude-code/issues/57875

Pricing table for Anthropic models is from the bundled `claude-api` skill's model table (cached 2026-06-24).
