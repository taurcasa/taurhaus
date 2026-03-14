# Windows Startup White Screen Investigation

## Task

**Task:** #1291  
**Owner:** dev-1  
**Status:** in_progress

Investigate the current Windows startup regression where Taurhaus launches to a white screen and hangs before normal UI startup completes.

## Scope

Treat this as a production-quality root-cause investigation, not a superficial symptom check.

Required investigation angles:

- Windows production log review
- current resource/usage monitor output if it is still being produced
- startup pipeline review across frontend bootstrap, backend startup phases, daemon/provider initialization, and runtime session sync
- Windows/WSL/UNC path normalization and provider-path handling
- regression review against the recent bounded session-resilience changes on `feature/session-activity-daemon-stability`

## Required evidence

- Canonical Windows log path:
  - `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- WSL-visible equivalent:
  - `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- Resource monitor output if current:
  - `/tmp/taurhaus-resource-monitor-v2.csv`

## Specific normalization check

Do not treat this as a generic startup hang until the path layer is explicitly cleared.

Verify whether the recent changes accidentally regressed any of:

- Windows drive path -> Linux path normalization
- WSL UNC path -> Linux path normalization
- provider-path reuse during startup/bootstrap
- daemon/client path identity checks
- any startup code path that now mixes:
  - `\\wsl.localhost\...`
  - `/home/...`
  - `/mnt/...`
  - provider-native paths

## Completion bar

This task is only complete when it includes:

- the real failing startup path or contention point
- concrete evidence from logs and/or code
- the root-cause fix if the issue is in current Taurhaus code
- regression coverage where appropriate
- a clear note on whether Windows/WSL path normalization was involved or ruled out
