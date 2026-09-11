"""Assess retained run5 evidence without opening credentials or invoking a CLI."""
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
events=complete_rows(text('events.jsonl'))
cleanup=read('cleanup.json')
assert not cleanup['survivors'] and all(cleanup[k] for k in ['port_closed','root_removed','auth_removed','auth_copy_removed_before_root'])
code=read('controller-exit.json')['exit']
steps=[]
for n in range(1,7):
    path=P/f'step{n}-outcome.json'
    if path.exists():
        outcome=read(path.name)
        if outcome['outcome']=='FAIL' and 'onboarding lacks' in outcome.get('reason',''):
            outcome=dict(outcome,classification='harness',reason=outcome['reason']+'; beta runtime recovery is submitted, but no consumed_by_read or card tool-result witness exists; direct user-message startup is not the specified read proof')
    else:
        outcome={'step':n,'outcome':'NOT RUN','classification':'blocked by earlier failure','paid_inputs':0,'metered_usd':0}
        path.write_text(json.dumps(outcome,indent=2)+'\n')
    steps.append(outcome)
journal_names=[str(p.relative_to(P)) for p in (P/'team/state/messaging-v2/segments').glob('*.jsonl')] or [n for n in packed if n.startswith('team/state/messaging-v2/segments/') and n.endswith('.jsonl')]
journal=[r for name in journal_names for r in complete_rows(text(name))]
identities={s:read(f'team/runtime/{s}.json') for s in ['alpha','beta']}
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
owners=complete_rows(text('owner-observations.jsonl'))
maximum=max((len(r.get('owners',[])) for r in owners),default=0)
failed=next((s for s in steps if s['outcome']=='FAIL'),None)
result={'verdict':f"FAIL step {failed['step']} ({failed['classification']}); remaining steps NOT RUN" if failed else 'Six runtime steps completed; independent review pending',
 'step_outcomes':steps,'started_utc':datetime.datetime.fromtimestamp(start,datetime.timezone.utc).isoformat(),
 'cleanup_utc':datetime.datetime.fromtimestamp(end,datetime.timezone.utc).isoformat(),
 'runtime_seconds':end-start,'journal':journal,
 'onboarding':{seat:read('onboarding-'+seat+'.json') for seat in ['alpha','beta'] if (P/('onboarding-'+seat+'.json')).exists() or 'onboarding-'+seat+'.json' in packed},
 'onboarding_assessment':read('onboarding-assessment.json'),
 'controller_sends':[r for r in events if r['kind']=='action' and r['action'].get('op')=='mesh' and r['action'].get('argv',[None])[0]=='send'],
 'restart_boundaries':[r for r in events if r['kind'] in ['normal_daemon_stop','normal_mesh_restart_initiated','pending_boundary_sample']],
 'identities':identities,'config':read('team/config.json'),'owner_epoch':read('team/state/delivery/epoch.json'),
 'owner_census':{'samples':len(owners),'max_simultaneous_observed_owners':maximum,
 'max_gap_seconds':max((b['at']-a['at'] for a,b in zip(owners,owners[1:])),default=None),
 'restart_window':'NOT RUN' if failed and failed['step']<5 else 'see step5-owner-census.json'},
 'working_windows':{'taurhaus_boundary':{'alpha':'NOT RUN','beta':'NOT RUN'},'mesh_boundary':{'alpha':'NOT RUN','beta':'NOT RUN'}} if failed and failed['step']<2 else 'see runtime turn durations and pending boundary samples',
 'turns':turns,'spend':ledger,'cleanup':cleanup,'controller_exit':code,
 'daemon_jsonl':{'rows':len(complete_rows(text('taurhaus.log.jsonl'))),
 'sha256':hashlib.sha256((P/'taurhaus.log.jsonl').read_bytes()).hexdigest()},
 'transient_refusals':[r for r in events if r['kind']=='transient_refusal'],
 'candidate':dict(json.loads((B/'candidate.json').read_text()),binaries=[r for r in events if r['kind']=='binary']),
 'gates':{n:json.loads((B/f'gates/gate-{n}.json').read_text()) for n in ['check-quick','lint','test-contracts']},
 'gate_cleanup':json.loads((B/'gates/gate-cleanup.json').read_text()),
 'deviations':['The referenced integration attempt9 checkout was absent (git show exit 128); reused retained run4/run3 controller and inspected messaging reference read-only.',
 'Unknown-cost notify-only inputs are counted; metered estimate is not complete billed spend.',
 'The onboarding guard requires a journal card; beta startup uses a direct hosted turn. Its runtime submitted receipt exists but the required read/tool-result witness does not. No baseline or restart was attempted after the 180.8-second deadline.',
 'Independent Opus evidence review and implementer/reviewer metering remain with the invoking orchestrator.'],
 'product_defect':'None established by the stopped onboarding assertion.' if failed and failed['step']==1 else 'See ordered outcomes.'}
(B/'final-audit.json').write_text(json.dumps(clean(result),indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['verdict','runtime_seconds','daemon_jsonl']}))
