"""Offline generated-data regression checks; no real homes or CLIs."""
import copy
import json
import os
import runpy
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
import controller


class Run8Tests(unittest.TestCase):
    def facts(self, **changes):
        from run8_guard import delivery_input_facts
        record = {'session_id': 'seat-session', 'jsonl_path': '/scratch/rollout',
                  'recovery': {'last_delivered': {'journal': {'message_id': 'card'}}}}
        activity = {'observed_at': '2026-09-11T00:00:00Z', 'activity_confidence': 'idle',
                    'pane_alive': True, 'pane_foreign': False}
        accepted = {'event_type': 'message_accepted', 'payload': {'message_id': 'card',
                    'body': '[taurhaus] recovery_card\nIdentity: alpha',
                    'delivery_targets': [{'recipient': 'alpha', 'delivery_id': 'delivery'}]}}
        receipt = {'event_type': 'receipt', 'payload': {'message_id': 'card',
                   'delivery_id': 'delivery', 'recipient': 'alpha', 'stage': 'submitted'}}
        read = {'payload': {'message_id': 'card', 'kind': 'consumed_by_read', 'reader_name': 'alpha'}}
        args = dict(record=record, activity=activity, journal=[accepted, receipt, read],
                    rows=[], lock_sample={'complete': True, 'held': False}, now=1789084800)
        args.update(changes)
        return delivery_input_facts(**args), args

    def test_pending_unknown_and_unread_onboarding_block_input(self):
        # // Regression: 2a38c109 reused a setup input during pending run7 onboarding.
        facts, args = self.facts()
        self.assertTrue(facts['ready'])
        for stage in ('attempt_started', 'outcome_unknown', 'native_enqueued'):
            altered = copy.deepcopy(args['journal']); altered[1]['payload']['stage'] = stage
            self.assertFalse(self.facts(journal=altered)[0]['ready'])
        self.assertFalse(self.facts(journal=args['journal'][:2])[0]['ready'])
        output = {'payload': {'type': 'function_call_output',
                  'output': json.dumps({'message_id': 'card', 'body': '[taurhaus] recovery_card\nIdentity: alpha'})}}
        self.assertTrue(self.facts(journal=args['journal'][:2], rows=[output])[0]['ready'])
        output['payload']['type'] = 'user_message'
        self.assertFalse(self.facts(journal=args['journal'][:2], rows=[output])[0]['ready'])

    def test_later_pending_delivery_idle_and_terminal_lock(self):
        facts, args = self.facts()
        message = copy.deepcopy(args['journal'][0]); message['payload']['message_id'] = 'later'
        message['payload']['delivery_targets'][0]['delivery_id'] = 'later-delivery'
        self.assertFalse(self.facts(journal=args['journal']+[message])[0]['ready'])
        attempt = {'event_type': 'delivery_attempt', 'payload': {'delivery_id': 'delivery', 'recipient': 'alpha', 'stage': 'attempt_started'}}
        self.assertFalse(self.facts(journal=args['journal']+[attempt])[0]['ready'])
        for lock in ({'complete': True, 'held': True}, {'complete': False, 'held': False}):
            self.assertFalse(self.facts(lock_sample=lock)[0]['ready'])
        self.assertFalse(self.facts(now=args['now']+121)[0]['ready'])
        activity = dict(args['activity'], activity_confidence='uncertain')
        self.assertFalse(self.facts(activity=activity)[0]['ready'])

    def test_guard_precedes_any_codex_input_or_budget_reservation(self):
        trial = controller.Trial.__new__(controller.Trial)
        trial.wait_for_alpha_delivery = Mock(side_effect=TimeoutError('pending'))
        trial.ledger = Mock(side_effect=AssertionError('input admitted before guard'))
        trial.run = Mock()
        with self.assertRaisesRegex(TimeoutError, 'pending'):
            trial.input('codex', '%2', 'READY')
        trial.run.assert_not_called(); trial.ledger.assert_not_called()

    def test_run8_routing_and_assignment_publication(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run8'}):
            driver = runpy.run_path(str(controller.BASE/'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE/'run8')
        source = (controller.BASE/'steps.py').read_text()
        self.assertIn("OUT.name in ('run7', 'run8', 'run9')", source)

    def test_passive_lock_sample_uses_fdinfo(self):
        from run8_guard import passive_terminal_lock
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); lock = root/'alpha.lock'; lock.touch()
            proc = root/'proc/3'; (proc/'fd').mkdir(parents=True); (proc/'fdinfo').mkdir()
            (proc/'fd/4').symlink_to(lock)
            (proc/'fdinfo/4').write_text('pos: 0\nlock: 1: FLOCK ADVISORY WRITE 3 00:01:2 0 EOF\n')
            sample = passive_terminal_lock(lock, [{'pid': 3}], root/'proc')
            self.assertTrue(sample['complete']); self.assertTrue(sample['held'])
            (proc/'fdinfo/4').write_text('pos: 0\n')
            self.assertFalse(passive_terminal_lock(lock, [{'pid': 3}], root/'proc')['held'])

if __name__ == '__main__': unittest.main()
