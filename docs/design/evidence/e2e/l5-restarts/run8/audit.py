"""Read-only assessment of this attempt; no historical runtime values reused."""
import datetime
import hashlib
import json
from pathlib import Path
from support import clean, complete_rows
from pack import unpack
B=Path(__file__).resolve().parent;P=B/'runtime'
packet=unpack(json.loads((P/'snapshots.json').read_text())) if (P/'snapshots.json').exists() else {}
def text(name):return (P/name).read_text() if (P/name).exists() else packet[name]
def read(name,default=None):
 try:return json.loads(text(name))
 except (FileNotFoundError,KeyError):return default
def seconds(stamp):return datetime.datetime.fromisoformat(stamp).timestamp()
events=complete_rows(text('events.jsonl'));cleanup=read('cleanup.json');ledger=read('cost-ledger.json')
assert not cleanup['survivors'] and all(cleanup[k] for k in ['port_closed','root_removed','auth_removed','auth_copy_removed_before_root'])
steps=[]
for n in range(1,7):
 outcome=read(f'step{n}-outcome.json')
 if outcome is None:
  stop=next((e for e in events if e['kind']=='stopped' and e.get('step')==n),None)
  outcome={'step':n,'outcome':'FAIL' if stop else 'NOT RUN','classification':'harness setup' if stop else 'blocked by earlier failure','reason':stop.get('error') if stop else None}
  (P/f'step{n}-outcome.json').write_text(json.dumps(outcome,indent=2)+'\n')
 steps.append(outcome)
usage=read('usage-events.json',[]);turns=[]
for tid in ledger['turn_ids']:
 rows=[r for r in usage if r['payload'].get('turn_id')==tid]
 generations=[g for g in ledger['generations'] if g['turn_id']==tid]
 start=next((r['timestamp'] for r in rows if r['payload']['type']=='task_started'),None)
 end=next((r['timestamp'] for r in rows if r['payload']['type']=='task_complete'),None)
 turns.append({'turn_id':tid,'thread_id':rows[0]['thread_id'] if rows else 'notify-only session','started':start,'completed':end,
  'duration_seconds':round(seconds(end)-seconds(start),3) if start and end else None,
  'generations':generations,'metered_usd':round(sum(g['api_equivalent_usd'] for g in generations),9) if generations else None})
start=next(r['at'] for r in events if r['kind']=='warmup_started')
end=next(r['at'] for r in events if r['kind']=='cleanup')
assert end-start<900 and ledger['paid_inputs']<=20 and ledger['api_equivalent_usd']<=.30
failed=next((s for s in steps if s['outcome']=='FAIL'),None)
result={'verdict':f"FAIL step {failed['step']} ({failed['classification']}); later steps NOT RUN" if failed else 'Six runtime steps completed; independent review pending',
 'step_outcomes':steps,'runtime_seconds':end-start,'started_utc':datetime.datetime.fromtimestamp(start,datetime.timezone.utc).isoformat(),
 'cleanup_utc':datetime.datetime.fromtimestamp(end,datetime.timezone.utc).isoformat(),
 'restart_boundaries':[r for r in events if r['kind'] in ['normal_daemon_stop','post_daemon_stop','post_daemon_restart','normal_mesh_restart_initiated','pending_boundary_sample']],
 'identities':{s:read(f'team/runtime/{s}.json') for s in ['alpha','beta']},'config':read('team/config.json'),
 'owner_census':read('step5-owner-census.json'),'obligation_accounting':read('obligation-accounting.json'),
 'observed_obligation_accounting':read('observed-obligation-accounting.json'),
 'working_windows':read('working-windows.json'),'turns':turns,'spend':ledger,'cleanup':cleanup,'controller_exit':read('controller-exit.json')['exit'],
 'daemon_jsonl':{'rows':len(complete_rows(text('taurhaus.log.jsonl'))),'sha256':hashlib.sha256((P/'taurhaus.log.jsonl').read_bytes()).hexdigest()},
 'transient_refusals':[r for r in events if r['kind']=='transient_refusal'],
 'candidate':dict(json.loads((B/'candidate.json').read_text()),binaries=[r for r in events if r['kind']=='binary']),
 'gates':{n:json.loads((B/f'gates/gate-{n}.json').read_text()) for n in ['check-quick','lint','test-contracts']},
 'gate_cleanup':json.loads((B/'gates/gate-cleanup.json').read_text()),
 'deviations':['Spec-referenced integration checkout absent (git show exit 128); inspected its retained attempt9 sources here and messaging run2 sources read-only.',
 'Unknown-cost inputs counted separately; metered estimate is not an invoice or complete billed spend.',
 'Independent Opus evidence lens and implementer/reviewer metering belong to invoking orchestrator; Opus unavailable in this tool surface.',
 ]}
(B/'final-audit.json').write_text(json.dumps(clean(result),indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['verdict','runtime_seconds','daemon_jsonl']}))
