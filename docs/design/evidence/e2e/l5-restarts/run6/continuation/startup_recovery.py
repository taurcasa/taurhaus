"""Retained one-off supported alpha recovery after its native startup exited.

Executed once from a Python heredoc during step 1; never an automatic retry.
Only the scratch daemon socket is contacted. No database is repaired or edited.
"""
import json, socket, time, secrets, sys
from pathlib import Path
from support import clean, enforce_budget

b = Path(__file__).resolve().parent
p = b / 'runtime'
events = [json.loads(l) for l in (p / 'events.jsonl').read_text().splitlines()]
iso = next(e for e in events if e['kind'] == 'isolation')
root = Path(iso['root'])
port = int(iso['environment']['TAURHAUS_DAEMON_PORT'])
a = json.loads((p / 'cost-ledger.json').read_text())
enforce_budget(a['paid_inputs'], a['api_equivalent_usd'])
params = {'request': {'team_name': 'l5-restarts', 'member_name': 'alpha'},
          'cli_commands': {'codex': {'fresh': 'codex --yolo',
                          'continue_cmd': 'codex --yolo', 'resume': 'codex --yolo resume'}},
          'tmux_layout': 'new_window'}
records = []

def rpc(method, params):
    rid = secrets.token_hex(8)
    record = {'at': time.time(), 'method': method, 'params': params}
    records.append(record)
    with socket.create_connection(('127.0.0.1', port), timeout=10) as c:
        c.sendall(json.dumps({'id': rid, 'method': method, 'params': params,
            'auth': (root / 'data/daemon.token').read_text().strip()}).encode() + b'\n')
        f = c.makefile('rb')
        while True:
            r = json.loads(f.readline())
            if r.get('id') == rid:
                break
    record['response'] = clean(r)
    (p / 'startup-alpha-recovery.json').write_text(json.dumps(clean({
        'reason': 'native Codex exited on concurrent scratch SQLite migration 38; no alpha turn began',
        'records': records}), indent=2) + '\n')
    assert 'error' not in r, str(clean(r.get('error')))
    return r['result']

a = rpc('coordination.resume_member', params)
end = time.monotonic() + 120
while time.monotonic() < end:
    s = rpc('coordination.resume_member_status', {'run_id': a['run_id']})
    if s['outcome']['status'] != 'running':
        print(json.dumps(clean(s['outcome'])))
        break
    time.sleep(.5)
else:
    raise TimeoutError('startup lifecycle recovery')
