"""Execute the committed controller once; retain passive run7 evidence only.

Builds and gates are separate. No predicate changes, paid retries, or tests.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

from run5_capture import atomic_write, capture
from support import complete_rows

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
OUT = BASE / 'run7'
LOGS = ROOT / '.check-logs/l7-ledger-run7'


def save(name, value):
    atomic_write(OUT / name, json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--auth-source', required=True)
    args = parser.parse_args()
    assert Path.cwd() == ROOT and not OUT.exists()
    os.umask(0o077)
    preflight = json.loads((LOGS / 'preflight.json').read_text())
    subprocess.run(['git', 'merge-base', '--is-ancestor', 'dabf846f', 'HEAD'], check=True)
    assert subprocess.check_output(['git', '-C', str(ROOT.parent / 'mesh-l7'),
                                   'rev-parse', 'HEAD'], text=True).strip() == preflight['mesh_commit']
    for name, path in [('daemon', ROOT / 'src-tauri/target/release/taurhaus-daemon'),
                       ('mesh', ROOT.parent / 'mesh-l7/target/debug/mesh')]:
        with path.open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == preflight[name + '_sha256']
    child = None
    published = False
    errors = []
    pane_digests = set()
    pane_count = 0

    def interrupted(sig, frame):
        raise RuntimeError('run7 driver interrupted')

    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        child = subprocess.Popen([sys.executable, '-B', str(BASE / 'controller.py'),
                                  '--auth-source', args.auth_source],
                                 env=dict(os.environ, L7_RUN_NAME='run7'), start_new_session=True)
        while child.poll() is None:
            if (OUT / 'events.jsonl').exists():
                if not published:
                    for name in ('preflight.json', 'builds.json', 'gate-cargo-polls.jsonl'):
                        shutil.copyfile(LOGS / name, OUT / name)
                    for name in ('controller', 'runtime', 'support', 'preflight'):
                        shutil.copyfile(BASE / (name + '.py'), OUT / (name + '-at-execution.py'))
                    published = True
                try:
                    capture(out=OUT)
                    events = complete_rows((OUT / 'events.jsonl').read_text())
                    isolation = next((r for r in events if r.get('kind') == 'isolation'), None)
                    assignment_started = (OUT / 'step1-assignment-receipt.json').exists()
                    if isolation and pane_count < (12 if assignment_started else 2):
                        scratch = Path(isolation['root'])
                        record_path = scratch / 'claude/teams/l7-ledger/runtime/alpha.json'
                        if record_path.exists():
                            record = json.loads(record_path.read_text())
                            pane = subprocess.run(['tmux', '-S', record['tmuxSocket'],
                                                   'capture-pane', '-p', '-S', '-12',
                                                   '-t', record['paneId']],
                                                  capture_output=True, text=True, timeout=3)
                            raw = pane.stdout
                            digest = hashlib.sha256(raw.encode()).hexdigest()
                            # Retain terminal layout/composer markers, never message text.
                            safe = []
                            for line in raw.splitlines()[-60:]:
                                markers = re.findall(r'\[Pasted text[^\]]*\]', line)
                                if re.fullmatch(r'[ ─│╭╮╰╯]*', line):
                                    safe.append(line)
                                elif markers:
                                    safe.append(('› ' if '›' in line else '') + ' '.join(markers))
                                elif '›' in line:
                                    safe.append('› <pane text redacted>' if line.split('›', 1)[1].strip() else '›')
                                else:
                                    safe.append('<pane text redacted>')
                            # Deduplicate by public geometry; bounded to twelve captures.
                            public = '\n'.join(safe) + '\n'
                            if public not in pane_digests and pane.returncode == 0:
                                pane_digests.add(public)
                                pane_count += 1
                                name = f'composer-pane-{pane_count:02d}.txt'
                                atomic_write(OUT / name, public)
                                with (OUT / 'pane-captures.jsonl').open('a') as stream:
                                    stream.write(json.dumps({'at': time.time(), 'file': name,
                                                            'pane_id': record['paneId'],
                                                            'source_sha256': digest,
                                                            'exit': pane.returncode}) + '\n')
                except (OSError, ValueError, KeyError, StopIteration, subprocess.TimeoutExpired) as error:
                    if type(error).__name__ not in errors:
                        errors.append(type(error).__name__)
            time.sleep(.5)
        return child.returncode
    finally:
        if child is not None:
            if child.poll() is None:
                child.send_signal(signal.SIGTERM)
            child.wait()
            if OUT.exists():
                save('execution.json', {
                    'command': 'L7_RUN_NAME=run7 python3 -B docs/design/evidence/e2e/l7-ledger/controller.py --auth-source <explicit-authorized-source>',
                    'driver': 'run7_driver.py --auth-source <explicit-authorized-source>',
                    'controller_exit': child.returncode,
                    'passive_capture': 'notify/rollout identities and bounded redacted pane geometry every 0.5 seconds',
                    'capture_error_classes': errors,
                    'reruns': 0,
                })


if __name__ == '__main__':
    raise SystemExit(main())
