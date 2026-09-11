"""Pure continuation admission: prior trial inputs and spend remain charged."""
def cumulative(prior,inputs,estimate,upper,observed_claude=0):
    # Run 6 ruling: transcript rows meter usage, never controller submissions.
    counts=prior['cap_inputs'] if 'cap_inputs' in prior else prior['controller_inputs']
    return {'controller_inputs':{k:inputs[k]+counts[k] for k in ('claude','codex')},'seat_usd_estimate':estimate+prior['seat_usd_estimate'],'seat_usd_upper_rate':upper+prior['seat_usd_upper_rate']}

def compact_hook_registered(hooks):
    return any(entry.get('matcher')=='compact' and any(h.get('type')=='command' and h.get('command') for h in entry.get('hooks',[])) for entry in hooks.get('SessionStart',[]))

def codex_bundle(binary):
    files=[binary, binary.with_name('codex-code-mode-host')]
    for path in files:
        if not path.is_file() or path.is_symlink():
            raise ValueError(f'missing regular Codex runtime component: {path.name}')
    return files

def submit_input(run, sleep, pane, text):
    run(['tmux','send-keys','-t',pane,'-l',text])
    sleep(1)
    run(['tmux','send-keys','-t',pane,'Enter'])

def codex_ready(rows):
    active=set(); completed=False
    for row in rows:
        if row.get('type')!='event_msg': continue
        payload=row.get('payload',{})
        if payload.get('type')=='task_started': active.add(payload['turn_id'])
        if payload.get('type')=='task_complete':
            active.discard(payload['turn_id']); completed=True
    return completed and not active

def installed_codex_bundle(which):
    from pathlib import Path
    launcher = which('codex')
    if not launcher:
        raise ValueError('Codex installation unavailable')
    package = Path(launcher).resolve().parents[1]
    candidates = list(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
    if len(candidates) != 1:
        raise ValueError('expected exactly one native Codex bundle')
    return codex_bundle(candidates[0])

def codex_submitted(pane, rows, prior_turns):
    composers = [line.strip()[1:].strip() for line in pane.splitlines() if line.strip().startswith('›')]
    empty = bool(composers) and composers[-1] in ('', 'Ask Codex to do anything')
    started = {r.get('payload', {}).get('turn_id') for r in rows
               if r.get('type') == 'event_msg' and r.get('payload', {}).get('type') == 'task_started'}
    return empty and bool(started - prior_turns)

def opportunity_deadline(now, outer_deadline, seconds=65):
    if seconds < 60 or now + seconds > outer_deadline:
        raise ValueError('outer deadline cannot provide full opportunity')
    return now + seconds

def alpha_attributed_idle(record, activity):
    return bool(record.get('session_id') and record.get('jsonl_path')
                and activity.get('activity_confidence') == 'idle')

def explicit_lead_read(rows):
    return any(r.get('payload', {}).get('kind') == 'consumed_by_read'
               and r['payload'].get('reader_name') == 'lead'
               and r['payload'].get('context') == 'explicit-mesh-cli' for r in rows)


def wait_lead_identity(read, now, sleep, timeout=65):
    """Allow the scanner a full window after initialize before claiming absence."""
    deadline = opportunity_deadline(now(), float('inf'), timeout)
    while True:
        record = read() or {}
        if record.get('session_id') and record.get('paneId') == '%1':
            return record
        if now() >= deadline:
            raise TimeoutError('lead session attribution absent after full observation window')
        sleep(1)
