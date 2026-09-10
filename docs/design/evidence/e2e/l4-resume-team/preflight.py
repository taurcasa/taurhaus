"""Fail closed before creating runtime roots or launching any paid seat."""
import os
from pathlib import Path


class PrerequisiteError(RuntimeError):
    pass


def validate_auth_source(value, operator_home):
    if not value:
        raise PrerequisiteError("missing explicit disposable auth.json source; no fallback permitted")
    source = Path(value)
    if not source.is_absolute():
        raise PrerequisiteError("disposable auth source must be absolute")
    source = Path(os.path.normpath(source))
    try:
        first = source.relative_to(operator_home).parts[0]
    except (ValueError, IndexError):
        first = ""
    if first.startswith(".claude") or first in {".codex", ".gemini", ".grok"}:
        raise PrerequisiteError("real harness auth source forbidden")
    if any(path.is_symlink() for path in [source, *source.parents]):
        raise PrerequisiteError("symlink auth source forbidden")
    if not source.is_file():
        raise PrerequisiteError("disposable auth source is not a regular file")
    return source
