# Lane brief: the versioned onboarding / recovery card — increment 1 (taurhaus)

Commissioned by the orchestrator on the operator's go (2026-09-08).
Implementation lane on Astra (`gpt-6-astra`, high) through `feature-pr`;
Opus lenses; orchestrator acceptance. Repository: **taurhaus**, worktree
`~/projects/taurhaus-onboard`, branch `feat/onboarding-card` off main.
Nothing installed, nothing released; the daemon on port 17233 and the
standing teams are never touched. The mesh half of the accepted design
(assignment/wait/correction projection, delivery identities) waits for
the assignment-card and ledger branches to land and is NOT this lane.

## Assignment

1. **Objective:** one compiled `RecoveryCard` replaces today's separate
   onboarding and compaction renderers: delivered once per context
   generation (activation or admitted compaction), keyed by the accepted
   identity tuple, with the baseline obligation claimed under the
   existing runtime lock, receipts that separate accepted from
   consumed, an operator-forced reonboard that mints a generation, and a
   pending obligation for a skipped compaction carried by the next
   delivery — with `render_role_sections` byte-stable for the agent
   export.
2. **Deliverable:** commits on `feat/onboarding-card` implementing
   increment 1 of `docs/design/onboarding-card-design.md` (its Result,
   "Identity and version contract", "Delivery rule and event table" rows
   for activation / cold resume / effort relaunch / admitted compaction /
   daemon restart / nudge / reonboard, "Bounded corrections",
   "Recovery card contents", "Receipts", "Ownership and persistence",
   and the Recommendation's acceptance fixtures), with tests; RESULT
   cites tip, tests, diff budget.
3. **First action:** read the design's identity table and the ownership
   table, then `src-tauri/src/coordination/pipelines/lifecycle.rs`
   (`prepare_*_onboarding_delivery`, `render_onboarding_message`),
   `pipelines/members.rs` (`deliver_onboarding`,
   `deliver_resume_onboarding`, the capture-identity → deliver → commit
   order at ~646–663), `coordination/reinjection.rs` (`compose`,
   `render_additional_context_text`), `coordination/compact_hook.rs`
   (~460–657), `coordination/delivery.rs` (`render_role_sections` and
   its doc comment), `templates/agent_definitions.rs:51`,
   `coordination/stores/{runtime,operational,compaction}.rs`,
   `daemon/team_runs.rs` (`reonboard` ~787–857) and
   `daemon/team_move.rs`; then write the red tests: one baseline per
   context across init / add / resume / reonboard / worker-restart
   retries; a forced reonboard mints a generation; a skipped compaction
   records a pending obligation satisfied by the next delivery;
   `render_role_sections` output unchanged (existing goldens).
4. **Completion signal:** feature-pr `complete`; RESULT.
5. **Review route:** implement; Opus conformance + operational; up to
   three fix rounds; orchestrator accepts. A lane closes at "no majors,
   gate green"; minors are listed, not fixed in extra rounds.

## Minimum deliverable

- **Identity:** a persisted team-incarnation id and member-incarnation
  id (typed fields on the existing team config / runtime record, minted
  once, preserved across root moves and rejoin, new on recreation or
  seat replacement; legacy records → `generation_unknown`, never
  suppression on a guessed identity); an activation generation reserved
  by the managed activation owner BEFORE delivery (the order becomes
  capture identity → reserve generation → deliver → commit); the
  compaction generation advanced once per admitted boundary
  (`Injected` OR `Skipped`) by the existing hook bookkeeping; the root
  tuple carried from `TeamRootRegistry`; `card_key`, `view_key`,
  `obligation_key = (recipient, context)`, `delivery_id =
  hash(card_key, delivery_kind)`, `content_revision = digest(view_key)`
  exactly as the design's three-key table states.
- **One compiler, one card:** a `RecoveryCard` type composed from
  authorized structured facts (identity/coverage, roots, assignment/
  wait from the operational snapshot, candidate/rubric refs when
  present, restart cursor link, corrections since the last delivered
  revision, evidence/handoff facts within audience, minimal steering)
  rendered by a NEW bounded `render_card_steering`; the existing
  onboarding and compaction renderers become callers of the one
  compiler; `render_role_sections` stays byte-identical (existing
  goldens in `src-tauri/tests/cli_renderers.rs` must not change; add a
  golden for the steering block).
- **Delivery rule:** init / add / cold resume / effort relaunch deliver
  ONE baseline per `(recipient, context)`, claimed under the existing
  team/runtime lock, retried under the same `delivery_id` with the
  content revision on the receipt; a daemon or watcher restart delivers
  nothing; an unforced `reonboard` against an unchanged context returns
  the existing delivery status; `reonboard --force` (IPC + daemon
  intent) mints a new activation generation, delivers, and logs
  `onboarding.generation.forced` with team/member/reason; a real
  root/role/contract/packet change produces a correction (its own
  delivery id, naming the last delivered revision) — latest version
  wins, one attempt plus one retry, single runtime pointer, no
  coalescing state.
- **Compaction:** the existing hook path admits the boundary
  (delivered or recorded); a `Skipped` delivery records a `pending`
  baseline obligation in the existing compaction store, satisfied by
  the next delivery to that seat (assignment delta, correction, forced
  reonboard, or explicit recovery read) and marked by that delivery's
  receipt; detection, idempotency and the compat-import dedup are
  untouched.
- **Nudges:** the taurhaus deadline nudge carries task id, token/stage,
  wait/release state and next action — never the card.
- **Receipts:** on the existing delivery/telemetry surfaces:
  `delivery_id`, obligation key, content revision, attempt, path, stage
  (`accepted` for a durable inbox append; `hook_response_offered`;
  `submitted` with the tmux evidence; `consumed_by_read`), `generated_bytes`
  vs `accepted_bytes` kept separate; unknown outcomes recorded as
  `outcome_unknown`, never resent across adapters.
- **Tests, red first,** per the design's acceptance list: the baseline-
  once matrix; team recreation / member replacement / root move
  (`team_move.rs`) / preserved-session relaunch / admitted compaction
  produce the correct distinct keys and daemon restart alone does not;
  the same-context correction supersedes the old acknowledgment rule
  and cannot release GO; receipt stages survive append-before-bookkeeping
  and hook-output ambiguity; waiting / unassigned / terminal recovery
  never says "continue immediately"; independent reviewers see no peer
  verdict; goldens for every renderer caller. Fixtures via the existing
  test harness in tempdir roots with fake transports; never a real CLI,
  tmux, or the live daemon.

## Not building

- The mesh half (assignment/wait/correction projection and delivery
  identities on the mesh side) — waits for the card and ledger
  branches; any message journal, scheduler, watcher, native push;
  changes to compaction detection; role YAML edits; install, release,
  or daemon restart.

## Constraints

- Diff budget ≤ 3,500 inserted lines (gross, Cargo.lock excluded); no
  new dependencies. Gates: `just check-quick`, `just lint`, `just
  test-contracts`, plus the Rust-diff rule's `just test-rust-unit`.
  Taurhaus cargo builds are HEAVY: strictly one cargo job at a time
  machine-wide (poll `pgrep -af '(^|/)cargo( |$)'`, wait up to 30 min);
  use this worktree's own `target/`.
- Never touch the live daemon (17233) or its data dir, the operator's
  tmux server, `~/.claude*`, `~/.codex`, `~/.gemini`, `~/.grok`, the
  standing teams; never `just install-daemon`.
- Commit after every green step; never `git add -A`.
