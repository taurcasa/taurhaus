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
                for k, v in value.items() if k not in ['rate_limits', 'rateLimits', 'account_observations']}
    if isinstance(value, list):
        return [v for item in value if (v := clean(item)) is not None]
    if isinstance(value, str):
        return re.sub(r'(?<![\w/-])/home/[^/\s"\']+/(?!projects/(?:taurhaus-l5-restarts|taurhaus-msg|taurhaus|mesh-l5)(?:/|$))[^\s"\']*',
                      '<operator-path-redacted>', value)
    return value


def ledger(events, rollout_turn_ids=(), notify_records=()):
    starts = set(rollout_turn_ids)
    starts.update(r["turn_id"] for r in notify_records if r.get("turn_id"))
    gaps = set()
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
        starts.add(p['turnId'])
        u = p['tokenUsage']['last']
        if u.get('totalTokens', 0) > 0 and u['inputTokens'] == 0 and u['outputTokens'] == 0:
            gaps.add(p['turnId'])
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
        'metering_complete': not missing and not gaps, 'unmetered_turn_ids': sorted(missing | gaps),
        'basis': 'Host tokenUsage.last via daemon hosted_transcript; packet rates '
                 '$0.20/$0.02/$1.20 per million input/cached/output. Estimate, not invoice.'}


def enforce_budget(turns, conservative_usd):
    assert turns <= 12, 'turn budget reached'
    assert conservative_usd <= .25, 'cost budget reached'


def complete_rows(text):
    rows=[]
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'): continue
        try: rows.append(json.loads(line))
        except ValueError: continue
    return rows


def new_events(previous, current):
    for overlap in range(min(len(previous), len(current)), 0, -1):
        if previous[-overlap:] == current[:overlap]: return current[overlap:]
    return current


def rollout_events(rows, thread_id):
    result=[]; turn=None
    for row in rows:
        p=row.get('payload',{})
        if row.get('type') != 'event_msg': continue
        if p.get('type') == 'task_started':
            turn=p['turn_id']
            result.append({'method':'turn/started','params':{'threadId':thread_id,'turn':{'id':turn}}})
        if p.get('type') != 'token_count' or not p.get('info') or not turn: continue
        def camel(u):
            names={'input_tokens':'inputTokens','cached_input_tokens':'cachedInputTokens','output_tokens':'outputTokens','reasoning_output_tokens':'reasoningOutputTokens','total_tokens':'totalTokens'}
            return {v:u.get(k,0) for k,v in names.items()}
        result.append({'method':'thread/tokenUsage/updated','params':{'threadId':thread_id,'turnId':turn,
          'tokenUsage':{'last':camel(p['info']['last_token_usage']),'total':camel(p['info']['total_token_usage'])}}})
    return result


def consumed_by_read(row):
    return row.get('payload',{}).get('kind') == 'consumed_by_read'



def delivered(receipts, activity, session):
    return (any(r.get('stage') in ['submitted','native_enqueued'] for r in receipts)
        and any(r.get('kind') == 'consumed_by_read' for r in receipts)
        and activity.get('state') == 'idle' and activity.get('age',999) <= 120
        and activity.get('session_id') == session)

def pending(receipts):
    return (any(r.get('stage') == 'pending' for r in receipts)
        and not any(r.get('stage') in ['submitted','native_enqueued','outcome_unknown'] for r in receipts))

def reply_seen(journal, rollout, marker):
    return any(marker in json.dumps(row) for row in journal + rollout)

def identity_preserved(before, after):
    def logical(r): return r.get('appServer',{}).get('threadId') or r.get('session_id')
    return bool(logical(before)) and logical(before)==logical(after) and after.get('attachmentGeneration',0)>=before.get('attachmentGeneration',0)

def attributed_activity(session, sidecar, age):
    attributed=session.get('activity_attribution')=='attributed'
    # The daemon attributes state to the matched session on both transports.
    # The activity sidecar is freshness evidence, not a fallback state authority.
    state=session.get('state')
    return {'state':state,'age':age,'session_id':session.get('session_id') if attributed else None,
            'runtime':session,'snapshot':sidecar}
