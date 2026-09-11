"""Offline controller regression tests: never boot, spawn or access harness homes."""
import unittest
from datetime import datetime, timezone
from unittest.mock import Mock
import controller

STAMP='2026-09-11T00:00:00+00:00'
class DeliveryContract(unittest.TestCase):
    def setUp(self):
        self.record={'session_id':'s','paneId':'%2','attachmentGeneration':1}
        self.activity={'observed_at':STAMP,'activity_confidence':'idle'}
        self.snapshot={'runtime_sessions':[{'session_id':'s','tmux_pane':'%2','state':'idle','activity_attribution':'attributed'}]}
        self.card={'event_type':'message_accepted','committed_at':STAMP,'payload':{'message_id':'A','body':'[taurhaus] recovery_card A-marker','delivery_targets':[{'recipient':'alpha'}]}}
        self.read={'event_type':'receipt','committed_at':STAMP,'payload':{'message_id':'A','kind':'consumed_by_read','reader_name':'alpha','observed_at':STAMP}}
        self.submitted={'event_type':'delivery_receipt','committed_at':STAMP,'payload':{'message_id':'A','stage':'submitted','recipient':'alpha','observed_at':STAMP}}
        self.tool={'type':'response_item','timestamp':STAMP,'payload':{'type':'function_call_output','output':self.card['payload']['body']}}
        self.reply={'type':'response_item','timestamp':STAMP,'payload':{'type':'message','role':'assistant','content':[{'text':'A-marker'}]}}

    def test_executed_startup_guard_accepts_alpha_read_alone(self):
        # // Regression: 5e89fb019's submitted-AND-read guard survived in de0161b4 run4.
        self.assertEqual(controller.onboarding_delivered(self.record,self.activity,self.snapshot,[self.card,self.read],datetime.fromisoformat(STAMP).timestamp()),'A')

    def test_executed_startup_guard_accepts_submitted_tool_alternative(self):
        args=(self.record,self.activity,self.snapshot,[self.card,self.submitted],datetime.fromisoformat(STAMP).timestamp())
        self.assertFalse(controller.onboarding_delivered(*args,native=[]))
        self.assertEqual(controller.onboarding_delivered(*args,native=[self.tool]),'A')
        self.assertFalse(controller.onboarding_delivered(*args,native=[self.reply]))

    def test_wrong_reader_does_not_satisfy_startup(self):
        other=self.read|{'payload':self.read['payload']|{'reader_name':'lead'}}
        self.assertFalse(controller.onboarding_delivered(self.record,self.activity,self.snapshot,[self.card,other],datetime.fromisoformat(STAMP).timestamp()))

    def test_actual_A_delivery_path_needs_no_submission(self):
        # // Regression: de0161b4 kept submitted-only A/reply predicates despite explicit-read delivery.
        t=object.__new__(controller.Trial);t.step=1
        t.journals=lambda:[self.card,self.read];t.sessions=lambda:[[self.reply]]
        t.record=lambda:self.record;t.activity=lambda:self.activity;t.fresh_idle=lambda:self.activity
        t.attributed=lambda:self.snapshot;t.save=Mock();t.capture=Mock();t.budget=Mock(return_value={})
        def wait(fn,why,*args,**kwargs):
            value=fn()
            assert value,why
            return value
        t.wait=wait
        t.delivered('A-marker','A',1)

    def test_actual_legacy_delivery_accepts_explicit_seat_read_without_paste(self):
        # // Regression: de0161b4 required a tmux receipt even when alpha read the card first.
        t=object.__new__(controller.Trial);t.step=4
        t.projection=lambda mid:{'id':'dB','message_id':'B','text':'B-marker','read':True}
        t.workflow=lambda:[{'eventType':'message_read','message_id':'dB','reader':'alpha','timestamp':STAMP}]
        t.sessions=lambda:[[self.tool|{'payload':{'type':'function_call_output','output':'B-marker'}},self.reply|{'payload':self.reply['payload']|{'content':[{'text':'B-marker'}]}}]]
        t.attributed=lambda:self.snapshot;t.save=Mock()
        def wait(fn,why,*args,**kwargs):
            value=fn();assert value,why;return value
        t.wait=wait
        self.assertTrue(t.legacy_delivery('B-marker','B'))

if __name__=='__main__':unittest.main()
