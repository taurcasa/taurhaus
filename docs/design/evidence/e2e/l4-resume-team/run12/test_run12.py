"""Run12 stop observation regressions, generated data and mocked RPCs only."""
import unittest
from unittest.mock import Mock, MagicMock, patch
import controller
import support


class ReadyStop(unittest.TestCase):
    # // Regression: ba431fbb followed the run11 ruling's wrong sessionDead wire spelling; 48f5d010 documented the failure.
    def setUp(self):
        self.old = {'session_id': 'recorded', 'jsonl_path': None,
                    'paneId': '%1', 'panePid': 7, 'paneStartTime': '42'}
        self.ready = {'session_id': 'recorded', 'jsonl_path': '/scratch/ready.jsonl'}
        self.dead = {**self.old, **self.ready, 'health': 'session_dead', 'daemon_pid': None}

    def test_ready_path_and_retained_pane_handles_are_accepted(self):
        support.require_stopped_identity(self.old, self.dead, self.ready)

    def test_wrong_ready_identity_path_health_or_binding_rejected(self):
        for change in [{'session_id': 'other'}, {'jsonl_path': '/scratch/other.jsonl'},
                       {'jsonl_path': None}, {'paneId': None}, {'panePid': None},
                       {'paneStartTime': None}, {'health': 'healthy'},
                       {'health': 'sessionDead'}, {'daemon_pid': 9}]:
            with self.subTest(change=change), self.assertRaises(AssertionError):
                support.require_stopped_identity(self.old, {**self.dead, **change}, self.ready)

    def test_all_stops_precede_poll_and_both_captures_are_saved(self):
        lane = object.__new__(controller.Lane)
        lane.old = {m: {**self.old, 'paneId': '%' + str(i)}
                    for i, m in enumerate(['alpha', 'beta', 'lead'], 1)}
        lane.ready_alpha = self.ready
        lane.env = {}; lane.identities = Mock(return_value=[]); lane.event = Mock()
        lane.team = MagicMock(); lane.journal_rows = Mock(return_value=[{}])
        order = []; dead = False
        def record(member):
            return {**lane.old[member], **self.ready, 'health': 'session_dead' if dead else 'healthy',
                    'daemon_pid': None if dead else 9}
        lane.record = record
        lane.rpc = lambda method, params: order.append(params['tmux_pane'])
        def wait(test, reason, timeout):
            nonlocal dead
            self.assertEqual(order[:4], ['before-stop-processes.json', '%1',
                                       'step2-alpha-runtime-record.json', '%2'])
            self.assertIn('%3', order)
            self.assertEqual(timeout, 100)
            self.assertFalse(test())
            dead = True
            self.assertTrue(test())
        lane.wait = wait
        with patch.object(controller, 'save', side_effect=lambda name, value: order.append(name)), \
             patch.object(controller.subprocess, 'run', return_value=Mock(stdout='%0\n')):
            lane.step2()
        self.assertIn('step2-alpha-converged-runtime-record.json', order)


if __name__ == '__main__':
    unittest.main()
