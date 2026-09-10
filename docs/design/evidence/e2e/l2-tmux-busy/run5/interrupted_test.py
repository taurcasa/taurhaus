"""Synthetic run-4e regressions; no CLI or credential access."""
import unittest
from unittest.mock import Mock, patch
from controller import Trial
from support import meter


class InterruptedTests(unittest.TestCase):
    def test_interrupted_turn_is_unknown_and_counts_as_input(self):
        # // Regression: f6880113 required usage after stop_session's aborted turn.
        rows=[{'type':'event_msg','payload':{'type':kind,'turn_id':'cut-short'}}
              for kind in ('task_started','turn_aborted')]
        value=meter([rows],[])
        self.assertEqual(value['paid_inputs'],1)
        self.assertEqual(value['turns'][0].get('classification'),'interrupted, cost unknown')
        self.assertIsNone(value['turns'][0]['usd'])
        self.assertEqual(value['api_equivalent_usd'],0)
        self.assertFalse(value['metering_complete'])

    def test_existing_usage_cap_allows_next_input_with_unknown_turn(self):
        # // Regression: f6880113 treated missing usage as a lifecycle/input gate.
        trial=Trial.__new__(Trial)
        trial.sessions=Mock(return_value=[[{'type':'event_msg','payload':{'type':'task_started','turn_id':'cut-short'}}]])
        trial.notify=Mock(return_value=[])
        trial.reservations=[{'reason':'Q2-work','confirmed':True}]
        trial.journals=Mock(return_value=[])
        trial.save=Mock();trial.started=0
        with patch('controller.time.monotonic',return_value=1):
            self.assertEqual(trial.budget(next_input=True)['paid_inputs'],1)

    def test_delivery_observations_do_not_wait_for_missing_usage(self):
        # // Regression: f6880113 bundled complete metering into resumed delivery.
        trial=Trial.__new__(Trial);trial.step=5
        trial.reply=Mock(return_value={'source':'alpha journal reply'})
        trial.fresh_idle=Mock(return_value={'activity_confidence':'idle'})
        trial.budget=Mock(return_value={'metering_complete':False})
        trial.save=Mock()
        def wait(predicate,why,*args,**kwargs):
            value=predicate()
            self.assertTrue(value,why)
            return value
        trial.wait=wait
        trial.settle_delivery('Q2','id')


if __name__=='__main__':unittest.main()
