"""Offline run8 guards: generated records only; no credentials or live CLI."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import controller
from support import classify_failure, rollout_usage


class ObserverRegressions(unittest.TestCase):
    def test_failure_classification_respects_mesh_origin_and_cap_phrases(self):
        # // Regression: 4a4c65d8 inherited bare 'cap', misclassifying capabilities and Mesh refusals.
        cases = {
            'Mesh refusal: delivery capabilities unavailable': 'mesh',
            'Mesh refusal: Permission denied': 'mesh',
            'delivery capabilities unavailable': 'taurhaus',
            'capture failed': 'taurhaus',
            'input cap exceeded': 'harness',
            'metered cap exceeded': 'harness',
            'missing headroom': 'harness',
            "'NoneType' object has no attribute 'startswith'": 'harness',
        }
        for reason, expected in cases.items():
            with self.subTest(reason=reason):
                self.assertEqual(classify_failure(reason), expected)

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
    def test_step5_recovery_context_matches_string_runtime_generation(self):
        # // Regression: 4a4c65d8 inherited an int/string comparison that rejects protocol-26 runtime records.
        with tempfile.TemporaryDirectory() as temporary:
            lane = object.__new__(controller.Lane)
            lane.team = Path(temporary)
            lane.original_config = {'team_incarnation_id': 'incarnation', 'members': [
                {'name': 'alpha', 'adapter_mode': 'tmux'},
                {'name': 'beta', 'adapter_mode': 'app_server'}]}
            (lane.team/'config.json').write_text(json.dumps(lane.original_config))
            lane.old = {member: {'attachmentGeneration': 1, 'session_id': member+'-old',
                                'tmuxSessionId': 'private'} for member in ['lead', 'alpha', 'beta']}
            lane.old['beta']['appServer'] = {'threadId': 'beta-old'}
            records = {member: {**old, 'attachmentGeneration': 2, 'contextGeneration': '0'}
                       for member, old in lane.old.items()}
            records['alpha'].update(session_id='alpha-new', paneId='%9')
            lane.rollouts = {}
            for member in ['alpha', 'beta']:
                key = {'context': [2, 0], 'recipient': ['incarnation', member]}
                records[member]['recovery'] = {'last_delivered': {'card_key': key}}
                lane.rollouts[records[member]['session_id']] = [{'payload': {'role': 'user',
                    'content': [{'text': '[taurhaus] recovery_card\nIdentity: '+member+'; key='+json.dumps(key)}]}}]
            lane.record = records.__getitem__
            lane.markers = {('pending', m): m+'-pending' for m in ['alpha', 'beta']}
            lane.replies = Mock(return_value=['reply'])
            lane.receipts = lambda kind, member: [{'payload': {'stage':
                'submitted' if member == 'alpha' else 'native_enqueued'}}]
            lane.settled = lane.alpha_idle = Mock(return_value=True)
            lane.wait = lambda predicate, *args, **kwargs: self.assertTrue(predicate())
            lane.export_daemon_log = Mock()
            lane.rpc = Mock(return_value={'runtime_sessions': [{'session_id': 'alpha-new',
                'tmux_pane': '%9', 'pid': 123, 'state': 'idle', 'activity_attribution': 'attributed'}]})
            logs = [{'event': 'launch.command.rendered', 'member': 'alpha', 'mode': 'resume',
                     'command': "codex resume 'alpha-old'"},
                    {'event': 'activity.state.changed', 'session_id': 'alpha-new', 'pid': 123,
                     'to': 'idle', 'source': 'notify'}]
            with patch.object(controller, 'save'), patch.object(controller, 'rows', return_value=logs):
                lane.step5()
                for member in ['alpha', 'beta']:
                    records[member]['contextGeneration'] = '1'
                    with self.assertRaisesRegex(AssertionError, member+' recovery receipt generation mismatch'):
                        lane.step5()
                    records[member]['contextGeneration'] = '0'
                    cards = lane.rollouts[records[member]['session_id']]
                    cards.append(cards[0])
                    with self.assertRaisesRegex(AssertionError, member+' needs exactly one'):
                        lane.step5()
                    cards.pop()

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
