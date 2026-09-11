"""Offline run5 routing and build admission checks; no runtime CLI or credentials."""
import os
from pathlib import Path
import runpy
import unittest
from unittest.mock import patch
import controller

class Run5Tests(unittest.TestCase):
    def test_run5_routes_to_fresh_attempt(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run5'}):
            driver = runpy.run_path(str(controller.BASE / 'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE / 'run5')
        self.assertEqual(driver['LEDGER_NAME'], 'admission-ledger.json')

    def test_required_product_revisions(self):
        self.assertEqual(controller.TAURHAUS_BASE, 'a7e6db7e')
        self.assertEqual(controller.MESH_COMMIT, '310144d')

    def test_gate_admission_allows_two_existing_cargos(self):
        source = (controller.BASE / 'build_gates.py').read_text()
        self.assertIn('len(probe.stdout.splitlines()) < 3', source)

    def test_step_commits_have_requested_session_trailer(self):
        source = (controller.BASE / 'run3_driver.py').read_text()
        self.assertIn('Claude-Session: https://claude.ai/code/session_01XJa6LsgXqhBdob1f1BS7BU', source)

    def test_hook_build_uses_three_job_admission(self):
        source = (controller.BASE / 'build_hook.py').read_text()
        self.assertIn('len(probe.stdout.splitlines()) < 3', source)
        self.assertIn('time.sleep(30)', source)
