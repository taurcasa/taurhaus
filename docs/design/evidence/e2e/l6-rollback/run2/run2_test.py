"""Synthetic evidence only; no subprocesses, CLIs, or credential access."""
import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run2_rules import pending as pending_observation

class Run2Tests(unittest.TestCase):
    def setUp(self):
        self.accepted={'event_type':'message_accepted','committed_at':'2026-09-10T00:00:02Z','payload':{'message_id':'B'}}
        self.activity={'activity_confidence':'likely_working','observed_at':'2026-09-10T00:00:02Z'}

    def test_pending_row_is_never_scheduler_opportunity(self):
        # // Regression: 82e2655b accepted an invented stage:pending row as evidence.
        fake={'event_type':'receipt','payload':{'message_id':'B','stage':'pending'}}
        self.assertIsNone(pending_observation([self.accepted,fake],'B',{},activity=self.activity,now=1788998403))

    def test_pending_obligation_can_prove_scheduler_opportunity(self):
        from run2_rules import pending
        obligation={'obligation':{'message_id':'B','recipient':'alpha'},'since':'2026-09-10T00:00:02Z'}
        result=pending([self.accepted],'B',{},activity=self.activity,now=1788998403,obligations=[obligation])
        self.assertEqual(result['scheduler_opportunity']['obligation'],obligation)
        for wrong in ('other',None):
            obligation['obligation']['message_id']=wrong
            self.assertIsNone(pending([self.accepted],'B',{},activity=self.activity,now=1788998403,obligations=[obligation]))

    def test_heartbeat_compared_as_time_and_working_required(self):
        from run2_rules import pending
        health={'heartbeat':'2026-09-10T00:00:03+00:00'}
        self.assertIsNotNone(pending([self.accepted],'B',health,activity=self.activity,now=1788998403))
        self.assertIsNone(pending([self.accepted],'B',health,activity={**self.activity,'activity_confidence':'idle'},now=1788998403))
        begun={'event_type':'receipt','payload':{'message_id':'B','stage':'attempt_started'}}
        self.assertIsNone(pending([self.accepted,begun],'B',health,activity=self.activity,now=1788998403))

    def test_format_boundary_requires_exact_format_and_retained_history(self):
        from run2_rules import format_boundary
        config={'messaging_format':1,'messaging_downgrade_sha256':'digest'}
        marker={'transition':'complete','legacy_cut':'digest'}
        format_boundary(config,marker,[self.accepted],[self.accepted],True)
        for fmt,after,verified in ((2,[self.accepted],True),(0,[self.accepted],True),(1,[],True),(1,[self.accepted],False)):
            with self.assertRaises(AssertionError):format_boundary({**config,'messaging_format':fmt},marker,[self.accepted],after,verified)

    def test_reply_requires_assistant_message_not_tool_echo(self):
        from run2_rules import assistant_reply
        rows=[{'timestamp':'2026-09-10T00:00:03Z','type':'response_item','payload':{'type':'function_call_output','output':'C-marker'}}]
        self.assertIsNone(assistant_reply(rows,'C-marker','2026-09-10T00:00:02Z'))
        rows.append({'timestamp':'2026-09-10T00:00:04Z','type':'response_item','payload':{'type':'message','role':'assistant','content':[{'type':'output_text','text':'C-marker'}]}})
        self.assertEqual(assistant_reply(rows,'C-marker','2026-09-10T00:00:02Z'),rows[-1])

if __name__=='__main__':unittest.main()
