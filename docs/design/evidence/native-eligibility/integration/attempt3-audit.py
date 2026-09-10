"""Read-only post-run audit. No CLI/model invocation or trial retry."""
import json
from pathlib import Path
import re
import socket
import subprocess

checkout = Path.cwd()
base = Path(__file__).resolve().parent
out = base / 'attempt3'
run = out / 'run'
events = [json.loads(line) for line in (run / 'events.jsonl').read_text().splitlines()]
isolation = next(row for row in events if row['kind'] == 'isolation')
root = Path(isolation['root'])
port = int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
assert 'TMUX' not in isolation['environment']
assert isolation['initial_codex_entries'] == ['auth.json']
request = next(row['request']['params']['request'] for row in events
               if row['kind'] == 'daemon_request' and row['request']['method'] == 'coordination.initialize_team')
assert len(request['agents']) == 1
assert request['agents'][0]['delivery'] == 'app_server'
assert request['agents'][0]['model'] == 'gpt-5.6-luna'
assert request['agents'][0]['reasoning_effort'] == 'low'
policy_source = (checkout / 'src/lib/components/meshTabUtils.js').read_text()
policy = json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)', policy_source, re.S)[1])
assert request['messaging'] == {'mode': 'canonical', 'retentionPolicy': policy}
report = json.loads((run / 'initialize-result.json').read_text())['outcome']['report']
assert report['failed_step'] == 'launch_host'
assert report['message'] == 'Conflict: host rejected request; reconcile before retrying'
assert not any(row['kind'] in ['action', 'inspection_ready'] for row in events)
logs = [json.loads(line) for line in (run / 'taurhaus.log.jsonl').read_text().splitlines()]
instructions = next(row for row in logs if row['event'] == 'hosted.instruction_sources.loaded')
receipt = next(row for row in logs if row['event'] == 'onboarding.delivery.observed')
assert instructions['count'] == 1 and instructions['thread_id']
assert receipt['stage'] == 'failed' and receipt['path'] == 'app_server'
assert receipt['accepted_bytes'] == receipt['offered_bytes'] == receipt['returned_by_read_bytes'] == 0
assert not (run / 'team/state/messaging-v2/segments/000001.jsonl').read_bytes()
assert json.loads((run / 'usage-events.json').read_text()) == []
assert json.loads((run / 'cost-ledger.json').read_text())['paid_inputs'] == 0
assert not root.exists()
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1', port)) != 0
assert not (root / 'tmux/tmux-1000/default').exists()
survivors = []
identities = json.loads((run / 'identities.json').read_text())
for identity in identities:
    try:
        stat = Path('/proc', str(identity['pid']), 'stat').read_text().rsplit(')', 1)[1].split()
        if stat[19] == identity['start_ticks']:
            survivors.append(identity['pid'])
    except (FileNotFoundError, ProcessLookupError):
        pass
# Also find any unrecorded descendant retaining the private run identity.
for process in Path('/proc').iterdir():
    if not process.name.isdigit():
        continue
    try:
        if (b'TAURHAUS_TRIAL_ID=' + root.name.encode() + b'\0') in (process / 'environ').read_bytes():
            survivors.append(int(process.name))
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        pass
assert not survivors, survivors
mesh = '/home/mstie/projects/mesh-push'
assert subprocess.check_output(['git', '-C', mesh, 'status', '--porcelain'], text=True) == ''
assert subprocess.check_output(['git', '-C', mesh, 'branch', '--show-current'], text=True).strip() == 'feat/native-push'
assert subprocess.check_output(['git', 'branch', '--show-current'], text=True).strip() == 'feat/integration-trial'
assert not subprocess.check_output(['git', 'diff', '--name-only', '--', 'src-tauri/', 'src/'], text=True)
assert next(row for row in events if row['kind'] == 'descriptor_restored')['exit'] == 0
gates = {}
for name in ['check-quick', 'lint', 'test-contracts']:
    result = json.loads((out / 'gates' / f'gate-{name}.json').read_text())
    assert result['exit'] == 0
    gates[name] = result['exit']
assert json.loads((out / 'gates/gate-cleanup.json').read_text())['root_removed']
files = list(out.rglob('*')) + [base / f'attempt3-{name}.py' for name in ['build', 'controller', 'audit']]
for path in files:
    if not path.is_file():
        continue
    assert path.name not in ['auth.json', 'daemon.token']
    text = path.read_text()
    assert not re.search(r'\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.', text), path
    assert not re.search(r'\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}', text), path
    for allowed in [str(checkout), mesh]:
        text = text.replace(allowed, '<checkout>')
    assert str(Path.home()) + '/' not in text, path
result = {'step_1': 'FAIL: launch_host rejected startup recovery',
          'steps_2_through_7': 'NOT RUN', 'observed_turns': 0, 'observed_usd': 0,
          'recorded_identities_absent': len(identities), 'survivors': survivors,
          'port_closed': port, 'private_tmux_socket_absent': True,
          'scratch_root_and_auth_removed': True, 'descriptor_restored': True,
          'mesh_worktree_clean': True, 'gates': gates, 'product_diff': [],
          'artifact_hygiene': 'passed', 'review': 'local read-only audit; no Opus lens'}
(out / 'final-verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
