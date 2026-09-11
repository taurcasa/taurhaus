"""Assess run4's stopped packet offline; no CLI, credentials or model inputs."""
import datetime
import hashlib
import json
from pathlib import Path
from support import clean, complete_rows
from pack import unpack
B=Path(__file__).resolve().parent
P=B/'runtime'
packed=unpack(json.loads((P/'snapshots.json').read_text())) if (P/'snapshots.json').exists() else {}
def text(name):return (P/name).read_text() if (P/name).exists() else packed[name]
def read(name):return json.loads(text(name))
def seconds(stamp):return datetime.datetime.fromisoformat(stamp).timestamp()
events=complete_rows((P/'events.jsonl').read_text())
cleanup=read('cleanup.json')
assert not cleanup['survivors'] and all(cleanup[k] for k in ['port_closed','root_removed','auth_removed','auth_copy_removed_before_root'])
assert read('controller-exit.json')['exit']==1
recorded=read('step1-outcome.json')
assert recorded['outcome']=='FAIL' and 'delivery receipt/read missing baseline alpha' in recorded['reason']
steps=[dict(recorded,classification='harness',reason='Baseline raced alpha startup inbox read: explicit consumption and model reply exist, but no transport submission for the baseline ID. '+recorded['reason'])]
for n in range(2,7):
    outcome={'step':n,'outcome':'NOT RUN','classification':'blocked by step 1 harness failure','paid_inputs':0,'metered_usd':0}
    (P/f'step{n}-outcome.json').write_text(json.dumps(outcome,indent=2)+'\n')
    steps.append(outcome)
journal_names=[str(p.relative_to(P)) for p in (P/'team/state/messaging-v2/segments').glob('*.jsonl')] or [n for n in packed if n.startswith('team/state/messaging-v2/segments/') and n.endswith('.jsonl')]
journal=[r for name in journal_names for r in complete_rows(text(name))]
message=read('baseline-alpha-message.json')
matching=[r for r in journal if r['payload'].get('message_id')==message['message_id']]
assert len([r for r in matching if r['event_type']=='message_accepted'])==1
assert len([r for r in matching if r['payload'].get('kind')=='consumed_by_read'])==1
assert not any(r['payload'].get('stage') in ['submitted','native_enqueued'] for r in matching)
identities={s:read(f'team/runtime/{s}.json') for s in ['alpha','beta']}
rollout=read('rollout-items.json')
alpha=[r for r in rollout if r['thread_id']==identities['alpha']['session_id']]
reply=[r for r in alpha if r['payload'].get('role')=='assistant' and message['marker'] in json.dumps(r['payload'])]
assert reply
read_tool=[r for r in alpha if r['payload'].get('type')=='custom_tool_call' and 'mesh read --unread --mark-read' in r['payload'].get('input','')]
assert read_tool
ledger=read('cost-ledger.json');usage=read('usage-events.json');turns=[]
for tid in ledger['turn_ids']:
    rows=[r for r in usage if r['payload'].get('turn_id')==tid]
    generations=[g for g in ledger['generations'] if g['turn_id']==tid]
    start=next((r['timestamp'] for r in rows if r['payload']['type']=='task_started'),None)
    end=next((r['timestamp'] for r in rows if r['payload']['type']=='task_complete'),None)
    turns.append({'turn_id':tid,'thread_id':rows[0]['thread_id'] if rows else 'notify-only session',
        'started':start,'completed':end,'duration_seconds':round(seconds(end)-seconds(start),3) if start and end else None,
        'generations':generations,'metered_usd':round(sum(g['api_equivalent_usd'] for g in generations),9) if generations else None})
start=next(r['at'] for r in events if r['kind']=='daemon_request' and r['request']['method']=='coordination.initialize_team')
end=next(r['at'] for r in events if r['kind']=='cleanup')
assert end-start<900 and ledger['paid_inputs']<=20 and ledger['api_equivalent_usd']<=.30
assert not any(r['kind'] in ['normal_daemon_stop','normal_mesh_restart_initiated','pending_boundary_sample'] for r in events)
owners=complete_rows((P/'owner-observations.jsonl').read_text())
maximum=max(len(r.get('owners',[])) for r in owners)
assert maximum==1
result={'verdict':'FAIL step 1 (harness baseline/startup race); steps 2–6 NOT RUN',
 'step_outcomes':steps,'recorded_step1_outcome':recorded,
 'started_utc':datetime.datetime.fromtimestamp(start,datetime.timezone.utc).isoformat(),
 'cleanup_utc':datetime.datetime.fromtimestamp(end,datetime.timezone.utc).isoformat(),
 'runtime_seconds':end-start,'baseline':{'message':message,'journal':matching,'native_read':read_tool,'native_reply':reply,
 'exposures':0,'explicit_reads':1,'beta_baseline_sent':False},
 'identities':identities,'config':read('team/config.json'),'owner_epoch':read('team/state/delivery/epoch.json'),
 'owner_census':{'samples':len(owners),'max_simultaneous_observed_owners':maximum,
 'max_gap_seconds':max(b['at']-a['at'] for a,b in zip(owners,owners[1:])),
 'restart_window':'NOT RUN; startup-only census does not prove restart exclusion'},
 'working_windows':{'taurhaus_boundary':{'alpha':'NOT RUN','beta':'NOT RUN'},'mesh_boundary':{'alpha':'NOT RUN','beta':'NOT RUN'}},
 'turns':turns,'spend':ledger,'cleanup':cleanup,'controller_exit':1,
 'daemon_jsonl':{'rows':len(complete_rows((P/'taurhaus.log.jsonl').read_text())),
 'sha256':hashlib.sha256((P/'taurhaus.log.jsonl').read_bytes()).hexdigest()},
 'transient_refusals':[r for r in events if r['kind']=='transient_refusal'],
 'candidate':dict(json.loads((B/'candidate.json').read_text()),binaries=[r for r in events if r['kind']=='binary']),
 'gates':{n:json.loads((B/f'gates/gate-{n}.json').read_text()) for n in ['check-quick','lint','test-contracts']},
 'gate_cleanup':json.loads((B/'gates/gate-cleanup.json').read_text()),
 'deviations':['Step 1 stopped at its required 120-second polling deadline; no automatic model retry or restart after failure.',
 'The referenced integration attempt9 checkout was absent (git show exit 128); reused retained run3 continuation and read messaging run2 source.',
 'One notify-only input has unknown cost, counted as one input; metered cap verified, complete billed spend unknown.',
 'Inherited execute.py exited 0 after handling the failed child; step 1 and controller exits are both 1 and govern the verdict.',
 'Independent Opus evidence review remains with the invoking orchestrator.'],
 'product_defect':'None established; a startup inbox read consumed the baseline before a separate transport submission.'}
(B/'final-audit.json').write_text(json.dumps(clean(result),indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['verdict','runtime_seconds','daemon_jsonl']}))
