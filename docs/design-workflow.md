# Design workflow

How design-led UI work flows through the team. This process ensures visual quality meets the taurhaus bar (9/10 per category) while leveraging the UI specialist (Gemini) for design leadership.

## The problem this solves

When the UI specialist receives implementation-heavy specs ("build this component with these 7 fields, these props, this layout"), it produces functionally correct but visually generic output. It's a strong implementer but won't spontaneously add the depth, micro-interactions, and visual grouping that make taurhaus feel premium. The fix is process, not tooling — give it design ownership, not just coding tasks.

## Design-first loop

Every UI task with visual impact follows this four-phase loop:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  1. BRIEF    │ ──▸ │  2. DESIGN   │ ──▸ │ 3. IMPLEMENT │ ──▸ │  4. REVIEW   │
│  (team lead) │     │ (UI spec.)   │     │ (UI spec.)   │     │ (team lead)  │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
       │                    │                    │                     │
  Functional            Design               Code                Visual
  requirements          proposal             changes              review
  + screenshots         + wireframes         (visual only)        + feedback
  + reference           + token values                            ──▸ iterate
    components           + both themes                               or ship
```

### Phase 1: Brief (team lead)

Provide:
- **Functional requirements** — what the component does, not how it looks
- **Screenshots** of current state (if redesigning existing UI)
- **Reference components** — 3-4 existing components that exemplify taurhaus's visual DNA
- **Constraints** — what must NOT change (test IDs, prop APIs, functional behavior)
- **Quality bar** — "minimum 9/10 visual review score"

Do NOT provide:
- Exact CSS classes or styling
- Pixel-level layout specifications
- Color values or specific token choices
- "Build it exactly like this" wireframes

### Phase 2: Design proposal (UI specialist)

The UI specialist studies the codebase's visual patterns and produces a structured proposal:

| Required element | Description |
|-----------------|-------------|
| ASCII wireframes | Layout structure for each component, showing visual grouping |
| Token values | Actual CSS values for every surface, border, shadow (not "subtle tint") |
| Theme parity | Dark AND light mode values side by side |
| New tokens | Any additions to `app.css` `@theme` system, with rationale |
| Interaction spec | Hover, focus, transition details |
| Reuse inventory | Which existing tokens/patterns are reused vs. created new |

**Approval gate**: Team lead reviews the proposal and either approves or requests revision with specific feedback. No code changes until approved.

### Phase 3: Implementation (UI specialist)

The UI specialist implements its own approved design. Constraints:

- Only change visual treatment (classes, styles, layout structure, animations)
- Never change `data-testid` attributes
- Never change component props, API signatures, or callback shapes
- Never change functional behavior (CRUD logic, IPC calls, validation)
- Run `just test` to verify no regressions

### Phase 4: Visual review (team lead)

Team lead reviews the implementation against the approved design. Scoring criteria:

| Category | What to evaluate |
|----------|-----------------|
| Typography | Hierarchy, weight contrast, size consistency |
| Spacing | Breathing room, density, alignment |
| Color | Token usage, contrast ratios, theme consistency |
| Hierarchy | Visual grouping, section separation, scanability |
| Consistency | Matches taurhaus DNA (12px radius, dark teal, Geist font) |
| Interactivity | Hover states, focus rings, transitions |
| Light mode | Full light mode pass, not just dark-first |
| Dark mode | Depth, layering, glow effects where appropriate |

Minimum score: 9/10 per category. If any category falls below, iterate.

## Anti-patterns

| Anti-pattern | Why it fails | Do this instead |
|-------------|-------------|-----------------|
| Over-specified brief | UI specialist becomes a code monkey, no design input | Give functional requirements, let it design |
| Vague proposal | "Subtle tints" and "better visual weight" can't be reviewed | Require actual token values and wireframes |
| Skip design phase | Generic form layouts, no visual grouping | Always do proposal → approval → implement |
| Dark mode only | Light mode breaks because it was never tested | Require dark AND light values in proposal |
| Change test IDs during visual work | Breaks E2E tests | Visual-only changes, never touch test IDs |

## Timing with E2E tests

Design work can conflict with E2E test development since both touch the same components:

- **Phase 1-2 (brief + design)**: Safe to run in parallel with E2E work — no code changes
- **Phase 3 (implementation)**: Must wait for E2E tests to land first, since visual changes can break element queries if layout structure changes
- **After implementation**: Re-run E2E tests to verify nothing broke

## Key files

| File | Purpose |
|------|---------|
| `src/app.css` | Design tokens (`@theme` system) — source of truth for colors, spacing, radii |
| `src/lib/themeTokens.js` | Runtime dark/light token switching for Svelte components |
| `docs/mesh-design-vision.md` | Current design vision with wireframes (reference for UI specialist) |

## Related documents

- [Documentation guidelines](GUIDELINES.md) — standards for writing docs
- [Mesh design vision](mesh-design-vision.md) — current wireframes and gap analysis
