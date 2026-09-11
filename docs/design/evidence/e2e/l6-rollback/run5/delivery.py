"""Run5 delivery rule, shared by startup, later markers and readiness checks."""
from datetime import datetime
import json
from support import attributed_idle, ready_session


def delivery_evidence(journal, native, message_id, body):
    receipts=[r for r in journal if r.get('payload',{}).get('message_id')==message_id]
    reads=[r for r in receipts if r.get('payload',{}).get('kind')=='consumed_by_read' and r['payload'].get('reader_name')=='alpha']
    submitted=[r for r in receipts if r.get('payload',{}).get('stage')=='submitted' and r['payload'].get('recipient')=='alpha']
    tool=[r for r in native if r.get('type')=='response_item' and r.get('payload',{}).get('type') in ('function_call_output','custom_tool_call_output') and body in json.dumps(r['payload'])]
    if reads:return {'mode':'consumed_by_read','reads':reads,'submitted':submitted,'tool':tool,'at':reads[0]['payload'].get('observed_at',reads[0].get('committed_at'))}
    if submitted and tool:return {'mode':'submitted_with_tool_result','reads':reads,'submitted':submitted,'tool':tool,'at':submitted[0]['payload'].get('observed_at',submitted[0].get('committed_at'))}
    return None


def onboarding_delivered(record,activity,snapshot,rows,now,*,native=()):
    if not attributed_idle(record,activity,now) or not ready_session(record,snapshot):return False
    for row in rows:
        card=row.get('payload',{})
        if row.get('event_type')=='message_accepted' and '[taurhaus] recovery_card' in card.get('body','') and any(t.get('recipient')=='alpha' for t in card.get('delivery_targets',[])):
            if delivery_evidence(rows,native,card['message_id'],card['body']):return card['message_id']
    return False


def reply_evidence(journal,native,message_id,marker):
    delivery=delivery_evidence(journal,native,message_id,marker)
    if not delivery:return None
    after=datetime.fromisoformat(delivery['at'].replace('Z','+00:00'))
    for row in native:
        p=row.get('payload',{})
        if row.get('type')=='response_item' and p.get('type')=='message' and p.get('role')=='assistant' and datetime.fromisoformat(row['timestamp'].replace('Z','+00:00'))>=after:
            if any(c.get('text','').strip().rstrip('.')==marker for c in p.get('content',[])):return {'source':'assistant reply after delivery','row':row,'delivery':delivery}
    for row in journal:
        p=row.get('payload',{})
        if row.get('event_type')=='message_accepted' and p.get('author',{}).get('name')=='alpha' and p.get('body','').strip()==marker and datetime.fromisoformat(row['committed_at'].replace('Z','+00:00'))>=after:
            return {'source':'alpha journal reply','row':row,'delivery':delivery}
    return None
