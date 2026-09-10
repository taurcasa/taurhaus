"""Offline trial guards. Synthetic files only; never imports the live controller."""
import ast
from pathlib import Path
import shutil
import tempfile
import unittest

B = Path(__file__).parent

def helper(file, name):
    tree = ast.parse((B / file).read_text())
    node = next((n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name), None)
    assert node is not None, 'missing helper: ' + name
    scope = {'Path': Path, 'shutil': shutil}
    exec(compile(ast.Module(body=[node], type_ignores=[]), file, 'exec'), scope)
    return scope[name]

class TrialGuards(unittest.TestCase):
    def test_complete_native_runtime(self):
        # // Regression: 5c4132a9 inherited a codex-only copy, omitting code-mode-host.
        copy = helper('attempt10-controller.py', 'copy_native_runtime')
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
        finalize = helper('attempt10_support.py', 'finalize_metering')
        row = {'generations': [], 'unmetered_turn_ids': ['started'],
               'api_equivalent_usd': 0, 'conservative_usd': 0}
        result = finalize(row)
        self.assertIsNone(result['api_equivalent_usd'])
        self.assertIsNone(result['conservative_usd'])
        self.assertFalse(result['metering_complete'])

    def test_rollback_requires_attributed_idle(self):
        # // Regression: e98ffd7a reached an unattributed pane; delivery stayed pending.
        check = helper('attempt10_support.py', 'validate_tmux_activity')
        good = {'runtime_sessions':[{'member_name':'seat','cli_tool':'codex','pid':123,
            'session_id':'new-thread','tmux_pane':'%18','state':'idle',
            'activity_attribution':'attributed','activity_confidence':'medium'}]}
        check(good, '%18')
        for key, value in [('session_id',None),('pid',0),('state','busy'),
                           ('activity_attribution','none'),('activity_confidence','uncertain')]:
            bad = {'runtime_sessions':[dict(good['runtime_sessions'][0], **{key:value})]}
            with self.assertRaises(AssertionError): check(bad,'%18')

if __name__ == '__main__': unittest.main()
