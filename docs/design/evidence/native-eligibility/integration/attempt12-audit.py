"""Offline final evidence audit; no credentials, harness calls or process signals."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess

BASE = Path(__file__).resolve().parent / 'attempt12'
RUN = BASE / 'run'

def read(name):
    return json.loads((RUN / name).read_text())

def rows(name):
    return [json.loads(line) for line in (RUN / name).read_text().splitlines() if line]

trace = rows('events.jsonl')
events = rows('host-events.jsonl')
stopped = next(r for r in trace if r['kind'] == 'stopped')
assert stopped['step'] == 6 and 'missing completed reply' in stopped['error']
for step in range(1, 6):
    assert read(f'step{step}-outcome.json')['outcome'] == 'PASS'
assert read('step6-outcome.json')['outcome'] == 'FAIL'
assert read('step7-outcome.json')['outcome'] == 'NOT RUN'
before, after = read('step6-runtime-before.json'), read('step6-runtime-after.json')
thread = before['appServer']['threadId']
assert after['appServer']['threadId'] == thread
assert read('step6-resume-result.json')['outcome']['status'] == 'completed'
marker = read('step6-marker.json')
assert marker in (RUN / f"step6-observed-pane-{after['paneId'][1:]}.txt").read_text()
reply_lines = [n for n, e in enumerate(events, 1) if e.get('method') == 'item/completed'
               and e.get('params', {}).get('item', {}).get('type') == 'agentMessage'
               and marker in e['params']['item'].get('text', '')]
assert len(reply_lines) == 1
assert read('hosted-transcript.json')['thread']['status']['type'] == 'idle'
ledger = read('cost-ledger.json')
assert ledger['paid_inputs'] == 10 and len(ledger['generations']) == 11
assert len(ledger['unmetered_turn_ids']) == 1
missing = ledger['unmetered_turn_ids'][0]
assert any(e.get('method') == 'turn/completed' and e['params']['turn']['id'] == missing for e in events)
receipt_rows = [r for p in (RUN / 'team/state/messaging-v2/segments').glob('*.jsonl')
                for r in map(json.loads, p.read_text().splitlines())]
send = next(json.loads(l) for l in (RUN / 'step6-send.txt').read_text().splitlines() if l.startswith('{'))
receipts = [r for r in receipt_rows if r.get('payload', {}).get('message_id') == send['message_id']]
assert any(r['payload'].get('stage') == 'native_enqueued' for r in receipts)
(RUN / 'step6-receipts.json').write_text(json.dumps(receipts, indent=2) + '\n')
gaps = [e['params']['turnId'] for e in events if e.get('method') == 'thread/tokenUsage/updated'
        and e['params']['tokenUsage']['last']['totalTokens'] > 0
        and e['params']['tokenUsage']['last']['inputTokens'] == 0
        and e['params']['tokenUsage']['last']['outputTokens'] == 0]
assert len(gaps) == 1
final = json.loads(json.dumps(ledger))
for g in final['generations']:
    if g['turn_id'] in gaps:
        g['api_equivalent_usd'] = g['conservative_usd'] = None
        g['limitation'] = 'Compaction counter reset; billable token classes unreported.'
final.update(measured_api_equivalent_subtotal_usd=ledger['api_equivalent_usd'],
             measured_conservative_subtotal_usd=ledger['conservative_usd'],
             api_equivalent_usd=None, conservative_usd=None, actual_billed_usd=None,
             metering_complete=False, compaction_metering_gaps=gaps,
             paid_generation_slots_including_steer_and_compaction=11,
             limitation='One restart turn lacks matching usage; later usage carries the earlier recovery ID. No inferred reassignment. Full billed USD/cap unverified.')
(RUN / 'cost-final.json').write_text(json.dumps(final, indent=2) + '\n')
cleanup = read('cleanup.json')
assert cleanup['survivors'] == [] and cleanup['port_closed'] and cleanup['root_removed']
assert cleanup['descriptor_restore_exit'] == 0 and cleanup['mesh_trial_artifact_removed']
assert not Path(cleanup['root']).exists()
identities = {(i['pid'], i['start_ticks']) for name in ['step1-identities.json', 'step6-identities.json', 'identities.json'] for i in read(name)}
for pid, ticks in identities:
    try:
        current = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
    except FileNotFoundError:
        continue
    assert current != ticks, 'owned PID/start identity survived'
gate_cleanup = json.loads((BASE / 'gates/gate-cleanup.json').read_text())
assert gate_cleanup['children_waited'] and gate_cleanup['root_removed']
assert not Path(gate_cleanup['root']).exists()
tokens = [(b'TAURHAUS_TRIAL_ID=' + Path(r).name.encode() + b'\0')
          for r in [cleanup['root'], gate_cleanup['root']]]
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit():
        continue
    try:
        env = (proc / 'environ').read_bytes()
        assert not any(t in env for t in tokens), 'owned environment survived'
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        pass
port = int(next(r for r in trace if r['kind'] == 'isolation')['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1', port)) != 0
mesh = Path('/home/mstie/projects/mesh-trial')
assert subprocess.check_output(['git', '-C', str(mesh), 'rev-parse', '--short', 'HEAD'], text=True).strip() == 'ed59187'
assert not subprocess.check_output(['git', '-C', str(mesh), 'status', '--porcelain'], text=True)
assert not (mesh / 'target/debug/mesh').exists()
assert 'disposition: "disabled", enabled: false' in (mesh / 'src/delivery/app_server/capabilities.rs').read_text()
assert not subprocess.check_output(['git', 'diff', '1db4f9bf', '--', 'src', 'src-tauri'], text=True)
for path in BASE.rglob('*'):
    if not path.is_file():
        continue
    content = path.read_text()
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}', content), path
    assert not re.search(r'(?<![\w/-])' + re.escape(str(Path.home())) + r'/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))', content), path
    assert 'item/agentMessage/delta' not in content, path
    assert path.name != 'auth.json', path
    if 'pane-' in path.name:
        assert len(content.splitlines()) <= 60, path
daemon = rows('taurhaus.log.jsonl')
assert len(daemon) == len({json.dumps(r, sort_keys=True) for r in daemon})
families = sorted({r['event'] for r in daemon})
assert 'hosted.instruction_sources.loaded' in families
assert 'compaction.codex_host.delivered' in families
gate_exits = {n: json.loads((BASE / f'gates/gate-{n}.json').read_text())['exit']
              for n in ['check-quick', 'lint', 'test-contracts']}
assert all(code == 0 for code in gate_exits.values())
print(json.dumps(dict(verdict='INCONCLUSIVE: step 6 metering-ID mismatch; controller FAIL',
    controller_exit=1, steps={str(n): read(f'step{n}-outcome.json') for n in range(1, 8)},
    thread_id=thread, marker=marker, reply_event_lines=reply_lines, unmatched_turn=missing,
    host_poll_deferrals=[r for r in trace if r['kind'] == 'host_poll_deferred'],
    private_port=port, cleanup=cleanup, verified_pid_start_identities=len(identities),
    daemon_jsonl_rows=len(daemon), daemon_jsonl_event_families=families,
    daemon_jsonl_sha256=hashlib.sha256((RUN / 'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    descriptor_flip_commit=None, mesh_clean=True, gate_exits=gate_exits, spend=final), indent=2))
