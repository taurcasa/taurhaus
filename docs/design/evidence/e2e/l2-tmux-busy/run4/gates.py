"""Exact gate driver; no runtime CLI, credential access or installation.

Run from the Lane 2 checkout root. Full transient logs stay in .check-logs;
the committed result retains exit codes, elapsed time and bounded safe excerpts.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[6]
MESH = ROOT.parent / "mesh-l2"
OUT = Path(__file__).resolve().parent
LOGS = ROOT / ".check-logs/l2-tmux-busy-run4"


def sanitize(text):
    return re.sub(
        r'/home/[^/\s]+/(?!projects/(?:taurhaus-l2-tmux-busy|mesh-l2)(?:/|\b))[^\s\"\']*',
        '<operator-path-redacted>', text,
    )


def command(argv, cwd, label):
    env = dict(os.environ, CARGO_TARGET_DIR=str(cwd / ('src-tauri/target' if cwd == ROOT else 'target')))
    started = time.monotonic()
    with (LOGS / (label + '.log')).open('w') as log:
        child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = child.wait()
        finally:
            if child.poll() is None:
                # Only this driver-owned process group; never another lane's cargo.
                os.killpg(child.pid, signal.SIGTERM)
                child.wait()
    lines = (LOGS / (label + '.log')).read_text(errors='replace').splitlines()
    result = {'command': ' '.join(argv), 'cwd': str(cwd),
              'target': env['CARGO_TARGET_DIR'], 'exit': code,
              'elapsed_s': round(time.monotonic() - started, 3),
              'last_lines': [sanitize(line) for line in lines[-30:]]}
    print(json.dumps({'command': result['command'], 'exit': code}), flush=True)
    return result


def main():
    if Path.cwd() != ROOT:
        raise SystemExit('Run only from the Lane 2 checkout root')
    LOGS.mkdir(parents=True, exist_ok=True)
    results = {'commands': [], 'binaries': []}
    try:
        commands = [
            (['just', 'check-quick'], ROOT, 'check-quick'),
            (['just', 'lint'], ROOT, 'lint'),
            (['just', 'test-contracts'], ROOT, 'test-contracts'),
        ]
        for argv, cwd, label in commands:
            results['commands'].append(command(argv, cwd, label))
            (OUT / 'checks-result.json').write_text(json.dumps(results, indent=2) + '\n')
        for path in (ROOT / 'src-tauri/target/release/taurhaus-daemon', MESH / 'target/debug/mesh'):
            if path.is_file():
                with path.open('rb') as stream:
                    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
                results['binaries'].append({'path': str(path), 'sha256': digest})
    finally:
        (OUT / 'checks-result.json').write_text(json.dumps(results, indent=2) + '\n')
    return int(any(row['exit'] != 0 for row in results['commands']))


if __name__ == '__main__':
    raise SystemExit(main())
