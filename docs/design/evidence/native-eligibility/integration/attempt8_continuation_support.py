"""Receipt waits and cumulative budget for the user-authorized attempt-8 continuation."""
import time

PRIOR_TURNS = 4
PRIOR_CONSERVATIVE_USD = 0.0550068


def enforce_budget(turns, conservative_usd):
    assert turns + PRIOR_TURNS <= 16, 'turn budget reached'
    assert conservative_usd + PRIOR_CONSERVATIVE_USD <= 3, 'cost budget reached'


def wait_receipt(snapshot, message_id, timeout=100, sleep=time.sleep):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        rows=[r.get('payload',r) for r in snapshot()]
        rows=[r for r in rows if r.get('message_id')==message_id]
        for r in rows:
            if r.get('stage')=='pending' and 'thread_active' in str(r.get('evidence')): return r
        assert not any(r.get('stage')=='native_enqueued' for r in rows), 'enqueued without thread_active evidence'
        sleep(.1)
    raise AssertionError('no thread_active receipt within step deadline')
