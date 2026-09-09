# Claude Code — UserPromptSubmit (pin 2.1.263; original preflight 2.1.266)

2026-09-09. **NOT RUN / categorical Claude drain refusal; eligibility disabled.**

Mesh `src/delivery/hook/mod.rs:111` rejects every `harness == "claude"`
boundary as `unverified_boundary_identity_or_native_mailbox`. A descriptor
flip cannot lift this refusal. The pinned 2.1.263 binary is available locally;
version procurement is not the operative blocker. In round 1, a read-only
mount of that pinned binary returned `2.1.263 (Claude Code)` in scratch
(exit 0); no Claude model or credentials were used.

## Observed preflight (S-runtime)

The isolated installed executable returned `2.1.266 (Claude Code)` for
`/opt/claude --version` (exit 0). `/opt/claude` was a read-only mount of
`/home/mstie/.local/share/claude/versions/2.1.266`; no model was started.
The actual release-candidate Mesh binary returned the following pin via
`delivery capabilities` (exit 0):

```json
{
  "build": "2.1.263",
  "config_trust": "explicit managed opt-in and live root; intended-host trust/uptake UNVERIFIED",
  "context_entry": "intended event context uptake UNVERIFIED",
  "continuation_budget": 0,
  "disposition": "disabled pending 4b scoped uptake; Codex new hooks NO-GO-for-now",
  "drop_rules": "unknown execution/trust/drop behavior disables entry",
  "enabled": false,
  "envelope": "hookSpecificOutput.additionalContext; event-specific validation required",
  "event": "UserPromptSubmit",
  "harness": "claude",
  "host": "print/stream probe; TUI UNVERIFIED",
  "id": "claude/2.1.263/UserPromptSubmit/1",
  "matcher": "event only; permission deny/error/cancel excluded",
  "max_bytes": 8192,
  "max_chars": 8000,
  "receipts": "bridge_rendered; hook_response_offered after final flush; no native acceptance or consumption evidence",
  "source": "ordinary"
}
```

The brief requires an exact pinned build and says **a mismatch means disabled,
no trial**. The original controller selected 2.1.266 instead of the available
`/home/mstie/.local/share/claude/versions/2.1.263`. Selecting that pinned binary
would resolve the mismatch; it cannot resolve the categorical drain refusal.
No pin was broadened and no Claude credential was accessed.

## Dispositive software prerequisites (S-source, not runtime uptake)

The paired bridge can represent UserPromptSubmit additional context, but that
is source-level envelope support, not evidence of model uptake.

Both Claude boundaries also encounter
`src/delivery/hook/mod.rs::validate_identity` in Mesh at
`504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08`: it refuses
`request.runtime.harness == "claude"` with
`unverified_boundary_identity_or_native_mailbox`. The supplied contract preserves
native-mailbox exclusion until independent native-consumer exclusion is proved.
A descriptor flip alone cannot remove that restriction. These are dispositive
source findings, not claims that the installed harness rejected a trial.

## Acceptance and spend

- Candidate pair: Mesh `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08`;
  Taurhaus `cadd533ebd83d75d848e289e32974a2db64875c9` (#154), protocol 26.
- Scratch root: `/tmp/native-elig-20260909-kj3o83ui`.
  HOME, CLAUDE_CONFIG_DIR, TAURHAUS_CLAUDE_DIR, CODEX_HOME, data and tmux
  directories were private. No team or model session was created for this boundary.
- Planned marker: `ELIG_USERPROMPTSUBMIT_09869403B0`. **Never delivered**; no model output exists.
- Control on `claude-haiku-4-5-20251001`: **not run** because the paired software categorically refuses Claude drains; model actually used: none.
- Boundary turns, generations and tokens: **0**. Spend: **$0.00**.
- `hook_response_offered`: none. Explicit read: not performed.
  `consumed_by_read`: none. No receipt or uptake was inferred from capability JSON.
- Hook-mode selection / team delivery owner / scratch-enabled pin: **not set up**
  because the paired software refuses this boundary.
- Cleanup: version/help commands exited and were reaped; no Claude process,
  tmux session, hook registration, or credential mount was left by this boundary.
- Descriptor changes / Mesh commits: **none**.

## Exact commands and limits

The sandbox helper is preserved in [the framing evidence](codex-0.153.4-app-server.md#reproduction-controller).
The relevant invocations were:

```python
subprocess.run(command(['/opt/claude', '--version']),
               capture_output=True, text=True, timeout=15)
subprocess.run(command([
    '/tmp/native-elig-20260909-kj3o83ui/home/.local/bin/mesh',
    '--claude-dir', '/tmp/native-elig-20260909-kj3o83ui/claude',
    '--team', 'eligibility', '--name', 'seat', 'delivery', 'capabilities'
]), capture_output=True, text=True, timeout=10)
```

The scratch Mesh copy's SHA-256 is
`8389f0a7a3a5dbcb1d614dfa819a5de08507de58e9d4e949adcc2cf8b046d9cc`;
it is byte-identical to the supplied release-candidate binary. Copying an
executable into scratch did not enable a descriptor. Authentication and network
were unnecessary; both invocations ran with network disabled.

Mid-turn/tool/permission/error/cancellation/compaction/switch/teardown uptake
coverage is **excluded**, not passed. Reopening requires a matching narrowed
build/host/config/trust pin and the software prerequisites above; this packet
adds neither an eligibility bypass nor a new adapter.
