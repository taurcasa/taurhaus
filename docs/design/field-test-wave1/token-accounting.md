# Wave-1 token accounting (recovered post-hoc from transcripts and rollouts)

Sources: Claude Code transcripts (`~/.claude/projects/-home-mstie-projects-taurjob/`,
8 files = 4 seats × pre/post-outage) and Codex rollouts (session_meta cwd
= taurjob, 10 rollouts = 5 seats × 2). Codex figures are the final
cumulative `total_token_usage` per rollout; Claude figures are summed
per-message usage. Recovered 2026-09-07; telemetry v2 should collect this
natively.

## Codex seats (both team generations summed)

| Seat | Model | Total input | of which cached | Fresh input | Output |
|---|---|---:|---:|---:|---:|
| implementer-1 | gpt-5.6-sol | 283.3M | 280.3M | 3.05M | 733K |
| heavy-implementer | gpt-6-astra | 78.7M | 76.6M | 2.20M | 380K |
| heavy-implementer-1 | gpt-6-astra | 97.3M | 95.6M | 1.67M | 373K |
| judge-astra-1 | gpt-6-astra | 58.0M | 56.1M | 1.93M | 221K |
| architect | gpt-6-astra | 17.9M | 17.2M | 0.68M | 67K |

## Claude seats

| Seat | Model | Fresh input | Cache write | Cache read | Output |
|---|---|---:|---:|---:|---:|
| lead-taurjob | fable | 55K | 6.8M | 1,292M | **5.16M** |
| product-reviewer | opus | 4K | 5.5M | 861M | 1.95M |
| design-taurjob | fable | 22K | 12.2M | 477M | 1.62M |
| altitude-reviewer | fable | 28K | 4.7M | 392M | 1.53M |

## The Sol-vs-Astra per-task comparison (Decision 2's question)

Compute-bearing tokens (fresh input + output) per accepted task
(routing-report acceptance; heavy row split 5/4 by ledger ownership):

| Seat | Compute tokens | Accepted | Per accepted task |
|---|---:|---:|---:|
| implementer-1 (Sol) | 3.78M | 3 | **1.26M** |
| heavy-implementer (Astra) | 2.58M | 5 | **0.52M** |
| heavy-implementer-1 (Astra) | 2.04M | 4 | **0.51M** |

Under the known ~2.5× Astra-per-token premium, 2.5 × ~0.4 ≈ **1.0: the
per-accepted-task cost lands near parity** — the operator's conjecture
("Sol cheaper per token but maybe not per task") is confirmed by this
wave's data, with Astra delivering the higher-graded quality at that
parity price.

**Stated assumptions and confounds** (they move the exact number, not the
ballpark): output and input priced equally within a family; cached input
at deep discount both families; Astra's costlier cache reads partially
offset Sol's 3.6× larger cache-read volume; and Sol's three tasks were
larger on average (M2 + the whole integration lane + the scanner) while
running at medium effort — the wave-2 controlled experiment (Sol at high,
numeric contract, clean lane) remains the decider.

## The ceremony bill, now in tokens

The lead's **5.16M output tokens exceed all five Codex seats' combined
output (1.77M) by ~3×** — rulings, ledger upkeep, and notices were the
wave's single largest generation load. Its 1.29 **billion** cache-read
tokens are the hundreds of superseded inbox notices being re-read in
context. The ~77–81%-of-commits ceremony estimate now has a token-side
counterpart: coordination text was the wave's dominant model product.
Review seats together (product + altitude + design + judge): ~5.3M output
— roughly 3× implementation output as well.

Wave totals: ~535M input-class tokens Codex-side (97% cached), ~3.0B
cache-reads + 29M cache-writes + 10.3M output Claude-side.
