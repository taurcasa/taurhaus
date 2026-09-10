"""Offline evidence shaping and conservative Luna accounting. No runtime side effects."""
import json
import re


def startup_composer(pane):
    return any(line.lstrip().startswith('› ') for line in pane.splitlines()) and bool(re.search(r'gpt-5\.6-luna\s+low',pane))


def composer_text(pane):
    """Reuse L1 run-3's last-prompt predicate, allowing wrapped composer text."""
    lines=pane.splitlines()
    prompts=[i for i,line in enumerate(lines) if line.strip().startswith('›')]
    if not prompts:return None
    start=prompts[-1]
    parts=[lines[start].strip()[1:].strip()]
    for line in lines[start+1:]:
        if not line.strip():break
        parts.append(line.strip())
    # Placeholder is sufficient even when the footer immediately follows it.
    if parts[0] in ('','Ask Codex to do anything'):return parts[0]
    return ' '.join(parts)


def confirm_submission(text,send,read,now,sleep,prior_turns):
    """Bounded paste/Enter handshake; never resubmit text or retry a model turn."""
    send(text)
    deadline=now()+5
    while True:
        pane,rows=read()
        composer=composer_text(pane)
        if composer and text in composer:break
        if now()>=deadline:raise RuntimeError('harness: pasted input not visible in composer')
        sleep(.2)
    for attempt in (1,2):
        send('Enter')
        deadline=now()+10
        while True:
            pane,rows=read()
            composer=composer_text(pane)
            started={r.get('payload',{}).get('turn_id') for r in rows
                     if r.get('type')=='event_msg' and r.get('payload',{}).get('type')=='task_started'}
            fresh=started-prior_turns
            if composer in ('','Ask Codex to do anything') and fresh:
                return {'confirmed_at':now(),'enter_count':attempt,'composer_empty':True,'new_turn_ids':sorted(fresh)}
            if now()>=deadline:break
            sleep(.2)
        # A second Enter is authorized only while the pasted text remains.
        if not composer or text not in composer:break
    raise RuntimeError('harness: Codex submission not confirmed after at most two Enters')


def clean(value):
    if isinstance(value, dict):
        return {k:clean(v) for k,v in value.items() if not any(word in k.lower().replace('_','') for word in ('installationid','accountusage','accountobservations','idtoken','apikey','accesstoken','refreshtoken','authorization')) and k not in ('auth','token','rate_limits')}
    if isinstance(value,list):return [clean(v) for v in value]
    if isinstance(value,str):
        return re.sub(r'(?<![\w/.-])/home/[^/\s]+/(?!projects/(?:taurhaus-l6-rollback|mesh-l6)(?:/|\b))[^\s"\']*','<operator-path-redacted>',value)
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
                if ident in turns:
                    turns[ident]['completed']=True
                    if p.get('type')=='turn_aborted':turns[ident]['interrupted']=True
                current=None
    for p in notifications:
        ident=p.get('turn-id',p.get('turn_id'))
        if ident:turns.setdefault(ident,{'turn_id':ident,'usd':None,'completed':True,'source':'notify-only','classification':'unknown-but-not-a-model-input'})
    for t in turns.values():
        if 'usage' in t:
            t['usd']=priced(t['usage'])
            t['conservative_usd']=(t['usage']['input_tokens']+t['usage']['output_tokens'])*1.2/1e6
    for t in turns.values():
        if t.get('interrupted') and t['usd'] is None:t['classification']='interrupted, cost unknown'
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


def pending_observation(rows,message_id,health,*,activity=None,now=None):
    if any(r.get('payload',{}).get('message_id')==message_id and r.get('payload',{}).get('stage') in ('submitted','consumed','native_enqueued') for r in rows):return None
    receipt=pending_receipt(rows,message_id)
    if receipt:return {'source':'journal','receipt':receipt}
    accepted=next((r for r in rows if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('message_id')==message_id),None)
    if accepted and activity and activity.get('activity_confidence') in ('active','likely_working'):
        from datetime import datetime
        accepted_at=datetime.fromisoformat(accepted['committed_at'].replace('Z','+00:00')).timestamp()
        observed_at=datetime.fromisoformat(activity['observed_at'].replace('Z','+00:00')).timestamp()
        receipts=[r for r in rows if r.get('event_type') in ('receipt','delivery_receipt') and r.get('payload',{}).get('message_id')==message_id]
        if health.get('heartbeat','')>=accepted['committed_at'] and now is not None and now>=accepted_at and 0<=now-observed_at<=120 and not receipts:
            return {'source':'message accepted without receipt while working','message_id':message_id,'accepted':accepted,'receipt_count':0,'health_corroboration':health}
    if accepted and health.get('last_defer_reason') and health['heartbeat']>=accepted['committed_at']:
        return {'source':'scheduler_health (not a receipt)','message_id':message_id,'accepted':accepted,'health':health}
    return None


def ready_session(record,snapshot):
    if snapshot.get('degraded'):return None
    return next((row for row in snapshot.get('runtime_sessions',[]) if row.get('session_id')==record.get('session_id') and record.get('session_id') and row.get('tmux_pane')==record.get('paneId') and row.get('state')=='idle' and row.get('activity_attribution')=='attributed'),None)


def validate_candidate(*, product_commit, product_diff, mesh_commit, mesh_diff, protocol):
    """Refuse a stale candidate or any product/descriptor edits before launch."""
    if not product_commit.startswith('a7e6db7e') or product_diff:
        raise ValueError('lane 6 requires unchanged Taurhaus product at a7e6db7e')
    if not mesh_commit.startswith('310144d') or mesh_diff:
        raise ValueError('lane 6 requires unchanged Mesh at 310144d')
    if protocol != 27:
        raise ValueError('lane 6 requires protocol 27')


def reply_evidence(journal,rollout,message_id,marker):
    """Run-4d ruling: journal reply or any native row after this submission."""
    from datetime import datetime
    def timestamp(value):
        try:return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()
        except (AttributeError,ValueError):return float('-inf')
    submissions=[r for r in journal if r.get('payload',{}).get('message_id')==message_id and r.get('payload',{}).get('stage')=='submitted']
    if not submissions:return None
    delivery=min(timestamp(r['payload'].get('observed_at',r.get('committed_at'))) for r in submissions)
    for row in journal:
        p=row.get('payload',{})
        if row.get('event_type')=='message_accepted' and p.get('author',{}).get('name')=='alpha' and marker in p.get('body','') and timestamp(row.get('committed_at'))>=delivery:
            return {'source':'alpha journal reply','delivery_at':delivery,'row':row}
    for row in rollout:
        if timestamp(row.get('timestamp'))>=delivery and marker in json.dumps(row):
            return {'source':'post-delivery rollout row','delivery_at':delivery,'row':row}
    return None
