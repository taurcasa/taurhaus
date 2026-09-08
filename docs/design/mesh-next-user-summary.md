# Mesh-next: what would change for you

A user-facing summary of the planned mesh/messaging/ledger overhaul,
written for the wave-2 team's review (2026-09-08). This is the concept
as USERS would experience it — the full design corpus lives in
docs/design/ (threads study, messaging overhaul study, ledger study,
native-push probe) for any seat that wants the depth. Where this
summary and the studies disagree, the studies govern.

## Why (this is your retro talking)

Crossed messages cost real time on your last two lanes. The task record
could not carry what messages carried — a released GO invisible from
`mesh task get`, a stale `blockedBy` nobody could remove, `blocked`
displayed for an active checkpoint. A mutable RESULT artifact leaked one
reviewer's verdict to another. The lead spent 109 mainline commits
maintaining ledger.md by hand. Two telemetry contracts turned out to be
unconnected pipes. The overhaul targets exactly these.

## What stays exactly as you know it

The five-line assignment contract. RESULT-first lines with hashes.
INFO ONLY with no acknowledgments. Declared waits and awaiting-GO.
Diff-confirms for bounded corrections. The proportionality rule.
Worktree isolation. Task lifecycle authority — nothing in any message,
thread, or ledger entry ever changes task state, a verdict, a budget,
or a GO; the existing commands remain the only way. Two-family review
routes and one-round-by-default.

## 1. The task record becomes the carrier

What today travels in messages moves onto the task record, delivered
WITH creation: the accepted base, the release condition, the packet
hash, the effort and its reason. `mesh task get` shows the GO state
and its release explicitly. Dependency edges become removable with an
audit line (no more permanent stale `blockedBy`). Progress notes work
on completed tasks. A RESULT that names a tip marks that step answered,
so a crossed later instruction is flagged instead of silently racing
you. A moved tip is re-frozen by the lead in the task record — never by
an owner's INFO ONLY.

## 2. Messages: retained once, delivered directed, readable on demand

All accepted messages land in one append-only journal per team
(authored bodies never lost again — 58 of wave-1's 86 sent messages
have no retained body today). Delivery stays DIRECTED: you receive what
is addressed to you, exactly as now. There are NO default
subscriptions and no obligation to read any thread — that is a
deliberate, measured decision (shared reading costs 1.7×–37× the
directed baseline). What you GAIN is on-demand access: any task's
evidence is inspectable as a bounded projection — a current card
(operative assignment, wait state, latest result, applicable rulings)
plus deltas since your last look. You choose when that read is worth
it. DMs stay DMs. Independent review gets enforcement: a submitted
RESULT is frozen by content identity at submission — the #65 leak
becomes structurally impossible, not a rule to remember.

## 3. Delivery: one owner, honest receipts, fewer keystrokes

A single per-team scheduler owns delivering messages (today: your
inbox plus terminal keystrokes that can interleave with your typing).
For Claude and Codex seats, native input channels progressively replace
keystroke injection; the terminal path remains as a guarded fallback.
Receipts become honest and typed — "submitted", "enqueued",
"consumed" are different facts, and nothing pretends a delivery was a
read or a read was uptake.

## 4. The ledger: you never edit ledger.md again

Ledger content becomes small appendable events: you write a short
markdown file (a few front-matter fields above literal prose — no JSON
escaping, no shell-argument mangling) and run one command to submit
it. Corrections are explicit supersessions with reasons — history is
never rewritten, and your honest outcomes, limitations, and remaining
items are first-class fields that cannot be silently dropped.
`ledger.md` becomes a GENERATED rendering, produced at wave boundaries
from the record — nobody hand-maintains tables, cells cannot go stale,
and malformed rows cannot exist. Ordinary facts (lifecycle, rulings,
budgets, completions) are projected from where they already live; you
only author what is genuinely new.

## 5. Waits and nudges

Your declared waits keep working as they did this wave. The monitor
learns your retro's lesson: it reads `started_at`, your cursor, and the
lifecycle before nudging — no more nudges at active reviewers or
completed tasks. And the telemetry gap that made this wave's zeros
unmeasurable (mesh's records never reaching the report; the GO marker
read in the wrong format) is being fixed now, ahead of everything else.

## 6. What it costs you

Honesty section. A few new verbs to learn (every one execution-
validated against the real binary before it enters your role text). A
small file plus one command per ledger entry, where today you write
prose into a message anyway. The task-evidence reads are optional but
tempting — the design deliberately does NOT push them at you, because
your attention is the scarcest budget in the system. If a part of this
would make your seat's work slower, noisier, or more ceremonial, that
is precisely what your review should say.
