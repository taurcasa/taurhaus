"""Read-only attempt-11 audit: evidence, process identities, and disabled pin."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt11_support import finalize_metering

BASE = Path(__file__).resolve().parent / 'attempt11'
RUN = BASE / 'run'
def read(path): return json.loads(path.read_text())
def rows(path): return [json.loads(line) for line in path.read_text().splitlines() if line]

trace = rows(RUN / 'events.jsonl')
stop = next(row for row in trace if row['kind'] == 'stopped')
assert stop['step'] == 1 and 'host member busy' in stop['error']
initialized = read(RUN / 'initialize-result.json')
assert initialized['outcome']['status'] == 'completed'
assert initialized['outcome']['report']['failed_step'] is None
record = read(RUN / 'step1-runtime.json')
assert record['terminalContract'] == 1 and record['appServer']['instructionSources']
assert record['appServer']['build'] == '0.153.4'
assert '--strict-config' in record['appServer']['attachArgv']
assert 'gpt-5.6-luna low' in (RUN / 'step-1-pane-2.txt').read_text()
cleanup = read(RUN / 'cleanup.json')
assert not cleanup['survivors'] and cleanup['port_closed'] and cleanup['root_removed']
assert cleanup['descriptor_restore_exit'] == 0 and cleanup['mesh_trial_artifact_removed']
assert not Path(cleanup['root']).exists()
identities = read(RUN / 'identities.json')
for identity in identities:
    try: ticks = Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(')', 1)[1].split()[19]
    except FileNotFoundError: continue
    assert ticks != identity['start_ticks'], 'owned PID/start identity survived'
isolation = next(row for row in trace if row['kind'] == 'isolation')
port = int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1', port)) != 0
mesh = Path('/home/mstie/projects/mesh-trial')
revision = subprocess.check_output(['git', '-C', str(mesh), 'rev-parse', '--short', 'HEAD'], text=True).strip()
porcelain = subprocess.check_output(['git', '-C', str(mesh), 'status', '--porcelain'], text=True)
assert revision == 'ed59187' and not porcelain
assert not (mesh / 'target/debug/mesh').exists()
assert 'disposition: "disabled", enabled: false' in (mesh / 'src/delivery/app_server/capabilities.rs').read_text()
accounted = finalize_metering(read(RUN / 'cost-ledger.json'))
assert accounted['paid_inputs'] == 1 and not accounted['generations']
assert accounted['api_equivalent_usd'] is None and not accounted['metering_complete']
for path in BASE.rglob('*'):
    if not path.is_file(): continue
    content = path.read_text()
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}', content), path
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))', content), path
    assert 'item/agentMessage/delta' not in content, path
    if 'pane-' in path.name: assert len(content.splitlines()) <= 60
logs = rows(RUN / 'taurhaus.log.jsonl')
assert len(logs) == len({json.dumps(row, sort_keys=True) for row in logs})
assert any(row['event'] == 'hosted.instruction_sources.loaded' for row in logs)
assert any(row['event'] == 'onboarding.delivery.observed' and row.get('path') == 'app_server' for row in logs)
gates = BASE / 'gates'
if (gates / 'gate-cleanup.json').exists():
    gate_cleanup = read(gates / 'gate-cleanup.json')
    assert gate_cleanup['children_waited'] and gate_cleanup['root_removed']
    assert not Path(gate_cleanup['root']).exists()
    token = ('TAURHAUS_TRIAL_ID=' + Path(gate_cleanup['root']).name).encode() + b'\0'
    for process in Path('/proc').iterdir():
        if not process.name.isdigit(): continue
        try: assert token not in (process / 'environ').read_bytes(), 'gate process survived'
        except (FileNotFoundError, PermissionError, ProcessLookupError): pass
print(json.dumps({
    'verdict': 'INCONCLUSIVE: step 1 transcript observation stopped on host member busy',
    'controller_exit': 1,
    'steps': {str(n): read(RUN / f'step{n}-outcome.json') for n in range(1, 8)},
    'initialize_run_id': initialized['run_id'], 'private_port': port,
    'thread_id': record['appServer']['threadId'], 'mesh_revision': revision,
    'mesh_clean': not porcelain, 'descriptor_enabled': False, 'descriptor_flip_commit': None,
    'cleanup': cleanup, 'verified_pid_start_identities': len(identities),
    'daemon_jsonl_rows': len(logs),
    'daemon_jsonl_sha256': hashlib.sha256((RUN / 'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    'daemon_jsonl_event_families': sorted({row['event'] for row in logs}),
    'spend': accounted,
    'gate_exits': {p.stem: read(p)['exit'] for p in gates.glob('gate-*.json') if 'exit' in read(p)},
}, indent=2))
