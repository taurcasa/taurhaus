"""Read-only final audit, except sanitizing retained evidence and writing its report.

Never launches a CLI/model/daemon or reads a credential. The runtime controller
already destroyed the authorized credential copy and disposable PID namespace.
"""
import json
from pathlib import Path
import re
import socket
import subprocess
import time
from attempt5_support import clean, ledger

BASE = Path(__file__).resolve().parent
OUT = BASE / 'attempt5'
RUN = OUT / 'run'

# The initial sanitizer hid too much authority/path metadata (retained honestly)
# and missed rollout rate_limits. Remove that account metadata before staging.
for directory in [RUN, OUT / 'step1']:
    for path in directory.rglob('*'):
        if path.suffix == '.json':
            path.write_text(json.dumps(clean(json.loads(path.read_text())), indent=2) + '\n')
        elif path.suffix == '.jsonl':
            rows = clean([json.loads(line) for line in path.read_text().splitlines() if line])
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows))

def read(name):
    return json.loads((RUN / name).read_text())

checks = []
def check(name, condition):
    assert condition, name
    checks.append(name)

events = [json.loads(line) for line in (RUN / 'events.jsonl').read_text().splitlines()]
iso = next(row for row in events if row['kind'] == 'isolation')
root = Path(iso['root'])
record = read('step1-runtime.json')
host = record['appServer']
check('production initialization completed', read('initialize-result.json')['outcome']['report']['failed_step'] is None)
check('terminal contract and instructed hosted seat', record['terminalContract'] == 1 and host['instructionSources'] == [str(root / 'project/AGENTS.md')])
check('exact descriptor tuple', all(host[k] == v for k, v in {
    'build':'0.153.4', 'transport':'unix-websocket', 'host':'taurhaus-daemon-owned-thread/1',
    'configuration':'strict-config/1', 'trust':'daemon-owned/1'}.items()))
pane = (RUN / 'step1-completed-pane-2.txt').read_text()
check('startup card and reply visible', '[taurhaus] recovery_card' in pane and 'Please provide the missing task' in pane)
diag = read('step2-diagnostic.json')
check('real scheduler refusal', diag['health']['last_defer_reason'] == 'IO error: delivery: pending: activity absent')
check('adapter selection absent and activity path unset', diag['adapter_selection_exists'] is False and diag['runtime_activity_snapshot_path'] is None)
config = read('team/config.json')
check('production native request survived in config', next(m for m in config['members'] if m['name'] == 'seat')['adapter_mode'] == 'app_server')
view = read('step2-hosted-transcript.json')
check('host idle with exactly startup turn', view['thread']['status']['type'] == 'idle' and len(view['thread']['turns']) == 1)
check('marker absent from model and pane', iso['marker'] not in json.dumps(view) and iso['marker'] not in (RUN / 'step2-pending-pane-2.txt').read_text())
journal = [json.loads(line) for line in (RUN / 'team/state/messaging-v2/segments/000001.jsonl').read_text().splitlines()]
accepted = [r for r in journal if r['event_type'] == 'message_accepted' and iso['marker'] in r['payload']['body']]
check('exactly one wake-eligible marker accepted', len(accepted) == 1 and accepted[0]['payload']['dispatch_decision']['wake_eligible'])
message_id = accepted[0]['payload']['message_id']
check('no marker receipt or explicit read', not any(r['event_type'] != 'message_accepted' and r['payload'].get('message_id') == message_id for r in journal))
check('no later steps or work submitted', not any(r['kind'] == 'action' and r['action'].get('step', 0) > 2 for r in events))
host_events = [json.loads(line) for line in (RUN / 'host-events.jsonl').read_text().splitlines()]
usage = read('usage-events.json')
rollout_turns = [r['payload']['turn_id'] for r in usage if r['payload']['type'] == 'task_started']
cost = ledger(host_events, rollout_turns)
check('exact metered cost and one turn', cost == read('cost-ledger.json') and cost['paid_inputs'] == 1 and cost['metering_complete'] and cost['api_equivalent_usd'] == .00095164)
check('budget', cost['paid_inputs'] <= 8 and cost['conservative_usd'] < 3)
structured = [json.loads(line) for line in (RUN / 'taurhaus.log.jsonl').read_text().splitlines()]
check('no host rejection (failure preceded host RPC)', not any(r['event'] == 'hosted.rpc.rejected' for r in structured))
check('descriptor restoration succeeded', any(r['kind'] == 'descriptor_restored' and r['exit'] == 0 for r in events))
mesh = '/home/mstie/projects/mesh-push'
check('Mesh worktree clean', not subprocess.check_output(['git','-C',mesh,'status','--porcelain'], text=True))
check('compiled descriptor source remains disabled', 'disposition: "disabled", enabled: false' in (Path(mesh) / 'src/delivery/app_server/capabilities.rs').read_text())
survivors = []
for identity in read('identities.json'):
    try:
        fields = Path(f'/proc/{identity["pid"]}/stat').read_text().rsplit(')',1)[1].split()
        if fields[19] == identity['start_ticks'] and fields[0] != 'Z':
            survivors.append(identity['pid'])
    except FileNotFoundError:
        pass
check('all eleven recorded processes absent', len(read('identities.json')) == 11 and not survivors)
port = int(iso['environment']['TAURHAUS_DAEMON_PORT'])
with socket.socket() as probe:
    probe.settimeout(.2)
    check('private port closed', 20000 <= port <= 31999 and probe.connect_ex(('127.0.0.1',port)) != 0)
check('scratch home, credentials and sockets destroyed', not root.exists() and not Path(host['socketPath']).exists())
gates = [json.loads((OUT / f'gates/gate-{name}.json').read_text()) for name in ['check-quick','lint','test-contracts']]
check('three exact gates passed', all(g['exit'] == 0 for g in gates))
check('gate scratch destroyed', json.loads((OUT / 'gates/gate-cleanup.json').read_text())['root_removed'])

def sensitive(value):
    if isinstance(value, dict):
        for key, item in value.items():
            if key in ['rate_limits','rateLimits'] or key == 'method' and str(item).startswith('account/'):
                return True
            if any(s in key.lower() for s in ['installation','accesstoken','refreshtoken','idtoken','controlauthtokenhash']) and item != '<redacted>':
                return True
            if sensitive(item): return True
    elif isinstance(value, list):
        return any(sensitive(item) for item in value)
    return False

for path in RUN.rglob('*'):
    if path.suffix == '.json': check('sanitized ' + str(path.relative_to(RUN)), not sensitive(json.loads(path.read_text())))
    elif path.suffix == '.jsonl': check('sanitized ' + str(path.relative_to(RUN)), not sensitive([json.loads(line) for line in path.read_text().splitlines()]))
result = {'at':time.time(), 'checks':checks, 'survivors':survivors, 'port':port,
          'outcomes':{'1':'PASS','2':'FAIL: pending: activity absent', **{str(i):'NOT RUN: stop on step 2' for i in range(3,8)}},
          'cost':cost, 'gates':gates, 'marker':iso['marker'], 'message_id':message_id}
(OUT / 'final-audit.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'checks_passed':len(checks), 'survivors':survivors, 'gate_exits':[g['exit'] for g in gates]}))
