# Codex 0.153.4 — SessionStart(compact) composed drain

2026-09-09. **INCONCLUSIVE, configuration-confounded; no valid uptake test;
disabled.** The only run that compacted had `trustStatus: "untrusted"` and
invalid bridge identity. Its `NO_MARKER` is a configuration artifact, not evidence
against boundary uptake. The corrected TUI continuation never compacted and is
off-pin. No run combined a trusted handler, compaction and valid bridge identity;
recovery-card/offer composition remains untested. This is the round-2 correction.

## Corrected activation finding

The original `9ff59169` packet stopped on a removable prerequisite. Mesh
`src/delivery/hook/capabilities.rs` builds the compact descriptor by cloning a
SessionStart descriptor with `enabled: false`. Discovery, Taurhaus
`Descriptor::supported()` and the Mesh owner all consult that **same flag**;
these are not independent refusals. There is no environment/config override.
A **scratch-only rebuild** was available and is now exercised. The earlier
claim that enabling a local binary necessarily enabled the release candidate
globally was wrong. No orchestrator policy exception was needed.

At `504b2b6`, the scratch source archive adds exactly this one line in the compact
mapping, after `entry.source = "compact"`:

```rust
entry.enabled = entry.harness == "codex";
```

This enables only `codex/0.153.4/SessionStart/compact/1`, not ordinary SessionStart
or any Claude entry. `just build-release` exited **0** in 49.04 seconds after the
Cargo preflight. The actual capability response was checked: baseline red
(expected that single enabled ID, got `[]`, exit 1), scratch green (exactly that
ID, exit 0). This checks preparation only, not model uptake.

The source was `git archive 504b2b6` from `/home/mstie/projects/mesh-push`, extracted
under `/tmp/elig-r1-v2r9vhiy/mesh-src` with its own `target/`. No extra Git checkout
was created. Only scratch `HOME/.local/bin/mesh` received the rebuilt executable,
which is the path Taurhaus's real bridge resolves. Its SHA-256 is
`8a8bc3bb6dbc42b6d156bd76caa15e0c138f80f027a9032417bdfc934b162f9b`.
The Mesh worktree and supplied RC binary remain unchanged.

## Runtime setup and limits

Pair: Mesh source `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08` plus the scratch flag;
Taurhaus `cadd533ebd83d75d848e289e32974a2db64875c9`, protocol 26. The daemon is the
binary built in this checkout, SHA-256
`0cfbed50bc31b3ca3e5c6c28ba8030fe86c3dcad022175771943f53f2547fc14`.
Its private authenticated ping returned protocol 26 and the exact scratch data
root on port **22077** (a zero-turn setup attempt used **25817**). Neither used 17233.

Bubblewrap hid operator homes, cleared the environment including TMUX, and
exposed only scratch writable roots plus read-only host binaries. Separate HOME,
CODEX_HOME, CLAUDE_CONFIG_DIR, TAURHAUS_CLAUDE_DIR, TAURHAUS_DATA_DIR and
TMUX_TMPDIR were under `/tmp/elig-r1-v2r9vhiy`. `createCodexScratchHome` from
`e2e/helpers/codexScratchHome.js` copied **auth.json only** and generated minimal
config; it was called before each isolated launch and the copy removed in every
controller's finally. No credential was printed or written back. Claude, agy
and Grok made no model calls.

A real `mesh team create --messaging-canonical --isolated --retention-policy
<scratch>/policy.json` created team `eligibility`, member `seat`, delivery_owner
`team`, incarnation `bf886331ff8075b86f24e021a7f56d76061d72b8b5ad494d91e95a328580c8b4`.
A foreground, owned `mesh team-daemon start` served the real hook socket.
`mesh team adapter --member seat --mode hook --fallback defer` recorded revision
1 / owner fence 1, then revision 2 / fence 2 for the TUI continuation.
All Mesh commands carried explicit scratch root/team/name flags.

The hook command was the **unmodified** `<scratch>/taurhaus-daemon --compact-hook`,
registered in scratch `hooks.json` under SessionStart matcher `compact`, timeout
10 seconds. No wrapper substituted payloads, cards, stdout or receipts. The
operational snapshot's task was `Recovery control CARD_CEDAR_R1`, in_progress;
this subject and both inbox markers were absent from the model's user prompts.
Snapshot/runtime fixture records were authored in scratch from observed thread
and pane facts; this is not daemon-launch/publication coverage.

## Observed sequence (S-runtime)

The Unix eligibility trial remains stopped at its framing failure. For this
**separate hook trial**, a real `codex --dangerously-bypass-hook-trust app-server
--listen stdio://` ran in a private tmux pane via a FIFO. The stdio transport is
the prior probe's known framing, not a repair or substitute pass for Unix hosting.
Initialization reported `eligibility_compact/0.153.4` and the scratch codexHome.
The thread used `gpt-5.6-luna`, low effort, never approvals, read-only sandbox,
no tools/delegation instructions, and `model_auto_compact_token_limit=2048`,
`model_context_window=16384`. Built-in context was larger than the short prompt.

1. Control: `Reply with exactly READY. Do not use tools.` → **READY**.
2. Mesh committed `ACTION: The orchard code is ELIG_COMPACT_22BAF2AA3A. Include
   that code in your next short reply.` (message
   `42409be0-2776-4fcd-a9b0-308e20170c5a`, delivery
   `1ffc2fc0-ee59-496d-ae07-b5baa6a9d1bc`). The next prompt asked for any hook code
   and recovery subject, otherwise NO_MARKER; its inert filler was
   `' '.join('cedar'+str(i) for i in range(1800))`.
   Codex compacted and then replied **NO_MARKER**. `hooks/list` recognized the
   enabled handler but reported `trustStatus: "untrusted"` despite the invocation's
   bypass flag, as the earlier probe did. No hook-start/completed notification
   or Taurhaus hook event was observed. The exact compaction event follows.
3. The same persistent thread resumed in a private TUI, with the same model,
   low effort and explicit hook-trust bypass. A new committed inbox marker was
   `ELIG_COMPACT_592307555C` (message `2b463bbe-59ec-4576-be93-2fcec2a1e15b`,
   delivery `ef10a6f1-1e1e-4912-8cb1-9635f62129ac`). The single prompt requested
   any compact-hook code and task subject, or NO_MARKER. It replied **NO_MARKER**;
   the rollout delta had **no compaction boundary** and no bridge event.

```json
{
  "method": "item/completed",
  "params": {
    "item": {
      "type": "contextCompaction",
      "id": "01a0873a-6e13-7fa3-badb-cb97724d820f"
    },
    "threadId": "01a0873a-6016-7e72-b1e6-92a778353552",
    "turnId": "01a0873a-6e06-73a3-b3f4-6a5c293c3db4",
    "completedAtMs": 1788975151024
  },
  "emittedAtMs": 1788975151026
}
```

The stdio fixture initially used `sessionId`; the later TUI fixture used
`session_id` (and snake_case cli/project fields). The running daemon rewrote the
stdio fixture to a dead/missing-session record, leaving downstream bridge identity
invalid. The previous attribution to spelling alone is not established:
`MemberRuntimeStore::load` uses `parse_runtime_record`, whose `RuntimeRecordWire`
accepts `sessionId`, `cliTool` and `projectPath` aliases at this tip. The invalid
runtime and untrusted handler remain separate confounds; neither observation
isolates the cause of absent hook execution. The TUI continuation changed the spellings,
bumped attachment generation to 2 with observed pane PID/start ticks, and ran
without a daemon scheduler overwriting this explicit fixture. The standalone
native bridge requires no daemon port. Its runtime stayed active with the exact
session and hookSessionId. TUI did not reach the compact boundary, so no pass is
claimed there either. These limitations preclude claiming a Codex product regression.

## Eligibility refusal and re-commission decision

**Review option (b): this packet could not establish hook trust under the pinned app-server host.**
The observed bypass invocation still returned `trustStatus: "untrusted"`; it did
not establish trusted execution. This is an unmet trial prerequisite, not proof
that trust is impossible or that SessionStart(compact) cannot deliver. The
`NO_MARKER` null cannot prove or refute model-visible uptake. Mesh's exact host
in `src/delivery/hook/capabilities.rs` is `app-server probe; TUI UNVERIFIED`;
even a future TUI-only pass would not enable that pin.

Rechecking surviving `evidence/trial-result.json` confirms the enabled command
`/tmp/elig-r1-v2r9vhiy/taurhaus-daemon --compact-hook` was untrusted;
`evidence/tui-rollout.jsonl` contains no compaction record. The pinned probe
report's section 3 already leaves bypass propagation, trust and invocation
conditions unresolved. This correction performs no new trust experiment.

The review identifies free setup affordances: `/hooks` (hook management), a
persisted hook-trust store and `/compact` (explicit compaction). These remain
unexercised here; slash-command availability does not establish trusted execution
under app-server hosting, and triggering compaction may spend model budget.
A re-commissioned trial should first verify trusted status inside scratch
CODEX_HOME on the pinned host, validate the live `session_id`/hookSessionId and
attachment tuple, then budget one actual compaction and check marker/card/receipts.
No production trust-store edits or host broadening are proposed.

The budget is **not exhausted**: 3/5 user turns and four conservatively accounted
generation slots, estimated $0.166959 of $2. Unreported compaction usage remains
an accounting limit, not a claim that the remaining authorization disappeared.
The orchestrator retains the re-commission decision. This documentation correction
adds **0 additional turns / $0.00** and leaves the descriptor disabled.

Explicit real Mesh `read --json --last 16`, then `read --json --mark-read --last 16`
ran after each trial. Final read-back showed both messages `read: true`; every
page had `outcomes: []`. No drain offer exists, so there is **no**
`hook_response_offered → consumed_by_read` offer chain to claim. These explicit
reads prove the messages remained readable, not model receipt. Recovery card
integrity after actual hook composition remains unproved.

## Durable turn and spend record

Three user/protocol turns produced three visible generations; one additional
compaction operation is conservatively counted as a fourth generation against
the five-turn budget. No fifth turn was requested: another turn could also
compact, consuming two more generations. All user turns used gpt-5.6-luna/low.
The original framing and Claude checks remain zero-turn, zero-spend.

The CLI does not report billed USD. Ordinary generation token counters support
**$0.00531472 API-equivalent** at the current
[Luna rates](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
($0.20 input, $0.02 cached input, $1.20 output per million). Compaction returned
zero input/output deltas while resetting context to 231 tokens; **that is not
proof it was free**. Its billed usage and backend generation count are unknown.
A conservative token-based estimate reserves 16,384 input at $0.25/M (including
cache-write uplift) and the documented maximum 128,000 output at $1.20/M for
that one operation: $0.157696. Pricing all 36,572 ordinary input tokens at the
higher $0.25/M and 100 output tokens at $1.20/M gives a combined conservative
estimate **$0.166959 (< $0.17, below $2)**. This is a budgeting estimate under the
configured model's rates, not a vendor billing receipt or proof of compact pricing.
No exact total dollar charge is fabricated; unreported compaction spend is a
remaining accounting limitation.

<!-- compact-runtime-record -->
```json
{
  "enabled_scratch_pins": [
    "codex/0.153.4/SessionStart/compact/1"
  ],
  "user_turns": 3,
  "observed_compactions": 1,
  "accounted_generations": 4,
  "model": "gpt-5.6-luna",
  "reported_input_tokens": 36572,
  "reported_cached_input_tokens": 11776,
  "reported_output_tokens": 100,
  "reported_reasoning_output_tokens": 77,
  "metered_api_equivalent_usd": 0.00531472,
  "compaction_tokens_reported": false,
  "billed_usd_reported": false,
  "conservative_usd_estimate": 0.166959,
  "descriptor_flips": [],
  "turns": [
    {
      "thread_id": "01a0873a-6016-7e72-b1e6-92a778353552",
      "turn_id": "01a0873a-60c5-72d2-864c-03f2aa051dfe",
      "marker": "ELIG_COMPACT_22BAF2AA3A",
      "output": "READY"
    },
    {
      "thread_id": "01a0873a-6016-7e72-b1e6-92a778353552",
      "turn_id": "01a0873a-6e06-73a3-b3f4-6a5c293c3db4",
      "marker": "ELIG_COMPACT_22BAF2AA3A",
      "output": "NO_MARKER"
    },
    {
      "thread_id": "01a0873a-6016-7e72-b1e6-92a778353552",
      "turn_id": "01a0873d-d041-7fd3-a2aa-775f2045aac7",
      "marker": "ELIG_COMPACT_592307555C",
      "output": "NO_MARKER"
    }
  ],
  "hook_invocations_observed": 0,
  "hook_response_offered": 0,
  "auth_removed": true
}
```

## Outcome, cleanup and exclusions

**No descriptor flip, no Mesh commit, no Taurhaus registry entry.** The compiled
activation obstacle was removed in scratch; the commissioned uptake question
remains unanswered because no executed run satisfied all trial prerequisites.
No other Codex hook boundary, agy or Grok trial was substituted. Permission/tool/
error/switch coverage and recovery-card correctness are excluded, not passed.

The first zero-turn setup lost its tmux pane before initialize; selecting
`/bin/sh` and the absolute native Codex binary fixed that local launcher setup.
The TUI's first Enter arrived before paste settling; a captured unsent composer
justified one more Enter, not a second prompt. Full controllers, replay commands,
regression checks, gates and durable cleanup are in [execution audit](execution-audit.md).
Private servers were stopped, owned children reaped, both ports and both tmux
sockets refused connections (errno 111), and copied auth.json was removed.

## Evidence regression checks

These credential-free checks test the summary's interpretation, not model uptake.
Before the prose correction, the three tests exited **1** with **11 failed
assertions/subtests**: missing confounds in both headlines, no explicit trust
decision, and stale round labels. After correction, all three passed (exit **0**).
The three earlier execution-audit checks also still pass (exit **0**).
Run from the checkout root; no harness, subprocess or account data is accessed:

```sh
python3 - <<'PYTEST'
from pathlib import Path
text = Path('docs/design/evidence/native-eligibility/codex-0.153.4-SessionStart-compact.md').read_text()
source = text.split('<!-- compact-confound-tests -->\n```python\n', 1)[1].split('\n```', 1)[0]
exec(compile(source, 'compact-confound-tests', 'exec'))
PYTEST
```

<!-- compact-confound-tests -->
```python
from pathlib import Path
import unittest

D = Path('docs/design/evidence/native-eligibility')
class CompactConfoundRegression(unittest.TestCase):
    # // Regression: 4651d244 buried untrusted-handler and invalid-identity confounds,
    # making its summary's null result look like evidence against boundary uptake.
    def test_summary_and_headline_disclose_invalid_trial(self):
        summary = (D / 'README.md').read_text()
        row = next(line for line in summary.splitlines() if line.startswith('| 3. Codex'))
        detail = (D / 'codex-0.153.4-SessionStart-compact.md').read_text()
        headline = detail.split('## ', 1)[0]
        for name, text in [('summary row', row), ('detail headline', headline)]:
            for fact in ['trustStatus: "untrusted"', 'invalid bridge identity',
                         'configuration-confounded', 'no valid uptake test']:
                with self.subTest(location=name, fact=fact):
                    self.assertIn(fact, text)

    def test_refusal_names_unestablished_trust_and_host_pin(self):
        detail = (D / 'codex-0.153.4-SessionStart-compact.md').read_text()
        decision = detail.split('## Eligibility refusal and re-commission decision\n', 1)
        self.assertEqual(len(decision), 2, 'explicit trust/refusal decision missing')
        decision = decision[1].split('\n## ', 1)[0]
        for fact in ['could not establish hook trust under the pinned app-server host',
                     'app-server probe; TUI UNVERIFIED', '/hooks', '/compact',
                     'not exhausted', 'orchestrator', '0 additional turns / $0.00']:
            with self.subTest(fact=fact):
                self.assertIn(fact, decision)

    def test_packet_headlines_do_not_mislabel_revision(self):
        for file in ['README.md', 'codex-0.153.4-SessionStart-compact.md']:
            with self.subTest(file=file):
                headline = (D / file).read_text().split('## ', 1)[0]
                self.assertNotIn('round 1', headline)

if __name__ == '__main__':
    unittest.main()
```
