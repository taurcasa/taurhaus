"""Offline evidence shaping and conservative Luna accounting. No runtime side effects."""
import json
import re


def clean(value):
    if isinstance(value, dict):
        if str(value.get('event', '')).startswith('usage.'):
            return {k: clean(v) for k,v in value.items() if k in ('ts','level','component','event','run_id')} | {'private_usage_details': '<redacted>'}
        daemon = 'event' in value and 'component' in value
        return {k: ('<message-body-redacted>' if k in ('body', 'message', 'text', 'content', 'summary', 'description', 'instructions') and not (daemon and k == 'message') else clean(v))
                for k, v in value.items()
                if not any(word in k.lower().replace('_', '') for word in ('installationid', 'accountusage', 'accountobservations', 'controltoken', 'idtoken', 'accesstoken', 'refreshtoken', 'apikey', 'authorization')) and k not in ('auth', 'token')}
    if isinstance(value, list):
        return [clean(v) for v in value]
    if isinstance(value, str):
        value = re.sub(r'(?<![\w/.-])/home/[^/\s]+/(?!projects/(?:taurhaus-l7-ledger|mesh-l7)(?:/|\b))[^\s"\']*', '<operator-path-redacted>', value)
        value = re.sub(r'(?i)(?:(?:member[_-]?)?control[_-]?token|access[_-]?token|refresh[_-]?token)[=: ]+[^\s,}]+', '<secret-redacted>', value)
        return value
    return value


def complete_rows(text):
    rows=[]
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'):continue
        try:rows.append(json.loads(line))
        except ValueError:continue
    return rows


def daemon_rows(text):
    """Retain each physical source line, including a complete non-newline tail."""
    import hashlib
    rows = []
    for line in text.splitlines():
        try:
            rows.append(clean(json.loads(line)))
        except ValueError:
            rows.append({'event': 'evidence.unparsed_daemon_line',
                         'source_line': clean(line),
                         'sha256': hashlib.sha256(line.encode()).hexdigest()})
    return rows


def meter(sessions, notifications):
    turns={}
    for rows in sessions:
        current=None; previous={'input_tokens':0,'cached_input_tokens':0,'output_tokens':0}
        for row in rows:
            p=row.get('payload',{})
            if row.get('type')!='event_msg':continue
            if p.get('type')=='task_started':
                current=p['turn_id'];turns.setdefault(current,{'turn_id':current,'usd':None,'completed':False})
            elif p.get('type')=='token_count' and p.get('info') and current:
                total=p['info'].get('total_token_usage',{})
                usage={k:max(0,total.get(k,0)-previous.get(k,0)) for k in previous}
                # Keep the latest cumulative delta for this turn, not every streaming update.
                turns[current]['usage']=usage
            elif p.get('type')=='task_complete' and p.get('turn_id') in turns:
                current=p['turn_id'];t=turns[current];t['completed']=True
                if 'usage' in t:
                    for k in previous:previous[k]+=t['usage'][k]
                current=None
    for p in notifications:
        ident=p.get('turn-id',p.get('turn_id'))
        if ident:turns.setdefault(ident,{'turn_id':ident,'usd':None,'completed':True,'source':'notify-only'})
    for t in turns.values():
        if 'usage' in t:
            u=t['usage'];t['usd']=(max(0,u['input_tokens']-u['cached_input_tokens'])*.2+u['cached_input_tokens']*.02+u['output_tokens']*1.2)/1e6
            t['conservative_usd']=(u['input_tokens']+u['output_tokens'])*1.2/1e6
    return {'paid_inputs':len(turns),'turns':list(turns.values()),'metering_complete':all(t['usd'] is not None and t['completed'] for t in turns.values()),'api_equivalent_usd':sum(t['usd'] or 0 for t in turns.values()),'conservative_usd':sum(t.get('conservative_usd',0) for t in turns.values()),'rates_usd_per_million':{'input':.2,'cached_input':.02,'output':1.2},'rate_source':'Inherited integration/messaging packet; estimates, not invoices'}


def native_runtime(native):
    """Resolve both siblings before copying any executable."""
    from pathlib import Path
    native=Path(native)
    result=[('codex',native),('codex-code-mode-host',native.with_name('codex-code-mode-host'))]
    for _,path in result:
        if not path.is_file():raise FileNotFoundError('incomplete native Codex runtime: '+path.name)
    return result


def retained_daemon_rows(rows):
    # Preserve row count, event order and shutdown rows; sanitize private fields.
    return [clean(row) for row in rows]


def evidence_jsonl(rows):
    return ''.join(json.dumps(clean(row))+'\n' for row in rows)


def attributed_idle(record,activity,now):
    from datetime import datetime
    try:age=now-datetime.fromisoformat(activity['observed_at'].replace('Z','+00:00')).timestamp()
    except (KeyError,ValueError):return False
    return bool(record.get('session_id')) and activity.get('activity_confidence')=='idle' and 0<=age<=120


def delivered(rows, message_id, rollout):
    receipts = [r.get('payload', {}) for r in rows if r.get('payload', {}).get('message_id') == message_id and (r.get('payload', {}).get('recipient') == 'alpha' or r.get('payload', {}).get('reader_name') == 'alpha')]
    if any(p.get('kind') == 'consumed_by_read' and p.get('reader_name', p.get('recipient')) == 'alpha' for p in receipts):
        return True
    return any(p.get('stage') in ('submitted', 'native_enqueued') for p in receipts) and any(r.get('payload', {}).get('type') in ('function_call_output', 'custom_tool_call_output') and message_id in json.dumps(r) for r in rollout)


def ready(rows, message_id, idle):
    receipts = [r.get('payload', {}) for r in rows if r.get('payload', {}).get('message_id') == message_id and (r.get('payload', {}).get('recipient') == 'alpha' or r.get('payload', {}).get('reader_name') == 'alpha')]
    return bool(idle) and any(p.get('stage') == 'submitted' for p in receipts) and any(p.get('kind') == 'consumed_by_read' and p.get('reader_name', p.get('recipient')) == 'alpha' for p in receipts)


def receipt_retry(original, retried, before_count, after_count):
    return original == retried and before_count == after_count
