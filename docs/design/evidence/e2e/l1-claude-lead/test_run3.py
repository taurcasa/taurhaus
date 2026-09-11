"""Generated offline fixtures only: no credential access or CLI invocation."""
import tempfile
from pathlib import Path
import unittest
import continuation_support as support
from controller_support import sanitize


class Run3Tests(unittest.TestCase):
    def test_resolves_both_native_siblings_from_installed_launcher(self):
        # // Regression: ff8dc10c copied a partial runtime from the launcher package.
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            launcher = package / 'bin/codex.js'
            launcher.parent.mkdir()
            launcher.write_text('fixture; never executed')
            native = package / 'node_modules/@openai/codex-linux-x64/vendor/linux/bin/codex'
            native.parent.mkdir(parents=True)
            native.write_text('fixture; never executed')
            host = native.with_name('codex-code-mode-host')
            host.write_text('fixture; never executed')
            link = package / 'codex'
            link.symlink_to(launcher)
            self.assertEqual(support.installed_codex_bundle(lambda name: str(link)), [native, host])
            host.unlink()
            with self.assertRaisesRegex(ValueError, 'codex-code-mode-host'):
                support.installed_codex_bundle(lambda name: str(link))

    def test_pasted_composer_is_not_a_submission(self):
        # // Regression: 347db839 waited after paste, but never verified submission.
        started = [{'type': 'event_msg', 'payload': {'type': 'task_started', 'turn_id': 'new'}}]
        self.assertFalse(support.codex_submitted('› bounded prompt\n  ? for shortcuts', started, set()))
        self.assertFalse(support.codex_submitted('› Ask Codex to do anything', started, {'new'}))
        self.assertTrue(support.codex_submitted('• Working (esc to interrupt)\n› Ask Codex to do anything', started, set()))
        self.assertTrue(support.codex_submitted('• READY\n› Ask Codex to do anything', started, set()))

    def test_confirmation_leaves_full_opportunity(self):
        # // Regression: 5a7fb7ee left only 28 seconds after the real submission.
        self.assertEqual(support.opportunity_deadline(100, 900, 65), 165)
        with self.assertRaisesRegex(ValueError, 'full opportunity'):
            support.opportunity_deadline(850, 900, 65)

    def test_alpha_requires_attribution_and_idle(self):
        # // Regression: 5a7fb7ee never asserted alpha's activity attribution.
        record = {'session_id': 'session', 'jsonl_path': '/scratch/session.jsonl'}
        self.assertFalse(support.alpha_attributed_idle({}, {'activity_confidence': 'idle'}))
        self.assertFalse(support.alpha_attributed_idle(record, {'activity_confidence': 'uncertain'}))
        self.assertTrue(support.alpha_attributed_idle(record, {'activity_confidence': 'idle'}))

    def test_read_receipt_must_be_explicit_and_from_lead(self):
        # // Regression: 347db839 step 4 could pass on alpha's earlier read.
        alpha = {'payload': {'kind': 'consumed_by_read', 'reader_name': 'alpha', 'context': 'explicit-mesh-cli'}}
        lead = {'payload': {**alpha['payload'], 'reader_name': 'lead'}}
        self.assertFalse(support.explicit_lead_read([alpha]))
        self.assertTrue(support.explicit_lead_read([alpha, lead]))

    def test_opaque_model_internals_are_not_retained(self):
        # // Regression: ff8dc10c retained irrelevant encrypted model internals.
        self.assertEqual(sanitize({'encrypted_content': 'opaque', 'signature': 'opaque', 'turn_id': 'turn'}), {'turn_id': 'turn'})


if __name__ == '__main__':
    unittest.main()
