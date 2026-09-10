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

class Run5Busy(unittest.TestCase):
    def test_lock_busy_read_is_pending(self):
        # // Regression: 17ef6635 handled host member busy but aborted on lock busy.
        events=[]
        def busy(): raise RuntimeError('coordination.hosted_transcript: lock busy')
        self.assertIsNone(observe_host(busy,events.append))
        self.assertEqual(len(events),1)

    def test_busy_stop_retries_only_refusals(self):
        from support import retry_busy
        attempts=[]; clock=[0]
        def call():
            attempts.append(1)
            if len(attempts)<3: raise RuntimeError('HOST_OPERATION_FAILED: host member busy')
            return 'stopped'
        def sleep(seconds): clock[0]+=seconds
        self.assertEqual(retry_busy(call,lambda _:None,clock=lambda:clock[0],sleep=sleep),'stopped')
        self.assertEqual(len(attempts),3)

    def test_busy_deadline_allows_sixty_seconds(self):
        from support import retry_busy
        clock=[0]
        def call(): raise RuntimeError('lock busy')
        def sleep(seconds): clock[0]+=seconds
        with self.assertRaisesRegex(RuntimeError,'lock busy'):
            retry_busy(call,lambda _:None,clock=lambda:clock[0],sleep=sleep)
        self.assertGreaterEqual(clock[0],60)

    def test_real_refusal_is_not_retried(self):
        from support import retry_busy
        attempts=[]
        def call():
            attempts.append(1)
            raise RuntimeError('host unavailable')
        with self.assertRaisesRegex(RuntimeError,'host unavailable'):
            retry_busy(call,lambda _:None)
        self.assertEqual(len(attempts),1)

class Run6Backlog(unittest.TestCase):
    def evidence(self, member='alpha', reason='pending: runtime session dead'):
        accepted={'event_type':'message_accepted','payload':{'message_id':'m','delivery_targets':[{'recipient':member,'delivery_id':'d'}]}}
        health={'member':member,'last_defer_reason':'IO error: delivery: '+reason}
        return [accepted], health

    def test_stopped_backlog_needs_no_pending_receipt(self):
        # // Regression: 92899b62 inherited a receipt predicate for a scheduler refusal before receipt creation.
        from support import stopped_backlog
        for member, reason in [('alpha','pending: runtime session dead'),('beta','pending: native_host_not_live')]:
            rows, health=self.evidence(member,reason)
            self.assertTrue(stopped_backlog(rows,'m',member,'pending',health))

    def test_backlog_requires_recipient_acceptance_projection_and_health(self):
        from support import stopped_backlog
        rows, health=self.evidence()
        for args in [([], 'm','alpha','pending',health), (rows,'other','alpha','pending',health),
                     (rows,'m','beta','pending',health), (rows,'m','alpha','complete',health),
                     (rows,'m','alpha','pending',{'member':'alpha','last_defer_reason':'pending: activity not freshly idle'}),
                     (rows,'m','alpha','pending',{'member':'beta','last_defer_reason':'pending: runtime session dead'})]:
            self.assertFalse(stopped_backlog(*args))

    def test_any_transport_or_consumed_receipt_disproves_stopped_backlog(self):
        from support import stopped_backlog
        rows, health=self.evidence()
        for stage in ['submitted','consumed','native_enqueued','consumed_by_read']:
            receipt={'event_type':'receipt','payload':{'message_id':'m','recipient':'alpha','stage':stage}}
            self.assertFalse(stopped_backlog(rows+[receipt],'m','alpha','pending',health))

    def test_controller_polls_member_health_for_both_stopped_seats(self):
        # // Regression: 92899b62 stopped before beta's obligation and never inspected health.
        import controller
        from unittest.mock import patch
        lane=controller.Lane.__new__(controller.Lane)
        lane.markers={}; lane.message_ids={}; lane.acceptances={}; calls=[]
        def send(member,phase):
            lane.markers[phase,member]='marker'
            lane.message_ids[phase,member]='m'
            lane.acceptances[phase,member]={'projection':'pending'}
        def mesh(args,label):
            member='alpha' if 'alpha' in label else 'beta'; calls.append(member)
            reason='runtime session dead' if member=='alpha' else 'native_host_not_live'
            return f'[mesh] delivery {member}: deferred=IO error: delivery: pending: {reason} stale=false'
        def wait(test,reason,timeout=100):
            self.assertGreaterEqual(timeout,60); self.assertTrue(test(),reason)
        lane.send=send; lane.mesh=mesh; lane.wait=wait
        lane.receipts=lambda phase,member:self.evidence(member)[0]
        lane.replies=lambda member,marker:[]
        with patch.object(controller,'save'):
            lane.step3()
        self.assertEqual(calls,['alpha','beta'])

    def test_census_includes_native_sibling(self):
        # // Regression: 92899b62 census matched only codex, missing codex-code-mode-host.
        from support import seat_process
        for binary in ['codex','codex-code-mode-host','claude']:
            self.assertTrue(seat_process({'argv':['/scratch/bin/'+binary]}))
        for argv in [[], ['/scratch/bin/mesh'], ['/scratch/bin/taurhaus-daemon']]:
            self.assertFalse(seat_process({'argv':argv}))

    def test_resume_headroom_uses_metered_cost_not_all_tokens_at_output_rate(self):
        # // Regression: 92899b62 would stop step 4 at $0.08 + four $0.05 reservations despite $0.006 metered.
        from controller import require_headroom
        require_headroom({'paid_inputs':4,'unmetered':[], 'api_equivalent_usd':.006,'conservative_usd':.08},4)
        with self.assertRaisesRegex(AssertionError,'cost headroom'):
            require_headroom({'paid_inputs':4,'unmetered':[], 'api_equivalent_usd':.22,'conservative_usd':.23},1)

if __name__=='__main__': unittest.main()

