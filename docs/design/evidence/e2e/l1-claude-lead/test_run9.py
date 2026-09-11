"""Offline run9 fixture regression tests: generated data and mocked processes only."""
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import Mock, patch
import controller

class Run9Tests(unittest.TestCase):
    def test_mount_isolation_preserves_host_pid(self):
        # // Regression: a363e328 isolated Claude PIDs, hiding registry identity from host observations.
        from run9_support import launch_wrapper
        argv = launch_wrapper(Path('/scratch/trial'), Path('/fixture/credential'))
        self.assertNotIn('--unshare-pid', argv)
        self.assertNotIn('--proc', argv)
        self.assertIn('--tmpfs', argv)
        i = argv.index('/fixture/credential')
        self.assertEqual(argv[i-1:i+2], ['--ro-bind', '/fixture/credential', '/scratch/trial/claude/.credentials.json'])

    def test_registry_attribution_and_absence_dispositions(self):
        from run9_support import registry_facts
        record = {'pid': 42, 'session_id': 'session'}
        activity = {'activity_attribution': 'attributed', 'session_id': 'session'}
        self.assertTrue(registry_facts(record, activity, ['42.json'])['ready'])
        self.assertEqual(registry_facts(record, {}, ['42.json'])['classification'], 'taurhaus')
        self.assertEqual(registry_facts(record, {}, [])['reason'], 'claude_registry_absent')
        self.assertFalse(registry_facts(record, activity, ['2.json'])['ready'])

    def test_registry_retained_including_empty_directory(self):
        from run9_support import retain_registry
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); source=root/'claude/sessions'; source.mkdir(parents=True)
            saved={}
            retain_registry(root, lambda n,v: saved.update({n:v}))
            self.assertEqual(saved['sessions/index.json']['files'], [])
            (source/'42.json').write_text(json.dumps({'pid':42,'sessionId':'s'}))
            retain_registry(root, lambda n,v: saved.update({n:v}))
            self.assertEqual(saved['sessions/42.json']['sessionId'], 's')

    def test_key_file_is_not_a_registry_and_peer_token_is_not_evidence(self):
        # // Regression: f2430277 counted Claude's peer-token key as a registry file.
        from run9_support import retain_registry
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); source=root/'claude/sessions'; source.mkdir(parents=True)
            (source/'42.hash.key').write_text(json.dumps({'peerToken':'synthetic-secret','procStart':'10','pidDomain':'host'}))
            saved={}; retain_registry(root,lambda n,v:saved.update({n:v}))
            self.assertEqual(saved['sessions/index.json']['files'],[])
            self.assertEqual(saved['sessions/index.json']['all_files'],['42.hash.key'])
            self.assertNotIn('synthetic-secret',json.dumps(saved))

    def test_teardown_signals_only_matching_owned_start_ticks(self):
        from run9_support import stop_owned
        old={'pid':42,'start_ticks':'10'}
        reused={'pid':42,'start_ticks':'11'}
        kill=Mock()
        stop_owned([old], lambda:[reused], kill, lambda _:None)
        kill.assert_not_called()
        stop_owned([old], Mock(side_effect=[[old],[old],[]]), kill, lambda _:None)
        self.assertEqual(kill.call_args.args[0],42)

    def test_run9_uses_existing_guard_publication_and_review_routes(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL':'run9'}):
            driver=runpy.run_path(str(controller.BASE/'run3_driver.py'))
        self.assertEqual(driver['OUT'],controller.BASE/'run9')
        source=(controller.BASE/'steps.py').read_text()
        self.assertIn("OUT.name in ('run8', 'run9')",source)
        self.assertIn("OUT.name in ('run7', 'run8', 'run9')",source)
        self.assertIn("'run9'",(controller.BASE/'review.py').read_text())

if __name__=='__main__':unittest.main()
