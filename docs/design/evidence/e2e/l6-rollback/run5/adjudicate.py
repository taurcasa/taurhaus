"""Keep raw outcomes immutable; separately distinguish delivery from rereads."""
from pathlib import Path
import json
from datetime import datetime

BASE=Path(__file__).resolve().parent

def assess_delivery(message):
    submissions=message['canonical_submissions']+message['legacy_submissions']
    reads=message['explicit_canonical_reads']+sum(r.get('eventType')=='message_read' for r in message['legacy_history'])
    replies=len(message['assistant_replies'])
    return {'submissions':submissions,'reads':reads,'assistant_replies':replies,
            'delivered':bool(reads or (submissions and message.get('tool_exposure'))),
            'single_read':reads==1,'one_transport_delivery':submissions==1}

def main():
    analysis=json.loads((BASE/'analysis.json').read_text())
    events=[json.loads(x) for x in (BASE/'run/events.jsonl').read_text().splitlines()]
    start=next(r for r in events if r['kind']=='member_executor_start')
    verified=json.loads((BASE/'run/step2-verified-format.json').read_text())
    handoff=analysis['executor_observation']['same_owner_end']
    census=analysis['executor_observation']['first_executor_sample']
    result={'verdict':'FAIL step (d), harness: literal single-read criterion not met; operational rollback completed; Opus review unavailable',
      'raw_controller_exit':analysis['controller']['exit'],'raw_steps':analysis['steps'],
      'operational_steps':{'a':'PASS: A delivered/read/replied, B pending with scheduler opportunity, owner stopped, marker and named skip',
        'b':'PASS: format 0/members/verified rollback, transition complete, journal retained, B pending/unread under original id',
        'c':'PASS: same-owner exit 0 clears marker with unchanged owner/digest/epoch and no new handoff',
        'd':'FAIL exact read-cardinality criterion: one B submission and reply, fresh C submission and reply; repeated seat reads violate literal one-read cardinality',
        'e':'PASS operations: explicit reads/acks, identity reconciliation, export and teardown; performed after the unguarded read-cardinality miss'},
      'classification':'harness instructions and missing cardinality guard; no product rollback failure demonstrated',
      'messages':{k:assess_delivery(v) for k,v in analysis['messages'].items()},
      'read_detail':{'B_first_turn_reads':['2026-09-11T05:16:49.972Z','2026-09-11T05:16:52.602Z'],
        'B_later_reads':['2026-09-11T05:16:59.911Z','2026-09-11T05:17:02.159Z'],
        'reason':'Inherited AGENTS instructions specify unread read for every notification and another unfiltered read for marker messages. Alpha executed both; subsequent C and controller reconciliation also reread B. Retain all four rows; never relabel them one read or a second transport submission.'},
      'executor':{'starter':'controller, sanctioned guarded RC path; none attached immediately after (c)',
        'start_at':start['at'],'verified_transition_complete_at':verified['at'],
        'handoff_end_at':handoff['at'],'seconds_after_verified_downgrade':start['at']-verified['at'],
        'seconds_after_handoff_end':start['at']-handoff['at'],'first_census_at':census['at'],
        'max_live_alpha_executors':max(len(r['executors']) for r in [json.loads(x) for x in (BASE/'run/owner-census.jsonl').read_text().splitlines()]),
        'binary':census['executors'][0],'runtime':census['runtime']},
      'alpha_initiated_journal_messages':analysis['alpha_journal_traffic'],
      'Opus_review':{'status':'unavailable','exit_code':None,'reason':'No Opus model or Workflow endpoint in active tool surface; no reviewer launched, no substitute claimed.'},
      'deviations':['The runtime controller checked at most one submission but omitted a strict single-read-row guard, so C and final reconciliation ran after B had been read twice. No paid retry or product change.','Inherited instructions elicited unfiltered rereads; one-read cardinality is not claimed.'],
      'raw_outcome_hashes':analysis['raw_outcome_hashes']}
    (BASE/'adjudication.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'verdict':result['verdict'],'messages':result['messages']}))

if __name__=='__main__':main()
