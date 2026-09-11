"""Run9 offline guards: generated scratch files and mocked commands only."""
import ast
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import controller
from support import classify_failure


class Warmup(unittest.TestCase):
    # // Regression: 4a4c65d8 launched both seats against a cold home; run8 hit the SQLite first-use race.
    def exercise(self, sqlite=True, exit_code='0', composer=True):
        with tempfile.TemporaryDirectory() as temporary:
            lane=object.__new__(controller.Lane)
            lane.root=Path(temporary); (lane.root/'codex').mkdir()
            lane.seat_starts=0; lane.observe=Mock(); lane.event=Mock()
            lane.started=set(); lane.identities=Mock(return_value=[])
            calls=[]
            def command(argv,label,**kwargs):
                calls.append((argv,label))
                if label=='warmup-launch': return '%1\n'
                if label=='warmup-composer':
                    return 'OpenAI Codex (v0.153.4)\n› Ask Codex to do anything\n' if composer else 'starting'
                if label=='warmup-quit':
                    (lane.root/'warmup-exit').write_text(exit_code)
                    if sqlite: (lane.root/'codex/state_5.sqlite').write_bytes(b'SQLite format 3\0'+b'\0'*64)
                return ''
            lane.command=command
            def wait(predicate,reason,timeout=100):
                self.assertGreaterEqual(timeout,60)
                if not predicate(): raise AssertionError(reason)
            lane.wait=wait
            with patch.object(controller,'save') as save:
                lane.warm_codex_home()
                self.assertEqual(lane.seat_starts,1)
                self.assertEqual(sum(label=='warmup-launch' for _,label in calls),1)
                self.assertTrue(any(label=='warmup-quit' and '/quit' in argv for argv,label in calls))
                result=next(c.args[1] for c in save.call_args_list if c.args[0]=='warmup.json')
                self.assertTrue(result['composer_seen'])
                self.assertEqual(result['exit_code'],0)
                self.assertEqual(result['input_reservation'],1)
                self.assertEqual(result['sqlite_files'],['state_5.sqlite'])

    def test_composer_clean_exit_sqlite_and_single_input(self): self.exercise()
    def test_missing_sqlite_rejected(self):
        with self.assertRaisesRegex(AssertionError,'warm-up.*SQLite'): self.exercise(sqlite=False)
    def test_failed_exit_rejected(self):
        with self.assertRaisesRegex(AssertionError,'warm-up.*exit'): self.exercise(exit_code='1')
    def test_no_composer_rejected(self):
        with self.assertRaisesRegex(AssertionError,'warm-up.*composer'): self.exercise(composer=False)
    def test_warmup_precedes_initialize(self):
        tree=ast.parse(Path(controller.__file__).read_text())
        main=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='main')
        calls=[n.func.attr for n in ast.walk(main) if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute)]
        self.assertIn('warm_codex_home',calls)
        self.assertLess(calls.index('setup'),calls.index('warm_codex_home'))


class Classification(unittest.TestCase):
    # // Regression: 4a4c65d8 matched bare cap; run9 keeps specific, bounded phrases and Mesh precedence.
    def test_unrelated_words_do_not_claim_harness_failure(self):
        self.assertEqual(classify_failure('uncapped capture failed'),'taurhaus')
        self.assertEqual(classify_failure('unmeteredness observer mismatch'),'taurhaus')
        self.assertEqual(classify_failure('Mesh refusal: metered cap exceeded'),'mesh')
