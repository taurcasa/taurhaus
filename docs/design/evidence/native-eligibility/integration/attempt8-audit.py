"""Offline final audit: count real usage, redact/deduplicate evidence, verify teardown."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt6_support import clean

BASE=Path(__file__).resolve().parent
OUT=BASE/'attempt8'
ledgers=[]; cleanup_evidence=[]
for name in ['setup-collector-abort','run']:
    run=OUT/name
    cleanup=json.loads((run/'cleanup.json').read_text())
    assert cleanup['root_removed'] and cleanup['port_closed'] and not cleanup['survivors']
    assert not Path(cleanup['root']).exists()
    for identity in json.loads((run/'identities.json').read_text()):
        try: ticks=Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(')',1)[1].split()[19]
        except FileNotFoundError: continue
        assert ticks != identity['start_ticks'], identity['pid']
    rows=[json.loads(l) for l in (run/'events.jsonl').read_text().splitlines()]
    port=int(next(r for r in rows if r['kind']=='isolation')['environment']['TAURHAUS_DAEMON_PORT'])
    with socket.socket() as probe:
        probe.settimeout(.2)
        assert probe.connect_ex(('127.0.0.1',port))!=0
    cleanup_evidence.append({'run':name,'private_port':port,'root_removed':True,'surviving_owned_processes':[],'port_closed':True})
    ledger=json.loads((run/'cost-ledger.json').read_text())
    assert ledger['metering_complete'] and not ledger['unmetered_turn_ids']
    ledgers.append({'run':name,**ledger})
combined={'model':'gpt-5.6-luna','effort':'low','runs':ledgers,
    'turns':sum(l['paid_inputs'] for l in ledgers),
    'api_equivalent_usd':round(sum(l['api_equivalent_usd'] for l in ledgers),9),
    'conservative_usd':round(sum(l['conservative_usd'] for l in ledgers),9),
    'metering_complete':True,'basis':ledgers[0]['basis']}
assert combined['turns']<=16 and combined['conservative_usd']<=3
(OUT/'cost-ledger.json').write_text(json.dumps(combined,indent=2)+'\n')
mesh=Path('/home/mstie/projects/mesh-push')
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
assert 'disposition: "disabled", enabled: false' in (mesh/'src/delivery/app_server/capabilities.rs').read_text()
run=OUT/'run'
host=[json.loads(l) for l in (run/'host-events.jsonl').read_text().splitlines()]
assert not any(e.get('method','').endswith('/delta') for e in host)
logs=[json.loads(l) for l in (run/'taurhaus.log.jsonl').read_text().splitlines() if l]
marker=json.loads((run/'step3-marker.json').read_text())
items=[e['params']['item'] for e in host if e.get('method')=='item/completed']
assert not any(marker in json.dumps(i) for i in items)
for path in OUT.rglob('*.log'):
    content=clean(path.read_text())
    if path.parent.name=='gates':content='\n'.join(content.splitlines()[-60:])+'\n'
    path.with_suffix('.txt').write_text(content);path.unlink()
# Normalize JSON before duplicate comparison; drop duplicate host events only.
for path in OUT.rglob('*'):
    if not path.is_file():continue
    text=path.read_text()
    if path.suffix=='.json':text=json.dumps(clean(json.loads(text)),indent=2)+'\n'
    elif path.suffix=='.jsonl':
        rows=clean([json.loads(l) for l in text.splitlines() if l.strip()])
        text=''.join(json.dumps(r)+'\n' for r in rows)
        if path.name=='host-events.jsonl':text=''.join(dict.fromkeys(text.splitlines(keepends=True)))
    else:text=clean(text)
    path.write_text(text)
    if 'pane-' in path.name and path.suffix=='.txt': assert len(text.splitlines())<=60
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-push)(?:/|\b))',text),path
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',text),path
aliases={};seen={}
for path in sorted((p for p in OUT.rglob('*') if p.is_file()),key=lambda p:(not p.name.startswith('step'),str(p))):
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    if digest in seen:
        aliases[str(path.relative_to(OUT))]=str(seen[digest].relative_to(OUT));path.unlink()
    else:seen[digest]=path
(OUT/'duplicate-aliases.json').write_text(json.dumps(aliases,indent=2)+'\n')
audit={'verdict':'FAIL step 3 acceptance; product behavior INCONCLUSIVE (premature snapshot assertion)',
    'controller_exit':1,'steps':{'1':'PASS','2':'PASS','3':'FAIL','4':'NOT RUN','5':'NOT RUN','6':'NOT RUN','7':'NOT RUN'},
    'cleanup':cleanup_evidence,'mesh_tree_clean':True,'descriptor_enabled':False,
    'hosted_rpc_rejected_rows':[r for r in logs if r['event']=='hosted.rpc.rejected'],
    'host_error_object':'NONE EMITTED: no host RPC rejection; controller stopped on missing pending receipt',
    'mesh_health_reason':'failed to acquire lock: delivery mirror busy',
    'deferred_marker_exposures':0,'turns':combined['turns'],'api_equivalent_usd':combined['api_equivalent_usd'],
    'conservative_usd':combined['conservative_usd'],
    'gate_exits':{p.stem:json.loads(p.read_text())['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in json.loads(p.read_text())},
    'retained_bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())}
(OUT/'final-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
print(json.dumps(audit))
