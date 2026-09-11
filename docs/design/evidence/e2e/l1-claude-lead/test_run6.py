"""Run 6 regressions use generated observations, never credentials or a CLI."""
import json
import os
import runpy
import unittest
from unittest.mock import patch
import controller
import steps
from continuation_support import cumulative


class Run6Tests(unittest.TestCase):
    def test_generated_compact_and_native_rows_are_not_controller_inputs(self):
        # // Regression: 9e71cc15 transcript counting inflated run5's four inputs to nine.
        prior = {'controller_inputs': {'claude': 0, 'codex': 0},
                 'seat_usd_estimate': 0, 'seat_usd_upper_rate': 0}
        result = cumulative(prior, {'claude': 4, 'codex': 4}, .04, .6, observed_claude=9)
        self.assertEqual(result['controller_inputs']['claude'], 4)
        self.assertEqual(result['seat_usd_estimate'], .04)

    def test_assignment_is_created_through_mesh_before_snapshot(self):
        # // Regression: 9e71cc15 compacted a taskless lead; the real hook correctly skipped.
        operational = {'member_name': 'lead', 'task': {
            'id': '1', 'subject': 'Run6 native mailbox recovery', 'status': 'in_progress'}}
        record = {'recovery': {'last_delivered': {'card_key': {'contract': {
            'role_id': '', 'effective_role_revision': 'real-role-revision'}}}}}
        calls = []
        def action(value): calls.append(value)
        def read(name):
            if name == 'step5-task-create.json':
                return {'exit': 0, 'output': json.dumps({'id': '1'})}
            if name == 'step5-task-assign.json':
                # // Regression: 52de2ac4 parsed merged warning/stdout as bare JSON.
                return {'exit': 0, 'output': '[mesh] warning [LEGACY_LANE_METADATA]: task #1 has no lane metadata\n'
                        + json.dumps({'assignment_id': 'assignment-1'}) + '\n'}
            if name.endswith('operational/lead.json'): return operational
            if name.endswith('runtime/lead.json'): return record
        with patch.object(steps, 'STEP', 5, create=True), patch.object(steps, 'action', side_effect=action), \
                patch.object(steps, 'read', side_effect=read), patch.object(steps, 'save') as save, \
                patch.object(steps, 'snapshot'), patch.object(steps, 'wait',
                    side_effect=lambda test, *args, **kwargs: self.assertTrue(test())):
            result = steps.prepare_resumable_assignment()
        self.assertEqual(result['task'], operational['task'])
        self.assertEqual(result['role']['effective_role_revision'], 'real-role-revision')
        self.assertEqual([c['op'] for c in calls], ['mesh', 'mesh'])
        self.assertEqual(calls[0]['argv'][:2], ['task', 'create'])
        assign = calls[1]['argv']
        self.assertEqual(assign[:3], ['task', 'assign', '1'])
        for option, expected in [('--owner', 'lead'), ('--status', 'in_progress')]:
            self.assertEqual(assign[assign.index(option) + 1], expected)
        for option in ('--description', '--deliverable', '--first-step', '--completion-signal', '--review-route'):
            self.assertIn(option, assign)
        save.assert_called_with('step5-assignment-context.json', result)

    def test_json_parse_error_is_harness_even_with_healthy_product(self):
        # // Regression: 52de2ac4 labeled its parser failure product-owner-pending.
        error = json.JSONDecodeError('Expecting value', '[mesh] warning', 1)
        facts = {'daemon_alive': True, 'runtime_readable': True, 'activity_fresh': True}
        self.assertEqual(steps.classify_failure(error, facts), 'harness')

    def test_run6_routes_to_fresh_attempt(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run6'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run6')

    def test_requested_continuation_preserves_original_run6_packet(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run6/continuation'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run6/continuation')
        self.assertEqual(driver['LEDGER_NAME'], 'admission-ledger.json')
        self.assertTrue(controller.valid_evidence_label('run6/continuation'))
        for label in ('../run6', 'run6/../run5', '/tmp/continuation'):
            self.assertFalse(controller.valid_evidence_label(label))


if __name__ == '__main__': unittest.main()
