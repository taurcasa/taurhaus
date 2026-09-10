"""Pure auth-path selection and cumulative Luna usage accounting."""
from pathlib import Path


def auth_source(environment, operator_home):
    return Path(environment.get('CODEX_HOME', str(operator_home / '.codex'))) / 'auth.json'


def meter(starts, usage):
    turns = {}
    for row in usage:
        key = row['turn_id']
        old = turns.get(key)
        if old is None or row['input'] + row['output'] >= old['input'] + old['output']:
            turns[key] = row
    for row in turns.values():
        i, c, o = row['input'], row.get('cached_input', 0), row['output']
        row['api_equivalent_usd'] = round(((i-c)*.20+c*.02+o*1.20)/1e6, 9)
        row['conservative_usd'] = round((i+o)*1.20/1e6, 9)
    ids = set(starts) | set(turns)
    return {'paid_inputs':len(ids), 'turn_ids':sorted(ids), 'turns':list(turns.values()),
            'unmetered':sorted(set(starts)-set(turns)),
            'api_equivalent_usd':round(sum(r['api_equivalent_usd'] for r in turns.values()),9),
            'conservative_usd':round(sum(r['conservative_usd'] for r in turns.values()),9),
            'basis':'Prior trial packet Luna input/cached/output rates $0.20/$0.02/$1.20 per million; conservative all tokens at $1.20/M, not an invoice.'}


def require_headroom(ledger, inputs):
    assert ledger['paid_inputs'] + inputs <= 16, 'input cap'
    assert not ledger['unmetered'], 'unmetered turns before next paid input'
    assert ledger['conservative_usd'] + .05 * inputs <= .25, 'cost headroom'
