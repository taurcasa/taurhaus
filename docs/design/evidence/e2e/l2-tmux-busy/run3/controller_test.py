"""Offline synthetic regressions: no CLI launches or real harness home reads."""
import tempfile
import unittest
from pathlib import Path
from support import native_runtime, retained_daemon_rows, attributed_idle, evidence_jsonl, pending_observation, ready_session

class Run3Tests(unittest.TestCase):
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
