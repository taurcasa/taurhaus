"""Offline harness guards: generated files only; no CLI, auth, or network access."""
import tempfile
import unittest
from pathlib import Path
from support import copy_native_runtime, sanitize_log_rows, classify_failure, reconciled_spend, observe_host, rollout_usage

class Guards(unittest.TestCase):
    def test_complete_native_runtime(self):
        # Regression: 4f946ee5 copied only codex; 0.153.4 also requires its native sibling.
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); package=root/'package'; entry=package/'bin/codex.js'
            entry.parent.mkdir(parents=True); entry.touch()
            native=package/'node_modules/@openai/codex-linux-x64/vendor/triple/bin'
            native.mkdir(parents=True)
            for name in ['codex','codex-code-mode-host']: (native/name).write_text(name)
            dest=root/'scratch'; dest.mkdir()
            copy_native_runtime(entry,dest)
            self.assertEqual(sorted(p.name for p in dest.iterdir()),['codex','codex-code-mode-host'])
            self.assertEqual((dest/'codex-code-mode-host').stat().st_mode & 0o777,0o700)
    def test_missing_sibling_copies_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); entry=root/'bin/codex.js'; entry.parent.mkdir(); entry.touch()
            native=root/'node_modules/@openai/codex-linux-x64/vendor/triple/bin'; native.mkdir(parents=True)
            (native/'codex').touch(); dest=root/'scratch'; dest.mkdir()
            with self.assertRaises(AssertionError): copy_native_runtime(entry,dest)
            self.assertEqual(list(dest.iterdir()),[])
    def test_complete_log_retains_repeated_diagnostics_excludes_usage(self):
        # Regression: 4f946ee5 exported only selected first/last daemon records.
        records=[{'event':'startup','message':'ready'}, {'event':'hosted.launch.failed','exit_status':'exit status: 1','stderr_tail':'runtime denied'}]*3
        actual=sanitize_log_rows(records+[{'event':'usage.fetched','account_id':'secret'}])
        self.assertEqual(actual,records)
    def test_opt_in_refusal_is_mesh(self):
        # Regression: f62bb158's generic classifier missed Mesh opt-in refusals.
        self.assertEqual(classify_failure('Backend error: mesh team activation failed: error: IO error: delivery: quiescent required before opt-in: IO error: delivery: team owner already holds lifetime lock'), 'mesh')
    def test_unmetered_turn_never_reports_zero_total(self):
        # Regression: f62bb158's raw meter subtotal was zero with one unmetered turn.
        result=reconciled_spend({'conservative_usd':0, 'unmetered':['turn-1']})
        self.assertIsNone(result['total_usd'])
        self.assertFalse(result['cap_verified'])
        self.assertEqual(result['metered_subtotal_usd'],0)
    def test_native_runtime_failure_is_harness(self):
        self.assertEqual(classify_failure('launch_host: app-server exited before transport readiness; stderr: bwrap: Creating new namespace failed: Operation not permitted'),'harness')
        self.assertEqual(classify_failure('launch_host: app-server exited before transport readiness; exit status: 1'),'taurhaus')
        self.assertEqual(classify_failure('resume team-daemon startup refused'),'mesh')

class CumulativeUsage(unittest.TestCase):
    def test_counts_every_response_in_a_tool_turn_once(self):
        # // Regression: e13eb5ff (original f62bb158) retained only last response usage per turn.
        def event(kind, **values): return {'type':'event_msg','payload':{'type':kind,**values}}
        def count(i,o): return event('token_count',info={'total_token_usage':{'input_tokens':i,'cached_input_tokens':0,'output_tokens':o}})
        rows=[event('task_started',turn_id='a'),count(10,2),count(25,5),event('task_started',turn_id='b'),count(40,9)]
        result=rollout_usage(rows,'session')
        self.assertEqual([(r['turn_id'],r['input'],r['output']) for r in result],[('a',25,5),('b',15,4)])

class BusyObservation(unittest.TestCase):
    def test_busy_read_keeps_cleanup_polling(self):
        # // Regression: e13eb5ff (original f62bb158) aborted cleanup on a transient busy transcript read.
        events=[]
        def busy(): raise RuntimeError('coordination.hosted_transcript: HOST_OPERATION_FAILED: host member busy')
        self.assertIsNone(observe_host(busy, events.append))
        self.assertEqual(len(events),1)
        view={'events':[], 'thread':{'status':{'type':'idle'}}}
        self.assertEqual(observe_host(lambda:view, events.append),view)

    def test_other_read_errors_remain_failures(self):
        def broken(): raise RuntimeError('host unavailable')
        with self.assertRaisesRegex(RuntimeError,'host unavailable'):
            observe_host(broken, lambda _:None)

if __name__=='__main__': unittest.main()

