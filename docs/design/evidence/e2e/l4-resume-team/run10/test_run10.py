"""Run10 acceptance guards; only generated data and mocked commands."""
import unittest
from unittest.mock import Mock, patch
import controller
import support

class RetainedStop(unittest.TestCase):
    # // Regression: fd5dd9f0 recorded run9's erased stop identity; #181 must retain it on the wire.
    def old(self):
        return {'session_id':'recorded','jsonl_path':'/scratch/rollout.jsonl','paneId':'%1'}
    def stopped(self):
        return {**self.old(),'health':'session_dead','daemon_pid':None,'panePid':None,'paneStartTime':None}
    def test_retained_identity_accepted(self):
        support.require_stopped_identity(self.old(),self.stopped())
    def test_erased_identity_or_live_handles_rejected(self):
        for change in [{'session_id':None},{'jsonl_path':None},{'paneId':None},
                       {'health':'healthy'},{'daemon_pid':12},{'panePid':13},{'paneStartTime':'42'}]:
            with self.subTest(change=change), self.assertRaises(AssertionError):
                support.require_stopped_identity(self.old(),{**self.stopped(),**change})
    def test_capture_happens_immediately_after_alpha_stop(self):
        lane=object.__new__(controller.Lane)
        lane.old={m:{**self.old(),'paneId':'%'+str(i)} for i,m in enumerate(['alpha','beta','lead'],1)}
        lane.identities=Mock(return_value=[]); lane.record=Mock(return_value=self.stopped())
        lane.event=Mock(); order=[]
        lane.rpc=lambda method,params:order.append(params['tmux_pane'])
        lane.wait=Mock(side_effect=RuntimeError('end observation'))
        with patch.object(controller,'save',side_effect=lambda name,value:order.append(name)):
            with self.assertRaisesRegex(RuntimeError,'end observation'): lane.step2()
        self.assertLess(order.index('%1'),order.index('step2-alpha-runtime-record.json'))
        self.assertLess(order.index('step2-alpha-runtime-record.json'),order.index('%2'))

class ResumeContract(unittest.TestCase):
    def test_fallback_is_rejected_even_if_final_launch_is_resume(self):
        old={'session_id':'old'}; new={'session_id':'new','paneId':'%2'}
        launch={'mode':'resume','command':"codex resume 'old'"}
        activity={'runtime_sessions':[{'session_id':'new','tmux_pane':'%2','state':'idle','activity_attribution':'attributed','pid':7}]}
        events=[{'event':'activity.state.changed','session_id':'new','pid':7,'to':'idle','source':'notify'},
                {'event':'launch.resume.fallback','member':'alpha'}]
        with self.assertRaisesRegex(AssertionError,'fallback'):
            support.require_alpha_resume(old,new,launch,activity,events)
    def test_metering_does_not_gate_resume_lifecycle(self):
        # // Regression: fd5dd9f0 inherited a headroom predicate before the lifecycle RPC.
        lane=object.__new__(controller.Lane)
        lane.ledger={'paid_inputs':4,'unmetered':['pending'],'api_equivalent_usd':.24}
        lane.seat_starts=3; lane.commands={}; lane.operation=Mock(); lane.record=Mock(return_value={})
        lane.allow_input=Mock(return_value=False)
        lane.step4()
        lane.operation.assert_called_once()
        self.assertEqual(lane.operation.call_args.args[0],'coordination.resume_team')
        lane.allow_input.assert_not_called()

class PendingDelivery(unittest.TestCase):
    def test_never_send_over_an_unfinished_recipient_delivery(self):
        accepted={'event_type':'message_accepted','payload':{'message_id':'m','delivery_targets':[{'recipient':'alpha'}]}}
        self.assertEqual(support.pending_deliveries([accepted],'alpha'),['m'])
        self.assertEqual(support.pending_deliveries([accepted],'beta'),[])
        for stage in ['submitted','native_enqueued','consumed_by_read']:
            receipt={'event_type':'receipt','payload':{'message_id':'m','recipient':'alpha','stage':stage}}
            self.assertEqual(support.pending_deliveries([accepted,receipt],'alpha'),[])
        receipt={'event_type':'receipt','payload':{'message_id':'m','recipient':'alpha','stage':'pending'}}
        self.assertEqual(support.pending_deliveries([accepted,receipt],'alpha'),['m'])

class Run10ObservedGuardRegression(unittest.TestCase):
    # // Regression: 99c59ab8 required jsonl_path before the scanner populated it and asserted liveness without polling.
    def test_late_rollout_path_binding_is_not_identity_loss(self):
        old={'session_id':'recorded','jsonl_path':None,'paneId':'%1'}
        stopped={**old,'jsonl_path':'/scratch/rollout.jsonl','health':'session_dead',
                 'panePid':None,'paneStartTime':None,'daemon_pid':None}
        support.require_stopped_identity(old,stopped)
    def test_immediate_capture_does_not_abort_before_stopping_other_seats(self):
        lane=object.__new__(controller.Lane)
        lane.old={m:{'session_id':m,'jsonl_path':None,'paneId':'%'+str(i)}
                  for i,m in enumerate(['alpha','beta','lead'],1)}
        lane.identities=Mock(return_value=[])
        lane.record=Mock(return_value={**lane.old['alpha'],'jsonl_path':'/scratch/rollout.jsonl',
                                      'health':'healthy','panePid':7})
        lane.event=Mock(); calls=[]
        lane.rpc=lambda method,params:calls.append(params['tmux_pane'])
        lane.wait=Mock(side_effect=RuntimeError('poll boundary'))
        with patch.object(controller,'save'):
            with self.assertRaisesRegex(RuntimeError,'poll boundary'): lane.step2()
        self.assertEqual(calls,['%1','%2','%3'])
        self.assertGreaterEqual(lane.wait.call_args.kwargs['timeout'],60)
