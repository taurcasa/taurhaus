"""Post-teardown classification and bounded accounting; never runs a product command."""
import datetime,json
from pathlib import Path
from support import complete_rows, owner_window_evidence, read_by_seat
P=Path(__file__).resolve().parent/'runtime'
def classify_step5(outcome, accounting):
    """Correct observer failures without inferring lost mail from a missing receipt."""
    result=dict(outcome)
    if outcome['outcome']!='FAIL':
        return result
    markers=[a for a in accounting if a['label']=='mesh-backlog']
    duplicates=[a for a in markers if a['accepted_target_count']>1 or a['transport_count']>1]
    result['observer_reason']=outcome.get('observer_reason',outcome['reason'])
    if duplicates:
        result.update(classification='product (mesh)',
            reason='Duplicate accepted target or transport exposure: '+', '.join(a['seat'] for a in duplicates),
            limitation='Counts establish duplicate accounting in this bounded run; attempt retries alone are not duplicate exposure.')
    elif (len(markers)==2 and {a['seat'] for a in markers}=={'alpha','beta'}
          and all(a['accepted_target_count']==a['transport_count']==1 for a in markers)):
        result.update(classification='harness',
            reason='Observer predicate failed despite one accepted target and one transport exposure per seat; pending owner retries are not duplicate exposure.',
            limitation='No duplicate exposure or lost obligation established. Native witnesses, owner census and explicit step-6 reads remain separate requirements; no retrospective PASS.')
    else:
        result.update(classification='harness',
            reason='Incomplete accepted-target or transport accounting; delivery remains unproved.',
            limitation='Missing evidence alone does not establish a lost obligation or a Mesh product defect.')
    return result


def read(name):return json.loads((P/name).read_text())

def main():
 assert (P/'cleanup.json').exists()
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
   outcome=classify_step5(outcome,accounting)
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

if __name__=='__main__':main()
