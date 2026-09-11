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
            self.assertFalse(support.transport_proven(accepted,receipts+[dict(receipts[0],attempt_id='b')],seat,transport,True))
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

if __name__=='__main__':unittest.main()
