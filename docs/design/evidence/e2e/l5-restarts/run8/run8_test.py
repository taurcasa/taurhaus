"""Run8 warm-up and packet regression checks; no real CLI or credentials."""
import ast
from pathlib import Path
import unittest
B=Path(__file__).resolve().parent
class Run8(unittest.TestCase):
 def test_warmup_before_initialize_and_deadline(self):
  # // Regression: 540f23ea initialized two first-use seats against an unwarmed SQLite home.
  source=(B/'controller.py').read_text()
  self.assertIn('warm_scratch_home()',source)
  call=source.rindex('warm_scratch_home()')
  self.assertLess(call,source.index('result = operation("coordination.initialize_team"'))
  self.assertLess(source.index('paid_deadline ='),call)
 def test_warmup_requires_composer_clean_exit_and_sqlite(self):
  # // Regression: 540f23ea had no first-use migration barrier before concurrent starts.
  tree=ast.parse((B/'controller.py').read_text())
  fn=next((n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='warm_scratch_home'),None)
  self.assertIsNotNone(fn)
  source=ast.unparse(fn)
  for fragment in ['context left','/quit','*.sqlite','exit','warmup.json']:
   self.assertIn(fragment,source)
 def test_warmup_is_counted(self):
  # // Regression: 540f23ea's ledger only counted model turns and omitted first-use launches.
  source=(B/'controller.py').read_text()
  self.assertIn("accounted['paid_inputs'] += warmup_inputs",source)
 def test_audit_has_no_stale_attempt_claims(self):
  # // Regression: 775085af audit regenerated the retracted one-attempt verdict.
  source=(B/'audit.py').read_text()
  self.assertNotIn('Step 5 failed the explicit one-attempt-per-id requirement',source)
  self.assertNotIn('1.713-second',source)
if __name__=='__main__':unittest.main()
