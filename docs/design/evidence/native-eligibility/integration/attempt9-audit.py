"""Offline final audit/retention; never contacts a CLI, credentials or live host."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
import sys
from attempt9_support import pack_events, unpack_events, compaction_metering_gaps
from attempt5_support import clean

B=Path(__file__).resolve().parent
OUT=B/'attempt9'; RUN=OUT/'run'
def read(name):return json.loads((RUN/name).read_text())
def rows(name):return [json.loads(l) for l in (RUN/name).read_text().splitlines() if l]
def save(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
if '--expand' in sys.argv:
    for row in unpack_events(rows('events.jsonl'),read('event-payloads.json')):print(json.dumps(row))
    raise SystemExit(0)
cleanup=read('cleanup.json')
assert not cleanup['survivors'] and cleanup['root_removed'] and cleanup['port_closed']
assert not Path(cleanup['root']).exists()
for identity in read('identities.json'):
    try:ticks=Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(')',1)[1].split()[19]
    except FileNotFoundError:continue
    assert ticks!=identity['start_ticks'],identity['pid']
events=rows('events.jsonl')
isolation=next(r for r in events if r['kind']=='isolation')
port=int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
with socket.socket() as probe:
    probe.settimeout(.2);assert probe.connect_ex(('127.0.0.1',port))!=0
mesh=Path('/home/mstie/projects/mesh-trial')
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
assert 'disposition: "disabled", enabled: false' in (mesh/'src/delivery/app_server/capabilities.rs').read_text()
assert read('step7-mesh-activity.json')['activity_confidence']=='uncertain'
assert 'pending: activity not freshly idle' in (RUN/'step7-final-status.txt').read_text()
marker=read('step7-marker.json');assert marker not in (RUN/'step7-final-pane-18.txt').read_text()
for n in range(1,7):assert read(f'step{n}-outcome.json')['outcome']=='PASS'
assert read('step7-outcome.json')['outcome']=='FAIL'
host=rows('host-events.jsonl'); logs=rows('taurhaus.log.jsonl')
assert not any(e.get('method','').startswith('account/') for e in host)
for method in ['compaction.codex_host.received','compaction.codex_host.delivered']:
    assert any(r.get('event')==method for r in logs)
ledger=read('cost-ledger.json'); gaps=compaction_metering_gaps(host)
ledger['protocol_turns']=len(ledger['turn_ids'])
ledger['paid_inputs']=len(ledger['generations'])
ledger['metering_complete']=not gaps and not ledger['unmetered_turn_ids']
ledger['unmetered_turn_ids']=sorted(set(gaps+ledger['unmetered_turn_ids']))
ledger['actual_billed_usd']='unreported by host; numeric totals are ordinary-generation subtotals'
for entry in ledger['generations']:
    if entry['turn_id'] in gaps:
        entry['api_equivalent_usd']=None;entry['conservative_usd']=None
        entry['metering']='compaction counter reset; billable classes unreported, not zero'
        entry['reported_last']=next(e['params']['tokenUsage']['last'] for e in host if e.get('method')=='thread/tokenUsage/updated' and e['params']['turnId']==entry['turn_id'])
assert ledger['paid_inputs']<=16 and ledger['conservative_usd']<3
save(RUN/'cost-ledger.json',ledger)
# The complete daemon JSONL is never thinned or filtered by event family.
assert len(logs)==len({json.dumps(r,sort_keys=True) for r in logs})
raw_digest=hashlib.sha256((RUN/'taurhaus.log.jsonl').read_bytes()).hexdigest()
packed,payloads=pack_events(events)
assert unpack_events(packed,payloads)==events
(RUN/'events.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in packed))
save(RUN/'event-payloads.json',payloads)
# Preserve bounded stderr/build/gate excerpts; canonical daemon JSONL stays complete.
excerpts={}
for path in [*OUT.glob('*.log'),*OUT.glob('*-output.txt'),*RUN.glob('*-stderr.txt'),* (OUT/'gates').glob('*.log')]:
    original=clean(path.read_text()); lines=original.splitlines()
    excerpts[str(path.relative_to(OUT))]={'lines':len(lines),'sha256_sanitized':hashlib.sha256(original.encode()).hexdigest()}
    content='\n'.join(lines[:20]+['[middle omitted; complete daemon events are in run/taurhaus.log.jsonl]']+lines[-40:])+'\n' if len(lines)>60 else original
    target=path.with_suffix('.txt');target.write_text(content)
    if target!=path:path.unlink()
save(OUT/'excerpt-manifest.json',excerpts)
# Sanitize/validate retained text, normalize JSON for byte-level snapshot deduplication.
for path in OUT.rglob('*'):
    if not path.is_file():continue
    text=clean(path.read_text())
    if path.suffix=='.txt':text=text.rstrip()+'\n'
    if path.suffix=='.json':text=json.dumps(clean(json.loads(text)),indent=2)+'\n'
    path.write_text(text)
    if 'pane-' in path.name:assert len(text.splitlines())<=60
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',text),path
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-trial|mesh-push)(?:/|\b))',text),path
aliases={};seen={}
for path in sorted([p for p in OUT.rglob('*') if p.is_file()],key=lambda p:(not p.name.startswith('step'),str(p))):
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    if digest in seen:
        aliases[str(path.relative_to(OUT))]=str(seen[digest].relative_to(OUT));path.unlink()
    else:seen[digest]=path
save(OUT/'duplicate-aliases.json',aliases)
assert hashlib.sha256((RUN/'taurhaus.log.jsonl').read_bytes()).hexdigest()==raw_digest
verdict='FAIL — known defect (codex identity lane)'
audit={'verdict':verdict,'steps':{str(n):'PASS' for n in range(1,7)}|{'7':verdict},
 'controller_exit':1,'step7_controller_error':'plain tmux delivery stalled',
 'step7_classification_evidence':['run/step7-mesh-activity.json','run/step7-final-status.txt','run/step7-runtime-after.json','run/step7-final-pane-18.txt'],
 'private_port':port,'cleanup':cleanup,'mesh_tree_clean':True,'descriptor_enabled':False,
 'daemon_jsonl_rows':len(logs),'daemon_jsonl_sha256':raw_digest,'daemon_jsonl_event_families':sorted({r.get('event') for r in logs}),
 'hosted_rpc_rejected':[r for r in logs if r.get('event')=='hosted.rpc.rejected'],
 'host_error_object':'none emitted; step 7 is a pre-input activity deferral',
 'event_rows_losslessly_packed':len(events),'event_payloads':len(payloads),
 'protocol_turns':ledger['protocol_turns'],'paid_inputs':ledger['paid_inputs'],'ordinary_api_equivalent_usd':ledger['api_equivalent_usd'],
 'ordinary_conservative_usd':ledger['conservative_usd'],'compaction_metering_gaps':gaps,
 'gate_exits':{p.stem:json.loads(p.read_text())['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in json.loads(p.read_text())},
 'retained_bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())}
save(OUT/'final-audit.json',audit)
print(json.dumps(audit,indent=2))
