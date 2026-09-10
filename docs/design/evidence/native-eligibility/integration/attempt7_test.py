"""Offline budget regression: no harness, credentials, or sockets."""
import ast
from pathlib import Path
import tempfile
import unittest
from attempt6_support import ledger

class BudgetTests(unittest.TestCase):
    def check_budget(self, count, size=0):
        source = Path(__file__).with_name('attempt7-controller.py').read_text()
        function = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == 'budget_check')
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / 'large.txt').write_bytes(b'x' * size)
            events = []
            for n in range(count):
                events.extend([{'method':'turn/started','params':{'turn':{'id':str(n)}}},
                  {'method':'thread/tokenUsage/updated','params':{'threadId':'t','turnId':str(n),'tokenUsage':{'last':{'inputTokens':100,'cachedInputTokens':0,'outputTokens':1}}}}])
            import json
            scope = dict(ROOT=root, OUT=root, host_events=events, ledger=ledger, json=json)
            exec(compile(ast.Module(body=[function], type_ignores=[]), '<budget>', 'exec'), scope)
            scope['budget_check']()

    def test_large_evidence_does_not_stop_steps(self):
        # // Regression: c4735af5 stopped a valid step for an evidence size reserve.
        self.check_budget(10, 1100000)

    def test_sixteen_turns_allowed(self):
        self.check_budget(16)

    def test_seventeenth_turn_refused(self):
        with self.assertRaisesRegex(AssertionError, 'turn budget'):
            self.check_budget(17)

if __name__ == '__main__': unittest.main()
