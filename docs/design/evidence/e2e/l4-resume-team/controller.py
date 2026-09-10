"""Recorded L4 preflight attempt. Stops before step 1 when scratch auth is absent.

This is the exact controller executed for this unavailable attempt, not an
unexecuted claim of whole-team runtime coverage. No real CLI is invoked here.
Usage: python3 -B docs/design/evidence/e2e/l4-resume-team/controller.py
"""
import json
import os
from pathlib import Path
import sys
import time

from preflight import PrerequisiteError, validate_auth_source

OUT = Path(__file__).resolve().parent


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')


def main():
    started = time.time()
    try:
        validate_auth_source(os.environ.get('L4_DISPOSABLE_AUTH_JSON'), Path.home())
    except PrerequisiteError as error:
        reason = str(error)
    else:
        # The recorded attempt contains no runtime implementation past its failed
        # prerequisite. Supplying credentials must never accidentally launch it.
        reason = 'auth prerequisite now available; this preflight-only attempt must be replaced by the six-step runtime controller'
    result = {
        'status': 'unavailable', 'classification': 'harness', 'boundary': 'preflight before step 1',
        'reason': reason, 'exit_code': 78, 'at': started,
        'auth_source_environment_variable': 'L4_DISPOSABLE_AUTH_JSON',
        'auth_source_provided': bool(os.environ.get('L4_DISPOSABLE_AUTH_JSON')),
        'real_harness_homes_accessed': False,
        'steps': [{'step': step, 'outcome': 'NOT RUN', 'classification': 'harness',
                   'reason': 'prerequisite blocked before initialize'} for step in range(1, 7)],
    }
    save('preflight-result.json', result)
    save('cost-ledger.json', {
        'model_requested': 'gpt-5.6-luna', 'effort_requested': 'low',
        'seat_starts': 0, 'paid_inputs': 0, 'turns': [], 'generations': [],
        'seat_spend_usd': 0, 'codex_spend_usd': 0, 'claude_spend_usd': 0,
        'max_codex_inputs': 16, 'max_seat_usd': 0.25, 'max_runtime_seconds': 900,
        'basis': 'No runtime launch or model input occurred; not inferred from reset counters.',
        'implementer_reviewer_spend': 'Separate orchestrator budget; not exposed to this controller.',
    })
    save('cleanup.json', {
        'runtime_processes_started': [], 'runtime_roots_created': [], 'survivors': [],
        'daemon_started': False, 'app_server_started': False, 'tui_started': False,
        'tmux_server_started': False, 'codex_started': False,
        'auth_copied': False, 'auth_copy_remaining': False, 'listeners_started': [],
        'verification': 'Controller stops before any subprocess/socket/root creation or auth read/copy.',
    })
    print(json.dumps(result))
    return 78


if __name__ == '__main__':
    sys.exit(main())
