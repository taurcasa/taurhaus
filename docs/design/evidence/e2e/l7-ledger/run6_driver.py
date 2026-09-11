"""Build verified candidates, run the unchanged controller once, capture passively.

Run from the checkout root with --auth-source naming the authorized disposable
file. Credentials are handled only by the committed controller. Gates run later.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import gates
from run5_capture import capture, atomic_write
from support import complete_rows

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
LOGS = ROOT / '.check-logs/l7-ledger-run6'
OUT = BASE / 'run6'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def save(path, value):
    atomic_write(path, json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--auth-source', required=True)
    args = parser.parse_args()
    assert Path.cwd() == ROOT and not OUT.exists()
    os.umask(0o077)
    LOGS.mkdir(parents=True, exist_ok=True)
    source = 'src-tauri/src/session_scanner/idle/codex.rs'
    content = gates.base_content_check(ROOT)
    count = subprocess.run(['grep', '-c', 'COMPLETION_FLUSH_TOLERANCE', source],
                           capture_output=True, text=True, check=True)
    assert int(count.stdout) >= 1 and content['passed']
    subprocess.run(['git', 'merge-base', '--is-ancestor', '37f0254d', 'HEAD'], check=True)
    mesh_commit = subprocess.check_output(['git', '-C', str(gates.MESH), 'rev-parse', 'HEAD'], text=True).strip()
    assert mesh_commit.startswith('1f7447f')
    prior = {}
    for name in ('run', 'run2', 'run3', 'run4', 'run5'):
        rows = complete_rows((BASE / name / 'events.jsonl').read_text())
        prior[name] = next(row['sha256'] for row in rows if row.get('kind') == 'binary' and row.get('name') == 'taurhaus-daemon')
    gates.OUT = LOGS
    gates.LOGS = LOGS
    builds = []
    for argv, cwd, label in [(['just', 'build-daemon'], ROOT, 'daemon-build'),
                            (['cargo', 'build', '--bin', 'mesh'], gates.MESH, 'mesh-build')]:
        builds.append(gates.command(argv, cwd, label))
        save(LOGS / 'builds.json', builds)
        if builds[-1]['exit']:
            return builds[-1]['exit']
    daemon_digest = digest(ROOT / 'src-tauri/target/release/taurhaus-daemon')
    assert daemon_digest not in prior.values(), 'daemon digest unchanged from historical trials'
    assert gates.base_content_check(ROOT) == content, 'build source changed unexpectedly'
    preflight = {
        'taurhaus_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'main_base': '37f0254d', 'content': content,
        'content_command': 'grep -c COMPLETION_FLUSH_TOLERANCE ' + source,
        'content_count': int(count.stdout), 'content_exit': count.returncode,
        'daemon_sha256': daemon_digest, 'historical_daemon_sha256': prior,
        'daemon_differs_from_all_five': True, 'mesh_commit': mesh_commit,
        'mesh_sha256': digest(gates.MESH / 'target/debug/mesh'),
        'descriptor': 'unchanged; tmux only', 'protocol': 27,
        'model': 'gpt-5.6-luna', 'effort': 'low', 'expected_codex_version': '0.153.4',
        'native_siblings': ['codex', 'codex-code-mode-host'],
        'controller_sha256': digest(BASE / 'controller.py'),
        'runtime_sha256': digest(BASE / 'runtime.py'),
        'unavailable_references': [
            'Designated lane-2 checkout absent; read retained lane-2/run3 controller in this checkout.',
            'Mesh docs/design/ledger-*.md absent at pinned revision; read USAGE.md ledger contracts.'],
        'opus_lens': 'Unavailable in current model inventory; orchestrator review required.',
    }
    save(LOGS / 'preflight.json', preflight)
    command = [sys.executable, '-B', str(BASE / 'controller.py'), '--auth-source', args.auth_source]
    child = None
    def interrupted(sig, frame):
        raise RuntimeError('run6 driver interrupted')
    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    published = False
    capture_errors = []
    try:
        child = subprocess.Popen(command, env=dict(os.environ, L7_RUN_NAME='run6'), start_new_session=True)
        while child.poll() is None:
            if (OUT / 'events.jsonl').exists():
                if not published:
                    for name in ('preflight.json', 'builds.json', 'gate-cargo-polls.jsonl',
                                 'capture-red.txt', 'capture-green.txt', 'controller-tests.txt'):
                        shutil.copyfile(LOGS / name, OUT / name)
                    for name in ('controller.py', 'runtime.py', 'support.py'):
                        shutil.copyfile(BASE / name, OUT / (name[:-3] + '-at-execution.py'))
                    published = True
                try:
                    capture(out=OUT)
                except (OSError, ValueError, StopIteration) as error:
                    label = type(error).__name__
                    if label not in capture_errors:
                        capture_errors.append(label)
            time.sleep(.5)
        return child.returncode
    finally:
        if child is not None:
            if child.poll() is None:
                child.send_signal(signal.SIGTERM)
            child.wait()
            if OUT.exists():
                save(OUT / 'execution.json', {
                    'command': 'L7_RUN_NAME=run6 python3 -B docs/design/evidence/e2e/l7-ledger/controller.py --auth-source <explicit-authorized-source>',
                    'driver': 'run6_driver.py --auth-source <explicit-authorized-source>',
                    'controller_exit': child.returncode,
                    'passive_capture': 'run5_capture.capture(out=run6), every 0.5 seconds; identities/digests only',
                    'capture_error_classes': capture_errors,
                    'tests': {'capture_red_exit': 1, 'capture_green_exit': 0, 'capture_tests': 8,
                              'controller_exit': 0, 'controller_tests': 14},
                })


if __name__ == '__main__':
    raise SystemExit(main())
