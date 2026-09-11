"""Generated-data checks only; no live homes, subprocesses, or credentials."""
import unittest
from continuation_support import cumulative, compact_hook_registered
from controller_support import sanitize

class ContinuationTests(unittest.TestCase):
    def test_previous_paid_trial_cannot_reset_on_restart(self):
        # // Regression: ff8dc10c controller initialized all input/spend counters to zero.
        prior={'controller_inputs':{'claude':1,'codex':1},'seat_usd_estimate':.02195125,'seat_usd_upper_rate':.06416}
        got=cumulative(prior,{'claude':2,'codex':3},.1,.5)
        self.assertEqual(got['controller_inputs'],{'claude':3,'codex':4})
        self.assertAlmostEqual(got['seat_usd_upper_rate'],.56416)
    def test_hook_requires_compact_matcher_and_real_command(self):
        # // Regression: ff8dc10c daemon-only setup omitted the production installer.
        self.assertFalse(compact_hook_registered({}))
        self.assertFalse(compact_hook_registered({'SessionStart':[{'matcher':'startup','hooks':[{'type':'command','command':'hook'}]}]}))
        self.assertTrue(compact_hook_registered({'SessionStart':[{'matcher':'compact','hooks':[{'type':'command','command':'/tmp/trial/hook'}]}]}))
    def test_scratch_home_subpath_is_not_operator_home(self):
        # // Regression: ff8dc10c sanitizer redacted /home inside a scratch path.
        self.assertEqual(sanitize('/tmp/trial/home/.local/bin/mesh'),'/tmp/trial/home/.local/bin/mesh')
    def test_authentication_result_is_preserved_but_secret_is_not(self):
        # // Regression: ff8dc10c auth substring filtering discarded a success subcheck.
        got=sanitize({'claude_authenticated_and_team_bound':'PASS','auth':'secret'})
        self.assertEqual(got,{'claude_authenticated_and_team_bound':'PASS'})
    def test_control_token_is_removed_but_usage_is_preserved(self):
        # // Regression: ff8dc10c sanitizer retained exact token keys in control state.
        self.assertEqual(sanitize({'token':'secret','input_tokens':10}),{'input_tokens':10})
    def test_native_self_messages_do_not_raise_controller_admission_count(self):
        # // Regression: 9e71cc15 counted native rows; run6 ruling counts submissions only.
        prior={'controller_inputs':{'claude':1,'codex':1},'seat_usd_estimate':0,'seat_usd_upper_rate':0}
        got=cumulative(prior,{'claude':2,'codex':3},0,0,observed_claude=4)
        self.assertEqual(got['controller_inputs']['claude'],3)
if __name__=='__main__': unittest.main()
