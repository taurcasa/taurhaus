"""Pure evidence filtering/accounting; never opens a host connection."""
import json
import re


def allowed_operator_path(path):
    # Only worktree paths may be retained literally.
    return any(
        path == root or path.startswith(root + '/') for root in
        ['/home/mstie/projects/taurhaus-l5-restarts', '/home/mstie/projects/mesh-l5'])


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
        value = re.sub(r"/home/[^/\s]+/\.codex[^/\s]*/auth\.json(?=$|[\s\"'])", '<authorized-source>', value)
        return re.sub(r"(?<![\w/-])/home/[^/\s\"']+/[^\s\"']*",
                      lambda m: m[0] if allowed_operator_path(m[0]) else '<operator-path-redacted>', value)
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


def enforce_budget(turns, api_equivalent_usd):
    """Require headroom before another paid submission; lifecycle calls bypass this."""
    assert turns < 20, 'hard input cap: no further paid submission'
    assert api_equivalent_usd < .30, 'hard metered dollar cap: no further paid submission'


def owner_evidence(observations):
    """Assess a bounded census; empty matches cannot establish owner exclusion."""
    maximum = max((len(o.get('owners', [])) for o in observations), default=0)
    epochs = {o['epoch']['epoch'] for o in observations
              if o.get('owners') and o.get('epoch') and 'epoch' in o['epoch']}
    if maximum > 1:
        outcome, classification, reason = 'FAIL', 'runtime', 'overlapping delivery owners observed'
    elif maximum == 0:
        outcome, classification, reason = 'UNPROVED', 'harness', 'owner-process filter matched nothing; zero owners sampled'
    elif len(epochs) < 2 or any('error' in o for o in observations):
        outcome, classification, reason = 'UNPROVED', 'harness', 'owner census lacks readable observations across both epochs'
    else:
        outcome, classification, reason = 'PASS', 'runtime', 'one owner per passive sample across both epochs; bounded evidence only'
    return {'outcome': outcome, 'classification': classification, 'reason': reason,
            'samples': len(observations), 'max_simultaneous_observed_owners': maximum}


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



def read_by_seat(receipts, seat=None):
    return any(r.get('kind') == 'consumed_by_read' and
               (seat is None or r.get('reader_name') == seat) for r in receipts)


def delivered(receipts, activity, session, seat=None, card_seen=False, transport='tmux'):
    consumed = read_by_seat(receipts, seat)
    submitted = any(r.get('stage') in ['submitted', 'native_enqueued'] for r in receipts)
    exposed = (any(r.get('stage') == 'native_enqueued' for r in receipts) and card_seen
               if transport == 'app_server' else consumed or (submitted and card_seen))
    return (exposed
        and activity.get('state') == 'idle' and activity.get('age',999) <= 120
        and activity.get('session_id') == session)


def host_card_seen(events, session, body):
    return bool(body) and any(e.get('method') in ['item/started', 'item/completed']
        and e.get('params', {}).get('threadId') == session
        and e['params'].get('item', {}).get('type') == 'userMessage'
        and any(body in c.get('text', '') for c in e['params']['item'].get('content', []))
        for e in events)


def hosted_startup_ready(rows, events, seat, activity, recipient):
    if not recipient or activity.get('runtime', {}).get('source') != 'host':
        return False
    if not any(r.get('event') == 'onboarding.delivery.observed'
        and r.get('stage') == 'submitted' and r.get('path') == 'app_server'
        and r.get('card_key', {}).get('recipient') == recipient for r in rows):
        return False
    turns = set()
    for e in events:
        p=e.get('params', {}); item=p.get('item', {})
        if e.get('method') not in ['item/started', 'item/completed'] or p.get('threadId') != activity.get('session_id') or item.get('type') != 'userMessage':
            continue
        for c in item.get('content', []):
            text=c.get('text', '')
            if '[taurhaus] recovery_card' in text and f'Identity: {seat} on ' in text:
                try: key=json.loads(text.split('key=',1)[1].splitlines()[0])
                except (ValueError, IndexError): continue
                if key.get('recipient') == recipient: turns.add(p.get('turnId'))
    return any(e.get('method') == 'turn/completed'
        and e.get('params', {}).get('threadId') == activity.get('session_id')
        and e['params'].get('turn', {}).get('id') in turns
        and e['params']['turn'].get('status') == 'completed' for e in events)


def send_ready(journal, seat, activity, onboarding=False, tool_results=(),
               transport='tmux', startup_rows=(), host_events=(), recipient=None):
    accepted = [r['payload'] for r in journal if r.get('event_type') == 'message_accepted'
                and any(t.get('recipient') == seat for t in r['payload'].get('delivery_targets', []))]
    cards = [p for p in accepted if '[taurhaus] recovery_card' in p.get('body', '')]
    startup = (hosted_startup_ready(startup_rows, host_events, seat, activity, recipient)
               if transport == 'app_server' else bool(cards))
    if onboarding and (not startup or activity.get('state') != 'idle'
                       or activity.get('age',999) > 120 or not activity.get('session_id')):
        return False
    for p in accepted:
        receipts = [r['payload'] for r in journal if r.get('event_type') != 'message_accepted'
                    and r.get('payload', {}).get('message_id') == p['message_id']]
        submitted = any(r.get('stage') in ['submitted', 'native_enqueued'] for r in receipts)
        card_seen = bool(p.get('body')) and any(p['body'] in str(t.get('payload',{}).get('output',''))
                       for t in tool_results if t.get('payload',{}).get('type') in
                       ['function_call_output','custom_tool_call_output'])
        exposed = (any(r.get('stage') == 'native_enqueued' for r in receipts)
                   and host_card_seen(host_events, activity.get('session_id'), p.get('body'))
                   if transport == 'app_server' else read_by_seat(receipts, seat) or (submitted and card_seen))
        if not exposed:
            return False
        if onboarding and transport == 'tmux' and p in cards and not any(r.get('stage') == 'submitted' for r in receipts):
            return False
    return True

def pending(journal, message_id, recipient, activity):
    matching=[r for r in journal if r.get('payload',{}).get('message_id')==message_id]
    accepted=any(r.get('event_type')=='message_accepted' and
        any(t.get('recipient')==recipient for t in r['payload'].get('delivery_targets',[]))
        for r in matching)
    exposed=any(r.get('payload',{}).get('stage') in
        ['submitted','consumed','native_enqueued','outcome_unknown'] or
        r.get('payload',{}).get('kind') == 'consumed_by_read' for r in matching)
    return accepted and not exposed and busy(activity)

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


def busy(session):
    return (session.get('activity_attribution') == 'attributed'
            and bool(session.get('session_id'))
            and session.get('state') in ['active', 'likely_working', 'working'])


def retry_busy(code, text, now, deadline):
    return code != 0 and now < deadline and any(s in text.lower() for s in ['host member busy','lock busy'])


def host_needs_resume(probe):
    return 'error' in probe or probe.get('result',{}).get('stopped') is True


def owner_window_evidence(observations, start, end, old_pid, new_pid, first_delivery):
    before=[o for o in observations if o['at']<=start]
    after=[o for o in observations if o['at']>=end]
    window=([before[-1]] if before else [])+[o for o in observations if start<o['at']<end]+([after[0]] if after else [])
    result=owner_evidence(window)
    gaps=[b['at']-a['at'] for a,b in zip(window,window[1:])]
    gone=next((o['at'] for o in window if o['at']>=start and old_pid not in [p['pid'] for p in o.get('owners',[])]),None)
    new_seen=any(new_pid in [p['pid'] for p in o.get('owners',[])] for o in window)
    valid=bool(before and after and gaps and max(gaps)<=1 and gone is not None and gone<first_delivery and new_seen and old_pid!=new_pid)
    if result['outcome']!='FAIL' and not valid:
        result.update(outcome='UNPROVED',classification='harness',reason='cadence, coverage or old-owner departure before delivery unproved')
    result.update(window_start=start,window_end=end,max_gap_seconds=max(gaps,default=None),old_pid=old_pid,new_pid=new_pid,old_gone_observed_at=gone,first_delivery_at=first_delivery)
    return result


def transport_proven(accepted, receipts, seat, transport, witness):
    """Step 5 proves one accepted target, one attempt and its native transport witness."""
    targets=[t for t in accepted.get('delivery_targets', []) if t.get('recipient')==seat]
    if len(targets)!=1 or not witness:
        return False
    target=targets[0]['delivery_id']
    matching=[r for r in receipts if r.get('delivery_id')==target and r.get('recipient')==seat]
    attempts=[r for r in matching if r.get('stage')=='attempt_started']
    exposed=[r for r in matching if r.get('stage') in ['submitted','native_enqueued']]
    stage='native_enqueued' if transport=='app_server' else 'submitted'
    return (len(attempts)==len(exposed)==1 and exposed[0]['stage']==stage
            and bool(attempts[0].get('attempt_id'))
            and attempts[0]['attempt_id']==exposed[0].get('attempt_id'))
