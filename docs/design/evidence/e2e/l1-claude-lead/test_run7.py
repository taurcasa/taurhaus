"""Generated-data tests only: no credentials, runtime CLI, or daemon."""
import json
import os
import runpy
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch
import controller


class Run7Tests(unittest.TestCase):
    def test_run7_routes_to_fresh_attempt(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run7'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run7')

    def test_publishes_real_assignment_once_without_recovery_state(self):
        # // Regression: bf7ee51a run6 continuation lacked the app snapshot publisher.
        footer = {'execution_mode': 'Measure', 'file_ownership_boundary': [],
                  'adjacent_fix_policy': 'No adjacent fixes.',
                  'validation_expectation': 'Controller validates native recovery.',
                  'response_expectation': 'Respond once to fresh native mail.'}
        labels = ['Execution mode', 'File-ownership boundary', 'Adjacent-fix policy',
                  'Validation expectation', 'Response expectation']
        description = 'Real objective\n' + '\n'.join(
            label + ': ' + (json.dumps(value) if isinstance(value, list) else value)
            for label, value in zip(labels, footer.values()))
        task = {'id': '9', 'subject': 'Real assignment', 'status': 'in_progress',
                'owner': 'lead', 'description': description,
                'metadata': {'assigned_at': '2026-09-11T00:00:00.123Z', 'assignment_id': 'real-assignment'}}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            task_path = root/'claude/tasks/l1-claude-lead/9.json'
            task_path.parent.mkdir(parents=True); task_path.write_text(json.dumps(task))
            trial = controller.Trial.__new__(controller.Trial)
            trial.root = root; trial.rpc = Mock(return_value={'published': 1})
            trial.save = Mock()
            trial.publish_assignment({'task_id': '9', 'assignment_id': 'real-assignment'})
        trial.rpc.assert_called_once()
        method, params = trial.rpc.call_args.args
        self.assertEqual(method, 'coordination.publish_operational_snapshots')
        self.assertEqual(len(params['publications']), 1)
        publication = params['publications'][0]; snapshot = publication['snapshot']
        self.assertEqual(snapshot['task'], {**{k:task[k] for k in ('id','subject','status','owner')},
                                           'assigned_at': task['metadata']['assigned_at']})
        self.assertEqual(snapshot['assignment_footer'], footer)
        self.assertEqual(snapshot['working_set']['project_path'], str(root/'project'))
        self.assertEqual(publication['task_state_changed_at'], task['metadata']['assigned_at'])
        self.assertNotIn('recovery_card', snapshot)
        self.assertEqual(snapshot['ownership'], {'override_allowed': False, 'active_override_reason': None})
        saved = {call.args[0]: call.args[1] for call in trial.save.call_args_list}
        self.assertEqual(saved['step5-publication-request.json'], {'method': method, 'params': params})
        self.assertEqual(saved['step5-publication-response.json'], {'published': 1})
        self.assertEqual(saved['step5-task-record.json'], task)


if __name__ == '__main__': unittest.main()
