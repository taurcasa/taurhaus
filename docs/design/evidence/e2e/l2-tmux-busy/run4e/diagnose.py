"""Read-only reconciliation of run 4e's stopped step-1 packet."""
import json
from pathlib import Path
from support import complete_rows
BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
aliases=json.loads((BASE/'export-manifest.json').read_text())['aliases']
def read(name):return json.loads((OUT/aliases.get(name,name)).read_text())
events=complete_rows((OUT/'events.jsonl').read_text())
journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
record=read('final-runtime.json');activity=read('final-activity.json')
alpha=next(r for r in read('final-runtime-sessions.json')['runtime_sessions'] if r.get('member_name')=='alpha')
card=record['recovery']['claim']['journal']
receipts=[r for r in journal if r.get('payload',{}).get('message_id')==card['message_id'] and r.get('event_type')=='receipt']
health=read('team/state/delivery/health-alpha.json')
assert record['terminalContract']==1 and record['attachmentGeneration']==1
assert record['session_id'] is None and record['jsonl_path'] is None
assert activity['activity_confidence']=='uncertain'
assert alpha['activity_attribution']=='none' and alpha['session_id'] is None
assert not receipts and 'activity not freshly idle' in health['last_defer_reason']
assert not list((OUT/'sessions').glob('rollout-*.jsonl'))
assert read('step1-outcome.json')['classification']=='taurhaus'
assert all(read(f'step{n}-outcome.json')['outcome']=='NOT RUN' for n in range(2,7))
cost=read('cost-ledger.json')
assert cost['paid_inputs']==0 and cost['api_equivalent_usd']==0 and not cost['usage_rows']
resumes=[e for e in events if e['kind']=='daemon_request' and e['request']['method']=='coordination.resume_member']
assert not resumes
locks=complete_rows((OUT/'terminal-locks.jsonl').read_text())
daemon=complete_rows((OUT/'taurhaus.log.jsonl').read_text())
requests=[e for e in events if e['kind']=='daemon_request' and e['request']['method']=='get_runtime_session_snapshot']
stopped=next(e for e in events if e['kind']=='stopped')
result={'exit':0,'verdict':'FAIL','classification':'taurhaus product','step':1,
 'readiness_observation_seconds':stopped['at']-requests[0]['at'],
 'runtime':record,'activity':activity,'alpha_snapshot':alpha,'onboarding':card,
 'onboarding_receipts':receipts,'pending_reason':health['last_defer_reason'],
 'native_session_inventory':read('codex-session-inventory.json'),
 'resume_calls':len(resumes),'paid_inputs':cost['paid_inputs'],'metered_usd':cost['api_equivalent_usd'],
 'lock_samples':len(locks),'flock_samples':sum(bool(r['holders']) for r in locks),
 'daemon_jsonl_rows':len(daemon),'controller':read('controller-exit.json'),'cleanup':read('cleanup.json')}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ('runtime','activity','alpha_snapshot','cleanup')}))
