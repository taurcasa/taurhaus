# Compaction Behavior After Fresh Deploy — 2026-03-09

## Scope

Verify the fresh deploy after the recent compaction fixes:

- wrong guard removal
- event-driven extractor
- diff-based daemon fanout
- inbox corruption fix
- dead code removal

Checks performed:

1. `python3 scripts/analyze-compaction.py --team taurhaus-team --last 1h`
2. searched both app logs for:
   - `compaction.detected`
   - `compaction.injected`
   - `compaction.skipped`
   - `skip_reason`
   - `fail_reason`
   - `compaction.claude_hook.*`
3. checked daemon binary and `/proc/<pid>/exe`
4. sampled daemon CPU with `ps` and `top`
5. inspected raw Codex JSONL boundaries to explain `compacted` vs `context_compacted`

## Result

End-to-end Codex compaction detection and delivery are now working on the fresh deploy.

For `taurhaus-team` in the last hour:

- detected: `5`
- injected: `5`
- skipped: `0`
- stale: `0`
- failed: `0`
- injected/detected ratio: `100%`

The pipeline is healthy at the checkpoint level:

- scanner cycles present and non-zero session counts
- runtime session IDs fully populated
- compaction signals emitted
- managed member resolution succeeds
- terminal delivery outcome follows detection

## Evidence

### Analyzer

`python3 scripts/analyze-compaction.py --team taurhaus-team --last 1h`

Key outputs:

- selected log: `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- parsed lines: `325522/325537`
- invalid JSON lines: `15`
- latest scanner run: `run_6127a4a0ba9d4284921f312b23b19368`
- latest run cycles: `291`
- latest run session count: always `15`
- runtime members with session ID: `10/10`
- `taurhaus-team/architect`: detected `5`, injected `5`, skipped `0`, stale `0`, failed `0`
- detection -> injected latency:
  - min: `11ms`
  - median: `18ms`
  - max: `25ms`

### App logs

Fresh `taurhaus-team` injected events are present in the Windows app log:

- `2026-03-09T00:20:08.826Z` `compaction.injected` for `architect`
- `2026-03-09T00:20:08.848Z` `compaction.injected` for `architect`
- `2026-03-09T00:30:21.657Z` `compaction.injected` for `architect`
- `2026-03-09T00:30:21.673Z` `compaction.injected` for `architect`
- `2026-03-09T01:14:54.779Z` `compaction.injected` for `architect`

No fresh `taurhaus-team` `compaction.skipped` or `compaction.failed` events appeared in the selected 1-hour window.

Because there were no skips/failures in this window:

- no `skip_reason` values were emitted
- no `fail_reason` values were emitted

### Claude hook

Current state:

- hook installed: `true`
- compact matcher present: `true`
- hook script exists: `true`
- configured command:
  - `\\wsl.localhost\Ubuntu\home\mstie\.claude\hooks\taurhaus-session-start-compact.cmd`

But:

- no `compaction.claude_hook.*` evidence appears in either app log for the selected window

So Claude hook installation looks correct, but this check did not observe a real Claude compaction fire.

## Daemon binary state

Live daemon:

- PID: `2139214`
- command: `/home/mstie/.local/bin/taurhaus-daemon --port 17233`
- `/proc/2139214/exe -> /home/mstie/.local/bin/taurhaus-daemon`

Important:

- it does **not** show `(deleted)`
- so the daemon is running the current installed binary path, not a stale deleted executable

## Daemon CPU

Observed samples:

- `ps`: `50.5%` CPU at `02:16`
- `ps` 3 seconds later: `50.3%`
- `top -bn1 -p 2139214`: `46.7%` CPU

Compared to the old reported baseline of `61%`, this is improved, but still high.

Conclusion:

- diff-based fanout likely helped
- daemon CPU is still materially elevated and remains worth further profiling

## Why some signals say `Compacted` and others say `Context compacted`

This is not two tools or two paths. It is two distinct boundary records inside the same Codex transcript JSONL.

Raw JSONL inspection for the active `architect` transcript shows the same pattern repeatedly:

- `type: "compacted"`
- immediately followed by
- `type: "event_msg"` with `payload.type: "context_compacted"`

Concrete examples from:

- [rollout-2026-03-05T12-56-33-019cbddb-5527-77a0-a457-7908cf7d790b.jsonl](/home/mstie/.codex/sessions/2026/03/05/rollout-2026-03-05T12-56-33-019cbddb-5527-77a0-a457-7908cf7d790b.jsonl)

Observed pairs:

- `2026-03-09T00:20:08.784Z` `compacted`
- `2026-03-09T00:20:08.814Z` `context_compacted`
- `2026-03-09T00:30:21.617Z` `compacted`
- `2026-03-09T00:30:21.631Z` `context_compacted`
- `2026-03-09T01:14:54.753Z` `compacted`
- `2026-03-09T01:14:54.765Z` `context_compacted`

So the mixed signal names are expected transcript content from Codex.

## Remaining bug

There is still one correctness problem in the fresh deploy:

- paired Codex boundaries are still generating duplicate deliveries

Evidence:

- one user-visible compaction episode can generate two `compaction.detected` and two `compaction.injected` events a few milliseconds apart
- `architect` at `00:20:08` and `00:30:21` both show this exact duplication

That means:

- end-to-end delivery works
- but paired-boundary normalization is still not collapsing `compacted` + `context_compacted` into a single delivery

## Actionable next steps

1. Fix paired-boundary deduplication in the extractor so one Codex compaction episode produces one downstream delivery.
2. Add regression coverage on the exact transcript pattern:
   - `type="compacted"`
   - followed immediately by `event_msg.payload.type="context_compacted"`
3. Profile daemon CPU now that correctness is improved:
   - likely focus on remaining scanner cadence and any repeated runtime/config work still happening at high frequency
4. Separately verify a real Claude compaction event so the installed hook path is exercised, not just statically validated.

## Bottom line

- Signals are being detected: yes
- Deliveries are succeeding: yes, `5/5` injected for `taurhaus-team` in the last hour
- Are they still skipping: no, not in this 1-hour window
- Daemon CPU improved: yes, from the old `61%` baseline down to about `47–50%`, but still too high to call solved
- Remaining issue: duplicate Codex deliveries from paired transcript boundaries
