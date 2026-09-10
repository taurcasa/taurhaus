"""Pure evidence filtering/accounting; never opens a host connection."""
import json
import re


def clean(value):
    if isinstance(value, dict):
        if value.get('method', '').startswith('account/'):
            return None
        return {k: '<redacted>' if ('installation' in k.lower() or k.lower() in
                ['accountid', 'account_id', 'auth', 'controlauthtokenhash',
                 'accesstoken', 'refreshtoken', 'idtoken', 'access_token',
                 'refresh_token', 'id_token']) else clean(v)
                for k, v in value.items() if k not in ['rate_limits', 'rateLimits']}
    if isinstance(value, list):
        return [v for item in value if (v := clean(item)) is not None]
    if isinstance(value, str):
        return re.sub(r'(?<![\w/-])/home/[^/\s"\']+/(?!projects/(?:taurhaus-trial|mesh-trial|mesh-push)(?:/|(?![\w.-])))[^\s"\']*',
                      '<operator-path-redacted>', value)
    return value


def ledger(events, rollout_turn_ids=()):
    starts = set(rollout_turn_ids)
    generations, seen = [], set()
    for event in events:
        p = event.get('params', {})
        if event.get('method') == 'turn/started':
            starts.add(p['turn']['id'])
        if event.get('method') != 'thread/tokenUsage/updated':
            continue
        key = json.dumps(p, sort_keys=True)
        if key in seen:
            continue
        seen.add(key)
        u = p['tokenUsage']['last']
        i, c, o = u['inputTokens'], u['cachedInputTokens'], u['outputTokens']
        generations.append({'thread_id': p['threadId'], 'turn_id': p['turnId'],
            'input': i, 'cached_input': c, 'output': o,
            'reasoning_output': u.get('reasoningOutputTokens', 0),
            'api_equivalent_usd': round(((i-c)*.20+c*.02+o*1.20)/1e6, 9),
            'conservative_usd': round((i+o)*1.20/1e6, 9)})
    missing = starts - {g['turn_id'] for g in generations}
    return {'model': 'gpt-5.6-luna', 'effort': 'low', 'turn_ids': sorted(starts),
        'paid_inputs': len(starts), 'generations': generations,
        'api_equivalent_usd': round(sum(g['api_equivalent_usd'] for g in generations), 9),
        'conservative_usd': round(sum(g['conservative_usd'] for g in generations), 9),
        'metering_complete': not missing, 'unmetered_turn_ids': sorted(missing),
        'basis': 'Host tokenUsage.last via daemon hosted_transcript; packet rates '
                 '$0.20/$0.02/$1.20 per million input/cached/output. Estimate, not invoice.'}
