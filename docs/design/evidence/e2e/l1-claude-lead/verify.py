"""Exact sequential build/gate controller; never installs or launches the daemon."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

CHECKOUT = Path('/home/mstie/projects/taurhaus-l1-claude-lead')
MESH = Path('/home/mstie/projects/mesh-l1')
OUT = Path(__file__).resolve().parent


def clean(text):
    for index, path in enumerate((CHECKOUT, MESH)):
        text = text.replace(str(path), f'<worktree-{index}>')
    text = text.replace(str(Path.home()), '<operator-home>')
    for index, path in enumerate((CHECKOUT, MESH)):
        text = text.replace(f'<worktree-{index}>', str(path))
    return text


def main():
    assert Path.cwd() == CHECKOUT
    assert sys.argv[1:] in ([], ['--retry-setup'], ['--gates-only'])
    retry = sys.argv[1:] == ['--retry-setup']
    gates_only = sys.argv[1:] == ['--gates-only']
    output = OUT / ('verification-continuation.json' if gates_only else
                   'verification-retry.json' if retry else 'verification.json')
    results = []
    child = None

    def interrupted(signum, _frame):
        raise RuntimeError(f'verification interrupted: {signum}')

    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, interrupted)
    try:
        start = time.monotonic()
        polls = 0
        while True:
            check = subprocess.run(['pgrep', '-af', '(^|/)cargo( |$)'], capture_output=True, text=True)
            polls += 1
            if check.returncode == 1:
                break
            if check.returncode != 0 or time.monotonic() - start >= 1800:
                raise RuntimeError('Cargo availability wait failed or exceeded 30 minutes')
            time.sleep(10)
        results.append({'command': "pgrep -af '(^|/)cargo( |$)'", 'exit': 1,
                        'polls': polls, 'wait_seconds': round(time.monotonic() - start, 2),
                        'meaning': 'no Cargo process at build admission'})
        commands = [('daemon-build', CHECKOUT, ['just', 'build-daemon']),
                    ('mesh-build', MESH, ['cargo', 'build', '--bin', 'mesh']),
                    ('check-quick', CHECKOUT, ['just', 'check-quick']),
                    ('lint', CHECKOUT, ['just', 'lint']),
                    ('test-contracts', CHECKOUT, ['just', 'test-contracts'])]
        if retry:
            # Existing ensure-tauri-resources and frozen Bun install repaired setup.
            # Preserve the first run; do not repeat already-green Mesh/contracts.
            commands = [entry for entry in commands
                        if entry[0] in ('daemon-build', 'check-quick', 'lint')]
        if gates_only:
            commands = [entry for entry in commands
                        if entry[0] in ('check-quick', 'lint', 'test-contracts')]
        for label, cwd, argv in commands:
            env = dict(os.environ)
            target = cwd / ('target' if cwd == MESH else 'src-tauri/target')
            env.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='2')
            suffix = '-continuation' if gates_only else '-retry' if retry else ''
            log = CHECKOUT / '.check-logs/l1-claude-lead' / (label + suffix + '.log')
            print(json.dumps({'starting': label}), flush=True)
            started = time.monotonic()
            with log.open('w') as stream:
                child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stream,
                                         stderr=subprocess.STDOUT, start_new_session=True)
                child.wait(timeout=1800)
            lines = clean(log.read_text()).splitlines()
            result = {'command': ' '.join(argv), 'cwd': str(cwd), 'exit': child.returncode,
                      'target': str(target), 'build_jobs': 2,
                      'elapsed_seconds': round(time.monotonic() - started, 2),
                      'last_lines': lines[-20:]}
            binary = (target / 'release/taurhaus-daemon' if label == 'daemon-build' else
                      target / 'debug/mesh' if label == 'mesh-build' else None)
            if binary and child.returncode == 0:
                result['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
            results.append(result)
            output.write_text(json.dumps(results, indent=2) + '\n')
            print(json.dumps({'finished': label, 'exit': child.returncode}), flush=True)
    except Exception as error:
        results.append({'controller_error': clean(str(error))})
        raise
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=10)
        output.write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
