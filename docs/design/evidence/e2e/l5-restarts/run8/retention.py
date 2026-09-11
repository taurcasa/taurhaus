"""Bounded evidence representations; raw diagnostics stay in the disposable root."""
def retained_view(view):
    return {**{key: value for key, value in view.items() if key != 'events'},
            'evidenceEvents': 'host-events.jsonl'}


def retain_host_event(event):
    # Complete items preserve the full text; intermediate streaming chunks do not
    # establish additional exposure, receipt, identity, or token accounting.
    return not event.get('method', '').endswith('/delta')
