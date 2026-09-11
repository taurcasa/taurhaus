"""Inherited shared-harness offline checks from L2 run 5 at 2690352e."""
import tempfile
import unittest
from unittest.mock import Mock, patch
from pathlib import Path
from support import native_runtime, retained_daemon_rows, attributed_idle, evidence_jsonl, pending_observation, ready_session

class SharedHarnessTests(unittest.TestCase):
    def test_retired_pending_entry_points_refuse_invented_evidence(self):
        # // Regression: 955df28f left the superseded stage:pending helper callable.
        from support import pending_receipt
        row={'payload':{'message_id':'B','stage':'pending'}}
        with self.assertRaisesRegex(RuntimeError,'run2_rules.pending'):
            pending_observation([row],'B',{})
        with self.assertRaisesRegex(RuntimeError,'run2_rules.pending'):
            pending_receipt([row],'B')

    def test_preflight_cli_uses_standing_authorization_without_reading_credentials(self):
        # // Regression: 82e2655b omitted the authorized pin from the standalone CLI.
        import preflight
        from controller import AUTHORIZED_AUTH_SOURCE
        for source, expected in ((AUTHORIZED_AUTH_SOURCE, 0), ('/home/example/.codex/auth.json', 78)):
            with self.subTest(source=source), patch('sys.argv', ['preflight.py','--auth-source',source]), \
                 patch.object(Path,'is_symlink',return_value=False), patch.object(Path,'is_file',return_value=True), \
                 patch.object(Path,'read_bytes',side_effect=AssertionError('credential read')), \
                 patch.object(Path,'read_text',side_effect=AssertionError('credential read')), patch('builtins.print'):
                self.assertEqual(preflight.main(),expected)

    def test_controller_cli_selects_new_output_or_historical_default(self):
        # // Regression: 82e2655b hardcoded the already-committed run directory.
        import controller
        with tempfile.TemporaryDirectory() as root:
            output=Path(root)/'rerun'
            for options, expected in (([],controller.BASE/'run'), (['--out',str(output)],output)):
                trial=Mock(step=1,started=0,code=1)
                with self.subTest(options=options), patch('sys.argv',['controller.py','--auth-source','/tmp/auth.json',*options]), \
                     patch('controller.Trial',return_value=trial) as factory, \
                     patch('controller.signal.signal'), patch('controller.os.umask'):
                    self.assertEqual(controller.main(),0)
                    factory.assert_called_once_with(expected)
                    trial.teardown.assert_called_once_with()

    def test_controller_output_guard_preserves_existing_evidence(self):
        # // Regression: 82e2655b offered no selectable fresh evidence directory.
        from controller import Trial
        with tempfile.TemporaryDirectory() as root:
            output=Path(root)/'rerun'
            with patch('controller.tempfile.mkdtemp',return_value=root):
                trial=Trial(output)
                try:
                    trial.save('sentinel.json',{'retained':True})
                    original=(output/'sentinel.json').read_bytes()
                    with patch('controller.tempfile.mkdtemp',side_effect=AssertionError('scratch allocated before guard')):
                        with self.assertRaises(FileExistsError):Trial(output)
                    self.assertEqual((output/'sentinel.json').read_bytes(),original)
                    self.assertEqual(trial.out,output)
                finally:
                    trial.events.close()

    def test_transcript_account_quota_is_removed_but_turn_tokens_remain(self):
        # // Regression: b320480f retained token_count.rate_limits account usage.
        from support import clean
        value={'payload': {'info': {'total_token_usage': {'input_tokens': 100}},
                           'rate_limits': {'primary': {'used_percent': 65}, 'plan_type': 'pro'}}}
        self.assertEqual(clean(value), {'payload': {'info': {'total_token_usage': {'input_tokens': 100}}}})

    def test_run5_rejects_wrong_product_mesh_or_protocol_before_launch(self):
        from support import validate_candidate
        good = {'product_commit': 'a7e6db7e', 'product_diff': '',
                'mesh_commit': '310144d', 'mesh_diff': '', 'protocol': 27}
        validate_candidate(**good)
        for key, value in [('product_commit', 'old'), ('product_diff', 'src/changed'),
                           ('mesh_commit', 'old'), ('mesh_diff', 'descriptor changed'),
                           ('protocol', 26)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_candidate(**{**good, key: value})

    def test_complete_native_runtime_required_before_copy(self):
        # // Regression: 8e8f1287 copied codex alone, omitting code-mode runtime.
        with tempfile.TemporaryDirectory() as d:
            native=Path(d)/'codex'; native.write_text('synthetic')
            with self.assertRaises(FileNotFoundError):native_runtime(native)
            host=Path(d)/'codex-code-mode-host';host.write_text('synthetic host')
            self.assertEqual(native_runtime(native), [('codex',native),('codex-code-mode-host',host)])

    def test_complete_daemon_stream_preserves_shutdown_and_safe_rows(self):
        # // Regression: 8e8f1287 exported selected daemon events instead of full JSONL.
        rows=[{'event':'startup.ready'},{'event':'usage.fetched','account_usage':[{'secret':'hidden'}]}, {'event':'shutdown.complete'}, {'event':'startup.ready'}]
        kept=retained_daemon_rows(rows)
        self.assertEqual(len(kept),4)
        self.assertEqual(kept[-2]['event'],'shutdown.complete')
        self.assertNotIn('account_usage',str(kept))

    def test_ready_requires_attributed_fresh_idle(self):
        record={'session_id':None,'jsonl_path':None}
        activity={'activity_confidence':'idle','observed_at':'2026-09-10T00:00:00+00:00'}
        self.assertFalse(attributed_idle(record,activity,now=1788998400))
        record['session_id']='scratch-thread'
        self.assertTrue(attributed_idle(record,activity,now=1788998400))
        self.assertFalse(attributed_idle(record,activity,now=1788998521))

    def test_ready_runtime_row_matches_seat_identity_and_attribution(self):
        record={'session_id':'thread','paneId':'%2'}
        row={'session_id':'thread','tmux_pane':'%2','state':'idle','activity_attribution':'attributed'}
        snapshot={'runtime_sessions':[row],'degraded':False}
        self.assertEqual(ready_session(record,snapshot),row)
        row['tmux_pane']='%3'
        self.assertIsNone(ready_session(record,snapshot))
        row['tmux_pane']='%2';row['activity_attribution']='none'
        self.assertIsNone(ready_session(record,snapshot))
        row['activity_attribution']='attributed';snapshot['degraded']=True
        self.assertIsNone(ready_session(record,snapshot))

    def test_journal_export_is_complete_jsonl(self):
        self.assertEqual(evidence_jsonl([{'a':1},{'a':2}]),'{"a": 1}\n{"a": 2}\n')
