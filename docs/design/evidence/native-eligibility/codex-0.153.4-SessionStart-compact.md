# Codex 0.153.4 — SessionStart(compact) composed drain

2026-09-09. **INCONCLUSIVE / blocked at activation preflight. Disabled.**
No paid compaction trial ran; this file must not be counted as model-uptake evidence.

## Preflight (S-runtime)

`codex --version` returned `codex-cli 0.153.4` (exit 0) in the isolated sandbox.
The supplied Mesh binary's real `delivery capabilities` command (exit 0) returned:

```json
{
  "build": "0.153.4",
  "config_trust": "explicit managed opt-in and live root; intended-host trust/uptake UNVERIFIED",
  "context_entry": "intended event context uptake UNVERIFIED",
  "continuation_budget": 0,
  "disposition": "disabled pending 4b scoped uptake; Codex new hooks NO-GO-for-now",
  "drop_rules": "unknown execution/trust/drop behavior disables entry",
  "enabled": false,
  "envelope": "hookSpecificOutput.additionalContext; event-specific validation required",
  "event": "SessionStart",
  "harness": "codex",
  "host": "app-server probe; TUI UNVERIFIED",
  "id": "codex/0.153.4/SessionStart/compact/1",
  "matcher": "compact; explicit composition after recovery owner decision only",
  "max_bytes": 8192,
  "max_chars": 8000,
  "receipts": "bridge_rendered; hook_response_offered after final flush; no native acceptance or consumption evidence",
  "source": "compact"
}
```

The build matches, but the pin is disabled and the advertised TUI host remains
unverified. The capability command supplies no scratch-root enablement. Its
result is generated from compiled descriptors independently of root arguments.

## Blocking seam (S-source)

At Mesh `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08`:

1. `src/delivery/hook/mod.rs::dispatch` returns
   `capabilities::descriptors()` for discovery without consulting a root policy.
2. `src/delivery/hook/rpc.rs::handle` independently resolves the requested ID
   from that same compiled table inside the real owner.
3. `src/delivery/hook/mod.rs::drain` returns
   `unsupported / disabled_or_unverified_event_pin` when `!pin.enabled`.

At Taurhaus `cadd533ebd83d75d848e289e32974a2db64875c9` (#154), protocol 26,
`src-tauri/src/coordination/compact_hook/drain.rs::descriptors` filters the real
capability response through `Descriptor::supported()`, which requires `enabled`.
`append` therefore cannot select the compaction pin in this binary pair.

This is an activation-preflight blocker, **not** observed failure of Codex's
existing compaction/recovery hook. Editing scratch `hooks.json`, changing a
member to hook mode, or decorating the discovery response cannot enable the
real owner's compiled pin. A wrapper that substitutes an offered payload or
receipt would not exercise the commissioned real bridge. Enabling the release
candidate globally before uptake would violate the evidence-first condition.
No such changes were made, and no new scratch activation mechanism was built
under this descriptor/evidence-only brief.

## Acceptance and spend

| Signal | Result |
|---|---|
| Pinned CLI build | Match: 0.153.4 |
| Usable scratch-enabled compiled compact pin | Unavailable on supplied candidate |
| Live canonical scratch team / hook-mode member | Not created for a blocked paid trial |
| Session ID / compact generation | None |
| Planned marker | `ELIG_COMPACT_9AF7C229EC` (never delivered) |
| Model output containing marker | None; model never invoked |
| Intact recovery card after actual compaction | Not exercised; no claim of pass or regression |
| Real bridge offer / receipt / explicit read | None |
| Model / protocol turns / generations / tokens | None / 0 / 0 / 0 |
| Cost, combined with app-server on this build | **$0.00**, 0 of 5 model turns |
| auth.json copying | Not needed; scratch CODEX_HOME stayed credential-free |
| Descriptor flips / Mesh commits | None |

Only SessionStart(compact) was considered. Startup and ordinary Codex hooks were
not substituted; agy and Grok were not run. Host lifecycle, permission/tool/error,
compaction uptake and switch coverage are unproved. The Unix framing blocker
belongs to the app-server trial; it does not itself prohibit a future TUI hook
trial once its independent activation prerequisite exists.

## Commands and cleanup

The exact sandbox helper and version command are retained in
[the framing evidence](codex-0.153.4-app-server.md#reproduction-controller).
Capability invocation:

```text
/tmp/native-elig-20260909-kj3o83ui/home/.local/bin/mesh --claude-dir /tmp/native-elig-20260909-kj3o83ui/claude --team eligibility --name seat delivery capabilities
```

This ran in the credential-free, network-disabled sandbox and exited 0. Its copy
is byte-identical to `/home/mstie/projects/mesh-push/target/debug/mesh` (SHA-256
`8389f0a7a3a5dbcb1d614dfa819a5de08507de58e9d4e949adcc2cf8b046d9cc`).
No persistent process was started for this preflight. The packet's separate
scratch-daemon build/smoke and gates are recorded in [RESULT](README.md); they
are software verification, not substitute model evidence.
