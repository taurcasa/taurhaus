"""Unpaid, read-only attempt-4 evidence/cleanup audit; invokes no harness."""
import json, re, socket, subprocess
from pathlib import Path

checkout = Path.cwd()
base = Path(__file__).resolve().parent
out = base / 'attempt4'
run = out / 'run'
events = [json.loads(line) for line in (run/'events.jsonl').read_text().splitlines()]
isolation = next(row for row in events if row['kind']=='isolation')
root = Path(isolation['root'])
port = int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999 and port != 17233
assert 'TMUX' not in isolation['environment']
assert isolation['initial_codex_entries']==['auth.json']
for key in ['HOME','CODEX_HOME','TAURHAUS_DATA_DIR','TAURHAUS_CLAUDE_DIR','TMUX_TMPDIR','CLAUDE_CONFIG_DIR','GROK_HOME','TAURHAUS_AGY_DIR']:
    assert Path(isolation['environment'][key]).is_relative_to(root)
request = next(row['request']['params']['request'] for row in events if row['kind']=='daemon_request' and row['request']['method']=='coordination.initialize_team')
assert len(request['agents'])==1
assert request['agents'][0]['delivery']=='app_server'
assert request['agents'][0]['model']=='gpt-5.6-luna'
assert request['agents'][0]['reasoning_effort']=='low'
assert request['lead']['cli_tool']=='claude'
policy = json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',(checkout/'src/lib/components/meshTabUtils.js').read_text(),re.S)[1])
assert request['messaging']=={'mode':'canonical','retentionPolicy':policy}
report=json.loads((run/'initialize-result.json').read_text())['outcome']['report']
assert report['failed_step']=='launch_host'
assert report['message']=='Conflict: unsupported app-server handshake'
assert not any(row['kind'] in ['action','inspection_ready'] for row in events)
logs=[json.loads(line) for line in (run/'taurhaus.log.jsonl').read_text().splitlines()]
assert not any(row['event']=='hosted.rpc.rejected' for row in logs)
assert not any(row['event']=='hosted.instruction_sources.loaded' for row in logs)
host=[json.loads(line) for line in (run/'host-events.jsonl').read_text().splitlines()]
assert host[0]['response'].startswith('HTTP/1.1 101')
assert host[1]['message']['params']['clientInfo']['name']=='trial_observer'
assert not any(row.get('message',{}).get('method') in ['thread/started','turn/started','thread/tokenUsage/updated'] for row in host)
ledger=json.loads((run/'cost-ledger.json').read_text())
assert ledger['paid_inputs']==0 and ledger['api_equivalent_usd']==0 and ledger['conservative_usd']==0
assert ledger['generations']==ledger['rollout_turn_ids']==[]
assert json.loads((run/'usage-events.json').read_text())==[]
assert not root.exists()
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1',port))!=0
survivors=[]
identities=json.loads((run/'identities.json').read_text())
for item in identities:
    try:
        stat=Path('/proc',str(item['pid']),'stat').read_text().rsplit(')',1)[1].split()
        if stat[19]==item['start_ticks']:survivors.append(item['pid'])
    except (FileNotFoundError,ProcessLookupError):pass
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit():continue
    try:
        if b'TAURHAUS_TRIAL_ID='+root.name.encode()+b'\0' in (proc/'environ').read_bytes():survivors.append(int(proc.name))
    except (FileNotFoundError,ProcessLookupError,PermissionError):pass
assert not survivors
mesh='/home/mstie/projects/mesh-push'
assert subprocess.check_output(['git','-C',mesh,'status','--porcelain'],text=True)==''
assert subprocess.check_output(['git','-C',mesh,'branch','--show-current'],text=True).strip()=='feat/native-push'
assert subprocess.check_output(['git','branch','--show-current'],text=True).strip()=='feat/integration-trial'
subprocess.run(['git','merge-base','--is-ancestor','6f61f611','HEAD'],check=True)
assert not subprocess.check_output(['git','diff','--name-only','--','src-tauri/','src/'],text=True)
assert next(row for row in events if row['kind']=='descriptor_restored')['exit']==0
gates={}
for name in ['check-quick','lint','test-contracts']:
    result=json.loads((out/'gates'/f'gate-{name}.json').read_text())
    assert result['exit']==0,result
    gates[name]=result['exit']
assert json.loads((out/'gates/gate-cleanup.json').read_text())['root_removed']
for path in list(out.rglob('*'))+[base/f'attempt4-{name}.py' for name in ['controller','observer','audit']]:
    if not path.is_file():continue
    assert path.name not in ['auth.json','daemon.token']
    text=path.read_text()
    assert not re.search(r'\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.',text),path
    assert not re.search(r'\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}',text),path
    for allowed in [str(checkout),mesh]:text=text.replace(allowed,'<checkout>')
    assert str(Path.home())+'/' not in text,path
result={'trial_verdict':'INCONCLUSIVE: observer initialization confounded step 1',
        'runtime_refusal':report['message'],'hosted_rpc_rejected_present':False,
        'host_error_object_present':False,'steps_2_through_7':'NOT RUN',
        'turns':0,'observed_usd':0,'recorded_identities_absent':len(identities),
        'survivors':survivors,'port_closed':port,'private_tmux_socket_absent':True,
        'root_and_auth_removed':True,'descriptor_restored':True,'mesh_worktree_clean':True,
        'gates':gates,'product_diff':[],'artifact_hygiene':'passed',
        'review':'local read-only audit; Opus model unavailable'}
(out/'final-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
