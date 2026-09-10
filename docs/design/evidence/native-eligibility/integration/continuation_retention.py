"""Bounded evidence representations; raw diagnostics stay in the disposable root."""
from collections import Counter

PERIODIC = {'inotify.telemetry', 'session_scanner.scan.completed'}


def retained_log(rows):
    counts = Counter(row.get('event') for row in rows)
    first, last = {}, {}
    for index, row in enumerate(rows):
        event = row.get('event')
        if event in PERIODIC:
            first.setdefault(event, index)
            last[event] = index
    indices = set(first.values()) | set(last.values())
    return ([row for index, row in enumerate(rows)
             if row.get('event') not in PERIODIC or index in indices], dict(counts))


def retained_view(view):
    return {**{key: value for key, value in view.items() if key != 'events'},
            'evidenceEvents': 'host-events.jsonl'}


def retain_host_event(event):
    # Complete items preserve the full text; intermediate streaming chunks do not
    # establish additional exposure, receipt, identity, or token accounting.
    return not event.get('method', '').endswith('/delta')
