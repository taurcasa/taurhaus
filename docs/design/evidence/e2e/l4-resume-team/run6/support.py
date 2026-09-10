"""Run 6 offline guards. Runtime source is the installation resolved by which(codex)."""
from pathlib import Path
import shutil


def copy_native_runtime(entry, destination):
    package=Path(entry).resolve().parents[1]
    candidates=list(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
    assert len(candidates)==1, 'expected one installed native Codex runtime'
    native=candidates[0].parent
    sources=[native/name for name in ['codex','codex-code-mode-host']]
    assert all(p.is_file() for p in sources), 'native runtime sibling missing'
    for source in sources:
        target=destination/source.name
        shutil.copyfile(source,target); target.chmod(0o700)


def sanitize_log_rows(records):
    return [r for r in records if not r.get('event','').startswith('usage.')]


def classify_failure(reason):
    if any(s in reason for s in ['harness command','auth source','unapproved Codex','cap','headroom','metered','bwrap:', 'code-mode-host', 'Operation not permitted', 'Permission denied']):
        return 'harness'
    return 'mesh' if 'Mesh refusal' in reason or 'mesh team activation failed' in reason or 'team-daemon' in reason else 'taurhaus'


def reconciled_spend(ledger):
    complete=not ledger['unmetered']
    return {'total_usd':ledger['conservative_usd'] if complete else None,
            'metered_subtotal_usd':ledger['conservative_usd'],
            'cap_verified':complete and ledger['conservative_usd']<=.25,
            'unmetered_turns':ledger['unmetered'],
            'note':'Unknown total is never zero spend. Raw meter values are metered subtotals only.'}


def observe_host(read, diagnostic):
    """A busy read is missing observation, never a failed turn or a paid retry."""
    try:
        return read()
    except RuntimeError as error:
        if not is_busy(error):
            raise
        diagnostic(str(error))
        return None


def rollout_usage(rows, session):
    """Cumulative session counters include all tool-response segments in each turn."""
    current=None; previous={}; baseline={}; turns={}
    keys={'input':'input_tokens','cached_input':'cached_input_tokens','output':'output_tokens'}
    for row in rows:
        if row.get('type')!='event_msg': continue
        p=row.get('payload',{})
        if p.get('type')=='task_started':
            current=p.get('turn_id'); baseline=previous.copy()
        if p.get('type')!='token_count' or not p.get('info'): continue
        total=p['info'].get('total_token_usage')
        if not total: continue
        if current:
            assert all(total.get(k,0)>=baseline.get(k,0) for k in keys.values()), 'unmetered reset token counter'
            turns[current]={'turn_id':current,'session_id':session,'observer':'rollout-cumulative',
                            **{out:total.get(key,0)-baseline.get(key,0) for out,key in keys.items()}}
        previous=total.copy()
    return list(turns.values())


def is_busy(error):
    return any(reason in str(error).lower() for reason in ['host member busy','lock busy'])


def retry_busy(call, diagnostic, *, clock=None, sleep=None):
    """Retry only explicit transient refusals; no accepted operation or paid input."""
    import time
    clock=clock or time.monotonic; sleep=sleep or time.sleep
    deadline=clock()+100
    while True:
        try: return call()
        except RuntimeError as error:
            if not is_busy(error) or clock()>=deadline: raise
            diagnostic(str(error)); sleep(1)


def seat_process(process):
    argv=process.get('argv',[])
    name=Path(argv[0]).name if argv else ''
    return name.startswith('codex') or name=='claude'


def stopped_backlog(records, message_id, member, projection, health):
    matching=[r for r in records if r.get('payload',{}).get('message_id')==message_id]
    accepted=any(r.get('event_type')=='message_accepted' and
                 any(t.get('recipient')==member for t in r['payload'].get('delivery_targets',[]))
                 for r in matching)
    forbidden={'submitted','consumed','native_enqueued','consumed_by_read'}
    presented=any(r.get('payload',{}).get('stage') in forbidden or
                  r.get('payload',{}).get('kind') in forbidden for r in matching)
    reason=health.get('last_defer_reason') or ''
    deferred=any(s in reason for s in ['pending: runtime session dead','pending: native_host_not_live'])
    return accepted and projection=='pending' and not presented and health.get('member')==member and deferred


def require_headroom(ledger, inputs):
    """Fresh run6 cap is metered; retain the all-output-rate estimate separately."""
    assert ledger['paid_inputs']+inputs<=16, 'input cap'
    assert not ledger['unmetered'], 'unmetered turns before next paid input'
    assert ledger['api_equivalent_usd']+.05*inputs<=.25, 'cost headroom'
