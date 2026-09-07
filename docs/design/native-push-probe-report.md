# Native push probe: installed Claude Code and Codex

2026-09-07. Commissioned probe lane; fulfills [the probe brief](native-push-probe-brief.md). Research and isolated probes only; no implementation or commit. Read alongside [the messaging overhaul study](mesh-messaging-overhaul-research.md), particularly its claim/present/record and receipt contracts.

**Decision: Codex app-server input is verified for a newly owned scratch thread. Claude channels are NO-GO-for-now for the tested print/stream hosting path: the notification was written, but model uptake was not observed. Codex accepts twelve hook events; that does not establish their execution or context-injection behavior.** These results authorize no change to the live wave.

## Evidence labels and scope

- **S-runtime**: observed here, with the exact installed version. A negative observation establishes only the stated experiment and bound.
- **S-source**: inspected installed code, CLI help, or an installed-binary-generated schema. This is the study's source-verification meaning of S; it is not a successful runtime test.
- **D**: opened primary vendor documentation, without installed-version verification of that particular behavior.
- **U**: unresolved or proposed. Adapter suggestions below are explicitly U/proposals, not implemented results.

**NO-GO-for-now is a disposition, not another evidence grade.** It means the available evidence is insufficient to enable that delivery mode. In particular, absence of uptake is not proof that the installed TUI lacks channels or that an organization denied eligibility.

| Question | Result for this machine | Disposition |
|---|---|---|
| Claude channel flags and protocol | **S-source, 2.1.263:** both flags, capability, notification parser, registration code, and eligibility gates are present. **S-runtime:** hidden `--channels` flag accepted alongside help; scratch MCP initialized. | Surface exists. |
| Claude notification reaches the model | **S-runtime, 2.1.263:** control response succeeded; one notification was written after it; no second response within 25 seconds. **U:** why this print-mode session did not process it; exact account eligibility. | **NO-GO-for-now** for tested hosting path. No claim of general TUI failure. |
| Codex app-server | **S-runtime, 0.153.4:** stdio handshake, scratch thread, `turn/start`, active `turn/steer`, rejection conditions, and distinctive-marker response all verified. | Verified native input for a server-owned thread. |
| Codex hook event configuration | **S-runtime, 0.153.4:** twelve event names recognized in both `hooks.json` and inline TOML; hook feature reports enabled. | Configuration contract verified. |
| Codex additional hook execution/context | **S-runtime:** attempted startup hook left no execution marker, and the model reported `NO_CODE`. Explicit startup-source follow-up without inference also left no marker. **U:** cause and context-injection semantics in this app-server setup. | **NO-GO-for-now** for a new hook delivery adapter. Existing compaction integration was not exercised or invalidated. |
| Retrofitting live sessions / retiring tmux | **U:** attachment, competing controllers, restart, permissions, resume, compaction, and simultaneous human input were not tested. | No live rollout; agy/grok retain fallback. |

## Isolation, execution budget, and artifacts

**S-runtime:** the probe used private scratch root `/tmp/native-push-probe-20260907-7qdrvkcu`, with separate `project`, `claude`, `codex`, `tmp`, and `evidence` directories. Harness subprocesses ran through Bubblewrap: read-only host filesystem, hidden `/home`, `/tmp`, and `/run`, a private PID namespace, and a writable bind of only the scratch root. The installed Claude binary and Node/Codex installation were exposed read-only. Capability/config-only Codex calls used a private network namespace. Authenticated harness calls used normal network access; no probe targeted a daemon or local service endpoint.

**S-runtime / execution audit:** no Mesh command, daemon command, tmux command, team-state read, existing-session resume, repository build, or test-suite command was issued. Neither port 17233 nor the operator's tmux server was inspected. The prohibited account root, team directories, taurjob project, and other live state were hidden from probe children and were not inspected by the controller. No environment dump was taken; children received a generated allowlist of environment values, without inherited team/session identity. `HOME` retained its original value; harness-specific state roots pointed into scratch.

**S-runtime:** the [paid-lane helper](../../e2e/helpers/codexScratchHome.js) was inspected but not called: it copies `auth.json`, conflicting with this brief's stricter credential rule. Instead, Bubblewrap exposed the default credential file directly at the scratch harness credential path through a **read-only mount**. Only the harness opened its contents. The probe did not read, print, copy, or supply tokens to an API client. No default settings, hooks, histories, or sessions were copied. After child shutdown, both scratch credential mount placeholders were verified to be zero-byte files. Credential source files were checked only for regular-file/non-symlink identity before mounting. All persistent child writes were confined to scratch; the sole repository write is this report.

**S-runtime, budgets:**

| Harness | Paid/model work | Other work |
|---|---|---|
| Claude 2.1.263 | One short control turn using `claude-haiku-4-5-20251001`; one channel payload produced no observed turn. CLI reported cost `$0.013338`, 124 output tokens including thinking. | Version/help, installed-code inspection, no-input print diagnostic, and no-input TUI startup. These diagnostics submitted zero prompts and zero additional channel payloads. |
| Codex 0.153.4 | One `turn/start` plus one accepted steer in that same turn. Two short model responses; one `turn/started` and one `turn/completed`. | Handshakes, model listing, config/schema checks, and a second ephemeral thread start without a model turn for a free hook check. |

**S-runtime:** Codex reported 17,455 total input tokens, 13,824 cached input tokens, and 231 output tokens, including 200 reasoning tokens, across its two response generations. Dollar cost was not reported. The 10,967 ms turn duration is one observation, not a latency guarantee. All spawned probe processes were terminated/waited by their controllers; no background probe server was left running.

**S-source:** repository HEAD at report preparation was `231bb74b92c31472615f90ae6becf822cd91a5f1`. This identifies the inspected checkout, not a frozen live-wave snapshot. Unrelated concurrent repository work was left alone.

## 1. Claude Code channels

**S-runtime:** `claude --version` returned `2.1.263 (Claude Code)`. Normal `--help` did not list either channel flag. `claude --channels server:probe --help` nevertheless exited successfully. This alone would be weak evidence because help can short-circuit startup; installed source and the actual MCP connection provide the additional evidence below.

**S-source, 2.1.263:** the installed executable is `/home/mstie/.local/share/claude/versions/2.1.263`, SHA-256 `26d020351e8112f4006790f3cfce43b4c9df0c1bb1d0e542364d64151b81d5ba`. Embedded JavaScript contains:

- `--channels <servers...>` and `--dangerously-load-development-channels <servers...>`.
- Capability checking for `experimental['claude/channel']` and a `notifications/claude/channel` schema requiring `content`, with optional string-valued `meta`.
- Registration handlers that wrap content in a channel tag and enqueue it as a prompt with `priority: "next"`, channel origin, and slash-command/attachment processing disabled.
- Checks for provider, feature availability, organization policy, session opt-in, plugin provenance/allowlist, and protocol compatibility.

**S-source:** exact installed gate reasons include `channels feature is not currently available` and `channels not enabled by org policy (set channelsEnabled: true in managed settings)`. These are **code branches, not observed denials for this account**. The source also rejects a connection whose protocol era has no supported notification path. No feature flag, account eligibility value, organization setting, or allowlist was edited.

**D:** current [channel documentation](https://code.claude.com/docs/en/channels) describes a research preview, first-party authentication, per-session opt-in, and organization-dependent enablement. The [channel reference](https://code.claude.com/docs/en/channels-reference) permits a development server entry through the development flag while keeping organization policy effective. It also states that notification transport completion has no Claude processing acknowledgment; events can be dropped when the channel is not enabled. These docs do not establish this account's eligibility.

**S-runtime, the one end-to-end trial:** a minimal local Python stdio MCP server answered `initialize`, `tools/list`, and `ping`. Its initialize response advertised protocol `2024-11-05`, no tools, and:

```json
{
  "capabilities": {
    "tools": {},
    "experimental": { "claude/channel": {} }
  },
  "serverInfo": { "name": "native-push-probe", "version": "1.0.0" }
}
```

**S-runtime:** the scratch MCP config registered `server:probe` with `/usr/bin/python3` and the scratch server script. Claude ran with `-p --input-format stream-json --output-format stream-json --verbose`, `--model haiku --effort low`, empty built-in tools and setting sources, `--strict-mcp-config`, the scratch MCP config, `--dangerously-load-development-channels server:probe`, `--max-turns 2`, `--max-budget-usd 0.15`, and `--no-session-persistence`. The process had a 70-second outer bound. The development flag was used only for this locally authored server, as specified by the vendor development workflow; it was not used to defeat an eligibility rejection.

**S-runtime:** stdin supplied only `Reply with exactly READY. Do not use tools.` Session `74163b8d-845b-42c6-87d6-cec1b9e9ac2d` returned `READY` and a successful one-turn result. Only then did the controller signal the server to write its single event:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel",
  "params": {
    "content": "Probe notice: the orchard color is saffron. Reply with exactly CHANNEL_SAFFRON_7QDRVKCU.",
    "meta": { "probe_id": "7qdrvkcu", "kind": "benign_probe" }
  }
}
```

**S-runtime:** the server log proves initialization, initialized notification, tool listing, and that event write. The marker was absent from the initial user prompt. Stdin remained open, but no second model response appeared within the subsequent 25 seconds. The controller then terminated its own child. This establishes **`submitted`**, not `native_enqueued`, `consumed_by_read`, or comprehension.

**S-runtime:** a subsequent no-input print diagnostic connected the MCP server but produced no channel-registration or gate-reason log line. A no-input TUI diagnostic stopped at the fresh-profile theme chooser; no keys were sent. Neither diagnostic emitted the payload or requested inference. **U:** the exact cause of non-delivery, including print-mode registration behavior, scratch-profile eligibility initialization, and account policy. No organization/preview denial was actually observed, so it would be incorrect to invent one. The TUI channel path remains untested. A successful MCP connection does not settle any of these gaps.

**Disposition:** **NO-GO-for-now** for replacing messaging with this tested Claude print/stream channel path. The installed surface is verified; end-to-end push is not. No second paid/channel trial was attempted under the one-trial constraint.

## 2. Codex app-server

**S-runtime:** `codex --version` returned `codex-cli 0.153.4`. Help lists `app-server`, and its help lists `stdio://`, Unix-socket and WebSocket transports. This probe launched a new `codex app-server --listen stdio://` child; it did not use `daemon`, `proxy`, `agents`, or a pre-existing control socket. **S-source:** the installed binary generated its own JSON schema with `codex app-server generate-json-schema --out <scratch>/schema`.

**S-runtime:** initialization used newline-delimited messages without a `jsonrpc` field:

```json
{"id":1,"method":"initialize","params":{"clientInfo":{"name":"native_push_probe","version":"1.0.0"},"capabilities":{"experimentalApi":true}}}
{"method":"initialized"}
```

The response reported `native_push_probe/0.153.4`, Linux, and the scratch `codexHome`. **D:** this handshake and the start/steer control model agree with the opened [official app-server reference](https://learn.chatgpt.com/docs/app-server).

**S-runtime:** `model/list` returned Astra, Sol, Terra, Luna, GPT-5.5, GPT-5.4 Mini, and Codex Spark, with no next page. Luna was described as fast and affordable; Mini carried a retirement timestamp already past at probe time and an upgrade to Luna. The trial selected **`gpt-5.6-luna`, low effort, default service tier**, the current small coding lane in that response. **U:** the endpoint does not expose parameter counts or a complete price ordering, so this is not a measured assertion that Luna has the fewest parameters or lowest price of every listed model. No premium/default Astra turn was used.

**S-runtime:** `thread/start` created ephemeral thread `01a07d69-888e-74b2-b127-cfaefe7dad50`, with scratch cwd, read-only sandbox, approval policy `never`, short base instructions forbidding tools/delegation, and no repository instruction sources. It reported idle state and `canAcceptDirectInput: true`. A startup-hook context check shared this turn; its negative result is detailed below.

**S-runtime, requests and outcomes:**

| Operation | Observed response |
|---|---|
| `turn/steer` on the idle scratch thread | Error `-32600`, `no active turn to steer`. |
| `turn/start`: `Reply START_OK and the startup probe code. If a follow-up probe arrives, include its marker too.` | Accepted turn `01a07d69-88dd-79f0-a4c5-c01fd966408b`, status `inProgress`; one `turn/started` notification. |
| Active steer with `expectedTurnId: "probe-wrong-turn-id"` | Error `-32600`, identifying the expected and actual IDs. |
| Active steer with the correct ID and the payload below | Successful response containing that same `turnId`. |
| Steer after `turn/completed`, retaining the old correct ID | Error `-32600`, `no active turn to steer`. |

```json
{
  "id": 6,
  "method": "turn/steer",
  "params": {
    "threadId": "01a07d69-888e-74b2-b127-cfaefe7dad50",
    "expectedTurnId": "01a07d69-88dd-79f0-a4c5-c01fd966408b",
    "input": [{
      "type": "text",
      "text": "Follow-up probe: include STEER_SAFFRON_7QDRVKCU, START_OK, and the startup probe code in your final reply."
    }]
  }
}
```

**S-runtime, acceptance versus comprehension:** steer acceptance arrived approximately 2 ms after submission. The first model answer was `START_OK NO_CODE`. The accepted steer then appeared as a user-message item under the **same thread and turn**, followed by model output `STEER_SAFFRON_7QDRVKCU START_OK NO_CODE`. The completion notification retained that final answer and reported success. The distinctive steer marker was not in the initial prompt, so its later appearance establishes basic payload comprehension by the correct model context, independently of the RPC acknowledgment. It does not establish task acceptance or work completion.

**S-runtime:** the accepted steer did not emit a second `turn/started`; it did cause another model response within the existing turn. An adapter must therefore not equate one protocol turn with one inference or assume steering is free. **U:** a steer arriving after generation is already streaming may have different timing; this steer was submitted immediately after `turn/start` acceptance. Disconnection ambiguity, idempotent retries, other transports, persistent-thread recovery, TUI attachment, and concurrent human submission were not tested.

## 3. Codex hooks beyond compaction

**S-source / S-runtime, 0.153.4:** the generated `HooksListResponse` schema enumerates twelve event names. For each event below, the probe placed a harmless `/usr/bin/true` command handler in scratch `hooks.json`, called `hooks/list`, then repeated using inline `[[hooks.EventName]]` / `[[hooks.EventName.hooks]]` TOML. Both calls returned all twelve handlers, correct source paths, `enabled: true`, no errors, and no warnings.

| Configuration event | App-server event name | Verified scope |
|---|---|---|
| `PreToolUse` | `preToolUse` | **S-runtime:** JSON and TOML recognition |
| `PermissionRequest` | `permissionRequest` | **S-runtime:** JSON and TOML recognition |
| `PostToolUse` | `postToolUse` | **S-runtime:** JSON and TOML recognition |
| `PreCompact` | `preCompact` | **S-runtime:** JSON and TOML recognition |
| `PostCompact` | `postCompact` | **S-runtime:** JSON and TOML recognition |
| `SessionStart` | `sessionStart` | **S-runtime:** JSON and TOML recognition |
| `SessionEnd` | `sessionEnd` | **S-runtime:** JSON and TOML recognition |
| `UserPromptSubmit` | `userPromptSubmit` | **S-runtime:** JSON and TOML recognition |
| `SubagentStart` | `subagentStart` | **S-runtime:** JSON and TOML recognition |
| `SubagentStop` | `subagentStop` | **S-runtime:** JSON and TOML recognition |
| `Stop` | `stop` | **S-runtime:** JSON and TOML recognition |
| `Interrupt` | `interrupt` | **S-runtime:** JSON and TOML recognition |

**S-runtime:** fresh handlers were reported `trustStatus: "untrusted"`, despite `enabled: true`. Default reported timeouts were 600 seconds except `SessionEnd` and `Interrupt`, which reported one second. `codex features list` reported `hooks stable true`; `plugin_hooks` was a removed flag. A negative control containing only `ProbeUnsupportedEvent` returned **zero hooks and zero errors/warnings**. Therefore successful configuration loading alone does not establish support: inspect returned event entries. That single negative control is not an exhaustive test of all unknown names.

**D:** [official hooks documentation](https://learn.chatgpt.com/docs/hooks) describes hook trust, context output at supported boundaries, Stop continuation, and the distinction between recognized handler configuration and supported execution behavior. It does not convert this probe's parser checks into execution evidence. **U:** context payloads, blocking behavior, matchers, async behavior, and invocation conditions for the eleven non-startup events were not exercised here.

**S-runtime, free boundary check:** the paid app-server trial also configured `SessionStart` matching `startup`, with a vetted local command and the invocation flag `--dangerously-bypass-hook-trust`. The command would log the hook event/source/session and return `hookSpecificOutput.additionalContext` containing `HOOK_CEDAR_7QDRVKCU`. No marker file or hook notification appeared; both model replies instead contained `NO_CODE`. A subsequent no-inference thread start explicitly set `sessionStartSource: "startup"`, using the same command and flag, and again produced no execution marker. Its preceding `hooks/list` still reported the command as untrusted. **U:** whether the bypass flag is propagated into this hosting path, whether trust is the blocker, and when this host executes SessionStart. These observations do not prove a Codex hook regression. No extra model turn was purchased to investigate.

## 4. Minimal delivery-service adapter implications

**U/proposal, grounded in verified Codex 0.153.4 behavior:** claim a batch against the delivery ID, member/session attachment generation, owned app-server thread ID, and fencing epoch. Present an idle batch with `turn/start`; present to a known active turn with `turn/steer` and its required `expectedTurnId`. Record the RPC success as `native_enqueued` with thread/turn evidence, never as `consumed_by_read` or task uptake. Keep subsequent model-marker or lifecycle evidence separately. A mismatch requires state reconciliation; a timeout after write remains `outcome_unknown`, with no immediate blind tmux resend. This adapter requires launcher-owned hosting; the experiment did not attach it to an existing TUI.

**U/proposal, conditional Claude adapter:** claim against the opted-in session and connected server generation; present one bounded, attributed channel notification with stable delivery metadata. The strongest ordinary transport receipt is `submitted`, because MCP notification writes provide no model-processing acknowledgment. A future explicit reply/ack tool could provide additional evidence with the same delivery ID. This probe does not justify enabling that adapter, and no eligibility bypass should be part of its design.

**U/proposal, conditional hook adapter:** the twelve verified config entries permit a version-specific capability map, not twelve verified input channels. At a separately verified boundary, a trusted bridge would claim a bounded batch, present `additionalContext` through that event's supported output, and record only `hook_response_offered` absent stronger evidence. Startup/compaction and tool/prompt boundaries do not establish autonomous idle wake. Preserve explicit fallback behavior until execution and uptake are verified in the intended host.

## Evidence and review

**S-source / S-runtime:** runnable probe controllers and sanitized evidence remain under the private scratch root, outside the repository. These are reproducibility artifacts, not installed adapters. The key controllers are [isolation wrapper](/tmp/native-push-probe-20260907-7qdrvkcu/probe_env.py), [Claude trial](/tmp/native-push-probe-20260907-7qdrvkcu/claude_trial.py), [local MCP server](/tmp/native-push-probe-20260907-7qdrvkcu/channel_server.py), [Codex RPC driver](/tmp/native-push-probe-20260907-7qdrvkcu/codex_rpc.py), and [Codex turn trial](/tmp/native-push-probe-20260907-7qdrvkcu/codex_trial.py). Do not rerun them as part of the live wave merely because their paths are recorded here.

| Scratch evidence under `evidence/` | SHA-256 |
|---|---|
| `claude-trial.jsonl` | `84d5b0bb1fbc93066b5b439d0fc5dbd45d14b5ac3c80668ca270b2156acaa652` |
| `claude-mcp.jsonl` | `a12952008118f85c34b378e53d8eed745a7dd0aca3d798aad6e21608130dac25` |
| `codex-trial.jsonl` | `3b4dc39e0cd2b92a0b92b81adb6c488ad4d224140d950cd741f2496ad545441a` |
| `codex-hooks-valid.jsonl` | `803ac170fa7678a302028e501b02f986f42d62d0a55393b0be86a791a854a47f` |
| `codex-hooks-inline.jsonl` | `92a9b49ecfc5efea26807197a3031dda6daeaa84415f0eaf9e9497615aa845e3` |
| `codex-hooks-invalid.jsonl` | `c7914a470daaecd27654759774c612075e81023e128cc621c2e3bb0799891bf0` |
| `codex-hook-startup-explicit.jsonl` | `d4ced5e31d8c68df3c340d49ae16277352d82530a5cd85a03c1979532c44cde0` |

**S-source:** Codex native binary SHA-256 is `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da`, from the installed `@openai/codex-linux-x64/vendor/x86_64-unknown-linux-musl/bin/codex` package beneath the CLI installation. Its generated schema and the capability/help captures remain in scratch. Claude source gate excerpts include byte offsets in `evidence/claude-source-excerpts.json`. The MCP log includes additional handshakes from the later no-payload diagnostics; it contains exactly one channel notification write.

**S-runtime / review:** validation consists of the isolated protocol/config probes, exact thread/turn and marker joins, execution-budget audit, evidence hashes, report link checks, and a scoped whitespace/diff check. No `just check-quick` or full gate was run: this task changes only documentation, and formatting/building unrelated implementation would add no probe evidence. No commit was created.
