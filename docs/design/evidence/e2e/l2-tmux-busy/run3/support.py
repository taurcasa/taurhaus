"""Offline evidence shaping and conservative Luna accounting. No runtime side effects."""
import json
import re


def clean(value):
    if isinstance(value, dict):
        return {k:clean(v) for k,v in value.items() if not any(word in k.lower().replace('_','') for word in ('installationid','accountusage','accountobservations','idtoken','apikey','accesstoken','refreshtoken','authorization')) and k not in ('auth','token')}
    if isinstance(value,list):return [clean(v) for v in value]
    if isinstance(value,str):
        return re.sub(r'(?<![\w/.-])/home/[^/\s]+/(?!projects/(?:taurhaus-l2-tmux-busy|mesh-l2)(?:/|\b))[^\s"\']*','<operator-path-redacted>',value)
    return value


def complete_rows(text):
    rows=[]
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'):continue
        try:rows.append(json.loads(line))
        except ValueError:continue
    return rows


def pending_receipt(rows, message_id):
    return next((p for r in rows if (p:=r.get('payload',r)).get('message_id')==message_id and p.get('stage')=='pending'),None)


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


def pending_observation(rows,message_id,health):
    if any(r.get('payload',{}).get('message_id')==message_id and r.get('payload',{}).get('stage') in ('submitted','consumed','native_enqueued') for r in rows):return None
    receipt=pending_receipt(rows,message_id)
    if receipt:return {'source':'journal','receipt':receipt}
    accepted=next((r for r in rows if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('message_id')==message_id),None)
    if accepted and health.get('last_defer_reason') and health['heartbeat']>=accepted['committed_at']:
        return {'source':'scheduler_health (not a receipt)','message_id':message_id,'accepted':accepted,'health':health}
    return None


def ready_session(record,snapshot):
    if snapshot.get('degraded'):return None
    return next((row for row in snapshot.get('runtime_sessions',[]) if row.get('session_id')==record.get('session_id') and record.get('session_id') and row.get('tmux_pane')==record.get('paneId') and row.get('state')=='idle' and row.get('activity_attribution')=='attributed'),None)
