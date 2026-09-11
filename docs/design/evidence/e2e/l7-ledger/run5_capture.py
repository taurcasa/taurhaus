"""Passive run5 native-tail capture; does not change or invoke the controller.

Run while the unchanged controller is waiting. Only the scratch root explicitly
recorded by that controller is read; credential files and message text are excluded.
"""
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

from support import complete_rows

BASE = Path(__file__).resolve().parent


def atomic_write(path, text):
    temporary = path.with_name(path.name + '.tmp')
    try:
        temporary.write_text(text)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def public_row(row):
    keys = ('timestamp', 'ts', 'event', 'type', 'turn_id', 'turn-id', 'session_id',
            'thread-id', 'call_id', 'model', 'effort', 'reasoning_effort')
    result = {key: row[key] for key in keys if key in row}
    payload = row.get('payload')
    if isinstance(payload, dict):
        result['payload'] = {key: payload[key] for key in keys if key in payload}
    result['source_sha256'] = hashlib.sha256(json.dumps(row, sort_keys=True).encode()).hexdigest()
    return result


def capture(out=None):
    out = out if out is not None else BASE / 'run5'
    events = complete_rows((out / 'events.jsonl').read_text())
    isolation = next((row for row in events if row.get('kind') == 'isolation'), None)
    if not isolation:
        return
    root = Path(isolation['root'])
    assert root.parent == Path('/tmp') and root.name.startswith('th-l7-')
    if not root.is_dir():
        return
    for source, name in [(root / 'data/codex-notify.jsonl', 'notify-records.jsonl')]:
        if source.exists():
            rows = complete_rows(source.read_text())
            atomic_write(out / name, ''.join(json.dumps(public_row(row)) + '\n' for row in rows))
    tails = []
    for source in (root / 'codex/sessions').rglob('rollout-*.jsonl'):
        raw = source.read_bytes()
        captured_at = datetime.now(timezone.utc).isoformat()
        rows = complete_rows(raw.decode('utf-8'))
        tails.append({'file': source.name, 'complete_rows': len(rows),
                      'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw),
                      'captured_at': captured_at,
                      'tail': [public_row(row) for row in rows[-60:]]})
    atomic_write(out / 'rollout-tail.json', json.dumps(tails, indent=2) + '\n')


if __name__ == '__main__':
    capture()
