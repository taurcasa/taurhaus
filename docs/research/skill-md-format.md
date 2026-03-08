# `SKILL.md` / Agent Skills Research Summary

Date: `2026-03-08`

## Short Answer

`SKILL.md` is not a Frontier Labs format.

The origin appears to be Anthropic's Agent Skills / Claude Skills system. Anthropic introduced the pattern as a folder containing a `SKILL.md` file plus optional resources, and GitHub, VS Code, and OpenAI now explicitly describe Agent Skills as an open standard or actively support the format.

So the right mental model is:
- origin: Anthropic / Claude Skills
- current status: expanding cross-agent convention with multiple first-party adopters
- spec maturity: real enough to build against, but still more ecosystem convention than long-standing IETF-style formal standard

## 1. What is `SKILL.md`? Who created it? What problem does it solve?

Anthropic's Claude Skills documentation and engineering blog describe a skill as a folder that teaches an agent a specialized workflow, with:
- a `SKILL.md` file for metadata + high-level instructions
- optional `references/`, scripts, templates, or other bundled resources

Problem it solves:
- package procedural knowledge once instead of restating it every session
- keep context efficient through progressive disclosure
- make reusable workflows portable across projects and, increasingly, across agent products

Anthropic's framing is explicit:
- skills teach Claude specific workflows and specialized knowledge
- agents first see only lightweight skill metadata, then load the full body when relevant
- detailed resources stay out of context until needed

That is substantively different from always-on repo instructions.

## 2. What is the schema / structure?

The core structure is simple:

```text
my-skill/
├── SKILL.md
├── references/        # optional
├── scripts/           # optional
└── assets/            # optional
```

`SKILL.md` itself is Markdown with YAML frontmatter.

Minimum required fields consistently documented across Anthropic, GitHub, and ecosystem tooling:
- `name`
- `description`

Then the Markdown body contains the actual instructions/workflow.

Anthropic's developer writeup shows the body organized as procedural guidance with links into bundled reference files. GitHub docs likewise describe skills as folders of instructions, scripts, and resources, loaded when relevant. Vercel's `vercel-labs/skills` tool also documents the same basic schema.

So structurally it is closest to:
- Claude/Copilot custom agents in being Markdown with YAML frontmatter
- but semantically it behaves more like an on-demand reusable workflow package than a persona definition

## 3. How does it compare to Claude Code custom agents, Copilot agents, `AGENTS.md`, and `GEMINI.md`?

### `SKILL.md`

- Purpose: reusable task/workflow package
- Activation: loaded on demand when the task matches the description
- Shape: YAML frontmatter + Markdown body + optional bundled resources
- Best for: repeatable procedures, domain workflows, tool-guided playbooks

### Claude Code custom agents

- Purpose: specialist agent/persona or behavior package
- Activation: selected/delegated as an agent, not just auto-loaded as a task skill
- Shape: Anthropic also uses Markdown + frontmatter in related customization surfaces, but the semantics are role/persona-oriented rather than workflow-package-oriented
- Best for: sustained role behavior and tool posture

### GitHub Copilot custom agents (`.agent.md`)

- Purpose: reusable persona with behavior, model, and tool configuration
- Activation: selected agent or delegated agent
- Shape: Markdown file defining agent behavior/configuration
- Best for: stable specialist identity, not portable workflow packaging

### `AGENTS.md`

- Purpose: always-on repo or workspace guidance
- Activation: generally loaded as persistent context
- Shape: plain Markdown convention, not strongly schema-driven
- Best for: coding rules, architecture, commands, repo norms

### `GEMINI.md`

- Purpose: instruction-only guidance for Gemini-oriented workflows
- Activation: instruction/context file, not a dynamic skill package
- Shape: plain Markdown convention
- Best for: persistent instructions, not packaged procedural modules

Bottom line:
- `AGENTS.md` / `GEMINI.md` are instruction documents
- custom agents are personas/configurations
- `SKILL.md` is a modular, on-demand SOP bundle

That is why Taurhaus should not treat it as just another instruction-only export. It is closer to a workflow bundle format.

## 4. Is it gaining adoption? Which tools/platforms support it?

Yes, there is substantive evidence of adoption.

Verified first-party or official support found in this research:
- Anthropic / Claude Skills
- GitHub Copilot coding agent
- GitHub Copilot CLI
- VS Code agent mode / VS Code Insiders
- OpenAI Codex (official `openai/skills` catalog and Codex skill docs links from that repo)

Evidence:
- GitHub Docs says Agent Skills are an open standard and supported by Copilot coding agent, Copilot CLI, and VS Code agent mode
- VS Code customization docs say Agent Skills are an open standard across multiple AI agents
- OpenAI's official `openai/skills` repository describes Agent Skills as reusable folders for Codex and links to both Codex skill docs and the open standard
- Vercel's `vercel-labs/skills` repository treats the format as cross-agent and ships installers/discovery across many agent directories

So this is beyond a niche Anthropic-only convention now.

## 5. Is there an open spec or just a convention?

Best current answer: there is an emerging open spec, but it still behaves like a fast-moving ecosystem standard.

What I could verify directly:
- GitHub Docs explicitly calls the Agent Skills specification an open standard
- VS Code docs explicitly call Agent Skills an open standard
- OpenAI's official skills catalog links to an Agent Skills open standard

What I found indirectly:
- ecosystem docs repeatedly point to `agentskills.io` and a spec repo associated with `github.com/agentskills/agentskills`

Important precision:
- I found strong primary-source confirmation that major vendors treat Agent Skills as an open standard
- I did not independently verify the full spec repository contents during this pass

So I would describe it as:
- stronger than an informal convention
- weaker than a long-settled, highly versioned spec with mature governance

## 6. Could / should Taurhaus support import/export for this format?

### Export

Yes, probably.

Why:
- Taurhaus already exports/imports:
  - Claude agent files
  - Copilot agent files
  - instruction-only `AGENTS.md`
  - instruction-only `GEMINI.md`
- `SKILL.md` is a better semantic fit for procedural role/workflow content than instruction-only files are
- Taurhaus already has adapter infrastructure in [adapters.rs](/home/mstie/projects/taurhaus/src-tauri/src/templates/adapters.rs)

Recommended export scope:
- export Taurhaus role/workflow content into:
  - `SKILL.md`
  - optional `references/` files for compiled long-form sections
- mark lossy fields explicitly, just as Taurhaus already does for other adapters

But do not pretend a Taurhaus role and an agent skill are identical:
- a Taurhaus role is partly persona/context steering
- a skill is primarily a triggered procedure

So export should likely be:
- role -> skill when the role is workflow-heavy
- or a dedicated Taurhaus "workflow/skill" concept later

### Import

Yes, but lower confidence than export.

Why:
- import is easy at the structural level: parse YAML frontmatter + Markdown body
- import is harder semantically because many skills are task procedures, not role/persona definitions

Recommended import strategy:
- support import as a separate source type, e.g. `skill_md`
- map:
  - `name` -> role/template name
  - `description` -> short context summary / trigger description
  - body -> instructions
  - provenance -> `skill_md`
- treat imported skills as workflow-oriented templates, not necessarily as team-member roles by default

### Recommendation

Support it, but not as "just fifth format next to AGENTS/GEMINI."

Support it as:
- a first-class adapter for Agent Skills / `SKILL.md`
- clearly labeled as workflow-oriented and partially lossy relative to Taurhaus role semantics

If Taurhaus keeps growing as a multi-agent orchestration product, `SKILL.md` support is strategically aligned.

## Recommendation for Team Lead

Practical recommendation:
1. Treat `SKILL.md` as a real external format worth supporting.
2. Add it after the current adapter/import/export work stabilizes, not before.
3. Model it as an Agent Skill / workflow bundle, not as an instruction-only markdown file.
4. Expect partial round-trip fidelity.
5. Keep provenance explicit so imported skills remain traceable.

## Design Framing: Roles vs Skills (from team discussion, 2026-03-08)

Before designing a SKILL.md adapter, we need to resolve how skills relate to our existing role system. Our working framing:

**Roles and skills are orthogonal dimensions:**
- **Role** = "who you are" — job, perspective, behavioral boundaries. A job description.
- **Skill** = "what you can do on demand" — a reusable workflow bundle with resources. A certification or tool proficiency.

**The baseline analogy:** All LLMs share a vast common baseline of capabilities — like how, from a fly's perspective, all humans look the same. Roles differentiate *perspective and focus* ("I'm a reviewer" vs "I'm an architect"). Skills would differentiate *specific workflow capabilities* ("I can run security audits" or "I can generate migration scripts"). Every agent has the baseline; roles shape identity; skills extend procedural repertoire.

**Why we're parking this:**
1. Our role system just shipped (multi-tool leads, import/export, provenance). We should use it on real teams before adding another dimension.
2. Skills would need their own design phase — they'd live *alongside* roles, not inside them. A role might reference skills it can use, but skills would be separate entities with their own storage, composition, and lifecycle.
3. Premature abstraction risk: we don't yet have concrete use cases where someone says "I wish I could attach a reusable workflow to this agent." When that pain point shows up organically, the design will be better informed.

**What's ready when we revisit:** The adapter pattern we built for roles (canonical internal schema → format-specific adapters → provenance tracking) extends naturally to skills. The research above provides the format spec. This framing provides the conceptual model.

## Sources

Primary / official:
- Anthropic engineering blog: https://claude.com/blog/building-skills-for-claude-code
- Anthropic Skills product page: https://claude.com/skills
- GitHub Docs, About agent skills: https://docs.github.com/en/copilot/concepts/agents/about-agent-skills
- VS Code customization docs: https://code.visualstudio.com/docs/copilot/customization/overview
- OpenAI official skills catalog: https://github.com/openai/skills
- Vercel skills tool: https://github.com/vercel-labs/skills

Secondary / ecosystem:
- Agent Skills guide: https://agentskill.sh/readme
