"""Offline audit/export after cleanup; never launches a harness or reads auth."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt6_support import clean

BASE=Path(__file__).resolve().parent
OUT=BASE/'attempt7'; RUN=OUT/'run'
cleanup=json.loads((RUN/'cleanup.json').read_text())
assert cleanup['root_removed'] and not cleanup['survivors'] and cleanup['port_closed']
assert not Path(cleanup['root']).exists()
# Recheck PID + start ticks, never signal anything in the audit.
survivors=[]
for identity in json.loads((RUN/'identities.json').read_text()):
    try:
        ticks=Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(')',1)[1].split()[19]
        if ticks==identity['start_ticks']:survivors.append(identity['pid'])
    except FileNotFoundError:pass
assert not survivors
rows=[json.loads(l) for l in (RUN/'events.jsonl').read_text().splitlines()]
port=int(next(r for r in rows if r['kind']=='isolation')['environment']['TAURHAUS_DAEMON_PORT'])
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1',port))!=0
mesh=Path('/home/mstie/projects/mesh-push')
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
cap=(mesh/'src/delivery/app_server/capabilities.rs').read_text()
assert 'disposition: "disabled", enabled: false' in cap
host=[json.loads(l) for l in (RUN/'host-events.jsonl').read_text().splitlines()]
assert not any(e.get('method','').endswith('/delta') for e in host)
assert len(host)==len({json.dumps(e,sort_keys=True) for e in host})
items=[e['params']['item'] for e in host if e.get('method')=='item/completed']
exposure={}
for marker in ['saffron7c82ab','juniper8e21cd','maple7d91cb','cedar9a38ef']:
    matched=[i for i in items if marker in json.dumps(i)]
    assert [i['type'] for i in matched]==['userMessage','agentMessage']
    exposure[marker]=[i['id'] for i in matched]
logs=[json.loads(l) for l in (RUN/'taurhaus.log.jsonl').read_text().splitlines() if l.strip()]
rejections=[r for r in logs if r['event']=='hosted.rpc.rejected']
ledger=json.loads((RUN/'cost-ledger.json').read_text())
ledger.update({'completed_generations_metered':len(ledger['generations']),
    'protocol_turns_started':len(ledger['turn_ids']), 'paid_inputs_including_tui_steer':8,
    'api_equivalent_usd_scope':'metered subtotal only; excludes the interrupted final turn',
    'total_usd':'UNREPORTED: final turn started during teardown; no tokenUsage event emitted',
    'final_turn':{'turn_id':'01a089a9-946d-7ca3-8b43-17d518b961f1',
        'cost_usd':'UNREPORTED', 'usage':'No tokenUsage or rollout token_count before owned namespace teardown'},
    'budget_verification':'8 paid inputs <= 16. Metered conservative subtotal $0.1042728; full-run dollar total cannot be verified.'})
(RUN/'cost-ledger.json').write_text(json.dumps(ledger,indent=2)+'\n')
# Keep the emitted PASS in the lock helper as a local observation; it is not the
# step verdict. The subsequent daemon error makes overall step 4 FAIL.
for path in OUT.rglob('*.log'):
    text=clean(path.read_text())
    if path.parent.name=='gates':text='\n'.join(text.splitlines()[-60:])+'\n'
    path.with_suffix('.txt').write_text(text);path.unlink()
for path in OUT.rglob('*'):
    if not path.is_file():continue
    text=path.read_text()
    if path.suffix=='.json':text=json.dumps(clean(json.loads(text)),indent=2)+'\n'
    elif path.suffix=='.jsonl':
        clean_rows=clean([json.loads(l) for l in text.splitlines() if l.strip()])
        text=''.join(json.dumps(r)+'\n' for r in clean_rows)
    else:text=clean(text)
    path.write_text(text)
    if 'pane-' in path.name and path.suffix=='.txt':assert len(text.splitlines())<=60
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-push)(?:/|\b))',text),path
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',text),path
# Keep named step snapshots first; record aliases for duplicate captures/state.
manifest={}; seen={}
paths=sorted((p for p in OUT.rglob('*') if p.is_file()),key=lambda p:(not p.name.startswith(('step1','step2','step3','step4','step-1')),str(p)))
for p in paths:
    digest=hashlib.sha256(p.read_bytes()).hexdigest()
    if digest in seen:
        manifest[str(p.relative_to(OUT))]=str(seen[digest].relative_to(OUT));p.unlink()
    else:seen[digest]=p
(OUT/'duplicate-aliases.json').write_text(json.dumps(manifest,indent=2)+'\n')
audit={'trial_exit':1,'verdict':'FAIL step 4 after controlled SIGSTOP/SIGCONT lock interleave',
    'surviving_owned_processes':survivors,'private_port':port,'port_closed':True,
    'scratch_root_removed':True,'mesh_tree_clean':True,'descriptor_enabled':False,
    'hosted_rpc_rejected_rows':rejections,'absence_reason':'Read/correlation failures are client-side; no JSON-RPC host error object was emitted.',
    'exposure_item_ids':exposure,'steps':{'1':'PASS','2':'PASS','3':'PASS','4':'FAIL','5':'NOT RUN','6':'NOT RUN','7':'NOT RUN'},
    'gate_exits':{p.stem:json.loads(p.read_text())['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in json.loads(p.read_text())},
    'retained_bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())}
(OUT/'final-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
print(json.dumps(audit))
