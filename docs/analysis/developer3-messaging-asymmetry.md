# Developer3 Messaging Asymmetry

Date: 2026-03-07
Task: #510
Owner: architect

## Summary

Root cause is on the inbound notification path for `developer3`, not on `mesh send`.

- Messages from `team-lead` to `developer3` are arriving on disk in `~/.claude/teams/taurhaus-team/inboxes/developer3.json`.
- `developer3` is not receiving the tmux wake-up notifications that prompt a Codex agent to run `mesh read --unread --mark-read`.
- Outbound messaging from `developer3` to `team-lead` still works because `mesh send` does not depend on the per-agent inbox daemon.

The failure correlates with the earlier pane kill/resume: `developer3` ended up without a stable, healthy per-agent `mesh daemon` process after resume.

## Evidence

### 1. Inbox delivery is working

`developer3.json` contains the missing `team-lead` messages from the period where delivery was reported broken.

Observed examples in the inbox tail:

- `2026-03-07T12:02:53.459Z` `team-lead`: status check on `#504`
- `2026-03-07T12:03:40.438Z` `team-lead`: continue `#504`

Those messages were present on disk but remained unread during the failure window.

This rules out:

- missing inbox file
- `mesh send` failing to append messages
- recipient name resolution failure

### 2. `developer3` pane mapping exists, but daemon/runtime state is inconsistent

Current config/runtime state during investigation:

- `config.json`: `developer3.tmuxPaneId = %224`
- `runtime/developer3.json`: `pane_id = %224`, `health = healthy`, `daemon_pid = 32396`

But the runtime `daemon_pid` was stale:

- PID `32396` was not the live daemon process

This already showed that the resume/repair path had left runtime metadata out of sync with reality.

### 3. The failed messages happened before any live `developer3` daemon existed

The missing `team-lead` messages were timestamped around `12:02` to `12:04`.

The only live `developer3` daemon process found during investigation had start time:

- `2026-03-07 12:26:50`

That means there was no live per-agent daemon for `developer3` at the time those inbound messages were being sent.

This is the cleanest explanation for the asymmetry:

- outbound `developer3 -> team-lead`: works
- inbound `team-lead -> developer3`: lands in inbox but no daemon wakes the agent

### 4. Working-agent comparison shows the missing behavior clearly

Comparison agent: `communication-analyst`

- pane `%225`
- live daemon with a PID file under `~/.claude/teams/taurhaus-team/daemons/communication-analyst.pid`

When a probe message was sent to `communication-analyst`, the pane output showed the expected injected notification:

```text
[mesh] You are "communication-analyst" on team "taurhaus-team".
Message from team-lead.
Read: mesh read --unread --mark-read --team taurhaus-team --name communication-analyst
```

By contrast, `developer3` pane `%224` showed no corresponding injected `[mesh]` notification.

So the difference is not inbox append behavior. The difference is tmux notification delivery.

### 5. Manual daemon restart for `developer3` is unstable

I attempted the normal recovery path:

```bash
mesh --team taurhaus-team team-daemon restart developer3
```

Observed behavior:

- command reported success
- `~/.claude/teams/taurhaus-team/daemons/developer3.pid` appeared
- shortly after, the PID file pointed to a dead process

So the detached restart path for `developer3` is also unstable right now.

This explains why the issue can persist after ad-hoc repair attempts: the notifier process is not staying alive reliably.

## Additional Unexpected Finding

During investigation, `aitx ls --json` reported this current Codex session as pane `%224`, which team config/runtime also map to `developer3`.

That suggests a live pane identity/mapping corruption in the environment. I reported that to `team-lead` immediately because it is likely related to the broader resume bug.

This does not change the main conclusion for `#510`, but it strongly reinforces that the resume lifecycle needs a deeper follow-up audit.

## Conclusion

The messaging asymmetry is caused by a broken inbound notifier state for `developer3` after resume:

1. `team-lead` messages are appended to `developer3`'s inbox correctly.
2. `developer3` does not get the tmux wake-up notification.
3. The per-agent daemon was absent during the original failure window.
4. A later restart attempt produced a short-lived/stale daemon instead of a stable detached notifier.

So the root cause is:

- broken or missing `mesh daemon` lifecycle for `developer3` after pane resume

not:

- send-side protocol failure
- inbox file corruption
- recipient lookup failure

## Current Workaround

Temporary operator workaround:

- if `developer3` appears unresponsive but outbound messaging still works, force a manual inbox read in the pane:

```bash
mesh read --unread --mark-read --team taurhaus-team --name developer3
```

- if a notifier restart is attempted, verify that the resulting daemon PID is actually alive instead of trusting the restart command output alone

## Recommended Next Step

The right permanent fix is task `#511`:

- trace the resume lifecycle
- identify how pane identity and daemon attachment became corrupted
- add a regression test covering resumed non-Claude members so pane mapping and notifier daemon state stay consistent after resume
