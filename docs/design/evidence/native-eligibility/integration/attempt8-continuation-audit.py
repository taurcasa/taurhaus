"""Offline audit of the continuation; no harness or credentials are accessed."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt6_support import clean
from attempt8_continuation_support import compaction_metering_gaps

B=Path(__file__).resolve().parent
OUT=B/'attempt8/continuation'; RUN=OUT/'run'
cleanup=json.loads((RUN/'cleanup.json').read_text())
assert not cleanup['survivors'] and cleanup['port_closed'] and cleanup['root_removed']
assert not Path(cleanup['root']).exists()
for i in json.loads((RUN/'identities.json').read_text()):
    try: ticks=Path(f"/proc/{i['pid']}/stat").read_text().rsplit(')',1)[1].split()[19]
    except FileNotFoundError:continue
    assert ticks!=i['start_ticks']
rows=[json.loads(l) for l in (RUN/'events.jsonl').read_text().splitlines()]
isolation=next(r for r in rows if r['kind']=='isolation')
port=int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
with socket.socket() as probe:
    probe.settimeout(.2); assert probe.connect_ex(('127.0.0.1',port))!=0
mesh=Path('/home/mstie/projects/mesh-push')
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
assert 'disposition: "disabled", enabled: false' in (mesh/'src/delivery/app_server/capabilities.rs').read_text()
host=[json.loads(l) for l in (RUN/'host-events.jsonl').read_text().splitlines()]
assert not any(e.get('method','').endswith('/delta') for e in host)
items=[e['params']['item'] for e in host if e.get('method')=='item/completed']
markers=[json.loads((RUN/name).read_text()) for name in ['step2-marker.json','step3-marker.json']]
markers+=list(json.loads((RUN/'step4-markers.json').read_text()).values())
exposure={}
for marker in markers:
    matched=list({i['id']:i for i in items if marker in json.dumps(i)}.values())
    assert [i['type'] for i in matched]==['userMessage','agentMessage']
    exposure[marker]=[i['id'] for i in matched]
locks=[json.loads(l) for l in (RUN/'step4-locks.jsonl').read_text().splitlines()]
assert {Path(h['argv'][0]).name for r in locks for h in r['holders'] if h['fdinfo']}=={'mesh','taurhaus-daemon'}
boundary=json.loads((RUN/'step5-boundary-events.json').read_text())
assert any(e.get('method')=='item/completed' and e['params']['item']['type']=='contextCompaction' for e in boundary)
assert not any('[taurhaus] recovery_card' in json.dumps(e) for e in boundary)
before=json.loads((RUN/'step5-runtime-before.json').read_text());after=json.loads((RUN/'step5-runtime-boundary.json').read_text())
assert before['appServer']['threadId']==after['appServer']['threadId']
assert before['contextGeneration']==after['contextGeneration']=='0'
assert before['recovery']==after['recovery']
ledger=json.loads((RUN/'cost-ledger.json').read_text());gaps=compaction_metering_gaps(host)
assert gaps==['01a089c7-9994-7261-afcc-082ec2169813']
ledger['host_token_event_coverage_complete']=ledger['metering_complete']
ledger['metering_complete']=False
ledger['unmetered_compaction_cost_turn_ids']=gaps
ledger['api_equivalent_usd_scope']='Metered ordinary-generation subtotal; excludes unreported compaction cost.'
for g in ledger['generations']:
    if g['turn_id'] in gaps:
        g['host_reported_total_tokens']=6344
        g['billing_status']='UNREPORTED: zero token classes are a compaction counter reset, not proof of zero charge.'
(RUN/'cost-ledger.json').write_text(json.dumps(ledger,indent=2)+'\n')
prior=json.loads((B/'attempt8/cost-ledger.json').read_text())
combined={'prior_turns':prior['turns'],'continuation_protocol_turns':ledger['paid_inputs'],
    'total_protocol_turns':prior['turns']+ledger['paid_inputs'],
    'total_paid_inputs_including_typed_steer':prior['turns']+ledger['paid_inputs']+1,
    'prior_api_equivalent_usd':prior['api_equivalent_usd'],
    'continuation_metered_subtotal_usd':ledger['api_equivalent_usd'],
    'cumulative_metered_subtotal_usd':round(prior['api_equivalent_usd']+ledger['api_equivalent_usd'],9),
    'cumulative_conservative_subtotal_usd':round(prior['conservative_usd']+ledger['conservative_usd'],9),
    'unmetered_compaction_cost_turn_ids':gaps,
    'full_usd_cap_independently_verified':False,
    'basis':ledger['basis'],'generations':ledger['generations']}
assert combined['total_paid_inputs_including_typed_steer']<=16
(OUT/'cost-ledger.json').write_text(json.dumps(combined,indent=2)+'\n')
logs=[json.loads(l) for l in (RUN/'taurhaus.log.jsonl').read_text().splitlines() if l.strip()]
health=json.loads((RUN/'team/state/delivery/health-seat.json').read_text())
for path in OUT.rglob('*.log'):
    text=clean(path.read_text())
    if path.parent.name=='gates':text='\n'.join(text.splitlines()[-60:])+'\n'
    path.with_suffix('.txt').write_text(text);path.unlink()
for path in OUT.rglob('*'):
    if not path.is_file():continue
    text=path.read_text()
    if path.suffix=='.json':text=json.dumps(clean(json.loads(text)),indent=2)+'\n'
    elif path.suffix=='.jsonl':
        text=''.join(json.dumps(r)+'\n' for r in clean([json.loads(l) for l in text.splitlines() if l.strip()]))
        if path.name=='host-events.jsonl':text=''.join(dict.fromkeys(text.splitlines(keepends=True)))
    else:text=clean(text)
    path.write_text(text)
    if 'pane-' in path.name and path.suffix=='.txt': assert len(text.splitlines())<=60
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-push)(?:/|\b))',text),path
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',text),path
aliases={};seen={}
for path in sorted((p for p in OUT.rglob('*') if p.is_file()),key=lambda p:(not p.name.startswith('step'),str(p))):
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    if digest in seen:aliases[str(path.relative_to(OUT))]=str(seen[digest].relative_to(OUT));path.unlink()
    else:seen[digest]=path
(OUT/'duplicate-aliases.json').write_text(json.dumps(aliases,indent=2)+'\n')
audit={'verdict':'FAIL step 5: no recovery card at compact boundary or first following input',
    'steps':{'1':'PASS','2':'PASS','3':'PASS','4':'PASS','5':'FAIL','6':'NOT RUN','7':'NOT RUN'},
    'controller_exit':1,'private_port':port,'port_closed':True,'surviving_owned_processes':[],
    'root_removed':True,'mesh_tree_clean':True,'descriptor_enabled':False,'exposure_item_ids':exposure,
    'hosted_rpc_rejected_rows':[r for r in logs if r['event']=='hosted.rpc.rejected'],
    'compaction_log_rows':[r for r in logs if r['event'].startswith('compaction.')],
    'host_error_object':'None emitted: compaction and next input succeeded; recovery card was absent.',
    'mesh_health':health,'context_generation_before':'0','context_generation_after':'0',
    'protocol_turns_including_prior':combined['total_protocol_turns'],
    'metered_subtotal_usd':combined['cumulative_metered_subtotal_usd'],
    'compaction_cost_unreported':True,
    'gate_exits':{p.stem:json.loads(p.read_text())['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in json.loads(p.read_text())},
    'retained_bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())}
(OUT/'final-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
print(json.dumps(audit))
