"""Exact build/gate driver; no runtime CLI, credential access or installation.

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

ROOT = Path(__file__).resolve().parents[5]
MESH = ROOT.parent / "mesh-l2"
OUT = Path(__file__).resolve().parent
LOGS = ROOT / ".check-logs/l2-tmux-busy"


def sanitize(text):
    return re.sub(
        r'/home/[^/\s]+/(?!projects/(?:taurhaus-l2-tmux-busy|mesh-l2)(?:/|\b))[^\s\"\']*',
        '<operator-path-redacted>', text,
    )


def wait_cargo():
    start = time.monotonic()
    previous = None
    samples = []
    while True:
        result = subprocess.run(['pgrep', '-af', '(^|/)cargo( |$)'], capture_output=True, text=True)
        current = (result.returncode, sanitize(result.stdout))
        if current != previous:
            samples.append({'after_s': round(time.monotonic() - start, 3),
                            'exit': current[0], 'output': current[1]})
            previous = current
        if result.returncode == 1:
            return samples
        if result.returncode != 0 or time.monotonic() - start >= 1800:
            raise RuntimeError('Cargo queue unavailable after bounded wait')
        time.sleep(10)


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
    results = {'commands': [], 'cargo_waits': [], 'binaries': []}
    try:
        commands = [
            (['just', 'ensure-tauri-resources'], ROOT, 'ensure-resources'),
            (['just', 'build-daemon'], ROOT, 'daemon-build-retry'),
            (['cargo', 'build', '--bin', 'mesh'], MESH, 'mesh-build'),
            (['just', 'check-quick'], ROOT, 'check-quick'),
            (['just', 'lint'], ROOT, 'lint'),
            (['just', 'test-contracts'], ROOT, 'test-contracts'),
        ]
        for argv, cwd, label in commands:
            if label in ('daemon-build-retry', 'mesh-build'):
                results['cargo_waits'].append(wait_cargo())
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
