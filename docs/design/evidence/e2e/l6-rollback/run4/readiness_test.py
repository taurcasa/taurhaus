"""Diagnose consumed-by-read startup without replaying a paid launch."""
from datetime import datetime,timezone
import unittest
from rules import startup_delivery, input_count

class ReadinessContract(unittest.TestCase):
    def setUp(self):
        self.accepted={'event_type':'message_accepted','payload':{'message_id':'onboard','body':'[taurhaus] recovery_card','delivery_targets':[{'recipient':'alpha'}]}}
        self.read={'event_type':'receipt','payload':{'message_id':'onboard','kind':'consumed_by_read','reader_name':'alpha'}}

    def test_alpha_read_is_sufficient_without_submitted(self):
        # // Regression: 5e89fb019 introduced a submitted-AND-read onboarding guard.
        self.assertEqual(startup_delivery([self.accepted,self.read],[]),'onboard')
        self.assertIsNone(startup_delivery([self.accepted,self.read|{'payload':self.read['payload']|{'reader_name':'lead'}}],[]))

    def test_submission_requires_card_in_tool_result(self):
        submit={'event_type':'receipt','payload':{'message_id':'onboard','stage':'submitted','recipient':'alpha'}}
        rows=[self.accepted,submit]
        self.assertIsNone(startup_delivery(rows,[]))
        tool={'type':'response_item','payload':{'type':'function_call_output','output':'[taurhaus] recovery_card'}}
        self.assertEqual(startup_delivery(rows,[tool]),'onboard')
        self.assertIsNone(startup_delivery(rows,[tool|{'payload':tool['payload']|{'type':'message'}}]))

    def test_count_does_not_erase_real_turn_without_submission_receipt(self):
        # // Regression: 22643c6f replaced rollout turn count with submitted transport count.
        self.assertEqual(input_count({'paid_inputs':0,'rollout_turns':1}),1)
        self.assertEqual(input_count({'paid_inputs':4,'rollout_turns':3}),4)

if __name__=='__main__':unittest.main()
