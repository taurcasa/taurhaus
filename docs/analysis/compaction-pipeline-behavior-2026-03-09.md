# Compaction Pipeline Behavior — 2026-03-09

Task: `#758`  
Owner: `architect`  
Date: `2026-03-09`

## Scope

This analysis checked current live compaction behavior using:

- `python3 scripts/analyze-compaction.py --team taurhaus-team --last 2h`
- app logs at:
  - `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`
  - `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

The running binary/daemon is not the latest source tree. Team-lead explicitly noted the deployed runtime is still from commit `aa7c50d` and does not yet include the newest audit cleanup fixes.

## Executive Summary

1. New compaction signals are definitely being detected.
2. The currently running Windows app has proven end-to-end **Codex injection works** for at least one live team/member.
3. For `taurhaus-team`, the most recent compaction visible in the 2-hour window is still a **skipped** event from an earlier run, not from the newest run.
4. Deployed logs in that older skip path still do **not** include `skip_reason`, so the exact blocking guard cannot be proven from logs alone.
5. Claude compact-hook installation now looks healthy, but there is still **no hook fire evidence** in the selected window.

## Analyzer Output

Command run:

```bash
python3 scripts/analyze-compaction.py --team taurhaus-team --last 2h
```

Key output:

- selected log: `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- parsed lines: `296286/296295`
- health summary:
  - `Compaction pipeline: compactions detected but none injected`
  - `Runtime session_id health: all 10 runtime members have session_id`
  - `Claude hook status: installed, but no hook fire evidence in selected window`
- team-specific outcomes for `taurhaus-team`:
  - detected: `1`
  - injected: `0`
  - skipped: `1`
  - stale: `0`
  - failed: `0`

Important nuance:
- the analyzer is team-filtered to `taurhaus-team`
- it therefore correctly says “none injected” for that team in the selected 2-hour window
- that does **not** mean the deployed global pipeline cannot inject at all

## What The Logs Show

## 1. New compaction signals are being detected

Yes.

Evidence:

- older Linux-side log still contains fresh `taurhaus-team` detections such as:
  - `2026-03-08T23:05:30.822Z` `compaction.detected` for `architect`
- current Windows app log contains fresh detections such as:
  - `2026-03-09T00:13:27.358Z` `compaction.detected` for `2ksim-team/developer3`
  - `2026-03-09T00:13:27.385Z` second detected event for the paired `context_compacted` timestamp

Conclusion:
- extractor + watcher + processor are alive and emitting real detections

## 2. Terminal outcome is mixed across runs, but the newest run proves injection works

### `taurhaus-team` in the selected 2-hour window

Observed terminal outcome:

- still `compaction.skipped`
- latest visible team event in the window:
  - `2026-03-08T23:37:26.002Z` signal timestamp
  - terminal event recorded as `compaction.skipped`
  - member: `architect`
  - session: `019cbddb-5527-77a0-a457-7908cf7d790b`

### Current Windows app run

Observed terminal outcome:

- `compaction.injected` is happening

Evidence in `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`:

- `2026-03-09T00:13:27.384Z` `compaction.injected`
- `2026-03-09T00:13:27.391Z` `compaction.injected`
- both for `2ksim-team/developer3`

Conclusion:
- the deployed runtime is capable of reaching `Injected`
- this is not a system-wide “still always skipping” failure anymore

## 3. `skip_reason` is still not observable in the deployed logs

I searched both log locations for:

- `skip_reason`
- `fail_reason`
- `compaction.skipped`

Result:

- deployed skipped events do **not** carry `skip_reason`
- the analyzer reports the same thing:
  - `Structured compaction events do not currently include skip/fail reason fields`

Conclusion:
- for the currently running binary, exact skip-cause diagnosis from logs alone is still blocked
- the newer observability work is not deployed yet

## 4. Claude compact hook: installed, but no fire evidence

Searches run against both log locations for:

- `compaction.claude_hook.*`
- `claude-compact-hook`
- `SessionStart`

Result:

- no hook fire records in either log location for the selected window

Analyzer status:

- settings file exists
- hook installed: `True`
- compact matcher present: `True`
- hook script exists: `True`
- configured command points at the Taurhaus compact-hook wrapper
- `Hook fire evidence: none in selected window`

Conclusion:
- installation looks healthy now
- there was simply no observed Claude compaction-hook execution in the chosen window

## Taurhaus-Team Specific Interpretation

The strongest supported conclusion is:

1. `taurhaus-team` still has an older skipped compaction event in the selected window.
2. That skipped event is from an earlier run, not from the newest Windows run that proved injection for another team.
3. There is **no fresh post-restart `taurhaus-team` compaction** in the inspected window to prove whether `taurhaus-team` itself would now inject under the current deployed app.

That means:

- we **cannot** honestly say “taurhaus-team is still broken right now”
- we also **cannot** honestly say “taurhaus-team is fixed” without forcing a fresh compaction for that team

## Likely Cause Of The Older `taurhaus-team` Skip

Because `skip_reason` is absent in the deployed logs, this is inference, not proof.

Most likely explanation:

- stale attachment/runtime mismatch at the time of the older `architect` compaction
- historically this was the dominant cause for `architect` skip behavior
- it is consistent with the old `session_id` / attachment drift investigations from `#692`, `#703`, and `#704`

Why I am not claiming more:

- current runtime files now show healthy attachment state for `architect`
- current runtime for `architect` has:
  - pane `%217`
  - session `019cbddb-5527-77a0-a457-7908cf7d790b`
  - matching transcript path
- that is current state, not guaranteed historical state at the time of the older skip event

So the exact old blocking guard remains unproven until a build with `skip_reason` logging is deployed or a fresh skip is reproduced.

## Additional Observations

## Runtime health is materially better now

Current `taurhaus-team` runtime records show:

- all `10` managed members have `session_id`
- `architect`, `developer1`, and `developer2` currently all point at the same session/transcript:
  - `019cbddb-5527-77a0-a457-7908cf7d790b`

That shared-session pattern matters:

- it means `taurhaus-team` still has multiple Codex members attached to the same transcript/session
- the current processor can still resolve a specific member if pane correlation is unique
- but it remains a fragile shape operationally and should be treated as higher-risk than one-member-per-session

## Actionable Next Steps

1. Deploy the latest build that includes the newer compaction observability work.
   - Without deployed `skip_reason`, old skipped events are still partly opaque.

2. Force a fresh Codex `/compact` in `taurhaus-team` under the current deployed app.
   - This is the shortest path to answering whether `taurhaus-team` still skips or now injects.

3. If that fresh `taurhaus-team` event still skips after deployment, inspect `skip_reason` first.
   - likely candidates are attachment validity / stale runtime state, not global pipeline failure

4. Trigger a real Claude compact for the managed `team-lead` session while watching logs.
   - installation is present
   - the missing piece is observed hook execution, not installer state

5. Longer-term hardening:
   - reduce same-transcript multi-member Codex attachment where possible
   - that shape is still operationally fragile even if current pane-based resolution works

## Bottom Line

Current live evidence says:

- **Detection works**
- **Injection works in the deployed binary**
- **`taurhaus-team` still only has older skipped evidence in the selected window**
- **Claude hook installation is present, but no hook fire was observed**

So the next correct step is not more abstract debugging. It is:

- deploy the newer observability build
- force one fresh `taurhaus-team` Codex compaction
- read the terminal outcome and `skip_reason` from the deployed logs
