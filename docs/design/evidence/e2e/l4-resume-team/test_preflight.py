"""Offline lane guards: tempdirs only, no credentials, CLI, network or processes."""
import tempfile
import unittest
from pathlib import Path

from preflight import PrerequisiteError, validate_auth_source


class AuthSourceTests(unittest.TestCase):
    def test_missing_explicit_source_never_falls_back(self):
        with self.assertRaisesRegex(PrerequisiteError, "explicit disposable"):
            validate_auth_source(None, Path("/unread-operator-home"))

    def test_rejects_real_harness_roots_before_access(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            for name in [".codex", ".claude", ".claude-work", ".gemini", ".grok"]:
                with self.subTest(name=name):
                    with self.assertRaisesRegex(PrerequisiteError, "real harness"):
                        validate_auth_source(str(home / name / "auth.json"), home)

    def test_only_regular_disposable_file_is_accepted_without_reading(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "auth.json"
            source.touch(mode=0o600)
            self.assertEqual(validate_auth_source(str(source), root / "operator"), source)
            alias = root / "alias.json"
            alias.symlink_to(source)
            with self.assertRaisesRegex(PrerequisiteError, "symlink"):
                validate_auth_source(str(alias), root / "operator")


if __name__ == "__main__":
    unittest.main()
