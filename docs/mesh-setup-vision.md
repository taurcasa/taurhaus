# Mesh Setup Vision

## 1. Core UX Concept

**Concept:** setup should feel like a mission launch panel, not a config form.

- User feeling: fast, confident, in control.
- Primary metaphor: "assemble team, launch, monitor."
- Tone: product-grade calm (Linear/Raycast), not infra-console technical.
- Promise: one decisive action gets a usable team; customization is available but never blocks momentum.

In one sentence: **Mesh setup is a preflight experience that gets you from zero to an operating roster in under 10 seconds.**

## 2. Layout And Hierarchy

Default state should present one obvious action, with detail layers behind it.

1. Header: concise purpose and current-project context.
2. Hero card: prominent Quick Start action.
3. Subtle preflight status banner (only when needed).
4. Advanced setup disclosure (collapsed by default).

Default state ASCII mockup:

```text
┌────────────────────────────────────────────────────────────────────┐
│ Mesh Team Setup                                                   │
│ Launch a team for this project. Customize only if you need to.    │
├────────────────────────────────────────────────────────────────────┤
│ ┌───────────────────────────────────────────────────────────────┐  │
│ │ Quick Start                                      [Start Team] │  │
│ │ One Codex dev agent + this project + current lead session.    │  │
│ └───────────────────────────────────────────────────────────────┘  │
│ [!] 2 setup warnings detected. You can continue safely.           │
│                                                                    │
│ ▸ Advanced setup                                                   │
│   Your current Claude session is used as team lead.               │
└────────────────────────────────────────────────────────────────────┘
```

## 3. Quick Start Path

Quick Start is the default and most prominent path.

- CTA label: action verb (`Start Team`, not `Initialize Configuration`).
- Immediate payload: inferred team name + fixed lead + one default dev agent.
- No form interaction required.
- If warnings exist, keep CTA enabled unless truly blocking; warnings stay informational.
- On click, transition directly into initialization progress, then runtime roster.

Quick Start should feel like "just do it now," not "review and approve."

## 4. Customize / Advanced Path

Advanced setup is a progressive disclosure path.

- Hidden behind a single toggle (`Advanced setup`).
- Contains only what changes outcomes:
  - team name / description
  - agent list (add/remove, tool/model/project, role description)
- Team lead internals are not editable in setup UI; show a simple statement that current Claude session is lead.
- Agent editing remains dense and efficient, using the same compact controls and row spacing as runtime.

Rule: advanced should be discoverable, powerful, and quiet. It should never compete visually with Quick Start.

## 5. Relationship To Runtime

Setup must feel like the first frame of the runtime experience, not a different product.

- Reuse card rhythm, keyline density, and control sizing from runtime roster.
- Keep action language consistent (`+ Agent`, compact overflow actions, lightweight captions).
- Keep the same visual cadence:
  - small title + small metadata line
  - thin separators
  - compact pills/badges
- Transition expectation: after launch, user should feel "same surface, now live."

If setup and runtime look unrelated, trust drops because launch feels like context switching.

## 6. Typography, Spacing, Color

Use existing taurhaus system: Geist + dark teal frame + brand teal accents.

- Type:
  - Title: `text-sm font-semibold`
  - Supporting body: `text-xs`
  - Labels/meta: `text-[11px]`
- Spacing:
  - Tight vertical rhythm (`space-y-2` to `space-y-4`)
  - Compact controls (`h-8`, slim paddings) to match dense runtime utility
- Color:
  - Base text and keylines from `themeTokens` zinc scale
  - Accent only for actions and high-signal highlights (`brand-500/600`)
  - Warnings use amber tokens in low-contrast surfaces; no loud all-caps error blocks
- Surface behavior:
  - one prominent accent card (Quick Start)
  - all secondary panels neutral and quiet

Visual goal: dense but breathable, technical but polished.

## 7. What Not To Do

- Do not expose low-value lead configuration fields in primary setup.
- Do not show giant review/summary blocks before launch.
- Do not use all-caps "config panel" section headers.
- Do not turn warnings into log-like raw diagnostics.
- Do not place equal visual weight on Quick Start and Advanced paths.
- Do not add decorative motion that competes with readability.

## 8. Success Criteria

We are done when all are true:

1. First-time users can launch a team with one click, without opening advanced setup.
2. Advanced setup is available but hidden by default.
3. Users can explain setup in one sentence: "Start now, customize if needed."
4. Team lead mechanics are understood without editable lead controls.
5. Setup and runtime feel visually continuous in dark and light modes.
6. Preflight warnings are calm, concise, and non-alarming unless blocking.
7. Internal team feedback calls the setup "product UI" rather than "developer form."

