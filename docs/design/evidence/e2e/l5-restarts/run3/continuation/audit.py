"""Assess the stopped runtime packet without launching a CLI or reading credentials."""
import datetime,hashlib,json
from pathlib import Path
from support import complete_rows,pending,identity_preserved,clean
B=Path(__file__).resolve().parent;P=B/'runtime'
def read(name):return json.loads((P/name).read_text())
def seconds(stamp):return datetime.datetime.fromisoformat(stamp).timestamp()
events=complete_rows((P/'events.jsonl').read_text())
def event(kind):return next(r for r in events if r['kind']==kind)
journal=[r for p in (P/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
steps=[read(f'step{n}-outcome.json') for n in range(1,7)]
assert all(s['outcome']=='PASS' for s in steps)
cleanup=read('cleanup.json');assert not cleanup['survivors'] and all(cleanup[k] for k in ['port_closed','root_removed','auth_removed','auth_copy_removed_before_root'])
assert read('controller-exit.json')['exit']==0
before=read('step1-identity.json');final=read('step6-identity.json')
assert before['config']['team_incarnation_id']==final['config']['team_incarnation_id']
for seat in ['alpha','beta']:assert identity_preserved(before[seat],final[seat])
markers=[]
for label in ['baseline','taurhaus-backlog','mesh-backlog']:
 for seat in ['alpha','beta']:
  item=read(f'{label}-{seat}-message.json');mid=item['message_id']
  rows=[r for r in journal if r['payload'].get('message_id')==mid]
  exposures=[r for r in rows if r['payload'].get('stage') in ['submitted','native_enqueued']]
  reads=[r for r in rows if r['payload'].get('kind')=='consumed_by_read']
  assert len(exposures)==1 and reads,(label,seat)
  entry={'label':label,'seat':seat,'message_id':mid,'delivery_id':item['delivery_targets'][0]['delivery_id'],
   'marker':item['marker'],'accepted_at':next(r['committed_at'] for r in rows if r['event_type']=='message_accepted'),
   'receipt':{'at':exposures[0]['committed_at'],'stage':exposures[0]['payload']['stage'],'owner_epoch':exposures[0]['payload']['owner_epoch']},
   'explicit_reads':len(reads)}
  if label!='baseline':
   sample=read(f'{label}-{seat}-pending.json')
   assert pending(sample['journal'],mid,seat,sample['activity'])
   boundary=event('normal_daemon_stop' if label=='taurhaus-backlog' else 'normal_mesh_restart_initiated')
   assert seconds(exposures[0]['committed_at'])>boundary['at'],entry
   entry['pending_sample']=sample
  markers.append(entry)
stop=event('normal_daemon_stop');restart=event('post_daemon_restart')
old=stop['identity'];new=next(i for i in restart['identities'] if i['argv'][0].endswith('/taurhaus-daemon') and '--port' in i['argv'])
assert (old['pid'],old['start_ticks'])!=(new['pid'],new['start_ticks']) and restart['ping']['protocol_version']==27
assert final['epoch']['epoch']>before['epoch']['epoch'] and final['epoch']['pid']!=before['epoch']['pid']
owners=complete_rows((P/'owner-observations.jsonl').read_text());assert all(len(o.get('owners',[]))<=1 for o in owners)
usage=read('usage-events.json');ledger=read('cost-ledger.json');turns=[]
for tid in ledger['turn_ids']:
 rows=[r for r in usage if r['payload'].get('turn_id')==tid]
 gens=[g for g in ledger['generations'] if g['turn_id']==tid]
 start=next((r['timestamp'] for r in rows if r['payload']['type']=='task_started'),None)
 end=next((r['timestamp'] for r in rows if r['payload']['type']=='task_complete'),None)
 turns.append({'turn_id':tid,'thread_id':rows[0]['thread_id'] if rows else 'notify-only session',
  'started':start,'completed':end,'duration_seconds':round(seconds(end)-seconds(start),3) if start and end else None,
  'generations':gens,'metered_usd':round(sum(g['api_equivalent_usd'] for g in gens),9) if gens else None})
start=next(r['at'] for r in events if r['kind']=='daemon_request' and r['request']['method']=='coordination.initialize_team')
stop_time=event('cleanup')['at'];assert stop_time-start<900
assert ledger['paid_inputs']<=20 and ledger['api_equivalent_usd']<=.30
samples=[r for r in events if r['kind']=='pending_boundary_sample'];assert len(samples)==2 and all(r['coverage']['pending_seats']==['alpha','beta'] for r in samples)
intervals={r['label']:(event('normal_daemon_stop' if r['label']=='taurhaus-backlog' else 'normal_mesh_restart_initiated')['at']-r['at'])*1000 for r in samples}
result={'verdict':'INCOMPLETE: six runtime checks PASS; first-boundary duration unproved and Opus review outstanding',
 'step_outcomes':steps,'started_utc':datetime.datetime.fromtimestamp(start,datetime.timezone.utc).isoformat(),
 'cleanup_utc':datetime.datetime.fromtimestamp(stop_time,datetime.timezone.utc).isoformat(),'runtime_seconds':stop_time-start,
 'markers':markers,'sample_to_restart_ms':intervals,'old_daemon':old,'new_daemon':new,'protocol':27,
 'before_identity':before,'final_identity':final,'owner_samples':len(owners),'max_simultaneous_observed_owners':max(len(o.get('owners',[])) for o in owners),
 'turns':turns,'spend':ledger,'cleanup':cleanup,'controller_exit':0,
 'daemon_jsonl':{'rows':len(complete_rows((P/'taurhaus.log.jsonl').read_text())), 'sha256':hashlib.sha256((P/'taurhaus.log.jsonl').read_bytes()).hexdigest()},
 'resume_operations':[r for r in events if r['kind']=='daemon_request' and r['request']['method']=='coordination.resume_member'],
 'transient_refusals':[r for r in events if r['kind']=='transient_refusal'],
 'candidate':{'product_commit':'a7e6db7e','mesh_commit':'310144d','descriptor':'enabled, unchanged','codex':'0.153.4','model':'gpt-5.6-luna','effort':'low','binaries':[r for r in events if r['kind']=='binary']},
 'gates':{n:json.loads((B/f'gates/gate-{n}.json').read_text()) for n in ['check-quick','lint','test-contracts']},
 'gate_cleanup':json.loads((B/'gates/gate-cleanup.json').read_text()),
 'limitations':['First paced task selected unavailable python; minimum 30 seconds at first boundary unproved. Second tasks used python3 and completed after 44.955/45.649 seconds.', 'Pacing used an ordinary Python task, rather than free-form model streaming.', 'Two unknown-cost inputs; metered cap verified only.', 'Independent Opus evidence lens unavailable; review outstanding.']}
(B/'final-audit.json').write_text(json.dumps(clean(result),indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['verdict','runtime_seconds','sample_to_restart_ms','daemon_jsonl']}))
