"""Post-teardown classification and bounded accounting; never runs a product command."""
import datetime,json
from pathlib import Path
from support import complete_rows, owner_window_evidence, read_by_seat
P=Path(__file__).resolve().parent/'runtime'
assert (P/'cleanup.json').exists()
def read(name):return json.loads((P/name).read_text())
events=complete_rows((P/'events.jsonl').read_text())
rows=[r for p in (P/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
accounting=[]
for label in ['baseline','taurhaus-backlog','mesh-backlog']:
 for seat in ['alpha','beta']:
  file=P/f'{label}-{seat}-message.json'
  if not file.exists():continue
  item=read(file.name);mid=item['message_id'];matching=[r for r in rows if r.get('payload',{}).get('message_id')==mid]
  accepted=[r for r in matching if r['event_type']=='message_accepted'];rs=[r for r in matching if r['event_type']!='message_accepted']
  accounting.append({'label':label,'seat':seat,'message_id':mid,'marker':item['marker'],'accepted':accepted,'receipts':rs,
   'accepted_target_count':sum(t['recipient']==seat for a in accepted for t in a['payload']['delivery_targets']),
   'attempt_count':sum(r['payload'].get('stage')=='attempt_started' for r in rs),
   'transport_count':sum(r['payload'].get('stage') in ['submitted','native_enqueued'] for r in rs),
   'read_observed':read_by_seat([r['payload'] for r in rs],seat)})
(P/'observed-obligation-accounting.json').write_text(json.dumps(accounting,indent=2)+'\n')
if (P/'step5-outcome.json').exists():
 outcome=read('step5-outcome.json')
 if outcome['outcome']=='FAIL':
  duplicate_attempts=[a for a in accounting if a['label']=='mesh-backlog' and a['attempt_count']!=1]
  if duplicate_attempts:
   outcome.update(observer_reason=outcome['reason'],classification='product (mesh)',
    reason='Step-5 one-attempt-per-id criterion failed: '+', '.join(f"{a['seat']} has {a['attempt_count']} attempt_started rows and {a['transport_count']} transport receipts" for a in duplicate_attempts),
    limitation='Pending pre-input thread_active refusals were retried by Mesh. One eventual native exposure is observed; this is not duplicate model delivery or lost mail.')
   (P/'step5-outcome.json').write_text(json.dumps(outcome,indent=2)+'\n')
 restart=next((r for r in events if r['kind']=='normal_mesh_restart_initiated'),None)
 if restart:
  observations=complete_rows((P/'owner-observations.jsonl').read_text())
  start=restart['at'];end=outcome['at'];old=next(p['pid'] for p in read('step5-before-identity.json')['processes'] if 'team-daemon' in p['argv'] and 'start' in p['argv'])
  after=[o for o in observations if o['at']>start and o.get('owners') and o['owners'][0]['pid']!=old]
  deliveries=[datetime.datetime.fromisoformat(r['committed_at']).timestamp() for a in accounting if a['label']=='mesh-backlog' for r in a['receipts'] if r['payload'].get('stage') in ['submitted','native_enqueued']]
  if after and deliveries:
   census=owner_window_evidence(observations,start,end,old,after[0]['owners'][0]['pid'],min(deliveries))
   (P/'step5-owner-census.json').write_text(json.dumps(census,indent=2)+'\n')
print(json.dumps([{k:a[k] for k in ['label','seat','attempt_count','transport_count','read_observed']} for a in accounting]))
