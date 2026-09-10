"""Offline guards; generated tempdirs only, never real credentials or CLIs."""
import tempfile
import unittest
from pathlib import Path
from preflight import auth_source, meter, require_headroom, require_resume_success

class LaneTests(unittest.TestCase):
    def test_authorized_default_auth_source_without_opening_it(self):
        # // Regression: 0267819e invented an auth environment prerequisite.
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            self.assertEqual(auth_source({}, home), home / '.codex/auth.json')

    def test_explicit_codex_home_is_the_only_override(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(auth_source({'CODEX_HOME':str(root/'account')}, root), root/'account/auth.json')

    def test_meter_deduplicates_a_turn_across_two_observers(self):
        result = meter(['t'], [{'turn_id':'t','input':100,'cached_input':20,'output':10}] * 2)
        self.assertEqual(result['paid_inputs'], 1)
        self.assertAlmostEqual(result['conservative_usd'], .000132)

    def test_missing_usage_blocks_the_next_paid_input(self):
        with self.assertRaisesRegex(AssertionError, 'unmetered'):
            require_headroom(meter(['t'], []), 1)

    def test_resume_terminal_status_does_not_hide_a_startup_refusal(self):
        # // Regression: 4f946ee5 treated terminal RPC status as whole-team success.
        with self.assertRaisesRegex(AssertionError, 'team-daemon'):
            require_resume_success({'resumed':True,'failed_members':[], 'started_team_daemon':False, 'team_daemon_warning':'startup refused'})
        with self.assertRaisesRegex(AssertionError, 'members'):
            require_resume_success({'resumed':False,'failed_members':[{'member_name':'beta'}], 'started_team_daemon':True})
        require_resume_success({'resumed':True,'failed_members':[], 'started_team_daemon':True, 'team_daemon_warning':None})

    def test_caps_include_automatic_startup_and_recovery(self):
        with self.assertRaisesRegex(AssertionError, 'input cap'):
            require_headroom(meter([str(n) for n in range(16)], [{'turn_id':str(n),'input':0,'output':0} for n in range(16)]), 1)
        with self.assertRaisesRegex(AssertionError, 'cost headroom'):
            require_headroom(meter(['t'], [{'turn_id':'t','input':190000,'output':0}]), 1)

if __name__ == '__main__': unittest.main()
