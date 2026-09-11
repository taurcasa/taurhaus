"""Offline run-4 admission fixtures; no credentials or real CLI calls."""
import unittest
import controller
import copy
import os
import runpy
from unittest.mock import patch
import continuation_support as support
import run3_driver
from continuation_support import cumulative
from controller_support import admit_input


class Run4Tests(unittest.TestCase):
    def test_run4_brief_review_uses_the_bounded_packet(self):
        # // Regression: 35a82a7d ignored --brief for run4 and exhausted the review budget.
        import review
        self.assertEqual(review.run4_review_files('run4d', True), ['run4d/review-brief.json'])
        self.assertIn('run4d/review-excerpts.json', review.run4_review_files('run4d', False))

    def test_run4d_routes_to_fresh_admission_without_overwriting_run4c(self):
        # // Regression: 5f9914d4 admitted run4c alone, rejecting the authorized next attempt.
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run4d'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run4d')
        self.assertEqual(driver['LEDGER_NAME'], 'admission-ledger.json')

    def test_run4c_routes_to_its_own_fresh_admission(self):
        # // Regression: 95620eb0 restricted the driver to the first run-4 attempt.
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run4c'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run4c')
        self.assertEqual(driver['LEDGER_NAME'], 'admission-ledger.json')

    def test_unknown_previous_attempt_spend_is_history_only(self):
        from run4_budget import fresh_ledger
        prior = {'controller_inputs': {'claude': 1, 'codex': 1},
                 'seat_usd_estimate': None, 'metering_complete': False}
        ledger = fresh_ledger(prior)
        self.assertEqual(ledger['closed_runs_history'], prior)
        current = cumulative(ledger, {'claude': 1, 'codex': 1}, .01, .1)
        self.assertEqual(current['controller_inputs'], {'claude': 1, 'codex': 1})
        self.assertEqual(current['seat_usd_upper_rate'], .1)
        self.assertTrue(admit_input('claude', 1, 1, .1, .5))

    def test_authorized_reset_preserves_history_and_enforces_fresh_caps(self):
        # // Regression: 52159c0c blocked run 4 on closed runs' admission counts.
        from run4_budget import fresh_ledger, RULING
        prior = {'cap_inputs': {'claude': 8, 'codex': 11},
                 'seat_usd_estimate': .10193487, 'seat_usd_upper_rate': 1.0758178,
                 'prior_runtime_seconds': 588.255963}
        original = copy.deepcopy(prior)
        ledger = fresh_ledger(prior)
        self.assertEqual(prior, original)
        self.assertEqual(ledger['closed_runs_history'], original)
        self.assertIn('Run 4 starts with a FRESH budget', RULING)
        self.assertEqual(ledger['authorization'], RULING)
        self.assertEqual(ledger['prior_runtime_seconds'], 0)
        totals = cumulative(ledger, {'claude': 0, 'codex': 0}, 0, 0)
        self.assertEqual(totals['seat_usd_upper_rate'], 0)
        self.assertTrue(admit_input('claude', **dict(claude_inputs=totals['controller_inputs']['claude'], codex_inputs=0, spent=0, next_turn=.5)))
        for tool, c, x, spent in [('claude', 8, 0, 0), ('codex', 0, 12, 0), ('claude', 0, 0, 1.6)]:
            with self.assertRaises(ValueError):
                admit_input(tool, c, x, spent, .5)

    def test_step1_waits_for_session_attribution_before_asserting(self):
        # // Regression: a94047bc asserted session_id immediately after initialize.
        elapsed = [0]
        def sleep(seconds): elapsed[0] += seconds
        def read(): return {'session_id': 'native-id' if elapsed[0] >= 3 else None, 'paneId': '%1'}
        result = support.wait_lead_identity(read, lambda: elapsed[0], sleep)
        self.assertEqual(result['session_id'], 'native-id')
        self.assertEqual(elapsed[0], 3)
        elapsed[0] = 0
        with self.assertRaisesRegex(TimeoutError, 'attribution'):
            support.wait_lead_identity(lambda: {'session_id': None, 'paneId': '%1'}, lambda: elapsed[0], sleep)
        self.assertGreaterEqual(elapsed[0], 65)

    def test_identity_assertion_does_not_claim_authentication_failed(self):
        # // Regression: b1ca7979 labeled every step-1 driver failure as auth unavailable.
        result = run3_driver.driver_failure(1)
        self.assertEqual(result['classification'], 'harness')
        self.assertNotIn('claude_auth_unavailable', result['reason'])

    def test_run4_pins_the_fixed_product_revisions(self):
        self.assertEqual(controller.TAURHAUS_BASE, 'a7e6db7e')
        self.assertEqual(controller.MESH_COMMIT, '310144d')


if __name__ == '__main__':
    unittest.main()
