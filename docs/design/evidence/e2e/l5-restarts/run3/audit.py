"""Read-only final assessment of this stopped attempt; never launches a seat."""
import datetime, hashlib, json, subprocess
from pathlib import Path
from support import clean, complete_rows, pending
B=Path(__file__).resolve().parent; P=B/'runtime'
def read(name):return json.loads((P/name).read_text())
def utc(stamp):return datetime.datetime.fromtimestamp(stamp,datetime.timezone.utc).isoformat()
events=complete_rows((P/'events.jsonl').read_text())
def event(kind):return next(r for r in events if r['kind']==kind)
journal=[r for f in (P/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(f.read_text())]
sample=event('pending_boundary_sample');stop=event('normal_daemon_stop');restart=event('post_daemon_restart')
cleanup=read('cleanup.json')
assert not cleanup['survivors'] and all(cleanup[k] for k in ['port_closed','root_removed','auth_removed','auth_copy_removed_before_root'])
old=stop['identity'];new=next(i for i in restart['identities'] if i['argv'][0].endswith('/taurhaus-daemon'))
assert (old['pid'],old['start_ticks'])!=(new['pid'],new['start_ticks'])
assert restart['ping']['protocol_version']==27
markers=[]
for label in ['baseline','taurhaus-backlog']:
 for seat in ['alpha','beta']:
  item=read(f'{label}-{seat}-message.json');mid=item['message_id']
  rows=[r for r in journal if r['payload'].get('message_id')==mid]
  exposures=[r for r in rows if r['payload'].get('stage') in ['submitted','native_enqueued']]
  accepted=next(r for r in rows if r['event_type']=='message_accepted')
  row={'label':label,'seat':seat,'message_id':mid,'delivery_id':item['delivery_targets'][0]['delivery_id'],
       'marker':item['marker'],'accepted_at':accepted['committed_at'],
       'transport_receipts':[{'at':r['committed_at'],'stage':r['payload']['stage']} for r in exposures],
       'consumed_by_read':sum(r['payload'].get('kind')=='consumed_by_read' for r in rows)}
  if label=='taurhaus-backlog':
   before=[r for r in rows if datetime.datetime.fromisoformat(r['committed_at']).timestamp()<stop['at']]
   row['pending_at_sample']=pending(sample['samples'][seat]['journal'],mid,seat,sample['samples'][seat]['activity'])
   row['transport_receipts_before_shutdown']=sum(r['payload'].get('stage') in ['submitted','consumed','native_enqueued'] for r in before)
   assert row['pending_at_sample'] and row['transport_receipts_before_shutdown']==0
  markers.append(row)
for n in range(4,7):
 (P/f'step{n}-outcome.json').write_text(json.dumps({'step':n,'outcome':'NOT RUN','classification':'blocked by step 3 harness failure','inputs':0,'metered_usd':0},indent=2)+'\n')
started=next(r['at'] for r in events if r['kind']=='daemon_request' and r['request']['method']=='coordination.initialize_team')
usage=read('usage-events.json');ledger=read('cost-ledger.json')
turns=[]
for tid in ledger['turn_ids']:
 rows=[r for r in usage if r['payload'].get('turn_id')==tid]
 gens=[g for g in ledger['generations'] if g['turn_id']==tid]
 begin=next((r['timestamp'] for r in rows if r['payload']['type']=='task_started'),None)
 end=next((r['timestamp'] for r in rows if r['payload']['type']=='task_complete'),None)
 turns.append({'turn_id':tid,'thread_id':rows[0]['thread_id'] if rows else 'notify-only session',
    'started':begin,'completed':end,'generation_count':len(gens),
    'metered_usd':round(sum(g['api_equivalent_usd'] for g in gens),9) if gens else None,
    'duration_seconds':(datetime.datetime.fromisoformat(end)-datetime.datetime.fromisoformat(begin)).total_seconds() if begin and end else None})
result={'verdict':'UNAVAILABLE — step 3 harness failure after real Taurhaus restart',
 'steps':[
  {'step':1,'outcome':'PASS','classification':'runtime','detail':'Both baseline replies, transport receipts and explicit reads.'},
  {'step':2,'outcome':'PARTIAL','classification':'runtime pending PASS; harness duration deviation','detail':'Both accepted/unexposed/working at the sample; no exposure before shutdown. Prompt requested >=30 seconds but alpha completed 400 lines in 17.492 seconds; beta interrupted by shutdown.'},
  {'step':3,'outcome':'FAIL','classification':'harness','detail':'Real PID/start-tick transition and protocol 27 observed; cached identities.json reused in checkpoint assertion. stopped:true hosted result incorrectly labeled readable; no resume_member called.'},
  *[read(f'step{n}-outcome.json') for n in range(4,7)]],
 'started_utc':utc(started),'cleanup_utc':utc(event('cleanup')['at']),
 'runtime_seconds':event('cleanup')['at']-started,
 'sample_to_shutdown_ms':(stop['at']-sample['at'])*1000,
 'pending_samples':sample['samples'],'old_daemon':old,'new_daemon':new,
 'restart_ping':restart['ping'],'markers':markers,'turns':turns,'spend':ledger,
 'transient_refusals':[r for r in events if r['kind']=='transient_refusal'],
 'cleanup':cleanup,'controller_exit':read('controller-exit.json')['exit'],
 'mesh_restart_commands':sum(r['kind']=='action' and r.get('action',{}).get('argv',[])[:2]==['team-daemon','restart-self'] for r in events),
 'resume_member_commands':sum(r['kind']=='daemon_request' and r['request']['method']=='coordination.resume_member' for r in events),
 'daemon_jsonl':{'rows':len(complete_rows((P/'taurhaus.log.jsonl').read_text())),
   'sha256':hashlib.sha256((P/'taurhaus.log.jsonl').read_bytes()).hexdigest()},
 'candidate':{'taurhaus_product_commit':'a7e6db7e','mesh_commit':'310144d','protocol':27,'codex':'0.153.4',
   'model':'gpt-5.6-luna','effort':'low','descriptor':'shipped enabled, unchanged',
   'binaries':[r for r in events if r['kind']=='binary']},
 'gates':{n:json.loads((B/f'gates/gate-{n}.json').read_text()) for n in ['check-quick','lint','test-contracts']},
 'gate_cleanup':json.loads((B/'gates/gate-cleanup.json').read_text()),
 'independent_opus_review':'unavailable in session tools; outstanding with orchestrator'}
assert result['runtime_seconds']<900 and ledger['paid_inputs']<=20 and ledger['api_equivalent_usd']<=.30
assert result['mesh_restart_commands']==result['resume_member_commands']==0
(B/'final-audit.json').write_text(json.dumps(clean(result),indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['verdict','runtime_seconds','sample_to_shutdown_ms','daemon_jsonl','controller_exit']}))
