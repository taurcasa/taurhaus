"""Offline restart regressions: generated files and mocked input only."""
import tempfile
from pathlib import Path
import unittest
import continuation_support as support


class RestartTests(unittest.TestCase):
    def test_authoritative_counts_survive_final_ledger_restart(self):
        # // Regression: 3a1a8fc1 finalized cap_inputs, but restart still loaded run/.
        prior = {'cap_inputs': {'claude': 5, 'codex': 6},
                 'controller_reserved_inputs': {'claude': 3, 'codex': 6},
                 'seat_usd_estimate': .0672021, 'seat_usd_upper_rate': .6578964}
        got = support.cumulative(prior, {'claude': 1, 'codex': 1}, .01, .05)
        self.assertEqual(got['controller_inputs'], {'claude': 6, 'codex': 7})
        self.assertAlmostEqual(got['seat_usd_upper_rate'], .7078964)

    def test_missing_command_host_fails_before_runtime_copy(self):
        # // Regression: ff8dc10c copied codex without its required code-mode host.
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'codex'
            binary.write_text('generated, never executed')
            with self.assertRaisesRegex(ValueError, 'codex-code-mode-host'):
                support.codex_bundle(binary)
            host = binary.with_name('codex-code-mode-host')
            host.write_text('generated, never executed')
            self.assertEqual(support.codex_bundle(binary), [binary, host])

    def test_paste_settles_before_single_enter(self):
        # // Regression: ff8dc10c immediate Enter left Codex input in the composer.
        calls = []
        support.submit_input(lambda argv: calls.append(argv),
                             lambda seconds: calls.append(('settle', seconds)),
                             '%2', 'bounded prompt')
        self.assertEqual(calls[0], ['tmux', 'send-keys', '-t', '%2', '-l', 'bounded prompt'])
        self.assertGreaterEqual(calls[1][1], .25)
        self.assertEqual(calls[2], ['tmux', 'send-keys', '-t', '%2', 'Enter'])
        self.assertEqual(len(calls), 3)

    def test_startup_must_finish_before_exposure_window(self):
        # // Regression: 5a7fb7ee step 2 started before any completed alpha turn.
        started = {'type': 'event_msg', 'payload': {'type': 'task_started', 'turn_id': 'a'}}
        done = {'type': 'event_msg', 'payload': {'type': 'task_complete', 'turn_id': 'a'}}
        self.assertFalse(support.codex_ready([started]))
        self.assertTrue(support.codex_ready([started, done]))
        busy = {'type': 'event_msg', 'payload': {'type': 'task_started', 'turn_id': 'b'}}
        self.assertFalse(support.codex_ready([started, done, busy]))


if __name__ == '__main__':
    unittest.main()
