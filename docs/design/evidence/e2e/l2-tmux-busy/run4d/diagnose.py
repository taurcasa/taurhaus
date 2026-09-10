"""Read-only reconciliation of this frozen evidence packet; no runtime calls."""
import json
from datetime import datetime
from pathlib import Path
from support import complete_rows, reply_evidence, pending_observation
BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
aliases=json.loads((BASE/'export-manifest.json').read_text())['aliases']
def read(name):return json.loads((OUT/aliases.get(name,name)).read_text())
def seconds(value):return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()
journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
events=complete_rows((OUT/'events.jsonl').read_text())
rollout=[r for p in (OUT/'sessions').glob('*.jsonl') for r in complete_rows(p.read_text())]
messages={}
for label in ('Q','Q2'):
    accepted=read(label+'-accepted.json');mid=accepted['message_id']
    rows=[r for r in journal if r.get('payload',{}).get('message_id')==mid]
    pending=read('pending/'+mid+'.json')
    preceding=[r for r in journal if seconds(r['committed_at'])<=pending['at']]
    facts=pending_observation(preceding,mid,{},activity=pending['activity'],now=pending['at'])
    assert facts and facts['source']=='message accepted without receipt while working'
    submitted=[r for r in rows if r.get('payload',{}).get('stage')=='submitted']
    messages[label]={'id':mid,'marker':accepted['marker'],'accepted_at':next(r['committed_at'] for r in rows if r['event_type']=='message_accepted'),
        'pending_at':pending['at'],'pending_facts_replayed':facts,'submitted_count':len(submitted),'receipt_stages':[r['payload'].get('stage',r['payload'].get('kind')) for r in rows if r['event_type']=='receipt'],
        'reply':reply_evidence(journal,rollout,mid,accepted['marker'])}
    if submitted:
        observation=next(json.loads(p.read_text()) for p in (OUT/'submissions').glob('*.json') if json.loads(p.read_text())['receipt']['payload']['message_id']==mid)
        at=seconds(submitted[0]['payload']['observed_at'])
        messages[label].update({'submitted_at':submitted[0]['payload']['observed_at'],'delay_seconds':at-seconds(messages[label]['accepted_at']),
            'idle_age_seconds':at-seconds(observation['activity']['observed_at']),'attachment':submitted[0]['payload']['attachment']})
assert messages['Q']['submitted_count']==1 and messages['Q']['reply']
assert messages['Q2']['submitted_count']==0
stop=next(e for e in events if e['kind']=='daemon_request' and e['request']['method']=='stop_session')
locks=complete_rows((OUT/'terminal-locks.jsonl').read_text())
stop_locks=[r for r in locks if r['at']>=stop['at'] and r.get('holders')]
assert stop_locks
cost=read('cost-ledger.json')
unknown=[t for t in cost['turns'] if t['usd'] is None and t.get('source')!='notify-only']
assert len(unknown)==1 and unknown[0]['completed']
assert not any(e['kind']=='daemon_request' and e['request']['method']=='coordination.resume_member' for e in events)
result={'exit':0,'steps':{str(n):read(f'step{n}-outcome.json') for n in range(1,7)},'messages':messages,
    'metered_usd':cost['api_equivalent_usd'],'unmetered_model_turns':unknown,'usage_increments':cost['usage_rows'],
    'abort_rows':[r for r in rollout if r.get('payload',{}).get('type')=='turn_aborted'],
    'resume_calls':0,'context_compactions':sum(r.get('payload',{}).get('type')=='context_compaction' for r in rollout),
    'lock_samples':len(locks),'flock_samples':sum(bool(r.get('holders')) for r in locks),'managed_stop_lock_samples':stop_locks,
    'daemon_jsonl_rows':len(complete_rows((OUT/'taurhaus.log.jsonl').read_text())),'cleanup':read('cleanup.json'),'controller':read('controller-exit.json')}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'exit':0,'Q_delay_seconds':messages['Q']['delay_seconds'],'Q_idle_age_seconds':messages['Q']['idle_age_seconds'],'resume_calls':0,'metered_usd':cost['api_equivalent_usd']}))
