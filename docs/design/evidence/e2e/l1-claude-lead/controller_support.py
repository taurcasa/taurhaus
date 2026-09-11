"""Pure bounded-trial accounting and evidence helpers (no runtime side effects)."""
import json
import re

WORKTREES=('/home/mstie/projects/taurhaus-l1-claude-lead','/home/mstie/projects/mesh-l1')

def complete_rows(text):
    rows=[]
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'): continue
        try: rows.append(json.loads(line))
        except ValueError: continue
    return rows

def sanitize(value):
    if isinstance(value,dict):
        return {k:sanitize(v) for k,v in value.items() if k.lower() not in ('auth','token','encrypted_content','signature','rate_limits') and not any(w in k.lower() for w in ('authorization','token_secret','accesstoken','refreshtoken','account','installation','email'))}
    if isinstance(value,list): return [sanitize(v) for v in value]
    if not isinstance(value,str): return value
    value=re.sub(r'[^\n]*Heads up, you have less than [^\n]*limit left\.[^\n]*', '[quota banner redacted]', value)
    for i,p in enumerate(WORKTREES): value=value.replace(p+'/',f'<worktree{i}>/').replace(p+'"',f'<worktree{i}>"')
    for i,p in enumerate(WORKTREES):
        if value==p: return p
    value=re.sub(r'(?<![A-Za-z0-9_./-])/home/[^\s\"\'<>;]+','<operator-path>',value)
    for i,p in enumerate(WORKTREES): value=value.replace(f'<worktree{i}>',p)
    value=re.sub(r'[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}', '<email>',value)
    value=re.sub(r'(?:sk-ant-|sk-proj-|eyJ)[A-Za-z0-9_.-]+','<secret-redacted>',value)
    return value

def claude_usage(rows):
    messages={}
    for row in rows:
        if row.get('type')!='assistant': continue
        msg=row.get('message',{}); usage=msg.get('usage')
        if not msg.get('id') or not usage: raise ValueError('Claude usage unverified')
        fields={k:usage.get(k,0) for k in ('input_tokens','output_tokens','cache_read_input_tokens','cache_creation_input_tokens')}
        if sum(fields.values())==0: raise ValueError('Claude usage unverified')
        old=messages.get(msg['id'],{})
        fields={k:max(v,old.get(k,0)) for k,v in fields.items()}
        fields.update(message_id=msg['id'],model=msg.get('model'),session_id=row.get('sessionId'))
        fields['usd']=(fields['input_tokens']+5*fields['output_tokens']+.1*fields['cache_read_input_tokens']+1.25*fields['cache_creation_input_tokens'])/1e6
        fields['upper_usd']=sum(fields[k] for k in ('input_tokens','output_tokens','cache_read_input_tokens','cache_creation_input_tokens'))*5/1e6
        messages[msg['id']]=fields
    return list(messages.values())

def admit_input(tool,claude_inputs,codex_inputs,spent,next_turn):
    count,limit=(claude_inputs,8) if tool=='claude' else (codex_inputs,12)
    if count>=limit: raise ValueError(tool+' input cap')
    if next_turn<=0 or spent+next_turn>2: raise ValueError('unverified dollar headroom')
    return True


def onboarding_facts(record, activity, rows, journal, daemon_alive, now):
    """Current source reads and native receipts outrank a stale pending projection."""
    from datetime import datetime
    readable = isinstance(record, dict) and bool(record)
    record = record if readable else {}
    try:
        age = now - datetime.fromisoformat(activity['observed_at'].replace('Z', '+00:00')).timestamp()
        fresh = 0 <= age <= 120
    except (TypeError, KeyError, ValueError, AttributeError):
        fresh = False
    card = record.get('recovery', {}).get('last_delivered') or {}
    projection = card.get('journal') or {}
    message_id = projection.get('message_id')
    receipts = [r.get('payload', {}) for r in journal
                if message_id and r.get('payload', {}).get('message_id') == message_id]
    consumed = any(r.get('kind') == 'consumed_by_read' and r.get('reader_name') == 'alpha'
                   for r in receipts)
    exposed = consumed or '[taurhaus] recovery_card' in json.dumps(rows)
    submitted = any(r.get('stage') == 'submitted' for r in receipts)
    return {'daemon_alive': daemon_alive, 'runtime_readable': readable,
            'activity_fresh': fresh,
            'session_attributed': bool(record.get('session_id') and record.get('jsonl_path')),
            'card_pending': projection.get('projection') == 'pending' and not (exposed or submitted),
            'card_exposed': exposed, 'observed_at': now}
