"""Offline evidence shaping and conservative Luna accounting. No runtime side effects."""
import json
import re


def clean(value):
    if isinstance(value, dict):
        return {k:clean(v) for k,v in value.items() if not any(word in k.lower().replace('_','') for word in ('installationid','accountusage','accesstoken','refreshtoken','authorization')) and k not in ('auth','token')}
    if isinstance(value,list):return [clean(v) for v in value]
    if isinstance(value,str):
        return re.sub(r'/home/[^/\s]+/(?!projects/(?:taurhaus-l2-tmux-busy|mesh-l2)(?:/|\b))[^\s"\']*','<operator-path-redacted>',value)
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
