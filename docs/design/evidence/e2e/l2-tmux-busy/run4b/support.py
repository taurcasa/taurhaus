"""Offline evidence shaping and conservative Luna accounting. No runtime side effects."""
import json
import re


def clean(value):
    if isinstance(value, dict):
        return {k:clean(v) for k,v in value.items() if not any(word in k.lower().replace('_','') for word in ('installationid','accountusage','accountobservations','idtoken','apikey','accesstoken','refreshtoken','authorization')) and k not in ('auth','token','rate_limits')}
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
    turns={}; usage_rows=[]
    keys=('input_tokens','cached_input_tokens','output_tokens')
    def priced(usage):
        return (max(0,usage['input_tokens']-usage['cached_input_tokens'])*.2+usage['cached_input_tokens']*.02+usage['output_tokens']*1.2)/1e6
    for rows in sessions:
        current=None; previous=dict.fromkeys(keys,0)
        for row in rows:
            p=row.get('payload',{})
            if row.get('type')!='event_msg':continue
            if p.get('type')=='task_started':
                current=p['turn_id'];turns.setdefault(current,{'turn_id':current,'usd':None,'completed':False})
            elif p.get('type')=='token_count' and p.get('info'):
                total=p['info'].get('total_token_usage',{})
                usage={k:max(0,total.get(k,0)-previous[k]) for k in keys}
                previous={k:total.get(k,0) for k in keys}
                if any(usage.values()):
                    usage_rows.append({'at':row.get('timestamp'),'turn_id':current,'usage':usage,'usd':priced(usage)})
                    if current:
                        t=turns[current];t.setdefault('usage',dict.fromkeys(keys,0))
                        for k in keys:t['usage'][k]+=usage[k]
            elif p.get('type') in ('task_complete','turn_aborted'):
                ident=p.get('turn_id',current)
                if ident in turns:turns[ident]['completed']=True
                current=None
    for p in notifications:
        ident=p.get('turn-id',p.get('turn_id'))
        if ident:turns.setdefault(ident,{'turn_id':ident,'usd':None,'completed':True,'source':'notify-only','classification':'unknown-but-not-a-model-input'})
    for t in turns.values():
        if 'usage' in t:
            t['usd']=priced(t['usage'])
            t['conservative_usd']=(t['usage']['input_tokens']+t['usage']['output_tokens'])*1.2/1e6
    model_turns=[t for t in turns.values() if t.get('source')!='notify-only']
    return {'paid_inputs':len(model_turns),'turns':list(turns.values()),'usage_rows':usage_rows,
            'metering_complete':all(t['usd'] is not None and t['completed'] for t in model_turns),
            'api_equivalent_usd':sum(r['usd'] for r in usage_rows),
            'conservative_usd':sum((r['usage']['input_tokens']+r['usage']['output_tokens'])*1.2/1e6 for r in usage_rows),
            'rates_usd_per_million':{'input':.2,'cached_input':.02,'output':1.2},
            'rate_source':'Inherited integration/messaging packet; estimates, not invoices'}


def onboarding_delivered(record, activity, snapshot, rows, now):
    if not attributed_idle(record,activity,now) or not ready_session(record,snapshot):return False
    cards={r['payload']['message_id'] for r in rows if r.get('event_type')=='message_accepted'
           and '[taurhaus] recovery_card' in r.get('payload',{}).get('body','')
           and any(t.get('recipient')=='alpha' for t in r['payload'].get('delivery_targets',[]))}
    for mid in cards:
        receipts=[r.get('payload',{}) for r in rows if r.get('payload',{}).get('message_id')==mid]
        if any(p.get('stage') in ('submitted','consumed') and p.get('adapter')=='tmux/1' and p.get('recipient')=='alpha' for p in receipts) and any(p.get('kind')=='consumed_by_read' for p in receipts):return mid
    return False


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


def validate_candidate(*, product_commit, product_diff, mesh_commit, mesh_diff, protocol):
    """Refuse a stale candidate or any product/descriptor edits before launch."""
    if not product_commit.startswith('1db4f9bf') or product_diff:
        raise ValueError('run 4 requires unchanged Taurhaus product at 1db4f9bf')
    if not mesh_commit.startswith('ed59187') or mesh_diff:
        raise ValueError('run 4 requires unchanged Mesh at ed59187')
    if protocol != 27:
        raise ValueError('run 4 requires protocol 27')
