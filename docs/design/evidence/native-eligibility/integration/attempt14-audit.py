"""Read-only runtime assertions and spend ledger for the step-7 continuation."""
import ast
import hashlib
import json
from pathlib import Path
import re
import socket
from unittest.mock import Mock
from attempt14_support import validate_tmux_activity

BASE = Path(__file__).resolve().parent
RUN = BASE / 'attempt14/run'
OLD = BASE / 'attempt13/run'
def read(name): return json.loads((RUN / name).read_text())
def rows(name): return [json.loads(line) for line in (RUN / name).read_text().splitlines() if line]

for n in range(1, 7):
    assert json.loads((OLD / f'step{n}-outcome.json').read_text())['outcome'] == 'PASS'
assert json.loads((OLD / 'step7-outcome.json').read_text())['outcome'] == 'FAIL'
for n in [1, 7]: assert read(f'step{n}-outcome.json')['outcome'] == 'PASS'
trace = rows('events.jsonl')
assert not any(r['kind'] == 'stopped' for r in trace)
old_trace = [json.loads(line) for line in (OLD/'events.jsonl').read_text().splitlines()]
binaries = {}
for name in ['taurhaus-daemon','mesh','codex','codex-code-mode-host']:
    prior_hash = next(r['sha256'] for r in old_trace if r['kind'] == 'binary' and r['name'] == name)
    current_hash = next(r['sha256'] for r in trace if r['kind'] == 'binary' and r['name'] == name)
    binaries[name] = dict(attempt13=prior_hash, continuation=current_hash, equal=prior_hash == current_hash)
    if name != 'mesh': assert prior_hash == current_hash
assert (BASE/'attempt13/mesh-trial-descriptor.diff').read_bytes() == (BASE/'attempt14/mesh-trial-descriptor.diff').read_bytes()
assert [r['action']['step'] for r in trace if r['kind'] == 'action' and r['action']['op'] == 'step'] == [1, 7]
assert read('step7-runtime-before.json')['appServer']['threadId'] == read('step1-runtime.json')['appServer']['threadId']
assert read('step7-stop.json') == {'ok': True}
assert read('step7-remove.json')['outcome']['report']['removed']
assert read('step7-add.json')['outcome']['report']['failed_step'] is None
plain = read('step7-runtime-after.json')
assert plain['terminalContract'] == 1 and not plain.get('appServer') and not plain['daemon_pid']
seat = validate_tmux_activity(read('step7-activity-before.json'), plain['paneId'], read('step7-mesh-activity.json'))
assert 'app_server_switch_requires_5b_recoverable_relaunch_packet' in (RUN/'step7-inplace-refusal.txt').read_text()
receipts = [r['payload'] for r in read('step7-receipts.json') if r['event_type'] == 'receipt']
assert len(receipts) == 1
assert all(receipts[0][k] == v for k,v in dict(stage='submitted', adapter='tmux/1', origin='tmux_send_keys').items())
pane = (RUN/f"step7-final-pane-{plain['paneId'][1:]}.txt").read_text()
assert read('step7-marker.json') not in pane
# Re-evaluate the actual controller predicate against retained runtime artifacts.
tree = ast.parse((BASE/'attempt14-steps.py').read_text())
node = next(n for n in ast.walk(tree) if isinstance(n, ast.FunctionDef) and n.name == 'delivered')
scope = dict(OUT=RUN, after=plain, capture=Mock(), status=Mock(), action=Mock(), save=Mock(),
             current_receipts=lambda name: read('step7-receipts.json'), marker=read('step7-marker.json'))
exec(compile(ast.Module(body=[node], type_ignores=[]), 'recorded-predicate', 'exec'), scope)
assert scope['delivered']()

raw = read('cost-ledger.json')
generations = raw['generations']
for g in generations: g['source'] = 'host tokenUsage.last'
turn = None
for index, row in enumerate(read('usage-events.json')):
    p = row['payload']
    if p['type'] == 'task_started': turn = p['turn_id']
    if p['type'] != 'token_count' or not p.get('info') or turn not in raw['unmetered_turn_ids']: continue
    u = p['info']['last_token_usage']
    i,c,o = u['input_tokens'],u['cached_input_tokens'],u['output_tokens']
    generations.append(dict(thread_id=seat['session_id'], turn_id=turn, input=i, cached_input=c,
        output=o, reasoning_output=u['reasoning_output_tokens'],
        api_equivalent_usd=round(((i-c)*.20+c*.02+o*1.20)/1e6,9),
        conservative_usd=round((i+o)*1.20/1e6,9), source=f'usage-events.json[{index}] (tmux rollout)'))
assert len(generations) == len(raw['turn_ids']) == 3
assert {g['turn_id'] for g in generations} == set(raw['turn_ids'])
prior = json.loads((OLD/'cost-final.json').read_text())
subtotal = round(sum(g['api_equivalent_usd'] for g in generations),9)
conservative = round(sum(g['conservative_usd'] for g in generations),9)
combined_turns = prior['paid_turns'] + len(raw['turn_ids'])
combined = round(prior['measured_conservative_subtotal_usd'] + conservative,9)
assert combined_turns <= 16 and combined <= 3
cost = dict(model=raw['model'], effort='low', paid_turns=3, generations=generations,
    measured_api_equivalent_subtotal_usd=subtotal, measured_conservative_subtotal_usd=conservative,
    combined_paid_turns=combined_turns, combined_conservative_subtotal_usd=combined,
    combined_api_equivalent_subtotal_usd=round(prior['measured_api_equivalent_subtotal_usd']+subtotal,9),
    actual_billed_usd=None, combined_metering_complete=False,
    limitation='Attempt 13 compaction billable classes and actual subscription debit remain unreported; no invented zero.')
(RUN/'cost-final.json').write_text(json.dumps(cost,indent=2)+'\n')
cleanup = read('cleanup.json')
assert cleanup['survivors'] == [] and cleanup['port_closed'] and cleanup['root_removed']
assert cleanup['descriptor_restore_exit'] == 0 and cleanup['mesh_trial_artifact_removed']
assert not Path(cleanup['root']).exists()
identities = {(i['pid'],i['start_ticks']) for n in ['step1-identities.json','identities.json'] for i in read(n)}
for pid,ticks in identities:
    try: current = Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19]
    except FileNotFoundError: continue
    assert current != ticks
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit(): continue
    try: env = (proc/'environ').read_bytes()
    except (FileNotFoundError, PermissionError, ProcessLookupError): continue
    assert b'TAURHAUS_TRIAL_ID='+Path(cleanup['root']).name.encode()+b'\0' not in env
port = int(next(r for r in trace if r['kind'] == 'isolation')['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1',port)) != 0
for path in (BASE/'attempt14').rglob('*'):
    if not path.is_file(): continue
    content = path.read_text()
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',content),path
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))',content),path
    assert path.name != 'auth.json',path
    if 'pane-' in path.name: assert len(content.splitlines()) <= 60,path
log = rows('taurhaus.log.jsonl')
assert len(log) == len({json.dumps(r,sort_keys=True) for r in log})
print(json.dumps(dict(verdict='PASS: attempt 13 steps 1-6 plus corrected live step 7',
    repeated_steps=[1,7], binaries=binaries, controller_exit=0, private_port=port, cleanup=cleanup,
    verified_pid_start_identities=len(identities), tmux_session=seat['session_id'], tmux_receipt=receipts[0],
    daemon_jsonl_rows=len(log), daemon_jsonl_sha256=hashlib.sha256((RUN/'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    warnings=[r for r in log if r.get('level') == 'WARN'], spend=cost),indent=2))
