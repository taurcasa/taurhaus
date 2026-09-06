# Mesh E2E flake audit

Baseline: `0a22cf7c`, branch `fix/mesh-e2e-flake-audit`. Implementer: GPT-6
Astra. Scope: test helpers/specs; no product changes. This is a lane report,
not a plan ledger. P1 = lost click across runtime polling; P2 = scanner-state
propagation. “Fix in item 2” records an outstanding finding at audit time.

## 1. Sweep

All seven specs containing Mesh UI selectors were read, including their local
helpers. Rows group repeated calls to the same helper; distinct transitions
are listed separately. The four paid specs drive coordination through IPC,
not Mesh runtime UI, and are excluded from the suite and this change.

| Site / action → wait | Pattern | Disposition and reason |
| --- | --- | --- |
| All seven specs: `openMeshTab` → surface / gate resolution | — | Not applicable: titlebar is outside the polled runtime subtree; gate resolves availability IPC. |
| `mesh-workflow`: overview → Mesh tab switching | — | Not applicable: shell navigation, not scanner state. |
| `meshBuilder.setInlineBuilderTeamName` → input, then setup | — | Already safe for these patterns: setup is not runtime-polled; native input event drives the transition. |
| `mesh-workflow`: lead selection → lead card | — | Not applicable: local setup roster. |
| `mesh-workflow`: initialize enable / click → runtime or error | — | Not applicable: command completion, not scanner propagation; never retry a launch. |
| `mesh-workflow`: primary action → add-agent form | P1 | Fix in item 2: single runtime click can vanish. |
| `mesh-workflow`: role/name/project → enabled submit | — | Already safe: editor state; role card is clicked atomically in the active SlideOver. |
| `mesh-workflow`: submit → form closed or error | — | Already safe: command result checked explicitly; never retry a mutation. |
| `mesh-workflow`: overflow → disband action | P1 | Fix in item 2: target-first retry avoids losing or re-toggling the menu. |
| `mesh-workflow`: disband action → open confirmation | P1 | Fix in item 2: retry the opener, not the confirmation. |
| `mesh-workflow`: confirm → empty/setup; reset → empty | — | Not applicable: disband command result and local reset. |
| `mesh-recovery`: overflow → menu; disband → confirmation | P1 | Fix in item 2: both runtime openers are single clicks. |
| `mesh-recovery`: confirm → empty/setup | — | Not applicable: command completion; ownership checked before disband. |
| `mesh-recovery`: builder lead/agent → cards; input → setup | — | Not applicable: local setup state, no live runtime poll. |
| `mesh-recovery`: initialize → runtime/error/title | — | Not applicable: command completion and title identity, not member liveness. |
| `mesh-recovery`: kill all panes → offline count → coldResume snapshot | P2 | Fix in item 2: 25s overrides undercut honest scanner propagation budgets. |
| `mesh-recovery`: reload → app/projects → stopped runtime copy | P2 | Fix in item 2 for runtime copy; app/project boot retains its own readiness checks. |
| `mesh-recovery`: resume click / IPC → zero offline → active runtime copy | P2 | Fix in item 2: one launch followed by scanner/UI propagation, not repeated resume clicks. |
| `mesh-recovery`: kill member → offline count / degraded snapshot / UI copy | P2 | Fix in item 2; existing product-issue skip remains visible. |
| `mesh-recovery`: named agent node → matching detail | P1 | Fix in item 2: current loop stops at click delivery, not matching detail. |
| `mesh-recovery`: detail → Offline status | P2 | Fix in item 2: status is fed by runtime polling. |
| `mesh-recovery`: Add Agent (primary or secondary) → form | P1 | Fix in item 2: retry only the open, after active-state validation. |
| `mesh-recovery`: role/name/project → submit → error/form/message | — | Not applicable: command/editor state; final named-node assertion still required. |
| `mesh-recovery`: successful add → named node | P2 | Fix in item 2: roster visibility follows runtime refresh. |
| `template-crud-ui`: runtime overflow → disband; disband → open dialog | P1 | Fix in item 2: last-element single clicks still race re-renders. |
| `template-crud-ui`: confirm → empty/setup; reset → empty | — | Not applicable: command completion and local reset. |
| `template-crud-ui`: lead → card; initialize → runtime/error/title | — | Not applicable: setup and initialize response, not scanner propagation. |
| `template-crud-ui`: Add Agent → form, including rebuild branch | P1 | Already safe shape; retrofit BOTH branches to the shared helper in item 2. |
| `template-crud-ui`: runtime node → detail capture button | P1 | Fix in item 2: cached node handle can be replaced before click. |
| `template-crud-ui`: detail capture → capture form | P1 | Fix in item 2: target-first retry for the runtime detail opener. |
| `template-crud-ui`: role card → autofill; unlock → enabled; cancel → closed | — | Not applicable: stable SlideOver editor state, not runtime node replacement. |
| `template-crud-ui`: capture save → success banner → catalog card | — | Already safe: atomic active-SlideOver save and exact success text; never retry save. |
| `template-crud-ui`: template browser → panel | — | Not applicable: empty/setup view is not runtime-polled. |
| `template-crud-ui`: create/edit role → editor; inspect → detail | — | Already safe: active-SlideOver query and DOM click share one browser task. |
| `template-crud-ui`: role save → persisted role / instructions | — | Not applicable: git-backed command result, not daemon scanner. |
| `template-crud-ui`: role delete → dialog → card absent | — | Already safe for P1/P2: atomic opener; final deletion assertion remains. |
| `template-crud-ui`: presets tab → create; create → customizer | — | Already safe: atomic SlideOver clicks, no runtime poll. |
| `template-crud-ui`: preset save → card / customizer closed; delete → dialog → absent | — | Not applicable: storage command and catalog refresh. |
| `template-crud-ui`: cleanup close/cancel → SlideOver closed | — | Already safe: bounded existing atomic clicks with closed-state checks. |
| `templates`: close overlay → absent; reset → empty; browser → panel | — | Not applicable: setup/catalog only; refuses every runtime disband. |
| `templates`: roles/presets tabs → cards; inspect → exact detail text | — | Already safe: atomic active-SlideOver clicks. |
| `templates`: upsert/flush → reopen → card/details; delete/flush → reopen → absent | — | Not applicable: awaited storage IPC, no scanner dependency. |
| `mesh-screenshots`: runtime disband opener → confirmation | P1 | Fix in item 2: open overflow first and await an actually open dialog. |
| `mesh-screenshots`: confirm → empty/setup | — | Not applicable: disband command completion. |
| `mesh-screenshots`: display → setup; customize → panel → input | — | Not applicable to timing classes: legacy setup selectors/input flow; report drift if executed. |
| `mesh-screenshots`: customizer save → closed; initialize → runtime/failure | — | Not applicable: command completion, never repeat mutations. |
| `mesh-screenshots`: theme → capture | — | Not applicable: shell styling, no propagation wait. |
| `template-screenshots`: runtime cleanup disband → confirmation | P1 | Fix in item 2: same runtime opener shape, behind existing ownership guard. |
| `template-screenshots`: init-back/reset → setup/empty; wait initializing exit | — | Not applicable: local setup or initialize response. |
| `template-screenshots`: preset/custom → setup; add roles → counts; pin → strip | — | Not applicable: setup-only local roster; legacy selectors are separate drift. |
| `template-screenshots`: overlay close → absent; theme → screenshot settle | — | Not applicable: setup overlays and visual capture. |
| `role-detail-screenshots`: setup/search → catalog; role info → detail | — | Not applicable: catalog preview, not a live runtime node. |
| `role-detail-screenshots`: edit → inputs; cancel/close → absent | — | Already safe for P1: atomic overlay clicks; existing bounded edit retries are setup-only. |
| `role-detail-screenshots`: create → editor; save → closed/card/searchable | — | Not applicable: local editor/storage response (existing git-write budget retained). |

## Cadence and safety evidence

`daemon/session_activity.rs` uses 500ms active / 1500ms idle scan intervals;
`meshTabGate.svelte.js` uses 2000ms live-status polling. These are separate
from the 30s background self-heal pass; recovery queries reconcile live
presence through the daemon directly (`commands/coordination/live_status.rs`).
Planned propagation budget: four idle scans (6000ms), two UI polls (4000ms),
plus 20000ms scheduling/IPC margin for suite contention = 30000ms. This is a
bounded allowance, not a promise that product work has a 30s upper bound.

The manifest currently runs one worker at a time (`maxInstances: 1`), despite
the historical seven-worker incident. Acceptance runs must use the requested
unmodified command/configuration, not introduce a load run. Paid lanes stay
excluded. Worker roots/tmux/daemon are isolated; any default harness commands
must be shadowed by inert fixtures before the suite is started.

The two unconditional recovery skips and prerequisite skips predate this
lane. No new skip is authorized. A failed wait remains a failure. Failure
screenshots can show cleanup's disband, so diagnosis must use the preceding
app log, as documented by `673dac42`.

## 2–4. Implementation and proof

Pending. The final update will record deterministic red/green tests, exact
gate exit codes, each of three consecutive tier-1 spec-file summaries, and
any blocker. Review/routing calibration belongs to the independent reviewer;
the implementer does not assign its own quality score.
