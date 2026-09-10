"""Read-only reconciliation of this attempt's sanitized, immutable exports."""
import json
from datetime import datetime
from pathlib import Path

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
def rows(path):return [json.loads(line) for line in path.read_text().splitlines()]
def load(name):return json.loads((OUT/name).read_text())
def stamp(value):return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()

journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in rows(p)]
native=[r for p in (OUT/'sessions').glob('*.jsonl') for r in rows(p)]
events=rows(OUT/'events.jsonl')
q=load('Q-accepted.json');mid=q['message_id'];marker=q['marker']
receipts=[r for r in journal if r.get('payload',{}).get('message_id')==mid]
submitted=[r for r in receipts if r.get('payload',{}).get('stage')=='submitted']
assert len(submitted)==1
receipt=submitted[0]['payload'];delivery_at=stamp(receipt['observed_at'])
prior_idle=[r for r in events if r['kind']=='activity' and r['at']<=delivery_at and r['value'].get('activity_confidence')=='idle'][-1]
attachment=receipt['attachment'];runtime=load('final-runtime.json')
identity_matches={key:attachment[key]==runtime[field] for key,field in
                  [('pane','paneId'),('pane_pid','panePid'),('pane_start','paneStartTime'),('socket','tmuxSocket'),('tmux_session','tmuxSessionId'),('attachment_generation','attachmentGeneration')]}
assert all(identity_matches.values())
replies=[r for r in journal if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('body')==marker and r['payload'].get('author',{}).get('name')=='alpha']
assert len(replies)==1
explicit_reads=[r for r in receipts if r.get('payload',{}).get('kind')=='consumed_by_read']
assert explicit_reads
compactions=[r for r in native if r.get('payload',{}).get('item',{}).get('type')=='ContextCompaction']
unpriced=[r for r in native if r.get('payload',{}).get('type')=='token_count'
          and (u:=r.get('payload',{}).get('info',{}).get('last_token_usage',{})).get('total_tokens',0)>0
          and u.get('input_tokens')==0 and u.get('output_tokens')==0]
ledger=load('cost-ledger.json')
locks=rows(OUT/'terminal-locks.jsonl')
result={'classification':'harness timeout; incomplete evidence, no Taurhaus/Mesh product failure established',
        'step_outcomes':{str(i):load(f'step{i}-outcome.json') for i in range(1,7)},
        'Q':{'marker':marker,'message_id':mid,'submission':receipt,'explicit_reads':explicit_reads,
             'mesh_reply':replies[0],'identity_matches':identity_matches,
             'prior_idle':prior_idle,'prior_idle_age_at_submission_s':delivery_at-stamp(prior_idle['value']['observed_at']),
             'first_receipt_observation_note':'Observer saw likely_working after submission; preceding idle row is retained, not an atomic lock-time snapshot.'},
        'cost':{'metered_usd':ledger['api_equivalent_usd'],'input_count':ledger['paid_inputs'],
                'complete_cost_bound_verified':False,'reason':'Two automatic compactions have zero-priced counters; Q turn unfinished at teardown.',
                'unpriced_compaction_rows':unpriced,'compaction_boundaries':compactions},
        'daemon_rows':len(rows(OUT/'taurhaus.log.jsonl')),'passive_lock_samples':len(locks),
        'samples_with_flock':sum(bool(r['holders']) for r in locks),
        'managed_stop_exclusion':'NOT RUN; launch/delivery samples do not prove managed stop',
        'cleanup':load('cleanup.json'),'controller_exit':load('controller-exit.json')}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'exit':0,'Q_submissions':len(submitted),'Q_mesh_replies':len(replies),'Q_explicit_reads':len(explicit_reads),
                  'automatic_compactions':len(compactions),'metered_usd':ledger['api_equivalent_usd']}))
