"""Run2's binding evidence predicates, independent of runtime and credentials."""
from datetime import datetime


def stamp(value):
    try:return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()
    except (AttributeError,ValueError):return float('-inf')


def pending(rows,message_id,health,*,activity=None,now=None,obligations=()):
    accepted=next((r for r in rows if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('message_id')==message_id),None)
    if not accepted or not activity or now is None:return None
    if activity.get('activity_confidence') not in ('active','likely_working'):return None
    if not 0<=now-stamp(activity.get('observed_at'))<=120:return None
    receipts=[r for r in rows if r.get('payload',{}).get('message_id')==message_id and r.get('event_type') in ('receipt','delivery_receipt')]
    if receipts:return None
    accepted_at=stamp(accepted.get('committed_at'))
    if now<accepted_at:return None
    obligation=next((r for r in obligations if r.get('obligation',{}).get('message_id')==message_id and r['obligation'].get('recipient')=='alpha'),None)
    heartbeat=stamp(health.get('heartbeat'))
    if obligation:opportunity={'obligation':obligation}
    elif heartbeat>=accepted_at:opportunity={'health':health}
    else:return None
    return {'source':'accepted without receipt while working; scheduler opportunity verified','message_id':message_id,'accepted':accepted,'receipt_count':0,'scheduler_opportunity':opportunity}


def format_boundary(config,marker,before,after,verified):
    assert config.get('messaging_format',1)==1,'required resulting format 1 not observed'
    assert marker.get('transition')=='complete','format boundary not committed'
    assert marker.get('legacy_cut')==config.get('messaging_downgrade_sha256') and verified,'format report digest not verified'
    assert before and after==before,'canonical history not retained unchanged'


def assistant_reply(rows,marker,after):
    for row in rows:
        payload=row.get('payload',{})
        if stamp(row.get('timestamp'))<stamp(after):continue
        if row.get('type')=='response_item' and payload.get('type')=='message' and payload.get('role')=='assistant':
            if any(marker in c.get('text','') for c in payload.get('content',[])):return row
    return None
