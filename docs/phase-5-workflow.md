# Phase 5: Development Workflow

> Governing document for all taurhaus implementation work. Every decision here was made via structured questionnaire. This workflow is designed for maximum AI autonomy with clear escalation boundaries.

---

## 1. Testing Strategy

### 1.1 Frameworks

| Layer | Framework | Notes |
|-------|-----------|-------|
| **Rust unit/integration** | Built-in `#[test]` + `pretty_assertions` + `tempfile` | Better failure diffs, temp dirs for integration tests |
| **Frontend unit/integration** | Vitest + JSDOM + `@testing-library/svelte` | Component testing with user-interaction-style assertions |
| **E2E** | WebdriverIO + `tauri-driver` | Official Tauri support, tests real app through webview |

No `rstest` — write helper functions for parameterized tests instead.

### 1.2 Test Layers

| Layer | Scope | Examples |
|-------|-------|---------|
| **Rust unit** | Pure functions, parsers, computation | YAML frontmatter parser, activity state calc, file tree builder, search query construction |
| **Rust integration** | Module interactions with real SQLite + real git repos in tempdir | Register project → query back, import session → search finds it, git status on test repo |
| **Frontend unit** | Single component rendering + state logic | Sidebar filters correctly, theme tokens switch, session card renders fields |
| **Frontend integration** | Multi-component flows with mocked Tauri IPC | Select project → overview populates, tab switch → view changes |
| **E2E** | Critical user journeys through real app | First-run wizard, project browse → overview → files, search overlay |

### 1.3 Coverage Philosophy

**Acceptance-criterion-driven.** Every AC gets a corresponding test. No numeric coverage targets.

Tests exist because they verify something that matters, not to hit a number. If the AC says "activity state shows Active for projects with activity <7 days", a test exists for exactly that.

### 1.4 Test-First Mechanics

**TDD for behavior/logic. Visual review for layout.**

For each acceptance criterion:
1. Write the test (fails — red)
2. Implement minimum to pass (green)
3. Refactor if needed (still green)

Visual correctness (spacing, colors, alignment) is verified by screenshot review (§4), not by asserting CSS classes in JSDOM. Behavior (click handlers, filter logic, data flow, state transitions) is verified by tests.

### 1.5 Test Data Strategy

**Generated on the fly.** Tests create their own data in tempdirs using `tempfile`. Each test declares exactly what it needs — `git init`, write files, create commits, seed SQLite.

No checked-in fixture repos. Generated data is self-documenting and impossible to drift from reality.

---

## 2. Task Definition

### 2.1 Format

Use **Claude Code native task format** directly:

- `subject` — brief imperative title
- `description` — what and why, including acceptance criteria, test expectations, and context. Written as clear prose/structured text, not a custom schema.
- `status` — `pending` → `in_progress` → `completed`
- `blocks` / `blockedBy` — task dependencies
- `metadata` — anything extra if needed

No custom fields. ACs and test requirements are part of writing a clear description. This aligns with taurhaus's future task integration (ADR-019).

### 2.2 Granularity

**Half-day units.** Each task is a meaningful, self-contained unit (~2-4 hours of AI work).

- Small enough to review and verify
- Big enough to be meaningful
- Example: "Implement session YAML parser with all AC tests"

### 2.3 Categories

| Category | Quality Gate | Visual Review |
|----------|-------------|---------------|
| **backend** | `cargo test` + `cargo clippy` | No |
| **frontend** | `vitest run` + `svelte-check` + visual review | Yes |
| **integration** | All layers pass | If UI touched |
| **e2e** | E2E tests pass | Yes |
| **infrastructure** | Commands run successfully | No |

---

## 3. Quality Gates

### 3.1 Automated Gate

A single `just check` recipe runs all applicable checks before marking any task done:

- All acceptance criteria verified by passing tests
- `cargo test` passes (backend/integration tasks)
- `cargo clippy` clean — no warnings (backend/integration)
- `vitest run` passes (frontend/integration tasks)
- `svelte-check` passes (frontend/integration)
- Visual review passes threshold (frontend/e2e tasks — see §4)
- No regressions — all existing tests still pass

### 3.2 Regression Testing

**Full test suite on every task.** Run `cargo test` + `vitest run` before marking any task done. The suite stays fast at our scale (<50 projects).

E2E tests run at milestone boundaries (end of each implementation phase), not on every task.

### 3.3 Commit Strategy

**Logical sub-commits where reasonable.** No strict rule — split commits logically when it makes the history clearer. Single commit is fine for cohesive changes.

---

## 4. Visual Review Process

### 4.1 Categories

Every frontend screenshot is evaluated on 8 dimensions, rated 1–10:

| # | Category | What We're Looking For |
|---|----------|----------------------|
| 1 | **Design Fidelity** | Matches spec (phase-3g tokens, spacing, colors) |
| 2 | **Typography** | Font sizes, weights, line heights, hierarchy. Geist renders correctly |
| 3 | **Color & Contrast** | Brand colors correct, dark/light both work, sufficient contrast |
| 4 | **Spacing & Alignment** | Consistent padding, margins, alignment. Nothing feels "off" |
| 5 | **Visual Polish** | No glitches, artifacts, overflow, clipping. Rounded corners clean, borders crisp |
| 6 | **Layout Integrity** | Correct structure (sidebar width, panel gaps, content max-width) |
| 7 | **State Completeness** | All required states shown (empty, loading, error, populated) |
| 8 | **Dark/Light Consistency** | Both modes look intentional. No "forgotten" elements that don't switch |

### 4.2 Scoring Threshold

**Minimum 9/10 per category. No exceptions.**

Calibration note: AI visual review tends toward optimistic scoring (7-8 comes easy). The 9 threshold is calibrated against this bias — a genuine 9 from the AI means the quality is truly solid.

### 4.3 Review Process

**Dual review — self-review + Gemini Pro 3 cross-review.**

1. **Self-review**: Score all 8 categories against the spec and prototype
2. **Cross-review**: Send screenshot to Gemini Pro 3 (via `consult_gemini`) with the spec context. Gemini scores all 8 categories independently.
3. **Resolution**: Start with the lower score per category. If Gemini scored lower and I (Claude) disagree, I can override — but only with specific justification referencing the design spec. Not just "I think it looks fine."
4. **Final scores must all be ≥ 9**

### 4.4 Cadence

**After every frontend task.** Each task that touches UI gets screenshotted in both light and dark mode and reviewed through the full dual-review process.

### 4.5 Reference

- **Prototype is source of truth** for areas it covers (shell layout, sidebar, overview tab). The prototype code migrates directly into the Tauri app.
- **Phase-3g spec** for areas the prototype doesn't cover (file tree, search overlay, settings, states, edge cases).
- **`/lookbook` skill** for design judgment calls during implementation (component patterns, visual density, interaction feel).
- **Pre-implementation**: Audit phase-3g spec against prototype, fix any discrepancies. Prototype wins.

---

## 5. Feedback Loop & Iteration

### 5.1 Iteration Cycle

When a quality gate fails (test failure, visual score below 9):

1. **Fix immediately** — iterate in the same task
2. **Maximum 7 attempts** — if still failing after 7 iterations, flag the user with:
   - What's failing
   - What was tried
   - What the suspected root cause is
3. **No moving on with broken gates** (within the 7-attempt budget)

### 5.2 Requirement Adjustments

**Document and adjust freely.** Full autonomy on deviations from spec during implementation.

All adjustments documented in a **deviation log** — reviewed at milestone boundaries. Log format:

```
- [file/component]: [what changed] — [why]
```

### 5.3 AI Decision Boundaries

**Autonomous (no need to ask):**
- Implementation approach within spec
- Rust patterns, error handling style
- Minor spec deviations (spacing, timing adjustments)
- Library/crate selection for specified needs
- Test strategy for a specific AC
- Refactoring within a module
- Adding small unplanned features that naturally emerge
- Minor architecture adjustments within the spirit of the ADRs

**Requires user input:**
- Skipping a planned feature
- Major architecture deviation (contradicts an ADR's intent)
- Changing module boundaries significantly
- Quality gate failure after 7 attempts

---

## 6. Dev Environment & Tooling

### 6.1 justfile Recipes

```
# ── Development ──
dev              — cargo tauri dev (Vite + Tauri hot-reload)
dev-frontend     — npm run dev (Vite only, fast frontend iteration)

# ── Quality ──
check            — full quality gate (lint + svelte-check + test)
lint             — cargo clippy + frontend linting
check-rust       — cargo check + clippy
check-svelte     — svelte-check

# ── Testing ──
test             — cargo test + vitest run
test-rust        — cargo test
test-frontend    — vitest run
test-e2e         — build + WebdriverIO

# ── Visual ──
screenshot       — take screenshots for visual review

# ── Database ──
db-reset         — reset SQLite database
db-migrate       — run pending migrations

# ── Build ──
build-linux      — Linux build
build-windows    — Windows release build
sync-windows     — rsync source to Windows build dir
```

### 6.2 Development Mode

**Vite for frontend, Tauri for integration.**

- `just dev-frontend` for pure UI work (styling, components, layout). Faster hot-reload, no Rust compile wait.
- `just dev` when testing IPC, backend integration, or the full app experience.

---

## 7. Code Review Protocol

### 7.1 Code Review

**Self-review only.** After completing a task, re-read changes with reviewer eyes before running the quality gate. Tests + clippy catch mechanical issues. Self-review catches logic and design issues.

No cross-model code review — the visual review already uses Gemini for the subjective UI evaluation where blind spots matter most.

### 7.2 Security Review

**Integration tasks + phase boundaries.** Run `/security-audit` skill:

- On tasks that connect modules (IPC wiring, file I/O, user input handling) — highest-risk areas
- At the end of each implementation phase (5A through 5G) — broad scope review

---

## Workflow Flow

This section describes the complete end-to-end flow — how all the decisions above connect into an actual working process. Three levels: the **phase cycle** (macro), the **task cycle** (core loop), and the **TDD cycle** (inner loop).

### Phase Cycle (Macro)

Each implementation phase (5A through 5G) follows this arc:

```
Phase Start
    │
    ├─→ Task Creation
    │     Define tasks for this phase. Each task has subject, description
    │     (with ACs and test expectations), dependencies. Half-day units.
    │
    ├─→ Task Execution Loop
    │     Pick → Execute → Gate → Commit → Next
    │     (see Task Cycle below — this is where most time is spent)
    │
    ├─→ Phase Milestone Review
    │     ├── Run E2E test suite (not run per-task)
    │     ├── Run /security-audit (broad scope for the phase)
    │     ├── Review deviation log (all spec adjustments this phase)
    │     ├── Full-app screenshot review (holistic, not per-component)
    │     └── Update BOOTSTRAP.md phase status
    │
    └─→ Phase Complete → Next Phase
```

**Between phases**: The milestone review is the checkpoint. E2E tests verify end-to-end journeys work. Security audit catches patterns across the phase's changes. The deviation log gets reviewed — if adjustments accumulated that change the product direction, flag the user.

### Task Cycle (Core Loop)

This is the primary execution loop. Every task follows this flow:

```
                    ┌──────────────────────────────┐
                    │     PICK TASK                 │
                    │  Lowest ID, unblocked,        │
                    │  mark in_progress             │
                    └──────────┬───────────────────┘
                               │
                               ▼
                    ┌──────────────────────────────┐
                    │     READ TASK                 │
                    │  Understand ACs, context,     │
                    │  determine category           │
                    └──────────┬───────────────────┘
                               │
                               ▼
                    ┌──────────────────────────────┐
                    │     TDD CYCLE                 │
                    │  For each AC:                 │
                    │  Write test → Implement →     │
                    │  Refactor                     │
                    │  (see inner loop below)       │
                    └──────────┬───────────────────┘
                               │
                               ▼
                    ┌──────────────────────────────┐
                    │     SELF-REVIEW               │
                    │  Re-read all changes with     │
                    │  reviewer eyes. Fix issues.   │
                    └──────────┬───────────────────┘
                               │
                               ▼
                    ┌──────────────────────────────┐
                    │     QUALITY GATE              │
                    │  `just check`                 │
                    │  cargo test + clippy +        │
                    │  vitest + svelte-check        │
                    └──────────┬───────────────────┘
                               │
                          Pass? ──No──┐
                               │      │
                              Yes     ▼
                               │  ┌──────────────┐
                               │  │  ITERATE     │
                               │  │  Fix issue   │
                               │  │  attempt++   │
                               │  └──────┬───────┘
                               │         │
                               │    attempt ≤ 7? ─Yes─→ (back to TDD/fix)
                               │         │
                               │        No
                               │         │
                               │         ▼
                               │  ┌──────────────┐
                               │  │ FLAG USER    │
                               │  │ What failed, │
                               │  │ what tried,  │
                               │  │ root cause   │
                               │  └──────────────┘
                               │
                               ▼
                ┌─── Is frontend task? ───┐
                │                         │
               Yes                       No
                │                         │
                ▼                         │
    ┌───────────────────────┐             │
    │   VISUAL REVIEW       │             │
    │                       │             │
    │   1. Take screenshots │             │
    │      (light + dark)   │             │
    │                       │             │
    │   2. Self-review      │             │
    │      Score 8 cats     │             │
    │                       │             │
    │   3. Gemini review    │             │
    │      Score 8 cats     │             │
    │                       │             │
    │   4. Resolve scores   │             │
    │      Lower wins,      │             │
    │      Claude arbiter   │             │
    │                       │             │
    │   All ≥ 9? ──No──→ ITERATE         │
    │      │                              │
    │     Yes                             │
    └──────┬────────────────┘             │
           │                              │
           ▼                              ▼
    ┌─── Is security-relevant? (integration task) ───┐
    │                                                 │
   Yes                                               No
    │                                                 │
    ▼                                                 │
    ┌───────────────────────┐                         │
    │   /security-audit     │                         │
    │   Fix any findings    │                         │
    └──────────┬────────────┘                         │
               │                                      │
               ▼                                      ▼
            ┌──────────────────────────────┐
            │     COMMIT                   │
            │  Logical sub-commits where   │
            │  reasonable. Log deviations. │
            └──────────┬───────────────────┘
                       │
                       ▼
            ┌──────────────────────────────┐
            │     MARK COMPLETED           │
            │  Update task status.         │
            │  Pick next task.             │
            └──────────────────────────────┘
```

### TDD Cycle (Inner Loop)

For each acceptance criterion within a task:

```
    AC from task description
            │
            ▼
    ┌─── Is this logic or layout? ───┐
    │                                │
  Logic                           Layout
    │                                │
    ▼                                ▼
┌──────────────┐          ┌──────────────────┐
│ WRITE TEST   │          │ IMPLEMENT FIRST  │
│ (fails—red)  │          │ Build the UI     │
└──────┬───────┘          └──────┬───────────┘
       │                         │
       ▼                         ▼
┌──────────────┐          ┌──────────────────┐
│ IMPLEMENT    │          │ VISUAL REVIEW    │
│ Min to pass  │          │ catches layout   │
│ (green)      │          │ issues (see §4)  │
└──────┬───────┘          └──────────────────┘
       │
       ▼
┌──────────────┐
│ REFACTOR     │
│ if needed    │
│ (still green)│
└──────┬───────┘
       │
       ▼
  Next AC
```

**Logic examples**: event handlers, filter functions, state transitions, data transformation, IPC call/response handling, computed values.

**Layout examples**: element positioning, colors, spacing, typography, dark/light mode appearance, responsive behavior.

### Deviation Flow

Runs in parallel with the task cycle:

```
During implementation, spec doesn't work in practice
        │
        ▼
  Adjust freely (autonomous)
        │
        ▼
  Log in deviation log:
  - [file/component]: [what changed] — [why]
        │
        ▼
  At phase milestone: review deviation log
        │
        ├── Minor adjustments → acknowledged, move on
        └── Accumulated direction change → flag user
```

### Visual Review Flow (Detail)

The dual-review process for frontend tasks:

```
    Take screenshots
    (light mode + dark mode)
            │
            ▼
    ┌───────────────────────────────────┐
    │         SELF-REVIEW               │
    │                                   │
    │  For each of 8 categories:        │
    │  Score 1-10 against spec +        │
    │  prototype reference              │
    │                                   │
    │  Reference docs:                  │
    │  - prototype/src/Shell.svelte     │
    │  - docs/phase-3g-specification.md │
    │  - /lookbook for design judgment  │
    └──────────┬────────────────────────┘
               │
               ▼
    ┌───────────────────────────────────┐
    │       GEMINI CROSS-REVIEW         │
    │                                   │
    │  Send screenshot + spec context   │
    │  to Gemini Pro 3 via taursult     │
    │                                   │
    │  Gemini scores 8 categories       │
    │  independently (no access to      │
    │  self-review scores)              │
    └──────────┬────────────────────────┘
               │
               ▼
    ┌───────────────────────────────────┐
    │         SCORE RESOLUTION          │
    │                                   │
    │  Per category:                    │
    │  final = min(self, gemini)        │
    │                                   │
    │  If Gemini lower AND Claude       │
    │  disagrees → override allowed     │
    │  with specific spec justification │
    │                                   │
    │  All 8 categories ≥ 9?           │
    │                                   │
    │  YES → Pass                       │
    │  NO  → Iterate (fix + re-review)  │
    └───────────────────────────────────┘
```

### Security Review Flow

```
    ┌─── Trigger ────────────────────────┐
    │                                    │
    │  Integration task completed   OR   │
    │  Phase milestone reached           │
    │                                    │
    └──────────┬─────────────────────────┘
               │
               ▼
    Run /security-audit skill
    (TaurSec knowledge base)
               │
               ▼
    ┌─── Findings? ───┐
    │                  │
   Yes                No
    │                  │
    ▼                  ▼
  Fix findings      Continue
  Re-run audit
  until clean
```

---

## Infographic

Visual workflow infographic: [`docs/workflow-infographic.jpg`](docs/workflow-infographic.jpg)

---

## Decisions Reference

| # | Topic | Decision |
|---|-------|----------|
| Q1.1a | Rust test crates | `pretty_assertions` + `tempfile` |
| Q1.1b | Frontend DOM env | JSDOM |
| Q1.1c | E2E framework | WebdriverIO + `tauri-driver` |
| Q1.2 | Test layers | 5 layers (Rust unit/integration, Frontend unit/integration, E2E) |
| Q1.3 | Coverage | AC-driven, no numeric targets |
| Q1.4 | TDD scope | Logic = test-first, Layout = visual review |
| Q1.5 | Test data | Generated on the fly in tempdirs |
| Q2.1 | Task format | Claude Code native tasks |
| Q2.2 | Task size | Half-day units |
| Q2.3 | Categories | backend, frontend, integration, e2e, infrastructure |
| Q3.1 | Quality gate | Automated via `just check` |
| Q3.2 | Regressions | Full suite every task |
| Q3.3 | Commits | Logical sub-commits where reasonable |
| Q4.1 | Review categories | 8 categories |
| Q4.2 | Score threshold | Min 9/10 per category |
| Q4.3 | Reviewer | Self + Gemini Pro 3 |
| Q4.4 | Disagreement | Lower score wins, Claude is final arbiter |
| Q4.5 | Cadence | After every frontend task |
| Q4.6 | Reference | Prototype = source of truth, spec for new areas |
| Q5.1 | Iteration | Fix immediately, max 7 attempts |
| Q5.2 | Req changes | Document and adjust freely |
| Q5.3 | Autonomy | Broad — implementation + small features + minor arch |
| Q6.1 | Justfile | Full set with lint, db, build-windows |
| Q6.2 | Dev mode | Vite for frontend, Tauri for integration |
| Q7.1 | Code review | Self-review only |
| Q7.2 | Security | Integration tasks + phase boundaries |
