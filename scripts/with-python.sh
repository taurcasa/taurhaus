#!/usr/bin/env bash
#
# Run a Python script under an interpreter that has the repo's Python
# dependencies (scripts/requirements.txt), and say how to get one when it does
# not — a missing package should read as a setup step, not as a traceback three
# frames into someone else's script.
#
# Interpreter, in order: $TAURHAUS_PYTHON, scripts/.venv/bin/python, python3.
#
# Usage: ./scripts/with-python.sh scripts/generate-infographics.py --dry-run

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
venv_python="$root/scripts/.venv/bin/python"

python="${TAURHAUS_PYTHON:-}"
if [ -z "$python" ]; then
    if [ -x "$venv_python" ]; then
        python="$venv_python"
    else
        python="python3"
    fi
fi

if ! "$python" -c 'import yaml, PIL' >/dev/null 2>&1; then
    cat >&2 <<EOF
error: the repo's Python tooling needs PyYAML and Pillow, and '$python' cannot import them.

Install them once, into a managed environment:

    just python-deps        # creates scripts/.venv from scripts/requirements.txt

Or point TAURHAUS_PYTHON at an interpreter that already has them:

    TAURHAUS_PYTHON=/path/to/python just test-scripts
EOF
    exit 1
fi

exec "$python" "$@"
