"""Offline failed-run audit; never imports a paid controller or reads credentials."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt10_support import finalize_metering, pack_events, unpack_events
from attempt5_support import clean

B = Path(__file__).resolve().parent
OUT = B/'attempt10'
RUN = OUT/'run'
def read(path): return json.loads(path.read_text())
def save(path, value): path.write_text(json.dumps(value, indent=2)+'\n')
def rows(path): return [json.loads(l) for l in path.read_text().splitlines() if l]

result = read(RUN/'initialize-result.json')
report = result['outcome']['report']
assert report['failed_step'] == 'opt_in_delivery'
assert 'team owner already holds lifetime lock' in report['message']
save(RUN/'step1-outcome.json', {'step':1, 'outcome':'FAIL', 'failed_pipeline_step':report['failed_step'], 'reason':report['message']})
for n in range(2,8):
    save(RUN/f'step{n}-outcome.json', {'step':n, 'outcome':'NOT RUN', 'reason':'Stopped at step 1 failure; no retry.'})
cleanup = read(RUN/'cleanup.json')
assert not cleanup['survivors'] and cleanup['port_closed'] and cleanup['root_removed'] and cleanup['auth_removed']
assert not Path(cleanup['root']).exists()
for identity in read(RUN/'identities.json'):
    stat = Path(f"/proc/{identity['pid']}/stat")
    try: ticks = stat.read_text().rsplit(')',1)[1].split()[19]
    except FileNotFoundError: continue
    assert ticks != identity['start_ticks'], 'owned process survived'
trace = rows(RUN/'events.jsonl')
if (RUN/'event-payloads.json').exists():
    trace = unpack_events(trace,read(RUN/'event-payloads.json'))
isolation = next(r for r in trace if r['kind']=='isolation')
port = int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1',port)) != 0
mesh = Path('/home/mstie/projects/mesh-trial')
assert subprocess.check_output(['git','-C',str(mesh),'rev-parse','--short','HEAD'],text=True).strip() == 'fcb9647'
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
assert not (mesh/'target/debug/mesh').exists()
assert 'disposition: "disabled", enabled: false' in (mesh/'src/delivery/app_server/capabilities.rs').read_text()
accounted = finalize_metering(read(RUN/'cost-ledger.json'))
assert accounted['paid_inputs'] == 1 and not accounted['generations']
save(RUN/'cost-ledger.json',accounted)
# Lossless trace packing; complete daemon JSONL is not filtered or thinned.
packed,payloads=pack_events(trace)
assert unpack_events(packed,payloads)==trace
(RUN/'events.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in packed))
save(RUN/'event-payloads.json',payloads)
for source in [Path('/tmp/attempt10-controller-output.txt'),Path('/tmp/attempt10-build-output.txt')]:
    if source.exists(): (OUT/source.name.removeprefix('attempt10-')).write_text(clean(source.read_text()))
for path in OUT.rglob('*'):
    if not path.is_file(): continue
    data = clean(path.read_text())
    path.write_text(data)
    if 'pane-' in path.name: assert len(data.splitlines()) <= 60
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',data),path
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))',data),path
    assert 'item/agentMessage/delta' not in data,path
logs = rows(RUN/'taurhaus.log.jsonl')
assert len(logs)==len({json.dumps(r,sort_keys=True) for r in logs})
save(OUT/'final-audit.json', {
    'verdict':'FAIL step 1: opt_in_delivery lifetime-lock refusal',
    'controller_exit':1, 'steps':{'1':'FAIL', **{str(n):'NOT RUN' for n in range(2,8)}},
    'taurhaus_build_revision':'5c4132a9', 'identity_merge_ancestor':'6398bfa3',
    'mesh_revision':'fcb9647', 'private_port':port, 'marker':isolation['marker'],
    'mesh_clean':True,'descriptor_enabled':False,'mesh_trial_binary_removed':True,
    'cleanup':cleanup,'verified_pid_start_identities':len(read(RUN/'identities.json')),
    'daemon_jsonl_rows':len(logs),'daemon_jsonl_sha256':hashlib.sha256((RUN/'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    'daemon_jsonl_event_families':sorted({r['event'] for r in logs}),
    'trace_rows':len(trace),'trace_unique_payloads':len(payloads),
    'model_turns':accounted['paid_inputs'],'metering_complete':False,'total_usd':None,
    'gates':{p.stem:read(p)['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in read(p)},
})
print(json.dumps(read(OUT/'final-audit.json'),indent=2))
