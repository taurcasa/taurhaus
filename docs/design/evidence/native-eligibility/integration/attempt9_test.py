"""Offline evidence guards; no CLI, credentials or model calls."""
import ast
import hashlib
from pathlib import Path
import shutil
import tempfile
from unittest.mock import patch
import unittest
from attempt5_support import clean
from attempt9_support import enforce_budget, retained_log, validate_compaction, pack_events, unpack_events

class Evidence(unittest.TestCase):
    def test_lossless_event_dedup(self):
        rows=[{'at':1,'kind':'command_result','output':'pane'}, {'at':2,'kind':'command_result','output':'pane'}, {'at':3,'kind':'daemon_response','response':{'id':'r','result':[]}}]
        packed,payloads=pack_events(rows)
        self.assertEqual(unpack_events(packed,payloads),rows)
        self.assertEqual(packed[0]['payload_ref'],packed[1]['payload_ref'])
        self.assertEqual(len(payloads),2)

    def test_fresh_budget(self):
        enforce_budget(16, 3)
        with self.assertRaises(AssertionError): enforce_budget(17, 0)
        with self.assertRaises(AssertionError): enforce_budget(1, 3.01)

    def test_complete_log(self):
        # // Regression: ddb7aef1 inherited family filtering and periodic thinning.
        rows=[{'event':e,'at':i} for i,e in enumerate(['other','inotify.telemetry','inotify.telemetry','inotify.telemetry'])]
        self.assertEqual(retained_log(rows+rows)[0],rows)

    def test_compaction_requires_generation_logs_and_card(self):
        # // Regression: ddb7aef1 observed a boundary with no recovery delivery.
        before={'contextGeneration':'1','appServer':{'threadId':'t'}}
        after={'contextGeneration':'2','appServer':{'threadId':'t'}}
        rows=[{'event':'compaction.codex_host.'+s} for s in ['received','delivered']]
        events=[{'params':{'item':{'type':'userMessage','text':'[taurhaus] recovery_card'}}}]
        validate_compaction(before,after,rows,events,'[taurhaus] recovery_card')
        for bad in [(before,after,[],events,'[taurhaus] recovery_card'),(before,before,rows,events,'[taurhaus] recovery_card'),(before,after,rows,[], '')]:
            with self.assertRaises(AssertionError):validate_compaction(*bad)


def helper(filename, name):
    # Load only a pure helper; importing a controller would launch the paid trial.
    tree = ast.parse((Path(__file__).parent / filename).read_text())
    node = next((n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name), None)
    assert node is not None, f"missing helper: {name}"
    scope = {"Path": Path, "shutil": shutil, "hashlib": hashlib}
    exec(compile(ast.Module(body=[node], type_ignores=[]), filename, "exec"), scope)
    return scope[name]

class ReviewRegressions(unittest.TestCase):
    # // Regression: df7d2993 audited restored source but retained the trial binary,
    # hardcoded credential removal, and pointed unrelated excerpts at daemon logs.
    def test_teardown_removes_trial_binary_and_measures_auth(self):
        teardown = helper('attempt9-controller.py', 'remove_trial_files')
        with tempfile.TemporaryDirectory() as tmp:
            root, mesh = Path(tmp) / 'scratch', Path(tmp) / 'mesh'
            (root / 'codex').mkdir(parents=True)
            (root / 'codex/auth.json').write_text('fake credential')
            binary = mesh / 'target/debug/mesh'
            binary.parent.mkdir(parents=True)
            binary.write_text('trial-enabled fake binary')
            with patch.object(shutil, 'rmtree'):
                result = teardown(root, mesh)
            self.assertFalse(binary.exists())
            self.assertTrue(result['mesh_trial_artifact_removed'])
            self.assertFalse(result['auth_removed'])
            self.assertFalse(result['root_removed'])
            result = teardown(root, mesh)
            self.assertTrue(result['auth_removed'])
            self.assertTrue(result['root_removed'])

    def test_audit_rejects_trial_binary_even_with_disabled_source(self):
        verify = helper('attempt9-audit.py', 'verify_mesh_artifact_removed')
        with tempfile.TemporaryDirectory() as tmp:
            mesh = Path(tmp)
            source = mesh / 'src/delivery/app_server/capabilities.rs'
            source.parent.mkdir(parents=True)
            source.write_text('disposition: "disabled", enabled: false')
            binary = mesh / 'target/debug/mesh'
            binary.parent.mkdir(parents=True)
            binary.write_text('trial-enabled fake binary')
            with self.assertRaisesRegex(AssertionError, 'trial Mesh binary'):
                verify(mesh)
            binary.unlink()
            self.assertTrue(verify(mesh))

    def test_excerpt_names_source_and_discloses_omission(self):
        excerpt = helper('attempt9-audit.py', 'excerpt_text')
        original = ''.join(f'line {n}\n' for n in range(100))
        for source in ['mesh-build.log', 'gates/gate-check-quick.log']:
            text = excerpt(original, source)
            self.assertIn(source, text)
            self.assertIn('40 lines', text)
            self.assertIn('sha256 in excerpt-manifest.json', text)
            self.assertNotIn('run/taurhaus.log.jsonl', text)
            self.assertIn('line 0\n', text)
            self.assertTrue(text.endswith('line 99\n'))
        self.assertIn('run/taurhaus.log.jsonl', excerpt(original, 'run/daemon-stderr.txt'))
        self.assertEqual(excerpt('short\n', 'mesh-build.log'), 'short\n')

    def test_sanitizer_preserves_trial_provenance_only(self):
        # // Regression: 9dd28b7e reused attempt5 clean(), erasing mesh-trial cwd.
        for name in ['taurhaus-trial', 'mesh-trial', 'mesh-push']:
            path = f'/home/test-operator/projects/{name}/target/debug/mesh'
            self.assertEqual(clean({'cwd': path}), {'cwd': path})
        for path in ['/home/test-operator/private/file', '/home/test-operator/projects/mesh-trial-other/file']:
            self.assertEqual(clean(path), '<operator-path-redacted>')

if __name__=='__main__':unittest.main()
