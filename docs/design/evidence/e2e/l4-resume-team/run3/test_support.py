"""Offline harness guards: generated files only; no CLI, auth, or network access."""
import tempfile
import unittest
from pathlib import Path
from support import copy_native_runtime, sanitize_log_rows, classify_failure

class Guards(unittest.TestCase):
    def test_complete_native_runtime(self):
        # Regression: 4f946ee5 copied only codex; 0.153.4 also requires its native sibling.
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); package=root/'package'; entry=package/'bin/codex.js'
            entry.parent.mkdir(parents=True); entry.touch()
            native=package/'node_modules/@openai/codex-linux-x64/vendor/triple/bin'
            native.mkdir(parents=True)
            for name in ['codex','codex-code-mode-host']: (native/name).write_text(name)
            dest=root/'scratch'; dest.mkdir()
            copy_native_runtime(entry,dest)
            self.assertEqual(sorted(p.name for p in dest.iterdir()),['codex','codex-code-mode-host'])
            self.assertEqual((dest/'codex-code-mode-host').stat().st_mode & 0o777,0o700)
    def test_missing_sibling_copies_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); entry=root/'bin/codex.js'; entry.parent.mkdir(); entry.touch()
            native=root/'node_modules/@openai/codex-linux-x64/vendor/triple/bin'; native.mkdir(parents=True)
            (native/'codex').touch(); dest=root/'scratch'; dest.mkdir()
            with self.assertRaises(AssertionError): copy_native_runtime(entry,dest)
            self.assertEqual(list(dest.iterdir()),[])
    def test_complete_log_retains_repeated_diagnostics_excludes_usage(self):
        # Regression: 4f946ee5 exported only selected first/last daemon records.
        records=[{'event':'startup','message':'ready'}, {'event':'hosted.launch.failed','exit_status':'exit status: 1','stderr_tail':'runtime denied'}]*3
        actual=sanitize_log_rows(records+[{'event':'usage.fetched','account_id':'secret'}])
        self.assertEqual(actual,records)
    def test_native_runtime_failure_is_harness(self):
        self.assertEqual(classify_failure('launch_host: app-server exited before transport readiness; stderr: bwrap: Creating new namespace failed: Operation not permitted'),'harness')
        self.assertEqual(classify_failure('launch_host: app-server exited before transport readiness; exit status: 1'),'taurhaus')
        self.assertEqual(classify_failure('resume team-daemon startup refused'),'mesh')

if __name__=='__main__': unittest.main()
