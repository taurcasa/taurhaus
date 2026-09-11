"""Exact local build/gate controller; never launches a harness or copies auth.

Run from the assigned checkout: python3 -B docs/design/evidence/e2e/l4-resume-team/prepare.py
"""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

CHECKOUT = Path('/home/mstie/projects/taurhaus-l4-resume-team')
MESH = Path('/home/mstie/projects/mesh-l4')
OUT = Path(__file__).resolve().parent
DESCRIPTOR = 'src/delivery/app_server/capabilities.rs'
children = []


def sanitize(text):
    return re.sub(r'/home/[^/\s]+/(?!projects/(?:taurhaus-l4-resume-team|mesh-l4)(?:/|\b))[^\s\"\']*',
                  '<operator-path-redacted>', text)


def run(argv, cwd, env, label):
    started = time.monotonic()
    raw = tempfile.TemporaryFile()
    child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=raw,
                             stderr=subprocess.STDOUT, start_new_session=True)
    children.append(child)
    code = child.wait()
    raw.seek(0)
    lines = sanitize(raw.read().decode(errors='replace')).splitlines()
    raw.close()
    # Build/gate output is diagnostic, not runtime evidence. Keep a bounded tail.
    (OUT / (label + '.txt')).write_text('\n'.join(lines[-60:]) + '\n')
    result = {'command': argv, 'cwd': str(cwd), 'exit_code': code,
              'seconds': round(time.monotonic() - started, 2), 'log_lines': len(lines)}
    (OUT / (label + '.json')).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result), flush=True)
    return code


def interrupted(signum, frame):
    raise RuntimeError('preparation interrupted: ' + str(signum))


def main():
    assert Path.cwd() == CHECKOUT
    assert subprocess.check_output(['git', 'branch', '--show-current'], text=True).strip() == 'feat/e2e-l4-resume-team'
    assert not subprocess.check_output(['git', '-C', str(MESH), 'status', '--porcelain'], text=True).strip()
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, interrupted)
    with tempfile.TemporaryDirectory(prefix='th-l4-build-') as temporary:
        root = Path(temporary)
        env = dict(os.environ)
        env.pop('TMUX', None)
        env['CARGO_HOME'] = os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))
        env['RUSTUP_HOME'] = os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup'))
        env['CARGO_BUILD_JOBS'] = '2'
        for key in ['HOME', 'CODEX_HOME', 'CLAUDE_CONFIG_DIR', 'CLAUDE_DIR', 'TAURHAUS_CLAUDE_DIR',
                    'GROK_HOME', 'GEMINI_CLI_HOME', 'TAURHAUS_AGY_DIR', 'TAURHAUS_DATA_DIR', 'TMUX_TMPDIR']:
            (root / key).mkdir()
            env[key] = str(root / key)
        changed = False
        try:
            deadline = time.monotonic() + 1800
            observations = []
            while True:
                probe = subprocess.run(['pgrep', '-af', '(^|/)cargo( |$)'], capture_output=True, text=True)
                observation = {'at': time.time(), 'exit_code': probe.returncode, 'output': sanitize(probe.stdout)}
                if not observations or observations[-1]['output'] != observation['output']:
                    observations.append(observation)
                    (OUT / 'cargo-wait.json').write_text(json.dumps(observations, indent=2) + '\n')
                if probe.returncode == 1:
                    break
                if probe.returncode != 0 or time.monotonic() >= deadline:
                    raise RuntimeError('Cargo prebuild wait failed or exceeded 30 minutes')
                time.sleep(10)
            text = (MESH / DESCRIPTOR).read_text()
            text = text.replace('host: "taurhaus-owned persistent thread; UNVERIFIED",',
                                'host: if build == "0.153.4" { "taurhaus-daemon-owned-thread/1" } else { "taurhaus-owned persistent thread; UNVERIFIED" },')
            text = text.replace('transport: TRANSPORT, configuration: "UNVERIFIED", trust: "UNVERIFIED",',
                                'transport: TRANSPORT, configuration: if build == "0.153.4" { "strict-config/1" } else { "UNVERIFIED" }, trust: if build == "0.153.4" { "daemon-owned/1" } else { "UNVERIFIED" },')
            text = text.replace('disposition: "disabled", enabled: false, strongest_receipt: "native_enqueued",',
                                'disposition: if build == "0.153.4" { "trial" } else { "disabled" }, enabled: build == "0.153.4", strongest_receipt: "native_enqueued",')
            changed = True
            (MESH / DESCRIPTOR).write_text(text)
            diff = subprocess.check_output(['git', '-C', str(MESH), 'diff', '--', DESCRIPTOR], text=True)
            (OUT / 'mesh-trial-descriptor.diff').write_text(diff)
            env['CARGO_TARGET_DIR'] = str(MESH / 'target')
            run(['cargo', 'build', '--bin', 'mesh'], MESH, env, 'mesh-build')
            env['CARGO_TARGET_DIR'] = str(CHECKOUT / 'src-tauri/target')
            run(['just', 'build-daemon'], CHECKOUT, env, 'daemon-build')
            binaries = {}
            for name, path in [('mesh', MESH / 'target/debug/mesh'), ('taurhaus-daemon', CHECKOUT / 'src-tauri/target/release/taurhaus-daemon')]:
                if path.is_file():
                    binaries[name] = {'path': str(path), 'sha256': hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()}
            (OUT / 'binaries.json').write_text(json.dumps(binaries, indent=2) + '\n')
            run(['bun', 'install', '--frozen-lockfile'], CHECKOUT, env, 'bun-install')
            for recipe in ['check-quick', 'lint', 'test-contracts']:
                run(['just', recipe], CHECKOUT, env, 'gate-' + recipe)
        finally:
            for child in reversed(children):
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait()
            if changed:
                run(['git', '-C', str(MESH), 'checkout', '--', DESCRIPTOR], CHECKOUT, env, 'descriptor-restore')


if __name__ == '__main__':
    main()
