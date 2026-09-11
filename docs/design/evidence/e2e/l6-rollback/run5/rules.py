"""Pure run4 assertions against Mesh's canonical escape-route contract."""
def downgrade_boundary(config,authority,before,after,format_verified,rollback_verified,projection,rollback,message_id):
    assert config.get('messaging_format')==0,'required resulting format 0 not observed'
    assert config.get('delivery_owner')=='members','downgrade did not commit members ownership'
    assert config.get('delivery_rollback_sha256') and rollback_verified,'unverified rollback compatibility'
    assert authority.get('transition')=='complete','format boundary not committed'
    assert authority.get('legacy_cut')==config.get('messaging_downgrade_sha256') and format_verified,'format report digest not verified'
    assert before and after==before,'canonical history not retained unchanged'
    assert projection and projection.get('message_id')==message_id and projection.get('read') is False,'B not preserved pending/unread under its logical id'
    matches=[r for r in rollback if r.get('delivery_id')==projection['id'] and r.get('recipient')=='alpha']
    assert len(matches)==1 and matches[0].get('stage')=='pending','B pending rollback disposition absent'

def same_owner_commit(before,after,code,verified):
    assert code==0,'same-owner rollback refused'
    assert before.get('owner-stopped') and after.get('owner-stopped') is None,'owner stop marker was not cleared'
    assert verified,'rollback digest mismatch'
    for key in ('delivery_owner','delivery_rollback_sha256'):
        assert before['config'].get(key)==after['config'].get(key),'same-owner command changed '+key
    assert after['config'].get('delivery_owner')=='members' and after['config'].get('delivery_rollback_sha256')
    # No new request/epoch is needed: format's atomic write already set members.

def attached_executor(record,processes,binary,digest,team):
    def value(argv,flag):return argv[argv.index(flag)+1] if flag in argv else None
    candidates=[r for r in processes if len(r['argv'])>1 and r['argv'][0]==binary and r['argv'][1]=='daemon' and value(r['argv'],'--name')=='alpha' and value(r['argv'],'--team')==team]
    assert len(candidates)<=1,'duplicate alpha member executors'
    if not candidates:return None
    row=candidates[0]
    assert row.get('namespace_pid')==record.get('daemon_pid'),'runtime daemon pid not attached'
    assert value(row['argv'],'--pane')==record.get('paneId'),'executor attached to wrong pane'
    assert row.get('sha256')==digest,'member executor is not the lane RC binary'
    return row

def startup_delivery(journal,native):
    """Shared harness contract, used offline to diagnose the executed readiness gate."""
    accepted=[r['payload'] for r in journal if r.get('event_type')=='message_accepted' and '[taurhaus] recovery_card' in r.get('payload',{}).get('body','') and any(t.get('recipient')=='alpha' for t in r['payload'].get('delivery_targets',[]))]
    for card in accepted:
        receipts=[r.get('payload',{}) for r in journal if r.get('payload',{}).get('message_id')==card['message_id']]
        if any(r.get('kind')=='consumed_by_read' and r.get('reader_name')=='alpha' for r in receipts):return card['message_id']
        submitted=any(r.get('stage')=='submitted' and r.get('recipient')=='alpha' for r in receipts)
        tool=any(r.get('type')=='response_item' and r.get('payload',{}).get('type') in ('function_call_output','custom_tool_call_output') and card['body'] in str(r['payload']) for r in native)
        if submitted and tool:return card['message_id']
    return None

def input_count(meter):
    return max(meter['paid_inputs'],meter['rollout_turns'])
