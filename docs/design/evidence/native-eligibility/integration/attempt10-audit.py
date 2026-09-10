"""Read-only setup-inconclusive audit; never imports a paid controller or reads credentials."""
import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
import sys


def regression_tests():
    """Extract only offline code; never import or execute the paid controller."""
    import ast
    import shutil
    import tempfile
    import unittest
    from unittest.mock import Mock
    base = Path(__file__).resolve().parent
    controller = ast.parse((base/'attempt10-controller.py').read_text())

    def helper(tree, name, **scope):
        node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name)
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<offline helper>', 'exec'), scope)
        return scope[name]

    class ReviewRegressions(unittest.TestCase):
        def test_retryable_setup_verdict(self):
            # // Regression: 9e367da4 called a retryable activation race a launch FAIL.
            doc = (base.parent/'codex-0.153.4-integration.md').read_text()
            self.assertIn('INCONCLUSIVE', doc.splitlines()[0])
            latest = doc.split('### Attempt 10 —')[1]
            self.assertIn('**INCONCLUSIVE**', latest)
            self.assertIn('15 of 16', latest)

        def test_restore_even_when_scratch_removal_fails(self):
            # // Regression: 9e367da4 rmtree exception skipped descriptor restoration.
            calls = []
            process = Mock()
            process.run.side_effect = lambda *a, **k: (calls.append('restore') or Mock(returncode=0))
            fs = Mock()
            fs.rmtree.side_effect = lambda p: (calls.append('remove') or (_ for _ in ()).throw(OSError('busy')))
            remove = helper(controller, 'remove_trial_files', subprocess=process, shutil=fs)
            with tempfile.TemporaryDirectory() as tmp:
                root, mesh = Path(tmp)/'root', Path(tmp)/'mesh'
                root.mkdir(); (mesh/'target/debug').mkdir(parents=True)
                (mesh/'target/debug/mesh').touch()
                result = remove(root, mesh)
                self.assertEqual(calls, ['restore', 'remove'])
                self.assertFalse(result['root_removed'])
                self.assertIn('busy', result['root_removal_error'])
                self.assertFalse((mesh/'target/debug/mesh').exists())

        def test_cleanup_does_not_claim_separate_auth_verification(self):
            # // Regression: 9e367da4 sampled auth absence only after deleting its root.
            remove = helper(controller, 'remove_trial_files', subprocess=Mock(), shutil=shutil)
            with tempfile.TemporaryDirectory() as tmp:
                root, mesh = Path(tmp)/'root', Path(tmp)/'mesh'
                root.mkdir(); (mesh/'target/debug').mkdir(parents=True)
                result = remove(root, mesh)
                self.assertNotIn('auth_removed', result)
                self.assertTrue(result['root_removed'])

        def test_port_exhaustion_is_explicit(self):
            # // Regression: b8149dd4 handed the final busy port to the daemon.
            loop = next(n for n in ast.walk(controller) if isinstance(n, ast.For)
                        and isinstance(n.target, ast.Name) and n.target.id == 'PORT')
            sock = Mock()
            sock.socket.return_value.__enter__ = Mock(return_value=Mock(bind=Mock(side_effect=OSError('busy'))))
            sock.socket.return_value.__exit__ = Mock(return_value=False)
            random = Mock()
            random.SystemRandom.return_value.sample.return_value = [20000, 31999]
            with self.assertRaisesRegex(RuntimeError, 'no free port in 20000-31999'):
                exec(compile(ast.Module(body=[loop], type_ignores=[]), '<port probe>', 'exec'),
                     {'socket': sock, 'secrets': random})

        def test_auth_source_uses_home_without_accessing_it(self):
            # // Regression: b8149dd4 split a fixed operator path to evade the path guard.
            source = next(n.value for n in ast.walk(controller) if isinstance(n, ast.Assign)
                          and any(isinstance(t, ast.Name) and t.id == 'source' for t in n.targets)
                          and isinstance(n.value, ast.BinOp))
            fake_path = Mock()
            fake_path.home.return_value = Path('/synthetic-operator')
            value = eval(compile(ast.Expression(source), '<auth source>', 'eval'), {'Path': fake_path})
            self.assertEqual(value, Path('/synthetic-operator/.codex/auth.json'))

        def test_audit_flags_follow_evidence(self):
            # // Regression: fe16495b serialized literals disconnected from audit checks.
            tree = ast.parse(Path(__file__).read_text())
            report = next(n for n in ast.walk(tree) if isinstance(n, ast.Dict)
                          and any(isinstance(k, ast.Constant) and k.value == 'mesh_clean' for k in n.keys))
            flags = {k.value: v for k, v in zip(report.keys, report.values) if isinstance(k, ast.Constant)}
            for enabled in [False, True]:
                scope = {'porcelain': 'dirty' if enabled else '', 'descriptor_enabled': enabled,
                         'binary_removed': not enabled,
                         'accounted': {'metering_complete': enabled, 'api_equivalent_usd': .25 if enabled else None}}
                for key, expected in [('mesh_clean', not enabled), ('descriptor_enabled', enabled),
                                      ('mesh_trial_binary_removed', not enabled), ('metering_complete', enabled),
                                      ('total_usd', .25 if enabled else None)]:
                    with self.subTest(key=key, enabled=enabled):
                        self.assertEqual(eval(compile(ast.Expression(flags[key]), '<flag>', 'eval'), scope), expected)

        def test_gate_cleanup_records_its_root(self):
            # // Regression: 7c206e36 gate cleanup omitted the root it verified.
            tree = ast.parse((base/'attempt2-gates.py').read_text())
            final = next(n.finalbody for n in tree.body if isinstance(n, ast.Try))
            with tempfile.TemporaryDirectory() as tmp:
                root, out = Path(tmp)/'scratch', Path(tmp)/'evidence'
                root.mkdir(); out.mkdir()
                exec(compile(ast.Module(body=final, type_ignores=[]), '<gate cleanup>', 'exec'),
                     {'children': [], 'root': root, 'out': out, 'shutil': shutil, 'json': json})
                record = json.loads((out/'gate-cleanup.json').read_text())
                self.assertEqual(record['root'], str(root))
                self.assertTrue(record['root_removed'])

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ReviewRegressions)
    return unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful()


if __name__ == '__main__' and '--self-test' in sys.argv:
    sys.exit(0 if regression_tests() else 1)

from attempt10_support import finalize_metering, pack_events, unpack_events
from attempt5_support import clean

B = Path(__file__).resolve().parent
OUT = B/'attempt10'
RUN = OUT/'run'
def read(path): return json.loads(path.read_text())
def rows(path): return [json.loads(l) for l in path.read_text().splitlines() if l]

result = read(RUN/'initialize-result.json')
report = result['outcome']['report']
assert report['failed_step'] == 'opt_in_delivery'
assert 'team owner already holds lifetime lock' in report['message']
assert report['retryable'] is True
steps = {'1': 'INCONCLUSIVE: setup/activation race; hosted launch observed',
         **{str(n): 'NOT RUN' for n in range(2, 8)}}
cleanup = read(RUN/'cleanup.json')
assert not cleanup['survivors'] and cleanup['port_closed'] and cleanup['root_removed']
assert not Path(cleanup['root']).exists()
# Historical auth_removed was sampled after rmtree; root absence is the proof.
cleanup.pop('auth_removed', None)
for identity in read(RUN/'identities.json'):
    stat = Path(f"/proc/{identity['pid']}/stat")
    try: ticks = stat.read_text().rsplit(')',1)[1].split()[19]
    except FileNotFoundError: continue
    assert ticks != identity['start_ticks'], 'owned process survived'
trace = rows(RUN/'events.jsonl')
if (RUN/'event-payloads.json').exists():
    trace = unpack_events(trace,read(RUN/'event-payloads.json'))
isolation = next(r for r in trace if r['kind']=='isolation')
port = int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
assert 20000 <= port <= 31999
with socket.socket() as probe:
    probe.settimeout(.2)
    assert probe.connect_ex(('127.0.0.1',port)) != 0
mesh = Path('/home/mstie/projects/mesh-trial')
assert subprocess.check_output(['git','-C',str(mesh),'rev-parse','--short','HEAD'],text=True).strip() == 'fcb9647'
porcelain = subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True)
assert not porcelain
binary_removed = not (mesh/'target/debug/mesh').exists()
assert binary_removed
descriptor = (mesh/'src/delivery/app_server/capabilities.rs').read_text()
pin = re.search(r'for build in \["0\.153\.4", "\*"\].*?entries.push\(Descriptor \{(?P<pin>.*?)evidence_scope:', descriptor, re.S)
assert pin, 'missing pinned descriptor'
descriptor_enabled = bool(re.search(r'enabled:\s*true', pin['pin']))
assert not descriptor_enabled and 'disposition: "disabled"' in pin['pin']
accounted = finalize_metering(read(RUN/'cost-ledger.json'))
assert accounted['paid_inputs'] == 1 and not accounted['generations']
# Lossless trace packing; complete daemon JSONL is not filtered or thinned.
packed,payloads=pack_events(trace)
assert unpack_events(packed,payloads)==trace
for path in OUT.rglob('*'):
    if not path.is_file(): continue
    data = clean(path.read_text())
    if 'pane-' in path.name: assert len(data.splitlines()) <= 60
    assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',data),path
    assert not re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-trial|mesh-trial)(?:/|(?![\w.-])))',data),path
    assert 'item/agentMessage/delta' not in data,path
gate_cleanup = OUT/'gates/gate-cleanup.json'
if gate_cleanup.exists():
    assert read(gate_cleanup)['children_waited'] and read(gate_cleanup)['root_removed']
    env = read(OUT/'gates/gate-isolation.json')['environment']
    assert not Path(env['HOME']).exists()
    owned = ('TAURHAUS_TRIAL_ID='+env['TAURHAUS_TRIAL_ID']).encode()+b'\0'
    for process in Path('/proc').iterdir():
        if not process.name.isdigit(): continue
        try: assert owned not in (process/'environ').read_bytes(), 'gate process survived'
        except (PermissionError, FileNotFoundError, ProcessLookupError): pass
logs = rows(RUN/'taurhaus.log.jsonl')
assert len(logs)==len({json.dumps(r,sort_keys=True) for r in logs})
audit = {
    'verdict':'INCONCLUSIVE: setup/activation race; hosted launch observed',
    'controller_exit':1, 'steps':steps,
    'taurhaus_build_revision':'5c4132a9', 'identity_merge_ancestor':'6398bfa3',
    'mesh_revision':'fcb9647', 'private_port':port, 'marker':isolation['marker'],
    'mesh_clean':not porcelain,'descriptor_enabled':descriptor_enabled,'mesh_trial_binary_removed':binary_removed,
    'cleanup':cleanup,'verified_pid_start_identities':len(read(RUN/'identities.json')),
    'daemon_jsonl_rows':len(logs),'daemon_jsonl_sha256':hashlib.sha256((RUN/'taurhaus.log.jsonl').read_bytes()).hexdigest(),
    'daemon_jsonl_event_families':sorted({r['event'] for r in logs}),
    'trace_rows':len(trace),'trace_unique_payloads':len(payloads),
    'model_turns':accounted['paid_inputs'],'metering_complete':accounted['metering_complete'],'total_usd':accounted['api_equivalent_usd'],
    'gates':{p.stem:read(p)['exit'] for p in (OUT/'gates').glob('gate-*.json') if 'exit' in read(p)},
}
print(json.dumps(audit,indent=2))
