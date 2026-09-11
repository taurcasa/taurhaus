"""Exact gate driver; no runtime CLI, credential access or installation.

Run from the Lane 7 checkout root. Full transient logs stay in .check-logs;
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
MESH = ROOT.parent / "mesh-l7"
OUT = Path(__file__).resolve().parent
LOGS = ROOT / ".check-logs/l7-ledger"


def sanitize(text):
    return re.sub(
        r'/home/[^/\s]+/(?!projects/(?:taurhaus-l7-ledger|mesh-l7)(?:/|\b))[^\s\"\']*',
        '<operator-path-redacted>', text,
    )


def command(argv, cwd, label):
    deadline = time.monotonic() + 1800
    while True:
        probe = subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
        with (OUT/'gate-cargo-polls.jsonl').open('a') as stream:
            stream.write(json.dumps({'at':time.time(),'command':"pgrep -af '(^|/)cargo( |$)'",'exit':probe.returncode,'pids':[line.split()[0] for line in probe.stdout.splitlines()]})+'\n')
        if len(probe.stdout.splitlines()) < 3:break
        if time.monotonic() >= deadline:raise RuntimeError('Cargo queue deadline exceeded')
        time.sleep(30)
    env = dict(os.environ, CARGO_BUILD_JOBS='1', CARGO_TARGET_DIR=str(cwd / ('src-tauri/target' if cwd == ROOT else 'target')))
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
        raise SystemExit('Run only from the Lane 7 checkout root')
    cleanup=json.loads((OUT/os.environ.get('L7_RUN_NAME','run')/'cleanup.json').read_text())
    assert cleanup['auth_removed'] and cleanup['root_removed'] and not cleanup['survivors'] and cleanup['port_closed'], 'teardown must precede gates'
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
            (OUT / (os.environ.get('L7_RUN_NAME','run')+'-checks-result.json')).write_text(json.dumps(results, indent=2) + '\n')
        for path in (ROOT / 'src-tauri/target/release/taurhaus-daemon', MESH / 'target/debug/mesh'):
            if path.is_file():
                with path.open('rb') as stream:
                    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
                results['binaries'].append({'path': str(path), 'sha256': digest})
    finally:
        (OUT / (os.environ.get('L7_RUN_NAME','run')+'-checks-result.json')).write_text(json.dumps(results, indent=2) + '\n')
    return int(any(row['exit'] != 0 for row in results['commands']))


if __name__ == '__main__':
    raise SystemExit(main())
