"""Offline synthetic run-4b regressions; no CLI or real credential access."""
import unittest
import support


def event(kind, **fields):
    return {'type':'event_msg', 'payload':{'type':kind, **fields}}

class RulingsTests(unittest.TestCase):
    def test_notify_only_is_recorded_without_blocking_metering(self):
        # // Regression: fb6200e0 inherited a notify-only identity as an unmetered paid input.
        rows=[event('task_started',turn_id='turn'),event('token_count',info={'total_token_usage':{'input_tokens':100,'cached_input_tokens':20,'output_tokens':10}}),event('task_complete',turn_id='turn')]
        result=support.meter([rows],[{'turn-id':'notify'}])
        self.assertTrue(result['metering_complete'])
        self.assertEqual(result['paid_inputs'],1)
        notify=next(t for t in result['turns'] if t['turn_id']=='notify')
        self.assertIsNone(notify['usd'])
        self.assertEqual(notify['classification'],'unknown-but-not-a-model-input')

    def test_each_usage_increment_is_retained_without_streaming_double_count(self):
        # // Regression: fb6200e0 retained only turn totals, hiding individual model-call spends.
        rows=[event('task_started',turn_id='turn')]
        for total in [100,100,250]:
            rows.append(event('token_count',info={'total_token_usage':{'input_tokens':total,'cached_input_tokens':0,'output_tokens':0}}))
        rows.append(event('task_complete',turn_id='turn'))
        result=support.meter([rows],[])
        self.assertEqual([r['usage']['input_tokens'] for r in result['usage_rows']],[100,150])
        self.assertAlmostEqual(result['api_equivalent_usd'],.00005)

    def test_onboarding_requires_matching_submission_read_and_fresh_attribution(self):
        # // Regression: fb6200e0 required card text in a user row although Mesh returns it in a tool result.
        record={'session_id':'thread','paneId':'%2'}
        activity={'activity_confidence':'idle','observed_at':'2026-09-10T00:00:00Z'}
        snapshot={'runtime_sessions':[{'session_id':'thread','tmux_pane':'%2','state':'idle','activity_attribution':'attributed'}]}
        accepted={'event_type':'message_accepted','payload':{'message_id':'card','body':'[taurhaus] recovery_card','delivery_targets':[{'recipient':'alpha'}]}}
        submitted={'payload':{'message_id':'card','recipient':'alpha','adapter':'tmux/1','stage':'submitted'}}
        read={'payload':{'message_id':'card','kind':'consumed_by_read'}}
        rows=[accepted,submitted,read]
        self.assertTrue(support.onboarding_delivered(record,activity,snapshot,rows,1788998400))
        self.assertFalse(support.onboarding_delivered(record,activity,snapshot,rows[:-1],1788998400))
        self.assertFalse(support.onboarding_delivered(record,activity,snapshot,rows,1788998521))
        read['payload']['message_id']='unrelated'
        self.assertFalse(support.onboarding_delivered(record,activity,snapshot,rows,1788998400))

if __name__=='__main__':unittest.main()
