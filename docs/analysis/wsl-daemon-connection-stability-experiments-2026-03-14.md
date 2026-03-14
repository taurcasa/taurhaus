# WSL Daemon Connection Stability Experiments

## Objective

Run one idle baseline and one controlled load experiment against the current WSL daemon build, try to reproduce the reported disconnect behavior, and record the first measurement set with a defensible interpretation.

## Setup

- Binary under test: `/home/mstie/.local/bin/taurhaus-daemon`
- Binary version: `0.5.10`
- Experiment daemon address: `127.0.0.1:17299`
- Auth: current token from `/home/mstie/.local/share/taurhaus/daemon.token`
- Reason for isolated port: keep the live `17233` daemon lane undisturbed while still testing the exact installed binary and protocol
- Heavy-load repo: `/tmp/daemon-load-measure/repo`
  - initialized as a normal git repo
  - `1` tracked file committed
  - `100000` additional untracked files created under `bulk/`

The heavy repo was chosen after a quick benchmark showed that toy repos were too cheap to create meaningful overlap. On this 100k-file dirty repo, direct daemon `git_status` was consistently around `0.4 s`, which is enough to exercise contention without inventing synthetic sleeps.

## Scenarios

### 1. Idle baseline

- one persistent TCP client
- `200` sequential `ping` requests
- `50 ms` gap between requests
- no competing daemon work

### 2. Controlled load

Ran for `15.43 s` with three lanes at once:

- `8` persistent worker clients continuously issuing `git_status` against the 100k-file dirty repo
- `1` separate persistent probe client issuing `ping` every `100 ms`
- `1` app-like shared-connection probe:
  - one background loop kept the shared connection busy with `git_status`
  - one foreground status probe tried to acquire that same connection non-blocking every `50 ms`
  - if the connection was already in use, the probe recorded a fast-fail `busy` outcome instead of queueing

That third lane is the closest reproduction of the Taurhaus shared-client transport model, where a foreground status read competes with another request on the same `DaemonProvider` connection.

## Results

### Idle baseline

| Metric | Value |
| --- | --- |
| Sample count | `200` |
| `ping` min | `0.150 ms` |
| `ping` p50 | `0.208 ms` |
| `ping` p95 | `0.289 ms` |
| `ping` max | `9.338 ms` |
| `ping` mean | `0.258 ms` |
| Errors | `0` |

### Controlled load

#### Daemon transport stayed up

| Metric | Value |
| --- | --- |
| Duration | `15.43 s` |
| `git_status` samples | `289` |
| `git_status` p50 | `416.042 ms` |
| `git_status` p95 | `438.627 ms` |
| `git_status` max | `475.443 ms` |
| Separate probe `ping` samples | `150` |
| Separate probe `ping` p50 | `0.238 ms` |
| Separate probe `ping` p95 | `0.316 ms` |
| Separate probe `ping` max | `49.012 ms` |
| Transport/protocol errors | `0` |
| Post-load sanity `ping` | success in `35.602 ms` |

#### Shared-client contention was easy to reproduce

| Metric | Value |
| --- | --- |
| Non-blocking shared probe `busy` fast-fails | `300` |
| Shared-connection successful call samples | `36` |
| Shared-connection successful call p50 | `414.470 ms` |
| Shared-connection successful call p95 | `439.612 ms` |
| Shared-connection successful call max | `478.376 ms` |

## What I Reproduced

I did **not** reproduce a raw WSL daemon socket drop, connection reset, auth failure, or daemon-side refusal under this load. The daemon remained reachable throughout the run, and an independent probe connection kept answering normally while eight other clients were hammering `git_status`.

I **did** reproduce the app-visible degradation pattern that matters for Taurhaus:

- when one client treats a single daemon connection as a shared lane
- and a slower request keeps that lane occupied
- foreground status-style probes hit immediate `busy` outcomes instead of making forward progress

That is consistent with the existing startup-freeze finding from [taurhaus-startup-freeze-investigation-2026-03-13.md](./taurhaus-startup-freeze-investigation-2026-03-13.md): the unstable feeling comes from shared-client contention, not from the daemon process falling over under modest concurrent load.

## Interpretation

The first measurement set points to this conclusion:

1. The current WSL daemon transport is stable under the tested concurrent load.
2. The most repeatable failure mode is not daemon disconnect; it is shared-connection starvation at the client boundary.
3. If Taurhaus still surfaces a "daemon disconnected" feeling during heavy work, the next place to look is:
   - client-side timeout or recovery behavior on the shared provider connection
   - foreground paths that still escalate a contention event into a disconnect or fallback path
   - a different lane than the one covered here, such as real Windows UNC access plus session/task work in the full app runtime

## Remaining Risks

- This was an isolated daemon instance on the installed binary, not the live port `17233`.
- The heavy workload was WSL-local git dirtiness, not the full Windows app mix of UNC-backed project access, session bridge activity, task scans, and UI-triggered IPC fan-out.
- The load source was enough to create overlap, but not enough to push `git_status` anywhere near the `30 s` transport timeout budget.
- I did not reproduce a true disconnect in this run, so the original field symptom may require a wider end-to-end runtime mix than direct daemon TCP load alone.

## Recommendation

Treat the current evidence as a narrowing result:

- the daemon itself is not the first suspect for the present load symptom
- the next experiment should be an end-to-end Taurhaus runtime run that correlates:
  - `daemon.connection.*`
  - `daemon.rpc.*`
  - shared-provider busy outcomes
  - frontend-visible stale/disconnected state

That is the shortest path to determining whether any remaining "disconnect" reports are real transport loss or client-side contention being surfaced as loss.
