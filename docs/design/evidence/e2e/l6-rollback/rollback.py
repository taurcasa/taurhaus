"""Rollback evidence assertions and bounded lock-refusal handling."""
def retry_transient(command,now,sleep,timeout):
    assert timeout>=60
    end=now()+timeout
    while True:
        code,output=command()
        if code==0 or not any(reason in output.lower() for reason in ('controller busy','lock busy','lock is busy','resource temporarily unavailable')):
            return code,output
        if now()>=end:return code,output
        sleep(min(1,end-now()))

def ownership_boundary(events,config,verified):
    changes=[r for r in events if r.get('eventType')=='delivery_owner_changed' and r.get('new_owner')=='members']
    assert len(changes)==1,'expected exactly one ownership-change boundary to members'
    assert changes[0].get('previous_owner')=='team','unexpected previous owner'
    assert config.get('delivery_owner')=='members' and config.get('delivery_rollback_sha256') and verified,'unverified rollback compatibility'
    return changes[0]

def preserved_message(before,after):
    for key in ('id','message_id','read','ackedAt','ackedBy','delivered'):
        assert before.get(key)==after.get(key),f'rollback changed {key}'
