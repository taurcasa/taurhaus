"""Offline only: generated temp files; no CLI, network or real credentials."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from preflight import credential_source, PreflightUnavailable
from controller import Trial, AUTHORIZED_AUTH_SOURCE


class CredentialPreflightTests(unittest.TestCase):
    def test_controller_accepts_only_pinned_authorized_file(self):
        # // Regression: 57c8ff36 did not distinguish authorization from candidate.
        with tempfile.TemporaryDirectory() as root:
            trial = Trial.__new__(Trial)
            trial.root = Path(root)
            with patch.object(Path, 'is_symlink', return_value=False), \
                 patch.object(Path, 'is_file', return_value=True), \
                 patch('controller.shutil.copyfile', side_effect=PreflightUnavailable('copy reached')) as copy:
                with self.assertRaisesRegex(PreflightUnavailable, 'copy reached'):
                    trial.boot(AUTHORIZED_AUTH_SOURCE)
                copy.assert_called_once_with(Path(AUTHORIZED_AUTH_SOURCE), trial.root / 'codex/auth.json')

    def test_controller_refuses_other_harness_home_before_copy(self):
        # // Regression: 57c8ff36 authorized the candidate itself at the live call site.
        for home in ('.codex', '.claude-work', '.gemini', '.grok'):
            with self.subTest(home=home), tempfile.TemporaryDirectory() as root:
                trial = Trial.__new__(Trial)
                trial.root = Path(root)
                with patch.object(Path, 'is_symlink', return_value=False), \
                     patch.object(Path, 'is_file', return_value=True), \
                     patch('controller.shutil.copyfile', side_effect=PreflightUnavailable('copy reached')) as copy:
                    with self.assertRaises(PreflightUnavailable):
                        trial.boot('/home/example/' + home + '/auth.json')
                    copy.assert_not_called()

    def test_standing_authorization_permits_only_the_exact_source(self):
        # // Regression: 70d3a95d unconditionally rejected an explicitly authorized source.
        source = "/home/example/.codex/auth.json"
        with patch.object(Path, "is_symlink", return_value=False), patch.object(Path, "is_file", return_value=True), patch.object(Path, "read_bytes", side_effect=AssertionError("credential read")):
            self.assertEqual(credential_source(source, authorized_source=source), Path(source))
            with self.assertRaises(PreflightUnavailable):
                credential_source("/home/example/.claude/auth.json", authorized_source=source)

    def test_missing_source_has_no_home_fallback_or_filesystem_read(self):
        with patch.object(Path, "resolve", side_effect=AssertionError("filesystem access")):
            with self.assertRaisesRegex(PreflightUnavailable, "explicit disposable"):
                credential_source(None)

    def test_operator_harness_paths_rejected_before_filesystem_access(self):
        for name in (".claude", ".claude-work", ".codex", ".gemini", ".grok"):
            with self.subTest(name=name):
                with patch.object(Path, "resolve", side_effect=AssertionError("filesystem access")):
                    with self.assertRaisesRegex(PreflightUnavailable, "real harness home"):
                        credential_source("/home/example/" + name + "/auth.json")

    def test_explicit_scratch_file_accepted_without_reading_its_contents(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / "auth.json"
            source.write_text("synthetic-test-only")
            with patch.object(Path, "read_text", side_effect=AssertionError("credential read")):
                self.assertEqual(credential_source(str(source)), source)

    def test_symlink_cannot_redirect_to_an_unapproved_source(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / "auth.json"
            source.symlink_to("/home/example/.codex/auth.json")
            with self.assertRaises(PreflightUnavailable):
                credential_source(str(source))

    def test_missing_relative_and_wrong_filename_sources_are_unavailable(self):
        with tempfile.TemporaryDirectory() as root:
            for source in ("auth.json", str(Path(root) / "auth.json"), str(Path(root) / "other.json")):
                with self.subTest(source=source):
                    with self.assertRaises(PreflightUnavailable):
                        credential_source(source)


if __name__ == "__main__":
    unittest.main()
