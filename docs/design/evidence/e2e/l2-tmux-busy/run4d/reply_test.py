"""Offline generated evidence only; no CLI or harness-home access."""
import unittest
from unittest.mock import Mock
import support
from controller import Trial

class ReplyTests(unittest.TestCase):
    # // Regression: 451ec092 inherited the assistant-only predicate, rejecting alpha's Mesh tool reply.
    def submitted(self):
        return {'event_type':'delivery_receipt','committed_at':'2026-09-10T00:00:02Z','payload':{'message_id':'q','stage':'submitted','observed_at':'2026-09-10T00:00:02Z'}}

    def test_alpha_journal_reply_is_sufficient(self):
        reply={'event_type':'message_accepted','committed_at':'2026-09-10T00:00:03Z','payload':{'author':{'name':'alpha','verified':True},'body':'Q-marker'}}
        result=support.reply_evidence([self.submitted(),reply],[],'q','Q-marker')
        self.assertEqual(result['source'],'alpha journal reply')
        reply['payload']['author']['name']='lead'
        self.assertFalse(support.reply_evidence([self.submitted(),reply],[],'q','Q-marker'))

    def test_any_post_delivery_rollout_row_is_sufficient(self):
        for kind in ('custom_tool_call','custom_tool_call_output','message'):
            row={'timestamp':'2026-09-10T00:00:03Z','type':'response_item','payload':{'type':kind,'content':'Q-marker'}}
            self.assertTrue(support.reply_evidence([self.submitted()],[row],'q','Q-marker'))
            row['timestamp']='2026-09-10T00:00:01Z'
            self.assertFalse(support.reply_evidence([self.submitted()],[row],'q','Q-marker'))
        self.assertFalse(support.reply_evidence([],[],'q','Q-marker'))

    def test_reply_idle_and_metering_have_distinct_waits(self):
        trial=Trial.__new__(Trial);trial.step=3
        trial.reply=Mock(return_value={'source':'alpha journal reply'})
        trial.fresh_idle=Mock(return_value={'activity_confidence':'idle'})
        trial.budget=Mock(return_value={'metering_complete':True})
        trial.save=Mock();trial.wait=Mock(side_effect=lambda predicate,*a,**kw:predicate())
        trial.settle_delivery('Q-marker','q')
        self.assertEqual([c.args[1] for c in trial.wait.call_args_list],['marker reply evidence missing','post-reply fresh idle missing','post-reply metering incomplete'])
        self.assertEqual(len(trial.save.call_args_list),3)

    def test_pending_uses_message_scope_without_health(self):
        accepted={'event_type':'message_accepted','committed_at':'2026-09-10T00:00:02Z','payload':{'message_id':'q'}}
        activity={'activity_confidence':'likely_working','observed_at':'2026-09-10T00:00:02Z'}
        result=support.pending_observation([accepted],'q',{},activity=activity,now=1788998403)
        self.assertEqual(result['source'],'message accepted without receipt while working')
        receipt={'event_type':'delivery_receipt','payload':{'message_id':'q','stage':'submitted'}}
        self.assertIsNone(support.pending_observation([accepted,receipt],'q',{},activity=activity,now=1788998403))

if __name__=='__main__':unittest.main()
