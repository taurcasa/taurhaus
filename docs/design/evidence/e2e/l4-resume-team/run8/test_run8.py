"""Offline run8 guards: generated records only; no credentials or live CLI."""
import json
import unittest
import controller
from support import rollout_usage


class ObserverRegressions(unittest.TestCase):
    def test_hosted_resume_opens_meter_epoch(self):
        # // Regression: 95d31ae3 (original c008cc2e) aborted run6 on resumed cumulative counters.
        def event(kind, **values):
            return {'type':'event_msg','payload':{'type':kind, **values}}
        def count(i,c,o):
            return event('token_count', info={'total_token_usage':{
                'input_tokens':i,'cached_input_tokens':c,'output_tokens':o}})
        records=[event('task_started',turn_id='before'),count(100,20,10),
                 event('task_started',turn_id='after'),count(0,0,0),count(30,10,4),
                 count(30,10,4),count(5,2,1),count(9,3,2)]
        actual=rollout_usage(records,'thread')
        self.assertEqual([(r['input'],r['cached_input'],r['output']) for r in actual],
                         [(100,20,10),(39,13,6)])
        self.assertEqual(actual,rollout_usage(records,'thread'))

    def test_null_snapshot_fields_remain_absent(self):
        # // Regression: 95d31ae3 (original c008cc2e), observed at 7a4595f2: null fields crashed snapshot export.
        value={'path':None,'jsonl_path':None,'output':json.dumps({
            'method':None,'event':None,'path':None,'cursor':'opaque','status':'completed'})}
        actual=controller.clean(value)
        self.assertIsNone(actual['path'])
        self.assertIsNone(actual['jsonl_path'])
        embedded=json.loads(actual['output'])
        self.assertIsNone(embedded['path'])
        self.assertEqual(embedded['cursor'],'<signed-read-cursor-redacted>')
        self.assertEqual(embedded['status'],'completed')


class ResumeIdentity(unittest.TestCase):
    # // Regression: 95d31ae3 required unchanged tmux rollout identity; PRs #172/#174 require resume then rebind.
    def test_recorded_resume_with_new_attributed_rollout(self):
        from support import require_alpha_resume
        old={'session_id':'old'}
        new={'session_id':'new','paneId':'%9'}
        launch={'mode':'resume','command':"codex --sandbox danger-full-access resume 'old'"}
        activity={'degraded':False,'runtime_sessions':[{'session_id':'new','tmux_pane':'%9',
                  'pid':123,'state':'idle','activity_attribution':'attributed'}]}
        events=[{'event':'activity.state.changed','session_id':'new','pid':123,
                 'to':'idle','source':'notify'}]
        require_alpha_resume(old,new,launch,activity,events)
        for changed in [{'mode':'fresh','command':launch['command']},
                        {'mode':'resume','command':"codex resume 'wrong'"}]:
            with self.assertRaises(AssertionError):
                require_alpha_resume(old,new,changed,activity,events)
        with self.assertRaises(AssertionError):
            require_alpha_resume(old,{**new,'session_id':'old'},launch,activity,events)
        with self.assertRaises(AssertionError):
            require_alpha_resume(old,new,launch,activity,[])


if __name__=='__main__': unittest.main()
