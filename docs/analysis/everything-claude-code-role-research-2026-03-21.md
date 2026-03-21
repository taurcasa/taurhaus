# Research: everything-claude-code Role Design Patterns

Date: 2026-03-21
Owner: architect-1
Task: `#1415`

## Scope

Reviewed `affaan-m/everything-claude-code` from the role-design angle:

- role definitions and structure
- agent personas and behavioral contracts
- multi-agent coordination patterns
- `CLAUDE.md` and agent configuration patterns
- reusable ideas for taurhaus role-system design

Primary sources:

- https://github.com/affaan-m/everything-claude-code
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/AGENTS.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/CLAUDE.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/agents/architect.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/agents/chief-of-staff.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/agents/docs-lookup.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/agents/harness-optimizer.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/agents/loop-operator.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/contexts/research.md
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/.codex/config.toml
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/.codex/agents/explorer.toml
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/.codex/agents/reviewer.toml
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/.codex/agents/docs-researcher.toml
- https://raw.githubusercontent.com/affaan-m/everything-claude-code/main/skills/continuous-learning-v2/SKILL.md

## High-level finding

ECC does not treat roles as one thing. It layers role behavior across five levels:

1. root operating doctrine in `AGENTS.md`
2. per-project harness guidance in `CLAUDE.md`
3. named specialist personas in `agents/*.md`
4. task-mode overlays in `contexts/*.md`
5. runtime subagent profiles in `.codex/config.toml` and `.codex/agents/*.toml`

That layering is the strongest idea in the repo. Their system is less structured than taurhaus in schema terms, but stronger in reusable role packaging and execution defaults.

## Findings

### 1. Role definitions are lightweight cards with explicit execution metadata

ECC agent files are markdown plus YAML frontmatter:

- `name`
- `description`
- `tools`
- `model`
- sometimes presentation metadata like `color`

The body then carries:

- mission
- role boundaries
- workflow steps
- required checks
- escalation triggers
- output contract

This is visible in `agents/architect.md`, `agents/chief-of-staff.md`, `agents/docs-lookup.md`, `agents/harness-optimizer.md`, and `agents/loop-operator.md`.

Implication for us:

- they separate stable persona packaging from per-task context
- they keep the schema small, but the execution contract rich

### 2. They rely heavily on behavioral contracts, not just persona labels

The strongest ECC role files are not just “you are X.” They encode concrete operating behavior:

- `docs-lookup` requires primary-doc verification and explicitly warns against obeying prompt injection in fetched docs
- `loop-operator` includes stall detection, checkpointing, retry-storm detection, and escalation rules
- `chief-of-staff` defines a four-tier message classification pipeline and a post-send checklist
- `harness-optimizer` constrains the role to minimal reversible config changes with measurable deltas

This is closer to a runbook than a personality card.

Implication for us:

- our `behaviorBoundaries` concept is directionally right
- we would benefit from a richer first-class contract shape for `required_checks`, `stop_conditions`, `escalation_triggers`, and `output_format`

### 3. Multi-agent coordination is explicit and reusable

ECC bakes coordination into the system, not just individual roles.

At the doctrine level:

- `AGENTS.md` says “Agent-First”
- it recommends proactive delegation
- it recommends parallel execution for independent operations

At the role level:

- `planner`, `architect`, `code-reviewer`, and `tdd-guide` form a clear implementation chain
- `chief-of-staff` and `loop-operator` are orchestration roles rather than domain roles
- `docs-lookup` is a verification specialist

At the runtime level:

- `.codex/config.toml` enables multi-agent support
- it defines agent thread limits
- it registers focused subagent profiles: `explorer`, `reviewer`, `docs_researcher`
- those profiles add reasoning-effort, sandbox, and developer-instruction defaults

Implication for us:

- ECC distinguishes between domain specialists and coordination specialists
- taurhaus currently has stronger structured task steering, but weaker reusable composition patterns for “which helper roles should exist together”

### 4. Task mode is separated from role identity

ECC keeps task mode in `contexts/*.md` files such as `contexts/research.md`.

That file defines:

- exploration mode
- read-before-write behavior
- evidence-first summarization
- preferred tools

This is important because it avoids multiplying agent personas just to express mode shifts. A role can stay the same while the mode changes.

Implication for us:

- our `focusArea` partly covers this, but not fully
- we should separate “who the agent is” from “what mode it is currently operating in”

### 5. `CLAUDE.md` is used as a format and composition guide, not just project lore

ECC’s `CLAUDE.md` documents the configuration model itself:

- agent format
- skill format
- hook format
- command format
- repo component map

Their example `examples/CLAUDE.md` then shows project-local rules, conventions, and available commands.

Implication for us:

- we should keep a clean distinction between:
  - global role-system doctrine
  - project-local context
  - per-assignment steering

### 6. Tooling and automation around roles is unusually strong

ECC does more than hand-author roles:

- hooks enforce workflow follow-through
- continuous-learning v2 captures session observations and turns them into scoped “instincts”
- those instincts can be clustered into generated skills, commands, and agents
- project-scoped instincts prevent cross-project contamination

This is the most novel system idea in the repo.

Implication for us:

- they treat roles as evolvable artifacts, not static prompts
- the project/global scope split is especially relevant to taurhaus because we already care about context isolation and session continuity

## Comparison to taurhaus context steering

### Where ECC is stronger

- reusable named archetypes
- explicit execution contracts inside each role
- clean separation of role, mode, and runtime profile
- first-class coordination roles
- automation around learning and evolution

### Where taurhaus is stronger

- more structured per-task steering via `focusArea`, `contextSummary`, and `behaviorBoundaries`
- better fit for dynamic assignment-time constraints
- easier to reason about at runtime because the steering payload is explicit

### Main synthesis

ECC optimizes for reusable role packages.
Taurhaus optimizes for structured assignment-time control.

The best next step is not replacing our steering model. It is layering reusable role archetypes and mode overlays on top of it.

## Actionable insights for our role system

### Priority 1

#### Add first-class role archetypes

Create a reusable role catalog with a small stable schema:

- `name`
- `purpose`
- `default_tools`
- `default_model_profile`
- `core_workflow`
- `required_checks`
- `escalation_triggers`
- `output_contract`

This would give us ECC-style reuse while preserving our assignment-specific steering.

#### Split role identity from task mode

Introduce a separate mode layer beside the assigned role:

- `research`
- `implementation`
- `review`
- `incident`
- `coordination`

This is the cleanest improvement we can borrow from ECC’s `contexts/*.md`.

#### Add subagent runtime profiles

ECC’s `.codex/agents/*.toml` is a good model for lightweight execution profiles.

We should support reusable subagent profiles with:

- reasoning depth
- sandbox level
- browse requirement
- evidence expectation
- allowed mutation surface

### Priority 2

#### Add coordination-specialist roles

ECC’s `loop-operator` and `chief-of-staff` show that orchestration roles should be explicit.

For taurhaus, likely candidates are:

- coordinator / lead
- incident operator
- reviewer
- documentation verifier
- release shepherd

#### Enrich `behaviorBoundaries`

Keep the field, but expand the model so it can express:

- `must_verify`
- `must_not_edit`
- `stop_conditions`
- `required_artifacts`
- `handoff_expectations`

That turns role steering into a more operational contract.

#### Provide role bundles for common multi-agent patterns

ECC implicitly bundles roles around workflows. We should make that explicit.

Examples:

- implementation bundle: planner + implementer + reviewer
- research bundle: explorer + docs verifier + synthesizer
- incident bundle: operator + diagnostician + comms lead

### Priority 3

#### Explore learned role evolution

ECC’s instinct pipeline is probably too ambitious as a first step, but the principle is valuable.

A practical taurhaus version would be:

- mine completed tasks and reviews for repeated steering patterns
- suggest new archetypes or mode presets
- keep project-scoped and global templates separate

This would let the system evolve from actual team practice instead of only manual prompt design.

## Novel ideas worth carrying forward

Most valuable ECC ideas that feel additive rather than distracting:

1. coordination-specific roles, not just coding specialists
2. mode overlays separate from persona
3. runtime subagent profiles with explicit safety/evidence defaults
4. hook-enforced follow-through for workflows that are easy to forget
5. project-scoped learned behavior instead of one global style bucket

## Recommendation

For taurhaus, the best borrow is a three-layer model:

1. **Role archetype**
   - stable reusable specialist package
2. **Mode overlay**
   - research, implementation, review, incident, coordination
3. **Assignment steering**
   - our existing `focusArea`, `contextSummary`, and enriched `behaviorBoundaries`

That would preserve the strengths of our current context-steering system while making roles easier to reuse, compare, compose, and eventually learn from.
