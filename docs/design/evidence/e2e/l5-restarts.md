# Lane 5 — IN PROGRESS; no runtime verdict yet

Pending mail across the ordinary Taurhaus daemon and Mesh team-owner restarts.
Candidate: Taurhaus `a7e6db7e`, protocol 27; Mesh `310144d`, shipped 0.153.4
app-server descriptor enabled and unchanged. Exact controller and offline checks
are in [l5-restarts/](l5-restarts/). No product change.

The lane spec's stricter limits govern: 12 Codex inputs, USD 0.25, 15 minutes.
Model is `gpt-5.6-luna`, effort `low`; login-only Claude lead takes no paid turns.
Metering is retained independently and never gates a lifecycle operation.

| Step | Outcome | Classification |
|---|---|---|
| 1 Initialize and complete/read baseline | PASS | Runtime; read-only hosted-activity observer corrected before baseline |
| 2 Observe both pending backlogs | Not run | — |
| 3 Restart Taurhaus normally | Not run | — |
| 4 Deliver backlog without baseline replay | Not run | — |
| 5 Restart Mesh with fresh pending mail | Not run | — |
| 6 Reconcile/read and teardown | Not run | — |

Provenance: messaging trial run2 controller/actions/support/retention read with
`git -C /home/mstie/projects/taurhaus-msg show HEAD:<path>`. The named
`taurhaus-trial` checkout was absent (git exit 128); attempt9 controller/actions/
steps/support were read from the identical named tracked paths in this checkout.
No other Taurhaus checkout was changed. Mesh source is read/build-only.

Offline TDD: acceptance-test discovery exited 1 before support existed; four
checks then passed, exit 0. Tests use generated data only. Runtime acceptance is
not inferred from offline tests. The required independent Opus evidence lens is
left to the invoking small-change orchestrator; this is its Codex implementer.

Builds use `build.py`; gates run only after the controller's cleanup record exists.
Exact gates: `just check-quick`, `just lint`, `just test-contracts`. Rust unit
execution is conditional on a `src-tauri/` diff; none is planned.
