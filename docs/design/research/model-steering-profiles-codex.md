# Steering profiles for coding-agent role descriptions

**Research date:** 2026-08-28  
**Harnesses in scope:** Claude Code 2.1.2xx, Codex CLI 0.149, Antigravity CLI 1.1.22, and Grok CLI 1.0.5, with the model names and effort choices in the request.  
**Purpose:** writing role-description text that behaves like a durable system prompt for an interactive coding teammate—not writing one-off task prompts.

## Reading this report

- Vendor-primary documentation is the evidence base. Every source entry gives its URL and either its vendor date or the access date when the page exposes no publication date.
- **INFERRED** marks synthesis, operational advice, cross-model comparison, or harness behavior not stated directly by the cited vendor source. It does not mean “low confidence”; it means “not a direct vendor claim.”
- The CLI is part of behavior. A role should describe semantic boundaries, but filesystem/network/action enforcement belongs in the CLI's permission, sandbox, and tool configuration. Repeating a hard security policy only in prose is not equivalent to enforcement.
- Vendor documentation is generally a rolling current snapshot, not an archive keyed to every CLI patch. **INFERRED:** Unless a source explicitly names the requested CLI version, its model semantics are the closest official evidence rather than proof of wrapper behavior in Claude Code 2.1.2xx, Codex CLI 0.149, Antigravity CLI 1.1.22, or Grok CLI 1.0.5.
- Current API documentation sometimes exposes effort settings beyond the named harness. The profiles use only the effort values in the request and call out the difference where useful.
- No independent benchmark run was performed. The final task-class table is deliberately a hypothesis, not a ranking.

## Claude Fable 5

### Profile

**Recommended role shape.** **INFERRED:** Use a short Markdown role with: mission; owned outcomes; exact scope/non-scope; irreversible-action boundary; evidence and verification standard; definition of done; communication cadence; final-response style. Prefer goals and constraints over a prescribed implementation recipe. Add ordered steps only where order is a real invariant. Give the full desired outcome at the start and let the model choose the path.

**Instruction detail, length, and structure.** Anthropic says Fable 5 follows instructions better and usually needs only brief direction; legacy, heavily scaffolded skills can reduce performance. Anthropic's general Claude guidance says to be clear and explicit, explain why constraints exist, use sequential steps when order or completeness matters, use consistent XML tags for complicated mixed content, and put long reference material before the query. Claude Code recommends concise, structured `CLAUDE.md` files with Markdown headings and bullets and warns that longer files reduce adherence. **INFERRED:** For a role, Markdown is the natural default in Claude Code; reserve XML for clearly delimited embedded policy, examples, or reference material. Remove duplicated rules and “reason through these N steps” scripts.

**Tone, definition of done, and examples.** State observable output qualities—“outcome first, complete sentences, name files/tests/remaining risk”—rather than a vague persona adjective. Define completion as externally checkable acceptance criteria plus evidence from actual tools. Anthropic generally recommends three to five relevant, diverse examples when examples are needed, enclosed consistently, but Fable 5's brief-instruction guidance argues against adding examples by default. **INFERRED:** Start zero-shot; add one or a small set of examples only to correct a measured style/format failure.

**Autonomy, check-ins, and implementation bias.** Fable is explicitly designed for long-horizon autonomous work and can operate for many minutes or hours. It may also take useful but unrequested actions, such as refactoring adjacent code or creating backup artifacts. Write the role so it proceeds through local, reversible, in-scope work; pauses only for destructive/irreversible actions, a real scope change, or information only the user can provide; and distinguishes “assess/review” from “fix/change.” Anthropic warns that Fable can occasionally state intent without making the tool call and can make progress claims not grounded in tool results. Require it to continue until done or genuinely blocked and to tie progress/completion claims to observed results.

**Tools, planning, and delegation.** Let it inspect first, form a lightweight plan internally, execute, and verify. Name relevant tools and action boundaries, but avoid long compulsory tool sequences. Anthropic says newer Claude models respond literally to wording such as “suggest” versus “implement” and can over-trigger tools under older aggressive `MUST use` prompts. Fable readily delegates. **INFERRED:** In a team role, authorize delegation only for sizeable independent work and require the lead to integrate and verify; otherwise Fable's willingness to delegate can turn a small task into coordination overhead.

**Effort semantics in this harness.** Anthropic documents five API effort levels, with `high` the default. For Fable, `low`/`medium` target routine or latency-sensitive work, `high` is the general default, and `xhigh` is for the hardest tasks; higher effort can increase thinking, text, tool calls, latency, and cost. Fable's model-specific guide says lower settings often beat the prior model at `xhigh`, while higher effort can cause excess information gathering or deliberation. The requested Claude invocation does not specify effort, so **INFERRED:** treat `claude --model fable` as the installation's default effort behavior (documented by Anthropic as high at the model/API level), not as a role-controlled constant. Use `xhigh` only through an explicit harness setting and only for hard architecture, deep research, cross-repository debugging, or other tasks where an evaluated quality gain justifies long turns.

**Verbosity and user-facing copy.** Anthropic says Fable can produce dense, technical final answers after long work. Put the final-copy contract in the role: outcome first; concise by default; complete sentences; evidence and unresolved risks; no arrow-chain shorthand. A short final reminder can re-ground it after a long agent trajectory.

**Failure modes to guard against.** Scope creep and unrequested cleanup; fabricated or premature progress claims; rare early stopping before a tool call; over-delegation; context-window countdown anxiety; overlong information gathering at high effort; temporary artifacts; over-engineering; hard-coding to tests; claims about code not inspected; and safety refusals in some benign cyber/bio work. Do not ask it to reproduce hidden reasoning: Anthropic says reasoning-extraction requests can trigger refusal. Ask for a concise decision rationale and evidence instead.

### Evidence with URLs

- [Prompting Claude Fable 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5) — accessed 2026-08-28. Exact-model guidance on autonomy, brief instructions, checkpoints, scope expansion, progress grounding, early stopping, context countdown, delegation, final-answer density, safety/refusal behavior, and legacy scaffolding.
- [Claude prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices) — accessed 2026-08-28. General Claude guidance on explicit instructions, context, examples, XML, long-context order, action bias, tool wording, verification, subagents, and common coding-agent failures.
- [Effort](https://platform.claude.com/docs/en/build-with-claude/effort) — accessed 2026-08-28. Effort levels, defaults, and effects on reasoning, output, tool use, latency, and cost.
- [Fable 5 migration guide](https://platform.claude.com/docs/en/models/fable-5/migration-guide) — accessed 2026-08-28. Exact migration mechanics and the direct comparison baseline, Opus 4.8.
- [Manage Claude's memory / `CLAUDE.md`](https://code.claude.com/docs/en/memory) — accessed 2026-08-28. Concise, structured project-instruction guidance and reduced adherence with long files.

### What a v4 role should change vs a v3 role written for the predecessor

Anthropic's exact Fable 5 material compares Fable with Opus 4.8, not Opus 4.6. **INFERRED:** The 4.6-to-Fable bridge below combines the documented Fable-vs-4.8 changes with the fact that a 4.6-era role predates them; it is not a vendor-published 4.6 migration table.

- Delete detailed “always plan, then do steps 1–12, then self-check twice” scaffolding. Retain the goal, constraints, risk boundary, acceptance criteria, and evidence requirement.
- Replace blanket “ask before acting” with a compact action policy: proceed on local/reversible/in-scope work; ask on destructive, shared, irreversible, or materially different scope.
- Add an explicit assess-versus-implement boundary because Fable is more action-oriented and may improve things beyond the request.
- Replace generic persistence language with two targeted rules: ground status in tool output, and keep working unless completion or a genuine user-only dependency is reached.
- Cap delegation and background work by task size; require integration ownership.
- Add final-copy calibration. A legacy role that relied on the model to stay conversational may yield dense, technical handoffs after long runs.
- Remove any request for verbatim hidden reasoning. Ask for conclusions, tradeoffs, evidence, and a short rationale.

## Claude Opus 5

### Profile

**Recommended role shape.** **INFERRED:** Give Opus 5 the whole specification up front in compact Markdown: role and mission; exact deliverable; constraints/non-goals; risk/approval boundary; acceptance criteria; communication cadence; final-answer contract. It is strong enough that a role should define ownership and boundaries, not simulate an implementation playbook.

**Instruction detail, length, and structure.** Anthropic says Opus 5 works with Opus 4.8 prompts but is better at completing full tasks when given the complete specification. General guidance favors clear, explicit instructions and reasons, ordered steps only when order matters, consistent XML for complex prompts, and long context before the query. Claude Code favors concise Markdown instruction files. **INFERRED:** Use headings and bullets for the standing role, with XML only around data/examples/policy whose boundaries genuinely matter. Long instruction sets can work, but duplication and legacy self-check machinery waste Opus 5's stronger reasoning and can create over-verification.

**Tone, definition of done, and examples.** Opus 5 is more literal about review severity: an instruction such as “only report severe issues” can suppress valid findings. Define what to collect and what to present separately. State concrete completion conditions, but do not force repeated self-check loops—Anthropic says Opus 5 self-verifies by default. Anthropic recommends positive examples over negative prohibitions and says a small tone reminder near the end of the prompt is effective. **INFERRED:** Examples are most valuable for calibrated review findings and exact final-copy shape; rules are better for safety and scope.

**Autonomy, check-ins, and implementation bias.** Opus 5 is optimized for complex, long-horizon coding and can complete full implementations rather than stubs. It can broaden task scope and tends to delegate. Tell it to proceed autonomously on the specified interpretation, ask only when different readings would materially change the outcome, and never mutate when the user requested only analysis/review. Explicitly define destructive/external/shared-state approval points. Do not add frequent mandatory check-ins; specify sparse progress updates by milestone or time interval if the interface needs them.

**Tools, planning, and delegation.** Give the full spec, ask for a proportionate plan, then let it run. Remove redundant “double-check everything” commands. Anthropic says Opus 5 catches and fixes mistakes and that extra double-check instructions can add cost without benefit. It delegates more readily; Claude Code 2.1.217 and later add deterministic subagent caps and a `claude_code` preset. **INFERRED:** Because “2.1.2xx” spans patches, do not assume those caps exist unless the exact build is at least 2.1.217. Put only the semantic delegation boundary in the role and enforce concurrency in the harness where supported.

**Effort semantics in this harness.** Anthropic documents `low`, `medium`, `high` (default), `xhigh`, and `max`; the requested invocation does not select one. `low` and `medium` can retain strong accuracy with lower latency; `high` is the default; `xhigh` is for demanding work. Effort changes thinking, not visible response length. **INFERRED:** Use low/medium for bounded edits, triage, conventional reviews, and latency-sensitive interaction; high for architecture, complex feature work, or ambiguous debugging; xhigh only when the task is genuinely hard and the extra delay is acceptable. Keep thinking enabled: Anthropic warns that disabling it can cause malformed visible tool/XML behavior.

**Verbosity and user-facing copy.** Opus 5 is more verbose than prior Claude models, in chat and authored documents. Effort does not solve that. Put explicit length and cadence constraints in the role, using observable choices: “lead with the outcome; two short paragraphs unless risk warrants more; no narration of routine tool calls; preserve nuance in reviews.” For long-form deliverables, give a word/section budget or exemplar.

**Failure modes to guard against.** Overlong answers and documents; narration of routine work; literal suppression of review findings; scope expansion; unnecessary delegation; unnecessary extra verification; and tool-call leakage if thinking is disabled. General Claude coding risks still apply: temporary files, speculative claims about unread code, over-engineering, and test-specific hacks.

### Evidence with URLs

- [Prompting Claude Opus 5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5) — accessed 2026-08-28. Exact-model guidance on complete specs, full-task completion, reviews, effort, verbosity, tone reminders, self-verification, scope, delegation, and thinking/tool-call behavior.
- [What's new in Claude Opus 5](https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5) — accessed 2026-08-28. Exact-model changes, context/output scale, thinking, effort sensitivity, and direct Opus 4.8 baseline.
- [Claude prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices) — accessed 2026-08-28. General structure, examples, XML, long-context, tools, autonomy, and coding-agent guidance.
- [Effort](https://platform.claude.com/docs/en/build-with-claude/effort) — accessed 2026-08-28. Effort semantics and the distinction between thinking effort and visible length.
- [Manage Claude's memory / `CLAUDE.md`](https://code.claude.com/docs/en/memory) — accessed 2026-08-28. Claude Code instruction-file structure and length guidance.
- [Optimizing for cost and intelligence](https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence) — accessed 2026-08-28. Anthropic's current workload-selection and relative cost guidance.

### What a v4 role should change vs a v3 role written for the predecessor

Anthropic's exact Opus 5 guidance compares it with Opus 4.8, not Opus 4.6. **INFERRED:** These changes are the safest update for an Opus-4.6-era role, but the direct vendor evidence covers the nearer 4.8 baseline.

- Move the complete deliverable and acceptance criteria up front; remove instructions that invite stubs, incremental permission-seeking, or a stop after planning.
- Remove explicit repeated self-check and double-verification loops. Keep one outcome-based acceptance test and require evidence.
- Separate review recall from presentation severity: collect all supported issues, then rank/filter what the user sees.
- Add explicit response/document length and narration cadence; old implicit brevity expectations are wrong for Opus 5.
- Replace “ask whenever ambiguous” with “ask when plausible interpretations materially change the result”; otherwise choose a reasonable in-scope interpretation and proceed.
- Add scope and delegation caps. Opus 5 is more willing to expand and fan out.
- Do not disable thinking merely to gain speed; lower effort instead.

## OpenAI gpt-5.6-sol

### Profile

**Recommended role shape.** OpenAI's GPT-5.6 guide says the model understands intent better and should receive domain context, hard constraints, approval boundaries, and success criteria rather than every implementation step. **INFERRED:** For Sol, use the leanest role that fully expresses those five things: ownership; desired outcome; relevant environment/context; non-negotiable constraints; action/approval policy; definition of done; terse communication style.

**Instruction detail, length, structure, and over-constraint.** GPT-5.6 specifically benefits from removing repeated instructions, examples, and irrelevant tools. OpenAI reports a roughly 10–15% internal coding-agent evaluation improvement in some prompt simplifications, with major token/cost reductions, while warning that the figures are directional rather than universal. Codex concatenates instruction files from global to local scope and applies closer instructions later, with a default combined instruction limit of 32 KiB. **INFERRED:** Use ordinary Markdown headings and bullets; OpenAI does not prescribe XML for this model. Keep each rule once, resolve contradictions, and put task-specific detail near the task rather than inflating the standing role. Over-constraint most often manifests as unnecessary approval requests, rigidity, and longer trajectories—not better control.

**Tone, definition of done, and examples.** OpenAI says GPT-5.6 is more concise than GPT-5.5 and recommends describing tone as observable writing choices instead of broad labels such as “friendly.” Define success criteria explicitly and require proportional validation. Remove examples unless they represent a product requirement or repair an evaluated gap. **INFERRED:** A good Sol definition of done is outcome-based—requested behavior works, relevant checks pass, no unauthorized scope, evidence reported—not a mandatory step list.

**Autonomy, check-ins, and implementation bias.** Use OpenAI's compact autonomy split: answer/diagnose means inspect and report; change/fix means mutate and validate; external, destructive, costly, or scope-expanding actions require confirmation. Repeating “ask first,” “do not mutate,” and “wait for approval” throughout the role can cause needless check-ins. **INFERRED:** Sol is the best OpenAI tier here for wide autonomy, but that makes the read-only versus change boundary essential. Do not force a visible plan for every task; require a plan only when the work is multi-step, high risk, or ambiguous.

**Tools.** Expose only relevant tools and describe them precisely. Require source/tool evidence for current or repository-specific claims, and verification after mutations. **INFERRED:** Avoid compulsory “call tool X before tool Y” rules except where the dependency is real. Sol can choose the route; the role should govern authority and outcomes.

**Effort semantics in this harness.** The API supports more levels, but the named CLI harness exposes `low|medium|high|xhigh`. OpenAI describes low as latency-oriented, medium as the balanced default, and high/xhigh as worthwhile when measurement shows a quality gain. Higher effort adds reasoning, latency, and output cost. **INFERRED:** Low for bounded edits, retrieval, summaries, and simple diagnosis; medium for normal feature work and review; high for complex debugging, architecture, and cross-cutting changes; xhigh for the hardest proofs, investigations, large migrations, or deeply coupled agent plans. Sol at xhigh is not a default—it is a quality-first, high-latency choice.

**Verbosity and user-facing copy.** GPT-5.6 is natively concise. A blanket “be concise” can make it too terse; use a concrete copy contract, for example “outcome first, then evidence and remaining risk; explain unfamiliar decisions; omit routine narration.” **INFERRED:** Sol should produce the strongest OpenAI-tier reasoning-to-copy handoff, but polished prose still needs style requirements when it is itself the deliverable.

**Failure modes to guard against.** Excess approval-seeking caused by repeated conservative language; terse handoffs that omit rationale; prompt bloat and conflicting inherited instructions; unnecessary tool exposure; role examples that fossilize one workflow; and spending high/xhigh reasoning on routine tasks. **INFERRED:** As the flagship tier, Sol may also solve beyond the requested scope unless the outcome and non-goals are explicit.

### Evidence with URLs

- [Using GPT-5.6](https://developers.openai.com/api/docs/guides/latest-model) — accessed 2026-08-28. Exact-model guidance on intent, lean prompts, autonomy, success criteria, approval boundaries, tone, concision, examples, tools, effort, and migration from GPT-5.4-era prompting.
- [gpt-5.6-sol model page](https://developers.openai.com/api/docs/models/gpt-5.6-sol) — accessed 2026-08-28. Exact tier capability, context/output limits, cutoff, effort support, and current API pricing.
- [Compare models](https://developers.openai.com/api/docs/models/compare) — accessed 2026-08-28. Current Sol/Terra/Luna positioning and price comparison.
- [Models](https://developers.openai.com/api/docs/models) — accessed 2026-08-28. OpenAI's current selection guidance by task complexity and cost sensitivity.
- [Codex model configuration](https://learn.chatgpt.com/docs/models) — accessed 2026-08-28. Codex model selection and effort labels.
- [`AGENTS.md` instruction discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md) — accessed 2026-08-28. Instruction precedence, concatenation, and default size limit.
- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) — accessed 2026-08-28. Agent description as role guidance and relevant model/effort configuration surface.

### What a v4 role should change vs a v3 role written for the predecessor

OpenAI's GPT-5.6 guide directly frames migration from older GPT-5.x behavior, including GPT-5.4-era prompts.

- Delete repeated rules, long procedural workflows, redundant examples, and irrelevant tool instructions. GPT-5.6's stronger intent understanding makes that old scaffolding counterproductive.
- Replace step-level micromanagement with domain context, hard constraints, approval boundaries, and measurable success criteria.
- Consolidate permission language into one action matrix. Repetition can create excessive check-ins.
- Recalibrate copy: remove generic brevity commands if the handoff becomes under-explained; specify the actual information and style required.
- Use examples only for a measured format/style need, not as default prompt furniture.
- Re-run effort sweeps. Do not carry forward the assumption that predecessor-quality work always needs high/xhigh.

## OpenAI gpt-5.6-terra

### Profile

**Recommended role shape.** Terra is OpenAI's balanced capability/cost tier. The family guidance is the same as Sol: context, constraints, approval boundary, success criteria, and minimal duplication. **INFERRED:** Make Terra's role only slightly more explicit about decomposition and verification than Sol's: name the deliverable, the first evidence to inspect, the scope boundary, and the checks required, while still leaving implementation choice to the model.

**Instruction detail, length, structure, and over-constraint.** Use compact Markdown headings and bullets. Codex's layered instruction discovery and 32 KiB default combined limit make inherited duplication a practical risk. The GPT-5.6 guide reports better internal agent outcomes from removing repeated instructions/examples/tools. **INFERRED:** Terra benefits from a short “inspect → change if authorized → validate → report” control loop, but not a task-specific step list embedded in every role. Over-constraint can turn a cost-balanced model into a slow, approval-heavy agent.

**Tone, definition of done, and examples.** Specify visible style choices and an acceptance checklist. Keep examples only when a concrete output contract cannot be expressed compactly or an evaluation shows drift. **INFERRED:** Terra's DoD should be narrower than Sol's on very open-ended tasks: identify the minimum complete in-scope result and explicitly report optional follow-ons rather than implementing them.

**Autonomy, check-ins, planning, and tools.** Apply the same read/report versus change/validate distinction and confirm only external/destructive/costly/scope-expanding actions. Expose only relevant tools. **INFERRED:** Let Terra act autonomously on ordinary repo work; ask it to form a short plan for cross-cutting changes and to update the plan only when evidence changes. Do not require user confirmation between ordinary steps.

**Effort semantics in this harness.** Low minimizes latency, medium is the balance point, and high/xhigh spend more reasoning and time. **INFERRED:** Terra-medium should be the default hypothesis for routine coding-agent work; Terra-high for multi-file debugging, architecture-sensitive features, or careful review; Terra-xhigh only after an evaluation shows it closes a real quality gap. At xhigh, consider whether Sol at medium/high is the better total-quality trade.

**Verbosity and user-facing copy.** The GPT-5.6 family is concise. **INFERRED:** Terra is a good default for clean engineering handoffs if the role asks for outcome, evidence, and risks. Add explicit audience and length when producing product copy or documentation; do not expect effort to set prose length.

**Failure modes to guard against.** The family risks—over-approval, inherited prompt conflicts, overly terse final answers, excess tools—plus **INFERRED:** shallower handling of unusually ambiguous or globally coupled tasks than Sol. Guard with a trigger to escalate uncertainty, not with permanent maximal process.

### Evidence with URLs

- [Using GPT-5.6](https://developers.openai.com/api/docs/guides/latest-model) — accessed 2026-08-28. Family-level prompt, autonomy, tone, tool, effort, and migration guidance.
- [Compare models](https://developers.openai.com/api/docs/models/compare) — accessed 2026-08-28. Terra's documented balance positioning, shared context/cutoff, and current API price.
- [Models](https://developers.openai.com/api/docs/models) — accessed 2026-08-28. Current task-class selection guidance.
- [Codex model configuration](https://learn.chatgpt.com/docs/models) — accessed 2026-08-28. CLI model/effort semantics.
- [`AGENTS.md` instruction discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md) — accessed 2026-08-28. Layering and combined instruction limit.
- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) — accessed 2026-08-28. Agent role-description configuration.

No official page located in this research documents a Terra-specific prompt syntax, prose personality, permission response, or failure taxonomy distinct from the GPT-5.6 family. **INFERRED:** The tier-specific recommendations above derive from OpenAI's “balanced” positioning and must be validated in the named Codex 0.149 harness.

### What a v4 role should change vs a v3 role written for the predecessor

- Apply the GPT-5.6 family migration: remove GPT-5.4-era duplicated workflow scaffolding and replace it with intent, boundaries, context, and success criteria.
- Collapse repeated approval language into one decision rule.
- Add a compact minimum-complete scope so the balanced tier does not spend effort on attractive optional work.
- Tune effort empirically; begin at medium instead of preserving a predecessor-era high/xhigh default.
- Keep one proportionate validation contract, not multiple self-review passes.
- Add an explicit final handoff schema if the family's native concision omits the reasoning the team needs.

## OpenAI gpt-5.6-luna

### Profile

**Recommended role shape.** Luna is OpenAI's high-volume, cost-sensitive GPT-5.6 tier. **INFERRED:** Give it the most concrete—but still compact—role of the three: one primary responsibility, sharply bounded inputs/outputs, a small decision policy, exact acceptance checks, and a fixed handoff format. Avoid assigning an open-ended “senior engineer for anything” identity.

**Instruction detail, length, structure, and over-constraint.** The family still benefits from lean prompts and no duplication. **INFERRED:** Luna may benefit from a short ordered checklist when the workflow is stable and genuinely invariant, but a long exception tree will consume context and increase brittle literal behavior. Prefer Markdown; split different roles rather than creating one enormous polymorphic role.

**Tone, definition of done, and examples.** Use concrete observable tone requirements. OpenAI says examples should remain only for a product requirement or measured gap. **INFERRED:** Luna is the tier where one compact positive example can be most useful for a repeated classifier, formatter, or code-transformation role—provided it is representative and does not replace acceptance criteria.

**Autonomy, check-ins, planning, and tools.** Keep the family autonomy matrix. **INFERRED:** Authorize Luna to execute clear, reversible, local tasks without check-ins; require escalation when it cannot establish the needed context or when the change crosses the named module/interface. Give it the fewest relevant tools. For simple tasks, skip formal planning; for multi-step tasks, require a short plan and a done check rather than deep exploration.

**Effort semantics in this harness.** Low is latency-oriented, medium balanced, high/xhigh deeper and slower. **INFERRED:** Luna-low is appropriate for formatting, extraction, simple mechanical edits, and fast triage; medium for bounded bug fixes and test-backed changes; high only for tasks still within Luna's narrow role but requiring careful local reasoning. Luna-xhigh is likely a poor default economic choice: benchmark it against Terra/Sol at lower effort.

**Verbosity and user-facing copy.** The family is concise. **INFERRED:** Luna is well suited to short, templated status and handoff copy. For nuanced user communication, policy explanation, or persuasive prose, specify an exemplar/audience or route to a stronger tier.

**Failure modes to guard against.** GPT-5.6 family over-approval and terseness, plus **INFERRED:** loss of nuance on broad ambiguous tasks, premature commitment to the obvious local fix, and brittle adherence to overlong exception lists. Bound the role and provide escalation conditions instead of adding more process.

### Evidence with URLs

- [Using GPT-5.6](https://developers.openai.com/api/docs/guides/latest-model) — accessed 2026-08-28. Family prompt, tool, autonomy, tone, effort, and migration guidance.
- [Compare models](https://developers.openai.com/api/docs/models/compare) — accessed 2026-08-28. Luna's cost-sensitive positioning, shared context/cutoff, and current API price.
- [Models](https://developers.openai.com/api/docs/models) — accessed 2026-08-28. OpenAI's current high-volume task positioning.
- [Codex model configuration](https://learn.chatgpt.com/docs/models) — accessed 2026-08-28. CLI effort semantics.
- [`AGENTS.md` instruction discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md) — accessed 2026-08-28. Layering and instruction cap.
- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) — accessed 2026-08-28. Agent role descriptions.

No official page located in this research documents Luna-specific steering behavior beyond family guidance and tier positioning. **INFERRED:** All Luna-specific behavioral and copy-quality hypotheses require harness evaluation.

### What a v4 role should change vs a v3 role written for the predecessor

- Remove GPT-5.4-era prompt bulk and duplicated examples, even though Luna is the smaller tier; smaller does not mean “feed it every possible branch.”
- Narrow ownership and make the interface/acceptance checks concrete.
- Preserve a short stable checklist only where the task really is repetitive; otherwise state the outcome and constraints.
- Add explicit escalation triggers for ambiguity and cross-module effects.
- Start effort at low/medium and route genuinely hard work up a model tier instead of reflexively choosing Luna-xhigh.
- Give the final response a small schema so concision does not become omission.

## Google gemini-3.7-flash

### Profile

**Recommended role shape.** Google's Gemini 3 guide says prompts should be direct, concise, well structured, and explicit about the task and constraints. **INFERRED:** Use a compact system role with identity/ownership, task class, constraints, autonomy and permissions, Plan/Execute/Validate expectations, definition of done, and output style. Put critical role behavior first. For long task context, supply the data/code first and the specific current request at the end.

**Instruction detail, long prompts, structure, and over-constraint.** Google allows either XML tags or Markdown headings but says to choose one and use it consistently. Gemini 3 can overanalyze verbose or complicated legacy prompts. Google's agentic guide nevertheless provides a detailed evaluated scaffold for logical dependencies, risk, adaptability, persistence, and permissions. **INFERRED:** Distill that scaffold into short standing rules for a role; use the full template only where an evaluation proves that complex policy adherence needs it. Over-constraint can increase analysis, latency, loops, and rigid formatting.

**Tone, definition of done, and examples.** Gemini 3 is direct and efficient by default; explicitly request conversational or detailed prose. Google's general prompt guide strongly recommends specific, varied few-shot examples and warns that too many can overfit. **INFERRED:** For a coding role, rules should define authority and acceptance; examples should teach exact output/copy shape. Define done as the requested outcome plus validation and every required edge case—not “a plan was produced.”

**Autonomy, check-ins, planning, and tools.** Google's agent template explicitly distinguishes low-risk exploration from high-risk state changes, recommends using available optional parameters rather than asking unnecessarily, and asks the agent to adapt when observations contradict the plan. Antigravity 1.1.22's best-practice page recommends explicit explore/plan/execute phases for complex changes and calls a local verification loop the most effective reliability mechanism. Its review/permission controls are a separate layer. **INFERRED:** Tell Flash to inspect and proceed on reversible in-scope work, ask on destructive/external/scope-expanding work, and keep the plan lightweight for ordinary work. Use CLI permissions for enforceable “ask before X” controls. Require a state recap and done check after tool failures to reduce repetitive tool loops.

**Effort semantics in this harness.** Gemini 3.7 Flash supports `low`, `medium` (default), and `high`; the model page documents a 1M input context and 64K output. Google positions low for latency-critical drafts/chat/data and simple work, medium for most work including complex coding and agents, and high for maximum thinking/tool use on difficult math, coding, and agent tasks. Higher levels increase thinking, latency, and cost. **INFERRED:** Low for quick transforms, simple edits, and interactive lookup; medium for normal implementation/review; high for hard multi-tool debugging, long-context synthesis, or high-cost-of-error changes. Because 3.7 improved first-pass agent reliability, retry cost—not only per-call cost—should be measured.

**Verbosity and user-facing copy.** Default copy is terse/direct. Explicitly request audience, tone, length, and degree of explanation. **INFERRED:** Flash is strong for high-throughput concise copy and code-adjacent summaries; it needs a style example or clear rubric for nuanced editorial voice.

**Failure modes to guard against.** Verbose prompts causing overanalysis; repeated tool calls/loops after losing state or hitting an unavailable endpoint; unnecessary visible chain-of-thought/plan requests; low-temperature loops; stale-year or cutoff errors in time-sensitive search; few-shot overfitting; and omission caused by default concision. Use current date in the harness, preserve function-call IDs/thought signatures across turns, and leave sampling at vendor defaults unless evaluated.

### Evidence with URLs

- [What's new in Gemini 3.7 Flash](https://ai.google.dev/gemini-api/docs/latest-model) — last updated 2026-08-26 UTC; accessed 2026-08-28. Exact-model capabilities, tier purposes, agent reliability, and migration requirements.
- [Gemini 3.7 Flash model page](https://ai.google.dev/gemini-api/docs/models/gemini-3.7-flash) — last updated 2026-08-13 UTC; accessed 2026-08-28. Exact context/output and supported thinking levels.
- [Gemini 3 developer guide](https://ai.google.dev/gemini-api/docs/gemini-3) — last updated 2026-08-26 UTC; accessed 2026-08-28. Direct prompting, verbosity, long context, thinking, sampling, and migration guidance.
- [Prompt design strategies](https://ai.google.dev/gemini-api/docs/prompting-strategies) — last updated 2026-06-10 UTC; accessed 2026-08-28. Structure, examples, agentic planning/risk/persistence/permission template, tools, and output control.
- [Troubleshooting guide](https://ai.google.dev/gemini-api/docs/troubleshooting) — accessed 2026-08-28; page exposes no publication date. Tool-loop, repetition, thinking-output, and sampling troubleshooting.
- [Antigravity CLI headless mode and model selection](https://www.antigravity.google/docs/cli/headless/) — accessed 2026-08-28; page exposes no publication date. Documented model/effort names and CLI execution behavior.
- [Antigravity CLI best practices](https://antigravity.google/docs/cli/best-practices/) — accessed 2026-08-28; documentation section identifies CLI v1.1.22. Verification loops, explore/plan/execute for complex work, rule files, permissions, recovery, and subagents.
- [Antigravity rules and workflows](https://antigravity.google/docs/rules-workflows?app=cli) — accessed 2026-08-28; page exposes no publication date. Markdown rule files, workflows, and rule-size behavior.
- [Antigravity custom subagents](https://www.antigravity.google/docs/subagents/) — accessed 2026-08-28; page exposes no publication date. Role body as system prompt, Markdown/YAML structure, tools, model choice, and command policy.

The Antigravity pages reviewed are explicitly filed under CLI v1.1.22. **INFERRED:** Its `--effort` labels are treated here as the model thinking levels documented by the Gemini API; the CLI page exposes the exact labels but does not state the lower-layer wire mapping.

### What a v4 role should change vs a v3 role written for the predecessor

Google's 3.7 guide gives migration instructions from Gemini 3.6/3.5/3 Flash and 3.1 Pro; the broader Gemini 3 guide also covers migration from 2.5.

- Simplify older verbose chain-of-thought and prompt-engineering scaffolds. Do not demand a visible reasoning transcript or returned plan for ordinary work.
- Move to `thinking_level`; do not encode old token budgets, prefills, candidate-count behavior, or low-temperature recipes in the role/harness.
- Make current date and grounding expectations explicit for time-sensitive research.
- Preserve a compact Plan/Execute/Validate/Format contract, but avoid pasting the full evaluated agent template into every role unless testing shows it helps.
- Strengthen tool-loop recovery: recap state, change approach after failure, and check whether the goal is already complete.
- Start at medium for normal agent work; use high selectively rather than carrying forward an older “maximum thinking for coding” default.

## Google gemini-3.1-pro

### Profile

**Recommended role shape.** Gemini 3.1 Pro is documented as a reasoning-first model optimized for software engineering and agentic workflows with precise tools and reliable multi-step work. **INFERRED:** Use the same concise Gemini 3 structure as Flash, but assign broader ownership: mission, domain context, constraints, permission boundary, adaptability/persistence policy, acceptance criteria, and output contract.

**Instruction detail, long prompts, structure, and over-constraint.** Use direct language and one consistent delimiter style—Markdown or XML. Put critical behavior at the beginning and the current query at the end of long context. Gemini 3 docs warn that verbose older prompts can trigger overanalysis. **INFERRED:** Pro needs less procedural decomposition than a legacy role assumes. Give goals and invariants, then a short Plan/Execute/Validate contract. Long policy sets should be prioritized and deduplicated; they can fit the context window but still degrade focus.

**Tone, definition of done, and examples.** Default output is direct/efficient. Define audience and length for user-facing prose. Google's general guide favors specific, varied few-shot examples and warns against too many. **INFERRED:** Use examples for nuanced review severity, structured artifacts, or house style; use rules and tests for safety/scope. Definition of done should name functional acceptance, relevant checks, and evidence, while allowing the model to change its plan when observations disagree.

**Autonomy, check-ins, planning, and tools.** The model page emphasizes precise tool use and reliable multi-step workflows. Google's agent template distinguishes read-like exploration from state changes and explicitly steers ambiguity, permissions, adaptability, and persistence. Antigravity 1.1.22 recommends explore/plan/execute for complex changes and local tests/build/formatters as the verification loop. **INFERRED:** Give Pro high autonomy on reversible in-scope tasks, sparse check-ins, and a requirement to pause for destructive/external/shared-state actions or materially divergent interpretations. Enforce those boundaries through Antigravity permissions, not solely role prose. Ask for a plan on architectural or ambiguous work, not on every small fix.

**Effort semantics in this harness.** Gemini 3.1 Pro supports `low`, `medium`, and `high`, with high the model/API default; it does not support minimal. Google describes low as simple/throughput work, medium as balanced, and high as maximum reasoning with longer time to first output. The Antigravity documentation section is explicitly versioned 1.1.22 and exposes the same labels. **INFERRED:** Low for bounded code explanation, focused reviews, and simple fixes; medium for standard multi-file implementation and diagnosis; high for architecture, hard debugging, long-context synthesis, and agent plans where correctness outweighs latency. The lower-layer mapping from the CLI flag to the API field is not stated.

**Verbosity and user-facing copy.** Direct by default, so request conversational depth explicitly. **INFERRED:** Pro should be preferred over Flash when prose must preserve technical nuance or synthesize many constraints, but it still needs an audience and size contract; model capability does not substitute for editorial direction.

**Failure modes to guard against.** Overanalysis from verbose/complex prompts; repeated tools after state loss; excessive internal effort for simple work; visible plan/reasoning requests that waste output; few-shot overfit; low-temperature loops; and default concision omitting stakeholder context. The 3.1 Pro API identifier is documented as preview, so operational stability/version drift is also a deployment risk outside the role.

### Evidence with URLs

- [Gemini 3.1 Pro model page](https://ai.google.dev/gemini-api/docs/models/gemini-3.1-pro-preview) — last updated 2026-08-18 UTC; accessed 2026-08-28. Exact model positioning, preview status, context/output, and agent/tool claims.
- [Gemini 3 developer guide](https://ai.google.dev/gemini-api/docs/gemini-3) — last updated 2026-08-26 UTC; accessed 2026-08-28. Exact 3.1 Pro effort settings/default, direct prompting, verbosity, long context, sampling, and migration guidance.
- [Prompt design strategies](https://ai.google.dev/gemini-api/docs/prompting-strategies) — last updated 2026-06-10 UTC; accessed 2026-08-28. Examples, structure, tools, and evaluated agent-behavior dimensions.
- [Troubleshooting guide](https://ai.google.dev/gemini-api/docs/troubleshooting) — accessed 2026-08-28; page exposes no publication date. Tool/repetition/thinking/sampling failure guidance.
- [Antigravity CLI headless mode and model selection](https://www.antigravity.google/docs/cli/headless/) — accessed 2026-08-28; page exposes no publication date. CLI model and effort surface.
- [Antigravity CLI best practices](https://antigravity.google/docs/cli/best-practices/) — accessed 2026-08-28; documentation section identifies CLI v1.1.22. Verification, explore/plan/execute, context, rules, permissions, recovery, and subagents.
- [Antigravity permissions](https://antigravity.google/docs/cli/permissions) — accessed 2026-08-28; page exposes no publication date. Harness action-review and policy behavior.
- [Antigravity custom subagents](https://www.antigravity.google/docs/subagents/) — accessed 2026-08-28; page exposes no publication date. Role/system-prompt and tool/policy structure.

### What a v4 role should change vs a v3 role written for the predecessor

There is no single predecessor arrow in Google's exact 3.1 Pro page matching the wording “Gemini 3.x.” **INFERRED:** Treat this as updating a role written for earlier Gemini 3 or 2.5 behavior.

- Remove elaborate visible-chain-of-thought requirements and old thinking-token-budget instructions; select low/medium/high at the harness.
- Restore default sampling, especially temperature 1.0, unless an evaluation justifies changing it.
- Compress the role to direct goals, constraints, permission policy, and acceptance criteria; long persuasive wording can induce overanalysis.
- Keep a short adaptability/persistence rule so the agent changes its plan after contradictory tool evidence without looping indefinitely.
- Use high for genuinely hard work, not by reflex; medium/low can preserve quality while improving interaction latency.
- Specify copy depth because Gemini 3's concise default can under-explain a technically correct result.

## xAI grok-4.6

### Profile

**Source limitation.** **INFERRED:** No general-purpose text-model prompting/steering guide for Grok 4.6 was located in xAI's official documentation as of 2026-08-28. xAI publishes an exact model announcement, reasoning documentation, and Grok Build harness documentation. Its detailed “Prompting Guide” is explicitly for the Realtime speech-to-speech model, so its fixed section order and example prescriptions are not treated here as evidence for Grok 4.6 text/coding behavior.

**Recommended role shape.** **INFERRED:** Use a concise Markdown role: mission/owned artifact; scope and non-goals; autonomy and irreversible-action boundary; tool evidence requirements; definition of done; loop/stop condition; user-facing handoff. Grok 4.6 is trained for long-running agents and can turn broad ideas into substantial working first versions, so role boundaries and acceptance criteria matter more than a detailed build recipe.

**Instruction detail, long prompts, structure, and over-constraint.** xAI provides no exact text-prompt recommendation on goals versus step lists, Markdown versus XML, or long-role adherence. Grok Build loads `CLAUDE.md`, `.claude/rules`, and `AGENTS.md` families and can append/override system rules. xAI's prompt-caching guide says static prompts/examples/reference material should be front-loaded; its context-compaction guide says tighter context reduces stale-output distraction. **INFERRED:** Prefer short Markdown compatible with the harness, stable instructions first, and task-local detail later. Do not infer from the 500K window that an enormous role is harmless. Avoid duplicated or contradictory imported rules.

**Tone, definition of done, and examples.** xAI does not document Grok 4.6's default text verbosity or a general examples-versus-rules policy. **INFERRED:** Define tone with observable choices and define done with functional acceptance plus tool-grounded evidence. Start without examples; add a compact positive example only after a harness evaluation shows a repeatable formatting or copy failure.

**Autonomy, check-ins, planning, and implementation bias.** xAI says 4.6 focuses on long-running agents, persists across many steps, is strong at taking a broad product idea to a working first version, and increasingly self-tests. That is evidence of implementation bias when the request sounds like a build. **INFERRED:** Explicitly distinguish review/diagnose from change/build, authorize local reversible work, and require approval for destructive, external, shared-state, or material scope expansion. Avoid routine check-ins. Grok Build's Plan mode is intended for ambiguous architecture/high-impact restructures and is unnecessary for clear fixes or pure research.

**Tools and enforcement.** Grok CLI has Ask (default), Auto, and Always-approve modes plus allow/deny rules; `deny` wins. Plan mode gates edit tools but not shell redirection, and child agents are not edit-gated by the parent's Plan mode. **INFERRED:** Put “ask before X” in both the role and enforceable CLI permission/sandbox policy. Treat Plan mode as workflow control, not a security boundary. Require the role to change approach after repeated tool failure and to stop when acceptance criteria are met.

**Effort semantics in this harness.** Grok 4.6 supports `low`, `medium`, `high` (default), and `xhigh`; reasoning cannot be disabled. xAI says low uses some reasoning but stays fast for latency-sensitive agents/simple tools; medium is for complex analysis and long context; high uses deeper reasoning for very challenging multi-step problems; xhigh maximizes depth and latency for the hardest work. `xhigh` is new relative to 4.5, where an xhigh request was treated as high. The model has a 500K context window. **INFERRED:** Low for simple edits/search/tool calls; medium for normal coding and long-context work; high for difficult multi-step implementation/research; xhigh only for the hardest architecture, formal reasoning, or failure-prone long trajectories. Measure total trajectory cost: xAI prices output and long prompts, and tool complexity adds turns.

**Verbosity and user-facing copy.** No exact vendor guidance was found for Grok 4.6's default prose quality or verbosity. **INFERRED:** Require concise milestone updates, no narration of routine calls, and an outcome/evidence/risk final schema. Evaluate polished stakeholder copy separately from coding success, especially because the launch evidence emphasizes working interactive artifacts rather than prose style.

**Failure modes to guard against.** **INFERRED:** Broad-prompt scope expansion; jumping from an assessment-like request into a substantial implementation; long-agent loops; duplicated imported rules; treating Plan mode as preventing shell writes; unbounded subagents; and unsupported claims in the final handoff. Vendor-documented operational risks include higher latency at xhigh and distraction from stale long context. Require goal-state checks and tool-grounded completion evidence.

### Evidence with URLs

- [Introducing Grok 4.6](https://x.ai/news/grok-4-6) — dated 2026-08-12; accessed 2026-08-28. Exact-model long-horizon, coding, broad-to-working-first-version, visual, self-testing, training, benchmark, and pricing claims.
- [Reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning) — accessed 2026-08-28; current page covers 4.6. Exact effort meanings/default, inability to disable reasoning, and the 4.5-to-4.6 xhigh change.
- [Release notes](https://docs.x.ai/developers/release-notes) — entry dated 2026-08-12; accessed 2026-08-28. Exact 500K context, effort list/default, and pricing tiers.
- [Introducing Grok 4.5](https://x.ai/news/grok-4-5) — dated 2026-07-16; accessed 2026-08-28. Direct predecessor positioning and single-prompt implementation behavior.
- [Grok Build CLI reference](https://docs.x.ai/build/cli/reference) — last updated 2026-07-21; accessed 2026-08-28. `--effort`, rule/system-prompt, tool, plan, subagent, and permission flags.
- [Grok Build permissions](https://docs.x.ai/build/features/permissions) — last updated 2026-07-21; accessed 2026-08-28. Ask/Auto/Always-approve and allow/deny semantics.
- [Grok Build Plan mode](https://docs.x.ai/build/features/plan-mode) — last updated 2026-07-21; accessed 2026-08-28. When planning is useful and the edit-gate/subagent caveats.
- [Skills, plugins, marketplaces, and instruction compatibility](https://docs.x.ai/build/features/skills-plugins-marketplaces) — last updated 2026-08-11; accessed 2026-08-28. Imported Claude/AGENTS instruction files and subagents.
- [Prompt caching best practices](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/best-practices) — last updated 2026-05-10; accessed 2026-08-28. Stable-prefix/front-loading advice.
- [Context compaction](https://docs.x.ai/developers/advanced-api-usage/context-compaction) — accessed 2026-08-28; page exposes no exact update date in the retrieved text. Focus and latency benefits of removing stale context.
- [Realtime prompting guide](https://docs.x.ai/developers/model-capabilities/audio/speech-to-speech/prompting-guide) — accessed 2026-08-28. Cited only to delimit scope: its prompt structure is explicitly for speech-to-speech, not evidence of Grok 4.6 text-model steering.

### What a v4 role should change vs a v3 role written for the predecessor

The launch post directly compares Grok 4.6 with 4.5 on long-running work, first-pass interactive/visual builds, and self-testing; xAI does not publish a 4.5-to-4.6 prompt-migration guide.

- **INFERRED:** Add a clear assess-versus-implement switch and non-goals. A broad request is more likely to become a substantial first version.
- **INFERRED:** Replace frequent progress permission gates with milestone updates and explicit high-risk approval points; 4.6 is designed to persist across longer trajectories.
- **INFERRED:** Add goal-state and loop-exit checks, because longer agent runs need a stopping rule even as self-testing improves.
- Remove any assumption that `xhigh` aliases `high`: it is a distinct 4.6 reasoning depth with higher latency.
- **INFERRED:** Keep one acceptance-based verification contract rather than prescribing every test step; the model shows more native self-testing.
- **INFERRED:** Re-evaluate roles for interactive/visual implementation. Old detailed aesthetic scaffolds may be less necessary, while scope and product acceptance become more important.

## Which model for which task class — hypotheses to test

Every recommendation and rationale in this table is **INFERRED** from vendor positioning and steering documentation, not from a controlled run of these exact CLI versions. “Effort” is a starting point for an evaluation sweep.

| Task class | First model/effort hypothesis | Why this is the first test | Important counter-test |
|---|---|---|---|
| Hardest long-horizon research, ambiguous architecture, cross-repository debugging | Claude Fable 5 high; xhigh only after a hard-case gate | Strongest documented long-horizon autonomy, ambiguity handling, and multi-hour work; brief roles reduce scaffolding burden | Compare gpt-5.6-sol high and Opus 5 high for latency, scope control, and final-answer density |
| Complex implementation with a complete specification | Claude Opus 5 medium/high | Full-task completion, self-verification, strong coding, and lower effort retaining accuracy | Compare gpt-5.6-sol medium for concision and Grok 4.6 high for first-pass breadth |
| High-recall code review with calibrated output | Claude Opus 5 low/medium | Vendor explicitly documents improved precision/recall and review steering nuances | Compare gpt-5.6-sol medium; score unsupported findings and presentation severity separately |
| General team coding default across features, fixes, and reviews | gpt-5.6-terra medium | Balanced tier plus lean, explicit autonomy semantics is a plausible quality/cost default | Compare Gemini 3.7 Flash medium and Opus 5 low on completed-task cost, not per-call price |
| Deep but concise technical diagnosis or architecture memo | gpt-5.6-sol high | Flagship reasoning with natively concise copy and strong intent following | Compare Gemini 3.1 Pro high for long-context synthesis and Fable high for investigation depth |
| High-volume bounded edits, extraction, formatting, test generation | gpt-5.6-luna low/medium | Cost-sensitive tier with narrow role and exact acceptance checks | Compare Gemini 3.7 Flash low; include retry and human-review cost |
| Fast interactive coding, routine multi-tool work, broad batch of small tasks | Gemini 3.7 Flash medium | Google positions medium for most coding/agent work and documents fewer failed loops in 3.7 | Compare Terra medium and Luna medium on first-pass completion and tool-call count |
| Large-context code/policy synthesis where nuance matters | Gemini 3.1 Pro high | 1M-class context and documented software-engineering/tool/multi-step focus | Compare Sol high; test instruction retention at beginning and query at end |
| Broad product idea to polished interactive/visual first version | Grok 4.6 high | This is the exact strength highlighted in the 4.6 launch material | Compare Fable high and Opus 5 high; evaluate functionality, visual quality, scope creep, and iteration count |
| Latency-sensitive read-only triage or repo orientation | Gemini 3.7 Flash low or gpt-5.6-luna low | Both are positioned for fast/cost-sensitive work; the task is easy to bound and verify | Compare Terra low when a slightly stronger diagnosis reduces follow-up turns |
| Safety-sensitive or destructive operational change | No model chosen by role alone; strongest suitable model at medium/high behind enforced permissions | Correctness and authorization require CLI policy, sandboxing, review, and explicit acceptance—not a personality instruction | Test refusal/over-approval and policy bypass behavior in each exact harness before deployment |
| Polished stakeholder-facing prose attached to technical work | Opus 5 low/medium with strict length contract, or gpt-5.6-sol medium | Opus has strong long-form capability but needs verbosity control; GPT-5.6 is concise but needs explicit depth | Blind-review copy separately from technical correctness; include Gemini 3.1 Pro as a synthesis counter-test |

## Practical evaluation protocol

**INFERRED:** Before shipping a v4 role, run a small crossed evaluation rather than changing model and prompt simultaneously without measurement:

1. Use the exact CLI version, model alias resolution, effort, permissions, tools, and context-loading files from production.
2. Test the old v3 role and the proposed lean v4 role on the same tasks. Include one easy, one ambiguous, one destructive-action boundary, one tool-failure recovery, one review-only, and one long-horizon task.
3. Score: task completion; unauthorized mutations; unnecessary questions; tool-loop count; unsupported claims; acceptance-test evidence; elapsed latency; billed cost; user-facing copy; and scope creep.
4. Sweep effort separately. A model upgrade can make lower effort outperform the predecessor's higher setting; vendor docs explicitly say this for Fable and recommend fresh sweeps for Opus/GPT/Gemini families.
5. Treat permission-policy failures as harness defects first. Role prose is defense in depth, not the access-control mechanism.
