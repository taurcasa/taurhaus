"""Lane 2 credential preflight. Never launches a CLI or reads credential bytes.

The live lane must not start unless this preflight accepts an explicitly supplied
disposable source. There is deliberately no CODEX_HOME or operator-home fallback.
Run: python3 -B docs/design/evidence/e2e/l2-tmux-busy/preflight.py
An authorized disposable source can be supplied with --auth-source.
"""
import argparse
import json
from pathlib import Path


class PreflightUnavailable(ValueError):
    """A required isolation input is absent or unsafe."""


def credential_source(value):
    if not value:
        raise PreflightUnavailable("Missing explicit disposable auth.json source; no fallback permitted")
    source = Path(value)
    if not source.is_absolute() or source.name != "auth.json" or ".." in source.parts:
        raise PreflightUnavailable("Source must be an absolute disposable auth.json path")
    parts = source.parts
    # Reject before touching even metadata in any operator harness home.
    for i, part in enumerate(parts):
        if part in ("home", "root"):
            index = i + (2 if part == "home" else 1)
            if index < len(parts) and parts[index].startswith((".claude", ".codex", ".gemini", ".grok")):
                raise PreflightUnavailable("Credential source is inside a real harness home")
    # Do not follow a parent symlink into a forbidden home either.
    for entry in reversed((source, *source.parents)):
        if entry.is_symlink():
            raise PreflightUnavailable("Credential source and parents must not be symlinks")
    if not source.is_file():
        raise PreflightUnavailable("Explicit disposable auth.json source is missing")
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--auth-source")
    args = parser.parse_args()
    try:
        credential_source(args.auth_source)
    except PreflightUnavailable as error:
        print(json.dumps({
            "status": "unavailable", "classification": "harness",
            "boundary": "before step 1 initialization", "exit_code": 78,
            "reason": str(error), "runtime_started": False,
            "codex_inputs": 0, "claude_inputs": 0, "seat_spend_usd": 0,
            "owned_processes": [], "auth_copied": False,
        }, indent=2))
        return 78
    print(json.dumps({"status": "ready", "scope": "credential preflight only", "runtime_started": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
