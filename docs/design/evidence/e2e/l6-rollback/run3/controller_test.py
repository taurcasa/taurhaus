"""Offline guards for the authorized run3 controller; no process or credential access."""
import ast
from pathlib import Path
import unittest

SOURCE = Path(__file__).with_name('controller.py')

class ControllerContract(unittest.TestCase):
    def setUp(self):
        self.source = SOURCE.read_text()
        self.tree = ast.parse(self.source)
        self.methods = {n.name: ast.get_source_segment(self.source, n) for n in ast.walk(self.tree) if isinstance(n, ast.FunctionDef)}

    def test_operator_stop_precedes_format_and_ownership(self):
        # // Regression: 66acc618 omitted the owner's lifetime-lock release before downgrade.
        code = self.methods['remaining_steps']
        self.assertIn("['team-daemon','stop']", code)
        self.assertLess(code.index("['team-daemon','stop']"), code.index("['team','format'"))
        self.assertLess(code.index("['team','format'"), code.index("['team','delivery'"))
        self.assertIn('owner-stopped.json', code)
        self.assertIn('self.owner_window=False', code)

    def test_metering_does_not_gate_lifecycle(self):
        # // Regression: d75916b9 waited for complete metering before an operational rollback.
        self.assertNotIn("self.wait(lambda:self.budget", self.methods['remaining_steps'])

    def test_new_inputs_require_drained_seat(self):
        self.assertIn('self.assert_no_pending_delivery()', self.methods['response'])
        self.assertIn('self.assert_no_pending_delivery()', self.methods['send_marker'])

    def test_owner_census_is_subsecond_and_observational(self):
        self.assertIn('owner-census.jsonl', self.methods.get('observe_owner',''))
        self.assertIn('self.stop.wait(.5)', self.methods.get('observe_owner',''))
        self.assertNotIn('os.kill', self.methods.get('observe_owner',''))

class RuntimePredicates(unittest.TestCase):
    def test_owner_census_recognizes_actual_start_verb(self):
        from controller import Trial
        from unittest.mock import Mock
        import tempfile
        with tempfile.TemporaryDirectory() as root:
            trial=Trial.__new__(Trial);trial.root=Path(root)
            owner={'pid':12,'start_ticks':'8','argv':[str(trial.root/'home/.local/bin/mesh'),'team-daemon','start']}
            stop={'pid':13,'argv':[str(trial.root/'home/.local/bin/mesh'),'team-daemon','stop']}
            trial.identities=Mock(return_value=[owner,stop])
            self.assertEqual(trial.owner_census()['owners'],[owner])

    def test_explicit_alpha_read_drains_accepted_delivery(self):
        from controller import Trial
        from unittest.mock import Mock
        import tempfile
        with tempfile.TemporaryDirectory() as root:
            trial=Trial.__new__(Trial);trial.root=Path(root)
            trial.team.mkdir(parents=True);(trial.team/'config.json').write_text('{}')
            trial.config=Mock(return_value={'messaging_format':2})
            trial.journals=Mock(return_value=[{'event_type':'message_accepted','payload':{'message_id':'A','delivery_targets':[{'recipient':'alpha'}]}}])
            trial.delivery_rows=Mock(return_value=[{'payload':{'kind':'consumed_by_read','reader_name':'alpha'}}])
            trial.assert_no_pending_delivery()
            trial.delivery_rows.return_value=[{'payload':{'kind':'consumed_by_read','reader_name':'lead'}}]
            with self.assertRaisesRegex(AssertionError,'still pending'):trial.assert_no_pending_delivery()

if __name__ == '__main__': unittest.main()
