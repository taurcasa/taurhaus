"""Synthetic offline checks; never launch a CLI or inspect a real harness home."""
import unittest
from support import complete_rows, meter, pending_receipt, clean

class ControllerTests(unittest.TestCase):
    def test_partial_rows_not_evidence(self):
        self.assertEqual(complete_rows('{"a":1}\n{"a":2}'), [{'a':1}])
    def test_cost_is_sum_across_sessions_not_max(self):
        sessions = [[{'type':'event_msg','payload':{'type':'task_started','turn_id':name}},
                     {'type':'event_msg','payload':{'type':'token_count','info':{'total_token_usage':{'input_tokens':1000,'cached_input_tokens':200,'output_tokens':100}}}},
                     {'type':'event_msg','payload':{'type':'task_complete','turn_id':name}}] for name in ('a','b')]
        result=meter(sessions, [])
        self.assertEqual(result['paid_inputs'],2)
        self.assertAlmostEqual(result['conservative_usd'],.00264)
        self.assertTrue(result['metering_complete'])
    def test_notify_only_turn_is_unknown_not_free(self):
        result=meter([], [{'turn-id':'unseen'}])
        self.assertEqual(result['paid_inputs'],1)
        self.assertFalse(result['metering_complete'])
        self.assertIsNone(result['turns'][0]['usd'])
    def test_pending_is_message_scoped_and_transport_is_not_read(self):
        rows=[{'payload':{'message_id':'old','stage':'pending','evidence':'activity not freshly idle'}}, {'payload':{'message_id':'q','kind':'consumed_by_read'}}]
        self.assertIsNone(pending_receipt(rows,'q'))
        rows.append({'payload':{'message_id':'q','stage':'pending','evidence':'activity not freshly idle'}})
        self.assertEqual(pending_receipt(rows,'q')['stage'],'pending')
    def test_private_account_fields_and_operator_paths_are_removed(self):
        result=clean({'installation_id':'secret','account_usage':[1], 'path':'/home/example/.codex/auth.json','message':'ok'})
        self.assertNotIn('installation_id',result)
        self.assertNotIn('account_usage',result)
        self.assertNotIn('/home/example',result['path'])

if __name__=='__main__':unittest.main()
