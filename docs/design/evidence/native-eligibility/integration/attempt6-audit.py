"""Offline final evidence assertions; never opens a model/daemon connection."""
import json
from pathlib import Path
import re
import socket
import subprocess
from attempt5_support import ledger

BASE = Path(__file__).resolve().parent
OUT = BASE / 'attempt6'
RUN = OUT / 'run'
checks = []
def check(label, ok):
    assert ok, label
    checks.append(label)
def read(path): return json.loads((OUT / path).read_text())
def lines(path): return [json.loads(l) for l in (OUT / path).read_text().splitlines() if l]

events = lines('run/events.jsonl')
iso = next(e for e in events if e['kind']=='isolation')
root = Path(iso['root'])
record = read('step1/step1-runtime.json')
app = record['appServer']
check('initialize production pipeline completed', read('step1/initialize-result.json')['outcome']['report']['failed_step'] is None)
check('terminal contract and AGENTS instruction source', record['terminalContract']==1 and app['instructionSources']==[str(root/'project/AGENTS.md')])
check('pinned exact classes', all(app[k]==v for k,v in {'build':'0.153.4','transport':'unix-websocket','host':'taurhaus-daemon-owned-thread/1','configuration':'strict-config/1','trust':'daemon-owned/1'}.items()))
check('startup card and response in attached pane', '[taurhaus] recovery_card' in (OUT/'step1/step-1-pane-2.txt').read_text() and 'Unassigned; no objective' in (OUT/'step1/step-1-pane-2.txt').read_text())
check('paired adapter config selected', 'adapter seat: mode=app_server source=config' in (OUT/'step2/step2-status-before.txt').read_text())
journal = lines('run/team/state/messaging-v2/segments/000001.jsonl')
markers = [iso['marker'], 'deferred4e17e869']
view = read('run/hosted-transcript.json')
check('same idle thread at stop', view['thread']['id']==app['threadId'] and view['thread']['status']['type']=='idle')
for marker in markers:
    accepted = [r for r in journal if r['event_type']=='message_accepted' and marker in r['payload'].get('body','')]
    check(marker+' accepted once', len(accepted)==1)
    mid = accepted[0]['payload']['message_id']
    receipts = [r for r in journal if r['event_type']=='receipt' and r['payload'].get('message_id')==mid]
    enqueue = [r for r in receipts if r['payload'].get('stage')=='native_enqueued']
    check(marker+' enqueued once', len(enqueue)==1)
    proof = json.loads(enqueue[0]['payload']['evidence'])
    check(marker+' start on same thread with ids', proof['method']=='turn/start' and proof['thread_id']==app['threadId'] and proof['turn_id'] and proof['request_id'])
    items = [e['params']['item'] for e in lines('run/host-events.jsonl') if e['method']=='item/completed']
    check(marker+' one user exposure and one reply', sum(i['type']=='userMessage' and marker in json.dumps(i) for i in items)==1 and sum(i['type']=='agentMessage' and i.get('text')==marker for i in items)==1)
    if marker==markers[1]:
        pending = [r for r in receipts if r['payload'].get('stage')=='pending']
        check('five explicit active deferrals before enqueue', len(pending)==5 and all('thread_active' in r['payload']['evidence'] and r['sequence']<enqueue[0]['sequence'] for r in pending))
        check('all pending receipts are pre-input', all(json.loads(r['payload']['evidence'])['method'] is None for r in pending))
pre = lines('step2/journal-before-read.jsonl')
check('no read receipt before explicit read', all(r['payload'].get('kind')!='consumed_by_read' for r in pre))
check('one explicit read context', read('step2/explicit-read-receipt.json')['payload']['context']=='explicit-mesh-cli')
host = lines('run/host-events.jsonl')
check('no duplicated host snapshots', len(host)==len({json.dumps(e,sort_keys=True) for e in host}))
usage = read('run/usage-events.json')
turns = [r['payload']['turn_id'] for r in usage if r['payload']['type']=='task_started']
cost = ledger(host,turns)
check('four fully metered turns match ledger', cost==read('run/cost-ledger.json') and cost['paid_inputs']==4 and cost['metering_complete'])
check('cost within hard budget', cost['api_equivalent_usd']==.00308304 and cost['conservative_usd']==.0570972 and cost['conservative_usd']<3)
check('no model tools', not any(i['type'] in ['commandExecution','mcpToolCall','webSearch','fileChange'] for t in view['thread']['turns'] for i in t['items']))
check('controller stopped honestly at step 3 reserve', any(e['kind']=='stopped' and e['step']==3 and e['error']=='evidence reserve reached' for e in events))
check('no steps 4–7 claimed executed', not any(e['kind']=='action' and e['action'].get('step',0)>3 for e in events))
check('no host rejection fabricated', not any(e['event']=='hosted.rpc.rejected' for e in lines('run/taurhaus.log.jsonl')))
check('descriptor restoration observed', any(e['kind']=='descriptor_restored' and e['exit']==0 for e in events))
check('mesh source clean', not subprocess.check_output(['git','-C','/home/mstie/projects/mesh-push','status','--porcelain'],text=True))
cleanup = read('run/cleanup.json')
check('owned scratch destroyed', cleanup['survivors']==[] and cleanup['root_removed'] and not root.exists())
survivors=[]
for item in read('step1/step1-identities.json'):
    try:
        fields=Path(f'/proc/{item["pid"]}/stat').read_text().rsplit(')',1)[1].split()
        if fields[19]==item['start_ticks'] and fields[0]!='Z':survivors.append(item['pid'])
    except FileNotFoundError:pass
check('recorded PID/start identities absent', not survivors)
port=int(iso['environment']['TAURHAUS_DAEMON_PORT'])
with socket.socket() as probe:
    probe.settimeout(.2)
    check('private port closed', 20000<=port<=31999 and probe.connect_ex(('127.0.0.1',port))!=0)
check('private app socket absent', not Path(app['socketPath']).exists())
for name in ['check-quick','lint','test-contracts']:
    gate=read(f'gates/gate-{name}.json')
    check('gate '+name+' passed', gate['command']==['just',name] and gate['exit']==0)
check('gate cleanup', read('gates/gate-cleanup.json')['root_removed'])
def safe(v):
    if isinstance(v,dict):
        for k,x in v.items():
            if k in ['rate_limits','rateLimits'] or k=='method' and str(x).startswith('account/'):return False
            if ('installation' in k.lower() or k.lower() in ['auth','controlauthtokenhash','accountid','account_id','access_token','refresh_token','id_token','accesstoken','refreshtoken','idtoken']) and x!='<redacted>':return False
            if not safe(x):return False
    elif isinstance(v,list):return all(safe(x) for x in v)
    return True
for p in OUT.rglob('*'):
    if not p.is_file():continue
    s=p.read_text()
    check('no credentials/operator paths '+str(p.relative_to(OUT)), not re.search(r'eyJ[A-Za-z0-9_-]{20,}\.|sk-[A-Za-z0-9]{20,}|(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-push)(?:/|\b))',s))
    if p.suffix=='.json':check('safe JSON '+p.name,safe(json.loads(s)))
    elif p.suffix=='.jsonl':check('safe JSONL '+p.name,all(safe(json.loads(l)) for l in s.splitlines() if l))
size=sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())
size+=sum(p.stat().st_size for p in BASE.glob('attempt6*.py'))
check('all attempt6 sidecars below 1 MB',size<1000000)
(OUT/'final-audit.json').write_text(json.dumps({'checks':checks,'sidecar_bytes_before_audit':size,'surviving_pids':survivors,'outcome':'INCONCLUSIVE: controller reserve stopped the trial before steps 4–7'},indent=2)+'\n')
print(json.dumps({'passed':len(checks),'sidecar_bytes_before_audit':size}))
