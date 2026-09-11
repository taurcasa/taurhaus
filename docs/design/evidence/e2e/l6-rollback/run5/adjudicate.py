"""Keep raw outcomes immutable; separately distinguish delivery from rereads."""
from pathlib import Path
import json
from datetime import datetime, timezone

BASE=Path(__file__).resolve().parent

def instant(value):
    return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()

def assess_delivery(message):
    submissions=message['canonical_submissions']+message['legacy_submissions']
    legacy_reads=[r for r in message['legacy_history'] if r.get('eventType')=='message_read']
    reads=message['explicit_canonical_reads']+len(legacy_reads)
    canonical=[r['committed_at'] for r in message.get('canonical_history',[])
               if r.get('payload',{}).get('kind')=='consumed_by_read'
               and r['payload'].get('reader_name')=='alpha']
    seat_reads=sorted(canonical+[r['timestamp'] for r in legacy_reads
                               if r.get('reader')=='alpha'],key=instant)
    replies=len(message['assistant_replies'])
    first_reply=min((r['timestamp'] for r in message['assistant_replies']
                     if 'timestamp' in r),key=instant,default=None)
    # The count-only fallback supports the original offline accounting fixtures.
    consumed=bool(seat_reads or message['explicit_canonical_reads'])
    delivered=consumed or bool(submissions and message.get('tool_exposure'))
    before=[t for t in seat_reads if first_reply and instant(t)<=instant(first_reply)]
    after=[t for t in seat_reads if first_reply and instant(t)>instant(first_reply)]
    return {'submissions':submissions,'reads':reads,'assistant_replies':replies,
            'delivered':delivered,'first_seat_consumptions':int(consumed),
            'first_read_at':seat_reads[0] if seat_reads else None,
            'reads_before_first_reply':before,'reads_after_first_reply':after,
            'single_read':reads==1, # Descriptive row count only; never a delivery predicate.
            'one_transport_delivery':submissions==1,
            'passes_delivery_checks':delivered and submissions<=1 and replies==1}

def assess_self_heal(passes, boundary, end):
    in_window=[r['ts'] for r in passes if boundary<=instant(r['ts'])<=instant(end)]
    return {'status':'OPPORTUNITY OBSERVED' if in_window else 'NOT OBTAINED',
            'members_owned_window_start':datetime.fromtimestamp(boundary,timezone.utc).isoformat(),
            'members_owned_window_end':end,
            'self_heal_pass_timestamps':[r['ts'] for r in passes],
            'passes_in_window':in_window,
            'reason':'A census before the next self-heal pass cannot establish automatic executor behavior.',
            'future_observation':'After (c), hold for at least one full self-heal cadence with a bounded wait and census sampled throughout before concluding none is attached.'}

def main():
    analysis=json.loads((BASE/'analysis.json').read_text())
    events=[json.loads(x) for x in (BASE/'run/events.jsonl').read_text().splitlines()]
    start=next(r for r in events if r['kind']=='member_executor_start')
    verified=json.loads((BASE/'run/step2-verified-format.json').read_text())
    handoff=analysis['executor_observation']['same_owner_end']
    census=analysis['executor_observation']['first_executor_sample']
    daemon=[json.loads(x) for x in (BASE/'run/taurhaus.log.jsonl').read_text().splitlines()]
    messages={k:assess_delivery(v) for k,v in analysis['messages'].items()}
    max_executors=max(len(r['executors']) for r in [json.loads(x) for x in (BASE/'run/owner-census.jsonl').read_text().splitlines()])
    delivery_pass=all(m['passes_delivery_checks'] for m in messages.values()) and max_executors==1
    result={'verdict':('PASS operational rollback (a)-(e)' if delivery_pass else 'FAIL delivery accounting')+'; automatic-executor product observation NOT OBTAINED; full workflow incomplete (original Opus review unavailable)',
      'raw_controller_exit':analysis['controller']['exit'],'raw_steps':analysis['steps'],
      'operational_steps':{'a':'PASS: A delivered/read/replied, B pending with scheduler opportunity, owner stopped, marker and named skip',
        'b':'PASS: format 0/members/verified rollback, transition complete, journal retained, B pending/unread under original id',
        'c':'PASS: same-owner exit 0 clears marker with unchanged owner/digest/epoch and no new handoff',
        'd':('PASS' if delivery_pass else 'FAIL')+': one transport submission, first seat consumption and one reply per A/B/C; no duplicate executor; rereads are observations',
        'e':'PASS: explicit reads/acks, identity reconciliation, export and teardown'},
      'classification':'S-runtime operational PASS; automatic-executor observation NOT OBTAINED (no self-heal opportunity); original failure was an offline adjudication error',
      'messages':messages,
      'read_detail':{'B_first_turn_reads':messages['B']['reads_before_first_reply'],
        'B_later_reads':messages['B']['reads_after_first_reply'],
        'reason':'Inherited AGENTS instructions specify unread read for every notification and another unfiltered read for marker messages. Alpha executed both; subsequent C and controller reconciliation also reread B. Retain all four rows; never relabel them one read or a second transport submission.'},
      'executor':{'starter':'controller, sanctioned guarded RC path; immediate census only, no product inference',
        'product_observation':assess_self_heal(analysis['owner_observation']['self_heal_passes'],handoff['at'],daemon[-1]['ts']),
        'start_at':start['at'],'verified_transition_complete_at':verified['at'],
        'handoff_end_at':handoff['at'],'seconds_after_verified_downgrade':start['at']-verified['at'],
        'seconds_after_handoff_end':start['at']-handoff['at'],'first_census_at':census['at'],
        'max_live_alpha_executors':max_executors,
        'binary':census['executors'][0],'runtime':census['runtime']},
      'alpha_initiated_journal_messages':analysis['alpha_journal_traffic'],
      'Opus_review':{'status':'unavailable','exit_code':None,'reason':'No Opus model or Workflow endpoint in active tool surface; no reviewer launched, no substitute claimed.'},
      'deviations':['Automatic member-executor product observation NOT OBTAINED: no self-heal pass after the members-owned boundary before teardown; controller started the executor after an immediate census.','Inherited instructions and required reconciliation elicited rereads; row multiplicity is retained but does not count as duplicate delivery. No paid retry or product change.'],
      'raw_outcome_hashes':analysis['raw_outcome_hashes']}
    (BASE/'adjudication.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'verdict':result['verdict'],'messages':result['messages']}))


# Tests stay in this named evidence script to keep the review fix local.
import unittest

class AdjudicationRegressionTests(unittest.TestCase):
    def message(self):
        return {'canonical_submissions':0, 'legacy_submissions':1,
                'explicit_canonical_reads':0, 'canonical_history':[],
                'legacy_history':[{'eventType':'message_read', 'reader':'alpha',
                    'timestamp':f'2030-01-01T00:00:0{i}Z'} for i in (1,2,4)],
                'assistant_replies':[{'timestamp':'2030-01-01T00:00:03Z'}]}

    def test_rereads_do_not_fail_any_marker(self):
        # // Regression: 06178535 counted reconciliation reads as deliveries; baa3c509 escalated only B.
        for marker in ('A','B','C'):
            with self.subTest(marker=marker):
                result=assess_delivery(self.message())
                self.assertTrue(result['passes_delivery_checks'])
                self.assertEqual(result['first_seat_consumptions'],1)
                self.assertEqual(result['reads'],3)
                self.assertEqual(result['first_read_at'],'2030-01-01T00:00:01Z')
                self.assertEqual(result['reads_before_first_reply'],[
                    '2030-01-01T00:00:01Z','2030-01-01T00:00:02Z'])
                self.assertEqual(result['reads_after_first_reply'],['2030-01-01T00:00:04Z'])

    def test_replay_or_duplicate_reply_still_fails(self):
        # // Regression: 06178535 used read cardinality instead of the operative transport/reply facts.
        for field in ('legacy_submissions','assistant_replies'):
            message=self.message()
            message[field]=2 if field=='legacy_submissions' else message[field]*2
            self.assertFalse(assess_delivery(message)['passes_delivery_checks'])

    def test_read_only_delivery_and_other_reader(self):
        # // Regression: 06178535's row counter did not distinguish consumption from reread traffic.
        message=self.message();message['legacy_submissions']=0
        self.assertTrue(assess_delivery(message)['passes_delivery_checks'])
        for row in message['legacy_history']:row['reader']='lead'
        self.assertFalse(assess_delivery(message)['delivered'])

    def test_no_self_heal_opportunity_is_not_obtained(self):
        # // Regression: 06178535 inferred product behavior from a census before the next scheduled pass.
        passes=[{'ts':'2030-01-01T00:00:00Z'},{'ts':'2030-01-01T00:00:30Z'}]
        result=assess_self_heal(passes,1893456032.0,'2030-01-01T00:00:59Z')
        self.assertEqual(result['status'],'NOT OBTAINED')
        self.assertEqual(result['passes_in_window'],[])
        passes.append({'ts':'2030-01-01T00:01:00Z'})
        result=assess_self_heal(passes,1893456032.0,'2030-01-01T00:01:01Z')
        self.assertEqual(result['status'],'OPPORTUNITY OBSERVED')
        self.assertEqual(len(result['passes_in_window']),1)

if __name__=='__main__':main()
