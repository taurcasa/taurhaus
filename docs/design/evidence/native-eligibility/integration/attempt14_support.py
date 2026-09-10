"""Attempt-14 step-7 continuation within attempt 13's remaining budget; reuses attempt-8 helpers."""
import json
from attempt8_continuation_support import wait_receipt, compaction_metering_gaps
from attempt5_support import clean, ledger

PRIOR_TURNS = 12
PRIOR_CONSERVATIVE_USD = .1698456

def enforce_budget(turns, conservative_usd):
    assert turns + PRIOR_TURNS <= 16, 'turn budget reached'
    assert conservative_usd + PRIOR_CONSERVATIVE_USD <= 3, 'cost budget reached'

def retained_log(rows):
    unique={json.dumps(row,sort_keys=True):row for row in rows}
    return list(unique.values()), {'raw_rows':len(rows),'unique_rows':len(unique)}

def validate_compaction(before, after, rows, events, pane):
    assert before['appServer']['threadId']==after['appServer']['threadId'], 'compaction changed thread identity'
    assert before['contextGeneration']!=after['contextGeneration'], 'contextGeneration did not advance'
    for suffix in ['received','delivered']:
        assert any(r.get('event')=='compaction.codex_host.'+suffix for r in rows), 'missing host compaction '+suffix
    assert any(e.get('params',{}).get('item',{}).get('type')=='userMessage' and '[taurhaus] recovery_card' in json.dumps(e) for e in events), 'no recovery card user item'
    assert '[taurhaus] recovery_card' in pane, 'attached pane did not show recovery card'

def pack_events(rows):
    """Intern repeated command/capture payloads without losing timestamps or order."""
    import hashlib
    payloads={}; packed=[]
    for row in rows:
        body={k:v for k,v in row.items() if k!='at'}
        key=hashlib.sha256(json.dumps(body,sort_keys=True).encode()).hexdigest()[:16]
        assert key not in payloads or payloads[key]==body
        payloads[key]=body
        packed.append({'at':row['at'],'payload_ref':key})
    return packed,payloads

def unpack_events(rows,payloads):
    return [{'at':row['at'],**payloads[row['payload_ref']]} for row in rows]


def validate_tmux_activity(snapshot, pane, activity):
    seats = [s for s in snapshot['runtime_sessions']
             if s.get('member_name') == 'seat' and s.get('tmux_pane') == pane]
    assert len(seats) == 1, 'rollback seat not uniquely attributed'
    seat = seats[0]
    assert seat.get('pid') and seat.get('session_id') and seat.get('cli_tool') == 'codex'
    assert seat['state'] == 'idle' and seat['activity_attribution'] == 'attributed'
    assert seat['activity_confidence'] in ['medium', 'high'], 'uncertain rollback activity'
    assert activity.get('source') in ['launch_ready', 'notify'], 'no native idle readiness source'
    return seat


def finalize_metering(ledger):
    result = dict(ledger)
    result['metering_complete'] = not result['unmetered_turn_ids']
    result['actual_billed_usd'] = None
    if not result['metering_complete']:
        result.setdefault('measured_api_equivalent_subtotal_usd', result['api_equivalent_usd'])
        result['api_equivalent_usd'] = None
        result['conservative_usd'] = None
        result['limitation'] = 'Started turn has no retained tokenUsage event; total spend unknown, not zero.'
    return result
