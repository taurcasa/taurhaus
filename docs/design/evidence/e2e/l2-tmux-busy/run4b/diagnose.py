"""Read-only reconciliation of the stopped run; never launches a CLI."""
import json
from pathlib import Path
from support import complete_rows, onboarding_delivered

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
read=lambda name:json.loads((OUT/name).read_text())
events=complete_rows((OUT/'events.jsonl').read_text())
ready=read('step1-ready.json')
step1=read('step1-outcome.json')
card=onboarding_delivered(ready['runtime'],ready['activity'],ready['attribution']['snapshot'],ready['journal'],step1['at'])
assert card
journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
submissions=[r for r in journal if r.get('payload',{}).get('message_id')==card and r.get('payload',{}).get('stage')=='submitted']
reads=[r for r in journal if r.get('payload',{}).get('message_id')==card and r.get('payload',{}).get('kind')=='consumed_by_read']
turns=[r for p in (OUT/'sessions').glob('*.jsonl') for r in complete_rows(p.read_text()) if r.get('type')=='event_msg' and r.get('payload',{}).get('type')=='task_started']
ordinary=next(r for r in events if r['kind']=='command' and 'send-keys' in r['argv'] and 'For this bounded ordinary response' in ' '.join(r['argv']))
subsequent=[r for r in events if r['at']>=ordinary['at'] and r['kind']=='activity']
assert not any(r['value'].get('activity_confidence') in ('active','likely_working') for r in subsequent)
assert not any(r['kind']=='mesh_command' and 'send' in r['argv'] for r in events)
assert '› For this bounded ordinary response' in (OUT/'final-pane-2.txt').read_text()
assert len(turns)==1 and len(submissions)==1 and reads
ledger=read('cost-ledger.json')
assert ledger['paid_inputs']==2 and ledger['metering_complete'] and ledger['api_equivalent_usd']<=.20
locks=complete_rows((OUT/'terminal-locks.jsonl').read_text())
result={
    'outcome':'UNAVAILABLE / INCOMPLETE','classification':'harness','failed_step':2,
    'reason':'Ordinary response remained in Codex composer after literal send-keys and Enter; no second rollout turn or production busy observation within 90 seconds. Q was never sent. Exact input-handling cause is unproved; no product defect established.',
    'step1_onboarding_message_id':card,'step1_submission_count':len(submissions),'step1_explicit_read_count':len(reads),
    'step1_attribution':ready['attribution']['alpha'],
    'ordinary_input_command':ordinary['argv'],'ordinary_input_at':ordinary['at'],
    'rollout_turn_ids':[r['payload']['turn_id'] for r in turns],
    'controller_exit':read('controller-exit.json'),
    'spend':ledger,'daemon_jsonl_rows':len(complete_rows((OUT/'taurhaus.log.jsonl').read_text())),
    'lock_samples':len(locks),'flock_samples':sum(bool(r.get('holders')) for r in locks),
    'managed_stop_exclusion':'NOT RUN; startup samples do not establish lifecycle exclusion',
    'steps':{str(i):read(f'step{i}-outcome.json') for i in range(1,7)},
    'opus_review':'unavailable: no callable Workflow/Opus tool or Opus model override; no substitute approval',
    'paid_retries':0,'product_changes':False,
}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'exit':0,'failed_step':2,'classification':'harness','step1':'PASS','metered_usd':ledger['api_equivalent_usd']}))
