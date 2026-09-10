"""Offline attempt-13 evidence audit: no harness calls, credentials or signals."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt13_support import validate_tmux_activity

BASE = Path(__file__).resolve().parent / 'attempt13'
RUN = BASE / 'run'
def read(name): return json.loads((RUN / name).read_text())
def rows(name): return [json.loads(s) for s in (RUN / name).read_text().splitlines() if s]

trace, events = rows('events.jsonl'), rows('host-events.jsonl')
stopped = next(r for r in trace if r['kind'] == 'stopped')
assert stopped['step'] == 7 and stopped['error'] == 'step 7: plain tmux delivery stalled'
for step in range(1, 7): assert read(f'step{step}-outcome.json')['outcome'] == 'PASS'
assert read('step7-outcome.json')['outcome'] == 'FAIL'
thread = read('step1-runtime.json')['appServer']['threadId']
for name in ['step5-runtime-after.json', 'step6-runtime-after.json', 'step7-runtime-before.json']:
    assert read(name)['appServer']['threadId'] == thread
assert read('step7-stop.json') == {'ok': True}
assert read('step7-remove.json')['outcome']['report']['removed']
assert read('step7-add.json')['outcome']['report']['failed_step'] is None
plain = read('step7-runtime-after.json')
assert plain['terminalContract'] == 1 and not plain.get('appServer') and not plain['daemon_pid']
seat = validate_tmux_activity(read('step7-activity-before.json'), plain['paneId'], read('step7-mesh-activity.json'))
refusal = (RUN / 'step7-inplace-refusal.txt').read_text()
assert 'app_server_switch_requires_5b_recoverable_relaunch_packet' in refusal
receipt = [r['payload'] for r in read('step7-receipts.json') if r['event_type'] == 'receipt']
assert len(receipt) == 1 and receipt[0]['stage'] == 'submitted'
assert receipt[0]['adapter'] == 'tmux/1' and receipt[0]['origin'] == 'tmux_send_keys'
pane = (RUN / f"step7-final-pane-{plain['paneId'][1:]}.txt").read_text()
assert 'Summary: step7' in pane and 'I can’t execute commands in this transport trial.' in pane
assert read('step7-marker.json') not in pane

# Host usage stays authoritative for hosted turns. The replacement tmux TUI has
# no hosted tokenUsage stream; its retained rollout supplies the two extra rows.
raw = read('cost-ledger.json')
generations = json.loads(json.dumps(raw['generations']))
gaps = {e['params']['turnId'] for e in events if e.get('method') == 'thread/tokenUsage/updated'
        and e['params']['tokenUsage']['last']['totalTokens'] > 0
        and e['params']['tokenUsage']['last']['inputTokens'] == 0
        and e['params']['tokenUsage']['last']['outputTokens'] == 0}
for g in generations:
    g['source'] = 'host tokenUsage.last'
    if g['turn_id'] in gaps:
        g['api_equivalent_usd'] = g['conservative_usd'] = None
        g['limitation'] = 'Compaction reports totalTokens=6262 but zero billable token classes; not zero cost.'
turn = None
for index, row in enumerate(read('usage-events.json')):
    p = row['payload']
    if p['type'] == 'task_started': turn = p['turn_id']
    if p['type'] != 'token_count' or not p.get('info') or turn not in raw['unmetered_turn_ids']: continue
    u = p['info']['last_token_usage']
    i, c, o = u['input_tokens'], u['cached_input_tokens'], u['output_tokens']
    generations.append(dict(thread_id=seat['session_id'], turn_id=turn,
        input=i, cached_input=c, output=o, reasoning_output=u['reasoning_output_tokens'],
        api_equivalent_usd=round(((i-c)*.20+c*.02+o*1.20)/1e6, 9),
        conservative_usd=round((i+o)*1.20/1e6, 9),
        source=f'usage-events.json[{index}] (tmux rollout, preceding task_started)'))
assert len(generations) == 13 and len(raw['turn_ids']) == 12
assert set(raw['turn_ids']) == {g['turn_id'] for g in generations}
subtotal = round(sum(g['api_equivalent_usd'] or 0 for g in generations), 9)
conservative = round(sum(g['conservative_usd'] or 0 for g in generations), 9)
assert subtotal < 3 and conservative < 3 and len(raw['turn_ids']) <= 16
cost = dict(model=raw['model'], effort='low', turn_ids=raw['turn_ids'], paid_turns=12,
    generation_slots_including_steer_and_compaction=13, generations=generations,
    measured_api_equivalent_subtotal_usd=subtotal, measured_conservative_subtotal_usd=conservative,
    api_equivalent_usd=None, actual_billed_usd=None, metering_complete=False,
    compaction_metering_gaps=sorted(gaps), unjoined_turn_ids=[],
    basis=raw['basis'] + ' Plain tmux usage is supplemented from the isolated rollout.',
    limitation='Compaction billable classes and actual subscription debit unreported. Full billed USD/cap unverified; no invented zero or metering-based step failure.')
(RUN / 'cost-final.json').write_text(json.dumps(cost, indent=2) + '\n')

cleanup = read('cleanup.json')
assert cleanup['survivors'] == [] and cleanup['port_closed'] and cleanup['root_removed']
assert cleanup['descriptor_restore_exit'] == 0 and cleanup['mesh_trial_artifact_removed']
assert not Path(cleanup['root']).exists()
identities = {(i['pid'], i['start_ticks']) for name in ['step1-identities.json', 'step6-identities.json', 'identities.json'] for i in read(name)}
for pid, ticks in identities:
    try: current = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
    except FileNotFoundError: continue
    assert current != ticks, 'owned PID/start identity survived'
gate_cleanup = json.loads((BASE / 'gates/gate-cleanup.json').read_text())
assert gate_cleanup['children_waited'] and gate_cleanup['root_removed']
assert not Path(gate_cleanup['root']).exists()
tokens = [b'TAURHAUS_TRIAL_ID=' + Path(r).name.encode() + b'\0' for r in [cleanup['root'], gate_cleanup['root']]]
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit(): continue
    try: env = (proc / 'environ').read_bytes()
    except (FileNotFoundError, PermissionError, ProcessLookupError): continue
    assert not any(t in env for t in tokens), 'owned environment survived'
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
assert not subprocess.check_output(['git', 'diff', '206f88b0', '--', 'src', 'src-tauri'], text=True)
for path in BASE.rglob('*'):
    if not path.is_file(): continue
    content = path.read_text()
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}', content), path
    assert not re.search(r'(?<![\w/-])' + re.escape(str(Path.home())) + r'/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))', content), path
    assert 'item/agentMessage/delta' not in content, path
    assert path.name != 'auth.json', path
    if 'pane-' in path.name: assert len(content.splitlines()) <= 60, path
daemon = rows('taurhaus.log.jsonl')
assert len(daemon) == len({json.dumps(r, sort_keys=True) for r in daemon})
families = sorted({r['event'] for r in daemon})
assert 'hosted.instruction_sources.loaded' in families and 'compaction.codex_host.delivered' in families
gate_exits = {n: json.loads((BASE / f'gates/gate-{n}.json').read_text())['exit'] for n in ['check-quick', 'lint', 'test-contracts']}
assert all(code == 0 for code in gate_exits.values())
print(json.dumps(dict(verdict='INCONCLUSIVE: step 7 controller FAIL; tmux notice submitted, full-body marker absent',
    controller_exit=1, steps={str(n): read(f'step{n}-outcome.json') for n in range(1, 8)},
    hosted_thread=thread, tmux_session=seat['session_id'], tmux_receipt=receipt[0],
    private_port=port, cleanup=cleanup, verified_pid_start_identities=len(identities),
    daemon_jsonl_rows=len(daemon), daemon_jsonl_event_families=families,
    daemon_jsonl_sha256=hashlib.sha256((RUN / 'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    host_poll_deferrals=[r for r in trace if r['kind'] == 'host_poll_deferred'],
    descriptor_flip_commit=None, mesh_clean=True, gate_exits=gate_exits, spend=cost), indent=2))
