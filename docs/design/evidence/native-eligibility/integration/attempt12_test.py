"""Offline trial guards. Synthetic files only; never imports the live controller."""
import ast
import json
from types import SimpleNamespace
from unittest.mock import Mock
from pathlib import Path
import shutil
import tempfile
import unittest

B = Path(__file__).parent

def helper(file, name, bindings=None):
    tree = ast.parse((B / file).read_text())
    node = next((n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name), None)
    assert node is not None, 'missing helper: ' + name
    scope = {'Path': Path, 'shutil': shutil, **(bindings or {})}
    exec(compile(ast.Module(body=[node], type_ignores=[]), file, 'exec'), scope)
    return scope[name]

class TrialGuards(unittest.TestCase):
    def test_complete_native_runtime(self):
        # // Regression: 5c4132a9 inherited a codex-only copy, omitting code-mode-host.
        copy = helper('attempt12-controller.py', 'copy_native_runtime')
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp)/'source', Path(tmp)/'bin'
            src.mkdir(); dst.mkdir()
            for name in ['codex', 'codex-code-mode-host']:
                (src/name).write_text('synthetic '+name)
            copy(src/'codex', dst)
            for name in ['codex', 'codex-code-mode-host']:
                self.assertEqual((dst/name).read_text(), 'synthetic '+name)
                self.assertEqual((dst/name).stat().st_mode & 0o777, 0o700)

    def test_missing_metering_is_not_zero_spend(self):
        # // Regression: 5c4132a9 ledger reports a zero sum when no usage was captured.
        finalize = helper('attempt12_support.py', 'finalize_metering')
        row = {'generations': [], 'unmetered_turn_ids': ['started'],
               'api_equivalent_usd': 0, 'conservative_usd': 0}
        result = finalize(row)
        self.assertIsNone(result['api_equivalent_usd'])
        self.assertIsNone(result['conservative_usd'])
        self.assertFalse(result['metering_complete'])
        self.assertEqual(finalize(result), result)

    def test_rollback_requires_attributed_idle(self):
        # // Regression: e98ffd7a reached an unattributed pane; delivery stayed pending.
        check = helper('attempt12_support.py', 'validate_tmux_activity')
        good = {'runtime_sessions':[{'member_name':'seat','cli_tool':'codex','pid':123,
            'session_id':'new-thread','tmux_pane':'%18','state':'idle',
            'activity_attribution':'attributed','activity_confidence':'medium'}]}
        check(good, '%18', {'source':'launch_ready'})
        with self.assertRaises(AssertionError): check(good, '%18', {'source':'none'})
        for key, value in [('session_id',None),('pid',0),('state','busy'),
                           ('activity_attribution','none'),('activity_confidence','uncertain')]:
            bad = {'runtime_sessions':[dict(good['runtime_sessions'][0], **{key:value})]}
            with self.assertRaises(AssertionError): check(bad,'%18', {'source':'notify'})


class ControllerRaces(unittest.TestCase):
    # // Regression: 15633a94 reused fatal transcript polling and a stale rollback
    # record; ordinary startup contention/readiness races aborted paid evidence.
    def poll_fixture(self, out, responses):
        clock = SimpleNamespace(now=10)
        clock.monotonic = lambda: clock.now
        clock.sleep = lambda seconds: setattr(clock, 'now', clock.now + seconds)
        replies = iter(responses)
        def rpc(method, params, allow_error=False):
            response = next(replies)
            if allow_error: return response
            if 'error' in response: raise RuntimeError(str(response['error']))
            return response['result']
        scope = dict(time=clock, rpc=Mock(side_effect=rpc), log=Mock(),
            TEAM='integration', MEMBER='seat', OUT=out, json=json,
            host_poll_enabled=True, last_host_poll=0, previous_host_events=[],
            host_events=[], new_events=lambda old, new: [e for e in new if e not in old],
            retain_host_event=lambda e: True, retained_view=lambda v: v, budget_check=Mock())
        poll = helper('attempt12-controller.py', 'poll_host', scope)
        return poll, poll.__globals__

    def test_busy_poll_defers_without_losing_or_duplicating_events(self):
        busy = {'error': {'code':'HOST_OPERATION_FAILED', 'message':'host member busy'}}
        ok = {'result': {'events':[{'method':'turn/completed'}]}}
        with tempfile.TemporaryDirectory() as tmp:
            poll, scope = self.poll_fixture(Path(tmp), [busy, ok, ok])
            scope['previous_host_events'] = [{'method':'turn/started'}]
            self.assertFalse(poll(force=True))
            self.assertEqual(scope['host_events'], [])
            self.assertEqual(scope['previous_host_events'], [{'method':'turn/started'}])
            self.assertEqual(list(Path(tmp).iterdir()), [])
            self.assertEqual(scope['log'].call_args.args, ('host_poll_deferred',))
            poll()  # Busy read still participates in the one-second rate limit.
            self.assertEqual(scope['rpc'].call_count, 1)
            scope['time'].sleep(1)
            self.assertTrue(poll())
            poll(force=True)
            self.assertEqual(scope['host_events'], ok['result']['events'])
            self.assertEqual(len((Path(tmp)/'host-events.jsonl').read_text().splitlines()), 1)

    def test_flock_refusal_defers_and_preserves_cursor(self):
        # // Regression: 19ffe981 handled only the seat mutex; a native compact
        # hook's cross-process flock still aborted the hosted-read controller.
        busy = {'error': {'code': 'HOST_OPERATION_FAILED',
                         'message': 'Conflict: host operation deferred: lock busy'}}
        ok = {'result': {'events': [{'method': 'turn/completed'}]}}
        with tempfile.TemporaryDirectory() as tmp:
            poll, scope = self.poll_fixture(Path(tmp), [busy, ok, ok])
            scope['previous_host_events'] = [{'method': 'turn/started'}]
            self.assertFalse(poll(force=True))
            self.assertEqual(scope['previous_host_events'], [{'method': 'turn/started'}])
            self.assertEqual(scope['host_events'], [])
            self.assertEqual(list(Path(tmp).iterdir()), [])
            poll()
            self.assertEqual(scope['rpc'].call_count, 1)
            scope['time'].sleep(1)
            self.assertTrue(poll())
            self.assertTrue(poll(force=True))
            self.assertEqual(scope['host_events'], ok['result']['events'])
            self.assertEqual(len((Path(tmp)/'host-events.jsonl').read_text().splitlines()), 1)

    def test_both_refusals_get_full_window_and_late_success(self):
        # // Regression: 19ffe981 omitted flock contention from bounded retries.
        for message in ['host member busy', 'Conflict: host operation deferred: lock busy']:
            busy = {'error': {'code': 'HOST_OPERATION_FAILED', 'message': message}}
            for succeeds in [False, True]:
                with self.subTest(message=message, succeeds=succeeds), tempfile.TemporaryDirectory() as tmp:
                    responses = [busy] * 119 + ([{'result': {'events': []}}] if succeeds else [busy])
                    poll, scope = self.poll_fixture(Path(tmp), responses)
                    scope['poll_host'] = poll
                    wait = helper('attempt12-controller.py', 'wait_host_poll', scope)
                    if succeeds:
                        wait()
                        self.assertEqual(scope['time'].now, 129)
                    else:
                        with self.assertRaisesRegex(AssertionError, 'deadline'): wait()
                        self.assertEqual(scope['time'].now, 130)
                    self.assertEqual(scope['rpc'].call_count, 120)

    def test_other_host_refusals_remain_fatal(self):
        for code, message in [('HOST_OPERATION_FAILED','NOT_HOSTED'),
                              ('HOST_OPERATION_FAILED','host attachment changed'),
                              ('HOST_OPERATION_FAILED','failed: host member busy'),
                              ('HOST_OPERATION_FAILED','member is not hosted'),
                              ('HOST_OPERATION_FAILED','Conflict: host operation deferred: lock busy extra'),
                              ('OTHER','Conflict: host operation deferred: lock busy'),
                              ('OTHER','host member busy')]:
            with self.subTest(code=code, message=message), tempfile.TemporaryDirectory() as tmp:
                poll, _ = self.poll_fixture(Path(tmp), [{'error':dict(code=code, message=message)}])
                with self.assertRaisesRegex(RuntimeError, message): poll(force=True)

    def test_forced_startup_wait_retries_and_has_a_deadline(self):
        with tempfile.TemporaryDirectory() as tmp:
            _, scope = self.poll_fixture(Path(tmp), [])
            scope['poll_host'] = Mock(side_effect=[False, False, True])
            wait = helper('attempt12-controller.py', 'wait_host_poll', scope)
            wait()
            self.assertEqual(scope['time'].now, 12)
            self.assertEqual(scope['poll_host'].call_count, 3)
            scope['poll_host'] = Mock(return_value=False)
            wait = helper('attempt12-controller.py', 'wait_host_poll', scope)
            with self.assertRaisesRegex(AssertionError, 'step 1.*host member busy'): wait()
            self.assertEqual(scope['time'].now, 132)
            self.assertEqual(scope['poll_host'].call_count, 120)
            self.assertGreaterEqual(scope['budget_check'].call_count, 120)
            scope['poll_host'] = Mock(side_effect=RuntimeError('NOT_HOSTED'))
            with self.assertRaisesRegex(RuntimeError, 'NOT_HOSTED'):
                helper('attempt12-controller.py', 'wait_host_poll', scope)()

    def test_final_drain_retries_busy_without_reading_absent_transcript(self):
        tree = ast.parse((B/'attempt12-controller.py').read_text())
        loop = next(n for n in ast.walk(tree) if isinstance(n, ast.For)
                    and ast.unparse(n.iter) == 'range(30)')
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            def poll(force=False):
                if poller.call_count == 1: return False
                (out/'hosted-transcript.json').write_text(json.dumps({'thread':{'status':{'type':'idle'}}}))
                return True
            poller = Mock(side_effect=poll)
            exec(compile(ast.Module(body=[loop], type_ignores=[]), 'final-drain', 'exec'),
                 dict(poll_host=poller, OUT=out, json=json, time=SimpleNamespace(sleep=Mock())))
            self.assertEqual(poller.call_count, 2)

    def test_rollback_refreshes_null_and_missing_activity_until_ready(self):
        tree = ast.parse((B/'attempt12-steps.py').read_text())
        body = next(n.body for n in ast.walk(tree) if isinstance(n, ast.If)
                    and any(isinstance(c, ast.FunctionDef) and c.name == 'attributed' for c in n.body))
        start = next(i for i,n in enumerate(body) if isinstance(n, ast.FunctionDef) and n.name == 'attributed')
        end = next(i for i,n in enumerate(body[start:], start) if isinstance(n, ast.Assign)
                   and any(isinstance(t, ast.Name) and t.id == 'marker' for t in n.targets))
        wrapper = ast.parse('def readiness():\n    global after').body[0]
        wrapper.body += body[start:end] + ast.parse('return after').body
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            record = out/'team/runtime/seat.json'; record.parent.mkdir(parents=True)
            activity = out/'activity.json'
            refreshed = dict(paneId='%18', activitySnapshotPath=str(activity))
            attempts = []
            def wait_for(predicate, why, timeout):
                self.assertEqual(timeout, 120)
                for rec, content in [(dict(paneId='%18',activitySnapshotPath=None),None),
                                     (refreshed,None), (refreshed,{'source':'none'}),
                                     (refreshed,{'source':'launch_ready'})]:
                    record.write_text(json.dumps(rec))
                    if content: activity.write_text(json.dumps(content))
                    attempts.append(predicate())
                self.assertEqual(attempts, [False,False,False,True])
            def validate(snapshot, pane, value):
                self.assertEqual(pane, '%18')
                assert value['source'] in ['launch_ready','notify']
            saved = Mock()
            scope = dict(OUT=out, Path=Path, action=Mock(), wait_for=wait_for,
                         read_json=lambda p: json.loads(p.read_text()), save=saved,
                         validate_tmux_activity=validate)
            (out/'step7-activity-before.json').write_text('{}')
            exec(compile(ast.fix_missing_locations(ast.Module(body=[wrapper], type_ignores=[])),
                         'rollback-readiness', 'exec'), scope)
            scope['after'] = dict(paneId='%18',activitySnapshotPath=None)
            result = scope['readiness']()
            self.assertEqual(result, refreshed)
            saved.assert_any_call('step7-mesh-activity.json', {'source':'launch_ready'})
            self.assertGreaterEqual(scope['action'].call_args_list.count((({'op':'snapshot'},), {})), 4)

    def test_cleanup_verdict_precedes_sanitize_and_binary_files_are_skipped(self):
        # // Regression: 15633a94 put text decoding ahead of the cleanup verdict.
        tree = ast.parse((B/'attempt12-controller.py').read_text())
        final = next(n.finalbody for n in tree.body if isinstance(n, ast.Try) and n.finalbody)
        start = next(i for i,n in enumerate(final) if ast.unparse(n) == 'EVENTS.close()') + 1
        code = compile(ast.Module(body=final[start:], type_ignores=[]), 'sanitize', 'exec')
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out/'pane.bin').write_bytes(b'\xff\xfe')
            (out/'note.txt').write_text('synthetic sensitive value')
            scope = dict(OUT=out, json=json, clean=lambda s:s.replace('sensitive','redacted'),
                         survivors=[123], port_closed=True, cleanup={}, exit_code=0)
            exec(code, scope)
            self.assertEqual(scope['exit_code'], 2)
            self.assertEqual((out/'pane.bin').read_bytes(), b'\xff\xfe')
            self.assertEqual((out/'note.txt').read_text(), 'synthetic redacted value')
            (out/'invalid.json').write_text('{')
            scope['exit_code'] = 0
            with self.assertRaises(json.JSONDecodeError): exec(code, scope)
            self.assertEqual(scope['exit_code'], 2)

if __name__ == '__main__': unittest.main()
