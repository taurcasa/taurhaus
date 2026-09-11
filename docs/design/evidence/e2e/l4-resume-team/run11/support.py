"""Run 6 offline guards. Runtime source is the installation resolved by which(codex)."""
from pathlib import Path
import shutil
import shlex
import re


def require_alpha_resume(old, new, launch, activity, events):
    assert not any(r.get('event')=='launch.resume.fallback' for r in events), 'alpha resume fallback observed'
    words=shlex.split(launch.get('command',''))
    assert launch.get('mode')=='resume', 'alpha launch mode must be resume'
    assert any(words[i:i+2]==['resume',old['session_id']] for i in range(len(words)-1)), 'alpha did not resume recorded session'
    assert new.get('session_id') and new['session_id']!=old['session_id'], 'alpha rollout was not rebound'
    attributed=[r for r in activity.get('runtime_sessions',[]) if
                r.get('session_id')==new['session_id'] and r.get('tmux_pane')==new['paneId'] and
                r.get('state')=='idle' and r.get('activity_attribution')=='attributed']
    assert not activity.get('degraded') and len(attributed)==1, 'alpha resumed activity not attributed idle'
    assert any(r.get('event')=='activity.state.changed' and r.get('session_id')==new['session_id'] and
               r.get('pid')==attributed[0]['pid'] and r.get('to')=='idle' and r.get('source')=='notify'
               for r in events), 'alpha resumed pid has no notify-sourced idle'


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
    if reason.startswith('Mesh refusal:') or 'mesh team activation failed' in reason or 'team-daemon' in reason:
        return 'mesh'
    if re.search(r"(?:^|\b)(?:'NoneType' object has no attribute 'startswith'|harness (?:command|warm-up)|auth source|unapproved Codex|(?:input|metered) cap exceeded|missing headroom|unmetered (?:turn|reset token counter)|bwrap:|code-mode-host|Operation not permitted|Permission denied)(?:\b|$)", reason):
        return 'harness'
    return 'taurhaus'


def reconciled_spend(ledger):
    complete=not ledger['unmetered']
    return {'total_usd':ledger['api_equivalent_usd'] if complete else None,
            'api_equivalent_subtotal_usd':ledger['api_equivalent_usd'],
            'all_output_rate_subtotal_usd':ledger['conservative_usd'],
            'cap_verified':complete and ledger['api_equivalent_usd']<=.25,
            'unmetered_turns':ledger['unmetered'],
            'note':'Unknown total is never zero spend. API-equivalent rates are not an invoice.'}


def observe_host(read, diagnostic):
    """A busy read is missing observation, never a failed turn or a paid retry."""
    try:
        return read()
    except RuntimeError as error:
        if not is_busy(error):
            raise
        diagnostic(str(error))
        return None


def rollout_usage(rows, session, diagnostic=lambda row: None):
    """Accumulate turn subtotals across process counter epochs in one rollout.

    A resumed process may reset even mid-turn. Retain the previous epoch's
    subtotal and count the new epoch's observed total once (including a nonzero
    first sample); rebase subsequent differences to that sample. Replaying the
    complete file produces identical totals and stable diagnostic identities.
    """
    current=None; previous={}; baseline={}; subtotal={}; turns={}; epoch=0
    keys={'input':'input_tokens','cached_input':'cached_input_tokens','output':'output_tokens'}
    for index,row in enumerate(rows):
        if row.get('type')!='event_msg': continue
        p=row.get('payload',{})
        if p.get('type')=='task_started':
            current=p.get('turn_id'); baseline=previous.copy(); subtotal={}
        if p.get('type')!='token_count' or not p.get('info'): continue
        total=p['info'].get('total_token_usage')
        if not total: continue
        if any(total.get(k,0)<previous.get(k,0) for k in keys.values()):
            epoch+=1
            diagnostic({'kind':'counter_epoch_reset','session_id':session,'turn_id':current,
                        'row':index,'epoch':epoch,'previous':previous,'observed':total})
            subtotal={out:turns.get(current,{}).get(out,0)+total.get(key,0) for out,key in keys.items()}
            baseline=total.copy()
        if current:
            turns[current]={'turn_id':current,'session_id':session,'observer':'rollout-cumulative',
                            **{out:subtotal.get(out,0)+total.get(key,0)-baseline.get(key,0) for out,key in keys.items()}}
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


def require_stopped_identity(old, stopped, ready):
    assert ready.get('session_id') and stopped.get('session_id')==ready['session_id'], 'alpha stopped session identity lost'
    assert ready.get('jsonl_path') and stopped.get('jsonl_path')==ready['jsonl_path'], 'alpha stopped rollout path lost'
    for key in ['paneId','panePid','paneStartTime']:
        assert stopped.get(key)==old.get(key), 'alpha stopped pane binding lost: '+key
    assert stopped.get('health')=='sessionDead', 'alpha stopped health not sessionDead'
    assert stopped.get('daemon_pid') is None, 'alpha stopped daemon_pid retained'


def pending_deliveries(records, member):
    accepted={r['payload']['message_id'] for r in records if r.get('event_type')=='message_accepted'
              and any(t.get('recipient')==member for t in r['payload'].get('delivery_targets',[]))}
    completed={r['payload'].get('message_id') for r in records if r.get('event_type')=='receipt'
               and r['payload'].get('recipient')==member
               and (r['payload'].get('stage') in {'submitted','native_enqueued','consumed_by_read'}
                    or r['payload'].get('kind')=='consumed_by_read')}
    return sorted(accepted-completed)
