"""Offline synthetic regressions: no CLI launches or real harness home reads."""
import tempfile
import unittest
from unittest.mock import Mock, patch
from controller import Trial
from pathlib import Path
from support import native_runtime, retained_daemon_rows, attributed_idle, evidence_jsonl, pending_observation, ready_session

class Run3Tests(unittest.TestCase):
    def test_wait_classifies_only_its_failed_assertion(self):
        # // Regression: 57c8ff36 preset step 1's owner before probes could fail.
        trial = Trial.__new__(Trial)
        trial.classification = 'harness'
        trial.budget = Mock()
        trial.snapshot = Mock()
        with patch('controller.time.monotonic', side_effect=[0, 0, 91]), patch('controller.time.sleep'):
            with self.assertRaisesRegex(AssertionError, 'readiness missing'):
                trial.wait(lambda: False, 'readiness missing', 90, owner='taurhaus')
        self.assertEqual(trial.classification, 'taurhaus')
        trial.classification = 'harness'
        with patch('controller.time.monotonic', return_value=0):
            with self.assertRaisesRegex(RuntimeError, 'RPC fault'):
                trial.wait(Mock(side_effect=RuntimeError('RPC fault')), 'readiness missing', 90, owner='taurhaus')
        self.assertEqual(trial.classification, 'harness')

    def test_send_marker_accepts_banner_and_pretty_json(self):
        # // Regression: 57c8ff36 parsed send/read stdout with incompatible parsers.
        trial = Trial.__new__(Trial)
        trial.budget = Mock(return_value={'paid_inputs': 0, 'conservative_usd': 0})
        trial.record = Mock(return_value={'attachmentGeneration': 1})
        trial.save = Mock()
        trial.step = 2
        trial.reservations = []
        trial.mesh = Mock(return_value='Mesh notice\n{\n  "message_id": "Q-id"\n}\n')
        _, message_id = trial.send_marker('Q')
        self.assertEqual(message_id, 'Q-id')

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
    def test_pending_health_must_follow_this_acceptance_without_submission(self):
        # // Regression: 8e8f1287 demanded a journal receipt for pre-claim deferral.
        accepted={'event_type':'message_accepted','committed_at':'2026-09-10T00:00:02+00:00','payload':{'message_id':'Q'}}
        health={'heartbeat':'2026-09-10T00:00:01+00:00','last_defer_reason':'pending: activity not freshly idle'}
        self.assertIsNone(pending_observation([accepted],'Q',health))
        health['heartbeat']='2026-09-10T00:00:03+00:00'
        self.assertEqual(pending_observation([accepted],'Q',health)['source'],'scheduler_health (not a receipt)')
        submitted={'payload':{'message_id':'Q','stage':'submitted'}}
        self.assertIsNone(pending_observation([accepted,submitted],'Q',health))

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

if __name__=='__main__':unittest.main()
