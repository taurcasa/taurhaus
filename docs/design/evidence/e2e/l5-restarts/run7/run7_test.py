"""Run7 observer regressions; synthetic data, no real CLI or authentication."""
import ast
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import steps
import support

class Run7(unittest.TestCase):
    def test_step5_transport_does_not_require_read_reply_or_idle(self):
        # // Regression: e6fa7d0e conditioned transport proof on a seat following its read cursor.
        for seat,transport,stage in [('alpha','tmux','submitted'),('beta','app_server','native_enqueued')]:
            accepted={'message_id':'m','delivery_targets':[{'recipient':seat,'delivery_id':'d'}]}
            receipts=[{'stage':'attempt_started','attempt_id':'a','delivery_id':'d','recipient':seat},
                      {'stage':stage,'attempt_id':'a','delivery_id':'d','recipient':seat}]
            self.assertTrue(support.transport_proven(accepted,receipts,seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts,seat,transport,False))
            self.assertFalse(support.transport_proven(accepted,receipts[:1],seat,transport,True))
            self.assertTrue(support.transport_proven(accepted,receipts+[dict(receipts[0],attempt_id='b')],seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts+receipts[1:],seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts,'foreign',transport,True))

    def test_step5_calls_transport_settle(self):
        # // Regression: e6fa7d0e called settle(), which waited for alpha's reply before explicit paging.
        tree=ast.parse(Path(steps.__file__).read_text())
        branch=next(n for n in ast.walk(tree) if isinstance(n,ast.If) and ast.unparse(n.test)=='n == 5')
        source='\n'.join(ast.unparse(n) for n in branch.body)
        self.assertIn("transport_settle('mesh-backlog', seat)",source)
        self.assertNotIn("settle('mesh-backlog', seat)",source.replace('transport_settle','transport_observe'))

    def test_read_follows_empty_unread_page_to_done_with_same_filters(self):
        # // Regression: e6fa7d0e's seat stopped after done:false; explicit step6 must always drain.
        pages=iter([{'done':False,'cursor':'one','messages':[]},
                    {'done':False,'cursor':'two','messages':[{'id':'m'}]},
                    {'done':True,'messages':[]},{'done':False,'cursor':'journal','events':[]},{'done':True}])
        with patch.object(steps,'mesh',side_effect=lambda *_:json.dumps(next(pages))) as mesh,patch.object(steps,'snap'):
            steps.explicit_read('alpha','final')
        self.assertEqual([c.args[0] for c in mesh.call_args_list[:3]],[
            ['read','--unread','--mark-read','--json'],
            ['read','--unread','--mark-read','--json','--since','one'],
            ['read','--unread','--mark-read','--json','--since','two']])
        self.assertTrue(all(c.args[2]=='alpha' for c in mesh.call_args_list))

    def test_scratch_instruction_requires_cursor_completion(self):
        # // Regression: e6fa7d0e's harness instructions omitted unread pagination.
        tree=ast.parse(Path(__file__).with_name('controller.py').read_text())
        instruction=next(n.args[0].value for n in ast.walk(tree) if isinstance(n,ast.Call)
                         and isinstance(n.func,ast.Attribute) and n.func.attr=='write_text'
                         and 'project/AGENTS.md' in ast.unparse(n.func))
        for text in ['done: false','--since','cursor','done: true']:
            self.assertIn(text,instruction)


    def test_deferred_attempts_allow_one_exposure_on_both_boundaries(self):
        # // Regression: 540f23ea counted attempt_started retries as duplicate exposure.
        for seat, transport, stage in [('alpha','tmux','submitted'), ('beta','app_server','native_enqueued')]:
            accepted={'message_id':'m','delivery_targets':[{'recipient':seat,'delivery_id':'d'}]}
            receipts=[]
            for i in range(17):
                attempt={'stage':'attempt_started','attempt_id':str(i),'delivery_id':'d','recipient':seat}
                receipts += [attempt, dict(attempt,stage='pending',reason='pre_input_failure: IO error: delivery: thread_active')]
            exposing=dict(attempt,attempt_id='exposing')
            receipts += [exposing,dict(exposing,stage=stage)]
            self.assertTrue(support.transport_proven(accepted,receipts,seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts[:-2]+receipts[-1:],seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts+receipts[-1:],seat,transport,True))
            self.assertFalse(support.transport_proven(dict(accepted,delivery_targets=accepted['delivery_targets']*2),receipts,seat,transport,True))
            self.assertFalse(support.transport_proven(accepted,receipts[:-1]+[dict(receipts[-1],attempt_id='unpaired')],seat,transport,True))

    def test_step6_reconciles_retried_deliveries_and_rejects_duplicate_exposure(self):
        # // Regression: 540f23ea rejected first-boundary beta retries already accepted by step4.
        tree=ast.parse(Path(steps.__file__).read_text())
        branch=next(n for n in ast.walk(tree) if isinstance(n,ast.If) and ast.unparse(n.test)=='n == 6')
        code=compile(ast.Module(body=branch.body,type_ignores=[]),'step6','exec')
        for duplicate in [False,True]:
            journal=[]
            for seat in ['alpha','beta']:
                for label in ['baseline','taurhaus-backlog','mesh-backlog']:
                    mid=label+'-'+seat
                    journal.append({'event_type':'message_accepted','payload':{'message_id':mid,'delivery_targets':[{'recipient':seat,'delivery_id':mid}]}})
            def receipts(mid):
                seat=mid.rsplit('-',1)[1]
                attempts=[{'stage':'attempt_started','attempt_id':str(i),'delivery_id':mid,'recipient':seat} for i in range(19)]
                exposure=dict(attempts[-1],stage='native_enqueued' if seat=='beta' else 'submitted')
                return attempts+[exposure]*(2 if duplicate else 1)+[{'kind':'consumed_by_read','reader_name':seat}]
            with patch.object(steps,'explicit_read'), patch.object(steps,'read',side_effect=lambda name:{'message_id':name.removesuffix('-message.json')}), patch.object(steps,'rows',return_value=journal), patch.object(steps,'receipts',side_effect=receipts), patch.object(steps,'transport',side_effect=lambda seat:'app_server' if seat=='beta' else 'tmux'), patch.object(steps,'save'), patch.object(steps,'unchanged'), patch.object(steps,'checkpoint'):
                if duplicate:
                    with self.assertRaisesRegex(AssertionError,'exposure'):
                        exec(code,vars(steps))
                else:
                    exec(code,vars(steps))

    def test_diagnostics_classify_obligations_not_retry_count(self):
        # // Regression: 775085af published a harness attempt-count timeout as a Mesh defect.
        from types import SimpleNamespace
        tree=ast.parse(Path(__file__).with_name('diagnostics.py').read_text())
        functions=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='classify_step5']
        self.assertEqual(len(functions),1,'classification must be pure and import-safe')
        namespace={}
        exec(compile(ast.Module(body=functions,type_ignores=[]),'diagnostics','exec'),namespace)
        diagnostics=SimpleNamespace(**namespace)
        outcome={'outcome':'FAIL','classification':'product (mesh)','reason':'transport receipt and native witness missing beta'}
        accounting=[{'label':'mesh-backlog','seat':seat,'accepted_target_count':1,'attempt_count':17,'transport_count':1} for seat in ['alpha','beta']]
        corrected=diagnostics.classify_step5(outcome,accounting)
        self.assertEqual(corrected['classification'],'harness')
        self.assertEqual(corrected['outcome'],'FAIL')
        self.assertEqual(outcome['classification'],'product (mesh)')
        for field in ['accepted_target_count','transport_count']:
            broken=[dict(accounting[0],**{field:2}),accounting[1]]
            self.assertEqual(diagnostics.classify_step5(outcome,broken)['classification'],'product (mesh)')
        missing=[dict(accounting[0],transport_count=0),accounting[1]]
        self.assertEqual(diagnostics.classify_step5(outcome,missing)['classification'],'harness')
        self.assertEqual(diagnostics.classify_step5(outcome,[])['classification'],'harness')

    def test_owner_sampler_subtracts_scan_time_from_wait(self):
        # // Regression: 540f23ea inherited a fixed post-scan sleep that inflated census gaps.
        from tempfile import TemporaryDirectory
        from types import SimpleNamespace
        from unittest.mock import Mock
        tree=ast.parse(Path(__file__).with_name('controller.py').read_text())
        fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='observe_owners')
        for duration,expected in [(.2,.3),(.8,0)]:
            now=[10.0]
            def identities(): now[0]+=duration; return []
            stop=Mock();stop.is_set.side_effect=[False,True]
            with TemporaryDirectory() as tmp:
                env={'OUT':Path(tmp),'ROOT':Path(tmp),'TEAM':'fixture','owner_stop':stop,'identities':identities,'time':SimpleNamespace(monotonic=lambda:now[0],time=lambda:now[0]),'json':json,'clean':lambda x:x}
                exec(compile(ast.Module(body=[fn],type_ignores=[]),'observer','exec'),env)
                env['observe_owners']()
                self.assertAlmostEqual(stop.wait.call_args.args[0],expected)

    def test_support_direct_entrypoint_follows_all_tests(self):
        # // Regression: 540f23ea retained a mid-file unittest.main, silently skipping later classes.
        tree=ast.parse(Path(__file__).with_name('support_test.py').read_text())
        self.assertIsInstance(tree.body[-1],ast.If)
        self.assertIn('unittest.main()',ast.unparse(tree.body[-1]))

if __name__=='__main__':unittest.main()
